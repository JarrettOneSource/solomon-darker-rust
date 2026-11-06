//! The Discorporeal's action list: starting an action and stepping every live action in list
//! order. Its actions are the scream, bite, eye shots, mouth beam, spit and flair.

use super::demon_skull_controller::{
    DEMON_SKULL_SCREAM_TICKS, DemonSkullActionKind, create_demon_skull_action,
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
use crate::spells::targeting::heading_turn_direction;

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
    let action = create_demon_skull_action(
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
        world.demon_skull_encounter.scream_stream_ticks_remaining = DEMON_SKULL_SCREAM_TICKS;
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyStream, actor.id, context.tick);
        event.stream = Some("unholy-scream".to_owned());
        world.emit_event(event);
        brain.body_pose = 1.0;
        brain.scream_active |= kind == DemonSkullActionKind::Scream;
    }
    if matches!(kind, DemonSkullActionKind::Eyes | DemonSkullActionKind::Mouth) {
        brain.charge_glow = 0.0;
        world.emit_action_sound(
            DeathEffectOwner { id: actor.id, position: actor.position },
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
    actor.body_pose = brain.body_pose;
    brain.actions = actions;
}

fn step_action(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    mut action: DemonSkullAction,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) -> DemonSkullAction {
    let EnemyBrain::DemonSkull(brain) = actor.brain.clone() else { unreachable!() };
    let target = target_by_id(actor.target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target));
    let aim = target.map(|target| {
        heading_degrees_from_vector(
            target.position.x - actor.position.x,
            target.position.y - actor.position.y,
        )
    });
    let mut step =
        ActionStep { world, actor, brain, retired: action.retired, context, order, target, aim };
    match &mut action.program {
        DemonSkullActionProgram::Scream => {}
        DemonSkullActionProgram::Bite { progress, rate, marker_emitted } => {
            step.bite(progress, *rate, marker_emitted);
        }
        DemonSkullActionProgram::Eyes {
            warmup_ticks,
            shots_remaining,
            recovery_ticks,
            attack_speed,
        } => step.eyes(warmup_ticks, shots_remaining, recovery_ticks, *attack_speed),
        DemonSkullActionProgram::Mouth {
            warmup_ticks,
            warmup_jitter,
            warmup_turn_speed,
            beam_power,
            tracking_delay_ticks,
            tracking_ramp,
            recovery_ticks,
        } => step.mouth(
            warmup_ticks,
            warmup_jitter,
            warmup_turn_speed,
            beam_power,
            tracking_delay_ticks,
            tracking_ramp,
            recovery_ticks,
        ),
        DemonSkullActionProgram::Spit {
            shots_remaining,
            cooldown_ticks,
            mouth_ticks,
            target_heading_deg,
        } => step.spit(shots_remaining, cooldown_ticks, mouth_ticks, target_heading_deg),
        DemonSkullActionProgram::Flair {
            remaining_ticks,
            settling_ticks,
            glow,
            original_heading_deg,
        } => step.flair(remaining_ticks, settling_ticks, glow, *original_heading_deg),
    }
    step.actor.body_pose = step.brain.body_pose;
    step.actor.brain = EnemyBrain::DemonSkull(step.brain);
    action.retired = step.retired;
    action
}

/// One action's update of the Discorporeal: the world and painter order it acts in, the
/// actor, the brain copy the action edits before it is written back, whether the action
/// retired, and the step context with the eligible target and its heading.
struct ActionStep<'a, 'c> {
    world: &'a mut EnemyWorld,
    actor: &'a mut EnemyActor,
    brain: DemonSkullBrain,
    retired: bool,
    context: &'a EnemyStepContext<'c>,
    order: &'a mut WorldPainterOrder,
    target: Option<&'a EnemyTarget>,
    aim: Option<f64>,
}

impl ActionStep<'_, '_> {
    fn bite(&mut self, progress: &mut f64, rate: f64, marker_emitted: &mut bool) {
        if *progress == 0.0 {
            self.world.emit_action_sound(
                DeathEffectOwner::from(&*self.actor),
                self.context.tick,
                EnemySound::SkullBite,
                1.0,
            );
        }
        let time =
            self.context.ability_effect(self.actor.id).time_scale * self.actor.staff_action_factor;
        let next = fround(*progress + fround(rate * time));
        self.brain.body_pose = if next.trunc() < 4.0 { 1.0 } else { 0.0 };
        let marker = next > *progress && *progress <= 4.0 && next >= 4.0;
        if marker {
            self.actor.brain = EnemyBrain::DemonSkull(self.brain.clone());
            bite_demon_skull(self.world, self.actor, self.context, self.order);
            let EnemyBrain::DemonSkull(updated) = self.actor.brain.clone() else { unreachable!() };
            self.brain = updated;
        }
        *progress = next;
        *marker_emitted |= marker;
        self.retired = next > 13.0;
    }

    fn eyes(
        &mut self,
        warmup_ticks: &mut u64,
        shots_remaining: &mut usize,
        recovery_ticks: &mut u64,
        attack_speed: f64,
    ) {
        self.brain.charge_glow = fround(self.brain.charge_glow + fround(0.025)).min(1.0);
        if *warmup_ticks > 0 {
            self.brain.jitter = self.world.random_enemy_offset(3.0);
            spawn_demon_skull_warmup(self.world, self.actor, self.context.tick, true, self.order);
            let heading = self.aim.unwrap_or(self.actor.heading_deg);
            turn(self.actor, &mut self.brain, heading, 1.0);
            *warmup_ticks -= 1;
        } else if *shots_remaining == 0 {
            self.brain.charge_glow = fround(self.brain.charge_glow - fround(0.05)).max(0.0);
            *recovery_ticks -= 1;
            if *recovery_ticks == 0 {
                recover(&mut self.brain, self.actor.config.scale);
                self.retired = true;
            }
        } else {
            if *shots_remaining >= 3 {
                self.brain.jitter = self.world.random_enemy_offset(3.0);
            }
            let time = self.context.ability_effect(self.actor.id).time_scale
                * self.actor.staff_action_factor;
            self.brain.eye_charge =
                fround(self.brain.eye_charge + attack_speed * fround(0.025) * time).min(1.0);
            self.actor.heading_deg = self.aim.unwrap_or(self.actor.heading_deg);
            self.brain.heading_delay_ticks = 11;
            if self.brain.eye_charge >= 1.0 {
                self.brain.jitter = Vec2::ZERO;
                self.brain.eye_charge = 0.0;
                self.actor.brain = EnemyBrain::DemonSkull(self.brain.clone());
                fire_demon_skull_eyes(self.world, self.actor, self.context, self.order);
                let EnemyBrain::DemonSkull(updated) = self.actor.brain.clone() else {
                    unreachable!()
                };
                self.brain = updated;
                self.brain.spin = self.world.steering_rng.draw_scaled_float(15.0, true);
                self.brain.charge_glow = 0.0;
                *shots_remaining -= 1;
            }
        }
    }

    #[allow(clippy::too_many_arguments)] // The mouth beam's seven timers are its program's fields.
    fn mouth(
        &mut self,
        warmup_ticks: &mut u64,
        warmup_jitter: &mut f64,
        warmup_turn_speed: &mut f64,
        beam_power: &mut f64,
        tracking_delay_ticks: &mut i64,
        tracking_ramp: &mut f64,
        recovery_ticks: &mut i64,
    ) {
        self.brain.charge_glow = fround(self.brain.charge_glow + fround(0.025)).min(1.0);
        if *warmup_ticks > 0 {
            self.brain.jitter = self.world.random_enemy_offset(*warmup_jitter);
            spawn_demon_skull_warmup(self.world, self.actor, self.context.tick, false, self.order);
            if let Some(aim) = self.aim
                && *warmup_turn_speed > 0.0
            {
                turn(self.actor, &mut self.brain, aim, *warmup_turn_speed);
                *warmup_turn_speed = fround(*warmup_turn_speed - fround(0.004));
            }
            *warmup_ticks -= 1;
            if *warmup_ticks == 0 {
                flash(
                    self.world,
                    self.actor,
                    self.context.tick,
                    EnemyScreenFlash {
                        red: fround(0.55),
                        green: 1.0,
                        blue: fround(0.05),
                        alpha: 1.0,
                        decay_per_tick: fround(0.05),
                        point_attenuated: true,
                    },
                );
                self.world.emit_action_sound(
                    DeathEffectOwner::from(&*self.actor),
                    self.context.tick,
                    EnemySound::LightningStart,
                    1.0,
                );
            }
            *warmup_jitter = fround(*warmup_jitter + fround(0.05));
        } else if *beam_power <= 0.0 {
            *recovery_ticks -= 1;
            if *recovery_ticks < 0 {
                recover(&mut self.brain, self.actor.config.scale);
                self.brain.body_offset = Vec2::ZERO;
                self.retired = true;
            }
        } else {
            self.brain.body_pose = 1.0;
            self.brain.charge_glow = 0.0;
            self.brain.body_phase_deg = 0.0;
            self.brain.body_offset = self.world.random_enemy_offset(3.0);
            *tracking_delay_ticks -= 1;
            if *tracking_delay_ticks <= 0
                && let Some(aim) = self.aim
            {
                turn(self.actor, &mut self.brain, aim, fround(0.5 * *tracking_ramp));
                *tracking_ramp = fround(*tracking_ramp + fround(0.01)).min(1.0);
            }
            shake(self.world, self.actor, self.context.tick, CameraShakeAttenuation::Point, 10.0);
            self.actor.brain = EnemyBrain::DemonSkull(self.brain.clone());
            step_demon_skull_mouth_beam(
                self.world,
                self.actor,
                self.context,
                *beam_power,
                self.order,
            );
            *beam_power = fround(*beam_power - fround(0.015));
            if *beam_power <= 0.0 {
                self.brain.body_pose = 0.0;
                self.brain.body_offset = Vec2::ZERO;
                self.world.emit_action_sound(
                    DeathEffectOwner::from(&*self.actor),
                    self.context.tick,
                    EnemySound::Jaw,
                    1.0,
                );
            }
        }
    }

    fn spit(
        &mut self,
        shots_remaining: &mut usize,
        cooldown_ticks: &mut i64,
        mouth_ticks: &mut i64,
        target_heading_deg: &mut f64,
    ) {
        if *mouth_ticks > 0 {
            *mouth_ticks -= 1;
            if *mouth_ticks == 0 {
                self.brain.body_pose = 0.0;
            }
        } else {
            turn(self.actor, &mut self.brain, *target_heading_deg, 1.0);
            *target_heading_deg = self.target.map_or(self.actor.heading_deg, |target| {
                if target.summoned {
                    self.aim.expect("eligible target has aim")
                } else {
                    heading_degrees_from_vector(
                        target.position.x + target.velocity_per_tick.x * 260.0
                            - self.actor.position.x,
                        target.position.y + target.velocity_per_tick.y * 260.0
                            - self.actor.position.y,
                    )
                }
            });
            *cooldown_ticks -= 1;
            if *cooldown_ticks <= 0 && (self.actor.heading_deg - *target_heading_deg).abs() < 3.0 {
                self.brain.body_pose = 1.0;
                self.actor.brain = EnemyBrain::DemonSkull(self.brain.clone());
                spit_demon_skull_fire(self.world, self.actor, self.context, self.order);
                let EnemyBrain::DemonSkull(updated) = self.actor.brain.clone() else {
                    unreachable!()
                };
                self.brain = updated;
                *shots_remaining -= 1;
                if *shots_remaining == 0 {
                    recover(&mut self.brain, self.actor.config.scale);
                    self.brain.body_pose = 0.0;
                    self.retired = true;
                }
                *cooldown_ticks = 75;
                *mouth_ticks = 35;
            }
        }
    }

    fn flair(
        &mut self,
        remaining_ticks: &mut u64,
        settling_ticks: &mut u64,
        glow: &mut f64,
        original_heading_deg: f64,
    ) {
        self.brain.body_pose = 1.0;
        if *settling_ticks > 0 {
            self.actor.heading_deg = original_heading_deg;
            *settling_ticks += 1;
            if *settling_ticks >= 50 {
                self.brain.body_pose = 0.0;
                self.world.emit_action_sound(
                    DeathEffectOwner::from(&*self.actor),
                    self.context.tick,
                    EnemySound::Jaw,
                    1.0,
                );
                self.retired = true;
            }
        } else {
            *remaining_ticks -= 1;
            self.actor.heading_deg = fround(
                float::sin(*remaining_ticks as f64 / 200.0 * 630.0 * std::f64::consts::PI / 180.0)
                    * 40.0
                    + original_heading_deg,
            );
            self.brain.body_heading_deg = self.actor.heading_deg;
            self.brain.flair_glow = *glow;
            shake(self.world, self.actor, self.context.tick, CameraShakeAttenuation::Fixed, 3.0);
            if *remaining_ticks == 0 {
                *settling_ticks = 1;
            }
            *glow = fround(*glow + fround(0.05)).min(1.0);
        }
    }
}

fn turn(actor: &mut EnemyActor, brain: &mut DemonSkullBrain, heading: f64, speed: f64) {
    actor.heading_deg = positive_modulo(
        fround(
            actor.heading_deg
                + f64::from(heading_turn_direction(actor.heading_deg, heading)) * speed,
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
    actor: &EnemyActor,
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
