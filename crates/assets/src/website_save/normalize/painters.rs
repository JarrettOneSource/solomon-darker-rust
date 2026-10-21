//! Saved world-manager enrollment and schema 21 Hub remapping.
//! Allocation order follows `migrateLegacyWorldPainterState`, then ownership validation.

mod enemies;
mod legacy;

use serde_json::{Value, json};
use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};

use super::super::contract::{MAX_SAFE_INTEGER, record};
use super::super::value::{decode, encode};
use super::{array, finite};

pub(super) fn normalize(mut value: Value, loaded: &Value, version: u64) -> Result<Value, String> {
    if version < 21 {
        legacy::migrate(&mut value, loaded)?;
    } else if version == 21 {
        legacy::schema_21_hub(&mut value)?;
    }
    let mut order: WorldPainterOrder = decode(value["worldManagerOrder"].clone())?;
    let missing = version < 23;
    let mut projectiles =
        array(&value["primarySpells"]["projectiles"], "game save primary projectiles")?.clone();
    for (index, actor) in projectiles.iter_mut().enumerate() {
        primary_actor(
            actor,
            (1, WorldManagerLane::Actor),
            &mut order,
            missing,
            &format!("game save primary projectile {index}"),
        )?;
    }
    let mut transients =
        array(&value["primarySpells"]["transients"], "game save primary transients")?.clone();
    for (index, actor) in transients.iter_mut().enumerate() {
        let field = format!("game save primary transient {index}");
        record(actor, &field)?;
        primary_actor(actor, primary_contract(actor)?, &mut order, missing, &field)?;
    }
    value["primarySpells"]["projectiles"] = Value::Array(projectiles);
    value["primarySpells"]["transients"] = Value::Array(transients);
    if value["world"]["kind"].as_str() == Some("boneyard") {
        enemies::normalize(&mut value["world"], &mut order, version)?;
    }
    value["worldManagerOrder"] = encode(&order)?;
    Ok(value)
}

fn valid_registration(value: &Value, lane: WorldManagerLane) -> bool {
    let lane = match lane {
        WorldManagerLane::Actor => "actor",
        WorldManagerLane::Transient => "transient",
    };
    value.is_object()
        && value["managerLane"].as_str() == Some(lane)
        && value["registrationOrdinal"].as_f64().is_some_and(|value| {
            value >= 0.0 && value.fract() == 0.0 && value <= MAX_SAFE_INTEGER as f64
        })
}

fn registration(order: &mut WorldPainterOrder, lane: WorldManagerLane) -> Result<Value, String> {
    encode(&order.register(lane))
}

fn registrations(
    order: &mut WorldPainterOrder,
    lane: WorldManagerLane,
    count: usize,
) -> Result<Value, String> {
    encode(&order.register_many(lane, count))
}

fn primary_contract(actor: &Value) -> Result<(usize, WorldManagerLane), String> {
    use WorldManagerLane::{Actor, Transient};
    Ok(match actor["kind"].as_str() {
        Some("weld-flame-lash-fade") => {
            (1, if actor["flameVisual"]["kind"] == "native-birth-v1" { Transient } else { Actor })
        }
        Some("air") => (3, Actor),
        Some("earth-impact") => (
            solomon_sim::spells::earth::earth_impact_fragment_count(finite(
                &actor["charge"],
                "legacy Earth impact charge",
            )?),
            Actor,
        ),
        // `NATIVE_ETHER_BLAST_PARTICLE_COUNT` in native-ether-blast.ts.
        Some("ether-blast") => (108, Transient),
        Some(
            "ether-impact" | "ether-pierce-streak" | "fire-explosion" | "fire-impact" | "water",
        ) => (1, Transient),
        Some(
            "player-staff-contact"
            | "player-staff-contact-knockback"
            | "player-staff-knockback"
            | "player-staff-melee"
            | "player-staff-spin",
        ) => (0, Actor),
        _ => (1, Actor),
    })
}

fn fill_primary_actor(
    actor: &mut Value,
    contract: (usize, WorldManagerLane),
    order: &mut WorldPainterOrder,
) -> Result<(), String> {
    let (count, lane) = contract;
    actor["painterRegistrations"] =
        if count == 1 && valid_registration(&actor["lightRegistration"], lane) {
            json!([actor["lightRegistration"]])
        } else {
            registrations(order, lane, count)?
        };
    Ok(())
}

fn primary_actor(
    actor: &mut Value,
    contract: (usize, WorldManagerLane),
    order: &mut WorldPainterOrder,
    missing: bool,
    field: &str,
) -> Result<(), String> {
    record(actor, field)?;
    let (count, lane) = contract;
    if let Some(existing) = actor.get("painterRegistrations") {
        if existing.as_array().is_none_or(|values| {
            values.len() != count || values.iter().any(|value| !valid_registration(value, lane))
        }) {
            return Err(format!("{field} painter registrations are invalid"));
        }
        return Ok(());
    }
    if !missing {
        return Err(format!("{field} painter registrations are missing"));
    }
    fill_primary_actor(actor, contract, order)
}

/// Registrations are identified structurally throughout the source JSON tree. Counters
/// deliberately do not match this shape and are adjusted separately by each migration.
fn map_actor_registrations(value: &mut Value, map: &dyn Fn(u64) -> u64) {
    match value {
        Value::Array(values) => {
            for value in values {
                map_actor_registrations(value, map);
            }
        }
        Value::Object(source) => {
            if source.get("managerLane").and_then(Value::as_str) == Some("actor")
                && let Some(ordinal) = source
                    .get("registrationOrdinal")
                    .and_then(Value::as_u64)
                    .filter(|value| *value <= MAX_SAFE_INTEGER)
            {
                source.insert("registrationOrdinal".into(), json!(map(ordinal)));
            } else {
                for value in source.values_mut() {
                    map_actor_registrations(value, map);
                }
            }
        }
        _ => {}
    }
}
