//! Light sources for fire and ether primary spells and for boss spells. Light providers
//! receive the authoritative actor values; presentation draws never advance combat randomness.

use super::BoneyardLightSource;
use crate::enemies::boss_spell::{BossSpell, BossSpellKind};
use crate::float::fround;
use crate::spells::fire::{
    fire_presentation_random, fire_presentation_random_int, fire_presentation_signed_random,
};
use crate::{Rng, Vec2};

pub const FIRE_EXPLOSION_LIT_ARRAY_VISIBLE_TICKS: u64 = 37;
pub const ETHER_PRIMARY_IMPACT_LIGHT_RADIUS: f64 = 0.75;

#[must_use]
pub fn fire_explosion_light_source(
    origin: Vec2,
    age_ticks: u64,
    point_gain: f64,
    multiple_shadows: bool,
) -> Option<BoneyardLightSource> {
    if age_ticks >= FIRE_EXPLOSION_LIT_ARRAY_VISIBLE_TICKS {
        return None;
    }
    Some(BoneyardLightSource::new(origin, fround(2.0 * point_gain.max(0.0)), 1.0, multiple_shadows))
}

#[must_use]
pub fn fire_ember_light_source(
    id: u64,
    position: Vec2,
    life: f64,
    presentation_sample: u64,
) -> BoneyardLightSource {
    let radius = fround(1.0 - fire_presentation_random(id, presentation_sample, 39, 0.25));
    BoneyardLightSource::new(position, radius, fround(life.min(1.0) * 0.25), false)
}

#[must_use]
pub fn fire_impact_light_source(origin: Vec2, age: f64) -> BoneyardLightSource {
    BoneyardLightSource::new(
        Vec2::new(origin.x, origin.y - 10.0 - age),
        1.5,
        (1.0 - age * 0.04).max(0.0),
        false,
    )
}

#[must_use]
pub fn fire_good_imp_light_source(
    id: u64,
    position: Vec2,
    light_glow: f64,
    presentation_frame: u64,
) -> BoneyardLightSource {
    let magnitude = fire_presentation_random(id, presentation_frame, 21, 0.1_f32 as f64);
    let sign =
        if fire_presentation_random_int(id, presentation_frame, 22, 2) == 1 { -1.0 } else { 1.0 };
    let intensity = fround(
        fround(light_glow)
            * fround(0.75 + fire_presentation_random(id, presentation_frame, 20, 0.25_f32 as f64)),
    );
    BoneyardLightSource::new(position, fround(0.25 + sign * magnitude), intensity, false)
}

#[must_use]
pub fn fireball_light_source(
    id: u64,
    position: Vec2,
    presentation_frame: u64,
    multiple_shadows: bool,
) -> BoneyardLightSource {
    let radius = fround(1.0 + fire_presentation_signed_random(id, presentation_frame, 8, 0.25));
    BoneyardLightSource::new(position, radius, 0.75, multiple_shadows)
}

#[must_use]
pub fn ether_primary_impact_light_source(origin: Vec2, age_ticks: u64) -> BoneyardLightSource {
    let mut intensity = fround(1.0);
    // Birth itself performs one decrement; multiplication would change float32 drift.
    for _ in 0..=age_ticks {
        intensity = fround(intensity + (-0.05_f32 as f64));
    }
    BoneyardLightSource::new(origin, ETHER_PRIMARY_IMPACT_LIGHT_RADIUS, intensity.min(1.0), false)
}

/// The light a boss spell gives off, if any. Its private presentation generator restarts
/// for each spell and frame, seeded by the owner's ID plus the frame; it never consumes
/// combat RNG.
#[must_use]
pub fn boss_spell_light(
    spell: &BossSpell,
    presentation_frame: u64,
    multiple_shadows: bool,
) -> Option<BoneyardLightSource> {
    let mut rng = Rng::new(spell.owner.id as i64 + presentation_frame as i64);
    let position = spell.owner.position;
    let source = match &spell.kind {
        BossSpellKind::UltraBanish { alpha, light_radius, .. } => {
            BoneyardLightSource::new(position, *light_radius, *alpha, multiple_shadows)
        }
        BossSpellKind::UnholySoul { .. } => return None,
        BossSpellKind::MouthBeamSegment { .. } | BossSpellKind::EyeLaser { .. } => {
            BoneyardLightSource::new(position, 1.0, 1.0, multiple_shadows)
        }
        BossSpellKind::UnholyBurst { light_intensity, .. } => {
            BoneyardLightSource::new(position, 3.0, light_intensity.min(1.0), multiple_shadows)
        }
        BossSpellKind::UnholySpit { .. } => {
            BoneyardLightSource::new(position, 0.5, 1.0, multiple_shadows)
        }
        BossSpellKind::GreenFire { fire, glow } => {
            if !glow {
                return None;
            }
            BoneyardLightSource::new(
                position,
                0.6_f32 as f64,
                (fire.life * 3.0).min(1.0),
                multiple_shadows,
            )
        }
        BossSpellKind::HeartmongerFlicker { .. }
        | BossSpellKind::Blightning { .. }
        | BossSpellKind::FallingBone { .. } => return None,
        BossSpellKind::HeartmongerSoul { .. } => BoneyardLightSource::new(
            position,
            1.0,
            fround(1.0 - spell.owner.age_ticks as f64 * 0.04_f32 as f64).max(0.0),
            multiple_shadows,
        ),
        BossSpellKind::DeathMagic { light, .. } => {
            let light = light.as_ref()?;
            BoneyardLightSource::new(position, light.radius, light.intensity, false)
        }
        BossSpellKind::SkullMissile { .. } => BoneyardLightSource::new(
            position,
            fround(0.75 + rng.draw_scaled_float(0.1_f32 as f64, true)),
            0.75,
            multiple_shadows,
        ),
        BossSpellKind::DarkFireball { .. } => BoneyardLightSource::new(
            position,
            fround(0.5 + rng.draw_scaled_float(0.25, true)),
            0.85_f32 as f64,
            false,
        ),
        BossSpellKind::RainOfBones { alpha, .. } => BoneyardLightSource::new(
            position,
            2.0,
            rng.draw_scaled_float(fround(alpha * 0.5), false),
            false,
        ),
        BossSpellKind::TragicCircle { .. } => BoneyardLightSource::new(
            position,
            2.0,
            fround(0.75 + rng.draw_scaled_float(0.25, true)),
            multiple_shadows,
        ),
        BossSpellKind::DireFire(fire) => BoneyardLightSource::new(
            position,
            0.6_f32 as f64,
            (fire.alpha * 3.0).min(1.0),
            multiple_shadows,
        ),
    };
    Some(source)
}
