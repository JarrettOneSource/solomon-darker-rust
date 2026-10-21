//! Skeleton contact actions, pike restraint and airborne recoil. The claw program uses
//! inclusive circular markers; weapon and pike markers deliberately use different edges.

use super::combat::{attack_marker, direct_contact_player_damage, direct_player_damage};
use super::config::EnemyFamilyConfig;
use super::damage::detach_skeleton_pike;
use super::model::*;
use super::movement::{move_toward_target, staff_attack_speed};
use super::path::{RadialDirection, TangentDirection};
use super::targeting::{
    target_by_id, target_eligible, target_within_attack_reach, track_enemy_action_heading,
};
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;

const WEAPON_POSES: [f64; 25] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 2.0, 2.0,
    2.0, 2.0, 1.0, 1.0, 1.0, 1.0,
];
const PIKE_POSES: [f64; 13] = [1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 1.0];
const CLAW_ARMORED: [f64; 8] = [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
const CLAW_UNARMORED: [f64; 8] = [4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0];

pub fn step_skeleton(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &mut EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Skeleton(mut brain) = actor.brain.clone() else {
        panic!("Skeleton brain required");
    };
    if actor.target_player_id.is_none() {
        reset_skeleton(&mut brain);
        actor.brain = EnemyBrain::Skeleton(brain);
        move_toward_target(
            world,
            actor,
            context,
            RadialDirection::Approach,
            TangentDirection::Hold,
        );
        return;
    }
    let armored = matches!(actor.config.family, EnemyFamilyConfig::Skeleton { armor: true, .. });
    let claw_poses = if armored { &CLAW_ARMORED } else { &CLAW_UNARMORED };
    if brain.phase != SkeletonPhase::Attack {
        if target_within_attack_reach(actor, context.players, 36.0) {
            actor.body_pose = match brain.action {
                SkeletonAction::Claw => claw_poses[0],
                SkeletonAction::Pike => PIKE_POSES[0],
                SkeletonAction::Weapon => WEAPON_POSES[0],
            };
            brain.action_progress = 0.0;
            brain.contact_target_player_id = actor.target_player_id.clone();
            brain.marker_emitted = false;
            brain.phase = SkeletonPhase::Attack;
            actor.loot_seed = world.draw_loot_seed(1_000_000, &mut context.roll_loot_seed);
            actor.brain = EnemyBrain::Skeleton(brain);
        } else {
            move_toward_target(
                world,
                actor,
                context,
                RadialDirection::Approach,
                TangentDirection::Hold,
            );
        }
        return;
    }
    track_enemy_action_heading(actor, context.players);
    let previous = brain.action_progress;
    match brain.action {
        SkeletonAction::Claw => {
            let raw = previous + 0.125 * staff_attack_speed(actor);
            let completed = raw > 7.0;
            let wrapped = if completed { raw - 8.0 } else { raw };
            for marker in [4.0, 8.0] {
                let crossed = if completed {
                    marker >= previous || marker <= wrapped
                } else {
                    previous <= marker && marker <= wrapped
                };
                if !crossed {
                    continue;
                }
                brain.vertical_velocity =
                    fround(-(world.steering_rng.draw_scaled_float(1.0, false) + 1.0));
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
                    36.0,
                    event,
                );
                brain.marker_emitted = true;
            }
            if completed {
                actor.body_pose = claw_poses[0];
                complete_skeleton_action(&mut brain);
            } else {
                actor.body_pose = body_pose(claw_poses, raw);
                brain.action_progress = raw;
            }
        }
        SkeletonAction::Weapon => {
            let progress = previous + 0.25 * staff_attack_speed(actor);
            for marker in [9.0, 20.0] {
                if previous >= marker || progress < marker {
                    continue;
                }
                brain.vertical_velocity =
                    fround(-(world.steering_rng.draw_scaled_float(1.0, false) + 1.5));
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
                    36.0,
                    event,
                );
                brain.marker_emitted = true;
            }
            if progress > 24.0 {
                actor.body_pose = WEAPON_POSES[0];
                complete_skeleton_action(&mut brain);
            } else {
                actor.body_pose = body_pose(&WEAPON_POSES, progress);
                brain.action_progress = progress;
            }
        }
        SkeletonAction::Pike => {
            let progress = previous + 0.125 * staff_attack_speed(actor);
            if !brain.marker_emitted && progress >= 2.0 {
                let event = attack_marker(
                    world,
                    actor,
                    context.tick,
                    brain.contact_target_player_id.clone(),
                    order,
                );
                brain.vertical_velocity =
                    fround(-(world.steering_rng.draw_scaled_float(0.5, false) + 1.0));
                if let Some(target) =
                    target_by_id(brain.contact_target_player_id.as_deref(), context.players)
                    && target_eligible(target)
                {
                    if !target.summoned {
                        brain.pike = Some(SkeletonPike {
                            player_id: target.id.clone(),
                            position: target.position,
                            distance: fround(actor.config.scale * 103.0),
                        });
                    }
                    direct_player_damage(world, actor, Some(&target.id), event);
                }
                brain.marker_emitted = true;
            }
            if progress > 12.0 {
                complete_skeleton_action(&mut brain);
            } else {
                actor.body_pose = body_pose(&PIKE_POSES, progress);
                brain.action_progress = progress;
            }
        }
    }
    actor.brain = EnemyBrain::Skeleton(brain);
}

#[must_use]
pub fn body_pose(poses: &[f64], progress: f64) -> f64 {
    assert!(
        progress.is_finite() && progress >= 0.0,
        "action progress must be finite and nonnegative"
    );
    poses[(progress.floor() as usize).min(poses.len() - 1)]
}

pub fn reset_skeleton(brain: &mut SkeletonBrain) {
    if brain.phase != SkeletonPhase::Approach {
        complete_skeleton_action(brain);
    }
}
fn complete_skeleton_action(brain: &mut SkeletonBrain) {
    brain.action_progress = 0.0;
    brain.contact_target_player_id = None;
    brain.marker_emitted = false;
    brain.phase = SkeletonPhase::Approach;
}

pub fn step_latched_skeleton(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
) {
    let EnemyBrain::Skeleton(brain) = &actor.brain else {
        return;
    };
    let Some(pike) = brain.pike.clone() else {
        return;
    };
    let effect = context.ability_effect(actor.id);
    let target = target_by_id(Some(&pike.player_id), context.players);
    let Some(target) = target.filter(|target| target_eligible(target) && !target.summoned) else {
        detach_skeleton_pike(actor);
        return;
    };
    if actor.target_player_id.is_none() || effect.dazzle_ticks > 0 || effect.disrupted_ticks > 0 {
        detach_skeleton_pike(actor);
        return;
    }
    let dx = fround(target.position.x - actor.position.x);
    let dy = fround(target.position.y - actor.position.y);
    let distance = fround(hypot(dx, dy));
    actor.heading_deg = heading_degrees_from_vector(dx, dy);
    if distance >= fround(pike.distance * 1.5) {
        detach_skeleton_pike(actor);
        return;
    }
    // Native pike restraint writes the player root directly, bypassing world collision.
    let factor = if distance == 0.0 { 0.0 } else { fround(pike.distance / distance) };
    let position = if distance == pike.distance {
        target.position
    } else {
        Vec2::new(
            fround(actor.position.x + fround(dx * factor)),
            fround(actor.position.y + fround(dy * factor)),
        )
    };
    world.player_positions.insert(pike.player_id.clone(), position);
    let EnemyBrain::Skeleton(brain) = &mut actor.brain else { unreachable!() };
    brain.pike = Some(SkeletonPike { position, ..pike });
}

pub fn step_skeleton_recoil(actor: &mut EnemyActor) {
    if actor.config.base_speed * actor.staff_movement_factor == 0.0 {
        return;
    }
    let (offset, velocity) = match &mut actor.brain {
        EnemyBrain::Skeleton(brain) => (&mut brain.vertical_offset, &mut brain.vertical_velocity),
        EnemyBrain::Archer(brain) => (&mut brain.vertical_offset, &mut brain.vertical_velocity),
        EnemyBrain::Mage(brain) => (&mut brain.vertical_offset, &mut brain.vertical_velocity),
        _ => return,
    };
    if *offset == 0.0 && *velocity == 0.0 {
        return;
    }
    let next = fround(*offset + *velocity);
    if next > 0.0 {
        *offset = 0.0;
        *velocity = 0.0;
    } else {
        *offset = next;
        *velocity = fround(*velocity + 0.3499999940395355);
    }
}
