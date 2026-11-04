//! Native two-tick steering, collision commit and gait writers from `enemies/movement.ts`.

use super::EnemyToken;
use super::config::{EnemyFamilyConfig, SkeletonWeapon};
use super::model::*;
use super::path::*;
use super::targeting::{enemy_navigation_clearance, target_by_id};
use super::world::{EnemyMovementPurpose, EnemyMovementRequest, EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;

pub const MOVEMENT_CADENCE_TICKS: u64 = 2;

#[must_use]
pub fn staff_attack_speed(actor: &EnemyActor) -> f64 {
    actor.config.attack_speed * actor.staff_action_factor
}
#[must_use]
pub fn staff_movement_speed(actor: &EnemyActor) -> f64 {
    let speed = match &actor.brain {
        EnemyBrain::DemonSkull(brain) => {
            brain.speed / actor.config.scale * (actor.config.base_speed / 4.0)
        }
        _ => actor.config.base_speed,
    };
    speed * actor.staff_movement_factor
}

/// Shared Spider/Wraith inherited route branch from `enemies/movement.ts`.
pub fn route_native_enemy_goal(
    actor: &mut EnemyActor,
    raw_goal: Vec2,
    target: Option<Vec2>,
    context: &EnemyStepContext<'_>,
    cadence: u64,
) -> EnemyPathGoal {
    let goal = if target.is_none() {
        context
            .native_movement_view
            .and_then(|view| {
                crate::line_obstruction::line_bounds_exit_obstruction(
                    actor.position,
                    raw_goal,
                    view.arena_bounds,
                )
            })
            .map_or(raw_goal, |exit| exit.point)
    } else {
        raw_goal
    };
    let routed = resolve_native_enemy_path_goal(
        actor.path,
        EnemyPathGoalRequest {
            actor_position: actor.position,
            body_radius: super::targeting::boneyard_enemy_collision_radius(actor),
            cadence_ticks: cadence,
            navigation_clearance: 25.0,
            raw_goal: goal,
            target_position: target,
            target_refresh_ticks: native_enemy_target_refresh_ticks(actor.config.pathfinding_mode),
        },
        context.geometry,
    );
    actor.path = routed.state;
    routed
}

pub fn move_toward_target(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    radial: RadialDirection,
    tangent: TangentDirection,
) {
    let fleeing = radial == RadialDirection::Retreat && tangent == TangentDirection::Hold;
    let status_factor = actor.staff_movement_factor * context.ability_effect(actor.id).time_scale;
    let mut cadence_ticks = MOVEMENT_CADENCE_TICKS;
    let mut full_movement = true;
    if fleeing {
        if status_factor <= 0.0001_f32 as f64 {
            return;
        }
        let view = context.native_movement_view;
        let visible = view.is_none_or(|view| {
            view.cameras.iter().any(|camera| {
                actor.position.x + 100.0 >= camera.x
                    && actor.position.x - 100.0 <= camera.x + camera.width
                    && actor.position.y + 100.0 >= camera.y
                    && actor.position.y - 100.0 <= camera.y + camera.height
            })
        });
        let illuminated =
            context.native_visibility.is_none_or(|sample| sample(actor.position).admitted);
        let clock = native_enemy_movement_clock(
            actor.id,
            context.tick,
            visible,
            illuminated,
            view.is_none_or(|view| view.enhanced_effects),
        );
        if !clock.due {
            return;
        }
        cadence_ticks = clock.cadence;
        full_movement = clock.full;
    } else if context.tick < actor.next_movement_tick {
        return;
    }
    if matches!(
        actor.config.enemy_token,
        EnemyToken::Skeleton | EnemyToken::SkeletonArcher | EnemyToken::SkeletonMage
    ) && actor.hit_reaction_timer > 0.0
    {
        actor.next_movement_tick = context.tick + cadence_ticks;
        return;
    }
    let target = target_by_id(actor.target_player_id.as_deref(), context.players);
    let target_heading = target.map_or(0.0, |target| {
        if hypot(target.velocity_per_tick.x, target.velocity_per_tick.y) == 0.0 {
            target.heading_deg.unwrap_or(0.0)
        } else {
            heading_degrees_from_vector(target.velocity_per_tick.x, target.velocity_per_tick.y)
        }
    });
    let movement_scalar = fround(
        actor.config.chase_speed
            * staff_movement_speed(actor)
            * actor.config.scale
            * actor.path.speed_factor,
    );
    let mut request = EnemySteeringRequest {
        actor_heading_deg: actor.heading_deg,
        actor_position: actor.position,
        cadence_ticks,
        full_movement,
        goal_position: None,
        movement_per_tick: 0.25 * movement_scalar,
        radial_direction: radial,
        status_factor,
        tangent_direction: tangent,
        target_heading_deg: target_heading,
        target_position: target.map(|target| target.position),
    };
    let uses_target_route =
        radial == RadialDirection::Approach && tangent == TangentDirection::Hold;
    let mut path = actor.path;
    if !uses_target_route {
        path.clear_route();
    }
    let raw_goal = native_enemy_steering_goal(&path, &request);
    let mut goal = raw_goal;
    if !fleeing {
        let routed = resolve_native_enemy_path_goal(
            path,
            EnemyPathGoalRequest {
                actor_position: actor.position,
                body_radius: actor.config.collision_radius,
                cadence_ticks: MOVEMENT_CADENCE_TICKS,
                navigation_clearance: enemy_navigation_clearance(actor),
                raw_goal,
                target_position: if uses_target_route {
                    target.map(|target| target.position)
                } else {
                    None
                },
                target_refresh_ticks: native_enemy_target_refresh_ticks(
                    actor.config.pathfinding_mode,
                ),
            },
            context.geometry,
        );
        path = routed.state;
        goal = routed.goal;
        if routed.turn_around {
            request.actor_heading_deg = positive_modulo(request.actor_heading_deg + 180.0, 360.0);
        }
    }
    request.goal_position = Some(goal);
    let steering = build_native_enemy_steering(path, request);
    let delta = steering.delta;
    let requested_position = Vec2::new(actor.position.x + delta.x, actor.position.y + delta.y);
    let position = context.geometry.resolve_movement(EnemyMovementRequest {
        actor_id: actor.id,
        delta,
        position: actor.position,
        purpose: EnemyMovementPurpose::Movement,
        radius: actor.config.collision_radius,
        requested_position,
    });
    assert!(
        position.x.is_finite() && position.y.is_finite(),
        "resolved enemy position must be finite"
    );
    let traveled = hypot(position.x - actor.position.x, position.y - actor.position.y);
    let body_phase =
        advance_locomotion_phase(actor.body_gait_phase, movement_scalar, cadence_ticks, 35.0, 4.0);
    actor.body_gait_phase = body_phase;
    actor.shadow_lateral_offset = body_phase;
    actor.body_pose = match actor.config.family {
        EnemyFamilyConfig::Skeleton { weapon: SkeletonWeapon::Claw, armor: false, .. }
        | EnemyFamilyConfig::Archer { .. } => {
            [0.0, 1.0, 2.0, 1.0, 0.5][body_phase.trunc() as usize]
        }
        EnemyFamilyConfig::Skeleton { .. } => 0.0,
        EnemyFamilyConfig::Mage { .. } => actor.rest_body_pose,
        _ => actor.body_pose,
    };
    actor.gait_pose =
        advance_locomotion_phase(actor.gait_pose, movement_scalar, cadence_ticks, 25.0, 8.0);
    actor.heading_deg = steering.heading_deg;
    if traveled != 0.0 {
        actor.last_movement_tick = Some(context.tick);
    }
    actor.next_movement_tick = context.tick + cadence_ticks;
    let mut path = steering.state;
    path.step_recovery(
        &mut world.steering_rng,
        PathRecoveryRequest {
            flanking_enabled: actor.config.flanking,
            requested_distance: hypot(delta.x, delta.y),
            status_factor,
            tick: context.tick,
            traveled_distance: traveled,
        },
    );
    actor.path = path;
    actor.position = position;
    let increment = fround(fround(movement_scalar) * 4.0);
    let mut stride = fround(actor.stride_phase_deg);
    for _ in 0..cadence_ticks {
        stride = fround(stride + increment);
    }
    actor.stride_phase_deg = stride;
}

fn advance_locomotion_phase(
    phase: f64,
    scalar: f64,
    cadence: u64,
    divisor: f64,
    period: f64,
) -> f64 {
    let mut phase = fround(phase);
    let scalar = fround(scalar);
    for _ in 0..cadence {
        phase = fround(phase + scalar / divisor);
        if phase > period {
            phase = fround(phase - period);
        }
    }
    phase
}

#[must_use]
pub fn positive_modulo(value: f64, period: f64) -> f64 {
    ((value % period) + period) % period
}
