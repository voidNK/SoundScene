//! Beat detectors. Each one turns a different musical cue into the `beat`
//! flag that effects react to. All of them run every frame, so switching is
//! instant and the tempo tracker is already locked in when it gets picked.

use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, clap::ValueEnum)]
pub enum Detector {
    /// Jumps in bass energy: the kick drum.
    #[default]
    Kick,
    /// Any new note or hit, anywhere in the spectrum (spectral flux).
    Onset,
    /// Noisy hits in the upper mids, usually beats 2 and 4.
    Snare,
    /// Short hits in the highs above 6 kHz.
    HiHat,
    /// Beats on an estimated tempo grid, steady through breaks.
    Tempo,
    /// Only the first beat of each 4/4 bar.
    Bar,
    /// Kicks only while the track is in full swing, plus a hit on every drop.
    Drop,
    /// Chord and key changes.
    Harmony,
    /// Jumps in overall loudness.
    Loudness,
}

impl Detector {
    pub const ALL: [Detector; 9] = [
        Detector::Kick,
        Detector::Onset,
        Detector::Snare,
        Detector::HiHat,
        Detector::Tempo,
        Detector::Bar,
        Detector::Drop,
        Detector::Harmony,
        Detector::Loudness,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Detector::Kick => "kick",
            Detector::Onset => "onset",
            Detector::Snare => "snare",
            Detector::HiHat => "hi-hat",
            Detector::Tempo => "tempo",
            Detector::Bar => "bar",
            Detector::Drop => "drop",
            Detector::Harmony => "harmony",
            Detector::Loudness => "loudness",
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&d| d == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// When to switch to the next detector on its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Rotation {
    #[default]
    Off,
    /// Every n seconds.
    Every(u64),
    /// When the music moves to a new section (experimental).
    Events,
}

impl std::str::FromStr for Rotation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "off" | "0" => Ok(Rotation::Off),
            "events" => Ok(Rotation::Events),
            n => n
                .parse()
                .map(Rotation::Every)
                .map_err(|_| format!("expected off, events or a number of seconds, got {s:?}")),
        }
    }
}

impl std::fmt::Display for Rotation {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Rotation::Off => f.write_str("off"),
            Rotation::Every(secs) => write!(f, "{secs}"),
            Rotation::Events => f.write_str("events"),
        }
    }
}

/// Peak picking with an adaptive threshold: a hit is a value clearly above
/// its recent mean, measured in standard deviations.
struct Picker {
    mean: f32,
    var: f32,
    last: f32,
    /// How many standard deviations above the mean count as a hit.
    k: f32,
    /// Time constant of the running mean, in seconds.
    tau: f32,
    /// Minimum time between hits, in seconds.
    lockout: f32,
    /// Absolute minimum, so noise in silence never triggers.
    floor: f32,
}

impl Picker {
    fn new(k: f32, tau: f32, lockout: f32, floor: f32) -> Self {
        Self {
            mean: 0.0,
            var: 0.0,
            last: f32::NEG_INFINITY,
            k,
            tau,
            lockout,
            floor,
        }
    }

    fn step(&mut self, x: f32, t: f32, dt: f32) -> bool {
        self.step_with(x, 1.0, t, dt)
    }

    /// Like `step`, with the threshold distance raised by `strict`.
    fn step_with(&mut self, x: f32, strict: f32, t: f32, dt: f32) -> bool {
        let hit = x > self.mean + strict * self.k * self.var.sqrt() && x > self.floor && t - self.last > self.lockout;
        if hit {
            self.last = t;
        }
        let a = 1.0 - (-dt / self.tau).exp();
        let d = x - self.mean;
        self.mean += a * d;
        self.var = (1.0 - a) * (self.var + a * d * d);
        hit
    }
}

/// Smooths towards a target with time constant `tau`.
fn ease(current: f32, target: f32, tau: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-dt / tau).exp())
}

/// Rate of the onset envelope used for tempo estimation.
const ENV_RATE: f32 = 60.0;
/// Seconds of onset envelope kept for tempo estimation.
const ENV_SECONDS: f32 = 8.0;
/// Tempo search range in BPM.
const BPM_RANGE: (f32, f32) = (60.0, 200.0);

/// Estimates tempo and beat phase from the onset envelope by autocorrelation
/// and fires beats on the predicted grid.
struct TempoTracker {
    env: VecDeque<f32>,
    tick: f32,
    next_eval: f32,
    /// Beat period in envelope samples, 0 while unknown.
    period: f32,
    candidate: f32,
    confidence: f32,
    next_beat: f32,
    beat_index: u64,
    /// Evidence per position in the bar for being beat 1.
    bar_weight: [f32; 4],
    /// Bar position currently taken as beat 1.
    downbeat: usize,
}

impl TempoTracker {
    fn new() -> Self {
        let len = (ENV_RATE * ENV_SECONDS) as usize;
        Self {
            env: VecDeque::with_capacity(len + 1),
            tick: 0.0,
            next_eval: 0.0,
            period: 0.0,
            candidate: 0.0,
            confidence: 0.0,
            next_beat: f32::INFINITY,
            beat_index: 0,
            bar_weight: [0.0; 4],
            downbeat: 0,
        }
    }

    fn bpm(&self) -> Option<f32> {
        (self.period > 0.0 && self.confidence > 0.15).then(|| 60.0 * ENV_RATE / self.period)
    }

    /// Returns (beat, downbeat) for this frame. `accents` are evidence for a
    /// bar start, positive for kicks and chord changes, negative for snares,
    /// each with the seconds its detector lags behind the music.
    fn step(&mut self, onset: f32, accents: &[(f32, f32)], t: f32, dt: f32) -> (bool, bool) {
        let len = (ENV_RATE * ENV_SECONDS) as usize;
        self.tick += dt * ENV_RATE;
        while self.tick >= 1.0 {
            self.tick -= 1.0;
            self.env.push_back(onset);
            if self.env.len() > len {
                self.env.pop_front();
            }
        }
        if t >= self.next_eval && self.env.len() >= len / 2 {
            self.next_eval = t + 0.5;
            self.estimate(t);
        }

        let Some(bpm) = self.bpm() else {
            self.next_beat = f32::INFINITY;
            return (false, false);
        };
        let period_s = 60.0 / bpm;
        if !self.next_beat.is_finite() || t > self.next_beat + period_s {
            self.next_beat = t + period_s;
        }
        // Credit each accent to the nearest beat on the grid.
        for &(accent, lag) in accents.iter().filter(|a| a.0 != 0.0) {
            let upcoming = self.next_beat - (t - lag) < period_s * 0.5;
            let pos = ((self.beat_index + upcoming as u64) % 4) as usize;
            self.bar_weight[pos] += accent;
        }
        if t < self.next_beat {
            return (false, false);
        }
        self.next_beat += period_s;
        self.beat_index += 1;
        for w in &mut self.bar_weight {
            *w *= 0.98;
        }
        // Only move beat 1 when another position is clearly ahead.
        let best = (0..4)
            .max_by(|&a, &b| self.bar_weight[a].total_cmp(&self.bar_weight[b]))
            .unwrap_or(0);
        if self.bar_weight[best] > self.bar_weight[self.downbeat] + 1.0 {
            self.downbeat = best;
        }
        (true, (self.beat_index % 4) as usize == self.downbeat)
    }

    fn estimate(&mut self, t: f32) {
        let n = self.env.len();
        let mean = self.env.iter().sum::<f32>() / n as f32;
        let env: Vec<f32> = self.env.iter().map(|v| v - mean).collect();
        let zero: f32 = env.iter().map(|v| v * v).sum();
        if zero <= 1e-12 {
            self.confidence = 0.0;
            return;
        }
        let lag_lo = (60.0 * ENV_RATE / BPM_RANGE.1) as usize;
        let lag_hi = ((60.0 * ENV_RATE / BPM_RANGE.0) as usize).min(n / 2);
        let acf = |lag: usize| env[lag..].iter().zip(&env).map(|(a, b)| a * b).sum::<f32>() / zero;
        let mut best = (0usize, f32::MIN, 0.0f32);
        for lag in lag_lo..=lag_hi {
            let r = acf(lag);
            // Prefer tempos around 120 BPM, a log-normal prior one octave wide.
            let bpm = 60.0 * ENV_RATE / lag as f32;
            let octaves = (bpm / 120.0).log2();
            let score = r * (-0.5 * octaves * octaves).exp();
            if score > best.1 {
                best = (lag, score, r);
            }
        }
        let (mut lag, _, mut r) = best;
        if lag == 0 {
            return;
        }
        // Patterns like kick-snare repeat every two beats, which can beat the
        // real period. If half the lag correlates nearly as well, take it.
        let half = lag / 2;
        if half >= lag_lo && acf(half) > 0.5 * r {
            lag = half;
            r = acf(half);
        }
        // Parabolic interpolation around the peak for sub-sample precision.
        let (a, b, c) = (acf(lag - 1), r, acf((lag + 1).min(n - 1)));
        let denom = a - 2.0 * b + c;
        let shift = if denom.abs() > 1e-9 {
            (0.5 * (a - c) / denom).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let mut period = lag as f32 + shift;
        // Breaks can make half or double the tempo win for a moment. Fold
        // those back onto the tempo we already have.
        if self.period > 0.0 {
            for k in [0.5, 2.0] {
                if (period / (self.period * k) - 1.0).abs() < 0.06 {
                    period /= k;
                }
            }
        }
        self.confidence = ease(self.confidence, r, 1.0, 0.5);
        // A new tempo has to show up twice in a row before it replaces the old one.
        if self.period > 0.0 && (period / self.period - 1.0).abs() < 0.06 {
            self.period += (period - self.period) * 0.3;
        } else if self.candidate > 0.0 && (period / self.candidate - 1.0).abs() < 0.06 {
            self.period = period;
            self.next_beat = f32::INFINITY;
        } else {
            self.candidate = period;
            return;
        }
        self.candidate = 0.0;
        self.align_phase(t);
    }

    /// Nudges the predicted next beat towards the phase where onsets line up
    /// best with the beat grid.
    fn align_phase(&mut self, t: f32) {
        let p = self.period;
        let n = self.env.len();
        if p < 2.0 || (n as f32) < p * 4.0 {
            return;
        }
        let mut best = (0.0f32, f32::MIN);
        let steps = p.ceil() as usize;
        for phase in 0..steps {
            let score: f32 = (0..4)
                .filter_map(|k| {
                    let back = phase as f32 + k as f32 * p;
                    let i = n.checked_sub(1 + back.round() as usize)?;
                    Some(self.env[i])
                })
                .sum();
            if score > best.1 {
                best = (phase as f32, score);
            }
        }
        // The last onset peak was `phase` samples ago, so the next beat is due
        // one period after it.
        let period_s = p / ENV_RATE;
        let predicted = t - best.0 / ENV_RATE + period_s;
        if !self.next_beat.is_finite() {
            self.next_beat = predicted;
            return;
        }
        let mut diff = (predicted - self.next_beat) % period_s;
        if diff > period_s * 0.5 {
            diff -= period_s;
        } else if diff < -period_s * 0.5 {
            diff += period_s;
        }
        self.next_beat += diff * 0.3;
    }
}

/// Tracks the energy of the track over seconds to find breakdowns and drops.
struct DropTracker {
    fast: f32,
    slow: f32,
    /// How long the current breakdown has lasted, 0 outside of one.
    quiet_for: f32,
}

impl DropTracker {
    /// Returns (in full swing, drop, breakdown started).
    fn step(&mut self, level: f32, dt: f32) -> (bool, bool, bool) {
        if self.slow == 0.0 {
            // Start from the current level instead of fading in from silence.
            (self.fast, self.slow) = (level, level);
        }
        self.fast = ease(self.fast, level, 0.4, dt);
        self.slow = ease(self.slow, level, 6.0, dt);
        let low = self.fast < self.slow * 0.75;
        let back = self.fast > self.slow * 0.9;
        let was = self.quiet_for;
        let mut drop = false;
        if low {
            self.quiet_for += dt;
        } else if back {
            drop = self.quiet_for >= 1.0;
            self.quiet_for = 0.0;
        }
        let started = was < 1.0 && self.quiet_for >= 1.0;
        (self.quiet_for < 0.5, drop, started)
    }
}

/// Per-frame input for the detectors.
pub struct Input<'a> {
    /// Linear FFT magnitudes.
    pub mags: &'a [f32],
    pub hz_per_bin: f32,
    /// Raw RMS level, before auto-gain.
    pub level: f32,
    /// Result of the original bass-energy beat detector.
    pub kick: bool,
    pub t: f32,
    pub dt: f32,
}

pub struct Detectors {
    prev: Vec<f32>,
    pitch_class: Vec<Option<u8>>,
    onset: Picker,
    snare: Picker,
    hihat: Picker,
    harmony: Picker,
    loudness: Picker,
    section: Picker,
    tempo: TempoTracker,
    drop: DropTracker,
    chroma_short: [f32; 12],
    chroma_long: [f32; 12],
    profile_short: Vec<f32>,
    profile_long: Vec<f32>,
    last_section: f32,
    last_drop: f32,
    /// This frame's beat for every detector, in `Detector::ALL` order.
    pub beats: [bool; 9],
    /// True on the frame a drop hits; it gets an extra strong pulse.
    pub drop_hit: bool,
    /// True on the frame the music seems to move to a new section.
    pub section_change: bool,
}

/// Log compression for spectral flux, so quiet partials count too.
fn compress(m: f32) -> f32 {
    (1.0 + 1000.0 * m).ln()
}

impl Detectors {
    pub fn new(bins: usize, hz_per_bin: f32) -> Self {
        // Pitch class per FFT bin, for the chroma vector (110 Hz to 4 kHz).
        let pitch_class = (0..bins)
            .map(|k| {
                let f = k as f32 * hz_per_bin;
                (110.0..4000.0)
                    .contains(&f)
                    .then(|| ((12.0 * (f / 440.0).log2() + 69.0).round() as i32).rem_euclid(12) as u8)
            })
            .collect();
        Self {
            prev: vec![0.0; bins],
            pitch_class,
            onset: Picker::new(1.5, 1.0, 0.1, 0.02),
            snare: Picker::new(1.6, 1.0, 0.18, 0.02),
            hihat: Picker::new(1.1, 0.8, 0.07, 0.02),
            harmony: Picker::new(2.0, 2.0, 0.5, 0.04),
            loudness: Picker::new(1.8, 0.6, 0.25, 0.002),
            section: Picker::new(3.0, 6.0, 8.0, 0.03),
            tempo: TempoTracker::new(),
            drop: DropTracker {
                fast: 0.0,
                slow: 0.0,
                quiet_for: 0.0,
            },
            chroma_short: [0.0; 12],
            chroma_long: [0.0; 12],
            profile_short: vec![0.0; 32],
            profile_long: vec![0.0; 32],
            last_section: 0.0,
            last_drop: f32::NEG_INFINITY,
            beats: [false; 9],
            drop_hit: false,
            section_change: false,
        }
    }

    pub fn bpm(&self) -> Option<f32> {
        self.tempo.bpm()
    }

    /// True for a moment after a drop.
    pub fn drop_glow(&self, t: f32) -> bool {
        t - self.last_drop < 0.8
    }

    pub fn update(&mut self, input: &Input) {
        let Input {
            mags,
            hz_per_bin,
            level,
            kick,
            t,
            dt,
        } = *input;
        let bin = |hz: f32| ((hz / hz_per_bin) as usize).clamp(1, mags.len());

        // Spectral flux per band: the summed increase of log magnitudes,
        // averaged over the band so the bands are comparable.
        let flux = |lo: f32, hi: f32, prev: &[f32]| {
            let (k0, k1) = (bin(lo), bin(hi).max(bin(lo) + 1).min(mags.len()));
            let sum: f32 = (k0..k1).map(|k| (compress(mags[k]) - prev[k]).max(0.0)).sum();
            sum / (k1 - k0).max(1) as f32
        };
        let low = flux(30.0, 150.0, &self.prev);
        let mid = flux(150.0, 800.0, &self.prev);
        let snare = flux(800.0, 5000.0, &self.prev);
        let hihat = flux(6000.0, 14_000.0, &self.prev);
        // Most FFT bins are high frequencies, so the bands are weighted
        // equally instead of averaging over all bins.
        let full = (low + mid + snare + hihat) * 0.25;
        for (p, &m) in self.prev.iter_mut().zip(mags) {
            *p = compress(m);
        }

        let (swing, drop, breakdown) = self.drop.step(level, dt);

        // Chroma: energy per pitch class, normalized.
        let mut chroma = [0.0f32; 12];
        for (m, pc) in mags.iter().zip(&self.pitch_class) {
            if let Some(pc) = pc {
                chroma[*pc as usize] += m * m;
            }
        }
        let norm = chroma.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-12);
        for (i, c) in chroma.iter().enumerate() {
            self.chroma_short[i] = ease(self.chroma_short[i], c / norm, 0.12, dt);
            self.chroma_long[i] = ease(self.chroma_long[i], c / norm, 1.5, dt);
        }
        let harmony_novelty = 1.0 - cosine(&self.chroma_short, &self.chroma_long);

        // Section changes: the spectral profile of the last few seconds drifts
        // away from that of the last half minute.
        let bands = self.profile_short.len();
        for b in 0..bands {
            let k0 = 1 + b * (mags.len() - 1) / bands;
            let k1 = 1 + (b + 1) * (mags.len() - 1) / bands;
            let e = mags[k0..k1.max(k0 + 1).min(mags.len())].iter().sum::<f32>();
            let e = compress(e);
            if self.profile_long[b] == 0.0 {
                (self.profile_short[b], self.profile_long[b]) = (e, e);
            }
            self.profile_short[b] = ease(self.profile_short[b], e, 2.0, dt);
            self.profile_long[b] = ease(self.profile_long[b], e, 12.0, dt);
        }
        let timbre_novelty = 1.0 - cosine(&self.profile_short, &self.profile_long);
        let timbre = self.section.step(timbre_novelty, t, dt);

        let onset = self.onset.step(full, t, dt);
        // The click of a kick drum spills into the snare band, so frames
        // with a kick need a much stronger hit to count.
        let snare_hit = self.snare.step_with(snare, if kick { 2.5 } else { 1.0 }, t, dt);
        let hihat_hit = self.hihat.step(hihat, t, dt);
        let harmony = self.harmony.step(harmony_novelty, t, dt);
        // Bars tend to start with a kick and a chord change, rarely a snare.
        // The chroma is smoothed, so chord changes show up about 0.2 s late.
        let accents = [
            (kick as u8 as f32 * 0.5, 0.0),
            (harmony as u8 as f32 * 2.0, 0.2),
            (-(snare_hit as u8 as f32), 0.0),
        ];
        let (tempo, downbeat) = self.tempo.step(full, &accents, t, dt);

        self.beats = [
            kick,
            onset,
            snare_hit,
            hihat_hit,
            tempo,
            tempo && downbeat,
            (kick && swing) || drop,
            harmony,
            self.loudness.step(level, t, dt),
        ];
        self.drop_hit = drop;
        if drop {
            self.last_drop = t;
        }
        // Section events are spaced at least 8 seconds apart.
        self.section_change = (drop || breakdown || timbre) && t - self.last_section > 8.0;
        if self.section_change {
            self.last_section = t;
        }
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < 1e-9 || nb < 1e-9 { 1.0 } else { dot / (na * nb) }
}

#[cfg(test)]
mod tests {
    use super::Detector;
    use crate::analysis::{Analyzer, FFT_SIZE};
    use crate::audio::DemoSynth;

    /// Runs the demo track through the analyzer at 30 fps and counts the
    /// beats of every detector over `secs` seconds.
    fn count(secs: f32) -> ([u32; 9], u32, Option<f32>) {
        let sr = 44_100.0;
        let mut synth = DemoSynth::new(sr);
        let mut analyzer = Analyzer::new(sr);
        let mut samples = vec![0.0f32; FFT_SIZE];
        let dt = 1.0 / 30.0;
        let mut counts = [0u32; 9];
        let mut sections = 0;
        let frames = (secs / dt) as usize;
        for frame in 0..frames {
            let n = (sr * dt) as usize;
            samples.drain(..n);
            samples.extend((0..n).map(|_| synth.next()));
            analyzer.update(&samples, frame as f32 * dt, dt);
            // Skip the warm-up, the tempo tracker needs a few seconds.
            if frame as f32 * dt < 10.0 {
                continue;
            }
            for (c, &b) in counts.iter_mut().zip(&analyzer.detector_beats()) {
                *c += b as u32;
            }
            sections += analyzer.section_change() as u32;
        }
        (counts, sections, analyzer.bpm())
    }

    /// The demo track runs at 124 BPM with a kick on every beat, a snare on
    /// 2 and 4, hi-hats on eighths, a new bass note every bar and a
    /// breakdown every 8 bars. One minute of it is measured.
    #[test]
    fn detectors_follow_the_demo_track() {
        let (counts, sections, bpm) = count(70.0);
        let rate = |d: Detector| counts[Detector::ALL.iter().position(|&x| x == d).unwrap()];
        for (d, c) in Detector::ALL.iter().zip(counts) {
            println!("{:<9} {c:4}/min", d.name());
        }
        let bpm = bpm.expect("tempo found");
        assert!((120.0..128.0).contains(&bpm), "bpm {bpm}");
        assert!((95..=125).contains(&rate(Detector::Kick)));
        assert!(
            (115..=130).contains(&rate(Detector::Tempo)),
            "tempo keeps going through breaks"
        );
        assert!((27..=35).contains(&rate(Detector::Bar)));
        assert!((40..=75).contains(&rate(Detector::Snare)));
        assert!(rate(Detector::HiHat) > 120);
        assert!((24..=40).contains(&rate(Detector::Harmony)));
        assert!(rate(Detector::Onset) > 60);
        assert!(rate(Detector::Drop) > 60);
        assert!(rate(Detector::Loudness) > 30);
        assert!((1..=7).contains(&sections), "sections {sections}");
    }
}
