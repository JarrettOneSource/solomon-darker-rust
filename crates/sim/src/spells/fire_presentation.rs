//! Fire's stock compositor passes from `renderer/primary-spell-fire-native.ts`.

use super::fire::{
    native_fire_particle_fade_step, native_fire_presentation_random,
    native_fire_presentation_random_int,
};
use super::model::*;
use super::presentation::*;
use crate::Vec2;
use crate::float;
use crate::float::{atan2, fround};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFireballPresentationState {
    pub age_ticks: f64,
    pub direction: Vec2,
    pub id: u64,
    pub position: Vec2,
    pub underpowered: bool,
}

impl From<ProjectileRef<'_, FireProjectileProgram>> for NativeFireballPresentationState {
    fn from(state: ProjectileRef<'_, FireProjectileProgram>) -> Self {
        Self {
            age_ticks: state.common.age_ticks as f64,
            direction: state.common.direction,
            id: state.common.id,
            position: state.common.position,
            underpowered: state.underpowered,
        }
    }
}

#[must_use]
pub fn native_fireball_plan(
    state: NativeFireballPresentationState,
    presentation_sample: u64,
) -> NativeSpellSpritePlan {
    let frame = 255 + ((state.age_ticks / 3.0).floor() % 12.0) as usize;
    let rotation = atan2(state.direction.y, state.direction.x) + std::f64::consts::PI / 2.0;
    let alpha = if state.underpowered { 0.5 } else { 1.0 };
    let mut plan = NativeSpellSpritePlan::at(state.position);
    for (record, blend, scale, opacity, tint) in [
        (
            110,
            SpellBlend::Normal,
            Vec2::new(3.2, 4.0),
            (0.2 + random(state.id, presentation_sample, 7, 0.25)) * alpha,
            0xff8000,
        ),
        (frame, SpellBlend::Add, Vec2::new(2.0, 2.5), alpha, 0xffffff),
        (frame, SpellBlend::Normal, Vec2::new(2.0, 2.5), 0.5 * alpha, 0xffffff),
    ] {
        plan.draws.push(NativeSpellSprite {
            alpha: opacity,
            blend,
            offset: Vec2::new(0.0, -10.0),
            rotation,
            scale,
            tint,
            ..NativeSpellSprite::new(record, 1.0)
        });
    }
    plan
}

#[must_use]
pub fn native_fire_particle_plan(state: &PrimarySpellFireParticle) -> NativeSpellSpritePlan {
    native_fire_particle_plan_at_age(state, state.common.age_ticks as f64)
}

/// Fractional received-snapshot age affects travel, scale and fading continuously.
#[must_use]
pub fn native_fire_particle_plan_at_age(
    state: &PrimarySpellFireParticle,
    age: f64,
) -> NativeSpellSpritePlan {
    let id = state.common.id;
    let angle = random(id, 0, 0, std::f64::consts::PI * 2.0);
    let radius = random(id, 0, 1, 10.0);
    let travel = -10.0 + age * 2.0;
    let position = Vec2::new(
        state.origin.x + float::cos(angle) * radius + state.direction.x * travel,
        state.origin.y - 10.0 + float::sin(angle) * radius + state.direction.y * travel,
    );
    let fade = native_fire_particle_fade_step(id, state.enhanced_effects);
    let red = (1.0 - fade * age).clamp(0.0, 1.0);
    let green_blue = (1.0 - fade * 2.0 * age).clamp(0.0, 1.0);
    let mut plan = NativeSpellSpritePlan::at(position);
    plan.world_y += 30.0;
    plan.draws.push(NativeSpellSprite {
        alpha: red,
        blend: SpellBlend::Add,
        rotation: (random(id, 0, 2, 360.0) + age) * std::f64::consts::PI / 180.0,
        tint: pack_spell_rgb(red, green_blue, green_blue),
        ..NativeSpellSprite::new(
            267 + state.variant as usize,
            (random(id, 0, 3, 1.0) + 0.5) * 1.25 * crate::float::pow(0.95, age),
        )
    });
    plan
}

#[must_use]
pub fn native_fire_impact_plan(state: &PrimarySpellFireImpact) -> NativeSpellSpritePlan {
    native_fire_impact_plan_at_age(state, state.common.age_ticks as f64)
}

#[must_use]
pub fn native_fire_impact_plan_at_age(
    state: &PrimarySpellFireImpact,
    age: f64,
) -> NativeSpellSpritePlan {
    let id = state.common.id;
    let scale = 1.0 + random(id, 0, 9, 0.1);
    let angular_magnitude = 0.5 + random(id, 0, 11, 1.0);
    let angular_direction =
        if native_fire_presentation_random_int(id, 0, 13, 2) == 1 { -1.0 } else { 1.0 };
    let rotation = (random(id, 0, 10, 360.0) + angular_direction * angular_magnitude * age)
        * std::f64::consts::PI
        / 180.0;
    let mut plan =
        NativeSpellSpritePlan::at(Vec2::new(state.origin.x, state.origin.y - 10.0 - age));
    plan.world_y += 50.0;
    plan.draws.push(NativeSpellSprite {
        alpha: 0.5 * (1.0 - age / 16.0),
        tint: 0xff8000,
        ..NativeSpellSprite::new(110, 5.0 * scale)
    });
    plan.draws.push(NativeSpellSprite {
        blend: SpellBlend::Add,
        rotation,
        tint: 0xffffbf,
        ..NativeSpellSprite::new(251 + (age / 4.0).floor() as usize, scale)
    });
    plan
}

#[must_use]
pub fn native_fire_explosion_plan(
    age_ticks: f64,
    origin: Vec2,
    presentation: FireExplosionPresentation,
    visual_scale: f64,
    point_gain: f64,
) -> NativeSpellSpritePlan {
    // The source explosion arrays deliberately sample whole native epochs even
    // when their containing snapshot has a fractional display age.
    let age = age_ticks.max(0.0).floor() as u64;
    let scale = visual_scale;
    let tint = if presentation == FireExplosionPresentation::Steam { 0xcccccc } else { 0xffffff };
    let mut plan = NativeSpellSpritePlan::at(origin);
    if age < 10 {
        plan.draws.push(NativeSpellSprite {
            alpha: repeated_add(1.0, -(0.1_f32 as f64), age),
            offset: Vec2::new(0.0, -25.0),
            queue: SpellDrawQueue::PostWorld,
            ..NativeSpellSprite::new(15, fround(scale * 6.0))
        });
    }
    if age < 35 {
        let phase = array_phase(age, 0.75, 0.98_f32 as f64);
        plan.draws.push(NativeSpellSprite {
            blend: SpellBlend::Add,
            tint,
            queue: SpellDrawQueue::PreWorld,
            ..NativeSpellSprite::new(401 + phase.trunc() as usize, fround(scale * 2.0))
        });
    }
    if age < 37 {
        let phase = array_phase(age, 0.625, 0.97_f32 as f64);
        let y = repeated_add(fround(scale * -15.0), fround(scale * -1.15), age);
        plan.draws.push(NativeSpellSprite {
            blend: SpellBlend::Add,
            tint,
            offset: Vec2::new(0.0, y),
            ..NativeSpellSprite::new(
                420 + phase.trunc() as usize,
                fround(2.0 * point_gain.max(0.0)),
            )
        });
    }
    plan
}

#[must_use]
pub fn native_fire_ember_plan(
    id: u64,
    height: f64,
    life: f64,
    phase: f64,
    position: Vec2,
    presentation_sample: u64,
    enhanced_effects: bool,
) -> NativeSpellSpritePlan {
    let alpha = life.min(1.0);
    let frame = 267 + phase.floor() as usize % 4;
    let mut plan = NativeSpellSpritePlan::at(position);
    // The view adds the source's registered airborne ground glow beneath this stack.
    plan.draws.push(NativeSpellSprite {
        alpha,
        offset: Vec2::new(0.0, height),
        ..NativeSpellSprite::new(frame, 0.5)
    });
    for index in 0..if enhanced_effects { 2 } else { 1 } {
        plan.draws.push(NativeSpellSprite {
            alpha,
            blend: SpellBlend::Add,
            offset: Vec2::new(0.0, height),
            rotation: random(id, presentation_sample, 31 + index * 2, 0.1) * std::f64::consts::PI
                / 180.0,
            ..NativeSpellSprite::new(
                frame,
                0.75 - random(id, presentation_sample, 32 + index * 2, 0.5),
            )
        });
    }
    plan.draws.push(NativeSpellSprite {
        alpha: (life * 0.2).min(1.0),
        blend: SpellBlend::Add,
        offset: Vec2::new(0.0, height * 0.8),
        tint: 0xff8000,
        ..NativeSpellSprite::new(15, life.min(1.0))
    });
    plan
}

#[must_use]
pub fn native_fire_patch_plan(
    state: &PrimarySpellFirePatch,
    presentation_sample: u64,
) -> NativeSpellSpritePlan {
    let transform = native_fire_patch_transform(
        state.position,
        state.scale,
        state.fade_alpha,
        state.horizontal_sign,
    );
    let glow = native_fire_ground_glow_plan(
        state.common.id,
        state.life,
        state.position,
        state.scale,
        presentation_sample,
    );
    let mut plan = NativeSpellSpritePlan::at(transform.position);
    plan.world_y = state.position.y;
    plan.draws.push(NativeSpellSprite {
        alpha: glow.alpha,
        queue: SpellDrawQueue::PreWorld,
        offset: Vec2::new(
            glow.position.x - transform.position.x,
            glow.position.y - transform.position.y,
        ),
        tint: glow.tint,
        ..NativeSpellSprite::new(15, glow.scale)
    });
    plan.draws.push(NativeSpellSprite {
        alpha: (state.draw_alpha * state.life).min(1.0),
        atlas: SpellAtlas::DeadHawg,
        blend: SpellBlend::Add,
        scale: Vec2::new(transform.scale_x, transform.scale_y),
        ..NativeSpellSprite::new(
            46 + state.atlas_phase.round_ties_even().rem_euclid(32.0) as usize,
            1.0,
        )
    });
    plan
}

/// The same native flame transform is used by player Fire and Demon fire patches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFirePatchTransform {
    pub position: Vec2,
    pub scale_x: f64,
    pub scale_y: f64,
}

/// `nativeFirePatchTransform` in `renderer/primary-spell-fire-native.ts`.
#[must_use]
pub fn native_fire_patch_transform(
    position: Vec2,
    scale: f64,
    fade_alpha: f64,
    horizontal_sign: f64,
) -> NativeFirePatchTransform {
    // Both stores round in the native transform; combining the products changes the edge.
    let scale_y = fround(fround(1.1_f32 as f64 * scale) * fade_alpha);
    NativeFirePatchTransform {
        position: Vec2::new(
            position.x,
            fround(fround(position.y + 10.0) + fround(-20.0 * scale_y)),
        ),
        scale_x: scale_y * horizontal_sign,
        scale_y,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFireGroundGlowPlan {
    pub alpha: f64,
    pub position: Vec2,
    pub scale: f64,
    pub tint: u32,
}

/// `nativeFireGroundGlowPlan`; its draw is submitted before the world painter.
#[must_use]
pub fn native_fire_ground_glow_plan(
    id: u64,
    life: f64,
    position: Vec2,
    scale: f64,
    presentation_sample: u64,
) -> NativeFireGroundGlowPlan {
    let green = (random(id, presentation_sample, 61, 1.0) * 255.0).round() as u32;
    NativeFireGroundGlowPlan {
        alpha: fround(life.min(1.0) * 0.5),
        position,
        scale: fround(scale * 2.0),
        tint: 0xff0000 | green << 8,
    }
}

#[must_use]
pub fn native_fire_good_imp_plan(state: &PrimarySpellFireGoodImp) -> NativeSpellSpritePlan {
    let facing = ((state.heading_degrees + 15.0) / 30.0).trunc().rem_euclid(12.0) as usize;
    let mut plan = NativeSpellSpritePlan::at(state.position);
    plan.region_light_point = Some(state.position);
    plan.draws.push(NativeSpellSprite {
        blend: SpellBlend::Add,
        offset: Vec2::new(0.0, state.vertical_offset),
        rotation: state.body_rotation_degrees * std::f64::consts::PI / 180.0,
        ..NativeSpellSprite::new(285 + state.body_variant as usize * 12 + facing, state.body_scale)
    });
    if state.effect_alpha > 0.0 {
        plan.draws.push(NativeSpellSprite {
            alpha: state.effect_alpha,
            blend: SpellBlend::Add,
            offset: Vec2::new(0.0, state.vertical_offset - 10.0),
            ..NativeSpellSprite::new(
                333 + state.effect_phase.trunc().clamp(0.0, 9.0) as usize,
                state.body_scale,
            )
        });
    }
    if let (Some(age), Some(origin)) = (state.contact_age_ticks, state.contact_origin) {
        plan.draws.push(NativeSpellSprite {
            offset: Vec2::new(origin.x - state.position.x, origin.y - state.position.y),
            ..NativeSpellSprite::new(251 + (age / 4).min(3) as usize, state.contact_scale)
        });
    }
    plan
}

fn random(id: u64, sample: u64, channel: u32, maximum: f64) -> f64 {
    native_fire_presentation_random(id, sample, channel, maximum)
}
fn repeated_add(mut value: f64, step: f64, count: u64) -> f64 {
    for _ in 0..count {
        value = fround(value + step);
    }
    value
}
fn array_phase(age: u64, mut step: f64, retention: f64) -> f64 {
    let mut phase = 0.0;
    for _ in 0..age {
        phase = fround(phase + step);
        step = fround(step * retention);
    }
    phase
}
