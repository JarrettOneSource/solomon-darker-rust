//! Retained field identities and private animations from `native-ether-drain.ts`.
//! A registration ordinal distinguishes a removed/reused world ID from its former actor.

mod field;
mod kernel;
mod trees;
pub(super) use field::step_field;
pub use kernel::*;

use crate::Vec2;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename_all = "kebab-case"))]
pub enum NativeEtherDrainTargetRef {
    Enemy { id: u64, registration_ordinal: usize },
    Golem { id: u64 },
    Loot { id: u64, registration_ordinal: usize },
    Player { id: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename_all = "kebab-case"))]
pub enum NativeEtherDrainWorldAnimationRef {
    EnemyDeathEffect { id: u64 },
    PrimaryTransient { id: u64 },
    SecondaryActor { id: u64 },
    DeathWeapon { id: u64 },
    GolemFragment { id: u64, index: usize },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEtherDrainWorldAnimationIds {
    #[cfg_attr(feature = "persistence", serde(rename = "enemy-death-effect"))]
    pub enemy_death_effect: u64,
    #[cfg_attr(feature = "persistence", serde(rename = "primary-transient"))]
    pub primary_transient: u64,
    #[cfg_attr(feature = "persistence", serde(rename = "secondary-actor"))]
    pub secondary_actor: u64,
    #[cfg_attr(feature = "persistence", serde(rename = "death-weapon"))]
    pub death_weapon: u64,
    #[cfg_attr(feature = "persistence", serde(rename = "golem-fragment"))]
    pub golem_fragment: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeEtherDrainAtlas {
    BadGuys,
    Demon,
}

/// A captured family image before the field supplies its own alpha and kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEtherDrainCapturedImage {
    pub atlas: NativeEtherDrainAtlas,
    pub entry: usize,
    pub tint: u32,
    pub body_y_offset: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename_all = "kebab-case"))]
pub enum NativeEtherDrainAnimation {
    Debris {
        position: Vec2,
        direction: Vec2,
        remaining_distance: f64,
        speed: f64,
        oscillation_degrees: f64,
        rotation_degrees: f64,
        variant: u32,
    },
    Captured {
        alpha: f64,
        atlas: NativeEtherDrainAtlas,
        entry: usize,
        tint: u32,
        body_y_offset: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEtherDrainState {
    pub animations: Vec<NativeEtherDrainAnimation>,
    pub targets_initialized: bool,
    /// Source's -1 is the initial notification cursor; actual registrations are nonnegative.
    pub last_loot_registration_ordinal: i64,
    pub loose: Vec<NativeEtherDrainTargetRef>,
    pub queried: Vec<NativeEtherDrainTargetRef>,
    pub scenery: Vec<u64>,
    pub world_animation_refs: Vec<NativeEtherDrainWorldAnimationRef>,
    pub last_world_animation_ids: NativeEtherDrainWorldAnimationIds,
}

impl Default for NativeEtherDrainState {
    fn default() -> Self {
        Self {
            animations: Vec::new(),
            targets_initialized: false,
            last_loot_registration_ordinal: -1,
            loose: Vec::new(),
            queried: Vec::new(),
            scenery: Vec::new(),
            world_animation_refs: Vec::new(),
            last_world_animation_ids: NativeEtherDrainWorldAnimationIds::default(),
        }
    }
}
