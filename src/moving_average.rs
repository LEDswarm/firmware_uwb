/// A ring buffer which averages accelerometer readings to provide the change in acceleration.
pub struct MovingAverage {
    buffer: Vec<f32>,
    index: usize,
    current_magnitude: f32,
    previous_magnitude: f32,
}

impl MovingAverage {
    pub fn new() -> Self {
        Self {
            buffer: vec![0.0; 20],
            index: 0,
            current_magnitude: 0.0,
            previous_magnitude: 0.0,
        }
    }

    /// Get the vector magnitude for the given accelerometer reading.
    fn get_magnitude(&self, vector: accelerometer::vector::F32x3) -> f32 {
        (vector.x.powf(2.0) + vector.y.powf(2.0) + vector.z.powf(2.0)).sqrt()
    }

    /// Add a new measurement to the ring buffer.
    pub fn add(&mut self, vector: accelerometer::vector::F32x3) {
        // Get the length of the current 3D vector
        self.current_magnitude = self.get_magnitude(vector);

        // Calculate the absolute change between the current and the last magnitude
        // to get the jerk (the change of acceleration over time).
        //
        // This provides an easy way to sense how much the controller is moving, while also
        // auto-calibrating at the same time, removing the biases in the accelerometer 
        // readings. This is one of the main interaction points for game modes.
        self.buffer[self.index] = (self.current_magnitude - self.previous_magnitude).abs();

        // Prepare for next iteration
        self.previous_magnitude = self.current_magnitude;
        self.advance();
    }

    /// Calculate the average of all buffer entries.
    pub fn get_average_delta(&self) -> f32 {
        self::round(self.buffer.iter().sum::<f32>() / self.buffer.len() as f32, 2)
    }

    /// Increase the index by one if possible, or wrap back around.
    fn advance(&mut self) {
        if self.index < self.buffer.len() - 1 {
            self.index += 1;
        } else {
            self.index = 0;
        }
    }
}

fn round(x: f32, decimals: u32) -> f32 {
    let y = 10i32.pow(decimals) as f32;
    (x * y).round() / y
}