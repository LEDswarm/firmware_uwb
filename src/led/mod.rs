use smart_leds_trait::{SmartLedsWrite, White};
use ws2812_esp32_rmt_driver::driver::color::LedPixelColorGrbw32;
use ws2812_esp32_rmt_driver::{LedPixelEsp32Rmt, RGBW8};

use crate::controller::ControllerMode;

pub mod blink;

use blink::{LedState, LedTimeline};

pub struct LedConfig {
    pub pin: u32,
    pub intensity: f32,
}

/// Driver for the NeoPixel Jewel, a small two-inch circular PCB with seven SK6812 LEDs.
///
/// This struct abstracts the interface to the `ws2812_esp32_rmt_driver` to provide a method API
pub struct Led {
    driver: LedPixelEsp32Rmt::<RGBW8, LedPixelColorGrbw32>,
    last_controller_mode: Option<ControllerMode>,
    timeline: LedTimeline,
    pub config: LedConfig,
}

impl Led {
    pub fn new(config: LedConfig) -> Self {
        Self {
            driver: LedPixelEsp32Rmt::<RGBW8, LedPixelColorGrbw32>::new(0, config.pin).unwrap(),
            last_controller_mode: None,
            timeline: LedTimeline::new(vec![
                LedState::all(1000, (0, 255, 255, 0)),
                LedState::all(1000, (0, 0, 0, 0)),
            ]),
            config,
        }
    }

    pub fn pattern(&mut self, state: &ControllerMode, time: u16) {
        // Regenerate timeline only when the controller mode changes
        if self.last_controller_mode != Some(state.clone()) {
            println!("Regenerating timeline");
            self.last_controller_mode = Some(state.clone());
            self.timeline = LedTimeline::from(state);
        }

        let color = self.timeline.get_current_color(time);
        self.set_rgbw(color.0, color.1, color.2, color.3);
    }


    /// Write an RGBW color to our NeoPixel Jewel.
    pub fn set_rgbw(
        &mut self,
        red: u8,
        green: u8,
        blue: u8,
        white: u8
    ) {
        let pixels = std::iter::repeat(RGBW8::from((
            (red as f32 * self.config.intensity) as u8,
            (green as f32 * self.config.intensity) as u8,
            (blue as f32 * self.config.intensity) as u8,
            White((white as f32 * self.config.intensity) as u8,),
        ))).take(8);

        self.driver.write(pixels).unwrap();
    }
}
