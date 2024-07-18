//! ![banner](https://ledswarm-book.s3.nl-ams.scw.cloud/Slim_LEDswarm_Banner2.svg)
//!
//! The official firmware for ESP32-based LEDswarm controller boards.
//! 
//! This crate implements a main loop along with several peripheral threads which share memory through communicating. All of the threads communicate via channels with the main loop,
//! using the [`InternalMessage`] enum from the `ledswarm_protocol` library to exchange commands and data packets.

use colored::*;
use esp_idf_hal::sys::EspError;
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

fn initialize_esp32_wifi<'a>(
    modem: esp_idf_hal::modem::Modem,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    timer: EspTaskTimerService,
) -> Result<AsyncWifi<EspWifi<'a>>, EspError> {
    println!("## {}  Initializing Wi-Fi ...", "[LEDswarm]".yellow().bold());

    let wifi_driver = WifiDriver::new(modem, sys_loop.clone(), Some(nvs.clone()))?;

    let wifi = EspWifi::wrap_all(
        wifi_driver,
        EspNetif::new(NetifStack::Sta).unwrap(),
        EspNetif::new(NetifStack::Ap).unwrap(),
    ).unwrap();

    Ok(AsyncWifi::wrap(wifi, sys_loop.clone(), timer.clone()).unwrap())
}

fn init_firmware() -> anyhow::Result<()> {
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

    let wifi = initialize_esp32_wifi(peripherals.modem, sys_loop.clone(), nvs.clone(), timer.clone())?;

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
    // imu::start(accel_tx, peripherals.i2c0, peripherals.pins.gpio21, peripherals.pins.gpio22)?;

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

    // Configure and Initialize Timer Drivers
    let config = esp_idf_hal::timer::config::Config::new();
    let mut timer1 = esp_idf_hal::timer::TimerDriver::new(peripherals.timer00, &config).unwrap();
    let mut timer2 = esp_idf_hal::timer::TimerDriver::new(peripherals.timer10, &config).unwrap();

    // Set Counter Start Value to Zero
    timer1.set_counter(0_u64).unwrap();
    timer2.set_counter(0_u64).unwrap();

    // Enable Counter
    timer1.enable(true).unwrap();
    timer2.enable(true).unwrap();


    println!("{}  Launched UWB thread", "[LEDswarm]".yellow().bold());

    println!("{}  Starting controller event loop", "[LEDswarm]".yellow().bold());

    controller.start_event_loop(timer1).expect("Failed to start controller event loop"); 

    Ok(())
}

fn main() -> anyhow::Result<()> {
    init_firmware()
}
