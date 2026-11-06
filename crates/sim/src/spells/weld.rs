//! The ten authored Welds, builds 1000 to 1009: their casting, world actors, audio and
//! presentation.

pub mod audio;
pub mod blizzard;
mod casting;
pub mod detonation;
pub mod effects;
pub mod flame_birth;
pub mod flame_cast;
pub mod flame_geometry;
pub mod flame_split;
pub mod flame_state;
pub mod hail;
pub mod meteor;
pub mod model;
pub mod persistent;
pub mod presentation;
pub mod projectiles;
pub mod steam;
mod world;
pub use audio::*;
pub use blizzard::*;
pub use detonation::*;
pub use effects::*;
pub use flame_birth::*;
pub use flame_cast::*;
pub use flame_geometry::*;
pub use flame_split::*;
pub use flame_state::*;
pub use hail::*;
pub use meteor::*;
pub use model::*;
pub use persistent::*;
pub use presentation::*;
pub use projectiles::*;
pub use steam::*;
pub use world::enroll_weld_actor_painter;
