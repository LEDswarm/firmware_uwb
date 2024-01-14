//! Compose time-based single-color patterns for the LED, such as blink codes or other visual indicators.

use crate::controller::ControllerMode;

/// A single LED color change with a given duration.
pub struct Blink {
    duration: u32,
    color: (u8, u8, u8, u8),
}

impl Blink {
    pub fn new(duration: u32, color: (u8, u8, u8, u8)) -> Self {
        Self {
            duration,
            color,
        }
    }
}

/// A sequence of color changes.
pub struct BlinkTimeline {
    patterns: Vec<Blink>,
}

impl BlinkTimeline {
    pub fn get_current_color(&self, time: u32) -> (u8, u8, u8, u8) {
        let total_duration: u32 = self.patterns.iter().map(|pattern| pattern.duration).sum();
        let time_in_cycle = time % total_duration;

        let mut elapsed = 0;
        for pattern in &self.patterns {
            elapsed += pattern.duration;
            if time_in_cycle < elapsed {
                return pattern.color;
            }
        }

        // Default color if no pattern matches
        (0, 0, 0, 30)
    }

    pub fn new(patterns: Vec<Blink>) -> Self {
        Self {
            patterns,
        }
    }
}

impl From<&ControllerMode> for BlinkTimeline {
    fn from(mode: &ControllerMode) -> Self {
        match &mode {
            ControllerMode::Discovery { .. } => {
                Self::new(vec![
                    Blink::new(1000, (0, 255, 255, 0)),
                    Blink::new(1000, (0, 0, 0, 0)),
                ])
            },
            ControllerMode::Connecting { .. } => {
                Self::new(vec![
                    Blink::new(500, (0, 255, 255, 0)),
                    Blink::new(500, (0, 0, 0, 0)),
                ])
            },
            ControllerMode::Client { .. } => {
                Self::new(vec![
                    Blink::new(50, (0, 0, 0, 100)),
                    Blink::new(50, (0, 0, 0, 0)),
                    Blink::new(50, (0, 0, 0, 100)),
                    Blink::new(850, (0, 0, 0, 0)),
                ])
            },
            ControllerMode::Master { .. } => {
                Self::new(vec![
                    Blink::new(50, (0, 100, 0, 0)),
                    Blink::new(50, (0, 0, 0, 0)),
                    Blink::new(50, (0, 100, 0, 0)),
                    Blink::new(850, (0, 0, 0, 0)),
                ])
            },
            ControllerMode::ServerMeditation => {
                let mut patterns = vec![];

                for t in 0 .. 400 {
                    let color = get_smooth_rainbow_color(t as f32 / 200.0);
                    patterns.push(Blink::new(2, (color.0, color.1, color.2, 0)));
                }

                Self::new(patterns)
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