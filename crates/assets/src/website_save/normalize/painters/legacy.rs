//! Legacy allocation order; constructor admissions must not reorder these births.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solomon_sim::hub::painter::HUB_FIXED_ACTOR_PAINTER_COUNT;
use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};
use solomon_sim::secondary::{NativeSecondaryActorKind, native_secondary_painter_manager_lane};

use super::super::super::contract::record;
use super::super::super::value::{decode, encode};
use super::{
    array, fill_primary_actor, map_actor_registrations, primary_contract, registration,
    registrations,
};

pub(super) fn schema_21_hub(value: &mut Value) -> Result<(), String> {
    if value["world"]["kind"].as_str() != Some("hub") {
        return Ok(());
    }
    // Schema 21 registered eleven fixed Hub objects; the complete source set has 48.
    let offset = HUB_FIXED_ACTOR_PAINTER_COUNT as u64 - 11;
    map_actor_registrations(value, &|ordinal| ordinal + offset);
    let mut players = Vec::new();
    for (index, lighting) in
        array(&value["playerEntities"]["lightings"], "schema 21 Hub player lightings")?
            .iter()
            .enumerate()
    {
        players.push(actor_ordinal(
            &lighting["lightRegistration"],
            &format!("schema 21 Hub player lighting {index}"),
        )?);
    }
    players.sort_unstable();
    let first = *players.first().ok_or("schema 21 Hub player lightings must contain a player")?;
    let mut initial_players = Vec::new();
    for ordinal in players {
        if ordinal != first + initial_players.len() as u64 {
            break;
        }
        initial_players.push(ordinal);
    }
    let mut students = Vec::new();
    for (index, student) in
        array(&value["world"]["studentPopulation"]["students"], "schema 21 Hub Students")?
            .iter()
            .enumerate()
    {
        let ordinal = actor_ordinal(
            &student["painterRegistration"],
            &format!("schema 21 Hub Student {index}"),
        )?;
        if ordinal < first {
            students.push(ordinal);
        }
    }
    students.sort_unstable();
    let mut remap = BTreeMap::new();
    let fixed = HUB_FIXED_ACTOR_PAINTER_COUNT as u64;
    for (index, ordinal) in initial_players.iter().enumerate() {
        remap.insert(*ordinal, fixed + index as u64);
    }
    for (index, ordinal) in students.iter().enumerate() {
        remap.insert(*ordinal, fixed + initial_players.len() as u64 + index as u64);
    }
    map_actor_registrations(value, &|ordinal| remap.get(&ordinal).copied().unwrap_or(ordinal));
    let mut order: WorldPainterOrder = decode(value["worldManagerOrder"].clone())?;
    order.next_actor_ordinal += offset as usize;
    value["worldManagerOrder"] = encode(&order)?;
    Ok(())
}

pub(super) fn migrate(value: &mut Value, loaded: &Value) -> Result<(), String> {
    let hub = value["world"]["kind"].as_str() == Some("hub");
    let offset = if hub { HUB_FIXED_ACTOR_PAINTER_COUNT as u64 } else { 0 };
    map_actor_registrations(value, &|ordinal| ordinal + offset);
    let mut order: WorldPainterOrder = decode(value["worldManagerOrder"].clone())?;
    order.next_actor_ordinal += offset as usize;
    let progressions =
        array(&value["playerEntities"]["progressions"], "legacy player progressions")?;
    let mut lightings =
        array(&value["playerEntities"]["lightings"], "legacy player lightings")?.clone();
    for (index, lighting) in lightings.iter_mut().enumerate() {
        record(lighting, &format!("legacy player lighting {index}"))?;
        let progression = progressions.get(index).ok_or("legacy player progression is absent")?;
        lighting["deathWeaponPainterRegistration"] =
            if progression["lifeState"].as_str() == Some("dying") {
                registration(&mut order, WorldManagerLane::Actor)?
            } else {
                Value::Null
            };
    }
    value["playerEntities"]["lightings"] = Value::Array(lightings);
    for (field, projectile) in [("projectiles", true), ("transients", false)] {
        let mut actors =
            array(&value["primarySpells"][field], &format!("legacy primary {field}"))?.clone();
        for actor in &mut actors {
            record(actor, "legacy primary actor")?;
            if actor.get("painterRegistrations").is_none() {
                let contract = if projectile {
                    (1, WorldManagerLane::Actor)
                } else {
                    primary_contract(actor)?
                };
                fill_primary_actor(actor, contract, &mut order)?;
            }
        }
        value["primarySpells"][field] = Value::Array(actors);
    }
    let mut actors =
        array(&value["secondaryAbilities"]["actors"], "legacy secondary actors")?.clone();
    for actor in &mut actors {
        record(actor, "legacy secondary actor")?;
        if actor.get("painterRegistrations").is_none() {
            let kind: NativeSecondaryActorKind = decode(actor["kind"].clone())?;
            actor["painterRegistrations"] =
                registrations(&mut order, native_secondary_painter_manager_lane(kind), 1)?;
        }
    }
    value["secondaryAbilities"]["actors"] = Value::Array(actors);
    if hub {
        let mut students =
            array(&value["world"]["studentPopulation"]["students"], "legacy Hub Students")?.clone();
        for student in &mut students {
            fill_actor(student, &mut order)?;
        }
        value["world"]["studentPopulation"]["students"] = Value::Array(students);
    } else {
        boneyard(&mut value["world"], loaded, &mut order)?;
    }
    value["worldManagerOrder"] = encode(&order)?;
    Ok(())
}

fn boneyard(
    value: &mut Value,
    loaded: &Value,
    order: &mut WorldPainterOrder,
) -> Result<(), String> {
    record(&value["enemies"], "legacy Boneyard enemies")?;
    for field in ["projectiles", "projectileEffects", "deathEffects"] {
        let mut actors = array(&value["enemies"][field], &format!("legacy enemy {field}"))?.clone();
        for actor in &mut actors {
            record(actor, "legacy enemy presentation")?;
            if actor.get("painterRegistration").is_some() {
                continue;
            }
            let kind = actor["kind"].as_str().unwrap_or("");
            let role = actor["role"].as_str().unwrap_or("");
            actor["painterRegistration"] =
                if field == "deathEffects" && role.starts_with("demon-death-fire-burst-") {
                    Value::Null
                } else {
                    registration(
                        order,
                        if field == "projectileEffects" && kind.starts_with("fire-burst-") {
                            WorldManagerLane::Transient
                        } else {
                            WorldManagerLane::Actor
                        },
                    )?
                };
        }
        value["enemies"][field] = Value::Array(actors);
    }
    let mut pulses =
        array(&value["enemies"]["mageLightningPulses"], "legacy Mage lightning pulses")?.clone();
    for pulse in &mut pulses {
        record(pulse, "legacy Mage lightning pulse")?;
        if pulse.get("painterRegistrations").is_none() {
            record(&pulse["contact"], "legacy Mage lightning contact")?;
            let count = if pulse["contact"]["kind"].as_str() == Some("world") { 3 } else { 2 };
            pulse["painterRegistrations"] = registrations(order, WorldManagerLane::Actor, count)?;
        }
    }
    value["enemies"]["mageLightningPulses"] = Value::Array(pulses);
    record(&value["loot"], "legacy Boneyard loot")?;
    for field in ["actors", "effects"] {
        let mut actors = array(&value["loot"][field], &format!("legacy loot {field}"))?.clone();
        for actor in &mut actors {
            fill_actor(actor, order)?;
        }
        value["loot"][field] = Value::Array(actors);
    }
    let objects = array(&loaded["scene"]["objects"], "game save Boneyard scene objects")?;
    let mut goodies = array(&value["loot"]["goodies"], "legacy Goodies")?.clone();
    for goodie in &mut goodies {
        record(goodie, "legacy Goodie")?;
        if goodie.get("sceneryRegistrationOrdinal").is_some() {
            continue;
        }
        let eid = goodie["eid"].as_str().unwrap_or("");
        let ordinal = objects
            .iter()
            .position(|object| object["eid"].as_str() == Some(eid))
            .ok_or_else(|| format!("legacy Goodie {eid} is absent from its Boneyard scene"))?;
        goodie["sceneryRegistrationOrdinal"] = json!(ordinal);
    }
    value["loot"]["goodies"] = Value::Array(goodies);
    value["solomonPainterRegistration"] = if value["encounter"].is_null() {
        Value::Null
    } else {
        registration(order, WorldManagerLane::Actor)?
    };
    Ok(())
}

fn fill_actor(actor: &mut Value, order: &mut WorldPainterOrder) -> Result<(), String> {
    record(actor, "legacy world actor")?;
    if actor.get("painterRegistration").is_none() {
        actor["painterRegistration"] = registration(order, WorldManagerLane::Actor)?;
    }
    Ok(())
}

fn actor_ordinal(value: &Value, field: &str) -> Result<u64, String> {
    record(value, &format!("{field} registration"))?;
    if value["managerLane"].as_str() != Some("actor") {
        return Err(format!("{field} must use the actor manager"));
    }
    super::super::integer_within(
        &value["registrationOrdinal"],
        &format!("{field} registration ordinal"),
        0,
        super::super::super::contract::MAX_SAFE_INTEGER,
    )
}
