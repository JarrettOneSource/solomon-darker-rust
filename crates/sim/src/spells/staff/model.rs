//! Source staff rows. Identity, age and enrollment use the existing transient base.

use crate::profile::runtime::{PlayerStaffAttackOutcome, PlayerStaffDamageLane};
use crate::spells::model::PrimarySpellTransientBase;
use crate::{NativeRng, Vec2};

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativePlayerStaffActionBase {
    pub common: PrimarySpellTransientBase,
    pub contact_sequence: u64,
    pub heading_degrees: f64,
    pub origin: Vec2,
    pub outcome: PlayerStaffAttackOutcome,
    pub swoosh_pitch: f64,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct NativePlayerStaffMeleeAction {
    pub base: NativePlayerStaffActionBase,
    pub action_timing_factor: f64,
    pub base_progress_per_tick: f64,
    pub lane: PlayerStaffDamageLane,
    pub progress: f64,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct NativePlayerStaffSpinAction {
    pub base: NativePlayerStaffActionBase,
    pub countdown: f64,
    pub turn_sign: i8,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum NativePlayerStaffAction {
    Melee(NativePlayerStaffMeleeAction),
    Spin(NativePlayerStaffSpinAction),
}

impl NativePlayerStaffAction {
    #[must_use]
    pub fn base(&self) -> &NativePlayerStaffActionBase {
        match self {
            Self::Melee(action) => &action.base,
            Self::Spin(action) => &action.base,
        }
    }

    pub fn base_mut(&mut self) -> &mut NativePlayerStaffActionBase {
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

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeStaffProcSound {
    CriticalHit,
    DisableEnemy,
    Knockback,
    SpinAttack,
}

impl NativeStaffProcSound {
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

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct NativePlayerStaffContactEvent {
    pub common: PrimarySpellTransientBase,
    pub impact_sound_pitches: Vec<f64>,
    pub origin: Vec2,
    pub outcome: PlayerStaffAttackOutcome,
    pub proc_sound: Option<NativeStaffProcSound>,
    pub proc_sound_pitches: Vec<f64>,
    pub pike_break_sound_indexes: Vec<usize>,
    pub swoosh_pitch: f64,
    pub target_ids: Vec<String>,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct NativePlayerStaffContactKnockback {
    pub common: PrimarySpellTransientBase,
    pub delta: Vec2,
    pub remaining_ticks: u64,
    pub target_id: String,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct NativePlayerStaffPikeBreakVfx {
    pub common: PrimarySpellTransientBase,
    #[cfg_attr(feature = "persistence", serde(default = "pike_break_shadow_default"))]
    pub enhanced_shadow: bool,
    pub heading_degrees: f64,
    pub position: Vec2,
    pub presentation_rng: NativeRng,
    pub target_id: String,
}

#[cfg(feature = "persistence")]
fn pike_break_shadow_default() -> bool {
    true
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct NativePlayerStaffSmokeVfx {
    pub common: PrimarySpellTransientBase,
    pub alpha: f64,
    pub alpha_loss: f64,
    pub angular_velocity_degrees: f64,
    pub entry: usize,
    pub position: Vec2,
    pub rotation_degrees: f64,
    pub scale: f64,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct NativePlayerStaffMoveFadeVfx {
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

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct NativePlayerStaffPerspectiveVfx {
    pub common: PrimarySpellTransientBase,
    pub alpha: f64,
    pub alpha_loss: f64,
    pub entry: usize,
    pub position: Vec2,
    pub rotation_degrees: f64,
    pub scale: f64,
    pub tint: u32,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum NativePlayerStaffVfx {
    Smoke(NativePlayerStaffSmokeVfx),
    MoveFade(NativePlayerStaffMoveFadeVfx),
    PerspectiveFade(NativePlayerStaffPerspectiveVfx),
}

impl NativePlayerStaffVfx {
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

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct NativeStaffKnockbackActor {
    pub common: PrimarySpellTransientBase,
    pub arc_degrees: f64,
    pub origin: Vec2,
    pub remaining_distance: f64,
    pub target_ids: Vec<String>,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum NativePlayerStaffTransient {
    Action(NativePlayerStaffAction),
    Contact(NativePlayerStaffContactEvent),
    ContactKnockback(NativePlayerStaffContactKnockback),
    PikeBreak(NativePlayerStaffPikeBreakVfx),
    Vfx(NativePlayerStaffVfx),
    Knockback(NativeStaffKnockbackActor),
}

impl NativePlayerStaffTransient {
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
            Self::Action(NativePlayerStaffAction::Melee(_)) => "player-staff-melee",
            Self::Action(NativePlayerStaffAction::Spin(_)) => "player-staff-spin",
            Self::Contact(_) => "player-staff-contact",
            Self::ContactKnockback(_) => "player-staff-contact-knockback",
            Self::PikeBreak(_) => "player-staff-pike-break",
            Self::Vfx(NativePlayerStaffVfx::Smoke(_)) => "player-staff-smoke",
            Self::Vfx(NativePlayerStaffVfx::MoveFade(_)) => "player-staff-move-fade",
            Self::Vfx(NativePlayerStaffVfx::PerspectiveFade(_)) => "player-staff-perspective-fade",
            Self::Knockback(_) => "player-staff-knockback",
        }
    }
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativePlayerStaffActionStep {
    pub action: Option<NativePlayerStaffAction>,
    pub contact: bool,
    pub sample: NativePlayerStaffAction,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStaffTarget {
    pub collision_radius: f64,
    pub id: String,
    pub position: Vec2,
    pub registration_order: Option<usize>,
}

impl AsRef<NativeStaffTarget> for NativeStaffTarget {
    fn as_ref(&self) -> &Self {
        self
    }
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStaffPhysicalTarget {
    pub target: NativeStaffTarget,
    pub pike: bool,
}

impl AsRef<NativeStaffTarget> for NativeStaffPhysicalTarget {
    fn as_ref(&self) -> &NativeStaffTarget {
        &self.target
    }
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStaffPlayer {
    pub collision_radius: f64,
    pub heading_degrees: f64,
    pub position: Vec2,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStaffPhysicalImpact {
    pub contact_knockback_delta: Option<Vec2>,
    pub pike_break_presentation_rng: Option<NativeRng>,
    pub sound_pitch: f64,
    pub target_id: String,
    pub vertical_velocity: f64,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStaffContactPresentation {
    pub event: NativePlayerStaffContactEvent,
    pub next_id: u64,
    pub vfx: Vec<NativePlayerStaffVfx>,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStaffDisplacement {
    pub delta: Vec2,
    pub target_id: String,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStaffHeadingPerturbation {
    pub heading_degrees: f64,
    pub target_id: String,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStaffKnockbackStep {
    pub camera_displacement: Vec2,
    pub actor: Option<NativeStaffKnockbackActor>,
    pub dazzled_target_ids: Vec<String>,
    pub displacements: Vec<NativeStaffDisplacement>,
    pub heading_perturbations: Vec<NativeStaffHeadingPerturbation>,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStaffContactKnockbackStep {
    pub actor: Option<NativePlayerStaffContactKnockback>,
    pub displacement: Option<Vec2>,
}
