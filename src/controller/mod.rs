
use std::sync::mpsc;
use std::time::{Instant, /*Duration*/};
use std::sync::Arc;

use colored::Colorize;
use esp_idf_svc::{
    sys::EspError,
    wifi::{EspWifi, AsyncWifi},
};
use nanoid::nanoid;

use crate::led::{Led, LedConfig};
use crate::network::wifi::WifiController;
use crate::event_bus::EventBus;

use ledswarm_protocol::{ClientMessage, ControllerMessage, Frame, FramePayload, GameMode, InternalMessage};

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteController {
    pub unique_id: String,
    pub id: u16,
}

impl RemoteController {
    pub fn new(id: u16) -> Self {
        Self {
            unique_id: nanoid!(10),
            id,
        }
    }
}

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
        /// The iterative ID of the controller in the mesh, starting from 1 since 0 is the master.
        id: usize,
        game: Option<GameMode>,
    },
    /// The controller has launched a Wi-Fi hotspot and is waiting for other controllers to join the session.
    ServerMeditation,
    /// The controller is currently connected to an UWB mesh as the master node.
    Master {
        /// Contains references to all the controllers in the mesh so the master can keep track of them.
        controllers: Vec<RemoteController>,
        /// An incrementing counter to assign unique IDs to new controllers joining the mesh.
        id_counter: u16,
        /// Keeps track of the current game state if a game is running.
        game: Option<GameState>,
    },
    /// The controller is currently in a game session.
    Game(GameMode),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameState {
    LastOneStanding {
        /// The IDs of all controllers currently active in the round.
        active_controller_ids: Vec<usize>,
        /// The IDs of all controllers that have been eliminated from the round.
        exited_controller_ids: Vec<usize>,
    },
}

pub struct Controller<'a> {
    pub mode: ControllerMode,
    pub connected_controllers: Vec<RemoteController>,
    pub sensors: Sensors,
    pub event_bus: Arc<EventBus>,
    rx: mpsc::Receiver<ControllerMode>,
    tx: mpsc::Sender<ControllerMode>,
    msg_rx: mpsc::Receiver<InternalMessage>,
    uwb_out_tx: mpsc::SyncSender<Frame>,
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
    pub fn new(msg_rx: mpsc::Receiver<InternalMessage>, uwb_out_tx: mpsc::SyncSender<Frame>) -> Self {
        let (tx, rx): (mpsc::Sender<ControllerMode>, mpsc::Receiver<ControllerMode>) = mpsc::channel();

        Self {
            mode:       ControllerMode::Discovery,
            connected_controllers: vec![],
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
        println!("Starting controller Wi-Fi");
        let mut wifi_controller = WifiController::new(
            wifi,
            self.tx.clone(),
        );

        wifi_controller.join_or_create_network().await?;
        self.wifi = Some(wifi_controller);

        Ok(())
    }

    fn send_uwb_frame(&mut self, frame: Frame) {
        self.uwb_out_tx.send(frame).unwrap();
    }

    fn handle_client_msg(&mut self, msg: ClientMessage) {
        match msg {
            ClientMessage::SetBrightness(brightness) => {
                // Set the brightness of the LED and make sure it's within the valid range
                self.led.config.intensity = brightness.clamp(0.0, 1.0);

                match self.mode {
                    ControllerMode::Master { .. } => {
                        println!("Broadcasting brightness to all clients");
                        self.uwb_out_tx.send(Frame::new().client_message(ClientMessage::SetBrightness(brightness))).unwrap();
                    },
                    _ => {},
                }
            },

            _ => println!("Unhandled client message: {:?}", msg),
        }
    }

    fn handle_internal_msg(&mut self, time: u16, msg: InternalMessage) {
        match msg {
            InternalMessage::ClientMessage(client_msg) => self.handle_client_msg(client_msg),
            InternalMessage::AccelerometerJoltDelta(delta) => self.sensors.accelerometer_jolt = delta,
            InternalMessage::Frame(frame) => self.handle_uwb_frame(time, *frame),
            _ => println!("Unhandled internal message: {:?}", msg),
        }
    }

    fn handle_uwb_frame(&mut self, time: u16, frame: Frame) {
        match frame.payload {
            FramePayload::ControllerMessage(controller_msg) => {
                match controller_msg {
                    ControllerMessage::JoinRequest => {
                        println!("Received UWB join request from controller at time {}", time);

                        let mut assigned_id = 65535;

                        if self.mode == ControllerMode::ServerMeditation {
                            assigned_id = 1;
                            self.mode = ControllerMode::Master {
                                controllers: vec![RemoteController::new(assigned_id)],
                                id_counter: 2,
                                game: None,
                            };
                        } else if let ControllerMode::Master { controllers, id_counter, game } = &mut self.mode {
                            *id_counter += 1;
                            assigned_id = *id_counter;
                            controllers.push(RemoteController::new(assigned_id));
                        }

                        self.send_uwb_frame(Frame::join_response(time, assigned_id));
                    },

                    // We have just successfully joined a mesh.
                    ControllerMessage::JoinResponse { assigned_id } => {
                        println!("Received UWB join response from controller at time {}", time);
                        self.mode = ControllerMode::Client {
                            id: assigned_id as usize,
                            game: None,
                        };
                    },
                }
            },

            FramePayload::ClientMessage(client_msg) => self.handle_client_msg(client_msg),

            _ => println!("Unhandled UWB frame: {:?}", frame),
        }
    }

    fn led_pattern(&mut self, time: u16, delta_threshold: f32, current_delta: f32, mut stay_red: bool) {
        match &self.mode {
            ControllerMode::Discovery | ControllerMode::Connecting => {
                // println!("Mode: Discovery | Connecting");
                /*self.led.set_rgbw(
                    0,
                    180,
                    255,
                    0,
                );*/
                self.led.pattern(&self.mode, time);
            },

            ControllerMode::Master { .. } => {
                // println!("Mode: Master");
                // Pink
                self.led.set_rgbw(
                    0,
                    30,
                    255,
                    0,
                );
            },

            ControllerMode::Client { .. } => {
                // println!("Mode: Client");
                // Orange
                self.led.set_rgbw(
                    250,
                    80,
                    0,
                    0,
                );
            },

            ControllerMode::ServerMeditation => {
                // println!("Mode: ServerMeditation (t {})", time);
                const CYCLE_LENGTH: u32 = 4096; // Full cycle length
                let time_wrapped = time as u32 % CYCLE_LENGTH;

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
                    GameMode::Idle => {
                        println!("GameIdle");
                    },
                    GameMode::LastOneStanding => {
                        println!("GameLastOneStanding");
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
        }
    }

    pub fn start_event_loop(&mut self) -> Result<(), EspError> {
        println!("## {}  Initializing controller loop", "[Controller]".bright_blue().bold());
        let mut time = 0u16;
        let delta_threshold = 0.4;
        let mut current_delta = 0.0;

        let mut stay_red = false;

        self.mode = ControllerMode::Discovery;

        println!("## {}  Controller Init to Discovery Mode", "[Controller]".bright_blue().bold());

        let delay = esp_idf_hal::delay::Delay::new_default();

        self.uwb_out_tx.send(Frame::join_request(time)).unwrap();
        println!("Sent UWB join request to see if there is a master");

        loop {
            //println!("Event loop iteration {}", time);
            // Check for new channel messages
            if let Ok(mode) = self.rx.try_recv() {
                println!("Mode change: {:?}", mode);
                self.mode = mode;
            }
            if let Ok(internal_msg) = self.msg_rx.try_recv() {
                self.handle_internal_msg(time, internal_msg);
            }

            self.led_pattern(time, delta_threshold, current_delta, stay_red);

            if time < u16::MAX {
                time += 1;
            } else {
                time = 0;
            }
            delay.delay_us(50);
        }

        Ok(())
    }
}