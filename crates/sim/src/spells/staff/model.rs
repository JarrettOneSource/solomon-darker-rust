//! The staff attack's actors and step results. Identity, age and enrollment use the
//! existing transient base.

use crate::profile::runtime::{PlayerStaffAttackOutcome, PlayerStaffDamageLane};
use crate::spells::model::PrimarySpellTransientBase;
use crate::{Rng, Vec2};

/// What every staff attack shares: identity, origin, heading, rolled outcome, contact
/// count and swoosh pitch.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffActionBase {
    pub common: PrimarySpellTransientBase,
    pub contact_sequence: u64,
    pub heading_degrees: f64,
    pub origin: Vec2,
    pub outcome: PlayerStaffAttackOutcome,
    pub swoosh_pitch: f64,
}

/// A staff swing: its progress, speed and damage lane.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffMeleeAction {
    pub base: PlayerStaffActionBase,
    pub action_timing_factor: f64,
    pub base_progress_per_tick: f64,
    pub lane: PlayerStaffDamageLane,
    pub progress: f64,
}

/// A whirling staff spin: the degrees left to turn and the turn direction.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffSpinAction {
    pub base: PlayerStaffActionBase,
    pub countdown: f64,
    pub turn_sign: i8,
}

/// A staff attack in progress: a swing or a spin.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerStaffAction {
    Melee(PlayerStaffMeleeAction),
    Spin(PlayerStaffSpinAction),
}

impl PlayerStaffAction {
    #[must_use]
    pub fn base(&self) -> &PlayerStaffActionBase {
        match self {
            Self::Melee(action) => &action.base,
            Self::Spin(action) => &action.base,
        }
    }

    pub fn base_mut(&mut self) -> &mut PlayerStaffActionBase {
        match self {
            Self::Melee(action) => &mut action.base,
            Self::Spin(action) => &mut action.base,
        }
    }

    #[must_use]
    pub fn common(&self) -> &PrimarySpellTransientBase {
        &self.base().common
    }

    pub fn common_mut(&mut self) -> &mut PrimarySpellTransientBase {
        &mut self.base_mut().common
    }
}

/// The sound of a staff attack's special outcome: a critical hit, a disabling hit, a
/// knockback or a spin.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaffProcSound {
    CriticalHit,
    DisableEnemy,
    Knockback,
    SpinAttack,
}

impl StaffProcSound {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CriticalHit => "critical-hit",
            Self::DisableEnemy => "disable-enemy",
            Self::Knockback => "knockback",
            Self::SpinAttack => "spin-attack",
        }
    }
}

/// The moment a staff attack connects: the targets hit, the outcome and its sounds.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffContactEvent {
    pub common: PrimarySpellTransientBase,
    pub impact_sound_pitches: Vec<f64>,
    pub origin: Vec2,
    pub outcome: PlayerStaffAttackOutcome,
    pub proc_sound: Option<StaffProcSound>,
    pub proc_sound_pitches: Vec<f64>,
    pub pike_break_sound_indexes: Vec<usize>,
    pub swoosh_pitch: f64,
    pub target_ids: Vec<String>,
}

/// A short push on one target from an Ether staff hit's knockback chance.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffContactKnockback {
    pub common: PrimarySpellTransientBase,
    pub delta: Vec2,
    pub remaining_ticks: u64,
    pub target_id: String,
}

/// Debris from a pike broken by a staff hit, replayed from a saved presentation RNG.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffPikeBreakVfx {
    pub common: PrimarySpellTransientBase,
    #[cfg_attr(feature = "persistence", serde(default = "pike_break_shadow_default"))]
    pub enhanced_shadow: bool,
    pub heading_degrees: f64,
    pub position: Vec2,
    pub presentation_rng: Rng,
    pub target_id: String,
}

#[cfg(feature = "persistence")]
fn pike_break_shadow_default() -> bool {
    true
}

/// A turning puff of smoke from a staff contact that fades out.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffSmokeVfx {
    pub common: PrimarySpellTransientBase,
    pub alpha: f64,
    pub alpha_loss: f64,
    pub angular_velocity_degrees: f64,
    pub entry: usize,
    pub position: Vec2,
    pub rotation_degrees: f64,
    pub scale: f64,
}

/// A tinted staff contact particle that moves, slows and fades out.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffMoveFadeVfx {
    pub common: PrimarySpellTransientBase,
    pub alpha: f64,
    pub alpha_loss: f64,
    pub entry: usize,
    pub position: Vec2,
    pub rotation_degrees: f64,
    pub scale: f64,
    pub tint: u32,
    pub velocity: Vec2,
    pub velocity_factor: f64,
}

/// A tinted staff contact sprite that stays in place and fades out.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffPerspectiveVfx {
    pub common: PrimarySpellTransientBase,
    pub alpha: f64,
    pub alpha_loss: f64,
    pub entry: usize,
    pub position: Vec2,
    pub rotation_degrees: f64,
    pub scale: f64,
    pub tint: u32,
}

/// One staff contact particle: smoke, a moving fade or a stationary fade.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerStaffVfx {
    Smoke(PlayerStaffSmokeVfx),
    MoveFade(PlayerStaffMoveFadeVfx),
    PerspectiveFade(PlayerStaffPerspectiveVfx),
}

impl PlayerStaffVfx {
    #[must_use]
    pub fn common(&self) -> &PrimarySpellTransientBase {
        match self {
            Self::Smoke(vfx) => &vfx.common,
            Self::MoveFade(vfx) => &vfx.common,
            Self::PerspectiveFade(vfx) => &vfx.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut PrimarySpellTransientBase {
        match self {
            Self::Smoke(vfx) => &mut vfx.common,
            Self::MoveFade(vfx) => &mut vfx.common,
            Self::PerspectiveFade(vfx) => &mut vfx.common,
        }
    }
}

/// A staff knockback in progress: its origin, arc, remaining push distance and targets.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StaffKnockbackActor {
    pub common: PrimarySpellTransientBase,
    pub arc_degrees: f64,
    pub origin: Vec2,
    pub remaining_distance: f64,
    pub target_ids: Vec<String>,
}

/// A staff actor in the shared primary transient queue.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerStaffTransient {
    Action(PlayerStaffAction),
    Contact(PlayerStaffContactEvent),
    ContactKnockback(PlayerStaffContactKnockback),
    PikeBreak(PlayerStaffPikeBreakVfx),
    Vfx(PlayerStaffVfx),
    Knockback(StaffKnockbackActor),
}

impl PlayerStaffTransient {
    #[must_use]
    pub fn common(&self) -> &PrimarySpellTransientBase {
        match self {
            Self::Action(action) => action.common(),
            Self::Contact(event) => &event.common,
            Self::ContactKnockback(actor) => &actor.common,
            Self::PikeBreak(vfx) => &vfx.common,
            Self::Vfx(vfx) => vfx.common(),
            Self::Knockback(actor) => &actor.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut PrimarySpellTransientBase {
        match self {
            Self::Action(action) => action.common_mut(),
            Self::Contact(event) => &mut event.common,
            Self::ContactKnockback(actor) => &mut actor.common,
            Self::PikeBreak(vfx) => &mut vfx.common,
            Self::Vfx(vfx) => vfx.common_mut(),
            Self::Knockback(actor) => &mut actor.common,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Action(PlayerStaffAction::Melee(_)) => "player-staff-melee",
            Self::Action(PlayerStaffAction::Spin(_)) => "player-staff-spin",
            Self::Contact(_) => "player-staff-contact",
            Self::ContactKnockback(_) => "player-staff-contact-knockback",
            Self::PikeBreak(_) => "player-staff-pike-break",
            Self::Vfx(PlayerStaffVfx::Smoke(_)) => "player-staff-smoke",
            Self::Vfx(PlayerStaffVfx::MoveFade(_)) => "player-staff-move-fade",
            Self::Vfx(PlayerStaffVfx::PerspectiveFade(_)) => "player-staff-perspective-fade",
            Self::Knockback(_) => "player-staff-knockback",
        }
    }
}

/// One staff action tick: the action if it continues, whether it made contact this tick,
/// and the stepped action even when it retires.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffActionStep {
    pub action: Option<PlayerStaffAction>,
    pub contact: bool,
    pub sample: PlayerStaffAction,
}

/// A body a staff attack can hit: ID, position, collision radius and registration order.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StaffTarget {
    pub collision_radius: f64,
    pub id: String,
    pub position: Vec2,
    pub registration_order: Option<usize>,
}

impl AsRef<StaffTarget> for StaffTarget {
    fn as_ref(&self) -> &Self {
        self
    }
}

/// A staff target that takes physical hits, and whether it carries a pike.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StaffPhysicalTarget {
    pub target: StaffTarget,
    pub pike: bool,
}

impl AsRef<StaffTarget> for StaffPhysicalTarget {
    fn as_ref(&self) -> &StaffTarget {
        &self.target
    }
}

/// The attacking wizard's position, heading and collision radius.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffPlayer {
    pub collision_radius: f64,
    pub heading_degrees: f64,
    pub position: Vec2,
}

/// One physical staff hit: the target, sound pitch, vertical velocity, knockback and any
/// pike-break debris stream.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StaffPhysicalImpact {
    pub contact_knockback_delta: Option<Vec2>,
    pub pike_break_presentation_rng: Option<Rng>,
    pub sound_pitch: f64,
    pub target_id: String,
    pub vertical_velocity: f64,
}

/// What a staff contact presents: its contact event and particles, plus the next free ID.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StaffContactPresentation {
    pub event: PlayerStaffContactEvent,
    pub next_id: u64,
    pub vfx: Vec<PlayerStaffVfx>,
}

/// A staff knockback's push on one target.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StaffDisplacement {
    pub delta: Vec2,
    pub target_id: String,
}

/// A staff knockback's new heading for one target.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StaffHeadingPerturbation {
    pub heading_degrees: f64,
    pub target_id: String,
}

/// One staff knockback tick: pushes, heading changes, dazzled targets, camera
/// displacement and the actor if it continues.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StaffKnockbackStep {
    pub camera_displacement: Vec2,
    pub actor: Option<StaffKnockbackActor>,
    pub dazzled_target_ids: Vec<String>,
    pub displacements: Vec<StaffDisplacement>,
    pub heading_perturbations: Vec<StaffHeadingPerturbation>,
}

/// One contact knockback tick: the push and the actor if it continues.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StaffContactKnockbackStep {
    pub actor: Option<PlayerStaffContactKnockback>,
    pub displacement: Option<Vec2>,
}
