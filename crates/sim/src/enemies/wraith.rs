//! Wraith flyby clock, two-substep flight and contact from `native-wraith-flight.ts`.
//! Wraith roots move directly; they do not call the solid-world movement resolver.
use super::combat::{attack_marker, direct_player_damage};
use super::effects::{
    DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec, radial_vector,
};
use super::model::*;
use super::movement::{positive_modulo, route_native_enemy_goal};
use super::path::{PathRecoveryRequest, native_enemy_movement_clock};
use super::targeting::{target_by_id, target_eligible};
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float;
use crate::float::{atan2, fround, hypot};
use crate::painter::WorldPainterOrder;

/// `createNativeWraithFlightState`: constructor samples are owned by the caller's stream.
#[must_use]
pub fn create_native_wraith_flight_state(
    chase_speed: f64,
    resting_sample: f64,
    initial_sample: f64,
    flyby_offset: u64,
) -> WraithBrain {
    let base = fround(chase_speed * (0.8_f32 as f64));
    WraithBrain {
        collision_recipient: true,
        base_flyby_speed: base,
        contact_cooldown_ticks: 0,
        current_speed: fround(fround(base * 25.0) * fround(1.0 + fround(initial_sample * 2.0))),
        current_turn_gain: 1.5,
        flyby_ticks_remaining: 200 + flyby_offset,
        resting_speed: fround(base * fround(20.0 + fround(resting_sample * 10.0))),
        target_turn_gain: 3.0,
        phase: FlightPhase::Flight,
    }
}

pub fn step_wraith(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Wraith(mut brain) = actor.brain.clone() else {
        panic!("Wraith brain required");
    };
    let admitted = context.native_visibility.is_none_or(|visible| visible(actor.position).admitted);
    step_wraith_flight_clock(&mut brain, admitted);
    let clear_recipient = brain.collision_recipient;
    brain.collision_recipient = false;
    brain.phase = FlightPhase::Flight;
    move_wraith(world, actor, &brain, context, admitted);
    let target = target_by_id(actor.target_player_id.as_deref(), context.players);
    if let Some(target) = target.filter(|target| target_eligible(target)) {
        let dx = actor.position.x - target.position.x;
        let dy = actor.position.y - target.position.y;
        if dx * dx + dy * dy < 40.0 * 40.0 {
            if brain.contact_cooldown_ticks == 0 {
                world.draw_unit(); // Contact sound pitch.
                let event =
                    attack_marker(world, actor, context.tick, Some(target.id.clone()), order);
                direct_player_damage(world, actor, Some(&target.id), event);
                world.draw_unit(); // Contact effect phase.
            }
            let offset = world.draw_integer(601);
            let turn = world.draw_unit();
            brain.contact_cooldown_ticks = 50;
            brain.current_speed = fround(brain.base_flyby_speed * 50.0);
            brain.current_turn_gain = 1.0;
            brain.flyby_ticks_remaining = 200 + u64::from(offset);
            brain.target_turn_gain = fround(7.0 + fround(turn * 5.0));
        }
    }
    actor.brain = EnemyBrain::Wraith(brain);
    if clear_recipient {
        context.geometry.on_collision_recipient_cleared(actor.id);
    }
}

fn move_wraith(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    brain: &WraithBrain,
    context: &EnemyStepContext<'_>,
    admitted: bool,
) {
    let effect = context.ability_effect(actor.id);
    let status = actor.staff_movement_factor * effect.time_scale;
    if status <= (0.0001_f32 as f64) {
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
    let clock = native_enemy_movement_clock(
        actor.id,
        context.tick,
        visible,
        admitted,
        view.is_none_or(|view| view.enhanced_effects),
    );
    if !clock.due || effect.disrupted_ticks > 0 {
        return;
    }
    let target = target_by_id(actor.target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target))
        .map(|target| target.position);
    if actor.path.reorientation_ticks_remaining > 0 {
        actor.heading_deg =
            actor.body.path.step_reorientation(actor.body.heading_deg, actor.body.position, target);
        return;
    }
    let (delta, heading) = if clock.full {
        actor.path.clear_route();
        native_wraith_movement(actor, brain, clock.cadence, status, target)
    } else {
        let wander = radial_vector(
            actor.id as f64 * 225.0,
            actor.path.speed_factor * brain.current_speed * status * clock.cadence as f64 * 0.25,
        );
        let raw_goal = target
            .unwrap_or_else(|| Vec2::new(actor.position.x + wander.x, actor.position.y + wander.y));
        let routed = route_native_enemy_goal(actor, raw_goal, target, context, clock.cadence);
        native_wraith_degraded_movement(
            actor,
            brain,
            clock.cadence,
            status,
            routed.goal,
            actor.heading_deg + if routed.turn_around { 180.0 } else { 0.0 },
        )
    };
    actor.path.flank_ticks_remaining =
        actor.path.flank_ticks_remaining.saturating_sub(clock.cadence);
    let distance = hypot(delta.x, delta.y);
    actor.body.path.step_recovery(
        &mut world.steering_rng,
        PathRecoveryRequest {
            flanking_enabled: actor.body.config.flanking,
            requested_distance: distance,
            status_factor: status,
            tick: context.tick,
            traveled_distance: distance,
        },
    );
    actor.heading_deg = heading;
    if distance != 0.0 {
        actor.last_movement_tick = Some(context.tick);
    }
    actor.next_movement_tick = context.tick + clock.cadence;
    actor.position =
        Vec2::new(fround(actor.position.x + delta.x), fround(actor.position.y + delta.y));
}

pub fn step_wraith_flight_clock(brain: &mut WraithBrain, admitted: bool) {
    brain.flyby_ticks_remaining = brain.flyby_ticks_remaining.saturating_sub(u64::from(admitted));
    let turn_step = 0.01_f32 as f64;
    if brain.current_turn_gain < brain.target_turn_gain {
        brain.current_turn_gain = fround(brain.current_turn_gain + turn_step);
    }
    // Stock performs both comparisons after the first float store. Crossing the
    // target can apply both steps in one update; an else-if changes that clock.
    if brain.target_turn_gain < brain.current_turn_gain {
        brain.current_turn_gain = fround(brain.current_turn_gain - turn_step);
    }
    if brain.contact_cooldown_ticks > 0 {
        brain.contact_cooldown_ticks -= 1;
    } else if brain.flyby_ticks_remaining > 0 {
        if brain.resting_speed * 2.0 < brain.current_speed {
            brain.current_speed = fround(brain.current_speed - 1.0);
        }
        brain.target_turn_gain = fround(brain.target_turn_gain - turn_step).max(1.5);
    } else {
        brain.target_turn_gain = fround(brain.target_turn_gain + turn_step);
    }
    if brain.resting_speed < brain.current_speed {
        brain.current_speed = fround(brain.current_speed - (0.025_f32 as f64));
    }
}

#[must_use]
pub fn native_wraith_movement(
    actor: &impl EnemyActorRead,
    brain: &WraithBrain,
    cadence: u64,
    status: f64,
    target: Option<Vec2>,
) -> (Vec2, f64) {
    let goal = if let Some(target) = target {
        if brain.flyby_ticks_remaining > 0 {
            let bearing = heading_to(actor.position, target, actor.heading_deg);
            let offset = radial_vector(bearing, 300.0);
            Vec2::new(target.x + offset.x, target.y + offset.y)
        } else {
            target
        }
    } else {
        let offset = radial_vector(actor.id as f64 * 225.0, 10_000.0);
        Vec2::new(actor.position.x + offset.x, actor.position.y + offset.y)
    };
    let desired = heading_to(actor.position, goal, actor.heading_deg);
    let turn_step = actor.path.turn_factor * brain.current_turn_gain * status;
    let movement = actor.path.speed_factor * brain.current_speed * status * 0.25;
    let mut heading = actor.heading_deg;
    let mut delta = Vec2::ZERO;
    for _ in 0..cadence {
        heading = turn_wraith_heading(heading, desired, turn_step);
        let radians = heading * std::f64::consts::PI / 180.0;
        delta.x = fround(delta.x + fround(float::sin(radians) * movement));
        delta.y = fround(delta.y - fround(float::cos(radians) * movement));
    }
    (delta, heading)
}

#[must_use]
pub fn native_wraith_degraded_movement(
    actor: &impl EnemyActorRead,
    brain: &WraithBrain,
    cadence: u64,
    status: f64,
    goal: Vec2,
    original_heading: f64,
) -> (Vec2, f64) {
    let desired = heading_to(actor.position, goal, original_heading);
    let direction = wraith_turn_direction(original_heading, desired);
    let mut heading = turn_wraith_heading(
        original_heading,
        desired,
        brain.current_turn_gain * status * cadence as f64,
    );
    if direction != wraith_turn_direction(heading, desired) {
        heading = desired;
    }
    let movement = actor.path.speed_factor * brain.current_speed * status * cadence as f64 * 0.25;
    let radians = heading * std::f64::consts::PI / 180.0;
    (
        Vec2::new(fround(float::sin(radians) * movement), fround(-float::cos(radians) * movement)),
        heading,
    )
}

fn heading_to(source: Vec2, target: Vec2, fallback: f64) -> f64 {
    let dx = target.x - source.x;
    let dy = target.y - source.y;
    if dx == 0.0 && dy == 0.0 {
        positive_modulo(fallback, 360.0)
    } else {
        positive_modulo(atan2(dx, -dy) * 180.0 / std::f64::consts::PI, 360.0)
    }
}

fn turn_wraith_heading(current: f64, target: f64, step: f64) -> f64 {
    positive_modulo(fround(current + wraith_turn_direction(current, target) * step), 360.0)
}

fn wraith_turn_direction(current: f64, target: f64) -> f64 {
    let current = positive_modulo(current, 360.0);
    let target = positive_modulo(target, 360.0);
    let separation = (current - target).abs();
    if !(1.0..359.0).contains(&separation) {
        return 0.0;
    }
    if target <= current {
        if current - target <= 180.0 { -1.0 } else { 1.0 }
    } else if target - current > 180.0 {
        -1.0
    } else {
        1.0
    }
}

pub fn spawn_wraith_wisp(
    world: &mut EnemyWorld,
    actor: &impl EnemyActorRead,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Wraith(brain) = actor.brain() else {
        return;
    };
    if actor.life_state != EnemyLifeState::Alive
        || context.native_visibility.is_some_and(|visible| !visible(actor.position).admitted)
    {
        return;
    }
    let roll = world.steering_rng.draw_integer(4);
    if roll != 1 && brain.contact_cooldown_ticks == 0 {
        return;
    }
    let offset = radial_vector(actor.heading_deg, 30.0);
    let alpha = fround(0.25 + world.steering_rng.draw_scaled_float(0.20000000298023224, false));
    world.spawn_simple_death_effect(
        DeathEffectOwner::from(actor),
        context.tick,
        SimpleDeathEffectSpec {
            alpha,
            alpha_loss_per_tick: fround(0.10000000149011612 * 0.15000000596046448),
            atlas: EnemyEffectAtlas::BadGuys,
            blend_mode: EnemyEffectBlend::Add,
            entry: 21,
            kind: EnemyDeathEffectKind::FadeAdditive,
            lifetime_ticks: 32,
            role: "wraith-soul-wisp",
            scale: 1.0,
        },
        DeathEffectOverrides {
            position: Some(Vec2::new(
                fround(actor.position.x - offset.x),
                fround(actor.position.y - 15.0 - offset.y),
            )),
            presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
            rotation_deg: Some(actor.heading_deg),
            ..DeathEffectOverrides::default()
        },
        order,
    );
}
