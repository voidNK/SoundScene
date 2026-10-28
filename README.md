# SoundScene

Old-school demoscene effects in your terminal, driven by whatever music you're playing.

I wanted something to put on a second screen while music is running: a plasma that speeds up with the bass, a tunnel that lurches forward on every kick, a fire that gets taller when the track gets louder. It runs in any truecolor terminal, needs no GPU, and on macOS it can listen straight to a single app (like the Chrome tab playing your music) without any loopback-driver setup.

![All 27 effects, page 1 of the 3×3 mosaic](docs/screenshots/mosaic-1.png)

| | |
|---|---|
| ![Spectrum](docs/screenshots/spectrum.png) | ![Plasma](docs/screenshots/plasma.png) |
| ![Tunnel](docs/screenshots/tunnel.png) | ![Inferno](docs/screenshots/inferno.png) |
| ![Fire](docs/screenshots/fire.png) | ![Scope](docs/screenshots/scope.png) |

## Effects

There are 27 of them. In rotation order:

| Effect | Reacts to |
|---|---|
| **Spectrum** | 64-band log spectrum with falling peak caps; the background flashes on beats |
| **Plasma** | bass sets the speed, mids the zoom, treble shifts the colors; beats brighten it |
| **Tunnel** | bass pushes you forward, mids spin and twist the tunnel |
| **Starfield** | bass and beats kick it into warp; treble tints the stars |
| **Fire** | bass feeds the flames, loudness sets their height, beats throw sparks |
| **Scope** | circular oscilloscope of the waveform, with mirrored spectrum spikes around it |
| **Inferno** | fire burning in from all four edges, each side in its own palette; the palettes rotate every 8 beats |
| **Metaballs** | bass inflates the blobs, beats shift their color |
| **Synthwave** | striped sun, mountains made of the spectrum and a neon grid that races with the bass |
| **Rotozoom** | rotating, zooming checkerboard; bass zooms in, mids spin it |
| **Copper** | Amiga-style copper bars; each bar's thickness follows a slice of the spectrum |
| **Rings** | circular equalizer: 16 concentric arcs, one per band |
| **Voxel** | Comanche-style landscape flyover; bass speeds up the flight and raises the mountains |
| **Kaleidoscope** | plasma folded into eight mirrored segments |
| **Cube** | two nested wireframe cubes with glow trails |
| **Waterfall** | scrolling spectrogram |
| **Julia** | animated Julia set; mids push it along, bass zooms in |
| **Matrix** | digital rain; bass speeds it up, treble tints it cyan |
| **Twister** | the classic twisting column; mids twist it, bass makes it sway |
| **DotSphere** | rotating sphere of dots that grows spikes with the spectrum |
| **Moire** | interference of two ring patterns; bass speeds up the waves |
| **Particles** | fountain fed by the loudness plus a firework burst on every beat |
| **Voronoi** | drifting cells, each lit by its own band |
| **Ripples** | water surface; beats drop stones in, treble adds a drizzle |
| **Helix** | DNA double helix whose rungs light up with the spectrum |
| **Lissajous** | morphing Lissajous figure; loudness sets its size |
| **Life** | Conway's Game of Life; bass speeds up the generations, beats seed new cells |

Effects rotate every 60 seconds. You can also flip through them with the arrow keys.

### Mosaic mode

Press `m` to see several effects at once in a 2×2 or 3×3 grid (press again to go back to a single effect), or start with `--mosaic 3`. The arrow keys and the rotation timer then move a whole page at a time. With 3×3, the 27 effects fit exactly on three pages:

| | |
|---|---|
| ![Mosaic page 2](docs/screenshots/mosaic-2.png) | ![Mosaic page 3](docs/screenshots/mosaic-3.png) |

## Building

You need a recent stable Rust toolchain (edition 2024).

```sh
cargo build --release
```

The binary ends up in `target/release/soundscene`. It's a single self-contained executable, so you can copy it wherever you like.

## Running

The easiest way in is to just start it without any arguments:

```sh
./target/release/soundscene
```

That opens a small setup screen. It lists the apps that are currently playing audio (on macOS), your input devices and the demo track. The selected source gets a live level meter, so you can see right away whether anything is coming through. Below that you can set the layout (single effect or mosaic), the delay, the rotation time and the frame rate. The bottom line shows the matching command line, so next time you can skip the setup and start straight into the effects.

```
  SoundScene  setup

  Audio source   apps show up here while they play audio · r refreshes
  ▸ ● App     Chrome       com.google.Chrome.helper  ▶ playing
    ○ Input   MacBook Pro Microphone  (default)
    ○ Input   BlackHole 2ch
    ○ Demo    built-in synth track

  Signal   ██████████··········  bass ██···  mid ██···  treb ███··  ○

  Settings
    Layout    ◂  single ▸   several effects at once; m switches while running
    Delay     ◂  200 ms ▸   hold visuals back, ~200 ms for Bluetooth headphones
    Rotate    ◂    60 s ▸   time per effect
    FPS       ◂      30 ▸   lower it if the terminal can't keep up

    [ Start ]

  skip this next time:  soundscene --tap com.google.Chrome.helper --delay 200
```

`↑`/`↓` moves, `←`/`→` changes a value, `Enter` picks a source or starts, `r` rescans (handy if you hit play after launching), and `q` quits.

Any argument skips the setup. The quickest way to see something is the built-in demo track, which needs no audio device:

```sh
./target/release/soundscene --demo
```

### Listening to an app (macOS 14.4+)

This is what I use day to day. It taps the audio output of one app directly through Core Audio. The sound still goes to your speakers or headphones as usual, and the volume keys keep working.

```sh
# see which apps are currently known to the audio system (▶ = playing)
./target/release/soundscene --list-apps

# visualize Chrome
./target/release/soundscene --tap chrome
```

`--tap` matches on a part of the bundle ID, so `chrome` picks up `com.google.Chrome.helper`, the process that actually plays audio in Chrome. The first time you run it, macOS asks to allow "System Audio Recording" for your terminal app.

### Listening to an input device

The default is your system's default input device, which is usually the microphone. That works surprisingly well with speakers. To pick a different device:

```sh
./target/release/soundscene --list-devices
./target/release/soundscene --device "macbook pro"
```

To visualize system audio on macOS without `--tap`, you can also go the classic loopback route. Install [BlackHole](https://github.com/ExistentialAudio/BlackHole), create a Multi-Output Device in Audio MIDI Setup with your speakers and BlackHole, make it the system output, and run with `--device blackhole`.

### Bluetooth headphones

Bluetooth adds roughly 150–250 ms of latency. The app hears the audio *before* it reaches your ears, so the visuals end up slightly ahead of the beat. `--delay` holds the visuals back to compensate:

```sh
./target/release/soundscene --tap chrome --delay 200
```

Tune the value by ear until the kicks and the flashes line up.

## Keys

| Key | Action |
|---|---|
| `←` / `→` | previous / next effect, or page in mosaic mode (also resets the rotation timer) |
| `m` | cycle the layout: single → 2×2 → 3×3 mosaic |
| `h` | toggle the status line |
| `q`, `Esc`, `Ctrl-C` | quit |

## Options

```
    --demo                 Use a built-in synthetic track instead of an audio device
-t, --tap <APP>            Capture an app's audio output (macOS 14.4+), matched by bundle ID
-d, --device <DEVICE>      Input device (case-insensitive substring), e.g. "BlackHole"
    --list-apps            List apps known to the audio system and exit
    --list-devices         List input devices and exit
    --delay <MS>           Delay the visuals, e.g. ~200 for Bluetooth headphones (0–2000)
    --interval <SECONDS>   Seconds before auto-rotating to the next effect [default: 60]
    --fps <FPS>            Target frames per second [default: 30]
    --mosaic <N>           Show several effects at once in an N×N grid, 1–4 [default: 1]
    --meter <SECONDS>      Print audio levels and beats instead of starting the UI
```

## Troubleshooting

**Nothing moves.** Run with `--meter 5`. It prints the raw peak and the normalized levels twice a second and marks detected beats. If `peak` stays at `0.0000`, no audio is reaching the app. Check that the app you tap is actually playing, that the right device is selected, and that your terminal has the Microphone or System Audio Recording permission in System Settings → Privacy & Security.

**Choppy output.** Rendering is just a lot of ANSI escape codes, so the terminal does most of the work. A smaller window or `--fps 20` helps. GPU-accelerated terminals (iTerm2, kitty, WezTerm, Ghostty) cope best.

**Headphones sound bad / spectrum looks cut off above 8 kHz.** Bluetooth headsets switch to a 16 kHz hands-free mode whenever their microphone is in use. Set your input device to something else, such as the built-in mic, and they go back to full-quality stereo.

## How it works

Every frame, the app takes the latest 2048 samples and runs a Hann-windowed FFT. From that it derives bass/mid/treble energy, a log-spaced spectrum, the waveform and a simple beat detector: bass energy jumping well above its recent average. All values are auto-gained against a slowly decaying peak, so a quiet microphone and a loud loopback produce similar visuals.

Effects draw into an RGB buffer. Each terminal cell holds two pixels using the upper half-block character `▀`, with the foreground color as the top pixel and the background color as the bottom one. The whole frame goes out as one write of truecolor escape codes.

The macOS app tap uses the Core Audio process tap API (`CATapDescription` + a private aggregate device). Nothing is recorded or stored. The audio is only analyzed in memory.

## License

No license has been chosen yet.
