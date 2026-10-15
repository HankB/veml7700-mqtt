use hostname::get;
use linux_embedded_hal::I2cdev;
use rumqttc::{Client, MqttOptions, QoS};
use serde::Serialize;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use veml7700::{Gain, IntegrationTime, Veml7700};

const MQTT_HOST: &str = "pilog3b";
const MQTT_PORT: u16 = 1883;

#[derive(Serialize)]
struct LightingMessage {
    t: u64,
    level: f32,
    device: &'static str,
}

fn main() {
    //
    // Determine our hostname and MQTT topic.
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
    // rumqttc's connection event loop must keep running.
    //
    thread::spawn(move || {
        for notification in connection.iter() {
            if let Err(error) = notification {
                eprintln!("MQTT connection error: {}", error);
            }
        }
    });

    //
    // Set up VEML7700.
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

    thread::sleep(Duration::from_secs(2));
    
    //
    // Read and publish.
    //
    loop {
        let lux = sensor.read_lux()
            .expect("Failed to read VEML7700");

        let level = lux;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System clock is before Unix epoch")
            .as_secs();

        let message = LightingMessage {
            t: timestamp,
            level,
            device: "VEML7700",
        };

        let payload = serde_json::to_string(&message)
            .expect("Failed to serialize MQTT payload");

        println!("{} {}", topic, payload);

        client.publish(
            &topic,
            QoS::AtLeastOnce,
            false,
            payload,
        )
        .expect("Failed to publish MQTT message");

        thread::sleep(Duration::from_secs(2));
    }
}