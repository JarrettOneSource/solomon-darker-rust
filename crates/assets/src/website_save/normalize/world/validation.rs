//! Source validation for versioned world timers, feedback and retained target bindings.

use std::collections::BTreeSet;

use serde_json::Value;

use super::super::super::contract::{MAX_SAFE_INTEGER, only_keys, record};
use super::super::{array, finite, integer_within, puppet_hit, signed_integer};

pub(super) fn demon_skull_encounter(value: &Value) -> Result<(), String> {
    record(value, "saved DemonSkull encounter")?;
    if value["healthTriggersEnabled"].as_bool().is_none() {
        return Err("saved DemonSkull health trigger activation must be boolean".into());
    }
    for (field, name, maximum) in [
        ("deathStreamTicksRemaining", "death stream", 600),
        ("screamStreamTicksRemaining", "scream stream", 275),
        ("healthTriggersFiredMask", "fired health triggers", 15),
        ("pendingCapabilities", "pending health triggers", 15),
    ] {
        integer_within(&value[field], &format!("saved DemonSkull {name}"), 0, maximum)?;
    }
    Ok(())
}

pub(super) fn target_bindings(value: &Value) -> Result<(), String> {
    for value in record(value, "game save target cell bindings")?.values() {
        record(value, "game save target cell binding")?;
        for key in ["X", "Y"] {
            signed_integer(
                &value[format!("cell{key}")],
                &format!("target cell {key}"),
                -(MAX_SAFE_INTEGER as f64),
                MAX_SAFE_INTEGER as f64,
            )?;
        }
        integer_within(&value["order"], "target cell order", 0, MAX_SAFE_INTEGER)?;
    }
    Ok(())
}

pub(super) fn knockbacks(value: &Value, tick: f64) -> Result<(), String> {
    for (index, value) in array(value, "game save projectile knockbacks")?.iter().enumerate() {
        let field = format!("game save projectile knockback {index}");
        only_keys(
            record(value, &field)?,
            &field,
            &["actorId", "delta", "eventId", "lastStepTick", "playerId", "remainingTicks"],
        )?;
        let point = format!("{field} displacement");
        only_keys(record(&value["delta"], &point)?, &point, &["x", "y"])?;
        let x = finite(&value["delta"]["x"], &format!("{point} X"))?;
        let y = finite(&value["delta"]["y"], &format!("{point} Y"))?;
        if x.abs() > 4.5 || y.abs() > 4.5 {
            return Err(format!("{point} is invalid"));
        }
        if value["playerId"]
            .as_str()
            .is_none_or(|id| id.is_empty() || id.encode_utf16().count() > 128)
        {
            return Err(format!("{field} player identity is invalid"));
        }
        integer_within(&value["actorId"], &format!("{field} actor"), 1, MAX_SAFE_INTEGER)?;
        integer_within(&value["eventId"], &format!("{field} event"), 1, MAX_SAFE_INTEGER)?;
        signed_integer(&value["lastStepTick"], &format!("{field} clock"), 0.0, tick)?;
        integer_within(&value["remainingTicks"], &format!("{field} remaining ticks"), 1, 10)?;
    }
    Ok(())
}

pub(super) fn puppet_hits(value: &Value, tick: u64) -> Result<(), String> {
    let hits = array(value, "saved Puppet hits")?;
    let mut ids = BTreeSet::new();
    for (index, value) in hits.iter().enumerate() {
        let field = format!("saved Puppet hits[{index}]");
        only_keys(record(value, &field)?, &field, &["feedback", "hitTick", "kind", "targetId"])?;
        let prefixes: &[&str] = match value["kind"].as_str() {
            Some("scenery") => &["scenery:", "fencepost:"],
            Some("goodie") => &["goodie:"],
            Some("arrow" | "firebolt") => &["projectile:"],
            Some("meteor") => &["primary:"],
            Some("leviathan") => &["secondary:"],
            _ => return Err(format!("{field}.kind is invalid")),
        };
        let id = value["targetId"]
            .as_str()
            .filter(|id| !id.is_empty() && id.encode_utf16().count() <= 256)
            .ok_or_else(|| format!("{field}.targetId is invalid"))?;
        if !prefixes.iter().any(|prefix| id.starts_with(prefix) && id.len() > prefix.len())
            || !ids.insert(id)
        {
            return Err(format!("{field}.targetId is invalid or duplicated"));
        }
        puppet_hit(&value["feedback"], &Value::Null, tick, 37, &format!("{field}.feedback"))?;
        let hit_tick =
            integer_within(&value["hitTick"], &format!("{field}.hitTick"), 0, MAX_SAFE_INTEGER)?;
        if value["feedback"]["timer"].as_f64().is_none_or(|timer| timer <= 0.0)
            || value["feedback"]["tick"].as_u64().is_none_or(|update| hit_tick > update)
        {
            return Err(format!("{field} hit clock is invalid"));
        }
    }
    Ok(())
}
