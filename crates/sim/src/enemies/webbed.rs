//! Native web status and Cocoon geometry from `core-kernels/native-webbed.ts`.
use super::model::WebbedState;
use crate::Vec2;
use crate::float::fround;
use crate::spells::targeting::direction_from_heading;

#[must_use]
pub fn apply_native_webbed(source: Option<WebbedState>, strength: f64) -> WebbedState {
    let prior = source.map_or(0.0, |source| source.severity);
    let severity = fround(prior + 1.0).floor().min(3.0);
    let strength = source.map_or(0.0, |source| source.strength).max(strength);
    WebbedState {
        severity,
        strength,
        cocoon_health: if prior < 3.0 && severity == 3.0 {
            strength
        } else {
            source.map_or(0.0, |source| source.cocoon_health)
        },
        hit_pulse: source.map_or(0.0, |source| source.hit_pulse),
    }
}

#[must_use]
pub fn step_native_webbed(mut source: WebbedState, velocity: Vec2) -> Option<WebbedState> {
    if source.severity < 3.0 && velocity.x * velocity.x + velocity.y * velocity.y > 0.5 {
        source.severity = fround(source.severity - fround(0.001));
    }
    if source.severity <= 0.0 {
        return None;
    }
    source.hit_pulse = fround(source.hit_pulse - fround(0.05)).max(0.0);
    Some(source)
}

#[must_use]
pub fn native_webbed_movement_scale(source: Option<&WebbedState>) -> f64 {
    source.map_or(1.0, |source| (1.0 - source.severity / (2.9_f32 as f64)).max(0.0))
}

#[must_use]
pub fn damage_native_cocoon(
    mut source: WebbedState,
    damage: f64,
    hit_pulse: f64,
) -> Option<WebbedState> {
    source.cocoon_health -= damage;
    source.hit_pulse = hit_pulse;
    if source.cocoon_health <= 0.0 { None } else { Some(source) }
}

#[must_use]
pub fn native_cocoon_position(position: Vec2, heading: f64) -> Vec2 {
    let direction = direction_from_heading(heading);
    Vec2::new(
        fround(position.x + direction.x * 60.0 + fround(0.1)),
        fround(position.y + direction.y * 60.0 + fround(0.1)),
    )
}

#[must_use]
pub fn native_cocoon_tilt(heading: f64) -> Vec2 {
    let direction = direction_from_heading((heading / 30.0).trunc() * 30.0);
    let x = fround(direction.x * -5.0);
    let y = fround(direction.y * -5.0);
    Vec2::new(x, fround(if y <= -3.0 { y * 0.5 } else { y + (y * 4.0).abs() }))
}
