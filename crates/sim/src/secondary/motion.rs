//! Shared secondary motion from `native-expanding-wave.ts` and `native-stoneskin.ts`.

use crate::float::{cos, fround, sin};
use crate::secondary::{SecondaryActorRead, SecondaryActorWrite};
use crate::{NativeRng, Vec2};

/// Both Comet debris and Stoneskin chips retain the same ordinary Bouncer fields.
/// The Source converts its stored radians to degrees for each shared motion step.
pub(super) fn step_secondary_bouncer(
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    tick: u64,
    rng: &mut NativeRng,
) -> bool {
    let mut motion = crate::death_animations::NativeBouncerMotion {
        bounce_velocity: source.endpoint.y,
        height: source.phase,
        position: source.position,
        rotation_degrees: source.rotation_radians * 180.0 / std::f64::consts::PI,
        rotation_step_degrees: source.slow_factor * 180.0 / std::f64::consts::PI,
        velocity: source.velocity,
        vertical_velocity: source.endpoint.x,
    };
    let skipped = crate::death_animations::step_native_bouncer_motion(&mut motion, tick, rng);
    let life = if skipped { source.alpha } else { fround(source.alpha - (0.015_f32 as f64)) };
    actor.alpha = life.max(0.0);
    actor.endpoint = Vec2::new(motion.vertical_velocity, motion.bounce_velocity);
    actor.phase = motion.height;
    actor.position = motion.position;
    actor.rotation_radians = motion.rotation_degrees * std::f64::consts::PI / 180.0;
    actor.slow_factor = motion.rotation_step_degrees * std::f64::consts::PI / 180.0;
    actor.velocity = motion.velocity;
    life > 0.0
}

pub const NATIVE_STONESKIN_GRID_SIDE: usize = 10;
pub const NATIVE_STONESKIN_GRID_COMPONENTS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeExpandingWave {
    pub radius: f64,
    pub growth: f64,
    pub scalar: f64,
    pub life: f64,
    pub fade_threshold: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeExpandingWaveStep {
    pub radius: f64,
    pub scalar: f64,
    pub life: f64,
    pub displacement: Vec2,
    pub retain: bool,
}

/// Camera proposals consume one native unit-vector draw even on the wave's last tick.
#[must_use]
pub fn step_native_expanding_wave(
    source: NativeExpandingWave,
    rng: &mut NativeRng,
) -> NativeExpandingWaveStep {
    let radius = fround(source.radius + source.growth);
    let vector = rng.draw_unit_vector();
    let displacement = Vec2::new(
        fround(fround(vector.x * 3.0) * source.scalar),
        fround(fround(vector.y * 3.0) * source.scalar),
    );
    let life = fround(source.life - (0.01_f32 as f64));
    NativeExpandingWaveStep {
        radius,
        displacement,
        life,
        retain: life > 0.0,
        scalar: if life < source.fade_threshold {
            fround(source.scalar * (0.9_f32 as f64))
        } else {
            source.scalar
        },
    }
}

/// `Game::MakeStoneCoords`: both quality modes generate the complete grid and grey bank.
#[must_use]
pub fn create_native_stoneskin_warp(rng: &mut NativeRng) -> Vec<f64> {
    let mut positions = Vec::with_capacity(NATIVE_STONESKIN_GRID_COMPONENTS);
    for x in 0..NATIVE_STONESKIN_GRID_SIDE {
        for y in 0..NATIVE_STONESKIN_GRID_SIDE {
            if rng.draw_integer(3) == 1 {
                rng.draw_scaled_float(0.5, false);
            }
            let distance = rng.draw_scaled_float(4.0, false);
            let angle = rng.draw_scaled_float(360.0, false);
            let radians = angle * std::f64::consts::PI / 180.0;
            positions.push(fround(
                fround(fround(x as f64 * (12.8_f32 as f64)) - 64.0)
                    + fround(fround(sin(radians)) * distance),
            ));
            positions.push(fround(
                fround(fround(y as f64 * (12.8_f32 as f64)) - 64.0)
                    + fround(fround(-cos(radians)) * distance),
            ));
        }
    }
    positions
}
