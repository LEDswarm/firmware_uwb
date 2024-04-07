//! The possible states of the LED controller along with utilities to compose them into a timeline for states.
//!
//! This definition serves as a common interface for the LED controller to be used by the game modes and can also be
//! used to implement blink codes for error handling or provide feedback to the user.

use crate::controller::ControllerMode;

pub struct BlinkCode {

}


/// A single color for all diodes, or an array individually addressing the LEDs.
#[derive(Debug)]
pub enum LedMode {
    /// All of the LEDs are set to the same color.
    Simultaneous((u8, u8, u8, u8)),

    /// The color of each LED is set individually.
    Individual([(u8, u8, u8, u8); 7]),

    /// The color of the LEDs is controlled by messages sent over MPSC.
    DirectDrive,
}

/// The color values of the LED at a certain point in time, addressed simultaneously or individually.
#[derive(Debug)]
pub struct LedState {
    /// The duration of this state change in milliseconds.
    pub duration: u32,
    /// Defines how the LEDs are addressed.
    pub mode:     LedMode,
}

impl LedState {
    /// Request a single color from the LED state.
    ///
    /// For simultanous mode, this will just return the color, and if mistakenly
    /// used in individual mode, the first color in the buffer will be used.
    pub fn get_single_color(&self, time: u32) -> (u8, u8, u8, u8) {
        match &self.mode {
            LedMode::Simultaneous(color) => *color,
            LedMode::Individual(colors) => colors[0],
            LedMode::DirectDrive => (0, 0, 0, 0),
        }
    }

    /// Request a buffer of seven individual colors from the LED state.
    ///
    /// This will simply return the buffer in individual mode, or create a buffer
    /// with seven identical colors from the color of the simultaneous mode.
    pub fn get_color_array(&self) -> [(u8, u8, u8, u8); 7] {
        match &self.mode {
            LedMode::Simultaneous(color) => [*color; 7],
            LedMode::Individual(colors) => *colors,
            LedMode::DirectDrive => [(0, 0, 0, 0); 7],
        }
    }

    /// Create a new LED state with a single color for all LEDs.
    pub fn all(duration: u32, color: (u8, u8, u8, u8)) -> Self {
        Self {
            duration,
            mode: LedMode::Simultaneous(color),
        }
    }

    /// Create a new LED state with a different color for each LED.
    pub fn each(duration: u32, colors: [(u8, u8, u8, u8); 7]) -> Self {
        Self {
            duration,
            mode: LedMode::Individual(colors),
        }
    }
}

/// A sequence of color changes.
#[derive(Debug)]
pub struct LedTimeline {
    states: Vec<LedState>,
}

impl LedTimeline {
    pub fn get_current_color(&self, time: u16) -> (u8, u8, u8, u8) {
        let total_duration: u32 = self.states.iter().map(|pattern| pattern.duration).sum();
        let time_in_cycle = time as u32 % total_duration;

        let mut elapsed = 0;
        for pattern in &self.states {
            elapsed += pattern.duration;
            if time_in_cycle < elapsed {
                return pattern.get_single_color(time as u32);
            }
        }

        // Default color if no pattern matches
        (0, 0, 0, 30)
    }

    pub fn new(states: Vec<LedState>) -> Self {
        Self {
            states,
        }
    }
}

impl From<&ControllerMode> for LedTimeline {
    fn from(mode: &ControllerMode) -> Self {
        match &mode {
            ControllerMode::Discovery { .. } => {
                Self::new(vec![
                    LedState::all(1000, (0, 255, 255, 0)),
                    LedState::all(1000, (0, 0, 0, 0)),
                ])
            },
            ControllerMode::Connecting { .. } => {
                Self::new(vec![
                    LedState::all(500, (0, 255, 255, 0)),
                    LedState::all(500, (0, 0, 0, 0)),
                ])
            },
            ControllerMode::Client { .. } => {
                Self::new(vec![
                    LedState::all(50, (0, 100, 0, 0)),
                    LedState::all(50, (0, 0, 0, 0)),
                    LedState::all(50, (0, 100, 0, 0)),
                    LedState::all(850, (0, 0, 0, 0)),
                ])
            },
            ControllerMode::Master { .. } => {
                Self::new(vec![
                    LedState::all(50, (0, 100, 0, 0)),
                    LedState::all(50, (0, 0, 0, 0)),
                    LedState::all(50, (0, 100, 0, 0)),
                    LedState::all(850, (0, 0, 0, 0)),
                ])
            },
            ControllerMode::ServerMeditation => Self::new(vec![]),

            _ => {
                Self::new(vec![
                    LedState::all(1000, (0, 0, 0, 0)),
                    LedState::all(1000, (0, 0, 0, 100)),
                ])
            },
        }
    }
}

// Function to calculate a smooth rainbow color based on time
fn get_smooth_rainbow_color(time: f32) -> (u8, u8, u8) {
    let pi = std::f32::consts::PI;

    // Time factor, adjust for speed (2.0 is an example factor for the speed of the cycle)
    let t = time * 1.0 * pi;

    // Calculate RGB components
    let r = ((t + 0.0 * pi / 3.0).sin() * 127.5 + 127.5) as u8;
    let g = ((t + 2.0 * pi / 3.0).sin() * 127.5 + 127.5) as u8;
    let b = ((t + 4.0 * pi / 3.0).sin() * 127.5 + 127.5) as u8;

    (r, g, b)
}