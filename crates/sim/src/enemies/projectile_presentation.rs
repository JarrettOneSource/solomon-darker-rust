//! Effect sprites from `renderer/native-enemy-projectile-effect-presentation.ts`.
//! Primary Fire owns the shared explosion passes, patch transform and ground glow.

use super::actor_presentation::{EnemyArtAtlas, EnemyArtBlend, EnemySpriteLayer};
use super::model::{EnemyEffectAtlas, EnemyEffectBlend};
use super::projectiles::{EnemyProjectileEffect, EnemyProjectileEffectKind};
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::spells::fire::native_fire_presentation_random;
use crate::spells::fire_presentation::{
    NativeFireGroundGlowPlan, native_fire_explosion_plan, native_fire_ground_glow_plan,
    native_fire_patch_transform,
};
use crate::spells::model::FireExplosionPresentation;
use crate::spells::presentation::{SpellAtlas, SpellBlend, SpellDrawQueue};

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyProjectileEffectPlan {
    pub ground_glow: Option<NativeFireGroundGlowPlan>,
    pub layers: Vec<EnemySpriteLayer>,
    pub position: Vec2,
}

#[must_use]
pub fn native_enemy_projectile_effect_plan(
    effect: &EnemyProjectileEffect,
    point_gain: f64,
) -> EnemyProjectileEffectPlan {
    native_enemy_projectile_effect_plan_with_input(
        effect,
        point_gain,
        super::presentation::NativeEnemyEffectPresentationInput {
            age_ticks: effect.state.age_ticks as f64,
            rotation_radians: effect.state.rotation_deg * std::f64::consts::PI / 180.0,
        },
    )
}

#[must_use]
pub fn native_enemy_projectile_effect_plan_with_input(
    effect: &EnemyProjectileEffect,
    point_gain: f64,
    sample: super::presentation::NativeEnemyEffectPresentationInput,
) -> EnemyProjectileEffectPlan {
    let mut plan = effect_plan(effect, point_gain, sample);
    for layer in &mut plan.layers {
        if layer.scale_y.is_none() {
            layer.scale_y = Some(layer.scale);
        }
    }
    plan
}

fn effect_plan(
    effect: &EnemyProjectileEffect,
    point_gain: f64,
    sample: super::presentation::NativeEnemyEffectPresentationInput,
) -> EnemyProjectileEffectPlan {
    let state = &effect.state;
    let mut plan = EnemyProjectileEffectPlan {
        ground_glow: None,
        layers: Vec::new(),
        position: state.position,
    };
    let explosion_queue = match effect.kind {
        EnemyProjectileEffectKind::DemonExplosionCore => Some(SpellDrawQueue::PostWorld),
        EnemyProjectileEffectKind::DemonExplosionArray => Some(SpellDrawQueue::PreWorld),
        EnemyProjectileEffectKind::DemonExplosionLitArray => Some(SpellDrawQueue::World),
        _ => None,
    };
    if let Some(queue) = explosion_queue {
        let explosion = native_fire_explosion_plan(
            sample.age_ticks,
            state.position,
            FireExplosionPresentation::Fire,
            state.scale,
            point_gain,
        );
        for draw in explosion.draws {
            if draw.queue != queue {
                continue;
            }
            let role = match queue {
                SpellDrawQueue::PostWorld => "explosion-core",
                SpellDrawQueue::PreWorld => "explosion-array",
                SpellDrawQueue::World => "explosion-lit-array",
            };
            plan.layers.push(EnemySpriteLayer {
                alpha: draw.alpha,
                blend: match draw.blend {
                    SpellBlend::Normal => EnemyArtBlend::Normal,
                    SpellBlend::Add => EnemyArtBlend::Add,
                },
                offset: draw.offset,
                rotation_radians: draw.rotation,
                scale: draw.scale.x,
                tint: draw.tint,
                ..EnemySpriteLayer::new(
                    match draw.atlas {
                        SpellAtlas::BadGuys => EnemyArtAtlas::BadGuys,
                        SpellAtlas::DeadHawg => EnemyArtAtlas::DeadHawg,
                    },
                    draw.record,
                    role,
                )
            });
        }
        return plan;
    }
    if effect.kind == EnemyProjectileEffectKind::GuidedImpact {
        plan.layers = guided_impact_layers(effect, sample);
        return plan;
    }
    if effect.kind == EnemyProjectileEffectKind::FireBurst {
        plan.layers.push(EnemySpriteLayer {
            alpha: state.alpha,
            scale: state.scale * 5.0,
            tint: 0xff8000,
            ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 110, "fire-burst-glow")
        });
        plan.layers.push(EnemySpriteLayer {
            blend: EnemyArtBlend::Add,
            rotation_radians: sample.rotation_radians,
            scale: state.scale,
            tint: 0xffffbf,
            ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, state.entry, "fire-burst-frame")
        });
        return plan;
    }
    let mut sprite = EnemySpriteLayer {
        alpha: state.alpha,
        blend: effect_blend(state.blend_mode),
        rotation_radians: sample.rotation_radians,
        scale: state.scale,
        tint: state.tint,
        ..EnemySpriteLayer::new(effect_atlas(state.atlas), state.entry, effect.kind.id())
    };
    if let EnemyProjectileEffectKind::DemonFire { fire } = &effect.kind {
        sprite.entry = 46 + fire.atlas_phase.round_ties_even().rem_euclid(32.0) as usize;
        let transform = native_fire_patch_transform(
            state.position,
            state.scale,
            fire.fade_alpha,
            fire.horizontal_sign,
        );
        plan.position = transform.position;
        sprite.scale = transform.scale_x;
        sprite.scale_y = Some(transform.scale_y);
        plan.ground_glow = Some(native_fire_ground_glow_plan(
            state.id as u64,
            state.alpha,
            state.position,
            state.scale,
            (state.spawn_tick as f64 + sample.age_ticks).floor() as u64,
        ));
    }
    plan.layers.push(sprite);
    plan
}

fn guided_impact_layers(
    effect: &EnemyProjectileEffect,
    sample: super::presentation::NativeEnemyEffectPresentationInput,
) -> Vec<EnemySpriteLayer> {
    let state = &effect.state;
    let rotation = sample.rotation_radians;
    // The source converts its stored radians back to degrees before the pulse.
    let phase = rotation * 180.0 / std::f64::consts::PI;
    let tick = (state.spawn_tick as f64 + sample.age_ticks).floor() as u64;
    let random =
        |channel, maximum| native_fire_presentation_random(state.id as u64, tick, channel, maximum);
    let sine = |degrees: f64| float::sin(degrees * std::f64::consts::PI / 180.0);
    let main = EnemySpriteLayer {
        alpha: state.alpha * (0.5 + random(40, 0.5)),
        blend: EnemyArtBlend::Add,
        scale: fround(1.1 + sine(phase * 15.0).abs() * 0.15 * state.scale),
        ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, state.entry, "guided-impact-main")
    };
    let mut layers = vec![main.clone(), main];
    for index in 1..=2 {
        layers.push(EnemySpriteLayer {
            alpha: state.alpha * sine(phase * f64::from(index) * 3.0).abs() * 0.55,
            blend: EnemyArtBlend::Add,
            rotation_radians: rotation * 0.5,
            scale: fround((1.0 + random(40 + index, 0.3)) * state.scale),
            tint: state.tint,
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::BadGuys,
                110 + index as usize,
                format!("guided-impact-aura-{index}"),
            )
        });
    }
    layers
}

#[must_use]
pub const fn enemy_projectile_effect_bypasses_world_tint(kind: &EnemyProjectileEffectKind) -> bool {
    matches!(
        kind,
        EnemyProjectileEffectKind::FireBurst
            | EnemyProjectileEffectKind::DemonBombParticle
            | EnemyProjectileEffectKind::GuidedImpact
            | EnemyProjectileEffectKind::DemonFire { .. }
            | EnemyProjectileEffectKind::DemonExplosionCore
            | EnemyProjectileEffectKind::DemonExplosionArray
            | EnemyProjectileEffectKind::DemonExplosionLitArray
    )
}

pub(super) const fn effect_atlas(atlas: EnemyEffectAtlas) -> EnemyArtAtlas {
    match atlas {
        EnemyEffectAtlas::BadGuys => EnemyArtAtlas::BadGuys,
        EnemyEffectAtlas::DeadHawg => EnemyArtAtlas::DeadHawg,
        EnemyEffectAtlas::Demon => EnemyArtAtlas::Demon,
        EnemyEffectAtlas::Unholy => EnemyArtAtlas::Unholy,
    }
}

pub(super) const fn effect_blend(blend: EnemyEffectBlend) -> EnemyArtBlend {
    match blend {
        EnemyEffectBlend::Normal => EnemyArtBlend::Normal,
        EnemyEffectBlend::Add => EnemyArtBlend::Add,
    }
}
