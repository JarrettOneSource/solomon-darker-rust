//! MagicMissile's double-pass compositor from `renderer/primary-spell-ether-native.ts`.

use super::model::{PrimarySpellEtherImpact, PrimarySpellEtherPierceStreak};
use super::presentation::{NativeSpellSprite, NativeSpellSpritePlan, SpellBlend};
use super::visual::visual_hash;
use crate::Vec2;
use crate::float;
use crate::float::fround;

#[must_use]
pub fn ether_primary_flight_plan(
    id: u64,
    age_ticks: f64,
    speed: f64,
    visual_scale: f64,
    underpowered: bool,
) -> NativeSpellSpritePlan {
    let phase = random(id, 0, 0) * 360.0 + age_ticks * speed * 3.0;
    let mut plan = ether_primary_compositor_plan(
        id,
        age_ticks.floor() as u64,
        phase,
        visual_scale,
        if underpowered { 0.5 } else { 1.0 },
    );
    plan.position = Vec2::new(0.0, -10.0);
    plan
}

#[must_use]
pub fn ether_primary_impact_plan(state: &PrimarySpellEtherImpact) -> NativeSpellSpritePlan {
    let mut fade = 2.0;
    for _ in 0..=state.common.age_ticks {
        fade = fround(fade - 0.1_f32 as f64);
    }
    let sample_tick = state.birth_tick + state.common.age_ticks;
    let mut plan = ether_primary_compositor_plan(
        state.common.id,
        sample_tick,
        sample_tick as f64,
        2.0 * state.visual_scale,
        fade,
    );
    plan.position = state.origin;
    plan.world_y = state.origin.y + 100.0;
    plan
}

#[must_use]
pub fn ether_primary_pierce_streak_plan(
    state: &PrimarySpellEtherPierceStreak,
) -> NativeSpellSpritePlan {
    let mut alpha = 1.0;
    for _ in 0..state.common.age_ticks {
        alpha = fround(alpha - 0.1_f32 as f64);
    }
    let mut plan = NativeSpellSpritePlan::at(state.origin);
    plan.draws.push(NativeSpellSprite {
        alpha: alpha.max(0.0),
        blend: SpellBlend::Add,
        rotation: state.heading_degrees * std::f64::consts::PI / 180.0,
        ..NativeSpellSprite::new(53, state.visual_scale)
    });
    plan
}

/// Shared native two-pass composition used by missiles and secondary Ether fades.
#[must_use]
pub fn ether_primary_compositor_plan(
    id: u64,
    sample_tick: u64,
    phase: f64,
    overall_scale: f64,
    alpha_multiplier: f64,
) -> NativeSpellSpritePlan {
    let mut channel = 0;
    let mut draw_random = || {
        channel += 1;
        random(id, sample_tick, channel)
    };
    let sampled_scale = (1.0 + draw_random() * 0.5) * overall_scale;
    let core_pulse = 0.15 * sin_degrees(15.0 * phase).abs();
    let mut plan = NativeSpellSpritePlan::at(Vec2::ZERO);
    for _ in 0..2 {
        plan.draws.push(NativeSpellSprite {
            alpha: 0.2 + draw_random() * 0.25,
            tint: 0xff80ff,
            ..NativeSpellSprite::new(110, (2.5 + core_pulse) * sampled_scale)
        });
        plan.draws.push(NativeSpellSprite {
            alpha: 0.35 + draw_random() * 0.55,
            tint: 0xff80ff,
            ..NativeSpellSprite::new(110, (1.5 + core_pulse) * sampled_scale)
        });
        plan.draws.push(NativeSpellSprite {
            alpha: 0.35 * sin_degrees(5.0 * phase).abs(),
            blend: SpellBlend::Add,
            rotation: 50.0 * sampled_scale * sin_degrees(phase) * std::f64::consts::PI / 180.0,
            ..NativeSpellSprite::new(111, (1.0 + draw_random() * 0.1) * sampled_scale)
        });
        let count = (draw_random() * 10.0).floor() as usize + 2;
        for _ in 0..count {
            let angle = draw_random() * std::f64::consts::PI * 2.0;
            let radius = draw_random() * 20.0 * sampled_scale;
            let scale = (0.25 + draw_random() * 0.2) * sampled_scale;
            let alpha = draw_random() * 0.75;
            // The source samples degrees before the view converts them to radians.
            let rotation = draw_random() * 360.0 * std::f64::consts::PI / 180.0;
            plan.draws.push(NativeSpellSprite {
                alpha,
                blend: SpellBlend::Add,
                offset: Vec2::new(float::sin(angle) * radius, -float::cos(angle) * radius),
                rotation,
                ..NativeSpellSprite::new(111, scale)
            });
        }
        plan.draws.push(NativeSpellSprite {
            alpha: 0.55 * sin_degrees(8.0 * phase).abs(),
            blend: SpellBlend::Add,
            rotation: 50.0 * sampled_scale * sin_degrees(0.5 * phase) * std::f64::consts::PI
                / 180.0,
            ..NativeSpellSprite::new(112, (1.0 + draw_random() * 0.3) * sampled_scale)
        });
    }
    if alpha_multiplier != 1.0 {
        for draw in &mut plan.draws {
            draw.alpha = fround(draw.alpha * alpha_multiplier);
        }
    }
    plan
}

fn sin_degrees(degrees: f64) -> f64 {
    float::sin(degrees * std::f64::consts::PI / 180.0)
}
fn random(id: u64, tick: u64, channel: u32) -> f64 {
    let salted_id = id ^ u64::from(channel.wrapping_add(1).wrapping_mul(0x85eb_ca6b));
    f64::from(visual_hash(salted_id, tick as u32)) / 4_294_967_296.0
}
