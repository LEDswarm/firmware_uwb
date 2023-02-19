use adxl343::Adxl343;
use adxl343::accelerometer::Accelerometer;
use accelerometer::vector::F32x3;
use esp_idf_hal::i2c::{I2cDriver, I2C0};
use esp_idf_hal::gpio::{Gpio6, Gpio7};

pub struct Sensors<'a> {
    accelerometer: Adxl343<I2cDriver<'a>>,
}

impl<'a> Sensors<'a> {
    pub fn new(i2c0: I2C0, sda: Gpio6, scl: Gpio7) -> Self {
        let i2c = I2cDriver::new(
            i2c0,
            sda,
            scl,
            &esp_idf_hal::i2c::config::Config {
                baudrate: esp_idf_hal::units::Hertz(400_000),
                sda_pullup_enabled: false,
                scl_pullup_enabled: false,
            },
        ).unwrap();

        Self {
            accelerometer: Adxl343::new(i2c).unwrap(),
        }
    }

    pub fn get_accelerometer_reading(&mut self) -> F32x3 {
        self.accelerometer.accel_norm().unwrap()
    }
}