//! Retained death crossing and cosmetic bouncer samples from
//! `renderer/player-death-burst-presentation.ts` and `player-death-weapon-presentation.ts`.

use crate::float;
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;

use crate::Vec2;
use crate::appearance::equipment::PlayerWeaponAppearance;
use crate::combat::{PlayerCombatState, player_death_frame_at_tick};
use crate::spells::casting::NativePlayerWeaponKind;

use super::{mix, stable_hash, unit};

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerDeathBurstTrigger {
    pub death_epoch: u64,
    pub player_id: String,
    pub position: Vec2,
    pub run_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerDeathBurstLayer {
    pub alpha: f64,
    pub offset: Vec2,
    pub scale: Vec2,
}

/// The canonical combat owner is borrowed; presentation retains only its observation.
#[derive(Clone, Copy)]
pub struct PlayerDeathObservation<'a> {
    pub player_id: &'a str,
    pub combat: &'a PlayerCombatState,
    pub position: Vec2,
}

#[derive(Clone, Copy)]
struct DeathObservation {
    death_epoch: u64,
    death_tick: u64,
}

/// A late join or restored dead player seeds the crossing without replaying an old burst.
/// Clock reversals within one death epoch cannot trigger the burst twice.
#[derive(Default)]
pub struct PlayerDeathBurstCrossingTracker {
    consumed: BTreeSet<(String, u64)>,
    observations: BTreeMap<String, DeathObservation>,
    run_id: String,
}

impl PlayerDeathBurstCrossingTracker {
    #[must_use]
    pub fn new(run_id: &str, players: &[PlayerDeathObservation<'_>]) -> Self {
        let mut tracker = Self::default();
        tracker.seed(run_id, players);
        tracker
    }

    pub fn update(
        &mut self,
        run_id: &str,
        players: &[PlayerDeathObservation<'_>],
    ) -> Vec<PlayerDeathBurstTrigger> {
        if run_id != self.run_id {
            self.seed(run_id, players);
            return Vec::new();
        }
        let mut triggers = Vec::new();
        for player in players {
            let current = DeathObservation {
                death_epoch: player.combat.death_epoch,
                death_tick: player.combat.death_tick,
            };
            let Some(previous) = self.observations.get(player.player_id).copied() else {
                self.seed_player(player.player_id, current);
                continue;
            };
            let key = (player.player_id.to_owned(), current.death_epoch);
            let crossed = !player.combat.corpse_consumed
                && current.death_epoch > 0
                && player_death_frame_at_tick(current.death_tick) == 3
                && (current.death_epoch > previous.death_epoch
                    || (current.death_epoch == previous.death_epoch
                        && player_death_frame_at_tick(previous.death_tick) < 3));
            if crossed && self.consumed.insert(key) {
                triggers.push(PlayerDeathBurstTrigger {
                    death_epoch: current.death_epoch,
                    player_id: player.player_id.to_owned(),
                    position: player.position,
                    run_id: run_id.to_owned(),
                });
            }
            self.observations.insert(
                player.player_id.to_owned(),
                DeathObservation {
                    death_epoch: current.death_epoch,
                    death_tick: if current.death_epoch == previous.death_epoch {
                        previous.death_tick.max(current.death_tick)
                    } else {
                        current.death_tick
                    },
                },
            );
        }
        self.observations.retain(|id, _| players.iter().any(|player| player.player_id == id));
        triggers
    }

    fn seed(&mut self, run_id: &str, players: &[PlayerDeathObservation<'_>]) {
        self.consumed.clear();
        self.observations.clear();
        self.run_id = run_id.to_owned();
        for player in players {
            self.seed_player(
                player.player_id,
                DeathObservation {
                    death_epoch: player.combat.death_epoch,
                    death_tick: player.combat.death_tick,
                },
            );
        }
    }

    fn seed_player(&mut self, id: &str, observation: DeathObservation) {
        self.observations.insert(id.to_owned(), observation);
        if observation.death_epoch > 0 && player_death_frame_at_tick(observation.death_tick) == 3 {
            self.consumed.insert((id.to_owned(), observation.death_epoch));
        }
    }
}

#[must_use]
pub fn player_death_burst_layers(
    trigger: &PlayerDeathBurstTrigger,
    age_ticks: f64,
) -> Vec<PlayerDeathBurstLayer> {
    assert!(age_ticks.is_finite(), "player death-burst age must be finite");
    // The view retains a fractional birth tick; only elapsed age is truncated.
    let age = age_ticks.trunc().max(0.0);
    if age >= 10.0 {
        return Vec::new();
    }
    let alpha = 1.0 - age / 10.0;
    let seed =
        stable_hash(&format!("{}:{}:{}", trigger.run_id, trigger.player_id, trigger.death_epoch));
    let travel = if age == 0.0 { 0.0 } else { (1.0 - 0.9_f64.powf(age)) / (1.0 - 0.9) };
    let mut layers = Vec::with_capacity(18);
    for index in 0..18 {
        let jitter = (unit(mix(seed, index * 3 + 1)) * 2.0 - 1.0) * 8.0;
        let angle = (f64::from(index) * 20.0 + jitter) * PI / 180.0;
        let radius = 15.0 + unit(mix(seed, index * 3 + 2)) * 5.0;
        let speed = 3.0 + unit(mix(seed, index * 3 + 3));
        let distance = radius + speed * travel;
        layers.push(PlayerDeathBurstLayer {
            alpha,
            offset: Vec2::new(float::cos(angle) * distance, float::sin(angle) * distance),
            scale: Vec2::new(0.5, 0.2),
        });
    }
    layers
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerDeathWeaponTrigger {
    pub death_epoch: u64,
    pub heading_index: usize,
    pub player_id: String,
    pub run_id: String,
    pub weapon: PlayerWeaponAppearance,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerDeathWeaponSample {
    pub height: f64,
    pub offset: Vec2,
    pub rotation_radians: f64,
    pub settled: bool,
}

#[must_use]
pub fn player_death_weapon_sample(
    trigger: &PlayerDeathWeaponTrigger,
    age_ticks: u64,
) -> PlayerDeathWeaponSample {
    let motion = legacy_player_death_weapon_motion(trigger, age_ticks, Vec2::ZERO);
    PlayerDeathWeaponSample {
        height: motion.height,
        offset: motion.position,
        rotation_radians: motion.rotation_degrees * PI / 180.0,
        settled: motion.bounce_velocity == 0.0,
    }
}

/// `save/legacy-death-animations.ts`: historical saves reconstruct the original
/// seeded display motion without drawing from the live secondary RNG.
#[must_use]
pub fn legacy_player_death_weapon_motion(
    trigger: &PlayerDeathWeaponTrigger,
    age_ticks: u64,
    origin: Vec2,
) -> crate::death_animations::NativeBouncerMotion {
    let kind = match trigger.weapon.kind {
        NativePlayerWeaponKind::Staff => "staff",
        NativePlayerWeaponKind::Wand => "wand",
    };
    let seed = stable_hash(&format!(
        "{}:{}:{}:{}:{}",
        trigger.run_id, trigger.player_id, trigger.death_epoch, kind, trigger.weapon.selector
    ));
    let heading = (trigger.heading_index % 24) as f64 * 15.0 * PI / 180.0;
    let direction = Vec2::new(float::sin(heading), -float::cos(heading));
    let radius = 15.0 + unit(mix(seed, 1)) * 10.0;
    let mut offset = Vec2::new(direction.x * 1.5 * (radius + 2.0), direction.y * 1.5 * radius);
    let mut velocity = Vec2::new(direction.x * 1.5, direction.y * 1.5);
    let mut height = -unit(mix(seed, 2)) * 20.0;
    let mut vertical_velocity = -(2.0 + unit(mix(seed, 3)) * 3.0);
    let mut bounce_velocity = vertical_velocity;
    let mut rotation_degrees = unit(mix(seed, 4)) * 360.0;
    let mut rotation_speed = 1.0 + unit(mix(seed, 5)) * 10.0;
    let mut settled = false;
    let mut bounce = 0;
    let skipped_phase = u64::from(mix(seed, 6) % 3);
    for tick in 0..age_ticks {
        if settled {
            break;
        }
        if tick % 3 == skipped_phase {
            continue;
        }
        offset.x += velocity.x;
        offset.y += velocity.y;
        height += vertical_velocity;
        vertical_velocity += 0.4;
        if height > 0.0 {
            bounce_velocity *= 0.65;
            vertical_velocity = bounce_velocity;
            height = vertical_velocity;
            if mix(seed, 100 + bounce * 2) & 1 == 1 {
                velocity.x *= 0.65;
                velocity.y *= 0.65;
            }
            rotation_speed = 1.0 + unit(mix(seed, 101 + bounce * 2)) * 10.0;
            bounce += 1;
            if vertical_velocity > -0.75 {
                height = 0.0;
                velocity = Vec2::ZERO;
                rotation_speed = 0.0;
                settled = true;
            }
        }
        rotation_degrees += rotation_speed;
    }
    crate::death_animations::NativeBouncerMotion {
        bounce_velocity: if settled { 0.0 } else { bounce_velocity },
        height,
        position: Vec2::new(origin.x + offset.x, origin.y + offset.y),
        rotation_degrees,
        rotation_step_degrees: rotation_speed,
        velocity,
        vertical_velocity: if settled { 0.0 } else { vertical_velocity },
    }
}

/// Stock hat sockets used by the nine layered death body passes.
#[must_use]
pub fn player_death_hat_anchor(frame: usize, facing: usize) -> Vec2 {
    DEATH_HAT_ANCHORS[frame][facing]
}

// Stock `player-character-death-hat-anchors.json`: frame × six death facings.
const DEATH_HAT_ANCHORS: [[Vec2; 6]; 4] = [
    [
        Vec2::new(0.5, -3.5),
        Vec2::new(1.0, -3.0),
        Vec2::new(0.5, -2.5),
        Vec2::new(-0.5, -2.5),
        Vec2::new(-1.0, -3.0),
        Vec2::new(-0.5, -4.0),
    ],
    [
        Vec2::new(0.5, -18.0),
        Vec2::new(16.0, -9.5),
        Vec2::new(15.5, 5.5),
        Vec2::new(-0.5, 13.0),
        Vec2::new(-16.0, 5.0),
        Vec2::new(-15.5, -10.5),
    ],
    [
        Vec2::new(0.5, -29.5),
        Vec2::new(41.0, -9.0),
        Vec2::new(40.5, 31.0),
        Vec2::new(-0.5, 50.5),
        Vec2::new(-41.0, 30.0),
        Vec2::new(-40.5, -10.0),
    ],
    [
        Vec2::new(0.5, -30.5),
        Vec2::new(51.5, -5.0),
        Vec2::new(51.0, 45.5),
        Vec2::new(-0.5, 70.5),
        Vec2::new(-51.5, 44.5),
        Vec2::new(-51.0, -5.5),
    ],
];
