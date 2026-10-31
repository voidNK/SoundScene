mod ants;
mod apollonian;
mod aurora;
mod balls;
mod bandballs;
mod beatburst;
mod blackhole;
mod blocks;
mod boids;
mod bokeh;
mod briansbrain;
mod bubbles;
mod buddhabrot;
mod bumpmap;
mod burningship;
mod cassette;
mod caustics;
mod chaosgame;
mod checkerboard;
mod chladni;
mod circlepacking;
mod circuit;
mod city;
mod clifford;
mod cloth;
mod clouds;
mod colorcycle;
mod copper;
mod corridor;
mod crt;
mod crystal;
mod cube;
mod cubegrid;
mod cubetunnel;
mod cyclic;
mod digits;
mod dla;
mod donut;
mod dotmorph;
mod dotsphere;
mod dottunnel;
mod dotwave;
mod doublependulum;
mod dragon;
mod drips;
mod dunes;
mod explode;
mod fern;
mod fire;
mod fireflies;
mod fireworks;
mod fish;
mod flame;
mod flowfield;
mod fluid;
mod forestfire;
mod galaxy;
mod girih;
mod glenz;
mod glitch;
mod globe;
mod grass;
mod guilloche;
mod halftone;
mod heartbeat;
mod helix;
mod hexdump;
mod hexpulse;
mod hilbert;
mod htree;
mod hypno;
mod inferno;
mod infinitezoom;
mod ink;
mod ising;
mod jelly;
mod jellyfish;
mod julia;
mod kaleidoscope;
mod kefrens;
mod klein;
mod koch;
mod langton;
mod lasers;
mod lavalamp;
mod leaves;
mod ledmatrix;
mod lenia;
mod lens;
mod levycurve;
mod life;
mod lightning;
mod lighttrails;
mod lissajous;
mod loader;
mod lorenz;
mod lyapunov;
mod mandala;
mod mandelbrot;
mod marble;
mod matrix;
mod maze;
mod metaballs;
mod meteors;
mod mirrorbars;
mod mobius;
mod mode7;
mod moire;
mod multibrot;
mod nbody;
mod neon;
mod newton;
mod nixie;
mod ocean;
mod opart;
mod orbits;
mod particles;
mod pendulum;
mod penrose;
mod phoenix;
mod phyllotaxis;
mod physarum;
mod pianoroll;
mod plasma;
mod pleasures;
mod polarscope;
mod polygons;
mod polyhedra;
mod pong;
mod prism;
mod pythagoras;
mod radar;
mod radial;
mod rain;
mod raymarch;
mod raytrace;
mod reaction;
mod reflection;
mod ribbons;
mod ringedplanet;
mod rings;
mod ripples;
mod rockfield;
mod rockpaperscissors;
mod rose;
mod rotozoom;
mod rubbercube;
mod rule30;
mod rules;
mod sand;
mod sandpile;
mod scope;
mod scroller;
mod seeds;
mod sevenseg;
mod shadebobs;
mod sierpinski;
mod sinedistort;
mod smoke;
mod snow;
mod speaker;
mod spectrum;
mod spectrumterrain;
mod spectrumtunnel;
mod spirograph;
mod spring;
mod stainedglass;
mod starfield;
mod static_noise;
mod stringart;
mod sunburst;
mod sunset;
mod synthwave;
mod teletext;
mod tesseract;
mod testcard;
#[cfg(test)]
mod tests;
mod torusknot;
mod tracker;
mod tree;
mod tricorn;
mod truchet;
mod tunnel;
mod turmite;
mod twirl;
mod twister;
mod vectorballs;
mod vines;
mod voronoi;
mod voxel;
mod vumeter;
mod warp;
mod waterfall;
mod wavering;
mod wavesim;
mod wavestack;
mod wireworld;
mod wormhole;
mod xorpattern;

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
        Box::new(dottunnel::DotTunnel::new()),
        Box::new(checkerboard::Checkerboard::new()),
        Box::new(sinedistort::SineDistort::new()),
        Box::new(dotmorph::DotMorph::new()),
        Box::new(infinitezoom::InfiniteZoom::new()),
        Box::new(polyhedra::Polyhedra::new()),
        Box::new(corridor::Corridor::new()),
        Box::new(reflection::Reflection::new()),
        Box::new(twirl::Twirl::new()),
        Box::new(xorpattern::XorPattern::new()),
        Box::new(wavering::WaveRing::new()),
        Box::new(spectrumterrain::SpectrumTerrain::new()),
        Box::new(mirrorbars::MirrorBars::new()),
        Box::new(beatburst::BeatBurst::new()),
        Box::new(polarscope::PolarScope::new()),
        Box::new(bandballs::BandBalls::new()),
        Box::new(spectrumtunnel::SpectrumTunnel::new()),
        Box::new(speaker::Speaker::new()),
        Box::new(pianoroll::PianoRoll::new()),
        Box::new(wavestack::WaveStack::new()),
        Box::new(penrose::Penrose::new()),
        Box::new(guilloche::Guilloche::new()),
        Box::new(opart::OpArt::new()),
        Box::new(polygons::Polygons::new()),
        Box::new(stringart::StringArt::new()),
        Box::new(girih::Girih::new()),
        Box::new(mandala::Mandala::new()),
        Box::new(htree::HTree::new()),
        Box::new(phyllotaxis::Phyllotaxis::new()),
        Box::new(circlepacking::CirclePacking::new()),
        Box::new(sierpinski::Sierpinski::new()),
        Box::new(koch::Koch::new()),
        Box::new(hilbert::Hilbert::new()),
        Box::new(lyapunov::Lyapunov::new()),
        Box::new(tricorn::Tricorn::new()),
        Box::new(multibrot::Multibrot::new()),
        Box::new(phoenix::Phoenix::new()),
        Box::new(apollonian::Apollonian::new()),
        Box::new(buddhabrot::Buddhabrot::new()),
        Box::new(levycurve::LevyCurve::new()),
        Box::new(cubegrid::CubeGrid::new()),
        Box::new(explode::Explode::new()),
        Box::new(donut::Donut::new()),
        Box::new(mobius::Mobius::new()),
        Box::new(klein::Klein::new()),
        Box::new(spring::Spring::new()),
        Box::new(rockfield::RockField::new()),
        Box::new(ringedplanet::RingedPlanet::new()),
        Box::new(cubetunnel::CubeTunnel::new()),
        Box::new(raytrace::Raytrace::new()),
        Box::new(doublependulum::DoublePendulum::new()),
        Box::new(cloth::Cloth::new()),
        Box::new(wavesim::WaveSim::new()),
        Box::new(ising::Ising::new()),
        Box::new(dla::Dla::new()),
        Box::new(physarum::Physarum::new()),
        Box::new(smoke::Smoke::new()),
        Box::new(nbody::NBody::new()),
        Box::new(jelly::Jelly::new()),
        Box::new(ants::Ants::new()),
        Box::new(forestfire::ForestFire::new()),
        Box::new(briansbrain::BriansBrain::new()),
        Box::new(lenia::Lenia::new()),
        Box::new(seeds::Seeds::new()),
        Box::new(sandpile::Sandpile::new()),
        Box::new(rockpaperscissors::RockPaperScissors::new()),
        Box::new(turmite::Turmite::new()),
        Box::new(rules::Rules::new()),
        Box::new(crystal::Crystal::new()),
        Box::new(wireworld::Wireworld::new()),
        Box::new(lavalamp::LavaLamp::new()),
        Box::new(meteors::Meteors::new()),
        Box::new(bubbles::Bubbles::new()),
        Box::new(jellyfish::Jellyfish::new()),
        Box::new(grass::Grass::new()),
        Box::new(sunset::Sunset::new()),
        Box::new(fish::Fish::new()),
        Box::new(dunes::Dunes::new()),
        Box::new(vines::Vines::new()),
        Box::new(leaves::Leaves::new()),
        Box::new(loader::Loader::new()),
        Box::new(hexdump::HexDump::new()),
        Box::new(circuit::Circuit::new()),
        Box::new(teletext::Teletext::new()),
        Box::new(tracker::Tracker::new()),
        Box::new(cassette::Cassette::new()),
        Box::new(ledmatrix::LedMatrix::new()),
        Box::new(nixie::Nixie::new()),
        Box::new(sevenseg::SevenSeg::new()),
        Box::new(crt::Crt::new()),
        Box::new(ink::Ink::new()),
        Box::new(drips::Drips::new()),
        Box::new(neon::Neon::new()),
        Box::new(lighttrails::LightTrails::new()),
        Box::new(bokeh::Bokeh::new()),
        Box::new(prism::Prism::new()),
        Box::new(halftone::Halftone::new()),
        Box::new(stainedglass::StainedGlass::new()),
        Box::new(marble::Marble::new()),
        Box::new(colorcycle::ColorCycle::new()),
    ]
}
