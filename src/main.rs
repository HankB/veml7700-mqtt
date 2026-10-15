use hostname::get;
use linux_embedded_hal::I2cdev;
use rumqttc::{Client, MqttOptions, QoS};
use serde::Serialize;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use veml7700::{Gain, IntegrationTime, Veml7700};

const MQTT_HOST: &str = "pilog3b";
const MQTT_PORT: u16 = 1883;

const REPORT_CHANGE: f32 = 0.10; // 10%
const MIN_CHANGE_LUX: f32 = 0.01;
const MAX_REPORT_INTERVAL: Duration = Duration::from_secs(15);

const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const SENSOR_STARTUP_DELAY: Duration = Duration::from_secs(2);
const RETRY_DELAY: Duration = Duration::from_secs(5);

#[derive(Serialize)]
struct LightingMessage {
    t: u64,
    level: f32,
    device: &'static str,
}

fn main() {
    //
    // Determine our hostname and construct the MQTT topic.
    //
    let hostname = get()
        .expect("Failed to get hostname")
        .into_string()
        .expect("Hostname is not valid UTF-8");

    let topic = format!("HA/{}/kiosk/lighting", hostname);

    println!("Publishing to MQTT topic: {}", topic);

    //
    // Set up MQTT.
    //
    let mut mqtt_options =
        MqttOptions::new("veml7700-mqtt", MQTT_HOST, MQTT_PORT);

    mqtt_options.set_keep_alive(Duration::from_secs(30));

    let (client, mut connection) = Client::new(mqtt_options, 10);

    //
    // The rumqttc connection event loop must continue running for
    // MQTT traffic to be processed.
    //
    thread::spawn(move || {
        for notification in connection.iter() {
            if let Err(error) = notification {
                eprintln!(
                    "MQTT connection error: {}. Retrying in {:?}",
                    error,
                    RETRY_DELAY
                );
                thread::sleep(RETRY_DELAY); // not 
            }
        }
    });

    //
    // Set up the VEML7700.
    //
    let i2c = I2cdev::new("/dev/i2c-1")
        .expect("Failed to open /dev/i2c-1");

    let mut sensor = Veml7700::new(i2c);

    sensor.set_gain(Gain::Two)
        .expect("Failed to set VEML7700 gain");

    sensor.set_integration_time(IntegrationTime::_800ms)
        .expect("Failed to set VEML7700 integration time");

    sensor.enable()
        .expect("Failed to enable VEML7700");

    //
    // The first reading was observed to be 0.0 with a shorter delay.
    //
    thread::sleep(SENSOR_STARTUP_DELAY);

    //
    // Reporting state.
    //
    let mut last_reported: Option<f32> = None;
    let mut last_report_time: Option<Instant> = None;

    loop {
        let lux = sensor.read_lux()
            .expect("Failed to read VEML7700");

        let should_report = match last_reported {
            //
            // Always report the first valid reading.
            //
            None => true,

            Some(previous) => {
                //
                // Use a percentage of the last reported value, but
                // never allow the threshold to fall below the
                // minimum absolute change.
                //
                let threshold =
                    (previous * REPORT_CHANGE).max(MIN_CHANGE_LUX);

                let changed_enough =
                    (lux - previous).abs() > threshold;

                let report_is_due = last_report_time
                    .map(|time| time.elapsed() >= MAX_REPORT_INTERVAL)
                    .unwrap_or(true);

                changed_enough || report_is_due
            }
        };

        if should_report {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System clock is before Unix epoch")
                .as_secs();

            let message = LightingMessage {
                t: timestamp,
                level: lux,
                device: "VEML7700",
            };

            let payload = serde_json::to_string(&message)
                .expect("Failed to serialize MQTT payload");

            println!("{} {}", topic, payload);

            client.publish(
                &topic,
                QoS::AtLeastOnce,
                true, // retain the last message
                payload,
            )
            .expect("Failed to publish MQTT message");

            //
            // Change detection is relative to the value we most
            // recently reported, not the previous sensor reading.
            //
            last_reported = Some(lux);
            last_report_time = Some(Instant::now());
        }

        thread::sleep(SAMPLE_INTERVAL);
    }
}