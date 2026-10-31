//! Audio sources: a cpal input device, a macOS process tap or a built-in demo
//! synth. All push mono samples into a shared ring buffer that the main loop
//! snapshots each frame.

use anyhow::{Context, Result, anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::any::Any;
use std::collections::VecDeque;
use std::f32::consts::TAU;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// ~2.7 s at 48 kHz: room for `--delay` plus one FFT window.
const BUFFER_LEN: usize = 1 << 17;

pub(crate) type Shared = Arc<Mutex<VecDeque<f32>>>;

fn shared() -> Shared {
    Arc::new(Mutex::new(VecDeque::with_capacity(BUFFER_LEN)))
}

pub struct AudioSource {
    buf: Shared,
    pub sample_rate: u32,
    pub label: String,
    // Dropping the stream/tap stops capture, so it lives as long as the source.
    _keep: Box<dyn Any>,
}

pub(crate) fn push(buf: &Shared, samples: impl Iterator<Item = f32>) {
    let mut b = buf.lock().unwrap();
    b.extend(samples);
    let excess = b.len().saturating_sub(BUFFER_LEN);
    b.drain(..excess);
}

pub(crate) fn mono<T: Copy>(data: &[T], channels: usize, conv: impl Fn(T) -> f32) -> impl Iterator<Item = f32> {
    data.chunks(channels)
        .map(move |frame| frame.iter().map(|&s| conv(s)).sum::<f32>() / channels as f32)
}

impl AudioSource {
    /// Opens the default input device, or the first one whose name contains
    /// `name` (case-insensitive).
    pub fn device(name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();
        let device = match name {
            Some(n) => find(host.input_devices()?, n).ok_or_else(|| anyhow!("no input device matching '{n}'"))?,
            None => host.default_input_device().context("no default input device")?,
        };
        let config = device.default_input_config()?;
        let label = device.name().unwrap_or_else(|_| "unknown device".into());
        Self::open(device, config, label)
    }

    /// Records everything that plays on an output device (WASAPI loopback):
    /// the default output, or the first one whose name contains `name`.
    #[cfg(windows)]
    pub fn loopback(name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();
        let device = match name {
            Some(n) => find(host.output_devices()?, n).ok_or_else(|| anyhow!("no output device matching '{n}'"))?,
            None => host.default_output_device().context("no default output device")?,
        };
        // cpal switches to loopback mode when an output device is opened for input.
        let config = device.default_output_config()?;
        let label = format!(
            "loopback: {}",
            device.name().unwrap_or_else(|_| "unknown device".into())
        );
        Self::open(device, config, label)
    }

    #[cfg(not(windows))]
    pub fn loopback(_name: Option<&str>) -> Result<Self> {
        bail!("--loopback is only supported on Windows (on macOS, use --tap)")
    }

    fn open(device: cpal::Device, supported: cpal::SupportedStreamConfig, label: String) -> Result<Self> {
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let channels = config.channels as usize;

        let buf = shared();
        let b = buf.clone();
        // Nowhere to report stream errors while the TUI owns the screen.
        let err = |_: cpal::StreamError| {};
        let stream = match format {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config,
                move |d: &[f32], _: &cpal::InputCallbackInfo| push(&b, mono(d, channels, |s| s)),
                err,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config,
                move |d: &[i16], _: &cpal::InputCallbackInfo| push(&b, mono(d, channels, |s| s as f32 / 32768.0)),
                err,
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config,
                move |d: &[u16], _: &cpal::InputCallbackInfo| {
                    push(&b, mono(d, channels, |s| (s as f32 - 32768.0) / 32768.0))
                },
                err,
                None,
            )?,
            other => bail!("unsupported sample format {other:?}"),
        };
        stream.play()?;

        Ok(Self {
            buf,
            sample_rate: config.sample_rate.0,
            label,
            _keep: Box::new(stream),
        })
    }

    /// Synthetic 124 BPM track generated in real time on a background thread.
    pub fn demo() -> Self {
        let sample_rate = 44_100;
        let buf = shared();
        let b = buf.clone();
        thread::spawn(move || {
            let mut synth = DemoSynth::new(sample_rate as f32);
            let start = Instant::now();
            let mut produced: u64 = 0;
            loop {
                let target = (start.elapsed().as_secs_f64() * sample_rate as f64) as u64;
                push(&b, (produced..target).map(|_| synth.next()));
                produced = target;
                thread::sleep(Duration::from_millis(5));
            }
        });
        Self {
            buf,
            sample_rate,
            label: "demo synth".into(),
            _keep: Box::new(()),
        }
    }

    /// Captures the output of apps whose bundle ID contains `app` (macOS only).
    #[cfg(target_os = "macos")]
    pub fn tap(app: &str) -> Result<Self> {
        let buf = shared();
        let tap = crate::tap::ProcessTap::start(app, buf.clone())?;
        Ok(Self {
            buf,
            sample_rate: tap.sample_rate,
            label: tap.label.clone(),
            _keep: Box::new(tap),
        })
    }

    #[cfg(not(target_os = "macos"))]
    pub fn tap(_app: &str) -> Result<Self> {
        bail!("--tap is only supported on macOS")
    }

    /// Copies `n` samples (or fewer at startup) into `out`, ending `delay`
    /// samples before the newest one.
    pub fn snapshot(&self, n: usize, delay: usize, out: &mut Vec<f32>) {
        let b = self.buf.lock().unwrap();
        let end = b.len().saturating_sub(delay);
        out.clear();
        out.extend(b.range(end.saturating_sub(n)..end));
    }
}

/// First device whose name contains `name` (case-insensitive).
fn find(mut devices: impl Iterator<Item = cpal::Device>, name: &str) -> Option<cpal::Device> {
    let needle = name.to_lowercase();
    devices.find(|d| d.name().is_ok_and(|s| s.to_lowercase().contains(&needle)))
}

/// Device names, each with a flag for the system default.
fn names(devices: impl Iterator<Item = cpal::Device>, default: Option<cpal::Device>) -> Vec<(String, bool)> {
    let default = default.and_then(|d| d.name().ok());
    devices
        .map(|d| {
            let name = d.name().unwrap_or_else(|_| "unknown device".into());
            let is_default = default.as_deref() == Some(name.as_str());
            (name, is_default)
        })
        .collect()
}

pub fn input_devices() -> Result<Vec<(String, bool)>> {
    let host = cpal::default_host();
    Ok(names(host.input_devices()?, host.default_input_device()))
}

/// Output devices, usable with `--loopback`.
#[cfg(windows)]
pub fn output_devices() -> Result<Vec<(String, bool)>> {
    let host = cpal::default_host();
    Ok(names(host.output_devices()?, host.default_output_device()))
}

pub fn list_devices() -> Result<()> {
    println!("Input devices (* = default):");
    for (name, is_default) in input_devices()? {
        println!("{} {name}", if is_default { '*' } else { ' ' });
    }
    #[cfg(windows)]
    {
        println!("\nOutput devices for --loopback (* = default):");
        for (name, is_default) in output_devices()? {
            println!("{} {name}", if is_default { '*' } else { ' ' });
        }
    }
    Ok(())
}

pub(crate) struct DemoSynth {
    sr: f32,
    n: u64,
    rng: crate::gfx::Rng,
    last_noise: f32,
    kick_phase: f32,
    bass_phase: f32,
    lead_phase: f32,
}

impl DemoSynth {
    pub(crate) fn new(sr: f32) -> Self {
        Self {
            sr,
            n: 0,
            rng: crate::gfx::Rng::new(0x5eed),
            last_noise: 0.0,
            kick_phase: 0.0,
            bass_phase: 0.0,
            lead_phase: 0.0,
        }
    }

    pub(crate) fn next(&mut self) -> f32 {
        const BPM: f64 = 124.0;
        let beat_len = 60.0 / BPM;
        let t = self.n as f64 / self.sr as f64;
        self.n += 1;

        let pos = t / beat_len;
        let beat = pos.floor() as u64;
        let bar = beat / 4;
        let since_beat = (pos.fract() * beat_len) as f32;
        let since_8th = ((pos * 2.0).fract() * beat_len / 2.0) as f32;
        let since_16th = ((pos * 4.0).fract() * beat_len / 4.0) as f32;
        // Every 8th bar is a breakdown without drums.
        let drums = bar % 8 != 7;

        let kick_freq = 50.0 + 110.0 * (-since_beat * 25.0).exp();
        self.kick_phase = (self.kick_phase + kick_freq / self.sr).fract();
        let kick = (self.kick_phase * TAU).sin() * (-since_beat * 7.0).exp() * 0.9;

        let noise = self.rng.range(-1.0, 1.0);
        let snare = if beat % 2 == 1 {
            noise * (-since_beat * 18.0).exp() * 0.35
        } else {
            0.0
        };
        let hat = (noise - self.last_noise) * 0.5 * (-since_8th * 60.0).exp() * 0.25;
        self.last_noise = noise;

        let root = [55.0, 55.0, 73.42, 65.41][(bar % 4) as usize];
        self.bass_phase = (self.bass_phase + root / self.sr).fract();
        let pump = if drums {
            1.0 - 0.8 * (-since_beat * 8.0).exp()
        } else {
            1.0
        };
        let bass = (self.bass_phase * 2.0 - 1.0) * 0.18 * pump;

        let notes = [440.0, 523.25, 659.25, 783.99];
        let step = (pos * 4.0).floor() as usize;
        let lift = if bar % 2 == 1 { 1.5 } else { 1.0 };
        self.lead_phase = (self.lead_phase + notes[step % 4] * lift / self.sr).fract();
        let square = if self.lead_phase < 0.5 { 1.0 } else { -1.0 };
        let lead = square * (-since_16th * 12.0).exp() * 0.08;

        let drums_mix = if drums { kick + snare + hat } else { 0.0 };
        (drums_mix + bass + lead).clamp(-1.0, 1.0)
    }
}
