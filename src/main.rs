mod analysis;
mod audio;
mod effects;
mod gfx;
mod setup;
#[cfg(target_os = "macos")]
mod tap;
mod term;

use analysis::{Analyzer, FFT_SIZE, Features};
use anyhow::{Context, Result};
use audio::AudioSource;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, terminal};
use gfx::Canvas;
use std::io::{self, Write};
use std::time::{Duration, Instant};

/// Sound-reactive demo effects in the terminal.
/// Run without arguments for an interactive setup.
/// Keys: ←/→ switch effect, m mosaic, c rotate colors, h toggle HUD, q/Esc quit.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Use a built-in synthetic track instead of an audio input device
    #[arg(long)]
    demo: bool,

    /// Input device (case-insensitive substring), e.g. "BlackHole".
    /// With --loopback, the output device to record instead
    #[arg(short, long, conflicts_with_all = ["demo", "tap"])]
    device: Option<String>,

    /// Capture an app's audio output directly (macOS 14.4+), matched by
    /// bundle ID substring, e.g. "chrome". See --list-apps
    #[arg(short, long, value_name = "APP", conflicts_with = "demo")]
    tap: Option<String>,

    /// Record everything that plays on an output device (Windows only).
    /// Uses the default output, or the one picked with --device
    #[arg(long, conflicts_with_all = ["demo", "tap"])]
    loopback: bool,

    /// List input devices (and on Windows, output devices) and exit
    #[arg(long)]
    list_devices: bool,

    /// List apps known to the audio system (for --tap) and exit
    #[arg(long)]
    list_apps: bool,

    /// Delay the visuals by this many milliseconds, e.g. ~200 to match
    /// Bluetooth headphone latency (max 2000)
    #[arg(long, value_name = "MS", default_value_t = 0, value_parser = clap::value_parser!(u64).range(0..=2000))]
    delay: u64,

    /// Seconds before auto-rotating to the next effect
    #[arg(long, value_name = "SECONDS", default_value_t = 60)]
    interval: u64,

    /// Target frames per second
    #[arg(long, default_value_t = 30)]
    fps: u32,

    /// Show several effects at once in an N×N grid (1 = single effect)
    #[arg(long, value_name = "N", default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=4))]
    mosaic: u8,

    /// Print raw and normalized audio levels for N seconds instead of
    /// starting the TUI (for checking that the input carries signal)
    #[arg(long, value_name = "SECONDS")]
    meter: Option<u64>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if std::env::args_os().len() == 1 {
        let _guard = TerminalGuard::enter()?;
        let Some((source, args)) = setup::run(args)? else {
            return Ok(());
        };
        let delay = (args.delay * source.sample_rate as u64 / 1000) as usize;
        return run(&args, &source, delay);
    }
    if args.list_devices {
        return audio::list_devices();
    }
    if args.list_apps {
        #[cfg(target_os = "macos")]
        return tap::list_apps();
        #[cfg(not(target_os = "macos"))]
        anyhow::bail!("--list-apps is only supported on macOS");
    }
    let source = if args.demo {
        AudioSource::demo()
    } else if let Some(app) = &args.tap {
        AudioSource::tap(app).context("could not start process tap")?
    } else if args.loopback {
        AudioSource::loopback(args.device.as_deref()).context("could not start loopback capture")?
    } else {
        AudioSource::device(args.device.as_deref())
            .context("could not open audio input (try --list-devices or --demo)")?
    };

    let delay = (args.delay * source.sample_rate as u64 / 1000) as usize;
    if let Some(secs) = args.meter {
        return meter(&source, secs, delay);
    }

    let _guard = TerminalGuard::enter()?;
    run(&args, &source, delay)
}

fn meter(source: &AudioSource, secs: u64, delay: usize) -> Result<()> {
    println!("{} @ {} Hz", source.label, source.sample_rate);
    let mut analyzer = Analyzer::new(source.sample_rate as f32);
    let mut samples = Vec::with_capacity(FFT_SIZE);
    let start = Instant::now();
    let mut last = start;
    let mut next_print = start;
    while start.elapsed() < Duration::from_secs(secs) {
        std::thread::sleep(Duration::from_millis(20));
        let now = Instant::now();
        source.snapshot(FFT_SIZE, delay, &mut samples);
        analyzer.update(
            &samples,
            now.duration_since(start).as_secs_f32(),
            now.duration_since(last).as_secs_f32(),
        );
        last = now;
        let f = &analyzer.features;
        if f.beat || now >= next_print {
            let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            println!(
                "{:6.2}s  peak {peak:.4}  level {:.2}  bass {:.2}  mid {:.2}  treb {:.2}{}",
                now.duration_since(start).as_secs_f32(),
                f.level,
                f.bass,
                f.mid,
                f.treble,
                if f.beat { "  BEAT" } else { "" },
            );
            if now >= next_print {
                next_print = now + Duration::from_millis(500);
            }
        }
    }
    Ok(())
}

fn run(args: &Args, source: &AudioSource, delay: usize) -> Result<()> {
    let mut effects = effects::all();
    let mut current = 0;
    let mut analyzer = Analyzer::new(source.sample_rate as f32);
    let mut canvas = Canvas::new();
    let mut samples = Vec::with_capacity(FFT_SIZE);
    let mut frame = Vec::new();
    let mut show_hud = true;
    let mut clear_frames = 0u8;
    // Grid size; 1 = a single effect. Each mosaic tile keeps its own canvas
    // so effects with trails don't smear into each other.
    let mut mosaic = args.mosaic as usize;
    let mut tiles: Vec<Canvas> = Vec::new();
    // Hue shift in 60° steps. It is applied to a copy because effects with
    // trails read their previous frame back from `canvas`.
    let mut hue_step = 0u8;
    let mut shifted = Canvas::new();

    let interval = Duration::from_secs(args.interval.max(1));
    let frame_time = Duration::from_secs_f64(1.0 / args.fps.clamp(5, 240) as f64);
    let start = Instant::now();
    let mut last = start;
    let mut switched = start;
    let mut stdout = io::stdout().lock();

    loop {
        let mut step = 0i32;
        while event::poll(Duration::ZERO)? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Release {
                    continue;
                }
                match k.code {
                    KeyCode::Right => step += 1,
                    KeyCode::Left => step -= 1,
                    KeyCode::Char('h') => show_hud = !show_hud,
                    KeyCode::Char('m') => {
                        mosaic = if mosaic >= 3 { 1 } else { mosaic + 1 };
                        tiles.clear();
                        canvas.clear();
                    }
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Char('c') => hue_step = (hue_step + 1) % 6,
                    _ => {}
                }
            }
        }

        let now = Instant::now();
        if step == 0 && now.duration_since(switched) >= interval {
            step = 1;
        }
        let page = mosaic * mosaic;
        if step != 0 {
            current = (current as i32 + step * page as i32).rem_euclid(effects.len() as i32) as usize;
            switched = now;
            canvas.clear();
            tiles.iter_mut().for_each(Canvas::clear);
        }

        let dt = now.duration_since(last).as_secs_f32().min(0.1);
        last = now;
        let t = now.duration_since(start).as_secs_f32();

        source.snapshot(FFT_SIZE, delay, &mut samples);
        analyzer.update(&samples, t, dt);

        let (cols, rows) = terminal::size()?;
        let draw_rows = rows.saturating_sub(show_hud as u16).max(1);
        // Frames at the old size may still be in flight when the terminal
        // resizes, so keep clearing for a few frames to wipe their leftovers.
        if canvas.resize(cols as usize, draw_rows as usize * 2) {
            clear_frames = 3;
        }
        let clear = clear_frames > 0;
        clear_frames = clear_frames.saturating_sub(1);
        if mosaic == 1 {
            effects[current].render(&mut canvas, t, dt, &analyzer.features);
        } else {
            render_mosaic(
                &mut canvas,
                &mut tiles,
                mosaic,
                &mut effects,
                current,
                t,
                dt,
                &analyzer.features,
            );
        }

        let hud = show_hud.then(|| {
            let total = effects.len();
            let (title, names) = if mosaic == 1 {
                (
                    format!("{}/{} {}", current + 1, total, effects[current].name()),
                    String::new(),
                )
            } else {
                let names: Vec<&str> = (0..page).map(|i| effects[(current + i) % total].name()).collect();
                let last = (current + page - 1) % total + 1;
                (
                    format!("{}–{last}/{total} {mosaic}×{mosaic}", current + 1),
                    names.join(" · "),
                )
            };
            let info = HudInfo {
                title,
                names,
                remaining: interval.saturating_sub(now.duration_since(switched)).as_secs(),
                source: &source.label,
            };
            hud_line(cols as usize, &info, &analyzer.features)
        });
        let shown = if hue_step == 0 {
            &canvas
        } else {
            gfx::hue_rotate(&mut shifted, &canvas, hue_step as f32 * 60.0);
            &shifted
        };
        term::compose(&mut frame, shown, clear, hud.as_deref().map(|h| (h, draw_rows + 1)));
        stdout.write_all(&frame)?;
        stdout.flush()?;

        if let Some(rest) = frame_time.checked_sub(now.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

/// Splits the canvas into an n×n grid and renders consecutive effects
/// (starting at `first`) into the tiles, with a 1-pixel gap between them.
#[allow(clippy::too_many_arguments)]
fn render_mosaic(
    canvas: &mut Canvas,
    tiles: &mut Vec<Canvas>,
    n: usize,
    effects: &mut [Box<dyn effects::Effect>],
    first: usize,
    t: f32,
    dt: f32,
    f: &Features,
) {
    tiles.resize_with(n * n, Canvas::new);
    canvas.clear();
    // Tile rows are aligned to whole terminal cells (2 pixels).
    let cell_rows = canvas.h / 2;
    for row in 0..n {
        for col in 0..n {
            let x0 = col * canvas.w / n;
            let x1 = (col + 1) * canvas.w / n;
            let y0 = row * cell_rows / n * 2;
            let y1 = (row + 1) * cell_rows / n * 2;
            let w = (x1 - x0).saturating_sub((col + 1 < n) as usize);
            let h = (y1 - y0).saturating_sub((row + 1 < n) as usize);
            let tile = &mut tiles[row * n + col];
            tile.resize(w, h);
            if w < 2 || h < 2 {
                continue;
            }
            let index = (first + row * n + col) % effects.len();
            effects[index].render(tile, t, dt, f);
            canvas.blit(tile, x0, y0);
        }
    }
}

struct HudInfo<'a> {
    title: String,
    /// Effect names in mosaic mode, empty otherwise.
    names: String,
    remaining: u64,
    source: &'a str,
}

fn hud_line(width: usize, info: &HudInfo, f: &Features) -> String {
    let meter = |v: f32| {
        let n = (v.clamp(0.0, 1.0) * 5.0).round() as usize;
        format!("{}{}", "█".repeat(n), "·".repeat(5 - n))
    };
    let mut text = format!(
        " ←/→ {:<18} │ next {:>2}s │ bass {} mid {} treb {} {} │ {} │ m mosaic · c color · h hud · q quit",
        info.title,
        info.remaining,
        meter(f.bass),
        meter(f.mid),
        meter(f.treble),
        if f.pulse > 0.5 { '●' } else { '○' },
        info.source,
    );
    if !info.names.is_empty() {
        text.push_str(" │ ");
        text.push_str(&info.names);
    }
    let mut line: String = text.chars().take(width).collect();
    let pad = width.saturating_sub(line.chars().count());
    line.extend(std::iter::repeat_n(' ', pad));
    format!("\x1b[48;2;16;16;28m\x1b[38;2;190;190;220m{line}")
}

/// Raw mode + alternate screen for the lifetime of the guard, restored on
/// drop and on panic.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        // Autowrap off: overlong lines during a resize get clipped instead of
        // wrapping and scrolling the screen.
        execute!(
            io::stdout(),
            terminal::EnterAlternateScreen,
            cursor::Hide,
            terminal::DisableLineWrap
        )?;
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            default_hook(info);
        }));
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let _ = execute!(
        io::stdout(),
        terminal::EnableLineWrap,
        cursor::Show,
        terminal::LeaveAlternateScreen
    );
    let _ = terminal::disable_raw_mode();
}
