//! Hostile Imp flight and bounce contact from `enemies/imp.ts` and `boneyard-imp-flight.ts`.
//! Its private xorshift presentation stream differs from the friendly Fire Imp's native RNG.
use super::combat::{attack_marker, direct_contact_player_damage};
use super::effects::{DeathEffectOwner, radial_vector};
use super::model::*;
use super::movement::{MOVEMENT_CADENCE_TICKS, positive_modulo};
use super::path::*;
use super::targeting::{
    enemy_navigation_clearance, target_by_id, target_eligible, target_player_within_attack_reach,
};
use super::world::{EnemyMovementPurpose, EnemyMovementRequest, EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;
use crate::rng::next_xorshift_random;

pub fn step_imp(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Imp(mut brain) = actor.brain.clone() else {
        panic!("Imp brain required");
    };
    if brain.phase == FlightPhase::Death {
        return;
    }
    move_imp(world, actor, &mut brain, context);
    if !step_imp_flight(&mut brain) {
        actor.brain = EnemyBrain::Imp(brain);
        return;
    }
    let mut rng = brain.visual_rng_state;
    let vocal = (next_xorshift_random(&mut rng) * 8.0).floor() as usize;
    let vocals = [
        EnemySound::ImpVocal1,
        EnemySound::ImpVocal2,
        EnemySound::ImpVocal3,
        EnemySound::ImpVocal4,
        EnemySound::ImpVocal5,
        EnemySound::ImpVocal6,
        EnemySound::ImpVocal7,
        EnemySound::ImpVocal8,
    ];
    let pitch = 1.0 + next_xorshift_random(&mut rng) * 0.1;
    world.emit_action_sound(DeathEffectOwner::from(&*actor), context.tick, vocals[vocal], pitch);
    // Landing effect scale and color are drawn before the contact-distance branch.
    next_xorshift_random(&mut rng);
    next_xorshift_random(&mut rng);
    let target = target_by_id(actor.target_player_id.as_deref(), context.players);
    if let Some(target) = target.filter(|target| target_eligible(target)) {
        let reach = (target.collision_radius + 45.0) * 1.25;
        if target_player_within_attack_reach(actor, Some(&target.id), context.players, reach) {
            let bite = (next_xorshift_random(&mut rng) * 3.0).floor() as usize;
            let pitch = 1.0 + next_xorshift_random(&mut rng) * 0.25;
            world.emit_action_sound(
                DeathEffectOwner::from(&*actor),
                context.tick,
                [EnemySound::Bite1, EnemySound::Bite2, EnemySound::Bite3][bite],
                pitch,
            );
            let event = attack_marker(world, actor, context.tick, Some(target.id.clone()), order);
            // FireBurst constructor rotation/magnitude/sign and caller scale.
            for _ in 0..4 {
                next_xorshift_random(&mut rng);
            }
            let heading = positive_modulo(
                actor.heading_deg + 180.0 + next_xorshift_random(&mut rng) * 45.0,
                360.0,
            );
            actor.heading_deg = heading;
            brain.escape_heading_deg = Some(heading);
            direct_contact_player_damage(
                world,
                actor,
                Some(&target.id),
                context.players,
                reach,
                event,
            );
        }
    }
    brain.visual_rng_state = rng;
    actor.brain = EnemyBrain::Imp(brain);
}

fn step_imp_flight(brain: &mut ImpBrain) -> bool {
    let mut rng = brain.visual_rng_state;
    brain.effect_phase =
        positive_modulo(brain.effect_phase + brain.horizontal_speed.abs() * 0.25, 10.0);
    brain.vertical_offset += brain.vertical_velocity;
    brain.vertical_velocity += 0.4;
    brain.effect_alpha = (brain.effect_alpha - 0.05).max(0.0);
    let bounced = brain.vertical_offset > 0.0;
    if bounced {
        brain.horizontal_speed =
            brain.base_horizontal_speed * (1.0 + next_xorshift_random(&mut rng) * 1.5);
        brain.vertical_offset = 0.0;
        brain.vertical_velocity = -(3.0 + next_xorshift_random(&mut rng) * 3.0);
        brain.body_variant = (next_xorshift_random(&mut rng) * 4.0).floor() as usize;
        let rotation = next_xorshift_random(&mut rng) * 60.0;
        brain.body_rotation_deg =
            if next_xorshift_random(&mut rng) < 0.5 { -rotation } else { rotation };
        brain.effect_alpha = 1.0;
        if (next_xorshift_random(&mut rng) * 20.0).floor() == 3.0 {
            brain.vertical_velocity *= 1.5;
        }
    }
    brain.visual_rng_state = rng;
    bounced
}

fn move_imp(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    brain: &mut ImpBrain,
    context: &EnemyStepContext<'_>,
) {
    if context.tick < actor.next_movement_tick {
        return;
    }
    let status_factor = actor.staff_movement_factor * context.ability_effect(actor.id).time_scale;
    let scalar = fround(
        actor.config.chase_speed
            * brain.horizontal_speed
            * actor.staff_movement_factor
            * actor.config.scale
            * actor.path.speed_factor
            * status_factor,
    );
    let movement_per_tick = scalar * 0.25;
    let target = target_by_id(actor.target_player_id.as_deref(), context.players);
    let mut path = actor.path;
    let (delta, heading) = if let Some(heading) = brain.escape_heading_deg {
        (radial_vector(heading, movement_per_tick * MOVEMENT_CADENCE_TICKS as f64), heading)
    } else {
        let target_heading = target.map_or(0.0, |target| {
            let velocity = target.velocity_per_tick;
            if hypot(velocity.x, velocity.y) == 0.0 {
                target.heading_deg.unwrap_or(0.0)
            } else {
                heading_degrees_from_vector(velocity.x, velocity.y)
            }
        });
        let mut request = EnemySteeringRequest {
            actor_heading_deg: actor.heading_deg,
            actor_position: actor.position,
            cadence_ticks: MOVEMENT_CADENCE_TICKS,
            full_movement: true,
            goal_position: None,
            movement_per_tick,
            radial_direction: RadialDirection::Approach,
            status_factor,
            tangent_direction: TangentDirection::Hold,
            target_heading_deg: target_heading,
            target_position: target.map(|target| target.position),
        };
        let goal = native_enemy_steering_goal(&path, &request);
        let routed = resolve_native_enemy_path_goal(
            path,
            EnemyPathGoalRequest {
                actor_position: actor.position,
                body_radius: actor.config.collision_radius,
                cadence_ticks: MOVEMENT_CADENCE_TICKS,
                navigation_clearance: enemy_navigation_clearance(actor),
                raw_goal: goal,
                target_position: target.map(|target| target.position),
                target_refresh_ticks: native_enemy_target_refresh_ticks(
                    actor.config.pathfinding_mode,
                ),
            },
            context.geometry,
        );
        path = routed.state;
        if routed.turn_around {
            request.actor_heading_deg = positive_modulo(request.actor_heading_deg + 180.0, 360.0);
        }
        request.goal_position = Some(routed.goal);
        let steering = build_native_enemy_steering(path, request);
        path = steering.state;
        (steering.delta, steering.heading_deg)
    };
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
        "resolved Imp position must be finite"
    );
    let traveled = hypot(position.x - actor.position.x, position.y - actor.position.y);
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
    if brain.escape_heading_deg.is_some() && position != requested_position {
        brain.escape_heading_deg = None;
    }
    actor.heading_deg = heading;
    if traveled != 0.0 {
        actor.last_movement_tick = Some(context.tick);
    }
    actor.next_movement_tick = context.tick + MOVEMENT_CADENCE_TICKS;
    actor.path = path;
    actor.position = position;
}
