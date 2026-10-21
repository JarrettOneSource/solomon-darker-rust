//! Stock draw plans from `renderer/native-secondary-presentation.ts` and its
//! `native-secondary-{burst,field,weather,golem}-presentation.ts` families.
//! `tests/secondary_presentation_goldens.rs` replays complete source plans.
//!
//! These are pure values; the GPU view owns atlas uploads, render targets and
//! painter submission. Presentation reconstructs children from captured RNG
//! without consuming the live gameplay stream.

mod bursts;
pub(crate) mod draws;
mod fields;
mod golem;
mod plan;
mod types;
mod weather;

pub use bursts::{
    dampen_draws, magic_trap_burst_draws, mindblast_burst_draws, shield_explosion_draws,
};
pub use draws::repeated_float_multiply;
pub use fields::{
    magic_circle_ring_draws, native_leviathan_composite_plan, native_player_magic_shield_plan,
    native_secondary_composite_owner_entries, plane_orb_mesh, prismatic_wave_draws,
};
pub use golem::{
    NativeGolemDeathMember, native_golem_death_member_plan, native_golem_death_presentation_plan,
    native_golem_facing, native_golem_presentation_plan,
};
pub use plan::{native_secondary_presentation_plan, native_secondary_presentation_plan_at_age};
pub use types::*;
pub use weather::{
    freeze_wave_visual_draws, raindrop_gradient, storm_auxiliary_draws, storm_cloud_draws,
    storm_weather_composite,
};
