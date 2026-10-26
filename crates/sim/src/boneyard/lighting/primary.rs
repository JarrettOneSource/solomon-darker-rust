//! `core-kernels/native-primary-light-sources.ts`. Light providers receive the
//! authoritative actor values; presentation draws never advance combat randomness.

use super::NativeBoneyardLightSource;
use crate::enemies::boss_spell::{BossSpellKind, BossSpellRead};
use crate::float::fround;
use crate::spells::fire::{
    native_fire_presentation_random, native_fire_presentation_random_int,
    native_fire_presentation_signed_random,
};
use crate::{NativeRng, Vec2};

pub const NATIVE_FIRE_EXPLOSION_LIT_ARRAY_VISIBLE_TICKS: u64 = 37;
pub const ETHER_PRIMARY_IMPACT_LIGHT_RADIUS: f64 = 0.75;

#[must_use]
pub fn native_fire_explosion_light_source(
    origin: Vec2,
    age_ticks: u64,
    point_gain: f64,
    multiple_shadows: bool,
) -> Option<NativeBoneyardLightSource> {
    if age_ticks >= NATIVE_FIRE_EXPLOSION_LIT_ARRAY_VISIBLE_TICKS {
        return None;
    }
    Some(NativeBoneyardLightSource::new(
        origin,
        fround(2.0 * point_gain.max(0.0)),
        1.0,
        multiple_shadows,
    ))
}

#[must_use]
pub fn native_fire_ember_light_source(
    id: u64,
    position: Vec2,
    life: f64,
    presentation_sample: u64,
) -> NativeBoneyardLightSource {
    let radius = fround(1.0 - native_fire_presentation_random(id, presentation_sample, 39, 0.25));
    NativeBoneyardLightSource::new(position, radius, fround(life.min(1.0) * 0.25), false)
}

#[must_use]
pub fn native_fire_impact_light_source(origin: Vec2, age: f64) -> NativeBoneyardLightSource {
    NativeBoneyardLightSource::new(
        Vec2::new(origin.x, origin.y - 10.0 - age),
        1.5,
        (1.0 - age * 0.04).max(0.0),
        false,
    )
}

#[must_use]
pub fn native_fire_good_imp_light_source(
    id: u64,
    position: Vec2,
    light_glow: f64,
    presentation_frame: u64,
) -> NativeBoneyardLightSource {
    let magnitude = native_fire_presentation_random(id, presentation_frame, 21, 0.1_f32 as f64);
    let sign = if native_fire_presentation_random_int(id, presentation_frame, 22, 2) == 1 {
        -1.0
    } else {
        1.0
    };
    let intensity = fround(
        fround(light_glow)
            * fround(
                0.75 + native_fire_presentation_random(id, presentation_frame, 20, 0.25_f32 as f64),
            ),
    );
    NativeBoneyardLightSource::new(position, fround(0.25 + sign * magnitude), intensity, false)
}

#[must_use]
pub fn native_fireball_light_source(
    id: u64,
    position: Vec2,
    presentation_frame: u64,
    multiple_shadows: bool,
) -> NativeBoneyardLightSource {
    let radius =
        fround(1.0 + native_fire_presentation_signed_random(id, presentation_frame, 8, 0.25));
    NativeBoneyardLightSource::new(position, radius, 0.75, multiple_shadows)
}

#[must_use]
pub fn ether_primary_impact_light_source(
    origin: Vec2,
    age_ticks: u64,
) -> NativeBoneyardLightSource {
    let mut intensity = fround(1.0);
    // Birth itself performs one decrement; multiplication would change float32 drift.
    for _ in 0..=age_ticks {
        intensity = fround(intensity + (-0.05_f32 as f64));
    }
    NativeBoneyardLightSource::new(
        origin,
        ETHER_PRIMARY_IMPACT_LIGHT_RADIUS,
        intensity.min(1.0),
        false,
    )
}

/// `core-kernels/native-boss-spell-light.ts`. The source restarts this private
/// presentation generator for each spell and frame; it never consumes combat RNG.
#[must_use]
pub fn native_boss_spell_light(
    spell: &impl BossSpellRead,
    presentation_frame: u64,
    multiple_shadows: bool,
) -> Option<NativeBoneyardLightSource> {
    let spell_view = spell.view();
    let mut rng = NativeRng::new(spell_view.owner.id as i64 + presentation_frame as i64);
    let position = spell_view.owner.position;
    let source = match spell_view.kind {
        BossSpellKind::UltraBanish { alpha, light_radius, .. } => {
            NativeBoneyardLightSource::new(position, *light_radius, *alpha, multiple_shadows)
        }
        BossSpellKind::UnholySoul { .. } => return None,
        BossSpellKind::MouthBeamSegment { .. } | BossSpellKind::EyeLaser { .. } => {
            NativeBoneyardLightSource::new(position, 1.0, 1.0, multiple_shadows)
        }
        BossSpellKind::UnholyBurst { light_intensity, .. } => NativeBoneyardLightSource::new(
            position,
            3.0,
            light_intensity.min(1.0),
            multiple_shadows,
        ),
        BossSpellKind::UnholySpit { .. } => {
            NativeBoneyardLightSource::new(position, 0.5, 1.0, multiple_shadows)
        }
        BossSpellKind::GreenFire { fire, glow } => {
            if !glow {
                return None;
            }
            NativeBoneyardLightSource::new(
                position,
                0.6_f32 as f64,
                (fire.life * 3.0).min(1.0),
                multiple_shadows,
            )
        }
        BossSpellKind::HeartmongerFlicker { .. }
        | BossSpellKind::Blightning { .. }
        | BossSpellKind::FallingBone { .. } => return None,
        BossSpellKind::HeartmongerSoul { .. } => NativeBoneyardLightSource::new(
            position,
            1.0,
            fround(1.0 - spell_view.owner.age_ticks as f64 * 0.04_f32 as f64).max(0.0),
            multiple_shadows,
        ),
        BossSpellKind::DeathMagic { light, .. } => {
            let light = light.as_ref()?;
            NativeBoneyardLightSource::new(position, light.radius, light.intensity, false)
        }
        BossSpellKind::SkullMissile { .. } => NativeBoneyardLightSource::new(
            position,
            fround(0.75 + rng.draw_scaled_float(0.1_f32 as f64, true)),
            0.75,
            multiple_shadows,
        ),
        BossSpellKind::DarkFireball { .. } => NativeBoneyardLightSource::new(
            position,
            fround(0.5 + rng.draw_scaled_float(0.25, true)),
            0.85_f32 as f64,
            false,
        ),
        BossSpellKind::RainOfBones { alpha, .. } => NativeBoneyardLightSource::new(
            position,
            2.0,
            rng.draw_scaled_float(fround(alpha * 0.5), false),
            false,
        ),
        BossSpellKind::TragicCircle { .. } => NativeBoneyardLightSource::new(
            position,
            2.0,
            fround(0.75 + rng.draw_scaled_float(0.25, true)),
            multiple_shadows,
        ),
        BossSpellKind::DireFire(fire) => NativeBoneyardLightSource::new(
            position,
            0.6_f32 as f64,
            (fire.alpha * 3.0).min(1.0),
            multiple_shadows,
        ),
    };
    Some(source)
}
