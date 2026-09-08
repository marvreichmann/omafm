//! Streaming mono WFM: staged anti-alias filtering, channel filter, phase
//! discriminator, 15 kHz audio filter, 50 us de-emphasis, 48 kHz resampling.
use std::f32::consts::PI;

struct Fir {
    taps: Vec<f32>,
    history: Vec<[f32; 2]>,
    pos: usize,
}
impl Fir {
    fn new(count: usize, cutoff: f32) -> Self {
        Self {
            taps: Self::kernel(count, cutoff, 0.0),
            history: vec![[0.0; 2]; count],
            pos: 0,
        }
    }
    fn kernel(count: usize, cutoff: f32, delay: f32) -> Vec<f32> {
        let mid = (count - 1) as f32 / 2.0;
        let mut taps: Vec<f32> = (0..count)
            .map(|i| {
                let x = i as f32 - mid + delay;
                let sinc = if x == 0.0 {
                    2.0 * cutoff
                } else {
                    (2.0 * PI * cutoff * x).sin() / (PI * x)
                };
                let w = 0.42 - 0.5 * (2.0 * PI * i as f32 / (count - 1) as f32).cos()
                    + 0.08 * (4.0 * PI * i as f32 / (count - 1) as f32).cos();
                sinc * w
            })
            .collect();
        let sum: f32 = taps.iter().sum();
        taps.iter_mut().for_each(|t| *t /= sum);
        taps
    }
    fn push(&mut self, x: [f32; 2]) {
        self.history[self.pos] = x;
        self.pos = (self.pos + 1) % self.history.len();
    }
    fn value(&self) -> [f32; 2] {
        self.value_with(&self.taps)
    }
    fn value_with(&self, taps: &[f32]) -> [f32; 2] {
        let mut v = [0.0; 2];
        let (a, b) = self.history.split_at(self.pos);
        for (x, t) in b.iter().chain(a.iter()).zip(taps) {
            v[0] += x[0] * t;
            v[1] += x[1] * t;
        }
        v
    }
}
struct Stage {
    filter: Fir,
    odd: bool,
}

pub struct Fm {
    stages: Vec<Stage>,
    channel: Fir,
    audio: Fir,
    audio_phases: Vec<Vec<f32>>,
    previous: [f32; 2],
    rate: f32,
    phase: f64,
    deemphasis: f32,
    alpha: f32,
    dc_in: f32,
    dc_out: f32,
}
impl Fm {
    pub fn new(rate: f64) -> Result<Self, String> {
        if !rate.is_finite() || !(240_000.0..=20_000_000.0).contains(&rate) {
            return Err(format!(
                "FM needs a server sample rate between 240 kHz and 20 MHz (received {rate})"
            ));
        }
        let mut rate = rate as f32;
        let mut stages = vec![];
        while rate > 600_000.0 {
            stages.push(Stage {
                filter: Fir::new(47, 0.22),
                odd: false,
            });
            rate /= 2.0;
        }
        Ok(Self {
            stages,
            channel: Fir::new(81, 100_000.0 / rate),
            audio: Fir::new(191, 15_000.0 / rate),
            audio_phases: (0..64)
                .map(|p| Fir::kernel(191, 15_000.0 / rate, p as f32 / 64.0))
                .collect(),
            previous: [0.0; 2],
            rate,
            phase: 0.0,
            deemphasis: 0.0,
            alpha: 1.0 - (-1.0 / (rate * 50e-6)).exp(),
            dc_in: 0.0,
            dc_out: 0.0,
        })
    }
    pub fn process(&mut self, samples: &[[f32; 2]], out: &mut Vec<f32>) {
        out.clear();
        'sample: for &input in samples {
            let mut x = input;
            for s in &mut self.stages {
                s.filter.push(x);
                s.odd = !s.odd;
                if s.odd {
                    continue 'sample;
                }
                x = s.filter.value();
            }
            self.channel.push(x);
            x = self.channel.value();
            let re = x[0] * self.previous[0] + x[1] * self.previous[1];
            let im = x[1] * self.previous[0] - x[0] * self.previous[1];
            self.previous = x;
            let demod = im.atan2(re) * self.rate / (2.0 * PI * 75_000.0);
            self.deemphasis += self.alpha * (demod - self.deemphasis);
            self.audio.push([self.deemphasis, 0.0]);
            // Only evaluate the long audio FIR when an output sample is due.
            self.phase += 48_000.0;
            if self.phase >= self.rate as f64 {
                self.phase -= self.rate as f64;
                // A fractional-delay polyphase bank avoids timing jitter when
                // the server sample rate is not an integer multiple of 48 kHz.
                let p = (self.phase / 48_000.0 * 64.0) as usize;
                let a = self.audio.value_with(&self.audio_phases[p.min(63)])[0];
                // DC blocker at roughly 30 Hz, after audio rate conversion.
                let dc = a - self.dc_in + 0.996 * self.dc_out;
                self.dc_in = a;
                self.dc_out = dc;
                out.push(dc.clamp(-1.0, 1.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(rate: f64, modulation: f64) -> Vec<f32> {
        let mut fm = Fm::new(rate).unwrap();
        let mut phase = 0.0f64;
        let input: Vec<[f32; 2]> = (0..(rate * 0.15) as usize)
            .map(|n| {
                phase += 2.0
                    * std::f64::consts::PI
                    * 45_000.0
                    * (2.0 * std::f64::consts::PI * modulation * n as f64 / rate).sin()
                    / rate;
                [phase.cos() as f32, phase.sin() as f32]
            })
            .collect();
        let mut out = vec![];
        let mut block = vec![];
        for chunk in input.chunks(7919) {
            fm.process(chunk, &mut block);
            out.extend_from_slice(&block);
        }
        out
    }
    fn rms(a: &[f32]) -> f32 {
        (a.iter().map(|x| x * x).sum::<f32>() / a.len() as f32).sqrt()
    }
    #[test]
    fn recovers_fm_tone_at_multiple_server_rates() {
        for rate in [250_000.0, 1_024_000.0, 2_400_000.0] {
            let out = tone(rate, 1000.0);
            assert!((out.len() as i32 - 7200).abs() <= 1);
            let a = &out[2000..];
            let energy = rms(a);
            assert!(energy > 0.3 && energy < 0.5, "{rate}: {energy}");
            let crossings = a.windows(2).filter(|p| p[0] <= 0.0 && p[1] > 0.0).count();
            assert!((crossings as f32 * 48_000.0 / a.len() as f32 - 1000.0).abs() < 12.0);
        }
    }
    #[test]
    fn rejects_stereo_pilot_from_mono_audio() {
        let low = tone(1_024_000.0, 1000.0);
        let high = tone(1_024_000.0, 19_000.0);
        assert!(rms(&high[2000..]) < rms(&low[2000..]) * 0.025);
    }
    #[test]
    fn invalid_rate() {
        assert!(Fm::new(f64::NAN).is_err());
        assert!(Fm::new(48_000.0).is_err());
    }
}
