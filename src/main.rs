//! ![banner](https://ghoust.s3.fr-par.scw.cloud/ledswarm_banner.svg)
//!
//! The official firmware for ESP32-based LEDswarm controller boards.
//! 
//!

use adxl343::accelerometer::Accelerometer;
use embedded_svc::http::Method;
use embedded_svc::ws::FrameType;
use embedded_svc::http::Headers;
use embedded_svc::io::Write;
use embedded_svc::io::Read;
use esp_idf_hal::gpio::{PinDriver, Pin, Gpio15};
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::spi::config::{Mode, Polarity, Phase};
use esp_idf_hal::spi::{SpiDriver, SPI2, SpiDriverConfig, SpiDeviceDriver, config};
use esp_idf_hal::sys::{ESP_ERR_INVALID_SIZE, EspError};
use esp_idf_hal::task::watchdog::TWDT;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::netif::{EspNetif, NetifStack};
use esp_idf_svc::wifi::{WifiDriver, EspWifi, AsyncWifi};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition
};
use esp_idf_svc::timer::EspTaskTimerService;
use esp_idf_svc::http::server::EspHttpServer;

use serde::{Deserialize, Serialize};
use serde_json::Result;
/*
    use dw3000_ng::{
        hl::DW3000,
        configs::{
            Config,
            UwbChannel,
            SfdSequence,
            PulseRepetitionFrequency,
            PreambleLength,
            BitRate,
            StsMode,
            StsLen,
        },
    };
*/
use ledswarm_protocol::{Message, Request, Notice};

//pub mod display;
pub mod configuration;
pub mod controller;
pub mod led;
pub mod network;
pub mod moving_average;
pub mod util;

use controller::{Controller, ControllerMode};

use std::sync::mpsc;
use std::time::Duration;

pub const STACK_SIZE: usize = 10240;
// Max payload length
const MAX_LEN: usize = 256;

#[derive(Serialize, Deserialize)]
/// A JSON document at the server root `/` which validates a successful connection and provides some information about the server.
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
/*
    println!("\n\n--------->   Initializing SPI\n\n");

    //
    //  DW3000 SPI
    //


    let spi = peripherals.spi2;

    let serial_out = peripherals.pins.gpio19; // MISO
    let serial_in = peripherals.pins.gpio23; // MOSI
    let sclk = peripherals.pins.gpio18;
    let cs = peripherals.pins.gpio4; // CS

    let config = config::Config::new()
        .baudrate(20.MHz().into())
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
    std::thread::sleep(Duration::from_millis(20));
    rst_n.set_high().unwrap();

    // Time needed for DW3000 to start up (transition from INIT_RC to IDLE_RC, or could wait for SPIRDY event)
    std::thread::sleep(Duration::from_millis(2000));

    println!("--------->   DW3000 Reset Pin initialized");

    let dw3000_config = Config {
        channel: UwbChannel::Channel5,
        sfd_sequence: SfdSequence::Decawave8,
        pulse_repetition_frequency: PulseRepetitionFrequency::Mhz16,
        preamble_length: PreambleLength::Symbols128,
        bitrate: BitRate::Kbps6800,
        frame_filtering: false,
        ranging_enable: false,
        sts_mode: StsMode::StsModeOff,
        sts_len: StsLen::StsLen64,
        sfd_timeout: 129,
    };

    let dw3000_default_config = Config::default();

    let mut dw3000 = DW3000::new(spi_device)
		.init()
		.expect("Failed DWM3000 init.");

    println!("--------->   DW3000 struct: {:?}", dw3000);

    let dw_res = dw3000.config(dw3000_default_config);

    println!("--------->   DW3000 config result: {:?}", dw_res);

    println!("--------->   🎉  DWM3000 initialized");
*/
    println!("Creating Wi-Fi driver");
    let wifi_driver = WifiDriver::new(peripherals.modem, sys_loop.clone(), Some(nvs.clone())).unwrap();
    println!("Creating EspWifi");

    let wifi = EspWifi::wrap_all(
        wifi_driver,
        EspNetif::new(NetifStack::Sta).unwrap(),
        EspNetif::new(NetifStack::Ap).unwrap(),
    ).unwrap();

    let wifi = AsyncWifi::wrap(wifi, sys_loop.clone(), timer.clone()).unwrap();
    let (msg_tx, msg_rx): (mpsc::Sender<Message>, mpsc::Receiver<Message>) = mpsc::channel();

    let accel_tx = msg_tx.clone();

    let mut controller = Controller::new(
        timer,
        sys_loop,
        nvs,
        msg_rx,
    );

    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };

    futures::executor::block_on(controller.init_wifi(wifi))?;

    let mut server = EspHttpServer::new(&server_configuration).unwrap();

    server.fn_handler("/", Method::Get, |req| {
        let root_doc = RootDocument {
            version: "0.1.0".to_string(),
        };
        let mut response = req.into_ok_response()?;
        response.write(serde_json::to_string(&root_doc)?.as_bytes()).unwrap();
        Ok(())
    }).unwrap();

    server.fn_handler("/message", Method::Post, |mut req| {
        let len = req.content_len().unwrap_or(0) as usize;

        if len > MAX_LEN {
            req.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }

        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;
        let mut resp = req.into_ok_response()?;

        if let Ok(form) = serde_json::from_slice::<Message>(&buf) {
            /*write!(
                resp,
                "Hello, {}-year-old {} from {}!",
                form.age, form.first_name, form.birthplace
            )?;*/
            println!("-->   Msg:   {:?}", form);
        } else {
            resp.write_all("JSON error".as_bytes())?;
        }

        Ok(())
    }).unwrap();

    server
        .ws_handler("/ws", move |ws| {
            if ws.is_new() {
                // sessions.insert(ws.session(), GuessingGame::new((rand() % 100) + 1));
                println!("New WebSocket session");

                let msg = Message::Request(Request::SetBrightness("0.5".to_string()));
                let json_string = serde_json::to_string(&msg).unwrap();

                ws.send(
                    FrameType::Text(false),
                    json_string.as_bytes(),
                )?;
                return Ok(());
            } else if ws.is_closed() {
                // sessions.remove(&ws.session());
                println!("Closed WebSocket session");
                return Ok(());
            }
            // let session = sessions.get_mut(&ws.session()).unwrap();

            // NOTE: Due to the way the underlying C implementation works, ws.recv()
            // may only be called with an empty buffer exactly once to receive the
            // incoming buffer size, then must be called exactly once to receive the
            // actual payload.

            let (_frame_type, len) = match ws.recv(&mut []) {
                Ok(frame) => frame,
                Err(e) => return Err(e),
            };

            if len > MAX_LEN {
                ws.send(FrameType::Text(false), "Request too big".as_bytes())?;
                ws.send(FrameType::Close, &[])?;
                return Err(EspError::from_infallible::<ESP_ERR_INVALID_SIZE>());
            }

            let mut buf = [0; MAX_LEN]; // Small digit buffer can go on the stack
            ws.recv(buf.as_mut())?;
            let Ok(user_string) = std::str::from_utf8(&buf[..len]) else {
                ws.send(FrameType::Text(false), "[UTF-8 Error]".as_bytes())?;
                return Ok(());
            };

            // Remove null terminator
            match serde_json::from_str::<Message>(&user_string[0 .. user_string.len() - 1]) {
                Ok(msg) => {
                    //println!("-->   Msg:   {:?}", msg);
                    msg_tx.send(msg).unwrap();
                },
                Err(e)  => println!("Failed to parse JSON:\n\n{}\n\n{}", e, user_string),
            }
            
            ws.send(FrameType::Text(false), user_string.as_bytes())?;

            Ok::<(), EspError>(())
        })
        .unwrap();

    std::thread::spawn(move || {
        let i2c = peripherals.i2c0;
        let sda = peripherals.pins.gpio21;
        let scl = peripherals.pins.gpio22;

        let config = I2cConfig::new().baudrate(100.kHz().into());
        let i2c = I2cDriver::new(i2c, sda, scl, &config).unwrap();
        
        let mut accelerometer = adxl343::Adxl343::new(i2c).unwrap();
        let mut moving_average = moving_average::MovingAverage::new();

        loop {
            let reading = accelerometer.accel_norm().unwrap();
            moving_average.add(reading);
            let delta = moving_average.get_average_delta();
            accel_tx.send(Message::Notice(Notice::Accelerometer(delta))).unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
    });

    controller.start_event_loop()?;
/*
    let mut display = ControllerDisplay::new(i2c);
    display.bootscreen();
*/
    Ok(())
}
