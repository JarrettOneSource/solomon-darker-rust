//! Native authoritative host composition. Gameplay state belongs to `GameSession`;
//! connection input scheduling and wall-clock readiness belong to this host.

pub mod checkpoint;
pub mod control;
pub mod hosted;
pub mod input;

mod admission;
mod audio;
mod authority;
mod commands;
mod connection;
mod content;
pub mod editor_test;
mod flame;
mod membership;
mod memorial;
mod persistence;
pub mod recovery;
mod rejoin;
pub mod run_archive;
pub mod runtime_events;
mod signing;
mod social;
pub mod supervisor;

pub use authority::{NativeAuthority, NativeHostConfig};
pub use content::HostContent;

/// Source `Date.toISOString` includes exactly three millisecond digits, including
/// whole seconds. Both signed receipts and runtime events use this wire spelling.
fn source_utc_timestamp(now: time::OffsetDateTime) -> Result<String, String> {
    let format = time::format_description::parse_borrowed::<1>(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z",
    )
    .expect("the Source UTC format is fixed");
    now.format(&format).map_err(|error| format!("Cannot format Source UTC time: {error}"))
}

fn source_wall_clock_ms() -> f64 {
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as f64
}
