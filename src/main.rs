#[macro_use]
extern crate dotenv_codegen;

use esp_idf_sys as _; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use std::{
    thread::sleep,
    time::Duration,
};
use esp_idf_svc::{
    nvs::EspDefaultNvsPartition,
    eventloop::EspSystemEventLoop,
};
use esp_idf_hal::peripherals::Peripherals;
use accelerometer::vector::F32x3;

mod wireless;
mod led;
mod sensors;
mod moving_average;

use wireless::Wireless;
use led::LED;
use sensors::Sensors;
use moving_average::MovingAverage;

fn main() {
    esp_idf_sys::link_patches();//Needed for esp32-rs
    println!("Entered Main function!");

    let peripherals         = Peripherals::take().unwrap();
    let sys_loop   = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let wireless = Wireless::new(
        peripherals.modem,
        sys_loop,
        nvs,
    );

    let mut led = LED::new();

    wireless.print_ip_info();

    let mut sensors = Sensors::new(
        peripherals.i2c0,
        peripherals.pins.gpio6, // SDA
        peripherals.pins.gpio7, // SCL
    );

    let mut deltas = MovingAverage::new();
    let mut current_vector_len = 0.1;
    let mut last_vector_len;

    loop {
        last_vector_len = current_vector_len;

        current_vector_len = get_vector_length(sensors.get_accelerometer_reading());
        deltas.add_value((current_vector_len - last_vector_len).abs());

        let mut hue = (120.0 - deltas.get_average() * 5.0 * 120.0).round();

        if hue > 120.0 {
            hue = 120.0;
        } else if hue < 0.0 {
            hue = 0.0;
        }

        println!("{:?}", hue);
        led.fill_hue(hue);

        sleep(Duration::from_millis(8));
    }
}

fn get_vector_length(xyz: F32x3) -> f32 {
    (
        f32::powf(xyz.x, 2.0)
        + f32::powf(xyz.y, 2.0)
        + f32::powf(xyz.z, 2.0)
    ).sqrt()
}
