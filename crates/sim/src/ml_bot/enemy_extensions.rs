//! Source `ml-bot-policy/enemy-extensions.ts`: family clocks, shields and statuses.

use crate::enemies::config::EnemyFamilyConfig;
use crate::enemies::model::*;
use crate::enemies::world::EnemyWorld;
use crate::secondary::{NativeSecondaryActorKind, NativeSecondarySimulationState};

use super::enemies::{EnemySource, MlBotPolicyEnemyRow, MlBotPolicyEnemySpecies as Species};
use super::numbers::{heading_vector, ratio, unsigned, write};
use super::spec::scales;

pub fn observe_ml_bot_policy_enemy_extensions(
    enemies: &EnemyWorld,
    rows: &[MlBotPolicyEnemyRow],
    secondary: &NativeSecondarySimulationState,
    self_player_id: &str,
    tick: u64,
    world_key: &str,
) -> Vec<f32> {
    let mut block = vec![0.0; 8 * 44];
    for (slot, row) in rows.iter().take(8).enumerate() {
        let start = slot * 44;
        let species = [
            Species::Skeleton,
            Species::Archer,
            Species::Mage,
            Species::Imp,
            Species::Zombie,
            Species::Wraith,
            Species::Demon,
            Species::Coffin,
            Species::Maggot,
        ]
        .iter()
        .position(|species| *species == row.species);
        if let Some(index) = species {
            block[start + index] = 1.0;
        } else if start > 0 {
            // The deployed v7 Source uses indexOf on nine legacy species. Its -1
            // typed-array write is ignored for slot zero, and lands in the prior
            // row's last field for later slots. Keep that actual model input.
            block[start - 1] = 1.0;
        }
        let facing = heading_vector(row.heading_deg);
        let (clock, contact, shield, armor) = match row.source {
            EnemySource::Actor(index) => {
                let actor = &enemies.actors[index];
                let contact = match &actor.brain {
                    EnemyBrain::Skeleton(brain) => brain.contact_target_player_id.as_deref(),
                    EnemyBrain::Zombie(brain) => brain.contact_target_player_id.as_deref(),
                    _ => None,
                };
                (
                    enemy_clock(actor),
                    contact,
                    ratio(actor.shield_health, actor.shield_maximum_health),
                    matches!(actor.config.family, EnemyFamilyConfig::Skeleton { armor: true, .. }),
                )
            }
            EnemySource::Maggot(index) => {
                (maggot_clock(&enemies.maggots[index], tick), None, 0.0, false)
            }
        };
        write(&mut block, start + 9, &[facing.x, facing.y]);
        block[start + 11 + clock.phase as usize] = 1.0;
        write(
            &mut block,
            start + 21,
            &[
                action_time(clock.strike),
                action_time(clock.end),
                unsigned(clock.remaining / scales::TICK_RATE, scales::ENEMY_PHASE_SECONDS),
                f64::from(clock.marker),
                f64::from(row.target_player_id.as_deref() == Some(self_player_id)),
                f64::from(contact == Some(self_player_id)),
                unsigned(row.maximum_health, scales::HP),
                shield,
                f64::from(armor),
            ],
        );
        let Some(effect) = secondary
            .target_effects
            .iter()
            .find(|effect| effect.world_key == world_key && effect.target_id == row.id)
        else {
            block[start + 42] = 1.0;
            block[start + 43] = 1.0;
            continue;
        };
        let carrier_burn = secondary.actors.iter().any(|actor| {
            actor.target_id == Some(row.id)
                && actor.world_key == world_key
                && matches!(
                    actor.kind,
                    NativeSecondaryActorKind::FireBurn
                        | NativeSecondaryActorKind::EtherBurn
                        | NativeSecondaryActorKind::ElectricBurn
                )
        });
        write(
            &mut block,
            start + 30,
            &[
                f64::from(effect.cold_slow_ticks > 0),
                status_time(effect.cold_slow_ticks),
                f64::from(effect.frozen_ticks > 0),
                status_time(effect.frozen_ticks),
                f64::from(effect.stun_ticks > 0),
                status_time(effect.stun_ticks),
                f64::from(effect.flee_ticks > 0),
                status_time(effect.flee_ticks),
                f64::from(effect.dazzle_ticks > 0),
                f64::from(effect.disrupted_ticks > 0),
                f64::from(effect.prismatic_ticks > 0),
                f64::from(
                    effect.electric_burn.is_some()
                        || effect.frost_burn_ticks > 0
                        || effect.steamed.is_some()
                        || carrier_burn,
                ),
                effect.weaken_factor.clamp(-1.0, 1.0),
                effect.time_scale.clamp(-1.0, 1.0),
            ],
        );
    }
    block
}

#[derive(Clone, Copy)]
enum Phase {
    Approach,
    RangeControl,
    Orbit,
    Windup,
    Recover,
    Cooldown,
    Knockback,
    Dormant,
    Opening,
    Open,
}

struct Clock {
    marker: bool,
    phase: Phase,
    remaining: f64,
    end: Option<f64>,
    strike: Option<f64>,
}

fn passive(phase: Phase, remaining: f64) -> Clock {
    Clock { marker: false, phase, remaining, end: None, strike: None }
}

fn progress_clock(progress: f64, end: f64, rate: f64, markers: &[f64], emitted: bool) -> Clock {
    let marker = markers.iter().copied().find(|marker| *marker > progress);
    Clock {
        marker: emitted,
        phase: if marker.is_some() { Phase::Windup } else { Phase::Recover },
        remaining: 0.0,
        end: ticks_for_progress((end - progress).max(0.0), rate),
        strike: marker.and_then(|marker| ticks_for_progress(marker - progress, rate)),
    }
}

fn ticks_for_progress(progress: f64, rate: f64) -> Option<f64> {
    (rate > 0.0).then(|| (progress / rate).max(0.0))
}

fn enemy_clock(actor: &EnemyActor) -> Clock {
    let speed = (actor.config.attack_speed * actor.staff_action_factor).max(0.0);
    match &actor.brain {
        EnemyBrain::DemonSkull(brain) => {
            let Some(action) = brain.actions.iter().find(|action| !action.retired) else {
                return passive(Phase::Approach, 0.0);
            };
            let time = speed.max(0.0001);
            let (strike, end) = match &action.program {
                DemonSkullActionProgram::Scream | DemonSkullActionProgram::Flair { .. } => {
                    return passive(Phase::Approach, 0.0);
                }
                DemonSkullActionProgram::Bite { progress, rate, .. } => {
                    ((4.0 - progress) / (rate * time), (13.0 - progress) / (rate * time))
                }
                DemonSkullActionProgram::Eyes {
                    warmup_ticks,
                    shots_remaining,
                    recovery_ticks,
                    ..
                } => (
                    *warmup_ticks as f64 + (1.0 - brain.eye_charge) / (0.025 * time),
                    *warmup_ticks as f64
                        + (*shots_remaining as f64 - brain.eye_charge).max(0.0) / (0.025 * time)
                        + *recovery_ticks as f64,
                ),
                DemonSkullActionProgram::Mouth {
                    warmup_ticks, beam_power, recovery_ticks, ..
                } => (
                    *warmup_ticks as f64,
                    *warmup_ticks as f64
                        + beam_power.max(0.0) / 0.015
                        + *recovery_ticks as f64
                        + 1.0,
                ),
                DemonSkullActionProgram::Spit {
                    mouth_ticks,
                    cooldown_ticks,
                    shots_remaining,
                    ..
                } => {
                    let strike = *mouth_ticks as f64 + *cooldown_ticks as f64;
                    (strike, strike + (*shots_remaining as f64 - 1.0).max(0.0) * 110.0)
                }
            };
            Clock {
                marker: strike <= 0.0,
                phase: Phase::Windup,
                remaining: 0.0,
                end: Some(end.ceil().max(0.0)),
                strike: Some(strike.ceil().max(0.0)),
            }
        }
        EnemyBrain::Faculty(brain) => {
            let Some(action) = &brain.action else { return passive(Phase::RangeControl, 0.0) };
            Clock {
                marker: action.progress >= action.marker,
                phase: Phase::Windup,
                remaining: 0.0,
                end: ticks_for_progress(action.end - action.progress, action.rate * speed),
                strike: ticks_for_progress(action.marker - action.progress, action.rate * speed),
            }
        }
        EnemyBrain::Heartmonger(_) => passive(Phase::Approach, 0.0),
        EnemyBrain::Skeleton(brain) => {
            match brain.phase {
                SkeletonPhase::Approach => return passive(Phase::Approach, 0.0),
                SkeletonPhase::Death => return passive(Phase::Recover, 0.0),
                SkeletonPhase::Attack => {}
            }
            match brain.action {
                SkeletonAction::Claw => {
                    let next = [4.0, 8.0]
                        .into_iter()
                        .find(|marker| *marker > brain.action_progress)
                        .unwrap_or(12.0);
                    Clock {
                        marker: brain.marker_emitted,
                        phase: Phase::Windup,
                        remaining: 0.0,
                        end: ticks_for_progress(8.0 - brain.action_progress, 0.125 * speed),
                        strike: ticks_for_progress(next - brain.action_progress, 0.125 * speed),
                    }
                }
                SkeletonAction::Pike => progress_clock(
                    brain.action_progress,
                    12.0,
                    0.125 * speed,
                    &[2.0],
                    brain.marker_emitted,
                ),
                SkeletonAction::Weapon => progress_clock(
                    brain.action_progress,
                    24.0,
                    0.25 * speed,
                    &[9.0, 20.0],
                    brain.marker_emitted,
                ),
            }
        }
        EnemyBrain::Archer(brain) => match brain.phase {
            ArcherPhase::RangeControl => passive(Phase::RangeControl, 0.0),
            ArcherPhase::Death => passive(Phase::Recover, 0.0),
            ArcherPhase::Attack => progress_clock(
                brain.action_progress,
                16.0,
                0.0843750015 * speed,
                &[13.0],
                brain.marker_emitted,
            ),
        },
        EnemyBrain::Mage(brain) => match brain.phase {
            MagePhase::RangeControl => passive(Phase::RangeControl, 0.0),
            MagePhase::Death => passive(Phase::Recover, 0.0),
            MagePhase::Cast => {
                let (marker, end) = match brain.cast_program {
                    MageCastProgram::Long => (31.0, 47.0),
                    MageCastProgram::Short => (25.0, 41.0),
                };
                progress_clock(
                    brain.action_progress,
                    end,
                    0.253125012 * (1.0 + brain.cast_roll) * speed,
                    &[marker],
                    brain.marker_emitted,
                )
            }
        },
        EnemyBrain::Imp(brain) => passive(
            if brain.phase == FlightPhase::Death { Phase::Recover } else { Phase::Approach },
            0.0,
        ),
        EnemyBrain::Cocoon(_) => passive(Phase::Dormant, 0.0),
        EnemyBrain::Spider(brain) => {
            if brain.phase != SpiderPhase::Active {
                passive(Phase::Recover, 0.0)
            } else {
                passive(
                    if brain.action_state == 3 { Phase::Cooldown } else { Phase::Approach },
                    brain.action_ticks_remaining as f64,
                )
            }
        }
        EnemyBrain::Portal(brain) => passive(
            if brain.phase == PortalPhase::Death { Phase::Recover } else { Phase::Dormant },
            0.0,
        ),
        EnemyBrain::Zombie(brain) => match brain.phase {
            ZombiePhase::Approach => passive(Phase::Approach, brain.phase_ticks_remaining as f64),
            ZombiePhase::Knockback => {
                passive(Phase::Knockback, brain.impact_state_ticks_remaining as f64)
            }
            ZombiePhase::Death => passive(Phase::Recover, 0.0),
            ZombiePhase::Swipe => progress_clock(
                brain.action_progress,
                125.0,
                brain.action_rate,
                &[100.0],
                brain.marker_emitted,
            ),
        },
        EnemyBrain::Wraith(brain) => {
            if brain.phase == FlightPhase::Death {
                passive(Phase::Recover, 0.0)
            } else if brain.contact_cooldown_ticks > 0 {
                passive(Phase::Cooldown, brain.contact_cooldown_ticks as f64)
            } else if brain.flyby_ticks_remaining > 0 {
                passive(Phase::Orbit, brain.flyby_ticks_remaining as f64)
            } else {
                passive(Phase::Approach, 0.0)
            }
        }
        EnemyBrain::Demon(brain) => match brain.phase {
            DemonPhase::Approach => passive(Phase::Approach, 0.0),
            DemonPhase::Death => passive(Phase::Recover, 0.0),
            DemonPhase::Bomb => progress_clock(
                brain.action_progress,
                8.0,
                0.09375 * speed,
                &[4.0],
                brain.marker_emitted,
            ),
        },
        EnemyBrain::Coffin(brain) => passive(
            match brain.phase {
                CoffinPhase::Opening => Phase::Opening,
                CoffinPhase::Open => Phase::Open,
                CoffinPhase::Death => return passive(Phase::Recover, 0.0),
                CoffinPhase::Hidden | CoffinPhase::Rising | CoffinPhase::Holding => Phase::Dormant,
            },
            brain.phase_ticks_remaining as f64,
        ),
    }
}

fn maggot_clock(actor: &EnemyMaggotActor, tick: u64) -> Clock {
    if actor.movement_phase == MaggotMovementPhase::Emerging {
        return passive(Phase::Approach, 0.0);
    }
    let remaining = actor.next_attack_tick.saturating_sub(tick) as f64;
    Clock {
        marker: false,
        phase: if actor.last_attack_tick.is_some() && remaining > 0.0 {
            Phase::Cooldown
        } else {
            Phase::Approach
        },
        remaining,
        end: actor.target_player_id.as_ref().map(|_| remaining),
        strike: actor.target_player_id.as_ref().map(|_| remaining),
    }
}

fn action_time(ticks: Option<f64>) -> f64 {
    ticks.map_or(1.0, |ticks| unsigned(ticks / scales::TICK_RATE, scales::ENEMY_ACTION_SECONDS))
}

fn status_time(ticks: u64) -> f64 {
    unsigned(ticks as f64 / scales::TICK_RATE, scales::STATUS_DURATION_SECONDS)
}
