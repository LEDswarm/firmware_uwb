//! Controller for the Inertial Measurement Unit (IMU)

use std::{time::Duration, sync::mpsc::SyncSender};

use adxl343::accelerometer::Accelerometer;
use esp_idf_hal::i2c::{I2C0, I2cConfig, I2cDriver};
use esp_idf_svc::sys::EspError;
use esp_idf_svc::hal::gpio::{Gpio21, Gpio22};
use esp_idf_svc::hal::prelude::*;

use ledswarm_protocol::{Frame, InternalMessage};

use crate::moving_average;

pub fn start(tx: SyncSender<Frame>, i2c: I2C0, sda: Gpio21, scl: Gpio22) -> Result<(), EspError> {
    std::thread::spawn(move || {
        let config = I2cConfig::new().baudrate(100.kHz().into());
        let i2c = I2cDriver::new(i2c, sda, scl, &config).unwrap();
        
        let mut accelerometer = adxl343::Adxl343::new(i2c).unwrap();
        let mut moving_average = moving_average::MovingAverage::new();

        let mut last_delta = 0.0;

        loop {
            let reading = accelerometer.accel_norm().unwrap();
            moving_average.add(reading);
            let delta = moving_average.get_average_delta();
            let delta_difference = (delta - last_delta).abs();
            if delta_difference > 0.02 {
                // The receiving end in the controller MUST handle these events or the buffer overflow will cause an out-of-memory error after about 15 seconds.
                tx.send(
                    Frame::new().internal_message(InternalMessage::AccelerometerJoltDelta(delta))
                ).unwrap();
                last_delta = delta;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    });

    Ok(())
}