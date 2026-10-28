# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.4.0] - 2026-10-28

### Added

- `f` toggles fullscreen: hides the status line and asks the terminal to go fullscreen where supported.
- Auto-rotation can be turned off: `--interval 0`, or "off" in the setup screen. The status line then shows "rotate off".
- `m` also cycles through the 4×4 mosaic.
- The setup screen lists the keys that work while the effects run.

### Changed

- Effects rotate every 10 seconds by default instead of 60.

## [0.3.0] - 2026-10-28

### Added

- 70 new effects (97 in total), grouped in the README: music and spectrum, old-school classics, fractals, 3D and space, simulations, nature and patterns, automata and retro.
- `c` rotates the colors of every effect (hue shift in 60° steps).
- A test that renders every effect at all kinds of canvas sizes, including 0×0, and a speed check (`cargo test --release -- --ignored --nocapture speed`).

## [0.2.0] - 2026-10-28

### Added

- Windows: record everything that plays on an output device (WASAPI loopback), so music from Chrome or any other app can drive the visuals. Use `--loopback` (optionally with `--device` to pick the output), or pick a "System" entry in the setup screen, which now preselects the default output on Windows.
- `--list-devices` also lists output devices on Windows.

## [0.1.0] - 2026-10-28

First release.

### Added

- 27 sound-reactive effects: Spectrum, Plasma, Tunnel, Starfield, Fire, Scope, Inferno, Metaballs, Synthwave, Rotozoom, Copper, Rings, Voxel, Kaleidoscope, Cube, Waterfall, Julia, Matrix, Twister, DotSphere, Moire, Particles, Voronoi, Ripples, Helix, Lissajous and Life.
- Mosaic mode (`m` or `--mosaic N`) to show an N×N grid of effects at once.
- Interactive setup screen when started without arguments, with a live level meter for the selected source.
- Audio input from any input device, a built-in demo track (`--demo`), or on macOS 14.4+ straight from a single app via a Core Audio process tap (`--tap`).
- `--delay` to hold the visuals back for Bluetooth headphone latency.
- `--meter` to print levels and detected beats without starting the UI.
- Auto-rotation between effects (`--interval`), adjustable frame rate (`--fps`).

[Unreleased]: https://github.com/voidNK/SoundScene/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/voidNK/SoundScene/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/voidNK/SoundScene/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/voidNK/SoundScene/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/voidNK/SoundScene/releases/tag/v0.1.0
