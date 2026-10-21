//! Faculty's private missile from `core-kernels/native-guided-missile.ts`.

use crate::float;
use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::spells::targeting::native_heading_turn_direction;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeGuidedMissileState {
    pub heading_deg: f64,
    pub minimum_speed: f64,
    pub phase_deg: f64,
    pub position: Vec2,
    pub remaining_ticks: f64,
    pub speed: f64,
    pub turn_rate: f64,
    pub visual_scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeGuidedMissileStep {
    pub state: NativeGuidedMissileState,
    pub terrain_probe: Vec2,
}

#[must_use]
pub fn create_native_guided_missile(
    rng: &mut NativeRng,
    position: Vec2,
    heading_deg: f64,
    speed_scale: f64,
) -> NativeGuidedMissileState {
    let phase_deg = rng.draw_scaled_float(360.0, false);
    let turn = rng.draw_scaled_float(0.75, false);
    let floor = rng.draw_scaled_float(0.449_999_988_079_071_04, false);
    let scale = rng.draw_scaled_float(0.200_000_047_683_715_82, false);
    NativeGuidedMissileState {
        heading_deg,
        minimum_speed: fround(fround(0.75 + floor) * speed_scale),
        phase_deg,
        position,
        remaining_ticks: 2000.0,
        speed: fround(3.0 * speed_scale),
        turn_rate: fround(0.5 + turn),
        visual_scale: fround(0.899_999_976_158_142_1 + scale),
    }
}

#[must_use]
pub fn step_native_guided_missile(
    source: NativeGuidedMissileState,
    target: Option<Vec2>,
    actor_time_scale: f64,
) -> NativeGuidedMissileStep {
    let radians = source.heading_deg * std::f64::consts::PI / 180.0;
    let delta = Vec2::new(
        fround(fround(float::sin(radians)) * source.speed * actor_time_scale),
        fround(fround(-float::cos(radians)) * source.speed * actor_time_scale),
    );
    let position =
        Vec2::new(fround(source.position.x + delta.x), fround(source.position.y + delta.y));
    let mut heading_deg = source.heading_deg;
    if let Some(target) = target
        && actor_time_scale != 0.0
    {
        let aim = heading_degrees_from_vector(target.x - position.x, target.y - position.y);
        let turn = f64::from(native_heading_turn_direction(source.heading_deg, aim));
        heading_deg = fround(source.heading_deg + source.turn_rate * actor_time_scale * turn);
    }
    NativeGuidedMissileStep {
        state: NativeGuidedMissileState {
            heading_deg,
            position,
            phase_deg: fround(
                source.phase_deg + fround(source.speed * actor_time_scale * 3.0) * 2.0,
            ),
            remaining_ticks: fround(source.remaining_ticks - actor_time_scale),
            speed: source
                .minimum_speed
                .max(fround(source.speed - actor_time_scale * 0.075_000_002_980_232_24)),
            ..source
        },
        terrain_probe: Vec2::new(
            fround(position.x + delta.x * 5.0),
            fround(position.y + delta.y * 5.0),
        ),
    }
}
