//! Ether Drain's retained field state: the targets it has found, its private animations
//! and the world animations it pulls in.
//! A registration ordinal distinguishes a removed/reused world ID from its former actor.

mod field;
mod kernel;
mod trees;
pub(super) use field::step_field;
pub use kernel::*;

use crate::Vec2;

/// Something an Ether Drain field pulls: an enemy, Golem, loot or player.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename_all = "kebab-case"))]
pub enum EtherDrainTargetRef {
    Enemy { id: u64, registration_ordinal: usize },
    Golem { id: u64 },
    Loot { id: u64, registration_ordinal: usize },
    Player { id: String },
}

/// A world animation an Ether Drain field pulls: death effects, transients, secondary
/// actors, death weapons and Golem fragments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename_all = "kebab-case"))]
pub enum EtherDrainWorldAnimationRef {
    EnemyDeathEffect { id: u64 },
    PrimaryTransient { id: u64 },
    SecondaryActor { id: u64 },
    DeathWeapon { id: u64 },
    GolemFragment { id: u64, index: usize },
}

/// The highest world animation ID seen in each family, so newer animations can join.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EtherDrainWorldAnimationIds {
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

/// The atlas a captured enemy's image comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EtherDrainAtlas {
    BadGuys,
    Demon,
}

/// A captured family image before the field supplies its own alpha and kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EtherDrainCapturedImage {
    pub atlas: EtherDrainAtlas,
    pub entry: usize,
    pub tint: u32,
    pub body_y_offset: u32,
}

/// One of an Ether Drain field's private animations: flying debris or a captured image.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename_all = "kebab-case"))]
pub enum EtherDrainAnimation {
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
        atlas: EtherDrainAtlas,
        entry: usize,
        tint: u32,
        body_y_offset: u32,
    },
}

/// An Ether Drain field's retained targets, scenery, private animations and world
/// animation cursors.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EtherDrainState {
    pub animations: Vec<EtherDrainAnimation>,
    pub targets_initialized: bool,
    /// -1 is the initial notification cursor; actual registrations are nonnegative.
    pub last_loot_registration_ordinal: i64,
    pub loose: Vec<EtherDrainTargetRef>,
    pub queried: Vec<EtherDrainTargetRef>,
    pub scenery: Vec<u64>,
    pub world_animation_refs: Vec<EtherDrainWorldAnimationRef>,
    pub last_world_animation_ids: EtherDrainWorldAnimationIds,
}

impl Default for EtherDrainState {
    fn default() -> Self {
        Self {
            animations: Vec::new(),
            targets_initialized: false,
            last_loot_registration_ordinal: -1,
            loose: Vec::new(),
            queried: Vec::new(),
            scenery: Vec::new(),
            world_animation_refs: Vec::new(),
            last_world_animation_ids: EtherDrainWorldAnimationIds::default(),
        }
    }
}
