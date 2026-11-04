//! Demon planted-foot controller and bomb action from `enemies/demon.ts` and
//! `boneyard-demon-articulation.ts`. Its articulation hash does not consume any RNG stream.
use super::combat::attack_marker;
use super::effects::DeathEffectOwner;
use super::model::*;
use super::movement::{move_toward_target, positive_modulo, staff_attack_speed};
use super::path::{RadialDirection, TangentDirection};
use super::projectile_emission::{SpawnEnemyProjectileOptions, spawn_projectile};
use super::projectiles::EnemyProjectileKind;
use super::ranged::native_enemy_projectile_velocity;
use super::targeting::target_within_attack_reach;
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::painter::WorldPainterOrder;

#[must_use]
pub fn create_demon_articulation(
    actor_id: usize,
    spawn_tick: u64,
    position: Vec2,
    heading: f64,
    scale: f64,
) -> DemonArticulation {
    let settled = |front| {
        let point = demon_extremity_target(position, heading, scale, front);
        DemonExtremity { current: point, lift_y: 0.0, phase: 1.0, start: point, target: point }
    };
    DemonArticulation {
        completed_steps: 0,
        front: settled(true),
        front_base_rotation_deg: 0.0,
        rear: settled(false),
        rear_base_rotation_deg: 1.0,
        step_interval_ticks: 150
            - (deterministic_unit(actor_id, spawn_tick, 0) * 75.0).floor() as u64,
    }
}

#[must_use]
pub fn demon_extremity_target(position: Vec2, heading: f64, scale: f64, front: bool) -> Vec2 {
    let facing = positive_modulo(((heading + 9.0) / 20.0).trunc(), 18.0);
    let radians = facing * 20.0 * std::f64::consts::PI / 180.0;
    let x = if front { 1.0 } else { -1.0 } * 12.0 * scale;
    let y = -30.0 * scale;
    Vec2::new(
        position.x + x * float::cos(radians) - y * float::sin(radians),
        position.y + x * float::sin(radians) + y * float::cos(radians),
    )
}

#[must_use]
pub fn demon_articulation_root(state: &DemonArticulation) -> Vec2 {
    Vec2::new(
        (state.front.current.x + state.rear.current.x) * 0.5,
        (state.front.current.y + state.front.lift_y + state.rear.current.y + state.rear.lift_y)
            * 0.5,
    )
}

pub fn snap_demon_root_to_extremities(actor: &mut EnemyActor) {
    let EnemyBrain::Demon(brain) = &actor.brain else {
        return;
    };
    let root = demon_articulation_root(&brain.articulation);
    let dx = root.x - actor.position.x;
    let dy = root.y - actor.position.y;
    if dx * dx + dy * dy > 30.0 * 30.0 {
        actor.position = root;
    }
}

pub fn step_demon(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Demon(mut brain) = actor.brain.clone() else {
        panic!("Demon brain required");
    };
    if actor.target_player_id.is_none() {
        reset_demon(&mut brain);
        actor.brain = EnemyBrain::Demon(brain);
        move_toward_target(
            world,
            actor,
            context,
            RadialDirection::Approach,
            TangentDirection::Hold,
        );
        advance_demon_articulation(actor, context.tick);
        return;
    }
    if brain.phase == DemonPhase::Bomb {
        let previous = brain.action_progress;
        let next = previous + 0.09375 * staff_attack_speed(actor);
        if !brain.marker_emitted && previous < 4.0 && next >= 4.0 {
            attack_marker(world, actor, context.tick, actor.target_player_id.clone(), order);
            let pitch = fround(1.0 + world.steering_rng.draw_scaled_float(0.1, true));
            world.emit_action_sound(
                DeathEffectOwner::from(&*actor),
                context.tick,
                EnemySound::SpitFire,
                pitch,
            );
            // Raw Anim_FireBurst rotation, angular magnitude and signed direction.
            world.steering_rng.draw_scaled_float(360.0, false);
            let magnitude = fround(0.5 + world.steering_rng.draw_scaled_float(1.0, false));
            world.steering_rng.draw_sign(magnitude);
            let root = demon_articulation_root(&brain.articulation);
            let bearing =
                positive_modulo(((actor.heading_deg + 10.0) / 20.0).round_ties_even(), 18.0) * 20.0;
            let forward = native_enemy_projectile_velocity(bearing, 35.0);
            let position = Vec2::new(fround(root.x + forward.x), fround(root.y + forward.y));
            spawn_projectile(
                world,
                actor,
                context.tick,
                EnemyProjectileKind::DemonBomb,
                actor.config.primary_damage.unwrap_or(0.0),
                SpawnEnemyProjectileOptions {
                    position: Some(position),
                    ..SpawnEnemyProjectileOptions::default()
                },
                context.enhanced_effects,
                order,
            );
            brain.marker_emitted = true;
        }
        if next > 8.0 {
            reset_demon(&mut brain);
        } else {
            brain.action_progress = next;
        }
        actor.brain = EnemyBrain::Demon(brain);
        return;
    }
    if target_within_attack_reach(actor, context.players, 180.0) {
        brain.action_progress = 0.0;
        brain.marker_emitted = false;
        brain.phase = DemonPhase::Bomb;
        actor.brain = EnemyBrain::Demon(brain);
        return;
    }
    move_toward_target(world, actor, context, RadialDirection::Approach, TangentDirection::Hold);
    advance_demon_articulation(actor, context.tick);
}

pub fn reset_demon(brain: &mut DemonBrain) {
    if brain.phase == DemonPhase::Approach {
        return;
    }
    brain.action_progress = 0.0;
    brain.marker_emitted = false;
    brain.phase = DemonPhase::Approach;
}

fn advance_demon_articulation(actor: &mut EnemyActor, tick: u64) {
    let EnemyBrain::Demon(brain) = &mut actor.brain else {
        return;
    };
    if brain.phase != DemonPhase::Approach || actor.config.scale <= 0.0 {
        return;
    }
    let state = &mut brain.articulation;
    let phase_tick = tick % state.step_interval_ticks;
    if phase_tick == 0 {
        replant_extremity(
            &mut state.front,
            demon_extremity_target(actor.position, actor.heading_deg, actor.config.scale, true),
        );
    }
    if phase_tick == state.step_interval_ticks / 2 {
        replant_extremity(
            &mut state.rear,
            demon_extremity_target(actor.position, actor.heading_deg, actor.config.scale, false),
        );
    }
    let front_completed = advance_extremity(&mut state.front);
    let rear_completed = advance_extremity(&mut state.rear);
    for completed in [front_completed, rear_completed] {
        if !completed {
            continue;
        }
        let channel = 1 + state.completed_steps * 2;
        let front = deterministic_unit(actor.id, actor.spawn_tick, channel) < 0.5;
        let rotation = -20.0 + deterministic_unit(actor.id, actor.spawn_tick, channel + 1) * 30.0;
        if front {
            state.front_base_rotation_deg = rotation;
        } else {
            state.rear_base_rotation_deg = rotation;
        }
        state.completed_steps += 1;
    }
}
fn replant_extremity(state: &mut DemonExtremity, target: Vec2) {
    state.lift_y = 0.0;
    state.phase = 0.0;
    state.start = state.current;
    state.target = target;
}
fn advance_extremity(state: &mut DemonExtremity) -> bool {
    if state.phase >= 1.0 {
        return false;
    }
    let raw = (state.phase + 0.015) * 1.06;
    let phase = raw.min(1.0);
    state.current = Vec2::new(
        state.start.x + phase * (state.target.x - state.start.x),
        state.start.y + phase * (state.target.y - state.start.y),
    );
    state.lift_y = -float::sin(phase * std::f64::consts::PI) * 6.0;
    state.phase = phase;
    raw > 1.0
}
fn deterministic_unit(actor_id: usize, spawn_tick: u64, channel: u64) -> f64 {
    let mut value = actor_id as u32
        ^ (spawn_tick as u32).wrapping_add(1).wrapping_mul(0x9e37_79b1)
        ^ (channel as u32).wrapping_add(1).wrapping_mul(0x85eb_ca6b);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    f64::from(value) / 4_294_967_296.0
}
