//! Zombie beat timing and the separately seeded body articulation from `enemies/zombie.ts`.
use super::combat::{attack_marker, direct_contact_player_damage};
use super::config::{EnemyFamilyConfig, desaturate_color};
use super::effects::{
    DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec, radial_vector,
};
use super::model::*;
use super::movement::{move_toward_target, positive_modulo, staff_attack_speed};
use super::path::{RadialDirection, TangentDirection};
use super::targeting::{target_within_attack_reach, track_enemy_action_heading};
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float::fround;
use crate::painter::WorldPainterOrder;
use crate::rng::next_xorshift_random;

pub fn step_zombie(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Zombie(source) = actor.brain.clone() else {
        panic!("Zombie brain required");
    };
    let mut brain = source.clone();
    if actor.target_player_id.is_none() {
        reset_zombie(&mut brain);
        actor.brain = EnemyBrain::Zombie(brain);
        move_toward_target(
            world,
            actor,
            context,
            RadialDirection::Approach,
            TangentDirection::Hold,
        );
    } else if brain.phase == ZombiePhase::Knockback {
        brain.phase_ticks_remaining = brain.phase_ticks_remaining.saturating_sub(1);
        if brain.phase_ticks_remaining == 0 {
            clear_zombie_action(&mut brain);
        }
        actor.brain = EnemyBrain::Zombie(brain);
    } else if brain.phase == ZombiePhase::Swipe {
        track_enemy_action_heading(actor, context.players);
        let previous = brain.action_progress;
        let next = previous
            + brain.action_rate * if brain.impact_state_ticks_remaining == 0 { 2.0 } else { 1.0 };
        let mut impact = brain.impact_state_ticks_remaining.saturating_sub(1);
        if !brain.marker_emitted && previous < 100.0 && next >= 100.0 {
            let event = attack_marker(
                world,
                actor,
                context.tick,
                brain.contact_target_player_id.clone(),
                order,
            );
            direct_contact_player_damage(
                world,
                actor,
                brain.contact_target_player_id.as_deref(),
                context.players,
                48.0,
                event,
            );
            brain.marker_emitted = true;
            impact = 50;
        }
        if next >= 125.0 {
            brain.action_progress = 0.0;
            brain.action_rate = 0.0;
            brain.contact_target_player_id = None;
            brain.impact_state_ticks_remaining = impact;
            brain.marker_emitted = false;
            brain.phase = ZombiePhase::Knockback;
            brain.phase_ticks_remaining = impact;
        } else {
            if previous < 80.0 {
                actor.gait_pose = positive_modulo(actor.gait_pose - 0.025, 8.0);
            }
            brain.action_progress = next;
            brain.impact_state_ticks_remaining = impact;
        }
        actor.brain = EnemyBrain::Zombie(brain);
    } else if target_within_attack_reach(actor, context.players, 48.0) {
        brain.action_rate = (0.9 + world.draw_unit() * 0.25) * staff_attack_speed(actor);
        brain.action_progress = 0.0;
        brain.attack_side = if brain.attack_side == 0 { 1 } else { 0 };
        brain.contact_target_player_id = actor.target_player_id.clone();
        brain.impact_state_ticks_remaining = 0;
        brain.marker_emitted = false;
        brain.phase = ZombiePhase::Swipe;
        actor.brain = EnemyBrain::Zombie(brain);
    } else {
        move_toward_target(
            world,
            actor,
            context,
            RadialDirection::Approach,
            TangentDirection::Hold,
        );
    }
    let EnemyBrain::Zombie(stepped) = &mut actor.brain else { unreachable!() };
    advance_zombie_visual(&source, stepped);
}

pub fn reset_zombie(brain: &mut ZombieBrain) {
    if brain.phase != ZombiePhase::Approach {
        clear_zombie_action(brain);
    }
}
fn clear_zombie_action(brain: &mut ZombieBrain) {
    brain.action_progress = 0.0;
    brain.action_rate = 0.0;
    brain.contact_target_player_id = None;
    brain.impact_state_ticks_remaining = 0;
    brain.marker_emitted = false;
    brain.phase = ZombiePhase::Approach;
    brain.phase_ticks_remaining = 0;
}

fn advance_zombie_visual(source: &ZombieBrain, stepped: &mut ZombieBrain) {
    let mut rng = source.visual_rng_state;
    let mut random = || next_xorshift_random(&mut rng);
    let mut body = source.body_phase_deg;
    let mut head = source.head_phase_deg;
    let mut head_rotation = source.head_base_rotation_deg;
    let mut rear_rotation = source.rear_arm_base_rotation_deg;
    let mut front_rotation = source.front_arm_base_rotation_deg;
    let mut swing = source.action_swing;
    let mut offset = source.vertical_offset;
    let mut velocity = source.vertical_velocity;
    if source.phase != ZombiePhase::Swipe {
        body += random() * 1.5;
        head += random() * 0.75;
        if (random() * 100.0).floor() == 5.0 {
            if (random() * 2.0).floor() == 1.0 {
                rear_rotation = random() * 45.0;
            } else {
                front_rotation = random() * 45.0;
            }
            if (random() * 15.0).floor() == 3.0 {
                head_rotation = (random() * 2.0 - 1.0) * 65.0;
            }
            if source.body_type == 3 {
                rear_rotation /= 3.0;
                front_rotation /= 3.0;
                head_rotation *= 0.5;
            }
        }
    } else {
        let next = source.action_progress
            + source.action_rate * if source.impact_state_ticks_remaining == 0 { 2.0 } else { 1.0 };
        swing +=
            source.action_rate * if source.impact_state_ticks_remaining == 0 { 0.5 } else { 0.25 };
        if source.action_progress < 50.0 && next >= 50.0 {
            offset = -0.1;
            velocity = -(3.0 + random() * 0.5);
        }
        if source.action_progress < 100.0 && next >= 100.0 {
            body = 0.0;
            swing = 0.0;
            offset = -0.1;
            velocity = -(1.0 + random() * 0.5);
        }
    }
    if source.phase != ZombiePhase::Swipe && stepped.phase == ZombiePhase::Swipe {
        swing = 0.0;
    }
    if offset < 0.0 {
        offset += velocity;
        velocity += 0.4;
        if offset > 0.0 {
            offset = 0.0;
        }
    }
    stepped.action_swing = swing;
    stepped.angular_offset_deg = source.angular_offset_deg * 0.95;
    stepped.body_phase_deg = body;
    stepped.front_arm_base_rotation_deg = front_rotation;
    stepped.head_base_rotation_deg = head_rotation;
    stepped.head_phase_deg = head;
    stepped.rear_arm_base_rotation_deg = rear_rotation;
    stepped.vertical_offset = offset;
    stepped.vertical_velocity = velocity;
    stepped.visual_rng_state = rng;
}

pub fn spawn_rotten_zombie_particle(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    if actor.life_state != EnemyLifeState::Alive
        || !matches!(actor.config.family, EnemyFamilyConfig::Zombie { rotten: true, .. })
        || world.steering_rng.draw_integer(75) != 3
    {
        return;
    }
    let radius = world.steering_rng.draw_scaled_float(8.0, false);
    let displacement = radial_vector(world.steering_rng.draw_scaled_float(360.0, false), radius);
    let entry = if world.steering_rng.draw_integer(2) == 1 { 10 } else { 11 };
    let phase_speed = fround(1.0 + world.steering_rng.draw_scaled_float(1.0, false));
    let speed = fround(
        0.10000000149011612
            + world.steering_rng.draw_scaled_float(0.25 - 0.10000000149011612, false),
    );
    let velocity = radial_vector(world.steering_rng.draw_scaled_float(360.0, false), speed);
    let scale = fround(1.0 + world.steering_rng.draw_scaled_float(2.0, false));
    let color = desaturate_color([fround(0.1), fround(0.3), fround(0.1), 1.0], fround(0.65));
    let tint = ((color[0] * 255.0).trunc() as u32) << 16
        | ((color[1] * 255.0).trunc() as u32) << 8
        | (color[2] * 255.0).trunc() as u32;
    world.spawn_simple_death_effect(
        DeathEffectOwner::from(actor),
        tick,
        SimpleDeathEffectSpec {
            alpha: 1.0,
            alpha_loss_per_tick: 0.0,
            atlas: EnemyEffectAtlas::BadGuys,
            blend_mode: EnemyEffectBlend::Normal,
            entry,
            kind: EnemyDeathEffectKind::MoveFadeSin,
            lifetime_ticks: 181,
            role: "zombie-rotten-particle",
            scale,
        },
        DeathEffectOverrides {
            frame_velocity: Some(phase_speed),
            position: Some(Vec2::new(
                fround(actor.position.x + displacement.x),
                fround(actor.position.y - 15.0 + displacement.y),
            )),
            presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
            tint: Some(tint),
            velocity: Some(Vec2::new(fround(velocity.x), fround(velocity.y))),
            ..DeathEffectOverrides::default()
        },
        order,
    );
}
