//! Spider world callbacks, Silk contacts and target-owned Cocoons from `enemies/spider.ts`.
use super::combat::{attack_marker, direct_player_damage};
use super::config::EnemyFamilyConfig;
use super::effects::{
    BouncerOptions, DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec,
};
use super::model::*;
use super::movement::{positive_modulo, route_native_enemy_goal};
use super::path::PathRecoveryRequest;
use super::silk::{EnemySilkActor, step_native_silk};
use super::spider_movement::*;
use super::targeting::{boneyard_enemy_collision_radius, target_by_id};
use super::webbed::{native_cocoon_position, step_native_webbed};
use super::world::{EnemyMovementPurpose, EnemyMovementRequest, EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::spells::targeting::{direction_from_heading, native_primary_cell_coordinate};

pub fn step_spider(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Spider(mut brain) = actor.brain.clone() else {
        panic!("Spider brain required");
    };
    let EnemyFamilyConfig::Spider { spit_webs, cocoon_health, suck_damage_per_second } =
        actor.config.family
    else {
        panic!("Spider config required");
    };
    let light_at = context.light_at.expect("Spider movement requires the native light sampler");
    let view = context.native_movement_view;
    let visible = view.is_none_or(|view| {
        view.cameras.iter().any(|camera| {
            actor.position.x + 100.0 >= camera.x
                && actor.position.x - 100.0 <= camera.x + camera.width
                && actor.position.y + 100.0 >= camera.y
                && actor.position.y - 100.0 <= camera.y + camera.height
        })
    });
    let illuminated = view.is_none() || light_at(actor.position) > 0.0;
    let clock = super::path::native_enemy_movement_clock(
        actor.id,
        context.tick,
        visible,
        illuminated,
        view.is_none_or(|view| view.enhanced_effects),
    );
    if !clock.due {
        return;
    }
    let status = actor.staff_movement_factor * context.ability_effect(actor.id).time_scale;
    if !clock.full {
        step_degraded_spider(actor, context, status, clock.cadence);
        return;
    }
    let target = target_by_id(actor.target_player_id.as_deref(), context.players);
    let target_state = target.map(|target| SpiderTarget {
        position: target.position,
        heading_deg: target.heading_deg.unwrap_or(0.0),
        radius: target.collision_radius,
        is_player: !target.summoned,
        web_severity: world.webbed_players.get(&target.id).map_or(0.0, |web| web.severity),
        velocity_per_tick: target.velocity_per_tick,
    });
    let speed = actor.config.base_speed;
    let mut delta = Vec2::ZERO;
    for _ in 0..clock.cadence {
        let movement_start = actor.position;
        let heading = actor.heading_deg;
        let position = actor.position;
        let wander = actor.path.wander_heading_deg;
        let mut route = |goal| {
            route_goal(actor, goal, target.map(|target| target.position), context, clock.cadence)
        };
        let result = move_native_spider(
            &brain,
            SpiderMovementRequest {
                route_goal: Some(&mut route),
                heading_deg: heading,
                position,
                speed,
                status_factor: status,
                light: light_at(position),
                target: target_state,
                wander_heading_deg: wander,
                spit_webs,
                cocoon_health,
                live_silk_count: world.silks.len(),
                shared_spit_ticks_remaining: world.spider_spit_ticks_remaining,
            },
            &mut world.steering_rng,
        );
        world.spider_spit_ticks_remaining = result.shared_spit_ticks_remaining;
        brain = result.state;
        brain.phase = SpiderPhase::Active;
        actor.position = result.position;
        actor.heading_deg = positive_modulo(result.heading_deg, 360.0);
        if let Some(silk) = result.silk {
            let id = world.next_projectile_id;
            world.next_projectile_id += 1;
            let painter_registration =
                world.register_projectile_painter(order, WorldManagerLane::Actor);
            world.silks.push(EnemySilkActor {
                id,
                owner_actor_id: actor.id,
                spawn_tick: context.tick,
                state: silk.state,
                native_registration_order: world.next_native_registration_order,
                native_cell_binding_order: world.next_native_cell_binding_order,
                painter_registration,
            });
            world.next_native_registration_order += 1;
            world.next_native_cell_binding_order += 1;
            let mut event =
                EnemyEvent::new(EnemyEventKind::EnemyActionSound, actor.id, context.tick);
            event.sound = Some(
                [EnemySound::ShootWeb1, EnemySound::ShootWeb2, EnemySound::ShootWeb3]
                    [silk.sound_index],
            );
            event.pitch = Some(silk.sound_pitch);
            event.gain_scale = Some(1.0);
            event.source_position = Some(silk.sound_position);
            world.emit_event(event);
        }
        if result.bite {
            if target_state.is_some_and(|target| target.is_player && target.web_severity > 2.0) {
                spawn_web_fragments(
                    world,
                    DeathEffectOwner { id: actor.id, position: movement_start },
                    context.tick,
                    "spider-grab-fragment",
                    context.enhanced_effects,
                    order,
                );
            }
            let event_id =
                attack_marker(world, actor, context.tick, actor.target_player_id.clone(), order);
            direct_player_damage(world, actor, actor.target_player_id.as_deref(), event_id);
            let sounds =
                result.bite_sounds.expect("a native Spider bite publishes its two sound choices");
            for (sound, pitch, gain) in [
                ([EnemySound::Bite1, EnemySound::Bite2][sounds[0]], 1.5, 1.0),
                ([EnemySound::MaggotSquish1, EnemySound::MaggotSquish2][sounds[1]], 0.8, 0.5),
            ] {
                let mut event =
                    EnemyEvent::new(EnemyEventKind::EnemyActionSound, actor.id, context.tick);
                event.sound = Some(sound);
                event.pitch = Some(pitch);
                event.gain_scale = Some(gain);
                event.source_position = Some(actor.position);
                world.emit_event(event);
            }
        }
        if result.suck
            && let Some(target) = &actor.target_player_id
        {
            let event_id = attack_marker(world, actor, context.tick, Some(target.clone()), order);
            let mut damage = EnemyPlayerDamage::new(actor.id, event_id, target.clone());
            damage.physical_damage = suck_damage_per_second * 0.25;
            world.player_damage.push(damage);
        }
        delta.x += result.goal.x - actor.position.x;
        delta.y += result.goal.y - actor.position.y;
    }
    let radius = if brain.attached { 5.0 } else { 15.0 };
    let position = context.geometry.resolve_movement(EnemyMovementRequest {
        actor_id: actor.id,
        delta,
        position: actor.position,
        purpose: EnemyMovementPurpose::Movement,
        radius,
        requested_position: Vec2::new(actor.position.x + delta.x, actor.position.y + delta.y),
    });
    actor.body.path.step_recovery(
        &mut world.steering_rng,
        PathRecoveryRequest {
            flanking_enabled: actor.body.config.flanking,
            requested_distance: hypot(delta.x, delta.y),
            status_factor: status,
            tick: context.tick,
            traveled_distance: hypot(
                position.x - actor.body.position.x,
                position.y - actor.body.position.y,
            ),
        },
    );
    actor.body_pose = brain.frame;
    actor.brain = EnemyBrain::Spider(brain);
    actor.position = position;
    actor.next_movement_tick = context.tick + clock.cadence;
    actor.last_movement_tick = Some(context.tick);
}

fn route_goal(
    actor: &mut EnemyActor,
    raw: Vec2,
    target: Option<Vec2>,
    context: &EnemyStepContext<'_>,
    cadence: u64,
) -> SpiderRouteGoal {
    let routed = route_native_enemy_goal(actor, raw, target, context, cadence);
    SpiderRouteGoal {
        goal: routed.goal,
        following_route: actor.path.route_ticks_remaining > 0,
        turn_around: routed.turn_around,
    }
}

fn step_degraded_spider(
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    status: f64,
    cadence: u64,
) {
    let target = target_by_id(actor.target_player_id.as_deref(), context.players);
    let speed = actor.config.base_speed * actor.config.chase_speed * status * cadence as f64 * 0.25;
    let direction = direction_from_heading(actor.id as f64 * 225.0);
    let raw = target.map_or(
        Vec2::new(actor.position.x + direction.x * speed, actor.position.y + direction.y * speed),
        |target| target.position,
    );
    let routed = route_goal(actor, raw, target.map(|target| target.position), context, cadence);
    let heading = heading_degrees_from_vector(
        routed.goal.x - actor.position.x,
        routed.goal.y - actor.position.y,
    );
    let facing = direction_from_heading(heading);
    let delta = Vec2::new(fround(facing.x * speed), fround(facing.y * speed));
    actor.position = context.geometry.resolve_movement(EnemyMovementRequest {
        actor_id: actor.id,
        delta,
        position: actor.position,
        purpose: EnemyMovementPurpose::Movement,
        radius: boneyard_enemy_collision_radius(actor),
        requested_position: Vec2::new(actor.position.x + delta.x, actor.position.y + delta.y),
    });
    actor.heading_deg = heading;
    actor.next_movement_tick = context.tick + cadence;
    actor.last_movement_tick = Some(context.tick);
}

pub fn step_cocoon_actor(
    world: &EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
) {
    let EnemyBrain::Cocoon(brain) = &mut actor.brain else {
        panic!("Cocoon requires its target owner");
    };
    let owner = target_by_id(brain.owner_player_id.as_deref(), context.players);
    let web = brain.owner_player_id.as_ref().and_then(|owner| world.webbed_players.get(owner));
    if !owner.is_some_and(|owner| owner.alive && owner.connected)
        || !web.is_some_and(|web| web.severity >= 3.0)
    {
        actor.body.life_state = EnemyLifeState::Dying;
        brain.phase = CocoonPhase::Death;
        return;
    }
    let owner = owner.unwrap();
    actor.body.position = native_cocoon_position(owner.position, owner.heading_deg.unwrap_or(0.0));
    brain.owner_position = owner.position;
}

impl EnemyWorld {
    pub fn step_spider_webs(&mut self, context: &EnemyStepContext<'_>) {
        self.spider_spit_ticks_remaining = self.spider_spit_ticks_remaining.saturating_sub(1);
        let actors = &self.actors;
        self.webbed_players.retain(|id, web| {
            // Full restraint belongs to a live Cocoon, unlike partial Silk webs.
            let restraint_valid = web.severity < 3.0
                || actors.iter().any(|actor| {
                    actor.life_state == EnemyLifeState::Alive
                        && matches!(&actor.brain, EnemyBrain::Cocoon(brain)
                            if brain.owner_player_id.as_deref() == Some(id.as_str()))
                });
            if !restraint_valid {
                return false;
            }
            let Some(player) = target_by_id(Some(id), context.players)
                .filter(|player| player.alive && player.connected)
            else {
                return false;
            };
            if let Some(next) = step_native_webbed(*web, player.velocity_per_tick) {
                *web = next;
                true
            } else {
                false
            }
        });
    }
    pub fn step_silks(&mut self, context: &EnemyStepContext<'_>, order: &mut WorldPainterOrder) {
        let mut actors = std::mem::take(&mut self.silks);
        actors.retain_mut(|actor| {
            if actor.spawn_tick == context.tick {
                return true;
            }
            let ended = actor.state.ended;
            let previous = actor.state.position;
            if !step_native_silk(&mut actor.state) {
                return false;
            }
            let state = &mut actor.state;
            let mut hit = false;
            if !ended && !state.age_ticks.is_multiple_of(4) {
                for player in context.players {
                    if !player.alive || !player.connected || player.summoned {
                        continue;
                    }
                    let dx = state.position.x - player.position.x;
                    let dy = state.position.y - player.position.y;
                    if dx * dx + dy * dy >= player.collision_radius * player.collision_radius {
                        continue;
                    }
                    hit = true;
                    let mut event = EnemyEvent::new(
                        EnemyEventKind::ProjectileImpact,
                        actor.owner_actor_id,
                        context.tick,
                    );
                    event.projectile_id = Some(actor.id);
                    event.target_player_id = Some(Some(player.id.clone()));
                    let event_id = self.emit_event(event);
                    let mut damage =
                        EnemyPlayerDamage::new(actor.owner_actor_id, event_id, player.id.clone());
                    // Native Silk clears its damage source; the parent owns only the event.
                    damage.source = Some(None);
                    damage.physical_damage = 1.0;
                    damage.webbed_strength = Some(state.cocoon_health);
                    self.player_damage.push(damage);
                    spawn_silk_impact(
                        self,
                        actor.owner_actor_id,
                        state.position,
                        context.tick,
                        context.enhanced_effects,
                        order,
                    );
                }
            }
            if !hit {
                let mut binding = actor.native_cell_binding_order;
                if native_primary_cell_coordinate(previous.x)
                    != native_primary_cell_coordinate(state.position.x)
                    || native_primary_cell_coordinate(previous.y)
                        != native_primary_cell_coordinate(state.position.y)
                {
                    binding = self.next_native_cell_binding_order;
                    self.next_native_cell_binding_order += 1;
                }
                actor.native_cell_binding_order = binding;
            }
            !hit
        });
        self.silks = actors;
    }
}

fn spawn_silk_impact(
    world: &mut EnemyWorld,
    id: usize,
    position: Vec2,
    tick: u64,
    enhanced_effects: bool,
    order: &mut WorldPainterOrder,
) {
    let owner = DeathEffectOwner { id, position };
    let rotation = world.draw_unit() * 360.0;
    world.spawn_simple_death_effect(
        owner,
        tick,
        SimpleDeathEffectSpec {
            alpha: 1.0,
            alpha_loss_per_tick: fround(fround(0.1) * fround(0.15)),
            atlas: EnemyEffectAtlas::DeadHawg,
            entry: 14,
            kind: EnemyDeathEffectKind::FadePerspective,
            lifetime_ticks: 267,
            blend_mode: EnemyEffectBlend::Add,
            role: "silk-contact",
            scale: 1.0,
        },
        DeathEffectOverrides {
            opacity_timer: Some(4.0),
            rotation_deg: Some(rotation),
            ..DeathEffectOverrides::default()
        },
        order,
    );
    spawn_web_fragments(world, owner, tick, "silk-fragment", enhanced_effects, order);
}
fn spawn_web_fragments(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    role: &str,
    enhanced_effects: bool,
    order: &mut WorldPainterOrder,
) {
    let mut heading = world.draw_unit() * 360.0;
    for _ in 0..5 {
        world.spawn_bouncer(
            owner,
            tick,
            |_| 27,
            role,
            |world| {
                let radians = heading * std::f64::consts::PI / 180.0;
                let velocity = Vec2::new(float::sin(radians) * 1.5, -float::cos(radians));
                let radius = 15.0 + world.draw_unit() * 10.0;
                heading += 72.0 + (world.draw_unit() * 2.0 - 1.0) * 10.0;
                BouncerOptions {
                    opacity_timer: Some(1.5),
                    scale_y: Some(0.75),
                    position: Some(Vec2::new(
                        owner.position.x + velocity.x * (radius + 2.0),
                        owner.position.y + velocity.y * radius,
                    )),
                    velocity: Some(velocity),
                    ..BouncerOptions::default()
                }
            },
            enhanced_effects,
            order,
        );
    }
}
