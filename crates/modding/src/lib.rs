//! The authority for mod content: package admission, definition compilation, the Lua VMs
//! and the engines that run authored content.
//!
//! The same canonical Rust Session owns stock and custom effects. This crate
//! compiles accepted packages and owns their Lua VMs; shared plain values
//! remain in `solomon_sim::modding` for persistence and client presentation.

#![forbid(unsafe_code)]

pub mod assets;
pub mod authoring;
pub mod console;
pub mod content;
pub mod definition;
mod duration;
pub mod engines;
pub mod host;
mod host_intents;
pub mod host_runtime;
mod host_transaction;
pub mod intents;
pub mod json;
pub mod package;
pub mod player_control;
pub mod rules;
pub mod session;
pub mod state;
