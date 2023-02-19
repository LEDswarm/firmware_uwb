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
use smart_leds_trait::{SmartLedsWrite, White};
use ws2812_esp32_rmt_driver::driver::color::LedPixelColorGrbw32;
use ws2812_esp32_rmt_driver::{LedPixelEsp32Rmt, RGBW8};
use colorsys::{Rgb, Hsl, ColorAlpha};

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

    let led_pin = 20;
    let mut ws2812 = LedPixelEsp32Rmt::<RGBW8, LedPixelColorGrbw32>::new(0, led_pin).unwrap();
/*
    let mut display = Display::new(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
    );
    display.render_raw(ImageRaw::new(include_bytes!("./rust.raw"), 64));
    sleep(Duration::new(5,0));
    display.clear();
*/
    /*loop{
        println!("IP info: {:?}", wifi_driver.sta_netif().get_ip_info().unwrap());
        sleep(Duration::new(10,0));
    }*/

    wireless.print_ip_info();
    let mut hue = 0.0;

    let brightness = 0.05;

    loop {
        let mut hsl = Hsl::default();
        // Hsl { h: 0, s: 0, l: 0, a: 1 }
        hsl.set_saturation(100.0);
        hsl.set_lightness(50.0);
        hsl.set_hue(hue);
        let rgb = Rgb::from(&hsl);
        let pixels = std::iter::repeat(
            RGBW8::from((
                (rgb.red() * brightness) as u8,
                (rgb.green() * brightness) as u8,
                (rgb.blue() * brightness) as u8,
                White(0),
            ))
        ).take(7);
        ws2812.write(pixels).unwrap();
        // wireless.print_ip_info();

        if hue < 360.0 {
            hue += 1.0;
        } else {
            hue = 0.0;
        }

        sleep(Duration::from_millis(20));
    }
}
