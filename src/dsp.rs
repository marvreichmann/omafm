//! Streaming stereo WFM: staged anti-alias filtering, channel filter, phase
//! discriminator, 19 kHz pilot PLL, 38 kHz L-R recovery, 15 kHz audio filter,
//! 50 us de-emphasis, 48 kHz resampling.
use std::f32::consts::{PI, TAU};

/// A pilot correlation below this reads as no pilot; above the second value the
/// output is fully stereo. A pilot at the standard 9 % deviation correlates at
/// about 0.045, so the window sits well under a healthy signal.
const PILOT_MONO: f32 = 0.012;
const PILOT_STEREO: f32 = 0.030;
/// Divisor floor for the normalised phase detector, under the mono threshold so
/// that noise alone cannot spin the loop.
const PILOT_FLOOR: f32 = 0.004;
/// PLL natural frequency in Hz and damping. 150 Hz locks well inside the 150 ms
/// mute that follows a retune, while keeping the loop's noise bandwidth far
/// below the 4 kHz gap between the pilot and the top of the audio band.
const PLL_NATURAL: f32 = 150.0;
const PLL_DAMPING: f32 = 0.707;
/// Widest pilot offset the loop may pull in, in Hz.
const PLL_PULL: f32 = 500.0;
/// Pilot envelope smoothing in Hz: slow enough to ignore programme material,
/// fast enough that stereo appears shortly after audio does.
const PILOT_ENVELOPE: f32 = 20.0;

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
    deemphasis: [f32; 2],
    alpha: f32,
    dc_in: [f32; 2],
    dc_out: [f32; 2],
    pilot_phase: f32,
    pilot_step: f32,
    pilot_offset: f32,
    pilot_i: f32,
    pilot_q: f32,
    pilot_envelope: f32,
    pll_pull: f32,
    pll_gain: f32,
    pll_integrator: f32,
    blend: f32,
    matrixing: bool,
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
            deemphasis: [0.0; 2],
            alpha: 1.0 - (-1.0 / (rate * 50e-6)).exp(),
            dc_in: [0.0; 2],
            dc_out: [0.0; 2],
            pilot_phase: 0.0,
            pilot_step: TAU * 19_000.0 / rate,
            pilot_offset: 0.0,
            pilot_i: 0.0,
            pilot_q: 0.0,
            pilot_envelope: 1.0 - (-TAU * PILOT_ENVELOPE / rate).exp(),
            pll_pull: TAU * PLL_PULL / rate,
            pll_gain: 2.0 * PLL_DAMPING * TAU * PLL_NATURAL / rate,
            pll_integrator: (TAU * PLL_NATURAL / rate).powi(2),
            blend: 0.0,
            matrixing: false,
        })
    }
    /// Whether the recovered pilot is strong enough to call the output stereo.
    pub fn stereo(&self) -> bool {
        self.matrixing
    }
    /// Writes interleaved left/right pairs at 48 kHz.
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
            // The composite: L+R at baseband, a 19 kHz pilot, and L-R on a
            // suppressed 38 kHz carrier. The pilot has to be read here, before
            // de-emphasis tilts everything above the audio band.
            let mpx = im.atan2(re) * self.rate / (2.0 * PI * 75_000.0);
            let (sin, cos) = self.pilot_phase.sin_cos();
            // Correlating against the local oscillator measures the pilot and
            // detects lock at once: an unlocked oscillator averages to nothing.
            self.pilot_i += self.pilot_envelope * (mpx * cos - self.pilot_i);
            self.pilot_q += self.pilot_envelope * (-mpx * sin - self.pilot_q);
            let level = self.pilot_i.hypot(self.pilot_q);
            // Dividing by the envelope keeps the loop gain independent of signal
            // strength, so lock time does not depend on how strong the station is.
            let error = (-mpx * sin / level.max(PILOT_FLOOR)).clamp(-1.0, 1.0);
            self.pilot_offset = (self.pilot_offset + self.pll_integrator * error)
                .clamp(-self.pll_pull, self.pll_pull);
            self.pilot_phase =
                (self.pilot_phase + self.pilot_step + self.pilot_offset + self.pll_gain * error)
                    .rem_euclid(TAU);
            // Doubling the pilot gives the 38 kHz carrier. The product lands L-R
            // at baseband next to an image at 76 kHz that the audio filter drops.
            let difference = mpx * 2.0 * (cos * cos - sin * sin);
            self.blend = ((level - PILOT_MONO) / (PILOT_STEREO - PILOT_MONO)).clamp(0.0, 1.0);
            // Hysteresis on the reported mode only. The blend itself stays
            // continuous; this just stops a station sitting on the threshold
            // from announcing itself over and over.
            self.matrixing = self.blend > if self.matrixing { 0.35 } else { 0.65 };
            // De-emphasis is linear, so running it on the sum and difference
            // separately is the same as running it on L and R after matrixing.
            self.deemphasis[0] += self.alpha * (mpx - self.deemphasis[0]);
            self.deemphasis[1] += self.alpha * (difference - self.deemphasis[1]);
            // The filter's second lane was idle in mono, so the long audio FIR
            // and its phase bank carry both channels for what one used to cost.
            self.audio.push(self.deemphasis);
            // Only evaluate the long audio FIR when an output sample is due.
            self.phase += 48_000.0;
            if self.phase >= self.rate as f64 {
                self.phase -= self.rate as f64;
                // A fractional-delay polyphase bank avoids timing jitter when
                // the server sample rate is not an integer multiple of 48 kHz.
                let p = (self.phase / 48_000.0 * 64.0) as usize;
                let a = self.audio.value_with(&self.audio_phases[p.min(63)]);
                let mut blocked = [0.0; 2];
                for (c, &x) in a.iter().enumerate() {
                    // DC blocker at roughly 30 Hz, after audio rate conversion.
                    let dc = x - self.dc_in[c] + 0.996 * self.dc_out[c];
                    self.dc_in[c] = x;
                    self.dc_out[c] = dc;
                    blocked[c] = dc;
                }
                // Fading the difference out rather than switching means a weak
                // station loses separation before it gains hiss. Mono material
                // keeps its level exactly, because the difference is then zero.
                let (sum, difference) = (blocked[0], blocked[1] * self.blend);
                out.push((sum + difference).clamp(-1.0, 1.0));
                out.push((sum - difference).clamp(-1.0, 1.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// FM-modulates a composite signal and returns interleaved 48 kHz stereo.
    fn modulate(rate: f64, seconds: f64, composite: impl Fn(f64) -> f64) -> Vec<f32> {
        let mut fm = Fm::new(rate).unwrap();
        let mut phase = 0.0f64;
        let input: Vec<[f32; 2]> = (0..(rate * seconds) as usize)
            .map(|n| {
                phase += 2.0 * std::f64::consts::PI * 75_000.0 * composite(n as f64 / rate) / rate;
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
    /// A mono station: no pilot, no subcarrier, 60 % deviation.
    fn tone(rate: f64, modulation: f64) -> Vec<f32> {
        modulate(rate, 0.15, |t| {
            0.6 * (2.0 * std::f64::consts::PI * modulation * t).sin()
        })
    }
    /// A stereo station carrying `left` and `right`, at the standard 9 % pilot
    /// injection with the remaining 90 % shared by the sum and difference.
    fn stereo(rate: f64, left: f64, right: f64) -> Vec<f32> {
        let sine = |f: f64, t: f64| (2.0 * std::f64::consts::PI * f * t).sin();
        modulate(rate, 0.4, move |t| {
            let (l, r) = (0.5 * sine(left, t), 0.5 * sine(right, t));
            0.9 * ((l + r) / 2.0
                + (l - r) / 2.0 * (2.0 * std::f64::consts::PI * 38_000.0 * t).cos())
                + 0.09 * (2.0 * std::f64::consts::PI * 19_000.0 * t).cos()
        })
    }
    fn channel(out: &[f32], index: usize, skip: usize) -> Vec<f32> {
        out.as_chunks::<2>()
            .0
            .iter()
            .skip(skip)
            .map(|f| f[index])
            .collect()
    }
    fn rms(a: &[f32]) -> f32 {
        (a.iter().map(|x| x * x).sum::<f32>() / a.len() as f32).sqrt()
    }
    #[test]
    fn recovers_fm_tone_at_multiple_server_rates() {
        for rate in [250_000.0, 1_024_000.0, 2_400_000.0] {
            let out = tone(rate, 1000.0);
            assert!((out.len() as i32 - 14400).abs() <= 2);
            let a = channel(&out, 0, 2000);
            // A mono transmission must reach both channels at the same level.
            assert_eq!(a, channel(&out, 1, 2000));
            let energy = rms(&a);
            assert!(energy > 0.3 && energy < 0.5, "{rate}: {energy}");
            let crossings = a.windows(2).filter(|p| p[0] <= 0.0 && p[1] > 0.0).count();
            assert!((crossings as f32 * 48_000.0 / a.len() as f32 - 1000.0).abs() < 12.0);
        }
    }
    #[test]
    fn rejects_stereo_pilot_from_mono_audio() {
        let low = tone(1_024_000.0, 1000.0);
        let high = tone(1_024_000.0, 19_000.0);
        assert!(rms(&channel(&high, 0, 2000)) < rms(&channel(&low, 0, 2000)) * 0.025);
    }
    #[test]
    fn separates_stereo_channels_at_multiple_server_rates() {
        for rate in [250_000.0, 1_024_000.0, 2_400_000.0] {
            // Left carries a tone, right is silent; the pilot must survive
            // demodulation well enough to keep the two apart.
            let out = stereo(rate, 1000.0, 0.0);
            let (l, r) = (channel(&out, 0, 8000), channel(&out, 1, 8000));
            let wanted = rms(&l);
            // 0.9 pilot injection loss on a half-amplitude tone.
            assert!(
                (wanted - 0.45 / 2.0f32.sqrt()).abs() < 0.03,
                "{rate}: {wanted}"
            );
            // Measured 37-42 dB across these rates; the bound leaves headroom.
            let separation = 20.0 * (rms(&r) / wanted).log10();
            assert!(separation < -25.0, "{rate}: {separation} dB");
        }
    }
    #[test]
    fn falls_back_to_mono_without_a_pilot() {
        // The same programme with no pilot must stay matrixed off: both
        // channels carry L+R rather than leaking the difference.
        let out = modulate(250_000.0, 0.4, |t| {
            0.5 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin()
        });
        assert_eq!(channel(&out, 0, 8000), channel(&out, 1, 8000));
    }
    #[test]
    fn invalid_rate() {
        assert!(Fm::new(f64::NAN).is_err());
        assert!(Fm::new(48_000.0).is_err());
    }
}
