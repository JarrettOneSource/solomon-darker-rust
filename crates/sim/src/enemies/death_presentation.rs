//! Corpse sprites and banishment columns from `native-enemy-death-effect-view.ts`
//! and `native-enemy-death-effect-presentation.ts`.

use super::actor_presentation::{EnemyArtAtlas, EnemyArtBlend, EnemySpriteLayer};
use super::model::{EnemyDeathEffect, EnemyDeathEffectKind};
use super::projectile_presentation::{effect_atlas, effect_blend};
use crate::Vec2;
use crate::float::fround;
use crate::secondary::presentation::NativeSecondaryQueueFamily;

/// One vertical gradient in Banish or Ultra Banish, relative to its actor root.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyBanishGradient {
    pub position: Vec2,
    pub width: f64,
    pub height: f64,
    pub start_color: u32,
    pub end_color: u32,
    pub start_alpha: f64,
    pub end_alpha: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnemyDeathEffectPlan {
    pub gradients: Vec<EnemyBanishGradient>,
    pub layers: Vec<EnemySpriteLayer>,
    pub queue_family: NativeSecondaryQueueFamily,
}

#[must_use]
pub fn native_enemy_death_effect_plan(
    effect: &EnemyDeathEffect,
    view_height: f64,
) -> EnemyDeathEffectPlan {
    native_enemy_death_effect_plan_with_input(
        effect,
        view_height,
        super::presentation::NativeEnemyEffectPresentationInput {
            age_ticks: effect.age_ticks as f64,
            rotation_radians: effect.rotation_deg * std::f64::consts::PI / 180.0,
        },
    )
}

#[must_use]
pub fn native_enemy_death_effect_plan_with_input(
    effect: &EnemyDeathEffect,
    view_height: f64,
    sample: super::presentation::NativeEnemyEffectPresentationInput,
) -> EnemyDeathEffectPlan {
    use EnemyDeathEffectKind as Kind;
    if matches!(effect.kind, Kind::Banish | Kind::BanishBlack) {
        return banish_plan(effect, view_height, sample.age_ticks);
    }
    // This row is an emitter. Its independent smoke children own all visible art.
    if effect.kind == Kind::BlackSmokyBouncer {
        return EnemyDeathEffectPlan::default();
    }
    let perspective = matches!(
        effect.kind,
        Kind::FadeScalePerspective
            | Kind::FadePerspective
            | Kind::FadePerspectiveClipped
            | Kind::LateSplat
    );
    let scale_y = if effect.kind == Kind::MoveFadePerspective {
        fround(effect.scale_y * f64::from(0.8_f32))
    } else if perspective {
        effect.scale_y * 0.75
    } else {
        effect.scale_y
    };
    let sprite = EnemySpriteLayer {
        alpha: effect.alpha,
        blend: effect_blend(effect.blend_mode),
        offset: Vec2::new(0.0, effect.height),
        rotation_radians: sample.rotation_radians,
        scale: effect.scale,
        scale_y: Some(scale_y),
        tint: effect.tint,
        ..EnemySpriteLayer::new(effect_atlas(effect.atlas), effect.entry, "death-effect")
    };
    let mut layers = Vec::new();
    if effect.shadow && (effect.kind != Kind::BoulderBit || effect.height != 0.0) {
        layers.push(EnemySpriteLayer {
            blend: EnemyArtBlend::Normal,
            offset: Vec2::new(0.0, 2.0),
            scale_y: Some(effect.scale_y * 0.75),
            tint: 0,
            role: "death-effect-shadow".to_owned(),
            ..sprite.clone()
        });
    }
    layers.push(sprite);
    EnemyDeathEffectPlan {
        gradients: Vec::new(),
        layers,
        queue_family: if effect.kind == Kind::BoulderBit {
            NativeSecondaryQueueFamily::OrdinaryDynamic
        } else {
            NativeSecondaryQueueFamily::Zanim
        },
    }
}

fn banish_plan(
    effect: &EnemyDeathEffect,
    view_height: f64,
    age_ticks: f64,
) -> EnemyDeathEffectPlan {
    let scale = effect.scale;
    let progress = (2.0 - age_ticks * (0.02 / scale)).max(0.0);
    let upper = view_height * 0.5 * scale;
    let lower = 50.0 * scale;
    let black = effect.kind == EnemyDeathEffectKind::BanishBlack;
    let orange = if black { 0x400000 } else { 0xff8000 };
    let yellow = if black { 0x400000 } else { 0xffbf00 };
    let mut gradients = Vec::new();
    let mut lower_gradients = Vec::new();
    for (half_width, extent, color, alpha) in [
        (10.0 * scale, upper, orange, (progress * 0.5).min(1.0)),
        (5.0 * progress * scale, upper, yellow, (progress * 0.5).min(1.0)),
        (2.0 * progress * scale, upper * 0.75, 0xffffff, (progress * 0.75).min(1.0)),
    ] {
        if half_width <= 0.0 {
            continue;
        }
        gradients.push(EnemyBanishGradient {
            position: Vec2::new(-half_width, -extent),
            width: half_width * 2.0,
            height: extent,
            start_color: 0,
            end_color: color,
            start_alpha: 1.0,
            end_alpha: alpha,
        });
        lower_gradients.push(EnemyBanishGradient {
            position: Vec2::new(-half_width, 0.0),
            width: half_width * 2.0,
            height: lower,
            start_color: color,
            end_color: 0,
            start_alpha: alpha,
            end_alpha: 1.0,
        });
    }
    // The source submits all three upper rails before the three lower rails.
    gradients.extend(lower_gradients);
    let tint = if black {
        (((progress * f64::from(0.15_f32)).min(1.0) * 255.0).round() as u32) << 16
    } else {
        0xff0000 | ((progress * 0.75 * 255.0).min(255.0).round() as u32) << 8
    };
    let mut layers = Vec::new();
    for index in 0..2 {
        layers.push(EnemySpriteLayer {
            alpha: progress.min(1.0),
            blend: EnemyArtBlend::Add,
            scale: 2.0 * progress * scale,
            tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 15, format!("banish-core-{index}"))
        });
    }
    let entry = 333 + (((effect.spawn_tick as f64 + age_ticks) / 4.0).floor() % 4.0) as usize;
    for index in 0..2 {
        layers.push(EnemySpriteLayer {
            alpha: progress.min(1.0),
            blend: EnemyArtBlend::Add,
            offset: Vec2::new(if black { 0.0 } else { 1.0 }, -40.0 * scale),
            scale: 2.0 * scale,
            scale_y: Some(3.0 * scale),
            tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, entry, format!("banish-upper-{index}"))
        });
    }
    EnemyDeathEffectPlan { gradients, layers, ..Default::default() }
}
