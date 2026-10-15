use hostname::get;
use linux_embedded_hal::I2cdev;
use rumqttc::{Client, Event, Incoming, MqttOptions, QoS};
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
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
    // Connection state is shared between the MQTT event-loop thread
    // and the sensor/reporting thread.
    //
    let mqtt_connected = Arc::new(AtomicBool::new(false));
    let connection_state = Arc::clone(&mqtt_connected);

    //
    // Drive the MQTT connection.
    //
    thread::spawn(move || {
        for notification in connection.iter() {
            match notification {
                //
                // CONNACK means the MQTT connection has been
                // successfully established with the broker.
                //
                Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                    connection_state.store(true, Ordering::Relaxed);
                    println!("MQTT connected");
                }

                //
                // Other MQTT events don't affect our connection
                // state.
                //
                Ok(_) => {}

                //
                // Any connection error marks MQTT unavailable.
                //
                Err(error) => {
                    connection_state.store(false, Ordering::Relaxed);

                    eprintln!(
                        "MQTT connection error: {}. Retrying in {:?}",
                        error,
                        RETRY_DELAY
                    );

                    thread::sleep(RETRY_DELAY);
                }
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
    // A shorter delay was observed to produce an initial 0.0
    // reading on the test hardware.
    //
    thread::sleep(SENSOR_STARTUP_DELAY);

    //
    // Reporting state. These represent the last reading we actually
    // submitted for publication while MQTT was connected.
    //
    let mut last_reported: Option<f32> = None;
    let mut last_report_time: Option<Instant> = None;

    loop {
        let lux = sensor.read_lux()
            .expect("Failed to read VEML7700");

        let should_report = match last_reported {
            //
            // Always report the first reading once MQTT is available.
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

        //
        // Do not queue measurements while MQTT is disconnected.
        //
        // Because last_reported and last_report_time are not updated
        // in that case, the current value will be eligible for
        // publication as soon as MQTT reconnects.
        //
        if should_report && mqtt_connected.load(Ordering::Relaxed) {
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

            match client.publish(
                &topic,
                QoS::AtLeastOnce,
                true, // retain latest lighting state
                payload,
            ) {
                Ok(()) => {
                    last_reported = Some(lux);
                    last_report_time = Some(Instant::now());
                }

                Err(error) => {
                    //
                    // Do not update reporting state. The current
                    // reading has not even been accepted into the
                    // MQTT request queue.
                    //
                    eprintln!("Failed to queue MQTT publish: {}", error);
                }
            }
        }

        thread::sleep(SAMPLE_INTERVAL);
    }
}