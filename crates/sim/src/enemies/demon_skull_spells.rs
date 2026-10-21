//! Discorporeal eye, bite, spit and mouth authority from `enemies/demon-skull-spells.ts`.

use super::boss_spell::{BossSpellKind, NativeBossSpell};
use super::demon_skull_attachments::DEMON_SKULL_EYE_ATTACHMENTS;
use super::demon_skull_effects::*;
use super::effects::{DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec};
use super::model::*;
use super::projectiles::EnemyProjectileKnockback;
use super::targeting::{target_by_id, target_eligible};
use super::world::{
    EnemyMovementPurpose, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemyStepContext,
    EnemyWorld,
};
use super::{EnemySpawnIntent, EnemyToken};
use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::line_obstruction::clip_line_to_bounds;
use crate::painter::WorldPainterOrder;
use crate::spells::fire::{
    CreateNativeFirePatchOptions, spawn_native_fire_patch, step_native_fire_patch,
};
use crate::spells::model::FirePatchType;
use crate::spells::targeting::native_primary_polygon_targets;

#[must_use]
pub fn native_demon_skull_facing(heading: f64) -> usize {
    (((heading.round_ties_even() + 7.0) / 15.0).trunc().rem_euclid(24.0)) as usize
}
#[must_use]
pub fn native_demon_skull_bob(phase: f64) -> f64 {
    fround(float::sin(phase * std::f64::consts::PI / 180.0) * 10.0 - 25.0)
}
#[must_use]
pub fn native_demon_skull_eyes(
    position: Vec2,
    brain: &DemonSkullBrain,
    scale: f64,
    heading: f64,
) -> [Vec2; 2] {
    let points =
        DEMON_SKULL_EYE_ATTACHMENTS[brain.body_pose as usize][native_demon_skull_facing(heading)];
    let root = Vec2::new(
        position.x + brain.body_offset.x,
        position.y + brain.body_offset.y + native_demon_skull_bob(brain.body_phase_deg),
    );
    points.map(|point| {
        Vec2::new(fround(root.x + point.x * 2.0 * scale), fround(root.y + point.y * 2.0 * scale))
    })
}

pub fn fire_demon_skull_eyes(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    world.emit_action_sound(
        DeathEffectOwner::from(&*actor),
        context.tick,
        EnemySound::LightningStart,
        1.0,
    );
    flash(
        world,
        actor,
        context.tick,
        EnemyScreenFlash {
            red: 0.5,
            green: 1.0,
            blue: 0.5,
            alpha: 0.75,
            decay_per_tick: fround(0.1),
            point_attenuated: true,
        },
    );
    let selected = target_by_id(actor.target_player_id.as_deref(), context.players);
    let target = selected.filter(|target| !target.summoned).or_else(|| {
        context.players.iter().find(|target| !target.summoned && target_eligible(target))
    });
    let aim = if let Some(target) = target {
        let lead =
            hypot(target.position.x - actor.position.x, target.position.y - actor.position.y)
                * 0.25;
        Vec2::new(
            target.position.x + target.velocity_per_tick.x * lead,
            target.position.y + target.velocity_per_tick.y * lead + 15.0,
        )
    } else {
        advance(actor.position, direction(actor.heading_deg, 1.0), 100.0)
    };
    let heading = heading_degrees_from_vector(aim.x - actor.position.x, aim.y - actor.position.y);
    let velocity = direction(heading, 4.0);
    let EnemyBrain::DemonSkull(brain) = &actor.brain else {
        panic!("DemonSkull eyes require their owner")
    };
    let eyes =
        native_demon_skull_eyes(actor.position, brain, actor.config.scale, brain.body_heading_deg);
    for position in eyes {
        let phase = world.steering_rng.draw_scaled_float(360.0, false);
        let spell = world.create_boss_spell(
            actor.id,
            context.tick,
            actor.config.secondary_damage,
            position,
            BossSpellKind::EyeLaser { heading_deg: heading, phase_deg: phase, velocity },
            order,
        );
        if blocked_line(context, spell.owner.id, actor.position, position, 0x380) {
            impact_eye(world, &spell, context, order);
        } else {
            world.boss_spells.push(spell);
        }
    }
    let EnemyBrain::DemonSkull(brain) = &mut actor.brain else { unreachable!() };
    brain.recoil = direction(actor.heading_deg + 180.0, 10.0);
}

pub fn bite_demon_skull(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let Some(target) = target_by_id(actor.target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target))
    else {
        return;
    };
    let dx = target.position.x - actor.position.x;
    let dy = target.position.y - actor.position.y;
    let distance = hypot(dx, dy);
    let gap = (distance - target.collision_radius - actor.config.collision_radius).max(0.0);
    if gap > 0.0 {
        let delta = Vec2::new(dx / distance * gap / 10.0, dy / distance * gap / 10.0);
        let EnemyBrain::DemonSkull(brain) = &actor.brain else {
            panic!("DemonSkull bite requires its owner")
        };
        let entry =
            99 + brain.body_pose as usize * 24 + native_demon_skull_facing(brain.body_heading_deg);
        let offset = Vec2::new(
            brain.body_offset.x,
            brain.body_offset.y + native_demon_skull_bob(brain.body_phase_deg),
        );
        for index in 0..10 {
            world.spawn_simple_death_effect(
                DeathEffectOwner::from(&*actor),
                context.tick,
                SimpleDeathEffectSpec {
                    alpha: fround(fround(0.1) * f64::from(index + 1)),
                    alpha_loss_per_tick: fround(
                        super::effects::NATIVE_FADE_ALPHA_LOSS * fround(0.35),
                    ),
                    atlas: EnemyEffectAtlas::Unholy,
                    entry,
                    blend_mode: EnemyEffectBlend::Normal,
                    kind: EnemyDeathEffectKind::Fade,
                    lifetime_ticks: 1000,
                    role: "discorporeal-bite-trail",
                    scale: actor.config.scale * 2.0,
                },
                DeathEffectOverrides {
                    position: Some(Vec2::new(
                        actor.position.x + offset.x,
                        actor.position.y + offset.y,
                    )),
                    ..Default::default()
                },
                order,
            );
            actor.position = context.geometry.resolve_movement(EnemyMovementRequest {
                actor_id: actor.id,
                position: actor.position,
                delta,
                purpose: EnemyMovementPurpose::Movement,
                radius: actor.config.collision_radius,
                requested_position: advance(actor.position, delta, 1.0),
            });
        }
    }
    if hypot(target.position.x - actor.position.x, target.position.y - actor.position.y)
        <= actor.config.collision_radius * 2.0
    {
        contact(
            world,
            actor.id,
            &target.id,
            EnemyPlayerDamageSource {
                position: actor.position,
                collision_radius: actor.config.collision_radius,
                reflectable_actor_id: Some(actor.id),
            },
            actor.config.primary_damage.unwrap_or(0.0),
            0.0,
            context.tick,
            1.0,
        );
    }
}

pub fn spit_demon_skull_fire(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let pitch = fround(1.0 + world.steering_rng.draw_scaled_float(0.1, true));
    world.emit_action_sound(
        DeathEffectOwner::from(&*actor),
        context.tick,
        EnemySound::UnholySpits,
        pitch,
    );
    let pitch = fround(1.0 + world.steering_rng.draw_scaled_float(0.1, true));
    world.emit_action_sound(
        DeathEffectOwner::from(&*actor),
        context.tick,
        EnemySound::ThrowFire,
        pitch,
    );
    let EnemyBrain::DemonSkull(brain) = &actor.brain else {
        panic!("DemonSkull spit requires its owner")
    };
    let capabilities = brain.capabilities;
    let mut travel = direction(actor.heading_deg, 200.0);
    let mut spread_direction = Vec2::ZERO;
    if let Some(target) = target_by_id(actor.target_player_id.as_deref(), context.players)
        .filter(|target| !target.summoned)
    {
        let mut distance = if capabilities & 8 != 0 { 300.0 } else { 150.0 };
        if world.steering_rng.draw_integer(4) == 1 {
            distance *= -0.25;
        }
        let velocity_heading =
            heading_degrees_from_vector(target.velocity_per_tick.x, target.velocity_per_tick.y);
        let offset = direction(
            velocity_heading + world.steering_rng.draw_scaled_float(10.0, true),
            distance,
        );
        let landing = advance(target.position, offset, 1.0);
        travel =
            Vec2::new(fround(landing.x - actor.position.x), fround(landing.y - actor.position.y));
        let spread = direction(
            heading_degrees_from_vector(offset.x, offset.y)
                + world.steering_rng.draw_scaled_float(20.0, true),
            1.0,
        );
        spread_direction = Vec2::new(spread.y, -spread.x);
        actor.heading_deg = heading_degrees_from_vector(travel.x, travel.y);
    }
    let distance = hypot(travel.x, travel.y);
    let spell = world.create_boss_spell(
        actor.id,
        context.tick,
        actor.config.extra_damage,
        actor.position,
        BossSpellKind::UnholySpit {
            origin: actor.position,
            travel,
            spread_direction,
            progress: 0.0,
            progress_per_tick: fround(200.0 / distance * 0.035),
            height: fround(distance / 5.0),
            spawn_fire: true,
            spawn_imps: capabilities & 8 != 0,
        },
        order,
    );
    world.boss_spells.push(spell);
}

pub fn step_demon_skull_spell(
    world: &mut EnemyWorld,
    mut spell: NativeBossSpell,
    context: &mut EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) -> Option<NativeBossSpell> {
    let source_age = spell.owner.age_ticks;
    let age = source_age + 1;
    match &mut spell.kind {
        BossSpellKind::MouthBeamSegment { .. } => {
            if age > 1 {
                None
            } else {
                spell.owner.age_ticks = age;
                Some(spell)
            }
        }
        BossSpellKind::UnholyBurst {
            frame_phase,
            frame_velocity,
            offset_y,
            offset_step_y,
            light_intensity,
        } => {
            *frame_phase = fround(*frame_phase + *frame_velocity);
            if *frame_phase > 14.0 {
                return None;
            }
            *frame_velocity = fround(*frame_velocity * fround(0.97));
            *offset_y = fround(*offset_y + *offset_step_y);
            *light_intensity = fround(*light_intensity - fround(0.02)).max(0.0);
            spell.owner.age_ticks = age;
            Some(spell)
        }
        BossSpellKind::GreenFire { fire, .. } => {
            let (patch, radial) = step_native_fire_patch((**fire).clone(), context.tick);
            if let Some(radial) = radial {
                for player in context.players {
                    if target_eligible(player)
                        && squared(spell.owner.position, player.position)
                            < radial.radius * radial.radius
                    {
                        contact(
                            world,
                            spell.owner.owner_actor_id,
                            &player.id,
                            EnemyPlayerDamageSource {
                                position: spell.owner.position,
                                collision_radius: 20.0,
                                reflectable_actor_id: None,
                            },
                            radial.amount,
                            radial.amount,
                            context.tick,
                            1.0,
                        );
                    }
                }
            }
            let patch = patch?;
            spell.owner.position = patch.position;
            **fire = patch;
            spell.owner.age_ticks = age;
            Some(spell)
        }
        BossSpellKind::UnholySpit {
            origin, travel, progress, progress_per_tick, height, ..
        } => {
            *progress = fround(*progress + *progress_per_tick);
            spell.owner.position = advance(*origin, *travel, *progress);
            spell.owner.age_ticks = age;
            spawn_unholy_spit_trail(
                world,
                DeathEffectOwner { id: spell.owner.owner_actor_id, position: spell.owner.position },
                context.tick,
                *progress,
                *height,
                order,
            );
            if *progress < 1.0 {
                Some(spell)
            } else {
                impact_spit(world, &spell, context, order);
                None
            }
        }
        BossSpellKind::EyeLaser { heading_deg, phase_deg, velocity } => {
            if age.is_multiple_of(5)
                && blocked_line(
                    context,
                    spell.owner.id,
                    spell.owner.position,
                    advance(spell.owner.position, *velocity, 5.0),
                    0,
                )
            {
                impact_eye(world, &spell, context, order);
                return None;
            }
            spell.owner.position = advance(spell.owner.position, *velocity, 1.0);
            spell.owner.age_ticks = age;
            *phase_deg =
                fround(*phase_deg + 5.0 + world.steering_rng.draw_scaled_float(5.0, false));
            let heading = *heading_deg;
            let mut retired =
                context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Bounds {
                    projectile_id: spell.owner.id,
                    position: spell.owner.position,
                    margin: 0.0,
                });
            for player in context.players {
                if player.summoned
                    || !target_eligible(player)
                    || squared(spell.owner.position, player.position) > 25.0 * 25.0
                {
                    continue;
                }
                impact_eye(world, &spell, context, order);
                retired = true;
            }
            spawn_unholy_eye_trail(
                world,
                owner(&spell),
                context.tick,
                heading,
                context.enhanced_effects,
                order,
            );
            if retired { None } else { Some(spell) }
        }
        _ => panic!("DemonSkull spell kind required"),
    }
}

fn impact_eye(
    world: &mut EnemyWorld,
    spell: &NativeBossSpell,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let pitch = fround(0.8 + world.steering_rng.draw_scaled_float(0.1, true));
    action_sound_gain(world, owner(spell), context.tick, EnemySound::FireballHit, pitch, 2.0);
    action_sound_gain(world, owner(spell), context.tick, EnemySound::ThrowFire, fround(0.6), 2.0);
    spawn_unholy_impact_ground(world, owner(spell), context.tick, true, order);
    spawn_rising_burst(world, spell, context.tick, true, order);
    let BossSpellKind::EyeLaser { velocity, .. } = spell.kind else { unreachable!() };
    for player in context.players {
        if player.summoned
            || !target_eligible(player)
            || squared(spell.owner.position, player.position) > 82.5 * 82.5
        {
            continue;
        }
        let event_id = contact(
            world,
            spell.owner.owner_actor_id,
            &player.id,
            EnemyPlayerDamageSource {
                position: spell.owner.position,
                collision_radius: 0.0,
                reflectable_actor_id: None,
            },
            spell.owner.damage * 0.5,
            spell.owner.damage * 0.5,
            context.tick,
            1.0,
        );
        world.projectile_knockbacks.push(EnemyProjectileKnockback {
            actor_id: spell.owner.owner_actor_id,
            delta: velocity,
            event_id,
            last_step_tick: context.tick,
            player_id: player.id.clone(),
            remaining_ticks: 3 + world.steering_rng.draw_integer(8) as u64,
        });
    }
}

fn impact_spit(
    world: &mut EnemyWorld,
    spell: &NativeBossSpell,
    context: &mut EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    spawn_unholy_impact_ground(world, owner(spell), context.tick, false, order);
    spawn_rising_burst(world, spell, context.tick, false, order);
    for (base, range) in [(0.8, 0.1), (1.0, 0.1), (1.0, 0.25)] {
        let pitch = fround(base + world.steering_rng.draw_scaled_float(range, true));
        world.emit_action_sound(owner(spell), context.tick, EnemySound::FireballHit, pitch);
    }
    let mut event =
        EnemyEvent::new(EnemyEventKind::EnemyStream, spell.owner.owner_actor_id, context.tick);
    event.stream = Some("trap".to_owned());
    event.source_position = Some(spell.owner.position);
    world.emit_event(event);
    let BossSpellKind::UnholySpit { spawn_fire, spawn_imps, spread_direction, .. } = spell.kind
    else {
        unreachable!()
    };
    if !spawn_fire {
        return;
    }
    if spread_direction == Vec2::ZERO {
        let scale = fround(2.0 + world.steering_rng.draw_scaled_float(0.2, false));
        spawn_green_fire(
            world,
            spell.owner.owner_actor_id,
            spell.owner.damage,
            context.tick,
            Vec2::new(spell.owner.position.x, spell.owner.position.y - 10.0),
            scale,
            20.0,
            true,
            order,
        );
        let magnitude = fround(10.0 + world.steering_rng.draw_scaled_float(10.0, false));
        let offset = world.steering_rng.draw_sign(magnitude);
        let scale = fround(1.2 + world.steering_rng.draw_scaled_float(0.2, false));
        spawn_green_fire(
            world,
            spell.owner.owner_actor_id,
            spell.owner.damage,
            context.tick,
            Vec2::new(spell.owner.position.x + offset, spell.owner.position.y + 5.0),
            scale,
            20.0,
            false,
            order,
        );
        return;
    }
    let length = 150.0 + world.steering_rng.draw_integer(101) as f64;
    let root = advance(spell.owner.position, spread_direction, -length * 0.5);
    let count = length / 20.0;
    for index in 0..=count.floor() as usize {
        let along = advance(root, spread_direction, index as f64 * 20.0);
        let offset = world.random_enemy_offset(10.0);
        let position = Vec2::new(along.x + offset.x, along.y - 10.0 + offset.y);
        let scale = fround(
            (1.0 + world.steering_rng.draw_scaled_float(0.2, false))
                * (1.0 + float::sin(index as f64 / count * std::f64::consts::PI) * 0.5),
        );
        spawn_green_fire(
            world,
            spell.owner.owner_actor_id,
            spell.owner.damage,
            context.tick,
            position,
            scale,
            20.0,
            squared(position, spell.owner.position) < 400.0,
            order,
        );
    }
    if !spawn_imps {
        return;
    }
    for index in 0..=(length / 40.0).floor() as usize {
        let position = advance(root, spread_direction, index as f64 * 40.0);
        if context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Point {
            projectile_id: spell.owner.id,
            position,
            radius: 10.0,
        }) {
            continue;
        }
        let mut intent = EnemySpawnIntent::new(
            EnemyToken::Imp,
            position,
            world.next_synthetic_spawn_intent_id,
            context.tick,
            0,
        );
        world.next_synthetic_spawn_intent_id += 1;
        intent.native_type_id = 2044;
        intent.green_imp_spit_damage = Some(spell.owner.damage);
        world.materialize_step_intents(&[intent], context, None, order);
    }
}

#[allow(clippy::too_many_arguments)] // Source GreenFire constructor receives these authored fields.
pub fn spawn_green_fire(
    world: &mut EnemyWorld,
    owner_actor_id: usize,
    damage: f64,
    tick: u64,
    position: Vec2,
    scale: f64,
    life: f64,
    glow: bool,
    order: &mut WorldPainterOrder,
) {
    let owner = world.create_boss_spell_owner(
        owner_actor_id,
        tick,
        damage,
        position,
        crate::painter::WorldManagerLane::Actor,
        order,
    );
    let mut fire = spawn_native_fire_patch(
        CreateNativeFirePatchOptions {
            atlas_phase_step: None,
            burn_damage: 0.0,
            damage,
            draw_alpha: None,
            fade_alpha: None,
            id: owner.id as u64,
            life: Some(life),
            native_type: FirePatchType::Fire,
            owner_id: format!("enemy:{owner_actor_id}"),
            painter_registration: Some(owner.painter_registration),
            position,
            scale: Some(scale),
            supplemental_contact: false,
            velocity: None,
            velocity_multiplier: None,
            world_key: "boneyard".to_owned(),
        },
        &mut world.steering_rng,
    );
    fire.atlas_phase = world.steering_rng.draw_scaled_float(32.0, false);
    fire.horizontal_sign = if world.steering_rng.draw_sign(1.0) < 0.0 { -1.0 } else { 1.0 };
    world.boss_spells.push(NativeBossSpell {
        owner: super::boss_spell::BossSpellOwner { position: fire.position, ..owner },
        kind: BossSpellKind::GreenFire { fire: Box::new(fire), glow },
    });
}

fn spawn_rising_burst(
    world: &mut EnemyWorld,
    source: &NativeBossSpell,
    tick: u64,
    eye: bool,
    order: &mut WorldPainterOrder,
) {
    let spell = world.create_boss_spell(
        source.owner.owner_actor_id,
        tick,
        0.0,
        source.owner.position,
        BossSpellKind::UnholyBurst {
            frame_phase: 0.0,
            frame_velocity: 0.625,
            offset_y: if eye { -22.5 } else { -15.0 },
            offset_step_y: if eye { fround(-1.725) } else { fround(-1.15) },
            light_intensity: 2.0,
        },
        order,
    );
    world.boss_spells.push(spell);
}

pub fn step_demon_skull_mouth_beam(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    context: &EnemyStepContext<'_>,
    power: f64,
    order: &mut WorldPainterOrder,
) {
    let unit = direction(actor.heading_deg, 1.0);
    let mut endpoint = advance(actor.position, unit, 1500.0);
    if let Some(bounds) = context.native_view_bounds {
        endpoint = clip_line_to_bounds(actor.position, endpoint, bounds)
            .map_or(actor.position, |(_, end)| end);
    }
    let terrain_end =
        context.geometry.line_obstruction(actor.position, endpoint, 0x380).unwrap_or(endpoint);
    let obstructed = squared(terrain_end, endpoint) > 1e-8;
    let end = if obstructed { Vec2::new(terrain_end.x, terrain_end.y - 20.0) } else { terrain_end };
    if obstructed {
        let scale = fround(1.5 + world.steering_rng.draw_scaled_float(1.5, false));
        let offset = world.random_enemy_offset(50.0);
        let rotation = world.steering_rng.draw_scaled_float(360.0, false);
        world.spawn_simple_death_effect(
            DeathEffectOwner::from(actor),
            context.tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: fround(0.01),
                atlas: EnemyEffectAtlas::BadGuys,
                entry: 15,
                blend_mode: EnemyEffectBlend::Add,
                kind: EnemyDeathEffectKind::FadeAdditive,
                lifetime_ticks: 1000,
                role: "mouth-beam-wall-contact",
                scale,
            },
            DeathEffectOverrides {
                position: Some(Vec2::new(end.x + offset.x, end.y + offset.y)),
                presentation_owner: Some(DeathPresentationOwner::LateWorldOverlay),
                tint: Some(UNHOLY_GREEN),
                rotation_deg: Some(rotation),
                ..Default::default()
            },
            order,
        );
    }
    let distance = hypot(end.x - actor.position.x, end.y - actor.position.y);
    let side = Vec2::new(-unit.y * 48.0, unit.x * 48.0);
    let polygon = [
        advance(actor.position, side, 1.0),
        advance(actor.position, side, -1.0),
        advance(end, side, -1.0),
        advance(end, side, 1.0),
    ];
    if context.tick.is_multiple_of(5) {
        let targets = world.mouth_targets(context.players, context.puppet_targets, Some(actor.id));
        let rows: Vec<_> = targets.iter().map(|target| target.target.clone()).collect();
        for target in native_primary_polygon_targets(u32::MAX, &polygon, &rows) {
            let offset = world.random_enemy_offset(10.0);
            let scale = (1.0 + world.steering_rng.draw_scaled_float(0.5, false)) * 2.0;
            world.spawn_simple_death_effect(
                DeathEffectOwner::from(actor),
                context.tick,
                SimpleDeathEffectSpec {
                    alpha: 1.0,
                    alpha_loss_per_tick: fround(0.2),
                    atlas: EnemyEffectAtlas::Unholy,
                    entry: 2,
                    blend_mode: EnemyEffectBlend::Add,
                    kind: EnemyDeathEffectKind::FadeAdditive,
                    lifetime_ticks: 1000,
                    role: "mouth-beam-contact",
                    scale,
                },
                DeathEffectOverrides {
                    opacity_timer: Some(power.min(1.0) * 2.0),
                    painter_sort_bias: Some(10.0),
                    position: Some(Vec2::new(
                        target.position.x + offset.x,
                        target.position.y - 10.0 + offset.y,
                    )),
                    tint: Some(UNHOLY_GREEN),
                    ..Default::default()
                },
                order,
            );
            let hit_strength = world.steering_rng.draw_scaled_float(1.0, false);
            let amount = actor.config.tertiary_damage / 5.0;
            if let Some(id) = target.id.strip_prefix("enemy:") {
                let mut damage = EnemyDamageRequest::new(
                    id.parse().expect("enemy target has its numeric actor ID"),
                    amount,
                    context.tick,
                    None,
                );
                damage.has_magic_damage = true;
                damage.enhanced_effects = Some(context.enhanced_effects);
                damage.magic = true;
                damage.hit_strength = hit_strength;
                world.damage(damage, Some(order));
            } else if let Some(id) = target.id.strip_prefix("target:") {
                contact(
                    world,
                    actor.id,
                    id,
                    EnemyPlayerDamageSource {
                        position: actor.position,
                        collision_radius: actor.config.collision_radius,
                        reflectable_actor_id: Some(actor.id),
                    },
                    0.0,
                    amount,
                    context.tick,
                    hit_strength,
                );
            } else if let Some(row) = targets.iter().find(|row| row.target.id == target.id) {
                world.hit_puppet(row, context.tick, hit_strength);
            }
            if context.tick.is_multiple_of(25) {
                let scale = 1.0 + world.steering_rng.draw_scaled_float(0.5, false);
                spawn_green_fire(
                    world,
                    actor.id,
                    amount * 0.25,
                    context.tick,
                    target.position,
                    scale,
                    1.0,
                    true,
                    order,
                );
            }
        }
    }
    spawn_demon_skull_beam_segments(world, actor, context.tick, distance, power, order);
}

#[allow(clippy::too_many_arguments)] // The source contact carries both damage lanes and its exact reflector root.
fn contact(
    world: &mut EnemyWorld,
    actor_id: usize,
    player_id: &str,
    source: EnemyPlayerDamageSource,
    physical: f64,
    magic: f64,
    tick: u64,
    strength: f64,
) -> usize {
    let mut event = EnemyEvent::new(EnemyEventKind::AttackMarker, actor_id, tick);
    event.target_player_id = Some(Some(player_id.to_owned()));
    let id = world.emit_event(event);
    let mut damage = EnemyPlayerDamage::new(actor_id, id, player_id.to_owned());
    damage.physical_damage = physical;
    damage.magic_damage = magic;
    damage.hit_strength = Some(strength);
    damage.source = Some(Some(source));
    world.player_damage.push(damage);
    id
}

pub(super) fn flash(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    tick: u64,
    screen: EnemyScreenFlash,
) {
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyScreenFlash, actor.id, tick);
    event.source_position = Some(actor.position);
    event.screen_flash = Some(screen);
    world.emit_event(event);
}
fn action_sound_gain(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    sound: EnemySound,
    pitch: f64,
    gain: f64,
) {
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyActionSound, owner.id, tick);
    event.source_position = Some(owner.position);
    event.sound = Some(sound);
    event.pitch = Some(pitch);
    event.gain_scale = Some(gain);
    world.emit_event(event);
}
fn owner(spell: &NativeBossSpell) -> DeathEffectOwner {
    DeathEffectOwner { id: spell.owner.owner_actor_id, position: spell.owner.position }
}
fn direction(heading: f64, length: f64) -> Vec2 {
    let angle = heading * std::f64::consts::PI / 180.0;
    Vec2::new(fround(float::sin(angle) * length), fround(-float::cos(angle) * length))
}
fn advance(point: Vec2, delta: Vec2, distance: f64) -> Vec2 {
    Vec2::new(fround(point.x + delta.x * distance), fround(point.y + delta.y * distance))
}
fn squared(a: Vec2, b: Vec2) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}
fn blocked_line(
    context: &EnemyStepContext<'_>,
    id: usize,
    start: Vec2,
    end: Vec2,
    mask: u32,
) -> bool {
    context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Line {
        projectile_id: id,
        start,
        end,
        native_exclusion_mask: mask,
    })
}
