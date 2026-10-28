mod aurora;
mod balls;
mod blackhole;
mod blocks;
mod boids;
mod bumpmap;
mod burningship;
mod caustics;
mod chaosgame;
mod chladni;
mod city;
mod clifford;
mod clouds;
mod copper;
mod cube;
mod cyclic;
mod digits;
mod dotsphere;
mod dotwave;
mod dragon;
mod fern;
mod fire;
mod fireflies;
mod fireworks;
mod flame;
mod flowfield;
mod fluid;
mod galaxy;
mod glenz;
mod glitch;
mod globe;
mod heartbeat;
mod helix;
mod hexpulse;
mod hypno;
mod inferno;
mod julia;
mod kaleidoscope;
mod kefrens;
mod langton;
mod lasers;
mod lens;
mod life;
mod lightning;
mod lissajous;
mod lorenz;
mod mandelbrot;
mod matrix;
mod maze;
mod metaballs;
mod mode7;
mod moire;
mod newton;
mod ocean;
mod orbits;
mod particles;
mod pendulum;
mod plasma;
mod pleasures;
mod pong;
mod pythagoras;
mod radar;
mod radial;
mod rain;
mod raymarch;
mod reaction;
mod ribbons;
mod rings;
mod ripples;
mod rose;
mod rotozoom;
mod rubbercube;
mod rule30;
mod sand;
mod scope;
mod scroller;
mod shadebobs;
mod snow;
mod spectrum;
mod spirograph;
mod starfield;
mod static_noise;
mod sunburst;
mod synthwave;
mod tesseract;
mod testcard;
#[cfg(test)]
mod tests;
mod torusknot;
mod tree;
mod truchet;
mod tunnel;
mod twister;
mod vectorballs;
mod voronoi;
mod voxel;
mod vumeter;
mod warp;
mod waterfall;
mod wormhole;

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
        Box::new(scroller::SineScroller::new()),
        Box::new(pleasures::UnknownPleasures::new()),
        Box::new(radial::RadialSpectrum::new()),
        Box::new(vumeter::VuMeter::new()),
        Box::new(city::EqualizerCity::new()),
        Box::new(fireworks::Fireworks::new()),
        Box::new(chladni::Chladni::new()),
        Box::new(kefrens::KefrensBars::new()),
        Box::new(lasers::Lasers::new()),
        Box::new(pendulum::PendulumWave::new()),
        Box::new(bumpmap::BumpMap::new()),
        Box::new(shadebobs::Shadebobs::new()),
        Box::new(vectorballs::VectorBalls::new()),
        Box::new(glenz::Glenz::new()),
        Box::new(lens::Lens::new()),
        Box::new(rubbercube::RubberCube::new()),
        Box::new(mode7::Mode7::new()),
        Box::new(wormhole::Wormhole::new()),
        Box::new(sunburst::Sunburst::new()),
        Box::new(hypno::HypnoSpiral::new()),
        Box::new(mandelbrot::MandelbrotZoom::new()),
        Box::new(flame::FractalFlame::new()),
        Box::new(newton::Newton::new()),
        Box::new(burningship::BurningShip::new()),
        Box::new(pythagoras::PythagorasTree::new()),
        Box::new(dragon::DragonCurve::new()),
        Box::new(chaosgame::ChaosGame::new()),
        Box::new(fern::Fern::new()),
        Box::new(tree::FractalTree::new()),
        Box::new(lorenz::Lorenz::new()),
        Box::new(raymarch::Raymarch::new()),
        Box::new(torusknot::TorusKnot::new()),
        Box::new(tesseract::Tesseract::new()),
        Box::new(globe::Globe::new()),
        Box::new(galaxy::Galaxy::new()),
        Box::new(blackhole::BlackHole::new()),
        Box::new(warp::Warp::new()),
        Box::new(ribbons::Ribbons::new()),
        Box::new(dotwave::DotWave::new()),
        Box::new(orbits::Orbits::new()),
        Box::new(reaction::ReactionDiffusion::new()),
        Box::new(flowfield::FlowField::new()),
        Box::new(boids::Boids::new()),
        Box::new(fluid::Fluid::new()),
        Box::new(sand::Sand::new()),
        Box::new(balls::BouncingBalls::new()),
        Box::new(clifford::Clifford::new()),
        Box::new(lightning::Lightning::new()),
        Box::new(caustics::Caustics::new()),
        Box::new(spirograph::Spirograph::new()),
        Box::new(rain::Rain::new()),
        Box::new(snow::Snow::new()),
        Box::new(aurora::Aurora::new()),
        Box::new(ocean::Ocean::new()),
        Box::new(clouds::Clouds::new()),
        Box::new(rose::Rose::new()),
        Box::new(radar::Radar::new()),
        Box::new(truchet::Truchet::new()),
        Box::new(hexpulse::HexPulse::new()),
        Box::new(fireflies::Fireflies::new()),
        Box::new(langton::Langton::new()),
        Box::new(rule30::Rule30::new()),
        Box::new(cyclic::Cyclic::new()),
        Box::new(maze::Maze::new()),
        Box::new(blocks::Blocks::new()),
        Box::new(pong::Pong::new()),
        Box::new(testcard::TestCard::new()),
        Box::new(static_noise::Static::new()),
        Box::new(glitch::Glitch::new()),
        Box::new(heartbeat::Heartbeat::new()),
    ]
}
