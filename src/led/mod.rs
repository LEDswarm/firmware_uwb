use esp_idf_svc::timer::EspTimerService;
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
    pub config: LedConfig,
}

impl Led {
    pub fn new(config: LedConfig) -> Self {
        Self {
            driver: LedPixelEsp32Rmt::<RGBW8, LedPixelColorGrbw32>::new(0, config.pin).unwrap(),
            config,
        }
    }

    pub fn pattern(&mut self, state: &ControllerMode, time: u32) {
        let timeline = LedTimeline::from(state);
        let color = timeline.get_current_color(time);
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
        ))).take(7);

        self.driver.write(pixels).unwrap();
    }
}
