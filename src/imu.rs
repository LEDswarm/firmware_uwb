//! Controller for the Inertial Measurement Unit (IMU)

use std::{time::Duration, sync::mpsc::Sender};

use adxl343::accelerometer::Accelerometer;
use esp_idf_hal::{i2c::{I2C0, I2cConfig, I2cDriver}, gpio::AnyIOPin};
use esp_idf_svc::sys::EspError;
use ledswarm_protocol::{Message, Notice};
use esp_idf_svc::hal::gpio::{Gpio21, Gpio22};
use esp_idf_svc::hal::prelude::*;

use crate::moving_average;

pub fn start(tx: Sender<Message>, i2c: I2C0, sda: Gpio21, scl: Gpio22) -> Result<(), EspError> {
    std::thread::spawn(move || {
        let config = I2cConfig::new().baudrate(100.kHz().into());
        let i2c = I2cDriver::new(i2c, sda, scl, &config).unwrap();
        
        let mut accelerometer = adxl343::Adxl343::new(i2c).unwrap();
        let mut moving_average = moving_average::MovingAverage::new();

        loop {
            let reading = accelerometer.accel_norm().unwrap();
            moving_average.add(reading);
            let delta = moving_average.get_average_delta();
            // The receiving end in the controller MUST handle these events or the buffer overflow will cause an out-of-memory error after about 15 seconds.
            tx.send(Message::Notice(Notice::Accelerometer(delta))).unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
    });

    Ok(())
}