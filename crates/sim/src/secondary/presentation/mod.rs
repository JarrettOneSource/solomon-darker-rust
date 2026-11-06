//! Stock draw plans for every secondary actor, grouped into burst, field, weather and
//! Golem families. `tests/secondary_presentation_goldens.rs` replays complete recorded plans.
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
    leviathan_composite_plan, magic_circle_ring_draws, plane_orb_mesh, player_magic_shield_plan,
    prismatic_wave_draws, secondary_composite_owner_entries,
};
pub use golem::{
    GolemDeathMember, golem_death_member_plan, golem_death_presentation_plan, golem_facing,
    golem_presentation_plan,
};
pub use plan::{secondary_presentation_plan, secondary_presentation_plan_at_age};
pub use types::*;
pub use weather::{
    freeze_wave_visual_draws, raindrop_gradient, storm_auxiliary_draws, storm_cloud_draws,
    storm_weather_composite,
};
