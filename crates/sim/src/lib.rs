//! Deterministic Solomon Dark simulation kernels.
//!
//! Every kernel mirrors the Website's native-verified `core-kernels`, including the exact
//! points at which values are narrowed to 32-bit floats. The kernels use no platform
//! randomness, time, or floating-point library functions whose rounding differs between
//! targets, so a native test run and the browser build produce identical state.
//! `tests/native_goldens.rs` replays the retail-recorded goldens in `tests/fixtures/native`
//! and `tests/website_goldens.rs` replays the Website differential fixtures.
pub mod actor_physics;
pub mod appearance;
pub mod audio;
pub mod belt;
pub mod boneyard;
pub mod cold_aura;
pub mod collision;
pub mod combat;
pub mod constants;
pub mod crow_blindness;
pub mod death_animations;
pub mod economy;
mod ecs;
pub use ecs::{ComponentEntities, ComponentIter, OptionalEntity};
pub mod element_vfx;
pub mod enemies;
pub mod equipment;
pub mod float;
pub mod game_run;
pub mod hall_of_fame;
pub mod harden_effects;
pub mod harmful;
pub mod heading;
pub mod hub;
pub mod hud;
pub mod hurricane;
pub mod input;
pub mod knockback;
pub mod line;
pub mod line_obstruction;
pub mod loot;
pub mod menu;
pub mod ml_bot;
pub mod mobile_ui;
pub mod modding;
pub mod movement;
pub mod native_ui_notebox;
pub mod native_ui_text;
pub mod observer_activity;
pub mod painter;
#[cfg(feature = "persistence")]
pub mod persistence;
pub mod player;
pub mod player_panels;
pub mod player_presentation;
pub mod player_rescue;
pub mod presentation;
pub mod profile;
pub mod random_domain;
pub mod region_feedback;
pub mod rng;
pub mod score;
pub mod screen_feedback;
pub mod screen_flash;
pub mod secondary;
pub mod session;
pub mod settings;
pub mod shared_worlds;
pub mod skill_picker;
pub mod social;
pub mod spells;
pub mod spline;
pub mod tutorial_surface;
pub mod viewport;
pub mod world_labels;

pub use appearance::{StarterColorOwner, StarterEquipmentAppearance};
pub use collision::{CollisionRectangle, SecondaryResponse, Vec2};
pub use hub::{HubCollisionMove, HubSegment, HubWizard, HubWizardPlan};
pub use movement::{EnemyActor, EnemyFamily, KnockbackState, PlayerActor, PlayerMovementState};
pub use rng::NativeRng;
