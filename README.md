# veml7700-mqtt

Read the VEML7700 light sensor and publish using MQTT in Rust.

## first build

(Not including "Hello World")

```text
cargo add veml7700
cargo add linux-embedded-hal
cargo build
cargo add veml7700 --features lux_as_f32
cargo build --release
```

## Errata

* Check for presence of the sensor using `i2cdetect -y 1` (and don't overlook the `10` in the second row, first column.)

```text
hbarta@pi64util:~ $ i2cdetect -y 1
     0  1  2  3  4  5  6  7  8  9  a  b  c  d  e  f
00:                         -- -- -- -- -- -- -- -- 
10: 10 -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- 
20: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- 
30: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- 
40: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- 
50: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- 
60: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- 
70: -- -- -- -- -- -- -- --                         
hbarta@pi64util:~ $
```

* Development is on a Pi CM5 (`rafiki`) with full desktop and testing on a headless Pi 3B (`pi64util`.) Copy code to the test target using:

```text
scp ./target/debug/veml7700-mqtt pi64util:Downloads/
scp ./target/release/veml7700-mqtt pi64util:Downloads/
```