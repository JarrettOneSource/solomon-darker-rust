//! Versioned disk-state migrations from Website `save/game-save-document.ts`.
//!
//! Normalization happens before deserializing canonical simulation owners. Constructors
//! rebuild derived data; only source-persisted choices and clocks are restored over them.

mod death;
mod ether_drain;
pub(super) mod feedback;
pub(super) mod hub;
mod painters;
mod players;
mod protocol_values;
mod spells;
mod visual_state;
mod world;

use serde_json::{Map, Value, json};
use solomon_sim::NativeRng;

use super::contract::{integer, only_keys, record};
use super::value::encode;

pub(super) fn simulation(
    value: &Value,
    loaded: &Value,
    owner: &str,
    version: u64,
) -> Result<Value, String> {
    let source = record(value, "game save simulation")?;
    for (field, first_version) in [("enhancedEffects", 46), ("screenFlashes", 47)] {
        if version < first_version && source.contains_key(field) {
            return Err(format!("game save simulation has unexpected field {field}"));
        }
    }
    reject_unexpected_keys(
        source,
        "game save simulation",
        &[
            "accumulatorSeconds",
            "combatRng",
            "enhancedEffects",
            "gameRng",
            "hallOfFameClockStartedAtTick",
            "levelUpBarrier",
            "worldManagerOrder",
            "modEffects",
            "nextLevelUpBarrierId",
            "nextModConsumableUseId",
            "playerEntities",
            "primarySpells",
            "run",
            "screenFlashes",
            "secondaryAbilities",
            "tick",
            "world",
            "playerOfferRng",
        ],
        if version < 21 { &["lightProviderOrder"] } else { &[] },
    )?;
    let tick = integer(&value["tick"], "game save simulation tick")?;
    let game_rng = source
        .get("gameRng")
        .filter(|value| !value.is_null())
        .or_else(|| source.get("playerOfferRng"))
        .ok_or("game save simulation is missing its game RNG")?;
    let mut run = record(&value["run"], "game save run")?.clone();
    run.entry("gameOverExitKind").or_insert(Value::Null);
    run.entry("loadoutReadyPlayerIds").or_insert_with(|| json!([]));
    let normalized = json!({
        "accumulatorSeconds": value["accumulatorSeconds"],
        "combatRng": source.get("combatRng").filter(|value| !value.is_null())
            .cloned().unwrap_or(encode(&NativeRng::new(0))?),
        "enhancedEffects": if version < 46 { true } else { value["enhancedEffects"].as_bool().ok_or("game save simulation enhancedEffects must be boolean")? },
        "gameRng": game_rng,
        "hallOfFameClockStartedAtTick": source.get("hallOfFameClockStartedAtTick")
            .filter(|value| !value.is_null()).cloned().unwrap_or(json!(0)),
        "levelUpBarrier": value["levelUpBarrier"],
        "worldManagerOrder": source.get("worldManagerOrder").filter(|value| !value.is_null())
            .or_else(|| source.get("lightProviderOrder")).cloned().unwrap_or(Value::Null),
        "modEffects": source.get("modEffects").filter(|value| !value.is_null())
            .cloned().unwrap_or(json!([])),
        "nextLevelUpBarrierId": value["nextLevelUpBarrierId"],
        "nextModConsumableUseId": source.get("nextModConsumableUseId")
            .filter(|value| !value.is_null()).cloned().unwrap_or(json!(1)),
        "playerEntities": players::normalize(&value["playerEntities"], version, tick)?,
        "primarySpells": spells::primary(&value["primarySpells"], version)?,
        "run": run,
        "screenFlashes": if version < 47 {
            encode(&solomon_sim::screen_flash::NativeScreenFlashState::default())?
        } else { visual_state::screen_flashes(&value["screenFlashes"], "game save screenFlashes", tick)? },
        "secondaryAbilities": spells::secondary(&value["secondaryAbilities"], version, tick)?,
        "tick": tick,
        "world": world::normalize(&value["world"], loaded, owner, version, tick)?,
    });
    painters::normalize(normalized, loaded, version)
}

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, String> {
    value.as_array().ok_or_else(|| format!("{field} is invalid"))
}

fn finite(value: &Value, field: &str) -> Result<f64, String> {
    value.as_f64().filter(|value| value.is_finite()).ok_or_else(|| format!("{field} is invalid"))
}

fn integer_within(value: &Value, field: &str, minimum: u64, maximum: u64) -> Result<u64, String> {
    let value = integer(value, field)?;
    if value < minimum || value > maximum {
        return Err(format!("{field} is invalid"));
    }
    Ok(value)
}

fn signed_integer(value: &Value, field: &str, minimum: f64, maximum: f64) -> Result<f64, String> {
    let number = finite(value, field)?;
    if number.fract() != 0.0 || number < minimum || number > maximum {
        return Err(format!("{field} is invalid"));
    }
    Ok(number)
}

fn numeric_or_null(value: &Value) -> Value {
    if value.is_number() { value.clone() } else { Value::Null }
}

fn reject_unexpected_keys(
    source: &Map<String, Value>,
    field: &str,
    allowed: &[&str],
    additional: &[&str],
) -> Result<(), String> {
    for key in source.keys() {
        if !allowed.contains(&key.as_str()) && !additional.contains(&key.as_str()) {
            return Err(format!("{field} has unexpected field {key}"));
        }
    }
    Ok(())
}

fn puppet_hit(
    value: &Value,
    last_damage: &Value,
    tick: u64,
    version: u64,
    field: &str,
) -> Result<Value, String> {
    use solomon_sim::combat::PuppetHitState;
    if version < 34 {
        let mut hit = PuppetHitState { tick, ..PuppetHitState::default() };
        if !last_damage.is_null() {
            hit.receive(
                integer_within(last_damage, &format!("{field} damage tick"), 0, tick)?,
                1.0,
            );
        }
        hit.timer = hit.alpha(tick as f64);
        hit.tick = tick;
        return encode(&hit);
    }
    let source = record(value, field)?;
    only_keys(source, field, &["strength", "tick", "timer"])?;
    let strength = finite(&value["strength"], &format!("{field}.strength"))?;
    let timer = finite(&value["timer"], &format!("{field}.timer"))?;
    let update_tick = integer(&value["tick"], &format!("{field}.tick"))?;
    if strength < 0.0 || !(0.0..=1.0).contains(&timer) {
        return Err(format!("{field} is invalid"));
    }
    if update_tick > tick {
        return Err(format!("{field} update tick is in the future"));
    }
    Ok(Value::Object(source.clone()))
}
