//! Primary spell kernels from the pinned Website's `core-kernels/primary-spells.ts`.

pub mod air;
pub mod air_water;
pub mod boneyard;
pub mod casting;
pub mod contacts;
pub mod earth;
pub mod earth_debris;
mod earth_fragments;
pub mod earth_presentation;
pub mod effects;
pub mod ether;
pub mod ether_presentation;
pub mod fire;
pub mod fire_presentation;
mod lifecycle;
pub mod model;
pub mod presentation;
mod projectiles;
mod simulation;
pub use simulation::{
    PrimarySpellBatchTickResult, PrimarySpellCastAuthority, PrimarySpellEpochContext,
    PrimarySpellInput, PrimarySpellOwnerObstruction, PrimarySpellPlayer, PrimarySpellPlayerResult,
    PrimarySpellTraversalActor, PrimarySpellWorldContext,
    native_primary_birth_terrain_exclusion_mask, native_primary_flight_terrain_exclusion_mask,
};
pub mod special_death;
pub mod staff;
pub mod targeting;
mod visual;
pub mod water;
mod weapon_offsets;
pub mod weld;
