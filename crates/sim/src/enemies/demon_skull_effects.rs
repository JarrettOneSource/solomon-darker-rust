//! Discorporeal warmups, trails and beam bands from `enemies/demon-skull-effects.ts`.

use super::boss_spell::BossSpellKind;
use super::effects::{DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec};
use super::model::*;
use super::world::EnemyWorld;
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;

pub const UNHOLY_GREEN: u32 = 0x8c_ff_0c;

impl EnemyWorld {
    pub(super) fn random_enemy_offset(&mut self, radius: f64) -> Vec2 {
        let length = self.steering_rng.draw_scaled_float(radius, false);
        self.random_enemy_offset_at_radius(length)
    }
    pub(super) fn random_enemy_offset_at_radius(&mut self, radius: f64) -> Vec2 {
        let angle =
            self.steering_rng.draw_scaled_float(360.0, false) * std::f64::consts::PI / 180.0;
        Vec2::new(fround(float::sin(angle) * radius), fround(-float::cos(angle) * radius))
    }
}

pub fn spawn_demon_skull_warmup(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    tick: u64,
    eyes: bool,
    order: &mut WorldPainterOrder,
) {
    let scale = fround(0.5 + world.steering_rng.draw_scaled_float(0.5, false))
        * if eyes { 2.0 } else { 1.0 };
    let length = if eyes {
        fround(90.0 + world.steering_rng.draw_scaled_float(40.0, false))
    } else {
        fround(100.0 + world.steering_rng.draw_scaled_float(70.0, false))
    };
    let offset = world.random_enemy_offset_at_radius(length);
    let position =
        Vec2::new(fround(actor.position.x + offset.x), fround(actor.position.y - 60.0 + offset.y));
    let velocity = if eyes {
        Vec2::ZERO
    } else {
        Vec2::new(fround(-offset.x / length * 3.0), fround(-offset.y / length * 3.0))
    };
    world.spawn_simple_death_effect(
        DeathEffectOwner::from(actor),
        tick,
        SimpleDeathEffectSpec {
            alpha: 1.0,
            alpha_loss_per_tick: if eyes { 0.02 } else { 0.005 },
            atlas: EnemyEffectAtlas::Unholy,
            entry: if eyes { 2 } else { 0 },
            blend_mode: EnemyEffectBlend::Add,
            kind: if eyes {
                EnemyDeathEffectKind::FadeAdditive
            } else {
                EnemyDeathEffectKind::MoveFade
            },
            lifetime_ticks: 1000,
            role: if eyes { "discorporeal-eyes-warmup" } else { "discorporeal-mouth-warmup" },
            scale,
        },
        DeathEffectOverrides {
            position: Some(position),
            presentation_owner: Some(DeathPresentationOwner::LateWorldOverlay),
            rotation_deg: Some(if eyes {
                0.0
            } else {
                heading_degrees_from_vector(velocity.x, velocity.y)
            }),
            tint: Some(UNHOLY_GREEN),
            velocity: Some(velocity),
            velocity_damping: Some(fround(0.95)),
            ..Default::default()
        },
        order,
    );
}

pub fn spawn_unholy_eye_trail(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    heading: f64,
    enhanced_effects: bool,
    order: &mut WorldPainterOrder,
) {
    let alpha = fround(1.0 - world.steering_rng.draw_scaled_float(fround(0.95), false));
    let loss = fround(fround(0.025) + world.steering_rng.draw_scaled_float(fround(0.035), false));
    world.spawn_simple_death_effect(
        owner,
        tick,
        SimpleDeathEffectSpec {
            alpha,
            alpha_loss_per_tick: fround(loss * if enhanced_effects { 0.5 } else { 1.0 }),
            atlas: EnemyEffectAtlas::Unholy,
            entry: 0,
            blend_mode: EnemyEffectBlend::Add,
            kind: EnemyDeathEffectKind::FadePerspective,
            lifetime_ticks: 1000,
            role: "eye-laser-trail",
            scale: 1.0,
        },
        DeathEffectOverrides {
            rotation_deg: Some(heading),
            tint: Some(UNHOLY_GREEN),
            ..Default::default()
        },
        order,
    );
}

pub fn spawn_unholy_spit_trail(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    progress: f64,
    height: f64,
    order: &mut WorldPainterOrder,
) {
    let offset = world.random_enemy_offset(10.0);
    let position = Vec2::new(
        fround(owner.position.x + offset.x),
        fround(owner.position.y - float::sin(progress * std::f64::consts::PI) * height + offset.y),
    );
    let scale = fround(0.5 + world.steering_rng.draw_scaled_float(0.5, false));
    world.spawn_simple_death_effect(
        owner,
        tick,
        SimpleDeathEffectSpec {
            alpha: 1.0,
            alpha_loss_per_tick: fround(0.02) * fround(0.65),
            atlas: EnemyEffectAtlas::Unholy,
            entry: 9 + ((tick / 2) % 32) as usize,
            blend_mode: EnemyEffectBlend::Add,
            kind: EnemyDeathEffectKind::FadeAdditive,
            lifetime_ticks: 1000,
            role: "unholy-spit-trail",
            scale,
        },
        DeathEffectOverrides {
            position: Some(position),
            opacity_timer: Some(1.5),
            ..Default::default()
        },
        order,
    );
}

pub fn spawn_unholy_impact_ground(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    eye: bool,
    order: &mut WorldPainterOrder,
) {
    if eye {
        world.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: fround(0.1),
                atlas: EnemyEffectAtlas::BadGuys,
                entry: 15,
                blend_mode: EnemyEffectBlend::Normal,
                kind: EnemyDeathEffectKind::Fade,
                lifetime_ticks: 1000,
                role: "eye-impact-flash",
                scale: 9.0,
            },
            DeathEffectOverrides {
                position: Some(Vec2::new(owner.position.x, owner.position.y - 25.0)),
                presentation_owner: Some(DeathPresentationOwner::DirectPostWorld),
                tint: Some(UNHOLY_GREEN),
                ..Default::default()
            },
            order,
        );
    }
    world.spawn_simple_death_effect(
        owner,
        tick,
        SimpleDeathEffectSpec {
            alpha: 1.0,
            alpha_loss_per_tick: 0.0,
            atlas: EnemyEffectAtlas::BadGuys,
            entry: 401,
            blend_mode: EnemyEffectBlend::Add,
            kind: EnemyDeathEffectKind::SpriteArray,
            lifetime_ticks: 1000,
            role: if eye { "unholy-eye-ground-burst" } else { "unholy-spit-ground-burst" },
            scale: 3.0,
        },
        DeathEffectOverrides {
            first_entry: Some(401),
            frame_count: Some(19),
            frame_ticks: Some(1.0),
            frame_velocity: Some(if eye { 0.75 } else { 0.5 }),
            frame_velocity_damping: Some(fround(0.98)),
            presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
            tint: Some(UNHOLY_GREEN),
            ..Default::default()
        },
        order,
    );
}

pub fn spawn_demon_skull_beam_segments(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    tick: u64,
    length: f64,
    power: f64,
    order: &mut WorldPainterOrder,
) {
    let angle = actor.heading_deg * std::f64::consts::PI / 180.0;
    let dx = float::sin(angle);
    let dy = -float::cos(angle);
    let jitter = world.random_enemy_offset_at_radius(1.0);
    let scale = (2.0 + world.steering_rng.draw_scaled_float(0.5, false)) * 2.0;
    world.spawn_simple_death_effect(
        DeathEffectOwner::from(actor),
        tick,
        SimpleDeathEffectSpec {
            alpha: 1.0,
            alpha_loss_per_tick: 0.5,
            atlas: EnemyEffectAtlas::Unholy,
            entry: 2,
            blend_mode: EnemyEffectBlend::Add,
            kind: EnemyDeathEffectKind::FadeAdditive,
            lifetime_ticks: 1000,
            role: "mouth-beam-source",
            scale,
        },
        DeathEffectOverrides {
            position: Some(Vec2::new(actor.position.x + dx * 32.0, actor.position.y + dy * 32.0)),
            tint: Some(UNHOLY_GREEN),
            ..Default::default()
        },
        order,
    );
    let mut distance = 0.0;
    while distance < length {
        let center = Vec2::new(
            actor.position.x + dx * (distance + 64.0),
            actor.position.y + dy * (distance + 64.0),
        );
        let vertices =
            [(-32.0, -64.0), (32.0, -64.0), (-32.0, 64.0), (32.0, 64.0)].map(|(x, y)| {
                Vec2::new(
                    fround(center.x + float::cos(angle) * x - float::sin(angle) * y),
                    fround(center.y + float::sin(angle) * x + float::cos(angle) * y - 40.0),
                )
            });
        let fade = |along: f64| {
            (1.0 - (along - (length - 200.0)) / 200.0).clamp(0.0, 1.0) * power.min(1.0)
        };
        let position = Vec2::new(fround(center.x + jitter.x), fround(center.y + jitter.y));
        let spell = world.create_boss_spell(
            actor.id,
            tick,
            0.0,
            position,
            BossSpellKind::MouthBeamSegment {
                vertices,
                uv_offset: (tick % 25) as f64 / 25.0,
                start_alpha: fade(distance + 128.0),
                end_alpha: if distance == 0.0 { 0.0 } else { fade(distance) },
            },
            order,
        );
        world.boss_spells.push(spell);
        distance += 128.0;
    }
}
