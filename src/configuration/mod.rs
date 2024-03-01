//! Various data structures used to configure the controller.

use std::time::Duration;

pub struct ControllerConfig {
    /// The initial brightness of the LEDs as a percentage between 0.0 and 1.0.
    pub initial_brightness: f32,
    /// How long to wait for a Wi-Fi connection to the master node before going into server meditation.
    pub wifi_join_timeout: Duration,
}