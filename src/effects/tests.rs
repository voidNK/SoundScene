//! Robustness and speed checks for every effect in `all()`.

use super::all;
use crate::analysis::{Features, SPECTRUM_BINS, WAVEFORM_LEN};
use crate::gfx::{Canvas, Rng};
use std::time::Instant;

fn features(i: usize) -> Features {
    let k = (i % 7) as f32 / 6.0;
    Features {
        level: k,
        bass: 1.0 - k,
        mid: k * 0.5,
        treble: k,
        beat: i.is_multiple_of(5),
        pulse: if i.is_multiple_of(5) { 1.0 } else { 0.3 },
        spectrum: (0..SPECTRUM_BINS).map(|b| ((b + i) % 11) as f32 / 10.0).collect(),
        waveform: (0..WAVEFORM_LEN).map(|s| ((s + i) as f32 * 0.1).sin()).collect(),
    }
}

/// Mosaic tiles get tiny and windows resize at any time, so every effect
/// must handle any canvas size, including zero, without panicking.
#[test]
fn every_effect_survives_any_size() {
    let sizes = [
        (0, 0),
        (1, 2),
        (2, 2),
        (3, 4),
        (1, 40),
        (40, 1),
        (5, 6),
        (8, 8),
        (17, 9),
        (40, 30),
        (160, 90),
    ];
    let mut rng = Rng::new(7);
    for mut e in all() {
        let mut c = Canvas::new();
        let mut t = 0.0;
        for (i, &(w, h)) in sizes.iter().enumerate() {
            c.resize(w, h);
            for j in 0..4 {
                t += 1.0 / 30.0;
                e.render(&mut c, t, 1.0 / 30.0, &features(i * 4 + j));
            }
        }
        for i in 0..60 {
            c.resize((rng.f32() * 90.0) as usize, (rng.f32() * 60.0) as usize);
            t += 1.0 / 30.0;
            e.render(&mut c, t, 1.0 / 30.0, &features(i));
        }
        // A long frame and a zero-length frame must not break the state either.
        c.resize(30, 20);
        e.render(&mut c, t + 5.0, 0.1, &features(0));
        e.render(&mut c, t + 5.0, 0.0, &features(1));
    }
}

/// `cargo test --release -- --ignored --nocapture speed` prints the cost of
/// one frame on a large terminal (240×60 cells).
#[test]
#[ignore]
fn speed() {
    let mut c = Canvas::new();
    c.resize(240, 120);
    for mut e in all() {
        for i in 0..5 {
            e.render(&mut c, i as f32 / 30.0, 1.0 / 30.0, &features(i));
        }
        let start = Instant::now();
        for i in 0..30 {
            e.render(&mut c, 1.0 + i as f32 / 30.0, 1.0 / 30.0, &features(i));
        }
        println!(
            "{:<20} {:6.2} ms/frame",
            e.name(),
            start.elapsed().as_secs_f64() * 1000.0 / 30.0
        );
    }
}
