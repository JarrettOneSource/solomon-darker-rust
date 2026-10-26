//! Cold Aura birth and Water Hail bounce epochs from `air-water-spell-actors.ts`.
use crate::float;
use crate::float::fround;
use crate::spells::model::{
    PrimarySpellTransientBase, PrimarySpellWaterAura, PrimarySpellWaterHail,
};
use crate::{NativeRng, Vec2};

pub const NATIVE_HAIL_LIFETIME_TICKS: u64 = 134;

/// Retained snapshot Hail omits its life; `nativeWaterHailLifeAtAge` reconstructs
/// the native single-precision losses before endpoint interpolation.
#[must_use]
pub fn native_water_hail_life_at_age(age_ticks: u64) -> f64 {
    assert!(age_ticks < NATIVE_HAIL_LIFETIME_TICKS, "Hail age is within its native lifecycle");
    let mut life = 2.0;
    for _ in 0..age_ticks {
        life = fround(life - 0.015_f32 as f64);
    }
    life
}

pub fn create_native_water_aura_actor(
    common: PrimarySpellTransientBase,
    birth_tick: u64,
    origin: Vec2,
    radius_scale: f64,
    rng: &mut NativeRng,
) -> PrimarySpellWaterAura {
    let alpha_decay = crate::cold_aura::native_cold_aura_alpha_decay(radius_scale);
    let rotation_step_degrees = rng.draw_scaled_float(1.0, true);
    let initial_rotation_degrees = rng.draw_scaled_float(360.0, false);
    PrimarySpellWaterAura {
        common,
        alpha_decay,
        birth_tick,
        duration_ticks: crate::cold_aura::native_cold_aura_lifetime_ticks(alpha_decay),
        initial_rotation_degrees,
        origin,
        rotation_step_degrees,
    }
}

pub fn create_native_water_hail_actor(
    common: PrimarySpellTransientBase,
    birth_tick: u64,
    emitter: Vec2,
    direction: Vec2,
    rng: &mut NativeRng,
) -> PrimarySpellWaterHail {
    let bounce = rng.draw_scaled_float(3.0, false);
    let height = rng.draw_scaled_float(20.0, false);
    let rotation_degrees = rng.draw_scaled_float(360.0, false);
    let rotation_step = rng.draw_scaled_float(10.0, false);
    let scale = rng.draw_scaled_float(0.1, true);
    let distance = rng.draw_scaled_float(15.0, false);
    let heading = rng.draw_integer(100_001);
    let speed = rng.draw_scaled_float(2.0, false);
    let degrees = fround(fround(f64::from(heading) / 100_000.0) * 360.0);
    // The native placement uses GMath's authored float PI, not the host's double PI.
    let radians = fround(degrees * 3.141592502593994 / 180.0);
    let radial = Vec2::new(
        fround(fround(float::sin(radians)) * distance),
        fround(-fround(float::cos(radians)) * distance),
    );
    let horizontal_speed = fround(4.0 + speed);
    PrimarySpellWaterHail {
        common,
        birth_tick,
        bounce_progress: 0.0,
        bounce_sound_index: None,
        bounce_sound_pitch: None,
        bounce_sound_sequence: 0,
        height: fround(-height),
        horizontal_velocity: Vec2::new(
            fround(direction.x * horizontal_speed),
            fround(direction.y * horizontal_speed),
        ),
        life: 2.0,
        position: Vec2::new(fround(emitter.x + radial.x), fround(emitter.y + radial.y)),
        rotation_degrees,
        rotation_step_degrees: fround(1.0 + rotation_step),
        saved_bounce_velocity: fround(-(2.0 + bounce)),
        scale: fround(0.5 + scale),
        vertical_velocity: 0.0,
    }
}

pub fn step_native_water_hail_actor(
    actor: &mut PrimarySpellWaterHail,
    rng: &mut NativeRng,
) -> bool {
    let life = fround(actor.life - 0.015_f32 as f64);
    if actor.height == 0.0 {
        actor.life = life;
        actor.common.age_ticks += 1;
        return life > 0.0;
    }
    let bounce_progress = fround(actor.bounce_progress + 0.02_f32 as f64).min(1.0);
    let acceleration = actor.bounce_progress * 0.4_f32 as f64;
    let mut vertical = fround(fround(actor.vertical_velocity + acceleration) + acceleration);
    let mut height =
        fround(fround(actor.height + actor.vertical_velocity) + actor.vertical_velocity);
    // The source moves with the incoming velocity, even on a bounce that damps it.
    let incoming_velocity = actor.horizontal_velocity;
    if height > 0.0 {
        actor.rotation_step_degrees = fround(1.0 + rng.draw_scaled_float(10.0, false));
        actor.saved_bounce_velocity = fround(actor.saved_bounce_velocity * 0.65_f32 as f64);
        vertical = actor.saved_bounce_velocity;
        if rng.draw_integer(3) == 1 {
            let pitch = rng.draw_scaled_float(0.2_f32 as f64, true);
            actor.bounce_sound_index = Some(rng.draw_integer(4) as u32);
            actor.bounce_sound_pitch = Some(fround(1.0 + pitch));
            actor.bounce_sound_sequence += 1;
        }
        if rng.draw_integer(2) == 1 {
            actor.horizontal_velocity = Vec2::new(
                fround(actor.horizontal_velocity.x * 0.65_f32 as f64),
                fround(actor.horizontal_velocity.y * 0.65_f32 as f64),
            );
        }
        if vertical > -0.75 {
            actor.horizontal_velocity = Vec2::ZERO;
            actor.rotation_step_degrees = 0.0;
            actor.saved_bounce_velocity = 0.0;
            vertical = 0.0;
        }
        height = vertical;
    }
    if life <= 0.0 {
        return false;
    }
    actor.common.age_ticks += 1;
    actor.bounce_progress = bounce_progress;
    actor.height = height;
    actor.life = life;
    actor.position = Vec2::new(
        fround(actor.position.x + incoming_velocity.x),
        fround(actor.position.y + incoming_velocity.y),
    );
    actor.rotation_degrees = fround(actor.rotation_degrees + actor.rotation_step_degrees);
    actor.vertical_velocity = vertical;
    true
}
