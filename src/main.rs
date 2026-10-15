use linux_embedded_hal::I2cdev;
use std::thread;
use std::time::Duration;
use veml7700::Veml7700;

fn main() {
    let i2c = I2cdev::new("/dev/i2c-1")
        .expect("Failed to open /dev/i2c-1");

    let mut sensor = Veml7700::new(i2c);

    sensor.enable()
        .expect("Failed to enable VEML7700");

    // Datasheet/driver requires at least 4 ms after enabling.
    thread::sleep(Duration::from_millis(10));

    loop {
        let lux = sensor.read_lux()
            .expect("Failed to read VEML7700");

        println!("Lux: {:.2}", lux);

        thread::sleep(Duration::from_secs(2));
    }
}