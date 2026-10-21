//! Imp landing/contact and Demon muzzle flashes from `renderer/native-enemy-attack-effect.ts`.
//! Their visual samples are private to the retained view and never consume world entropy.

use super::actor_presentation::{EnemyArtAtlas, EnemyArtBlend, EnemySpriteLayer};
use crate::Vec2;
use crate::float;
use crate::float::fround;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyRawFireBurstKind {
    DemonBombMuzzle,
    ImpContact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyAttackEffectKind {
    FireBurst(EnemyRawFireBurstKind),
    ImpLandingFlare,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnemyRawFireBurstSample {
    pub frame_entry: usize,
    pub frame_rotation_radians: f64,
    pub glow_alpha: f64,
    pub scale: f64,
    pub vertical_offset: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeImpLandingFlareSample {
    pub alpha: f64,
    pub green: f64,
    pub scale_x: f64,
    pub scale_y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyAttackEffectPlan {
    pub offset_y: f64,
    pub layers: Vec<EnemySpriteLayer>,
}

#[must_use]
pub fn native_imp_contact_burst_origin(position: Vec2, heading_deg: f64) -> Vec2 {
    let radians = heading_deg * std::f64::consts::PI / 180.0;
    Vec2::new(
        position.x + float::sin(radians) * 15.0,
        position.y - 15.0 - float::cos(radians) * 15.0,
    )
}

#[must_use]
pub fn native_demon_bomb_bearing(heading_deg: f64) -> f64 {
    ((heading_deg + 10.0) / 20.0).round_ties_even().rem_euclid(18.0) * 20.0
}

#[must_use]
pub fn native_demon_bomb_muzzle_origin(
    position: Vec2,
    heading_deg: f64,
    controller_point_5: Vec2,
    vertical_offset: f64,
) -> Vec2 {
    let radians = native_demon_bomb_bearing(heading_deg) * std::f64::consts::PI / 180.0;
    Vec2::new(
        position.x + controller_point_5.x + float::sin(radians) * 25.0,
        position.y + vertical_offset + controller_point_5.y - float::cos(radians) * 25.0,
    )
}

#[must_use]
pub fn native_enemy_raw_fire_burst_sample(
    kind: EnemyRawFireBurstKind,
    event_id: usize,
    age_ticks: u64,
) -> Option<NativeEnemyRawFireBurstSample> {
    native_enemy_raw_fire_burst_sample_at_age(kind, event_id, age_ticks as f64)
}

#[must_use]
pub fn native_enemy_raw_fire_burst_sample_at_age(
    kind: EnemyRawFireBurstKind,
    event_id: usize,
    age_ticks: f64,
) -> Option<NativeEnemyRawFireBurstSample> {
    assert!(event_id > 0, "enemy raw fire burst requires a positive event id");
    assert!(
        age_ticks.is_finite() && age_ticks >= 0.0,
        "enemy raw fire burst age is finite and non-negative"
    );
    let phase_per_tick: f64 = match kind {
        EnemyRawFireBurstKind::DemonBombMuzzle => 0.25 * 0.75,
        EnemyRawFireBurstKind::ImpContact => 0.25,
    };
    let lifetime = (4.0 / phase_per_tick).ceil();
    if age_ticks >= lifetime {
        return None;
    }
    let phase = age_ticks * phase_per_tick;
    let rotation = deterministic_unit(event_id, 1) * std::f64::consts::PI * 2.0;
    let magnitude = 0.5 + deterministic_unit(event_id, 2);
    let direction = if deterministic_unit(event_id, 3) < 0.5 { -1.0 } else { 1.0 };
    let scale = match kind {
        EnemyRawFireBurstKind::ImpContact => 0.5 + deterministic_unit(event_id, 4) * 0.1,
        EnemyRawFireBurstKind::DemonBombMuzzle => 1.0,
    };
    Some(NativeEnemyRawFireBurstSample {
        frame_entry: 251 + (phase.floor() as usize).min(3),
        frame_rotation_radians: rotation
            + direction * magnitude * std::f64::consts::PI / 180.0 * age_ticks,
        glow_alpha: 0.5 * (1.0 - phase / 4.0),
        scale,
        vertical_offset: -age_ticks,
    })
}

#[must_use]
pub fn native_imp_landing_flare_sample(
    event_id: usize,
    age_ticks: u64,
) -> Option<NativeImpLandingFlareSample> {
    native_imp_landing_flare_sample_at_age(event_id, age_ticks as f64)
}

#[must_use]
pub fn native_imp_landing_flare_sample_at_age(
    event_id: usize,
    age_ticks: f64,
) -> Option<NativeImpLandingFlareSample> {
    assert!(event_id > 0, "Imp landing flare requires a positive event id");
    assert!(
        age_ticks.is_finite() && age_ticks >= 0.0,
        "Imp landing flare age is finite and non-negative"
    );
    if age_ticks >= 13.0 {
        return None;
    }
    let loss = fround(fround(0.1) * f64::from(0.8_f32));
    let scale_x = 0.5 + deterministic_unit(event_id, 5) * 0.6;
    Some(NativeImpLandingFlareSample {
        alpha: (1.0 - age_ticks * loss).max(0.0),
        green: 0.25 + deterministic_unit(event_id, 6) * 0.5,
        scale_x,
        scale_y: scale_x * 0.8,
    })
}

#[must_use]
pub fn native_enemy_attack_effect_plan(
    kind: EnemyAttackEffectKind,
    event_id: usize,
    age_ticks: u64,
) -> Option<EnemyAttackEffectPlan> {
    native_enemy_attack_effect_plan_at_age(kind, event_id, age_ticks as f64)
}

#[must_use]
pub fn native_enemy_attack_effect_plan_at_age(
    kind: EnemyAttackEffectKind,
    event_id: usize,
    age_ticks: f64,
) -> Option<EnemyAttackEffectPlan> {
    match kind {
        EnemyAttackEffectKind::FireBurst(kind) => {
            let sample = native_enemy_raw_fire_burst_sample_at_age(kind, event_id, age_ticks)?;
            Some(EnemyAttackEffectPlan {
                offset_y: sample.vertical_offset,
                layers: vec![
                    EnemySpriteLayer {
                        alpha: sample.glow_alpha,
                        scale: sample.scale * 5.0,
                        tint: 0xff8000,
                        ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 110, "raw-fire-burst-glow")
                    },
                    EnemySpriteLayer {
                        blend: EnemyArtBlend::Add,
                        rotation_radians: sample.frame_rotation_radians,
                        scale: sample.scale,
                        tint: 0xffffbf,
                        ..EnemySpriteLayer::new(
                            EnemyArtAtlas::BadGuys,
                            sample.frame_entry,
                            "raw-fire-burst-frame",
                        )
                    },
                ],
            })
        }
        EnemyAttackEffectKind::ImpLandingFlare => {
            let sample = native_imp_landing_flare_sample_at_age(event_id, age_ticks)?;
            Some(EnemyAttackEffectPlan {
                offset_y: 0.0,
                layers: vec![EnemySpriteLayer {
                    alpha: sample.alpha,
                    blend: EnemyArtBlend::Add,
                    scale: sample.scale_x,
                    scale_y: Some(sample.scale_y),
                    tint: 0xff0000 | ((sample.green * 255.0).round() as u32) << 8,
                    ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 15, "imp-landing-flare")
                }],
            })
        }
    }
}

fn deterministic_unit(id: usize, channel: u32) -> f64 {
    let mut value = id as u32 ^ channel.wrapping_add(1).wrapping_mul(0x9e3779b1);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    f64::from(value) / 4_294_967_296.0
}
