//! FFT-based feature extraction. Everything an effect reacts to is in
//! `Features`, normalized to roughly 0..1 via auto-gain so effects work the
//! same for a quiet microphone and a loud loopback device.

use crate::detect::{Detector, Detectors, Input};
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::collections::VecDeque;
use std::sync::Arc;

pub const FFT_SIZE: usize = 2048;
pub const SPECTRUM_BINS: usize = 64;
pub const WAVEFORM_LEN: usize = 512;
const BEAT_HISTORY: usize = 43;

#[derive(Default)]
pub struct Features {
    /// Overall loudness.
    pub level: f32,
    /// Band energies: 20–150 Hz, 150–2000 Hz, 2–10 kHz.
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    /// True only on the frame a beat was detected.
    pub beat: bool,
    /// 1.0 on a beat, decays exponentially towards 0.
    pub pulse: f32,
    /// Log-spaced spectrum, `SPECTRUM_BINS` values.
    pub spectrum: Vec<f32>,
    /// Most recent samples, normalized to -1..1, `WAVEFORM_LEN` values.
    pub waveform: Vec<f32>,
}

/// Divides by a slowly decaying running peak.
struct AutoGain {
    peak: f32,
    floor: f32,
}

impl AutoGain {
    fn new(floor: f32) -> Self {
        Self { peak: floor, floor }
    }

    fn apply(&mut self, raw: f32, dt: f32) -> f32 {
        // Peak halves every 4 seconds of quieter input.
        self.peak = raw.max(self.peak * 0.5f32.powf(dt / 4.0)).max(self.floor);
        (raw / self.peak).min(1.0)
    }
}

/// Fast attack, slow release smoothing.
fn follow(current: f32, target: f32, dt: f32) -> f32 {
    let rate = if target > current { 25.0 } else { 5.0 };
    current + (target - current) * (1.0 - (-rate * dt).exp())
}

pub struct Analyzer {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    buf: Vec<Complex<f32>>,
    mags: Vec<f32>,
    hz_per_bin: f32,
    bin_edges: Vec<(usize, usize)>,
    level_gain: AutoGain,
    bass_gain: AutoGain,
    mid_gain: AutoGain,
    treble_gain: AutoGain,
    spectrum_gain: AutoGain,
    wave_gain: AutoGain,
    bass_history: VecDeque<f32>,
    last_beat: f32,
    detectors: Detectors,
    /// Which detector drives `features.beat` and `features.pulse`.
    pub detector: Detector,
    pub features: Features,
}

impl Analyzer {
    pub fn new(sample_rate: f32) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT_SIZE as f32).cos())
            .collect();
        let hz_per_bin = sample_rate / FFT_SIZE as f32;
        let max_bin = FFT_SIZE / 2 - 1;
        let (lo, hi) = (30.0f32, 16_000.0f32.min(sample_rate / 2.0));
        // At low frequencies the log spacing is finer than the FFT resolution,
        // so each band starts no earlier than where the previous one ended.
        // That way neighboring bars never show the same FFT bin.
        let mut next_free = 1;
        let bin_edges = (0..SPECTRUM_BINS)
            .map(|i| {
                let f0 = lo * (hi / lo).powf(i as f32 / SPECTRUM_BINS as f32);
                let f1 = lo * (hi / lo).powf((i + 1) as f32 / SPECTRUM_BINS as f32);
                let k0 = ((f0 / hz_per_bin) as usize).max(next_free).min(max_bin);
                let k1 = ((f1 / hz_per_bin) as usize).clamp(k0 + 1, max_bin + 1);
                next_free = k1;
                (k0, k1)
            })
            .collect();
        Self {
            fft,
            window,
            buf: vec![Complex::default(); FFT_SIZE],
            mags: vec![0.0; FFT_SIZE / 2],
            hz_per_bin,
            bin_edges,
            level_gain: AutoGain::new(0.005),
            bass_gain: AutoGain::new(0.003),
            mid_gain: AutoGain::new(0.002),
            treble_gain: AutoGain::new(0.001),
            spectrum_gain: AutoGain::new(0.002),
            wave_gain: AutoGain::new(0.01),
            bass_history: VecDeque::with_capacity(BEAT_HISTORY + 1),
            last_beat: f32::NEG_INFINITY,
            detectors: Detectors::new(FFT_SIZE / 2, hz_per_bin),
            detector: Detector::default(),
            features: Features {
                spectrum: vec![0.0; SPECTRUM_BINS],
                waveform: vec![0.0; WAVEFORM_LEN],
                ..Default::default()
            },
        }
    }

    /// `samples` are the most recent mono samples; shorter input is zero-padded.
    pub fn update(&mut self, samples: &[f32], t: f32, dt: f32) {
        let tail = &samples[samples.len().saturating_sub(FFT_SIZE)..];
        let pad = FFT_SIZE - tail.len();
        for (i, c) in self.buf.iter_mut().enumerate() {
            let s = if i < pad { 0.0 } else { tail[i - pad] };
            *c = Complex::new(s * self.window[i], 0.0);
        }
        self.fft.process(&mut self.buf);
        // 2/N for the single-sided spectrum, /0.5 for the Hann window gain:
        // a full-scale sine ends up at ~1.0.
        let norm = 4.0 / FFT_SIZE as f32;
        for (m, c) in self.mags.iter_mut().zip(&self.buf) {
            *m = c.norm() * norm;
        }

        let raw_level = (tail.iter().map(|s| s * s).sum::<f32>() / tail.len().max(1) as f32).sqrt();
        let raw_bass = self.band(20.0, 150.0);
        let raw_mid = self.band(150.0, 2000.0);
        let raw_treble = self.band(2000.0, 10_000.0);

        let f = &mut self.features;
        f.level = follow(f.level, self.level_gain.apply(raw_level, dt), dt);
        f.bass = follow(f.bass, self.bass_gain.apply(raw_bass, dt), dt);
        f.mid = follow(f.mid, self.mid_gain.apply(raw_mid, dt), dt);
        f.treble = follow(f.treble, self.treble_gain.apply(raw_treble, dt), dt);

        // Beat = bass energy clearly above its recent average.
        self.bass_history.push_back(raw_bass);
        if self.bass_history.len() > BEAT_HISTORY {
            self.bass_history.pop_front();
        }
        let avg = self.bass_history.iter().sum::<f32>() / self.bass_history.len() as f32;
        let kick = raw_bass > avg * 1.4 && raw_bass > self.bass_gain.floor * 2.0 && t - self.last_beat > 0.18;
        if kick {
            self.last_beat = t;
        }
        self.detectors.update(&Input {
            mags: &self.mags,
            hz_per_bin: self.hz_per_bin,
            level: raw_level,
            kick,
            t,
            dt,
        });
        let index = Detector::ALL.iter().position(|&d| d == self.detector).unwrap_or(0);
        f.beat = self.detectors.beats[index];
        if f.beat {
            f.pulse = 1.0;
        } else {
            // A drop's pulse fades slower, so it hits harder.
            let fade = if self.detector == Detector::Drop && self.detectors.drop_glow(t) {
                2.0
            } else {
                6.0
            };
            f.pulse *= (-dt * fade).exp();
        }

        // Log-spaced spectrum in dB relative to the running peak, with a tilt
        // so high frequencies are not drowned out by the bass.
        let mut bins = [0.0f32; SPECTRUM_BINS];
        let mut loudest = 0.0f32;
        for (i, &(k0, k1)) in self.bin_edges.iter().enumerate() {
            let mag = self.mags[k0..k1].iter().copied().fold(0.0, f32::max);
            let freq = (k0 + k1) as f32 * 0.5 * self.hz_per_bin;
            bins[i] = mag * (freq / 1000.0).powf(0.3);
            loudest = loudest.max(bins[i]);
        }
        self.spectrum_gain.apply(loudest, dt);
        let peak = self.spectrum_gain.peak;
        for (s, &b) in f.spectrum.iter_mut().zip(&bins) {
            let db = 20.0 * (b.max(1e-9) / peak).log10();
            *s = follow(*s, (1.0 + db / 50.0).clamp(0.0, 1.0), dt);
        }

        let wave = &tail[tail.len().saturating_sub(WAVEFORM_LEN)..];
        let wave_peak = wave.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        self.wave_gain.apply(wave_peak, dt);
        let gain = 1.0 / self.wave_gain.peak;
        let offset = WAVEFORM_LEN - wave.len();
        f.waveform.fill(0.0);
        for (w, s) in f.waveform[offset..].iter_mut().zip(wave) {
            *w = (s * gain).clamp(-1.0, 1.0);
        }
    }

    /// This frame's beat for every detector, in `Detector::ALL` order.
    #[cfg(test)]
    pub fn detector_beats(&self) -> [bool; 9] {
        self.detectors.beats
    }

    /// Current tempo estimate, if the music has a steady beat.
    pub fn bpm(&self) -> Option<f32> {
        self.detectors.bpm()
    }

    /// True on the frame the music seems to move to a new section (a drop,
    /// a breakdown or a clear change of sound). Experimental.
    pub fn section_change(&self) -> bool {
        self.detectors.section_change
    }

    /// RMS magnitude over a frequency range.
    fn band(&self, lo: f32, hi: f32) -> f32 {
        let max_bin = self.mags.len() - 1;
        let k0 = ((lo / self.hz_per_bin).round() as usize).clamp(1, max_bin);
        let k1 = ((hi / self.hz_per_bin).round() as usize).clamp(k0, max_bin);
        let slice = &self.mags[k0..=k1];
        (slice.iter().map(|m| m * m).sum::<f32>() / slice.len() as f32).sqrt()
    }
}
