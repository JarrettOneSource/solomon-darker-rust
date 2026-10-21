//! Mutable prepared owners from `modding/content/*-engine.ts`.

pub mod affix;
pub mod effects;
pub mod enemies;
pub mod powerups;
pub mod scenes;
pub mod semantic;
pub mod shops;
pub mod skills;
pub mod spells;
pub mod statuses;

const MAXIMUM_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

fn validate_tick(value: u64, label: &str) -> Result<(), String> {
    if value > MAXIMUM_SAFE_INTEGER {
        return Err(format!("mod {label} tick is invalid"));
    }
    Ok(())
}

fn validate_tick_rate(value: u32, label: &str) -> Result<(), String> {
    if !(1..=1_000).contains(&value) {
        return Err(format!("mod {label} tick rate is invalid"));
    }
    Ok(())
}

fn validate_player(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.encode_utf16().count() > 128 {
        return Err(format!("mod {label} player is invalid"));
    }
    Ok(())
}

fn duration_ticks(milliseconds: f64, ticks_per_second: u32) -> u64 {
    (milliseconds * f64::from(ticks_per_second) / 1_000.0).ceil() as u64
}
