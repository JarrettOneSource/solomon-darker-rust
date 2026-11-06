//! `playtime-store.ts`: title, loading and settings count toward page/app lifetime.
//! Every flush adds to the latest committed total from all open instances.

use super::{scoped_key, with_preferences};
use std::cell::Cell;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::PlaytimeRecorder;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::PlaytimeRecorder;

const STORAGE_KEY: &str = "sdr.game.playtime.v1";
const FLUSH_INTERVAL_MS: u64 = 30_000;

fn read(value: Option<&str>) -> u64 {
    let value = value.map_or(0.0, crate::ui::parse_javascript_number);
    if value.is_finite() && value.fract() == 0.0 && (0.0..=9_007_199_254_740_991.0).contains(&value)
    {
        value as u64
    } else {
        0
    }
}

pub fn read_total_playtime_ms() -> Result<u64, String> {
    let key = scoped_key(STORAGE_KEY)?;
    with_preferences(|storage| Ok(read(storage.get(&key)?.as_deref())))
}

fn flush(marked_at: &Cell<f64>) -> Result<(), String> {
    let now = crate::platform::epoch_ms();
    let elapsed = (now - marked_at.get()).round().max(0.0);
    if elapsed == 0.0 {
        return Ok(());
    }
    let key = scoped_key(STORAGE_KEY)?;
    with_preferences(|storage| {
        let total = read(storage.get(&key)?.as_deref()) as f64 + elapsed;
        storage.set(&key, &total.to_string())
    })?;
    marked_at.set(now);
    Ok(())
}
