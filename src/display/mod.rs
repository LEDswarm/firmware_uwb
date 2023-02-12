use esp_idf_hal::i2c::*;
use esp_idf_hal::peripherals::Peripherals;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use ssd1306::mode::BufferedGraphicsMode;
use esp_idf_hal::prelude::*;
use embedded_graphics::{
    image::{Image, ImageRaw},
    pixelcolor::BinaryColor,
    prelude::*,
};

pub struct Display<'a> {
    display: Ssd1306<
        I2CInterface<I2cDriver<'a>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>
    >,
}

impl<'a> Display<'a> {
    pub fn new(peripherals: &'a mut Peripherals) -> Self {
        let i2c = &mut peripherals.i2c0;
        let sda = &mut peripherals.pins.gpio21;
        let scl = &mut peripherals.pins.gpio22;

        let config = I2cConfig::new().baudrate(100.kHz().into());
        let i2c = I2cDriver::new(i2c, sda, scl, &config).unwrap();

        let interface = I2CDisplayInterface::new(i2c);
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        display.init().unwrap();

        Self {
            display,
        }
    }

    pub fn render_raw(&mut self, raw: ImageRaw<BinaryColor>) {
        let im = Image::new(&raw, Point::new(32, 0));
        im.draw(&mut self.display).unwrap();
        self.display.flush().unwrap();
    }
}