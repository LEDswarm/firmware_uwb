pub struct MovingAverage {
    buffer: Vec<f32>,
    index:  usize,
}

impl MovingAverage {
    pub fn new() -> Self {
        Self {
            buffer: vec![0.0; 20],
            index:  0,
        }
    }

    pub fn get_average(&self) -> f32 {
        self.buffer.iter().sum::<f32>() / self.buffer.len() as f32
    }

    pub fn add_value(&mut self, value: f32) {
        self.buffer[self.index] = value;
        
        if self.index < self.buffer.len() - 1 {
            self.index += 1;
        } else {
            self.index = 0;
        }
    }
}