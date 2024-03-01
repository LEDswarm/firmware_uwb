//! ![banner](https://ghoust.s3.fr-par.scw.cloud/ledswarm_banner.svg)
//!
//! The official firmware for ESP32-based LEDswarm controller boards.
//! 
//!

use colored::*;
use esp_idf_hal::gpio::{InterruptType, PinDriver};
use esp_idf_hal::spi::config::{Mode, Phase, Polarity};
use esp_idf_hal::spi::{config, SpiDeviceDriver, SpiDriver, SpiDriverConfig, SPI2};
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::netif::{EspNetif, NetifStack};
use esp_idf_svc::wifi::{WifiDriver, EspWifi, AsyncWifi};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition
};
use esp_idf_hal::gpio::Pull;
use esp_idf_svc::timer::EspTaskTimerService;
use serde::{Deserialize, Serialize};

use dw3000_ng::{
    configs::{BitRate, Config, PreambleLength, PulseRepetitionFrequency, SfdSequence, StsLen, StsMode, UwbChannel},
    DW3000,
    block,
};

use ledswarm_protocol::Message;

//pub mod display;
pub mod configuration;
pub mod controller;
pub mod led;
pub mod network;
pub mod imu;
pub mod moving_average;
pub mod util;
pub mod server;

use controller::Controller;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

pub const STACK_SIZE: usize = 10240;

static WAS_INTERRUPT_TRIGGERED: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize)]
/// A JSON document at the server root `/` which validates a successful connection and provides some information about the server.
struct RootDocument {
    version: String,
}

fn gpio_int_callback() {
    // Assert FLAG indicating a press button happened
    WAS_INTERRUPT_TRIGGERED.store(true, Ordering::Relaxed);
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("{}", util::LOGO);

    let delay = esp_idf_hal::delay::Delay::new_default();

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
    let (msg_tx, msg_rx): (mpsc::Sender<Message>, mpsc::Receiver<Message>) = mpsc::channel();

    let accel_tx = msg_tx.clone();

    println!("## {}  Initializing controller ...", "[LEDswarm]".yellow().bold());
    let mut controller = Controller::new(msg_rx);
    println!("## {}  Starting controller Wi-Fi ...", "[LEDswarm]".yellow().bold());
    futures::executor::block_on(controller.init_wifi(wifi))?;

    println!("## {}  Creating server endpoints ...", "[LEDswarm]".yellow().bold());
    server::create_endpoints(msg_tx.clone())?;
    println!("## {}  Starting controller IMU ...", "[LEDswarm]".yellow().bold());
    imu::start(accel_tx, peripherals.i2c0, peripherals.pins.gpio21, peripherals.pins.gpio22)?;

    println!("\n\n--------->   Initializing SPI\n\n");

    //
    //  DW3000 SPI
    //


    let spi = peripherals.spi2;
/*
    let serial_out = peripherals.pins.gpio19; // MISO
    let serial_in = peripherals.pins.gpio23; // MOSI
    let sclk = peripherals.pins.gpio18;
    let cs = peripherals.pins.gpio4; // CS
*/
    let serial_out = peripherals.pins.gpio12; // MISO
    let serial_in = peripherals.pins.gpio13;  // MOSI
    let sclk = peripherals.pins.gpio14;
    let cs = peripherals.pins.gpio15; // CS

    let config = config::Config::new()
        .baudrate(5.MHz().into())
        .data_mode(Mode {
            polarity: Polarity::IdleLow,
            phase: Phase::CaptureOnFirstTransition,
        });

    let driver = SpiDriver::new::<SPI2>(
        spi,
        sclk,
        serial_out,
        Some(serial_in),
        &SpiDriverConfig::new(),
    )?;

    let mut spi_device = SpiDeviceDriver::new(&driver, Some(cs), &config)?;
    
    println!("\n\n--------->   SPI initialized\n\n");


    // Time needed for DW3000 to start up (transition from INIT_RC to IDLE_RC, or could wait for SPIRDY event)
    // std::thread::sleep(Duration::from_millis(200));

    let mut rst_n = PinDriver::output(peripherals.pins.gpio27)?;
    rst_n.set_low().unwrap();
    delay.delay_ms(200);
    rst_n.set_high().unwrap();

    println!("--------->   DW3000 Reset");

    println!("--------->   Waiting for DW3000 to start up ... (5s)");
    // Time needed for DW3000 to start up (transition from INIT_RC to IDLE_RC, or could wait for SPIRDY event)
    //delay.delay_ms(5000);

    let dw3000_config = Config {
        channel: UwbChannel::Channel5,
        sfd_sequence: SfdSequence::Decawave8,
        pulse_repetition_frequency: PulseRepetitionFrequency::Mhz16,
        preamble_length: PreambleLength::Symbols1024,
        bitrate: BitRate::Kbps6800,
        frame_filtering: false,
        ranging_enable: true,
        sts_mode: StsMode::StsModeOff,
        sts_len: StsLen::StsLen64,
        sfd_timeout: 129,
    };

    let dw3000_default_config = Config::default();

    println!("--------->   Trying to create DW3000 instance ...");
    let mut dw3000 = DW3000::new(spi_device)
		.init()
		.expect("Failed DWM3000 init.");

    println!("--------->   Created DW3000 instance: {:?}", dw3000);

    delay.delay_ms(5000);

    let dw_res = dw3000.config(dw3000_config);

    println!("--------->   DW3000 config result: {:?}", dw_res);
    match dw_res {
        Ok(mut uwb) => {
            println!("--------->   🎉  DWM3000 initialized");

            loop {
                // Initiate Reception
                let mut buffer = [0; 1023];
                let mut receiving = uwb
                    .receive(Config::default())
                    .expect("Failed configure receiver.");
        
                // Waiting for an incomming frame
                let result = match block!(receiving.r_wait(&mut buffer)) {
                    Ok(t) => t,
                    Err(e) => {
                        println!("UWB Error: {:?}", e);
                        uwb = receiving.finish_receiving().expect("Failed to finish receiving");
                        continue // Start a new loop iteration
                    }
                };
        
                println!("Received '{:?}' at {:?}", result.frame.payload(), result.rx_time.value());
                uwb = receiving.finish_receiving().expect("Failed to finish receiving");
            }
        },
        Err(e) => println!("--------->  DW3000 config error: {:?}", e),
    }

    //controller.start_event_loop()?;

    Ok(())
}
