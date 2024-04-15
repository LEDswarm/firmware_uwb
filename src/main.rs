//! ![banner](https://ledswarm-book.s3.nl-ams.scw.cloud/Slim_LEDswarm_Banner2.svg)
//!
//! The official firmware for ESP32-based LEDswarm controller boards.
//! 
//! This crate implements a main loop along with several peripheral threads which share memory through communicating. All of the threads communicate via channels with the main loop,
//! using the [`InternalMessage`] enum from the `ledswarm_protocol` library to exchange commands and data packets.

use colored::*;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::netif::{EspNetif, NetifStack};
use esp_idf_svc::wifi::{WifiDriver, EspWifi, AsyncWifi};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition
};

use esp_idf_svc::timer::EspTaskTimerService;
use serde::{Deserialize, Serialize};

use ledswarm_protocol::{Frame, InternalMessage};

//pub mod display;
pub mod configuration;
pub mod controller;
pub mod led;
pub mod network;
pub mod imu;
pub mod moving_average;
pub mod util;
pub mod server;
pub mod uwb;
pub mod event_bus;

use controller::Controller;

use std::sync::mpsc;

pub const STACK_SIZE: usize = 10240;

#[derive(Serialize, Deserialize)]
/// A JSON document at the server root `/` which provides basic information about the configuration of the master node.
struct RootDocument {
    version: String,
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("{}", util::LOGO);

    let peripherals = Peripherals::take().unwrap();
    let timer = EspTaskTimerService::new().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();


    println!("## {}  Initializing Wi-Fi ...", "[LEDswarm]".yellow().bold());
    let wifi_driver = WifiDriver::new(peripherals.modem, sys_loop.clone(), Some(nvs.clone())).unwrap();

    let wifi = EspWifi::wrap_all(
        wifi_driver,
        EspNetif::new(NetifStack::Sta).unwrap(),
        EspNetif::new(NetifStack::Ap).unwrap(),
    ).unwrap();

    let wifi = AsyncWifi::wrap(wifi, sys_loop.clone(), timer.clone()).unwrap();
    let (msg_tx, msg_rx): (flume::Sender<InternalMessage>, flume::Receiver<InternalMessage>)  = flume::bounded(512);
    let (uwb_out_tx, uwb_out_rx): (flume::Sender<Frame>, flume::Receiver<Frame>)     = flume::bounded(512);

    println!("{}  Initializing controller ...", "[LEDswarm]".yellow().bold());
    let mut controller = Controller::new(msg_rx, uwb_out_tx);
    println!("{}  Starting controller Wi-Fi ...", "[LEDswarm]".yellow().bold());
    futures::executor::block_on(controller.init_wifi(wifi))?;

    println!("{}  Creating server endpoints ...", "[LEDswarm]".yellow().bold());
    server::create_endpoints(msg_tx.clone())?;
    println!("{}  Starting controller IMU ...", "[LEDswarm]".yellow().bold());

    /*
        UWB
    */
    let spi = peripherals.spi3;

    let serial_out = peripherals.pins.gpio23; // MISO
    let serial_in = peripherals.pins.gpio19; // MOSI
    let sclk = peripherals.pins.gpio18;
    let cs = peripherals.pins.gpio4; // CS

    // TODO: make sure the buffer is always consumed to prevent memory leaks!!!
    let accel_tx = msg_tx.clone();
    imu::start(accel_tx, peripherals.i2c0, peripherals.pins.gpio21, peripherals.pins.gpio22)?;

    println!("{}  Launched IMU thread", "[LEDswarm]".yellow().bold());

    // Use 8K stack size for UWB thread to prevent overflow
    std::thread::Builder::new().stack_size(8192).spawn(move || {
        uwb::start(
            msg_tx.clone(),
            uwb_out_rx,
            spi,
            serial_out,
            serial_in,
            sclk,
            cs,
            peripherals.pins.gpio34,
            peripherals.pins.gpio27,
        ).expect("Failed to initialize UWB");
    })?;

    println!("{}  Launched UWB thread", "[LEDswarm]".yellow().bold());

    println!("{}  Starting controller event loop", "[LEDswarm]".yellow().bold());

    controller.start_event_loop().expect("Failed to start controller event loop"); 

    Ok(())
}
