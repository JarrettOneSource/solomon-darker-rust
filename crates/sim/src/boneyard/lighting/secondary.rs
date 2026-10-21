//! Secondary actor providers and MiscLight from `native-boneyard-light-model.ts`.

use crate::float::fround;
use crate::random_domain::native_random_float_from_word;
use crate::secondary::{NativeSecondaryActorKind, NativeSecondaryActorState};

use super::{
    NativeBoneyardLightSource, native_fire_patch_light_source, presentation_random,
    presentation_random_word,
};

#[must_use]
pub fn native_secondary_provider_light_source(
    actor: &NativeSecondaryActorState,
    frame: u64,
    multiple_shadows: bool,
    point_gain: f64,
) -> Option<NativeBoneyardLightSource> {
    native_secondary_provider_light_source_at_age(
        actor,
        actor.age_ticks as f64,
        frame,
        multiple_shadows,
        point_gain,
    )
}

/// The display sampler supplies fractional age without changing logical light
/// queries. Ether Fade retains the source's explicit floor before decay steps.
#[must_use]
pub fn native_secondary_provider_light_source_at_age(
    actor: &NativeSecondaryActorState,
    age_ticks: f64,
    frame: u64,
    multiple_shadows: bool,
    point_gain: f64,
) -> Option<NativeBoneyardLightSource> {
    use NativeSecondaryActorKind::*;
    let (radius, intensity, casts_shadow) = match actor.kind {
        MovingFire | FirePatch => {
            if actor.radius <= 0.0 {
                return None;
            }
            return Some(native_fire_patch_light_source(
                actor.position,
                actor.radius,
                multiple_shadows,
            ));
        }
        RingFireExplosion => (fround(2.0 * point_gain.max(0.0)), 1.0, multiple_shadows),
        ElectricBurnFlare => {
            let mut intensity = fround(1.0);
            for _ in 0..age_ticks.floor().max(0.0) as u64 {
                intensity = fround(intensity - fround(0.05));
            }
            (actor.radius, intensity.max(0.0), false)
        }
        RingFireFragment => (
            fround(
                1.0 - native_random_float_from_word(
                    presentation_random_word(actor.id, frame as u32 ^ 0x6ac6_90c5),
                    0.25,
                ),
            ),
            fround(actor.alpha.min(1.0) * 0.25),
            false,
        ),
        Shockwave | MindblastShockwave | RescueShockwave | FreezeWave => {
            (actor.radius / 140.0, actor.alpha, false)
        }
        StormCloud => (2.0, actor.alpha * 0.5, false),
        AcidRain => {
            if actor.phase <= 0.0 {
                return None;
            }
            (2.0, actor.phase * 0.5, false)
        }
        Leviathan => (1.0, 1.0, multiple_shadows),
        EtherBolt => (0.5, 1.0, multiple_shadows),
        Golem => (1.0, 0.75, multiple_shadows),
        MagicTrap => (0.25, 1.0, false),
        EtherDrain => (
            2.0,
            actor.scale.min(1.0)
                * (0.5
                    + presentation_random(actor.id.wrapping_mul(131).wrapping_add(frame), 0, 1.0)
                        * 0.5),
            multiple_shadows,
        ),
        EtherFade if actor.variant == 1 => {
            let mut intensity = fround(actor.alpha);
            for _ in 0..=age_ticks.floor().max(0.0) as u64 {
                intensity = fround(intensity - fround(actor.slow_factor));
            }
            (actor.scale, intensity.clamp(0.0, 1.0), multiple_shadows)
        }
        Comet => (2.0, 0.5, multiple_shadows),
        _ => return None,
    };
    Some(NativeBoneyardLightSource::new(actor.position, radius, intensity, casts_shadow))
}

#[must_use]
pub fn native_secondary_misc_light_source(
    actor: &NativeSecondaryActorState,
) -> Option<NativeBoneyardLightSource> {
    native_secondary_misc_light_source_at_age(actor, actor.age_ticks as f64)
}

/// Burn lights become present after age zero, including a fractional display
/// sample before the first complete tick. The source does not floor this test.
#[must_use]
pub fn native_secondary_misc_light_source_at_age(
    actor: &NativeSecondaryActorState,
    age_ticks: f64,
) -> Option<NativeBoneyardLightSource> {
    use NativeSecondaryActorKind::*;
    match actor.kind {
        MagicCircle if age_ticks < 1500.0 => Some(NativeBoneyardLightSource::new(
            actor.position,
            actor.scale * 0.5,
            actor.alpha,
            true,
        )),
        FireBurn | EtherBurn | ElectricBurn if age_ticks > 0.0 => {
            Some(NativeBoneyardLightSource::new(actor.position, actor.radius, actor.alpha, false))
        }
        _ => None,
    }
}
