mod copper;
mod cube;
mod dotsphere;
mod fire;
mod helix;
mod inferno;
mod julia;
mod kaleidoscope;
mod life;
mod lissajous;
mod matrix;
mod metaballs;
mod moire;
mod particles;
mod plasma;
mod rings;
mod ripples;
mod rotozoom;
mod scope;
mod spectrum;
mod starfield;
mod synthwave;
mod tunnel;
mod twister;
mod voronoi;
mod voxel;
mod waterfall;

use crate::analysis::Features;
use crate::gfx::Canvas;

pub trait Effect {
    fn name(&self) -> &'static str;

    /// Draws one frame. The canvas may change size between calls (window
    /// resize, mosaic layout change), and is cleared when this effect becomes
    /// visible; effects that keep trails rely on the previous frame still
    /// being in it. In mosaic mode each tile has its own canvas.
    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features);
}

/// Rotation order for ←/→, the auto-rotate timer and mosaic pages.
pub fn all() -> Vec<Box<dyn Effect>> {
    vec![
        Box::new(spectrum::Spectrum::new()),
        Box::new(plasma::Plasma::new()),
        Box::new(tunnel::Tunnel::new()),
        Box::new(starfield::Starfield::new()),
        Box::new(fire::Fire::new()),
        Box::new(scope::Scope::new()),
        Box::new(inferno::Inferno::new()),
        Box::new(metaballs::Metaballs::new()),
        Box::new(synthwave::Synthwave::new()),
        Box::new(rotozoom::Rotozoom::new()),
        Box::new(copper::Copper::new()),
        Box::new(rings::Rings::new()),
        Box::new(voxel::Voxel::new()),
        Box::new(kaleidoscope::Kaleidoscope::new()),
        Box::new(cube::Cube::new()),
        Box::new(waterfall::Waterfall::new()),
        Box::new(julia::Julia::new()),
        Box::new(matrix::Matrix::new()),
        Box::new(twister::Twister::new()),
        Box::new(dotsphere::DotSphere::new()),
        Box::new(moire::Moire::new()),
        Box::new(particles::Particles::new()),
        Box::new(voronoi::Voronoi::new()),
        Box::new(ripples::Ripples::new()),
        Box::new(helix::Helix::new()),
        Box::new(lissajous::Lissajous::new()),
        Box::new(life::Life::new()),
    ]
}
