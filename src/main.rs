use embedded_svc::http::Method;
use embedded_svc::ws::FrameType;
use esp_idf_hal::sys::{ESP_ERR_INVALID_SIZE, EspError};
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

//pub mod display;
pub mod controller;
pub mod led;
pub mod network;
pub mod moving_average;
pub mod util;

use controller::{Controller, ControllerMode};

pub const STACK_SIZE: usize = 10240;
// Max payload length
const MAX_LEN: usize = 8;

#[derive(Serialize, Deserialize)]
/// A JSON document at the server root `/` which validates a successful connection and provides some information about the server.
struct RootDocument {
    version: String,
}

pub enum GameMode {
    /// The controller is currently not in a game session.
    Idle,
    /// Keep your own light green while pushing (softly) on the other light rods to make them red. Being
    /// green or red determines if you're still in the round or not. The last remaining player wins.
    LastOneStanding,
    /// All players are divided into two or more groups and each group is assigned a color. To take over territory, get
    /// your own controller within range of another, try to push it and it may take on the color of yours. The game is 
    /// finished and a winner may be declared when all controllers have the same color.
    Territory,
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let mut state = ControllerMode::Discovery;

    log::info!("Hello, world!");

    println!("{}", util::LOGO);

    let peripherals = Peripherals::take().unwrap();
    let timer = EspTaskTimerService::new().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    println!("Creating Wi-Fi driver");
    let wifi_driver = WifiDriver::new(peripherals.modem, sys_loop.clone(), Some(nvs.clone())).unwrap();
    println!("Creating EspWifi");

    let wifi = EspWifi::wrap_all(
        wifi_driver,
        EspNetif::new(NetifStack::Sta).unwrap(),
        EspNetif::new(NetifStack::Ap).unwrap(),
    ).unwrap();

    let wifi = AsyncWifi::wrap(wifi, sys_loop.clone(), timer.clone()).unwrap();

    let mut controller = Controller::new(
        timer,
        sys_loop,
        nvs,
    );

    futures::executor::block_on(controller.init_wifi(wifi))?;

    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };

    let mut server = EspHttpServer::new(&server_configuration).unwrap();

    server.fn_handler("/", Method::Get, |req| {
        let root_doc = RootDocument {
            version: "0.1.0".to_string(),
        };
        req.into_ok_response()?.write(serde_json::to_string(&root_doc)?.as_bytes()).unwrap();
        Ok(())
    }).unwrap();

    server
        .ws_handler("/ws", move |ws| {
            if ws.is_new() {
                // sessions.insert(ws.session(), GuessingGame::new((rand() % 100) + 1));
                println!("New WebSocket session");

                ws.send(
                    FrameType::Text(false),
                    "Welcome to the guessing game! Enter a number between 1 and 100".as_bytes(),
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

            ws.send(FrameType::Text(false), user_string.as_bytes())?;

            Ok::<(), EspError>(())
        })
        .unwrap();
    controller.start_event_loop()?;
/*
    let mut display = ControllerDisplay::new(i2c);
    display.bootscreen();
*/
    Ok(())
}
