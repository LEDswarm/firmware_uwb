
use std::sync::mpsc;
use std::time::{Instant, Duration};
use std::sync::Arc;

use esp_idf_svc::{
    sys::EspError,
    wifi::{EspWifi, AsyncWifi},
};

use crate::led::{Led, LedConfig};
use crate::network::wifi::WifiController;
use crate::event_bus::EventBus;

use ledswarm_protocol::{InternalMessage, UwbMessage, UwbPacket};

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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
    pub sensors: Sensors,
    pub event_bus: Arc<EventBus>,
    rx: mpsc::Receiver<ControllerMode>,
    tx: mpsc::Sender<ControllerMode>,
    msg_rx: mpsc::Receiver<InternalMessage>,
    uwb_out_tx: mpsc::Sender<UwbPacket>,
    pub start_time: Instant,
    wifi: Option<WifiController<'a>>,
    led:  Led,
}

pub struct Sensors {
    /// The current average jolt (change of acceleration) experienced by the controller enclosure,
    /// a moving average of recent vector sums of the X, Y and Z readings from the accelerometer.
    pub accelerometer_jolt: f32,
}

impl Sensors {
    pub fn new() -> Self {
        Self {
            accelerometer_jolt: 0.0,
        }
    }
}

impl<'a> Controller<'a> {
    pub fn new(msg_rx: mpsc::Receiver<InternalMessage>, uwb_out_tx: mpsc::Sender<UwbPacket>) -> Self {
        let (tx, rx): (mpsc::Sender<ControllerMode>, mpsc::Receiver<ControllerMode>) = mpsc::channel();

        Self {
            mode:       ControllerMode::Discovery,
            sensors:    Sensors::new(),
            event_bus:  Arc::new(EventBus::new()),
            rx,
            tx,
            msg_rx,
            uwb_out_tx,
            start_time: Instant::now(),
            wifi:       None,
            led:        Led::new(LedConfig { pin: 0, intensity: 0.3 }),
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

    fn handle_internal_msg(&mut self, message: InternalMessage) {
        match message {
            InternalMessage::Packet(packet) => {
                match packet.message {
                    UwbMessage::SetBrightness(brightness) => {
                        // Set the brightness of the LED and make sure it's within the valid range
                        self.led.config.intensity = brightness.clamp(0.0, 1.0);
                    },

                    _ => {},
                }
            },

            InternalMessage::AccelerometerDelta(delta) => {
                self.sensors.accelerometer_jolt = delta;
            },

            _ => {},
        }
    }

    pub fn start_event_loop(&mut self) -> Result<(), EspError> {
        let mut time = 0u32;
        let delta_threshold = 0.4;
        let mut current_delta = 0.0;

        let mut stay_red = false;

        self.mode = ControllerMode::Game(GameMode::LastOneStanding);

        loop {
            // Check for new channel messages
            if let Ok(mode) = self.rx.try_recv() {
                self.mode = mode;
            }
            if let Ok(msg) = self.msg_rx.try_recv() {
                self.handle_internal_msg(msg);
            }

            match &self.mode {
                ControllerMode::Discovery | ControllerMode::Connecting => {
                    self.led.set_rgbw(
                        0,
                        180,
                        255,
                        0,
                    );
                },

                ControllerMode::ServerMeditation => {
                    const CYCLE_LENGTH: u32 = 1024; // Full cycle length
                    let time_wrapped = time % CYCLE_LENGTH;
                
                    // Sine wave calculations
                    let r = (((2.0 * std::f64::consts::PI * time_wrapped as f64 / CYCLE_LENGTH as f64).sin() * 0.5 + 0.5) * 255.0) as u8;
                    let g = (((2.0 * std::f64::consts::PI * (time_wrapped as f64 + CYCLE_LENGTH as f64 / 3.0) / CYCLE_LENGTH as f64).sin() * 0.5 + 0.5) * 255.0) as u8;
                    let b = (((2.0 * std::f64::consts::PI * (time_wrapped as f64 + 2.0 * CYCLE_LENGTH as f64 / 3.0) / CYCLE_LENGTH as f64).sin() * 0.5 + 0.5) * 255.0) as u8;
                
                    // Calculate combined RGB intensity
                    let combined_intensity = (r as u16 + g as u16 + b as u16) / 3;
                
                    // Desired total intensity (tweak as needed)
                    let desired_intensity: u16 = 120; // Example value
                
                    // Calculate and adjust white intensity
                    let white_intensity = if combined_intensity > desired_intensity {
                        0
                    } else {
                        (desired_intensity - combined_intensity) as u8
                    };
                
                    // Set the RGBW values
                    self.led.set_rgbw(
                        r,
                        g,
                        b,
                        white_intensity,
                    );
                }

                ControllerMode::Game(game_mode) => {
                    match game_mode {
                        GameMode::Idle => {},
                        GameMode::LastOneStanding => {
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