# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/voidNK/SoundScene/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/voidNK/SoundScene/releases/tag/v0.1.0
