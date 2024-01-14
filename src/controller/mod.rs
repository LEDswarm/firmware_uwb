
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
    /// The controller is managing a mesh network along with a Wi-Fi hotspot and waits for other controllers to join the session so that it can start a game.
    ServerMeditation,
    Master,
}

pub struct Controller<'a> {
    pub mode: ControllerMode,
    rx: mpsc::Receiver<ControllerMode>,
    tx: mpsc::Sender<ControllerMode>,
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
        println!("--> Controller.init_wifi()");
        let mut wifi_controller = WifiController::new(
            wifi,
            self.tx.clone(),
        );

        println!("--> Controller.join_or_create_network()");
        wifi_controller.join_or_create_network().await?;

        println!("--> Setting Controller.wifi_controller to Some(...)");

        self.wifi = Some(wifi_controller);

        println!("--> Returning from Controller.init_wifi()");

        Ok(())
    }

    pub fn start_event_loop(&mut self) -> Result<(), EspError> {
        let mut time = 0u32;
        loop {
            let try_recv = self.rx.try_recv();
            if !try_recv.is_err() {
                self.mode = try_recv.unwrap();
            }
            self.led.pattern(&self.mode, time);
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