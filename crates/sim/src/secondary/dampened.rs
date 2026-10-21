//! Native Dampen projectile flyout and stationary smoke constructors.

use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeDampenedSpellState {
    pub heading_degrees: f64,
    pub phase_degrees: f64,
    pub position: Vec2,
    pub velocity: Vec2,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeDampenedSmoke {
    pub alpha_loss: f64,
    pub entry: i32,
    pub rotation_degrees: f64,
    pub scale: f64,
    pub tint: u32,
    pub color_alpha: f64,
    pub position: Vec2,
}

pub fn create_native_dampened_spell(
    origin: Vec2,
    position: Vec2,
    rng: &mut NativeRng,
) -> NativeDampenedSpellState {
    let phase = rng.draw_scaled_float(360.0, false);
    let dx = fround(position.x - origin.x);
    let dy = fround(position.y - origin.y);
    let distance = hypot(dx, dy);
    let velocity = if distance == 0.0 {
        Vec2::ZERO
    } else {
        Vec2::new(fround(dx / distance * 7.0), fround(dy / distance * 7.0))
    };
    NativeDampenedSpellState { heading_degrees: -1.0, phase_degrees: phase, position, velocity }
}
pub fn step_native_dampened_spell(
    state: &mut NativeDampenedSpellState,
    variant: i32,
    rng: &mut NativeRng,
) -> NativeDampenedSmoke {
    state.position = Vec2::new(
        fround(state.position.x + state.velocity.x),
        fround(state.position.y + state.velocity.y),
    );
    state.phase_degrees = fround(state.phase_degrees + 5.0);
    if state.heading_degrees == -1.0 {
        state.heading_degrees = heading_degrees_from_vector(state.velocity.x, state.velocity.y);
    }
    let mut entry = 10;
    let rotation;
    let scale;
    let alpha_loss;
    let tint;
    if variant == 3 {
        entry += rng.draw_integer(2);
        rotation = rng.draw_scaled_float(360.0, false);
        scale = rng.draw_float_range(0.6_f32 as f64, 0.8_f32 as f64);
        alpha_loss = fround((0.01_f32 as f64) + rng.draw_scaled_float(0.02_f32 as f64, false));
        tint = ((rng.draw_scaled_float(0.15_f32 as f64, false) * 255.0 + 0.5).floor() as u32) << 16;
    } else {
        scale = rng.draw_float_range(0.5, 0.8_f32 as f64);
        rotation = rng.draw_scaled_float(360.0, false);
        alpha_loss = fround((0.02_f32 as f64) * (0.8_f32 as f64));
        tint = if variant == 0 {
            0xff8000
        } else if variant == 1 {
            0x40ff40
        } else {
            0x4080ff
        };
    }
    NativeDampenedSmoke {
        alpha_loss,
        entry,
        rotation_degrees: rotation,
        scale,
        tint,
        color_alpha: if variant == 0 { 0.5 } else { 1.0 },
        position: Vec2::new(
            state.position.x,
            fround(state.position.y - if variant == 3 { 0.0 } else { 15.0 }),
        ),
    }
}
