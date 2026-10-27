mod fire;
mod plasma;
mod scope;
mod spectrum;
mod starfield;
mod tunnel;

use crate::analysis::Features;
use crate::gfx::Canvas;

pub trait Effect {
    fn name(&self) -> &'static str;

    /// Draws one frame. The canvas may change size between calls, and is
    /// cleared when this effect becomes active; effects that keep trails rely
    /// on the previous frame still being in it.
    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features);
}

/// Rotation order for ←/→ and the auto-rotate timer.
pub fn all() -> Vec<Box<dyn Effect>> {
    vec![
        Box::new(spectrum::Spectrum::new()),
        Box::new(plasma::Plasma::new()),
        Box::new(tunnel::Tunnel::new()),
        Box::new(starfield::Starfield::new()),
        Box::new(fire::Fire::new()),
        Box::new(scope::Scope::new()),
    ]
}
