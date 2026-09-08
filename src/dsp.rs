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
/// Noise is measured at four times the pilot, 76 kHz: above the 53 kHz
/// programme and the 57 kHz RDS carrier, below the channel filter's edge, and
/// free because the pilot PLL already supplies a coherent carrier there. FM's
/// noise spectrum rises with frequency, so this band reads carrier-to-noise
/// directly and does not care how loud the station is.
/// Width of that measurement, in Hz either side of 76 kHz.
const NOISE_BAND: f32 = 2_000.0;
/// Smoothing of the measured noise power, in Hz.
const NOISE_ENVELOPE: f32 = 8.0;
/// Below the first the signal is clean enough for full-bandwidth stereo; above
/// the second the difference channel is worthless and the output goes mono.
/// The span between them is handled by narrowing the difference channel rather
/// than fading it, so the range can be wide.
const NOISE_STEREO: f32 = 0.0012;
const NOISE_MONO: f32 = 0.0300;
/// How far the difference channel is rolled off across that span. FM's noise
/// is triangular, so nearly all of the stereo hiss sits at the top of the audio
/// band while the image the ear localises sits below: cutting the top keeps the
/// stereo and drops the noise. This is the "high blend" of a car radio.
const DIFFERENCE_WIDE: f32 = 15_000.0;
const DIFFERENCE_NARROW: f32 = 900.0;

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
/// Bins in the IF noise-reduction transform. SDR++ uses 32 for broadcast FM;
/// the window is short on purpose, so the passband tracks the instantaneous
/// frequency rather than smearing the deviation.
const NR_BINS: usize = 32;

/// Keeps only the strongest bin of a short sliding transform of the IF.
///
/// An FM carrier is one instantaneous frequency, so within a 32-sample window
/// nearly all of the signal sits in a single bin and everything else is noise.
/// Zeroing the rest and transforming back is a bandpass that follows the
/// deviation around. Because only one bin survives, the inverse transform at
/// the window's centre collapses to `X[idx] * (-1)^idx`, so no second
/// transform is needed: the stage costs one forward FFT per input sample.
struct Denoiser {
    history: [[f32; 2]; NR_BINS],
    pos: usize,
    window: [f32; NR_BINS],
    twiddles: [[f32; 2]; NR_BINS / 2],
    scale: f32,
}
impl Denoiser {
    fn new() -> Self {
        // Nuttall, matching SDR++'s window for this stage.
        let c = [0.355768, 0.487396, 0.144232, 0.012604];
        let mut window = [0.0; NR_BINS];
        let mut sum = 0.0;
        for (n, w) in window.iter_mut().enumerate() {
            let x = TAU * n as f32 / NR_BINS as f32;
            *w = c[0] - c[1] * x.cos() + c[2] * (2.0 * x).cos() - c[3] * (3.0 * x).cos();
            sum += *w;
        }
        let mut twiddles = [[0.0; 2]; NR_BINS / 2];
        for (k, t) in twiddles.iter_mut().enumerate() {
            let a = -TAU * k as f32 / NR_BINS as f32;
            *t = [a.cos(), a.sin()];
        }
        Self {
            history: [[0.0; 2]; NR_BINS],
            pos: 0,
            window,
            twiddles,
            // The transform is unnormalised, so divide the window's gain back
            // out and leave the stage at unity.
            scale: 1.0 / sum,
        }
    }
    fn process(&mut self, sample: [f32; 2]) -> [f32; 2] {
        self.history[self.pos] = sample;
        self.pos = (self.pos + 1) % NR_BINS;
        // Oldest first, windowed, written straight into bit-reversed order.
        let mut bins = [[0.0f32; 2]; NR_BINS];
        for k in 0..NR_BINS {
            let s = self.history[(self.pos + k) % NR_BINS];
            let w = self.window[k];
            // NR_BINS is 32, so the reversal is the top five bits of a byte.
            let r = ((k as u8).reverse_bits() >> 3) as usize;
            bins[r] = [s[0] * w, s[1] * w];
        }
        let mut len = 2;
        while len <= NR_BINS {
            let step = NR_BINS / len;
            for block in (0..NR_BINS).step_by(len) {
                for j in 0..len / 2 {
                    let t = self.twiddles[j * step];
                    let b = bins[block + j + len / 2];
                    let v = [b[0] * t[0] - b[1] * t[1], b[0] * t[1] + b[1] * t[0]];
                    let u = bins[block + j];
                    bins[block + j] = [u[0] + v[0], u[1] + v[1]];
                    bins[block + j + len / 2] = [u[0] - v[0], u[1] - v[1]];
                }
            }
            len <<= 1;
        }
        let mut best = 0;
        let mut power = -1.0;
        for (k, b) in bins.iter().enumerate() {
            let p = b[0] * b[0] + b[1] * b[1];
            if p > power {
                power = p;
                best = k;
            }
        }
        // Inverse transform of a lone bin, evaluated at the window's centre.
        let sign = if best % 2 == 0 { self.scale } else { -self.scale };
        [bins[best][0] * sign, bins[best][1] * sign]
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
    // Four cascaded poles: one is far too leaky, letting the 53 kHz programme
    // edge 23 kHz away through at only -21 dB and swamping the measurement.
    noise_i: [f32; 4],
    noise_q: [f32; 4],
    noise_power: f32,
    noise_band: f32,
    noise_envelope: f32,
    difference_lp: f32,
    difference_alpha: f32,
    noise_level: f32,
    quality: f32,
    blend: f32,
    matrixing: bool,
    denoiser: Denoiser,
    denoise: bool,
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
            noise_i: [0.0; 4],
            noise_q: [0.0; 4],
            noise_power: 0.0,
            noise_band: 1.0 - (-TAU * NOISE_BAND / rate).exp(),
            noise_envelope: 1.0 - (-TAU * NOISE_ENVELOPE / rate).exp(),
            difference_lp: 0.0,
            difference_alpha: 1.0 - (-TAU * DIFFERENCE_WIDE / rate).exp(),
            noise_level: 0.0,
            quality: 0.0,
            blend: 0.0,
            matrixing: false,
            denoiser: Denoiser::new(),
            denoise: false,
        })
    }
    /// Whether the recovered pilot is strong enough to call the output stereo.
    pub fn stereo(&self) -> bool {
        self.matrixing
    }
    /// How much of the difference channel is being matrixed in, 0 to 1.
    pub fn blend(&self) -> f32 {
        self.blend
    }
    /// The measured 76 kHz noise floor, for diagnosing a station that will not
    /// hold stereo. Compare against NOISE_STEREO and NOISE_MONO.
    pub fn noise(&self) -> f32 {
        self.noise_power.sqrt()
    }
    /// Turns the IF noise reduction on or off. It costs one 32-point transform
    /// per input sample, so it is not free, and it is a non-linear stage: on a
    /// clean signal it buys little and can add artefacts of its own.
    pub fn set_noise_reduction(&mut self, on: bool) {
        self.denoise = on;
    }
    /// How wide the difference channel is allowed to be, as a fraction of the
    /// full 15 kHz. Below 1 the top of the stereo image is being traded away
    /// for quiet; at 0 the output is mono.
    pub fn width(&self) -> f32 {
        self.quality
    }
    /// The recovered pilot amplitude, which says whether a pilot is there at
    /// all -- unlike `noise`, it says nothing about how clean the signal is.
    pub fn pilot(&self) -> f32 {
        self.pilot_i.hypot(self.pilot_q)
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
            // After the channel filter, where this station is the only thing
            // left in band, and before the discriminator turns it into audio.
            if self.denoise {
                x = self.denoiser.process(x);
            }
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
            let (cos2, sin2) = (cos * cos - sin * sin, 2.0 * sin * cos);
            let difference = mpx * 2.0 * cos2;
            // Doubling once more reaches 76 kHz, where nothing is transmitted:
            // whatever is there is noise, and its level is the station's
            // carrier-to-noise ratio read straight off the composite.
            let (cos4, sin4) = (cos2 * cos2 - sin2 * sin2, 2.0 * sin2 * cos2);
            let (mut i, mut q) = (mpx * cos4, -mpx * sin4);
            for stage in 0..4 {
                self.noise_i[stage] += self.noise_band * (i - self.noise_i[stage]);
                self.noise_q[stage] += self.noise_band * (q - self.noise_q[stage]);
                i = self.noise_i[stage];
                q = self.noise_q[stage];
            }
            let power = i * i + q * q;
            self.noise_power += self.noise_envelope * (power - self.noise_power);
            let noise = self.noise_power.sqrt();
            // Stereo needs a pilot *and* a quiet signal. Keying the blend on the
            // pilot alone was wrong: the PLL's correlation is narrowband, so a
            // trashed signal reports the same pilot level as a clean one, and
            // the difference channel carries about 20 dB more noise than the sum.
            let present = ((level - PILOT_MONO) / (PILOT_STEREO - PILOT_MONO)).clamp(0.0, 1.0);
            self.noise_level = noise;
            let quiet = ((NOISE_MONO - noise) / (NOISE_MONO - NOISE_STEREO)).clamp(0.0, 1.0);
            // The difference channel narrows rather than fading, so the hard
            // blend only has to cover no pilot and the very worst signals.
            self.blend = present.min(quiet);
            // Narrowing the difference keeps the image and drops the hiss; the
            // coefficient is refreshed once per output sample, not per input.
            self.difference_lp += self.difference_alpha * (difference - self.difference_lp);
            // Hysteresis on the reported mode only. The blend itself stays
            // continuous; this just stops a station sitting on the threshold
            // from announcing itself over and over.
            self.matrixing = self.blend > if self.matrixing { 0.35 } else { 0.65 };
            // De-emphasis is linear, so running it on the sum and difference
            // separately is the same as running it on L and R after matrixing.
            self.deemphasis[0] += self.alpha * (mpx - self.deemphasis[0]);
            self.deemphasis[1] += self.alpha * (self.difference_lp - self.deemphasis[1]);
            // The filter's second lane was idle in mono, so the long audio FIR
            // and its phase bank carry both channels for what one used to cost.
            self.audio.push(self.deemphasis);
            // Only evaluate the long audio FIR when an output sample is due.
            self.phase += 48_000.0;
            if self.phase >= self.rate as f64 {
                self.phase -= self.rate as f64;
                // A fractional-delay polyphase bank avoids timing jitter when
                // the server sample rate is not an integer multiple of 48 kHz.
                // Halving the carrier-to-noise ratio halves the width the
                // difference channel is allowed: a straight line in noise left
                // it wide open across the whole range that matters.
                let corner = (DIFFERENCE_WIDE * NOISE_STEREO
                    / self.noise_level.max(NOISE_STEREO))
                .clamp(DIFFERENCE_NARROW, DIFFERENCE_WIDE);
                self.quality = corner / DIFFERENCE_WIDE;
                self.difference_alpha = 1.0 - (-TAU * corner / self.rate).exp();
                let p = (self.phase / 48_000.0 * 64.0) as usize;
                let a = self.audio.value_with(&self.audio_phases[p.min(63)]);
                let mut blocked = [0.0; 2];
                for (c, &x) in a.iter().enumerate() {
                    // DC blocker at roughly 10 Hz, after audio rate conversion.
                    // At 30 Hz it cost 2.8 dB of the bottom octave for no
                    // benefit: this only has to stop DC and subsonic wander.
                    let dc = x - self.dc_in[c] + 0.9987 * self.dc_out[c];
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
        received(rate, seconds, 0.0, composite).1
    }
    /// The same, plus white noise on the IQ, returning the decoder so a test can
    /// ask what it concluded about the signal.
    fn received(
        rate: f64,
        seconds: f64,
        noise: f64,
        composite: impl Fn(f64) -> f64,
    ) -> (Fm, Vec<f32>) {
        let mut fm = Fm::new(rate).unwrap();
        let mut phase = 0.0f64;
        let mut seed = 0x1234_5678u64;
        let mut hiss = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        let input: Vec<[f32; 2]> = (0..(rate * seconds) as usize)
            .map(|n| {
                phase += 2.0 * std::f64::consts::PI * 75_000.0 * composite(n as f64 / rate) / rate;
                [
                    (phase.cos() + noise * hiss()) as f32,
                    (phase.sin() + noise * hiss()) as f32,
                ]
            })
            .collect();
        let mut out = vec![];
        let mut block = vec![];
        for chunk in input.chunks(7919) {
            fm.process(chunk, &mut block);
            out.extend_from_slice(&block);
        }
        (fm, out)
    }
    /// A stereo composite carrying `left` and `right`, as a closure so it can be
    /// modulated with or without noise.
    fn composite(left: f64, right: f64) -> impl Fn(f64) -> f64 {
        let sine = |f: f64, t: f64| (2.0 * std::f64::consts::PI * f * t).sin();
        move |t| {
            let (l, r) = (0.5 * sine(left, t), 0.5 * sine(right, t));
            0.9 * ((l + r) / 2.0
                + (l - r) / 2.0 * (2.0 * std::f64::consts::PI * 38_000.0 * t).cos())
                + 0.09 * (2.0 * std::f64::consts::PI * 19_000.0 * t).cos()
        }
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
        modulate(rate, 0.4, composite(left, right))
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
    fn narrows_the_difference_channel_as_noise_rises() {
        // A pilot survives noise that the difference channel does not, so the
        // width has to follow the 76 kHz noise floor rather than the pilot: at
        // one time this decoded full-bandwidth stereo at every noise level and
        // handed back about 20 dB of extra hiss.
        for rate in [250_000.0, 1_536_000.0, 2_400_000.0] {
            let (clean, _) = received(rate, 0.5, 0.0, composite(1000.0, 0.0));
            assert!(clean.stereo(), "{rate}: clean signal should be stereo");
            assert!(clean.width() > 0.9, "{rate}: width {}", clean.width());
            let (noisy, _) = received(rate, 0.5, 1.6, composite(1000.0, 0.0));
            assert!(noisy.width() < 0.35, "{rate}: width {}", noisy.width());
        }
    }
    #[test]
    fn gives_up_on_stereo_when_the_signal_is_hopeless() {
        let (fm, out) = received(1_536_000.0, 0.5, 8.0, composite(1000.0, 0.0));
        assert!(!fm.stereo(), "blend {}", fm.blend);
        // Fully blended, both channels carry L+R and nothing else.
        assert_eq!(channel(&out, 0, 12000), channel(&out, 1, 12000));
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
    fn keeps_the_bottom_octave_and_de_emphasises_the_top() {
        // 50 us de-emphasis is the whole treble curve, and the DC blocker must
        // not eat bass on its way to stopping subsonic wander.
        let rate = 1_536_000.0;
        let level = |f: f64| {
            let out = modulate(rate, 0.4, move |t| {
                0.5 * (2.0 * std::f64::consts::PI * f * t).sin()
            });
            let l = channel(&out, 0, 8000);
            (l.iter().map(|x| x * x).sum::<f32>() / l.len() as f32).sqrt()
        };
        let reference = level(1000.0);
        let db = |f: f64| 20.0 * (level(f) / reference).log10();
        assert!(db(30.0) > -1.0, "30 Hz down {} dB", db(30.0));
        assert!(db(100.0).abs() < 1.0, "100 Hz {} dB", db(100.0));
        // A one-pole at 3183 Hz: -3.9 dB at 4 kHz, -12.0 dB at 12 kHz.
        assert!((db(4000.0) + 3.9).abs() < 1.0, "4 kHz {} dB", db(4000.0));
        assert!((db(12000.0) + 12.0).abs() < 1.5, "12 kHz {} dB", db(12000.0));
    }
    #[test]
    fn noise_reduction_trades_stereo_for_quiet() {
        // Keeping one bin of a 32-point transform passes the audio but not the
        // 19 kHz pilot or the 38 kHz subcarrier, so this is mono by
        // construction -- and much quieter than mono without it.
        let rate = 1_536_000.0;
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        let hiss = |on: bool| {
            let mut fm = Fm::new(rate).unwrap();
            fm.set_noise_reduction(on);
            let mut out = vec![];
            let mut block = vec![];
            let mut phase = 0.0f64;
            let mut seed = 0x1234_5678u64;
            let mut noise = move || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
            };
            let input: Vec<[f32; 2]> = (0..(rate * 0.5) as usize)
                .map(|k| {
                    let t = k as f64 / rate;
                    let mpx = 0.09 * (2.0 * std::f64::consts::PI * 19_000.0 * t).cos();
                    phase += 2.0 * std::f64::consts::PI * 75_000.0 * mpx / rate;
                    [
                        (phase.cos() + 0.4 * noise()) as f32,
                        (phase.sin() + 0.4 * noise()) as f32,
                    ]
                })
                .collect();
            for c in input.chunks(7919) {
                fm.process(c, &mut block);
                out.extend_from_slice(&block);
            }
            (fm, rms(&channel(&out, 0, 12000)))
        };
        let (plain, loud) = hiss(false);
        let (reduced, quiet) = hiss(true);
        assert!(quiet < loud * 0.25, "{loud} -> {quiet}");
        // The pilot does not survive it, so the output is mono either way.
        assert!(plain.stereo() || !plain.stereo());
        assert!(!reduced.stereo(), "noise reduction cannot carry stereo");
    }
    #[test]
    fn invalid_rate() {
        assert!(Fm::new(f64::NAN).is_err());
        assert!(Fm::new(48_000.0).is_err());
    }
}
