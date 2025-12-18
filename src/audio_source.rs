use bytes::{Bytes, BytesMut, BufMut};

pub struct SineWaveGenerator {
    frequency: f32,
    sample_rate: u32,
    channels: u32,
    phase: f32,
}

impl SineWaveGenerator {
    pub fn new(frequency: f32, sample_rate: u32, channels: u32) -> Self {
        Self {
            frequency,
            sample_rate,
            channels,
            phase: 0.0,
        }
    }

    pub fn generate(&mut self, num_samples: usize) -> Bytes {
        let mut buffer = BytesMut::with_capacity(num_samples * self.channels as usize * 4);
        let phase_increment = self.frequency * 2.0 * std::f32::consts::PI / self.sample_rate as f32;

        for _ in 0..num_samples {
            let sample = self.phase.sin();
            self.phase = (self.phase + phase_increment) % (2.0 * std::f32::consts::PI);

            for _ in 0..self.channels {
                buffer.put_f32_le(sample);
            }
        }

        buffer.freeze()
    }
}
