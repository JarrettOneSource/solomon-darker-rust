//! Faculty dying interval and final bone/scrap burst from `enemies/faculty-death.ts`.

use super::config::EnemyFamilyConfig;
use super::effects::*;
use super::faculty_effects::{FacultySmokePhase, spawn_faculty_smoke};
use super::model::*;
use super::skeleton_death::skeleton_death_fragment_entries;
use super::terminal_effects::shuffle_actor_fragments;
use super::world::EnemyWorld;
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::painter::WorldPainterOrder;

pub fn spawn_faculty_dying_effects(
    world: &mut EnemyWorld,
    actor: &impl EnemyActorRead,
    tick: u64,
    death_tick: u64,
    enhanced_effects: bool,
    order: &mut WorldPainterOrder,
) {
    let owner = DeathEffectOwner::from(actor);
    if death_tick == 1 {
        for _ in (0..360).step_by(5) {
            spawn_faculty_smoke(world, owner, tick, FacultySmokePhase::Start, order);
        }
    }
    if (250_i64 - death_tick as i64) % 2 == 0 {
        world.spawn_bouncer(
            owner,
            tick,
            |_| 118,
            "faculty-dying-bone",
            |world| {
                let unit = radial_vector(world.draw_unit() * 360.0, 1.0);
                let velocity = Vec2::new(unit.x * 1.5, unit.y);
                let distance = 15.0 + world.draw_unit() * 30.0;
                let position = Vec2::new(
                    owner.position.x + velocity.x * (distance + 2.0),
                    owner.position.y + velocity.y * distance,
                );
                let launch = if world.draw_integer(3) == 2 { 4.0 } else { 2.0 };
                BouncerOptions {
                    bounce_velocity_scale: Some(launch),
                    kind: BouncerKind::BlackSmoky,
                    position: Some(position),
                    velocity: Some(Vec2::new(velocity.x * launch, velocity.y * launch)),
                    ..Default::default()
                }
            },
            enhanced_effects,
            order,
        );
    }
    spawn_faculty_smoke(world, owner, tick, FacultySmokePhase::Dying, order);
}

pub fn spawn_faculty_finale(
    world: &mut EnemyWorld,
    actor: &impl EnemyActorRead,
    tick: u64,
    enhanced_effects: bool,
    order: &mut WorldPainterOrder,
) {
    let EnemyFamilyConfig::Faculty(family) = &actor.config.family else {
        panic!("Faculty finale requires its family")
    };
    let owner = DeathEffectOwner::from(actor);
    let mut entries = skeleton_death_fragment_entries(enhanced_effects).to_vec();
    shuffle_actor_fragments(world, &mut entries);
    let mut angle = world.draw_unit() * 360.0;
    for entry in entries {
        world.spawn_bouncer(
            owner,
            tick,
            |_| entry,
            "faculty-bone",
            |world| {
                let unit = radial_vector(angle, 1.0);
                let velocity = Vec2::new(unit.x * 3.0, unit.y * 2.0);
                let distance = 15.0 + world.draw_unit() * 10.0;
                BouncerOptions {
                    bounce_velocity_scale: Some(2.0),
                    height_scale: Some(2.0),
                    scale: Some(1.200_000_047_683_715_8),
                    position: Some(Vec2::new(
                        owner.position.x + velocity.x * (distance + 2.0),
                        owner.position.y + velocity.y * distance,
                    )),
                    velocity: Some(velocity),
                    ..Default::default()
                }
            },
            enhanced_effects,
            order,
        );
        angle = fround(angle + 72.0 + (world.draw_unit() * 2.0 - 1.0) * 10.0);
    }
    let velocity = radial_vector(angle, 4.0);
    world.spawn_bouncer(
        owner,
        tick,
        |world| 1819 + world.draw_integer(4) as usize,
        "faculty-skull",
        |_| BouncerOptions {
            bounce_velocity_scale: Some(2.0),
            height_scale: Some(2.0),
            velocity: Some(velocity),
            ..Default::default()
        },
        enhanced_effects,
        order,
    );
    world.spawn_unbind(actor, tick, order);
    world.spawn_simple_death_effect(
        owner,
        tick,
        SimpleDeathEffectSpec {
            alpha: 1.0,
            alpha_loss_per_tick: 0.009_999_999_776_482_582,
            atlas: EnemyEffectAtlas::BadGuys,
            blend_mode: EnemyEffectBlend::Add,
            entry: 15,
            kind: EnemyDeathEffectKind::BanishBlack,
            lifetime_ticks: 1000,
            role: "faculty-banish",
            scale: 2.0,
        },
        DeathEffectOverrides { opacity_timer: Some(2.0), ..Default::default() },
        order,
    );
    world.spawn_simple_death_effect(
        owner,
        tick,
        SimpleDeathEffectSpec {
            alpha: 1.0,
            alpha_loss_per_tick: 0.0,
            atlas: EnemyEffectAtlas::BadGuys,
            blend_mode: EnemyEffectBlend::Normal,
            entry: 401,
            kind: EnemyDeathEffectKind::SpriteArray,
            lifetime_ticks: 1000,
            role: "faculty-terminal-burst",
            scale: 4.0,
        },
        DeathEffectOverrides {
            first_entry: Some(401),
            frame_count: Some(19),
            frame_ticks: Some(1.0),
            frame_velocity: Some(0.5),
            frame_velocity_damping: Some(0.980_000_019_073_486_3),
            presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
            tint: Some(0),
            ..Default::default()
        },
        order,
    );
    for degrees in (0..360).step_by(5) {
        spawn_faculty_smoke(world, owner, tick, FacultySmokePhase::Terminal, order);
        let opacity = fround(2.0 + world.draw_unit() * 2.0);
        let scale = fround(0.25 + world.draw_unit() * 0.799_999_952_316_284_2);
        let phase = world.draw_unit() * 360.0;
        let amplitude = fround(10.0 + world.draw_unit() * 40.0);
        let step = fround(1.0 + world.draw_unit() * 4.0);
        let color = if world.draw_integer(4) == 1 { family.head_color } else { family.body_color };
        let distance = fround(30.0 + world.draw_unit() * 20.0);
        let direction = radial_vector(f64::from(degrees), 1.0);
        let position = Vec2::new(
            fround(owner.position.x + direction.x * distance),
            fround(owner.position.y - 35.0 + direction.y * distance),
        );
        let velocity = radial_vector(f64::from(degrees), fround(1.0 + world.draw_unit() * 9.0));
        let sort_bias = fround(world.draw_unit() * 100.0 - 50.0);
        let tint = (((color[0] * 255.0).round() as u32) << 16)
            | (((color[1] * 255.0).round() as u32) << 8)
            | (color[2] * 255.0).round() as u32;
        world.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: 0.009_999_999_776_482_582,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Normal,
                entry: 66,
                kind: EnemyDeathEffectKind::Scrap,
                lifetime_ticks: 1000,
                role: "faculty-scrap",
                scale,
            },
            DeathEffectOverrides {
                opacity_timer: Some(opacity),
                painter_sort_bias: Some(sort_bias),
                position: Some(position),
                rotation_deg: Some(float::sin(phase * std::f64::consts::PI / 180.0) * amplitude),
                scrap_oscillation: Some(ScrapOscillation {
                    phase_deg: phase,
                    amplitude_deg: amplitude,
                    step_deg: step,
                }),
                tint: Some(tint),
                velocity: Some(velocity),
                velocity_damping: Some(0.920_000_016_689_300_5),
                ..Default::default()
            },
            order,
        );
    }
}
