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
use embedded_graphics::image::ImageRaw;

mod display;
mod wireless;

use display::Display;
use wireless::Wireless;

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

    let mut display = Display::new(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
    );
    display.render_raw(ImageRaw::new(include_bytes!("./rust.raw"), 64));
    sleep(Duration::new(5,0));
    display.clear();

    /*loop{
        println!("IP info: {:?}", wifi_driver.sta_netif().get_ip_info().unwrap());
        sleep(Duration::new(10,0));
    }*/

    loop {
        wireless.print_ip_info();
        sleep(Duration::new(5,0));
    }
}
