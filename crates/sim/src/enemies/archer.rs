//! Archer action scheduling and native tangential strafe from `skeleton-family.ts`
//! and `native-archer-strafe.ts`. Volley births retain their private action seed.
use super::combat::attack_marker;
use super::config::EnemyFamilyConfig;
use super::model::*;
use super::movement::{move_toward_target, staff_attack_speed, staff_movement_speed};
use super::path::{RadialDirection, TangentDirection};
use super::projectile_emission::emit_archer_volley;
use super::ranged::{RangedAttackRange, restore_native_range_easy_after_volley};
use super::skeleton::body_pose;
use super::targeting::{
    enemy_target_line_of_sight_is_clear, target_by_id, target_distance, track_enemy_action_heading,
};
use super::world::{EnemyMovementPurpose, EnemyMovementRequest, EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;

const ARCHER_SHOT_POSES: [f64; 17] =
    [3.0, 4.0, 5.0, 6.0, 7.0, 6.0, 7.0, 6.0, 7.0, 6.0, 7.0, 6.0, 7.0, 8.0, 8.0, 8.0, 8.0];

pub fn step_archer(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &mut EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    step_archer_action(world, actor, context, order);
    let EnemyBrain::Archer(mut brain) = actor.brain.clone() else {
        panic!("Archer brain required");
    };
    let target = target_by_id(actor.target_player_id.as_deref(), context.players);
    let enabled = matches!(actor.config.family, EnemyFamilyConfig::Archer { strafing: true, .. });
    let scalar =
        fround(actor.config.chase_speed * staff_movement_speed(actor) * actor.config.scale);
    let movement = step_archer_strafe(
        &mut brain.strafe,
        brain.phase == ArcherPhase::Attack,
        enabled,
        actor.heading_deg,
        scalar,
        actor.position,
        target.map(|target| target.position),
        |end| context.geometry.line_obstruction(actor.position, end, 0).is_none(),
    );
    actor.brain = EnemyBrain::Archer(brain);
    let Some(movement) = movement else {
        return;
    };
    actor.position = context.geometry.resolve_movement(EnemyMovementRequest {
        actor_id: actor.id,
        delta: movement.delta,
        position: actor.position,
        purpose: EnemyMovementPurpose::Movement,
        radius: actor.config.collision_radius,
        requested_position: Vec2::new(
            fround(actor.position.x + movement.delta.x),
            fround(actor.position.y + movement.delta.y),
        ),
    });
    let gait = fround(actor.gait_pose + movement.gait_advance);
    actor.gait_pose = if gait > 8.0 { fround(gait - 8.0) } else { gait };
    actor.last_movement_tick = Some(context.tick);
    actor.stride_phase_deg = fround(actor.stride_phase_deg + movement.stride_advance);
}

fn step_archer_action(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &mut EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Archer(mut brain) = actor.brain.clone() else {
        panic!("Archer brain required");
    };
    if actor.target_player_id.is_none() {
        reset_archer(&mut brain);
        actor.brain = EnemyBrain::Archer(brain);
        move_toward_target(
            world,
            actor,
            context,
            RadialDirection::Approach,
            TangentDirection::Hold,
        );
        return;
    }
    if brain.phase == ArcherPhase::Attack {
        track_enemy_action_heading(actor, context.players);
        let progress = brain.action_progress + 0.0843750015 * staff_attack_speed(actor);
        if !brain.marker_emitted && progress >= 13.0 {
            attack_marker(world, actor, context.tick, actor.target_player_id.clone(), order);
            let mut range = RangedAttackRange {
                range: brain.attack_range,
                range_easy_pending: brain.range_easy_pending,
            };
            restore_native_range_easy_after_volley(&mut range);
            brain.attack_range = range.range;
            brain.range_easy_pending = range.range_easy_pending;
            emit_archer_volley(world, actor, brain.aim_seed, context, order);
            brain.marker_emitted = true;
        }
        if progress > 16.0 {
            reset_archer(&mut brain);
        } else {
            actor.body_pose = body_pose(&ARCHER_SHOT_POSES, progress);
            brain.action_progress = progress;
        }
        actor.brain = EnemyBrain::Archer(brain);
    } else if target_distance(actor, context.players) < brain.attack_range
        && enemy_target_line_of_sight_is_clear(actor, context)
    {
        let seed = world.draw_loot_seed(1_000_000, &mut context.roll_loot_seed);
        actor.body_pose = ARCHER_SHOT_POSES[0];
        brain.action_progress = 0.0;
        brain.aim_seed = seed;
        brain.marker_emitted = false;
        brain.phase = ArcherPhase::Attack;
        actor.loot_seed = seed;
        actor.brain = EnemyBrain::Archer(brain);
    } else {
        move_toward_target(
            world,
            actor,
            context,
            RadialDirection::Approach,
            TangentDirection::Hold,
        );
    }
}

pub fn reset_archer(brain: &mut ArcherBrain) {
    if brain.phase == ArcherPhase::RangeControl {
        return;
    }
    brain.action_progress = 0.0;
    brain.marker_emitted = false;
    brain.phase = ArcherPhase::RangeControl;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcherStrafeMovement {
    pub delta: Vec2,
    pub gait_advance: f64,
    pub stride_advance: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn step_archer_strafe(
    state: &mut ArcherStrafeState,
    attacking: bool,
    enabled: bool,
    heading: f64,
    scalar: f64,
    position: Vec2,
    target: Option<Vec2>,
    path_is_clear: impl Fn(Vec2) -> bool,
) -> Option<ArcherStrafeMovement> {
    state.limb_heading_deg = heading;
    let target = target?;
    if !enabled || scalar == 0.0 {
        return None;
    }
    if !attacking || state.direction == 0 {
        state.movement_ramp = 0.0;
        state.turn_blend = 0.0;
        return None;
    }
    let dx = fround(position.x - target.x);
    let dy = fround(position.y - target.y);
    let distance = fround(hypot(dx, dy));
    let angle = fround(heading_degrees_from_vector(dx, dy) + state.turn_blend * 10.0)
        * std::f64::consts::PI
        / 180.0;
    let goal = Vec2::new(
        fround(target.x + distance * float::sin(angle)),
        fround(target.y - distance * float::cos(angle)),
    );
    if !path_is_clear(goal) {
        state.direction = 0;
        state.movement_ramp = 0.0;
        state.turn_blend = 0.0;
        return None;
    }
    let ramp = fround((state.movement_ramp + fround(0.01)).min(1.0));
    let movement = fround(scalar * ramp);
    let difference = Vec2::new(fround(goal.x - position.x), fround(goal.y - position.y));
    let length = hypot(difference.x, difference.y);
    let delta = if length == 0.0 {
        Vec2::ZERO
    } else {
        Vec2::new(
            fround(difference.x / length * movement * 0.25),
            fround(difference.y / length * movement * 0.25),
        )
    };
    let blend = if state.direction > 0 {
        (state.turn_blend + fround(0.005)).min(f64::from(state.direction))
    } else {
        (state.turn_blend - fround(0.005)).max(f64::from(state.direction))
    };
    state.limb_heading_deg = heading_degrees_from_vector(delta.x, delta.y);
    state.movement_ramp = ramp;
    state.turn_blend = fround(blend);
    Some(ArcherStrafeMovement {
        delta,
        gait_advance: fround(movement * ramp / 25.0),
        stride_advance: fround(movement * 4.0),
    })
}
