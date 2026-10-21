//! Authored-content values shared by the Rust authority and client.
//!
//! Source: Website `game/modding/` and `protocol/game-mod-contract.ts`.
//! The native `solomon-modding` crate owns Lua VMs. These values contain no VM
//! handles, platform I/O or second player/world state.

pub mod assets;
pub mod console;
pub mod consumable_presentation;
pub mod content;
pub mod definition;
pub mod events;
pub mod extensions;
pub mod identity;
pub mod inventory;
pub mod presentation;
pub mod projection;
pub mod runtime;
pub mod skill_quickbar;
pub mod ui;
pub mod value;

pub use value::{LuaObject, LuaValue};
