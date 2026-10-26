//! Shared game client: stock assets, input, presentation, GPU UI and audio.
//!
//! Gameplay rules live in `solomon-sim`. The native application and browser WASM
//! embedding construct the same Game; platform modules own OS and Web API calls.

pub mod audio;
pub mod battle;
mod boneyard;
pub mod boneyard_weather;
pub mod cheats;
pub mod college_intro;
pub mod compact_masks;
mod courtyard;
pub mod deployment;
pub mod desktop;
pub mod enemies;
pub mod fullscreen;
mod game;
pub mod game_over;
pub mod gpu_ui;
pub mod hosted;
pub mod hotbar;
pub mod hud;
pub mod input;
pub mod loot;
pub mod match_loading;
pub mod menu;
mod mobile_editor;
pub mod mobile_layout;
pub mod mod_audio;
pub mod mod_consumable_effects;
pub mod mod_content;
pub mod mod_content_projection;
pub mod mod_minimap;
pub mod mod_panels;
pub mod mod_presentation_assets;
pub mod mod_skill_quickbar;
pub mod mod_skills;
pub mod mod_world_overlays;
mod native_mesh;
pub mod navigation_data;
pub mod network;
mod npc;
pub mod observer;
pub mod platform;
pub mod player_effects;
pub mod player_panels;
pub mod presentation;
mod rooms;
mod runtime_ui;
pub mod secondary;
#[cfg(all(target_arch = "wasm32", feature = "browser-fixtures"))]
mod secondary_fixture;
pub mod shared_gameplay;
mod skill_picker;
pub mod social;
pub mod solomon;
mod spectator;
pub mod spells;
mod storage;
mod teacher;
pub mod tutorial;
pub mod tutorial_scene;
pub mod ui;
pub mod wizard;
pub mod world_overlays;
pub mod world_screen_feedback;

#[cfg(target_arch = "wasm32")]
mod bootstrap;
#[cfg(target_arch = "wasm32")]
pub use bootstrap::*;
pub use game::Game;
pub(crate) use game::tasks::{load_pending_world, run_hosted_work, write_pending_save};
pub(crate) use runtime_ui::set_status;
#[cfg(not(target_arch = "wasm32"))]
pub use storage::configure_native_storage;
