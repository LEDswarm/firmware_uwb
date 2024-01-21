
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Instant, Duration};

use esp_idf_hal::modem::Modem;
use esp_idf_svc::timer::EspTaskTimerService;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::EspError;
use esp_idf_svc::wifi::{EspWifi, AsyncWifi};

use crate::led::{Led, LedConfig};
use crate::network::wifi::WifiController;
use crate::moving_average::{MovingAverage, self};

use ledswarm_protocol::{Message, Request, Notice};

pub enum ControllerMode {
    /// The controller is currently trying to find nearby devices to build a mesh with.
    ///
    /// If no devices are found within 30 seconds, the controller will switch itself into master mode, 
    /// start a new mesh network and wait for other controllers to join the session.
    /// 
    /// Blinks cyan for a second followed by a pause of equal length.
    Discovery,
    /// The controller has found a nearby mesh and is currently trying to join the existing session.
    Connecting,
    /// The controller is currently connected to an UWB mesh as a non-master node.
    Client {
        /// The iterative ID of the controller in the mesh, but always 0 if this controller happens to be the master.
        id: usize,
    },
    /// The controller has launched a Wi-Fi hotspot and is waiting for other controllers to join the session.
    ServerMeditation,
    /// The controller is currently connected to an UWB mesh as the master node.
    Master,
    /// The controller is currently in a game session.
    Game(GameMode),
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

pub struct Controller<'a> {
    pub mode: ControllerMode,
    rx: mpsc::Receiver<ControllerMode>,
    tx: mpsc::Sender<ControllerMode>,
    msg_rx: mpsc::Receiver<Message>,
    pub start_time: Instant,
    wifi: Option<WifiController<'a>>,
    led:  Led,
    timer: EspTaskTimerService,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    //i2c: I2cDriver<'a>,
}

impl<'a> Controller<'a> {
    pub fn new(
        timer: EspTaskTimerService,
        sys_loop: EspSystemEventLoop,
        nvs: EspDefaultNvsPartition,
        msg_rx: mpsc::Receiver<Message>,
    ) -> Self {
    /*
        let i2c = peripherals.i2c0;
        let sda = peripherals.pins.gpio21;
        let scl = peripherals.pins.gpio22;

        let config = I2cConfig::new().baudrate(100.kHz().into());
        let i2c = I2cDriver::<'a>::new(i2c, sda, scl, &config).unwrap();
    */

        let (tx, rx): (mpsc::Sender<ControllerMode>, mpsc::Receiver<ControllerMode>) = mpsc::channel();

        Self {
            mode:       ControllerMode::Discovery,
            rx,
            tx,
            msg_rx,
            start_time: Instant::now(),
            wifi:       None,
            led:        Led::new(LedConfig { pin: 0, intensity: 0.3 }),
            timer,
            sys_loop,
            nvs,
            //i2c,
        }
    }

    pub async fn init_wifi(
        &mut self,
        wifi: AsyncWifi<EspWifi<'a>>,
    ) -> Result<(), EspError> {
        let mut wifi_controller = WifiController::new(
            wifi,
            self.tx.clone(),
        );

        wifi_controller.join_or_create_network().await?;
        self.wifi = Some(wifi_controller);

        Ok(())
    }

    pub fn start_event_loop(&mut self) -> Result<(), EspError> {
        let mut time = 0u32;
        let delta_threshold = 0.4;
        let mut current_delta = 0.0;

        let mut stay_red = false;

        self.mode = ControllerMode::Game(GameMode::LastOneStanding);

        loop {
            match &self.mode {
                ControllerMode::Discovery
                | ControllerMode::Connecting
                | ControllerMode::ServerMeditation => {
                    self.led.pattern(&self.mode, time);
                },

                ControllerMode::Game(game_mode) => {
                    match game_mode {
                        GameMode::Idle => {},
                        GameMode::LastOneStanding => {
                            let try_recv = self.rx.try_recv();
                            if !try_recv.is_err() {
                                self.mode = try_recv.unwrap();
                            }

                            let try_msg_recv = self.msg_rx.try_recv();
                            if !try_msg_recv.is_err() {
                                match try_msg_recv.unwrap() {
                                    Message::Request(req) => {
                                        match req {
                                            Request::SetBrightness(brightness) => {
                                                let mut num = brightness.parse::<f32>().unwrap();

                                                if num > 1.0 {
                                                    num = 1.0;
                                                } else if num < 0.0 {
                                                    num = 0.0;
                                                }

                                                self.led.config.intensity = num;
                                            },
                                            _ => {},
                                        }
                                    },
                                    Message::Notice(notice) => {
                                        match notice {
                                            Notice::Accelerometer(delta) => {
                                                current_delta = delta;
                                            },
                                            _ => {},
                                        }
                                    },
                                    _ => {},
                                }
                            }

                            let mut factor;
                            if current_delta > delta_threshold {
                                factor = 1.0;
                            } else {
                                factor = current_delta / delta_threshold;
                            }

                            if current_delta >= delta_threshold {
                                stay_red = true;
                            };

                            if stay_red {
                                factor = 1.0;
                            };

                            let red = (255.0 * factor) as u8;
                            self.led.set_rgbw(red, 255 - red, 0, 0);
                        },
                        GameMode::Territory => {
                            println!("Territory mode not implemented yet");
                        },
                    }
                },
                // Do not use LED here, causes reset
                _ => {},
            }

            if time < u32::MAX {
                time += 1;
            } else {
                time = 0;
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        Ok(())
    }
}