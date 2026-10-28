//! Interactive setup screen, shown when the app is started without arguments.
//! Lets the user pick an audio source (with a live level meter, so it's
//! obvious whether anything arrives) and tweak layout, delay, rotation and fps.

use crate::Args;
use crate::analysis::{Analyzer, FFT_SIZE, Features};
use crate::audio::{self, AudioSource};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::fmt::Write as _;
use std::io::{self, Write};
use std::time::{Duration, Instant};

enum Choice {
    // Only discovered on macOS, where process taps exist.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    App {
        bundle_id: String,
        playing: bool,
    },
    Device {
        name: String,
        default: bool,
    },
    Demo,
}

impl Choice {
    fn label(&self) -> String {
        match self {
            Choice::App { bundle_id, playing } => format!(
                "App     {:<12} {bundle_id}{}",
                short_name(bundle_id),
                if *playing { "  ▶ playing" } else { "" }
            ),
            Choice::Device { name, default } => {
                format!("Input   {name}{}", if *default { "  (default)" } else { "" })
            }
            Choice::Demo => "Demo    built-in synth track".into(),
        }
    }

    fn open(&self) -> Result<AudioSource> {
        match self {
            Choice::App { bundle_id, .. } => AudioSource::tap(bundle_id),
            Choice::Device { name, .. } => AudioSource::device(Some(name)),
            Choice::Demo => Ok(AudioSource::demo()),
        }
    }

    fn apply(&self, args: &mut Args) {
        args.demo = false;
        args.tap = None;
        args.device = None;
        match self {
            Choice::App { bundle_id, .. } => args.tap = Some(bundle_id.clone()),
            Choice::Device { name, .. } => args.device = Some(name.clone()),
            Choice::Demo => args.demo = true,
        }
    }

    fn same_key(&self, other: &Choice) -> bool {
        match (self, other) {
            (Choice::App { bundle_id: a, .. }, Choice::App { bundle_id: b, .. }) => a == b,
            (Choice::Device { name: a, .. }, Choice::Device { name: b, .. }) => a == b,
            (Choice::Demo, Choice::Demo) => true,
            _ => false,
        }
    }
}

/// "com.google.Chrome.helper" → "Chrome".
fn short_name(bundle_id: &str) -> &str {
    let mut parts: Vec<&str> = bundle_id.split('.').collect();
    while parts.len() > 1
        && matches!(
            parts.last().map(|s| s.to_lowercase()).as_deref(),
            Some("helper" | "renderer" | "gpu" | "serviceextension")
        )
    {
        parts.pop();
    }
    parts.last().copied().unwrap_or(bundle_id)
}

fn quote(s: &str) -> String {
    if s.contains(' ') {
        format!("\"{s}\"")
    } else {
        s.to_string()
    }
}

/// Apps that are currently playing (macOS), input devices, then the demo track.
fn discover() -> Vec<Choice> {
    let mut list = Vec::new();
    #[cfg(target_os = "macos")]
    if let Ok(procs) = crate::tap::processes() {
        let own = std::process::id() as i32;
        for p in procs {
            let dup = list
                .iter()
                .any(|c| matches!(c, Choice::App { bundle_id, .. } if *bundle_id == p.bundle_id));
            if p.playing && !p.bundle_id.is_empty() && p.pid != own && !dup {
                list.push(Choice::App {
                    bundle_id: p.bundle_id,
                    playing: true,
                });
            }
        }
    }
    for (name, default) in audio::input_devices().unwrap_or_default() {
        list.push(Choice::Device { name, default });
    }
    list.push(Choice::Demo);
    list
}

#[derive(Clone, Copy, PartialEq)]
enum Row {
    Source(usize),
    Layout,
    Delay,
    Interval,
    Fps,
    Start,
}

enum Action {
    Continue,
    Start,
    Quit,
}

struct Setup {
    choices: Vec<Choice>,
    focus: usize,
    selected: Option<usize>,
    source: Option<AudioSource>,
    analyzer: Option<Analyzer>,
    error: Option<String>,
    mosaic: u8,
    delay: u64,
    interval: u64,
    fps: u32,
}

impl Setup {
    fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = (0..self.choices.len()).map(Row::Source).collect();
        rows.extend([Row::Layout, Row::Delay, Row::Interval, Row::Fps, Row::Start]);
        rows
    }

    fn focused(&self) -> Row {
        self.rows()[self.focus]
    }

    fn select(&mut self, i: usize) {
        // Tear down the old source first; two taps on the same app are wasteful.
        self.source = None;
        self.analyzer = None;
        self.selected = Some(i);
        match self.choices[i].open() {
            Ok(source) => {
                self.analyzer = Some(Analyzer::new(source.sample_rate as f32));
                self.source = Some(source);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    /// Re-scans sources, keeping the current selection if it still exists.
    fn refresh(&mut self) {
        let fresh = discover();
        let selected = self
            .selected
            .and_then(|i| fresh.iter().position(|c| c.same_key(&self.choices[i])));
        self.choices = fresh;
        self.selected = selected;
        if selected.is_none() {
            self.source = None;
            self.analyzer = None;
        }
        self.focus = self.focus.min(self.rows().len() - 1);
    }

    fn adjust(&mut self, dir: i64) {
        match self.focused() {
            Row::Layout => self.mosaic = (self.mosaic as i64 + dir).clamp(1, 4) as u8,
            Row::Delay => self.delay = (self.delay as i64 + dir * 10).clamp(0, 2000) as u64,
            Row::Interval => self.interval = (self.interval as i64 + dir * 10).clamp(10, 600) as u64,
            Row::Fps => self.fps = (self.fps as i64 + dir * 5).clamp(10, 60) as u32,
            _ => {}
        }
    }

    fn handle(&mut self, code: KeyCode, mods: KeyModifiers) -> Action {
        let last = self.rows().len() - 1;
        match code {
            KeyCode::Up => self.focus = self.focus.saturating_sub(1),
            KeyCode::Down => self.focus = (self.focus + 1).min(last),
            KeyCode::Left => self.adjust(-1),
            KeyCode::Right => self.adjust(1),
            KeyCode::Char(' ') => {
                if let Row::Source(i) = self.focused() {
                    self.select(i);
                }
            }
            KeyCode::Enter => match self.focused() {
                Row::Source(i) => {
                    self.select(i);
                    if self.source.is_some() {
                        self.focus = last;
                    }
                }
                _ if self.source.is_some() => return Action::Start,
                _ => self.error = Some("pick a working audio source first".into()),
            },
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => return Action::Quit,
            _ => {}
        }
        Action::Continue
    }

    fn command_line(&self) -> String {
        let mut cmd = String::from("soundscene");
        match self.selected.map(|i| &self.choices[i]) {
            Some(Choice::App { bundle_id, .. }) => write!(cmd, " --tap {bundle_id}").unwrap(),
            Some(Choice::Device { name, .. }) => write!(cmd, " --device {}", quote(name)).unwrap(),
            Some(Choice::Demo) => cmd.push_str(" --demo"),
            None => {}
        }
        if self.mosaic != 1 {
            write!(cmd, " --mosaic {}", self.mosaic).unwrap();
        }
        if self.delay != 0 {
            write!(cmd, " --delay {}", self.delay).unwrap();
        }
        if self.interval != 60 {
            write!(cmd, " --interval {}", self.interval).unwrap();
        }
        if self.fps != 30 {
            write!(cmd, " --fps {}", self.fps).unwrap();
        }
        cmd
    }

    fn draw(&self, out: &mut String, f: Option<&Features>) {
        const DIM: &str = "\x1b[38;2;130;130;160m";
        const FOCUS: &str = "\x1b[48;2;40;40;72m\x1b[38;2;235;235;255m";
        const ACCENT: &str = "\x1b[38;2;120;200;255m";
        const ERROR: &str = "\x1b[38;2;255;110;110m";

        out.clear();
        out.push_str("\x1b[?2026h\x1b[H");
        let mut line = |s: &str| {
            out.push_str(s);
            out.push_str("\x1b[0m\x1b[K\r\n");
        };
        let marker = |row: Row| {
            if self.focused() == row {
                (FOCUS, "▸")
            } else {
                ("", " ")
            }
        };

        line("");
        line(&format!(
            "  \x1b[1mSoundScene\x1b[0m {DIM}v{}  ·  setup",
            env!("CARGO_PKG_VERSION")
        ));
        line("");
        line(&format!(
            "  {ACCENT}Audio source{DIM}   apps show up here while they play audio · r refreshes"
        ));
        for (i, choice) in self.choices.iter().enumerate() {
            let (style, arrow) = marker(Row::Source(i));
            let radio = if self.selected == Some(i) { "●" } else { "○" };
            line(&format!("  {style}{arrow} {radio} {} ", choice.label()));
        }
        line("");
        match (&self.error, f) {
            (Some(e), _) => line(&format!("  Signal   {ERROR}{e}")),
            (None, Some(f)) => {
                let bar = |v: f32, n: usize| {
                    let k = (v.clamp(0.0, 1.0) * n as f32).round() as usize;
                    format!("{}{DIM}{}\x1b[39m", "█".repeat(k), "·".repeat(n - k))
                };
                line(&format!(
                    "  Signal   {}  bass {}  mid {}  treb {}  {}",
                    bar(f.level, 20),
                    bar(f.bass, 5),
                    bar(f.mid, 5),
                    bar(f.treble, 5),
                    if f.pulse > 0.5 { "●" } else { "○" }
                ));
            }
            (None, None) => line(&format!("  Signal   {DIM}no source selected")),
        }
        line("");
        line(&format!("  {ACCENT}Settings"));
        let setting = |row: Row, name: &str, value: String, hint: &str| {
            let (style, arrow) = marker(row);
            format!("  {style}{arrow} {name:<9} ◂ {value:>7} ▸ \x1b[0m  {DIM}{hint}")
        };
        let layout = if self.mosaic == 1 {
            "single".to_string()
        } else {
            format!("{0}×{0}", self.mosaic)
        };
        line(&setting(
            Row::Layout,
            "Layout",
            layout,
            "several effects at once; m switches while running",
        ));
        line(&setting(
            Row::Delay,
            "Delay",
            format!("{} ms", self.delay),
            "hold visuals back, ~200 ms for Bluetooth headphones",
        ));
        line(&setting(
            Row::Interval,
            "Rotate",
            format!("{} s", self.interval),
            "time per effect",
        ));
        line(&setting(
            Row::Fps,
            "FPS",
            self.fps.to_string(),
            "lower it if the terminal can't keep up",
        ));
        line("");
        let (style, arrow) = marker(Row::Start);
        line(&format!("  {style}{arrow} [ Start ] "));
        line("");
        line(&format!(
            "  {DIM}↑/↓ move · ←/→ change · Enter select / start · Space try source · r refresh · q quit"
        ));
        line(&format!("  {DIM}skip this next time:  \x1b[39m{}", self.command_line()));
        out.push_str("\x1b[J\x1b[?2026l");
    }
}

/// Runs the setup screen. Returns the opened source and the filled-in args,
/// or `None` if the user quit. Expects the terminal to already be in raw mode
/// on the alternate screen.
pub fn run(mut args: Args) -> Result<Option<(AudioSource, Args)>> {
    let choices = discover();
    let initial = choices
        .iter()
        .position(|c| matches!(c, Choice::App { playing: true, .. }))
        .or_else(|| {
            choices
                .iter()
                .position(|c| matches!(c, Choice::Device { default: true, .. }))
        })
        .unwrap_or(choices.len() - 1);
    let mut setup = Setup {
        choices,
        focus: initial,
        selected: None,
        source: None,
        analyzer: None,
        error: None,
        mosaic: args.mosaic,
        delay: args.delay,
        interval: args.interval,
        fps: args.fps,
    };
    setup.select(initial);

    let mut stdout = io::stdout().lock();
    let mut screen = String::new();
    let mut samples = Vec::with_capacity(FFT_SIZE);
    let start = Instant::now();
    let mut last = start;
    loop {
        while event::poll(Duration::ZERO)? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Release {
                    continue;
                }
                match setup.handle(k.code, k.modifiers) {
                    Action::Continue => {}
                    Action::Quit => return Ok(None),
                    Action::Start => {
                        let i = setup.selected.expect("a source is open");
                        setup.choices[i].apply(&mut args);
                        args.mosaic = setup.mosaic;
                        args.delay = setup.delay;
                        args.interval = setup.interval;
                        args.fps = setup.fps;
                        let source = setup.source.take().expect("a source is open");
                        return Ok(Some((source, args)));
                    }
                }
            }
        }

        let now = Instant::now();
        if let (Some(source), Some(analyzer)) = (&setup.source, &mut setup.analyzer) {
            source.snapshot(FFT_SIZE, 0, &mut samples);
            analyzer.update(
                &samples,
                now.duration_since(start).as_secs_f32(),
                now.duration_since(last).as_secs_f32().min(0.1),
            );
        }
        last = now;

        setup.draw(&mut screen, setup.analyzer.as_ref().map(|a| &a.features));
        stdout.write_all(screen.as_bytes())?;
        stdout.flush()?;
        std::thread::sleep(Duration::from_millis(33));
    }
}
