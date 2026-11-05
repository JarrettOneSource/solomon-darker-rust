//! Staff actions and contact projections from `core-kernels/native-player-staff-action.ts`.
//! The host enrolls these rows in the canonical primary transient queue and owns
//! target mutation, actor collision and sound delivery.

mod model;
pub use model::*;
mod contact;
pub mod presentation;
mod vfx;
pub use contact::*;
pub use vfx::*;

use crate::float::fround;
use crate::profile::runtime::{
    PlayerSkillDerivedStats, PlayerStaffAttackOutcome, PlayerStaffDamageLane,
    resolve_player_staff_attack,
};
use crate::spells::model::PrimarySpellTransientBase;
use crate::{Rng, Vec2};

pub const STAFF_ADMISSION_HEADING_DEGREES: f64 = 50.0;
pub const STAFF_MELEE_MARKER_PROGRESS: f64 = 3.0;
pub const STAFF_MELEE_END_PROGRESS: f64 = 8.0;
pub const STAFF_MELEE_BASE_PROGRESS: f64 = 0.1_f32 as f64;
pub const STAFF_MELEE_ACCELERATION: f64 = 1.35_f32 as f64;
pub const STAFF_SPIN_COUNTDOWN: f64 = 360.0;
pub const STAFF_SPIN_STEP_DEGREES: f64 = 20.0;
pub const STAFF_WHIRL_RADIUS: f64 = 100.0;
pub const STAFF_KNOCKBACK_STEP: f64 = 10.0;
pub const STAFF_KNOCKBACK_DAZZLE_TICKS: u64 = 200;
pub const STAFF_CONTACT_EVENT_TICKS: u64 = 40;
pub const STAFF_CONTACT_KNOCKBACK_TICKS: u64 = 5;
pub const STAFF_CONTACT_KNOCKBACK_STEP: f64 = 6.0;
pub const STAFF_PIKE_BREAK_RNG_WORDS: usize = 50;
pub const STAFF_PIKE_BREAK_LIFETIME_TICKS: u64 = 100;

pub struct PlayerStaffActionInput<'a> {
    pub derived: &'a PlayerSkillDerivedStats,
    pub heading_degrees: f64,
    pub id: u64,
    pub lane: PlayerStaffDamageLane,
    pub origin: Vec2,
    pub owner_id: &'a str,
    pub world_key: &'a str,
}

#[must_use]
pub fn create_player_staff_action(
    input: PlayerStaffActionInput<'_>,
    rng: &mut Rng,
) -> PlayerStaffAction {
    let attack = resolve_player_staff_attack(input.derived, rng);
    let base = PlayerStaffActionBase {
        common: PrimarySpellTransientBase {
            age_ticks: 0,
            id: input.id,
            light_registration: None,
            owner_id: input.owner_id.into(),
            painter_registrations: Vec::new(),
            world_key: input.world_key.into(),
        },
        contact_sequence: 0,
        heading_degrees: normalized_degrees(input.heading_degrees),
        origin: input.origin,
        outcome: attack.outcome,
        swoosh_pitch: 1.0,
    };
    if attack.outcome == PlayerStaffAttackOutcome::Whirl {
        let turn_sign = if rng.draw_sign(1.0) < 0.0 { -1 } else { 1 };
        return PlayerStaffAction::Spin(PlayerStaffSpinAction {
            base,
            countdown: STAFF_SPIN_COUNTDOWN,
            turn_sign,
        });
    }
    let jitter = rng.draw_scaled_float(0.05, false);
    let acceleration = rng.draw_integer(8);
    let base_progress_per_tick = fround(
        fround(STAFF_MELEE_BASE_PROGRESS + jitter)
            * if acceleration == 2 { STAFF_MELEE_ACCELERATION } else { 1.0 },
    );
    PlayerStaffAction::Melee(PlayerStaffMeleeAction {
        base: PlayerStaffActionBase {
            swoosh_pitch: fround((base_progress_per_tick - STAFF_MELEE_BASE_PROGRESS) + 1.0),
            ..base
        },
        action_timing_factor: attack.action_timing_factor,
        base_progress_per_tick,
        lane: input.lane,
        progress: 0.0,
    })
}

#[must_use]
pub fn step_player_staff_action(
    source: &PlayerStaffAction,
    owner_position: Vec2,
) -> PlayerStaffActionStep {
    let mut sample = source.clone();
    sample.common_mut().age_ticks += 1;
    sample.base_mut().origin = owner_position;
    let (contact, retired) = match &mut sample {
        PlayerStaffAction::Spin(action) => {
            action.countdown -= STAFF_SPIN_STEP_DEGREES;
            action.base.heading_degrees = normalized_degrees(
                action.base.heading_degrees + f64::from(action.turn_sign) * STAFF_SPIN_STEP_DEGREES,
            );
            let finished = action.countdown <= 0.0;
            action.base.contact_sequence += u64::from(finished);
            (finished, finished)
        }
        PlayerStaffAction::Melee(action) => {
            let progress = fround(
                action.progress + action.base_progress_per_tick * action.action_timing_factor,
            );
            let contact = action.base.contact_sequence == 0
                && action.progress < STAFF_MELEE_MARKER_PROGRESS
                && progress >= STAFF_MELEE_MARKER_PROGRESS;
            action.base.contact_sequence += u64::from(contact);
            action.progress = progress;
            (contact, progress > STAFF_MELEE_END_PROGRESS)
        }
    };
    PlayerStaffActionStep {
        action: if retired { None } else { Some(sample.clone()) },
        contact,
        sample,
    }
}

#[must_use]
pub fn player_staff_action_pose(source: &PlayerStaffAction) -> usize {
    match source {
        PlayerStaffAction::Spin(_) => 3,
        PlayerStaffAction::Melee(action) => {
            let program = match action.lane {
                PlayerStaffDamageLane::Primary => [0, 4, 5, 6, 6, 6, 6, 6, 6],
                PlayerStaffDamageLane::Secondary => [0, 1, 2, 3, 3, 3, 3, 3, 3],
            };
            program[(action.progress.floor().max(0.0) as usize).min(program.len() - 1)]
        }
    }
}

pub(super) fn normalized_degrees(value: f64) -> f64 {
    let normalized = value % 360.0;
    if normalized < 0.0 { normalized + 360.0 } else { normalized }
}

/// Factory rows begin without enrollment; the host allocates only the source's
/// actual contact presentation painters, not the invisible action bodies.
pub(super) fn new_common(id: u64, action: &PlayerStaffAction) -> PrimarySpellTransientBase {
    PrimarySpellTransientBase {
        age_ticks: 0,
        id,
        light_registration: None,
        owner_id: action.common().owner_id.clone(),
        painter_registrations: Vec::new(),
        world_key: action.common().world_key.clone(),
    }
}
