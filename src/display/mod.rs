use embedded_graphics::{
    mono_font::{ascii::FONT_9X15, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
    image::Image,
};
use tinybmp::Bmp;
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};
use esp_idf_hal::i2c::*;

pub struct ControllerDisplay<'a> {
    display: Option<Ssd1306<I2CInterface<esp_idf_hal::i2c::I2cDriver<'a>>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>>,
}

impl<'a> ControllerDisplay<'a> {
    pub fn new() -> Self {
        Self {
            display: None,
        }
    }

    pub fn setup(&mut self, i2c: I2cDriver<'a>) -> anyhow::Result<()> {
        let interface = I2CDisplayInterface::new(i2c);
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        display.init()?;
        self.display = Some(display);
        Ok(())
    }

    pub fn bootscreen(&mut self) {
        // Include the BMP file data.
        let bmp_data = include_bytes!("../../assets/bootscreen_h48.bmp");

        // Parse the BMP file.
        let bmp = Bmp::from_slice(bmp_data).unwrap();

        let display = self.display.as_mut().unwrap();

        // Draw the image with the top left corner at (10, 20) by wrapping it in
        // an embedded-graphics `Image`.
        Image::new(&bmp, Point::new(0, 16)).draw(display).unwrap();

        // Create a new character style
        let style = MonoTextStyle::new(&FONT_9X15, BinaryColor::On);

        // Create a text at position (0, 0) and draw it using the previously defined style
        Text::new("0.1.0", Point::new(0, 13), style).draw(display).unwrap();

        Text::new("D", Point::new(70, 13), style).draw(display).unwrap();

        display.flush().unwrap();
    }

    pub fn draw_image(&mut self, bmp_data: &[u8]) {
        let display = self.display.as_mut().unwrap();

        let bmp = Bmp::from_slice(bmp_data).unwrap();
        Image::new(&bmp, Point::new(0, 0)).draw(display).unwrap();
        display.flush().unwrap();
    }
}