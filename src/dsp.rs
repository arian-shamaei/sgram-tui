use rustfft::{num_complex::Complex32, FftPlanner};

pub enum WindowType { Hann, Hamming, Blackman }

pub struct Spectrogram {
    fft_size: usize,
    frame_len: usize,
    hop: usize,
    db_floor: f32,
    sample_rate: u32,
    window: Vec<f32>,
    tmp: Vec<Complex32>,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    overlap_buf: Vec<f32>,
    alpha: u8,
    pre_emph: Option<f32>,
    prev_sample: f32,
}

pub struct SpectrogramBuilder {
    fft_size: usize,
    frame_len: usize,
    hop: usize,
    db_floor: f32,
    sample_rate: u32,
    window: WindowType,
    alpha: u8,
    pre_emph: Option<f32>,
}

impl SpectrogramBuilder {
    pub fn new(fft_size: usize, frame_len: usize, hop: usize) -> Self {
        Self { fft_size, frame_len, hop, db_floor: -80.0, sample_rate: 48000, window: WindowType::Hann, alpha: 1, pre_emph: None }
    }
    pub fn db_floor(mut self, f: f32) -> Self { self.db_floor = f; self }
    pub fn sample_rate(mut self, sr: u32) -> Self { self.sample_rate = sr; self }
    pub fn window(mut self, w: WindowType) -> Self { self.window = w; self }
    pub fn alpha(mut self, a: u8) -> Self { self.alpha = if a == 2 { 2 } else { 1 }; self }
    pub fn pre_emphasis(mut self, beta: Option<f32>) -> Self { self.pre_emph = beta; self }
    pub fn build(self) -> Spectrogram {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(self.fft_size);
        let window = match self.window {
            WindowType::Hann => hann(self.frame_len),
            WindowType::Hamming => hamming(self.frame_len),
            WindowType::Blackman => blackman(self.frame_len),
        };
        Spectrogram {
            fft_size: self.fft_size,
            frame_len: self.frame_len,
            hop: self.hop.min(self.frame_len).max(1),
            db_floor: self.db_floor,
            sample_rate: self.sample_rate,
            window,
            tmp: vec![Complex32::new(0.0, 0.0); self.fft_size],
            fft,
            overlap_buf: Vec::new(),
            alpha: self.alpha,
            pre_emph: self.pre_emph,
            prev_sample: 0.0,
        }
    }
}

impl Spectrogram {
    pub fn process_samples(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        // Ingest input with optional pre-emphasis
        if let Some(beta) = self.pre_emph {
            for &x in samples {
                let y = x - beta * self.prev_sample;
                self.prev_sample = x;
                self.overlap_buf.push(y);
            }
        } else {
            self.overlap_buf.extend_from_slice(samples);
        }

        let mut out = Vec::new();
        while self.overlap_buf.len() >= self.frame_len {
            let frame = &self.overlap_buf[..self.frame_len];

            // Zero-pad to fft_size
            for i in 0..self.fft_size {
                if i < self.frame_len {
                    let x = frame[i] * self.window[i];
                    self.tmp[i].re = x;
                    self.tmp[i].im = 0.0;
                } else {
                    self.tmp[i].re = 0.0;
                    self.tmp[i].im = 0.0;
                }
            }
            self.fft.process(&mut self.tmp);

            // First N/2 bins to dB (magnitude or power)
            let n_bins = self.fft_size / 2;
            let mut row = vec![0.0f32; n_bins];
            for i in 0..n_bins {
                let c = self.tmp[i];
                let re2 = c.re * c.re; let im2 = c.im * c.im;
                if self.alpha == 2 {
                    let p = (re2 + im2).max(1e-24);
                    row[i] = 10.0 * p.log10();
                } else {
                    let m = (re2 + im2).sqrt().max(1e-12);
                    row[i] = 20.0 * m.log10();
                }
            }
            out.push(row);

            // Advance by hop
            let hop = self.hop.min(self.overlap_buf.len());
            self.overlap_buf.drain(0..hop);
        }
        out
    }
}

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let a = std::f32::consts::PI * 2.0 * (i as f32) / (n as f32);
            0.5 - 0.5 * a.cos()
        })
        .collect()
}

fn hamming(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let a = std::f32::consts::PI * 2.0 * (i as f32) / (n as f32);
            0.54 - 0.46 * a.cos()
        })
        .collect()
}

fn blackman(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let a = (i as f32) / (n as f32);
            0.42 - 0.5 * (2.0 * std::f32::consts::PI * a).cos() + 0.08 * (4.0 * std::f32::consts::PI * a).cos()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hann_sum_reasonable() {
        let w = hann(1024);
        let sum: f32 = w.iter().sum();
        assert!((sum / 1024.0) > 0.4 && (sum / 1024.0) < 0.6);
    }
}
