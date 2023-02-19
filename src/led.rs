use ws2812_esp32_rmt_driver::driver::color::LedPixelColorGrbw32;
use ws2812_esp32_rmt_driver::{LedPixelEsp32Rmt, RGBW8};
use smart_leds_trait::{SmartLedsWrite, White};
use colorsys::{Rgb, Hsl};

pub struct LED {
    ws2812: LedPixelEsp32Rmt::<RGBW8, LedPixelColorGrbw32>,
    brightness: f64,
}

impl LED {
    pub fn new() -> Self {
        let led_pin = 20;

        Self {
            ws2812: LedPixelEsp32Rmt::<RGBW8, LedPixelColorGrbw32>::new(0, led_pin).unwrap(),
            brightness: 0.05,
        }
    }

    pub fn fill_hue(&mut self, hue: f32) {
        let mut hsl = Hsl::default();
        // Hsl { h: 0, s: 0, l: 0, a: 1 }
        hsl.set_saturation(100.0);
        hsl.set_lightness(50.0);
        hsl.set_hue(hue as f64);
        let rgb = Rgb::from(&hsl);
        let pixels = std::iter::repeat(
            RGBW8::from((
                (rgb.red()   * self.brightness) as u8,
                (rgb.green() * self.brightness) as u8,
                (rgb.blue()  * self.brightness) as u8,
                White(0),
            ))
        ).take(7);
        self.ws2812.write(pixels).unwrap();
    }
}