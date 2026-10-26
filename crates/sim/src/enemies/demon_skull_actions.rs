//! Discorporeal ActionManager ordering from `enemies/demon-skull-actions.ts`.

use super::demon_skull_controller::{
    DemonSkullActionKind, NATIVE_DEMON_SKULL_SCREAM_TICKS, create_native_demon_skull_action,
};
use super::demon_skull_effects::spawn_demon_skull_warmup;
use super::demon_skull_spells::{
    bite_demon_skull, fire_demon_skull_eyes, flash, spit_demon_skull_fire,
    step_demon_skull_mouth_beam,
};
use super::effects::DeathEffectOwner;
use super::model::*;
use super::movement::positive_modulo;
use super::targeting::{target_by_id, target_eligible};
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;
use crate::spells::targeting::native_heading_turn_direction;

pub fn begin_demon_skull_action(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    kind: DemonSkullActionKind,
    context: &EnemyStepContext<'_>,
    authored_attack_speed: f64,
) {
    let target_heading = target_by_id(actor.target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target))
        .map(|target| {
            heading_degrees_from_vector(
                target.position.x - actor.position.x,
                target.position.y - actor.position.y,
            )
        });
    let action = create_native_demon_skull_action(
        kind,
        actor.heading_deg,
        target_heading,
        authored_attack_speed,
        actor.loot_seed,
    );
    let EnemyBrain::DemonSkull(brain) = &mut actor.brain else {
        panic!("DemonSkull action requires its owner")
    };
    brain.actions.push(action);
    if matches!(kind, DemonSkullActionKind::Scream | DemonSkullActionKind::Flair) {
        world.demon_skull_encounter.scream_stream_ticks_remaining = NATIVE_DEMON_SKULL_SCREAM_TICKS;
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyStream, actor.body.id, context.tick);
        event.stream = Some("unholy-scream".to_owned());
        world.emit_event(event);
        brain.body_pose = 1.0;
        brain.scream_active |= kind == DemonSkullActionKind::Scream;
    }
    if matches!(kind, DemonSkullActionKind::Eyes | DemonSkullActionKind::Mouth) {
        brain.charge_glow = 0.0;
        world.emit_action_sound(
            DeathEffectOwner { id: actor.body.id, position: actor.body.position },
            context.tick,
            EnemySound::EyeLaserCharge,
            authored_attack_speed * if kind == DemonSkullActionKind::Eyes { 2.0 } else { 0.5 },
        );
    }
    actor.body_pose = brain.body_pose;
}

pub fn step_demon_skull_actions(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::DemonSkull(brain) = &actor.brain else {
        panic!("DemonSkull action requires its owner")
    };
    let source_actions = brain.actions.clone();
    let mut actions = Vec::new();
    // Retired entries are removed before traversal, so a just-retired action remains
    // visible to the outer controller for this update.
    for action in source_actions {
        if action.retired {
            continue;
        }
        actions.push(step_action(world, actor, action, context, order));
    }
    let EnemyBrain::DemonSkull(brain) = &mut actor.brain else { unreachable!() };
    actor.body.body_pose = brain.body_pose;
    brain.actions = actions;
}

fn step_action(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    mut action: DemonSkullAction,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) -> DemonSkullAction {
    let EnemyBrain::DemonSkull(mut brain) = actor.brain.clone() else { unreachable!() };
    let target = target_by_id(actor.target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target));
    let aim = target.map(|target| {
        heading_degrees_from_vector(
            target.position.x - actor.position.x,
            target.position.y - actor.position.y,
        )
    });
    match &mut action.program {
        DemonSkullActionProgram::Scream => {}
        DemonSkullActionProgram::Bite { progress, rate, marker_emitted } => {
            if *progress == 0.0 {
                world.emit_action_sound(
                    DeathEffectOwner::from(&*actor),
                    context.tick,
                    EnemySound::SkullBite,
                    1.0,
                );
            }
            let time = context.ability_effect(actor.id).time_scale * actor.staff_action_factor;
            let next = fround(*progress + fround(*rate * time));
            brain.body_pose = if next.trunc() < 4.0 { 1.0 } else { 0.0 };
            let marker = next > *progress && *progress <= 4.0 && next >= 4.0;
            if marker {
                actor.brain = EnemyBrain::DemonSkull(brain);
                bite_demon_skull(world, actor, context, order);
                let EnemyBrain::DemonSkull(updated) = actor.brain.clone() else { unreachable!() };
                brain = updated;
            }
            *progress = next;
            *marker_emitted |= marker;
            action.retired = next > 13.0;
        }
        DemonSkullActionProgram::Eyes {
            warmup_ticks,
            shots_remaining,
            recovery_ticks,
            attack_speed,
        } => {
            brain.charge_glow = fround(brain.charge_glow + fround(0.025)).min(1.0);
            if *warmup_ticks > 0 {
                brain.jitter = world.random_enemy_offset(3.0);
                spawn_demon_skull_warmup(world, actor, context.tick, true, order);
                let heading = aim.unwrap_or(actor.heading_deg);
                turn(actor, &mut brain, heading, 1.0);
                *warmup_ticks -= 1;
            } else if *shots_remaining == 0 {
                brain.charge_glow = fround(brain.charge_glow - fround(0.05)).max(0.0);
                *recovery_ticks -= 1;
                if *recovery_ticks == 0 {
                    recover(&mut brain, actor.config.scale);
                    action.retired = true;
                }
            } else {
                if *shots_remaining >= 3 {
                    brain.jitter = world.random_enemy_offset(3.0);
                }
                let time = context.ability_effect(actor.id).time_scale * actor.staff_action_factor;
                brain.eye_charge =
                    fround(brain.eye_charge + *attack_speed * fround(0.025) * time).min(1.0);
                actor.heading_deg = aim.unwrap_or(actor.heading_deg);
                brain.heading_delay_ticks = 11;
                if brain.eye_charge >= 1.0 {
                    brain.jitter = Vec2::ZERO;
                    brain.eye_charge = 0.0;
                    actor.brain = EnemyBrain::DemonSkull(brain);
                    fire_demon_skull_eyes(world, actor, context, order);
                    let EnemyBrain::DemonSkull(updated) = actor.brain.clone() else {
                        unreachable!()
                    };
                    brain = updated;
                    brain.spin = world.steering_rng.draw_scaled_float(15.0, true);
                    brain.charge_glow = 0.0;
                    *shots_remaining -= 1;
                }
            }
        }
        DemonSkullActionProgram::Mouth {
            warmup_ticks,
            warmup_jitter,
            warmup_turn_speed,
            beam_power,
            tracking_delay_ticks,
            tracking_ramp,
            recovery_ticks,
        } => {
            brain.charge_glow = fround(brain.charge_glow + fround(0.025)).min(1.0);
            if *warmup_ticks > 0 {
                brain.jitter = world.random_enemy_offset(*warmup_jitter);
                spawn_demon_skull_warmup(world, actor, context.tick, false, order);
                if let Some(aim) = aim
                    && *warmup_turn_speed > 0.0
                {
                    turn(actor, &mut brain, aim, *warmup_turn_speed);
                    *warmup_turn_speed = fround(*warmup_turn_speed - fround(0.004));
                }
                *warmup_ticks -= 1;
                if *warmup_ticks == 0 {
                    flash(
                        world,
                        actor,
                        context.tick,
                        EnemyScreenFlash {
                            red: fround(0.55),
                            green: 1.0,
                            blue: fround(0.05),
                            alpha: 1.0,
                            decay_per_tick: fround(0.05),
                            point_attenuated: true,
                        },
                    );
                    world.emit_action_sound(
                        DeathEffectOwner::from(&*actor),
                        context.tick,
                        EnemySound::LightningStart,
                        1.0,
                    );
                }
                *warmup_jitter = fround(*warmup_jitter + fround(0.05));
            } else if *beam_power <= 0.0 {
                *recovery_ticks -= 1;
                if *recovery_ticks < 0 {
                    recover(&mut brain, actor.config.scale);
                    brain.body_offset = Vec2::ZERO;
                    action.retired = true;
                }
            } else {
                brain.body_pose = 1.0;
                brain.charge_glow = 0.0;
                brain.body_phase_deg = 0.0;
                brain.body_offset = world.random_enemy_offset(3.0);
                *tracking_delay_ticks -= 1;
                if *tracking_delay_ticks <= 0
                    && let Some(aim) = aim
                {
                    turn(actor, &mut brain, aim, fround(0.5 * *tracking_ramp));
                    *tracking_ramp = fround(*tracking_ramp + fround(0.01)).min(1.0);
                }
                shake(world, actor, context.tick, CameraShakeAttenuation::Point, 10.0);
                actor.brain = EnemyBrain::DemonSkull(brain.clone());
                step_demon_skull_mouth_beam(world, actor, context, *beam_power, order);
                *beam_power = fround(*beam_power - fround(0.015));
                if *beam_power <= 0.0 {
                    brain.body_pose = 0.0;
                    brain.body_offset = Vec2::ZERO;
                    world.emit_action_sound(
                        DeathEffectOwner::from(&*actor),
                        context.tick,
                        EnemySound::Jaw,
                        1.0,
                    );
                }
            }
        }
        DemonSkullActionProgram::Spit {
            shots_remaining,
            cooldown_ticks,
            mouth_ticks,
            target_heading_deg,
        } => {
            if *mouth_ticks > 0 {
                *mouth_ticks -= 1;
                if *mouth_ticks == 0 {
                    brain.body_pose = 0.0;
                }
            } else {
                turn(actor, &mut brain, *target_heading_deg, 1.0);
                *target_heading_deg = target.map_or(actor.heading_deg, |target| {
                    if target.summoned {
                        aim.expect("eligible target has aim")
                    } else {
                        heading_degrees_from_vector(
                            target.position.x + target.velocity_per_tick.x * 260.0
                                - actor.position.x,
                            target.position.y + target.velocity_per_tick.y * 260.0
                                - actor.position.y,
                        )
                    }
                });
                *cooldown_ticks -= 1;
                if *cooldown_ticks <= 0 && (actor.heading_deg - *target_heading_deg).abs() < 3.0 {
                    brain.body_pose = 1.0;
                    actor.brain = EnemyBrain::DemonSkull(brain);
                    spit_demon_skull_fire(world, actor, context, order);
                    let EnemyBrain::DemonSkull(updated) = actor.brain.clone() else {
                        unreachable!()
                    };
                    brain = updated;
                    *shots_remaining -= 1;
                    if *shots_remaining == 0 {
                        recover(&mut brain, actor.config.scale);
                        brain.body_pose = 0.0;
                        action.retired = true;
                    }
                    *cooldown_ticks = 75;
                    *mouth_ticks = 35;
                }
            }
        }
        DemonSkullActionProgram::Flair {
            remaining_ticks,
            settling_ticks,
            glow,
            original_heading_deg,
        } => {
            brain.body_pose = 1.0;
            if *settling_ticks > 0 {
                actor.heading_deg = *original_heading_deg;
                *settling_ticks += 1;
                if *settling_ticks >= 50 {
                    brain.body_pose = 0.0;
                    world.emit_action_sound(
                        DeathEffectOwner::from(&*actor),
                        context.tick,
                        EnemySound::Jaw,
                        1.0,
                    );
                    action.retired = true;
                }
            } else {
                *remaining_ticks -= 1;
                actor.heading_deg = fround(
                    float::sin(
                        *remaining_ticks as f64 / 200.0 * 630.0 * std::f64::consts::PI / 180.0,
                    ) * 40.0
                        + *original_heading_deg,
                );
                brain.body_heading_deg = actor.heading_deg;
                brain.flair_glow = *glow;
                shake(world, actor, context.tick, CameraShakeAttenuation::Fixed, 3.0);
                if *remaining_ticks == 0 {
                    *settling_ticks = 1;
                }
                *glow = fround(*glow + fround(0.05)).min(1.0);
            }
        }
    }
    actor.body_pose = brain.body_pose;
    actor.brain = EnemyBrain::DemonSkull(brain);
    action
}

fn turn(actor: &mut EnemyActor, brain: &mut DemonSkullBrain, heading: f64, speed: f64) {
    actor.heading_deg = positive_modulo(
        fround(
            actor.heading_deg
                + f64::from(native_heading_turn_direction(actor.heading_deg, heading)) * speed,
        ),
        360.0,
    );
    brain.heading_delay_ticks = 11;
}
fn recover(brain: &mut DemonSkullBrain, scale: f64) {
    brain.target_speed = fround(scale * 4.0);
    brain.spin = 0.0;
    brain.charge_glow = 0.0;
}
pub(super) fn shake(
    world: &mut EnemyWorld,
    actor: &impl EnemyActorRead,
    tick: u64,
    attenuation: CameraShakeAttenuation,
    radius: f64,
) {
    let displacement = world.random_enemy_offset(radius);
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyCameraShake, actor.id, tick);
    event.source_position = Some(actor.position);
    event.camera_shake = Some(EnemyCameraShake { attenuation, displacement });
    world.emit_event(event);
}
