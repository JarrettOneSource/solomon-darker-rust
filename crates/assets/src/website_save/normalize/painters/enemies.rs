//! Projectile, death particle and Mage pulse painter ownership at the save boundary.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};

use super::super::super::contract::{MAX_SAFE_INTEGER, record};
use super::super::{finite, integer_within};
use super::{array, registration, valid_registration};

pub(super) fn normalize(
    world: &mut Value,
    order: &mut WorldPainterOrder,
    version: u64,
) -> Result<(), String> {
    record(&world["enemies"], "game save Boneyard enemies")?;
    let pulses = mage_pulses(&world["enemies"], version)?;
    let mut projectiles =
        array(&world["enemies"]["projectiles"], "game save enemy projectiles")?.clone();
    for projectile in &mut projectiles {
        projectile_owner(projectile, order, version < 32)?;
    }
    let mut death_effects =
        array(&world["enemies"]["deathEffects"], "game save enemy death effects")?.clone();
    for (index, effect) in death_effects.iter_mut().enumerate() {
        death_owner(effect, order, version, &format!("game save enemy death effect {index}"))?;
    }
    let mut loot_effects = array(&world["loot"]["effects"], "game save loot effects")?.clone();
    for (index, effect) in loot_effects.iter_mut().enumerate() {
        death_owner(effect, order, version, &format!("game save loot effect {index}"))?;
    }
    if version < 34 {
        let mut effects = Vec::new();
        for value in array(&world["enemies"]["projectileEffects"], "saved projectile effects")? {
            record(value, "saved projectile effect")?;
            let mut effect = value.clone();
            if value["kind"].as_str() == Some("firebolt-trail") {
                let Some(parent) = projectiles.iter().find(|parent| {
                    parent["id"] == value["ownerProjectileId"]
                        && parent["kind"].as_str() == Some("firebolt")
                }) else {
                    continue;
                };
                effect["blendMode"] = json!("add");
                effect["painterRegistration"] = parent["painterRegistration"].clone();
            }
            effects.push(effect);
        }
        world["enemies"]["projectileEffects"] = Value::Array(effects);
    }
    world["enemies"]["projectiles"] = Value::Array(projectiles);
    world["enemies"]["deathEffects"] = Value::Array(death_effects);
    world["enemies"]["mageLightningPulses"] = Value::Array(pulses);
    world["loot"]["effects"] = Value::Array(loot_effects);
    Ok(())
}

fn projectile_owner(
    actor: &mut Value,
    order: &mut WorldPainterOrder,
    migrate: bool,
) -> Result<(), String> {
    record(actor, "game save enemy projectile")?;
    let kind = actor["kind"].as_str().unwrap_or("");
    let lane = if matches!(kind, "arrow" | "firebolt") {
        WorldManagerLane::Transient
    } else {
        WorldManagerLane::Actor
    };
    let lit = matches!(kind, "firebolt" | "guided-missile" | "demon-bomb")
        || (kind == "arrow" && actor["payload"].as_str() == Some("fire"));
    if migrate {
        let painter = if valid_registration(&actor["lightRegistration"], lane) {
            actor["lightRegistration"].clone()
        } else if valid_registration(&actor["painterRegistration"], lane) {
            actor["painterRegistration"].clone()
        } else {
            registration(order, lane)?
        };
        actor["lightRegistration"] = if lit { painter.clone() } else { Value::Null };
        actor["painterRegistration"] = painter;
        return Ok(());
    }
    if !valid_registration(&actor["painterRegistration"], lane) {
        return Err("saved projectile painter registration is invalid".into());
    }
    let light = &actor["lightRegistration"];
    if if lit {
        !valid_registration(light, lane)
            || light["registrationOrdinal"] != actor["painterRegistration"]["registrationOrdinal"]
    } else {
        !light.is_null()
    } {
        return Err("saved projectile light registration is invalid".into());
    }
    Ok(())
}

fn death_owner(
    effect: &mut Value,
    order: &mut WorldPainterOrder,
    version: u64,
    field: &str,
) -> Result<(), String> {
    record(effect, field)?;
    let kind = effect["kind"].as_str().unwrap_or("");
    let role = effect["role"].as_str().unwrap_or("");
    let expected = death_presentation_owner(kind, role);
    let owner = if !effect.as_object().unwrap().contains_key("presentationOwner") && version < 23 {
        expected
    } else {
        effect["presentationOwner"].as_str().unwrap_or("")
    };
    let mixed_smoke = kind == "move-fade"
        && matches!(
            role,
            "dampen-caster-smoke"
                | "tragic-circle-smoke"
                | "skull-trail"
                | "dark-trail"
                | "skull-impact"
                | "dark-impact"
                | "dark-ring-impact"
                | "faculty-living-smoke"
                | "faculty-start-smoke"
                | "faculty-dying-smoke"
                | "faculty-terminal-smoke"
        )
        && owner == "pre-world-queue";
    if owner != expected && !mixed_smoke {
        return Err(format!("{field} presentation owner is invalid"));
    }
    let owner = owner.to_owned();
    if owner == "world-sorted" {
        if !valid_registration(&effect["painterRegistration"], WorldManagerLane::Transient) {
            if version >= 34 {
                return Err(format!("{field} painter registration is invalid"));
            }
            effect["painterRegistration"] = registration(order, WorldManagerLane::Transient)?;
        }
    } else {
        if !effect["painterRegistration"].is_null() && version >= 23 {
            return Err(format!(
                "{field} direct presentation must not retain a painter registration"
            ));
        }
        effect["painterRegistration"] = Value::Null;
    }
    effect["presentationOwner"] = json!(owner);
    Ok(())
}

fn death_presentation_owner(kind: &str, role: &str) -> &'static str {
    if kind == "unbind"
        || matches!(
            role,
            "dampen-caster-flash"
                | "mouth-beam-wall-contact"
                | "discorporeal-eyes-warmup"
                | "discorporeal-mouth-warmup"
        )
    {
        return "late-world-overlay";
    }
    if role == "ultra-banish-bone" || kind == "fade-perspective-clipped" {
        return "background";
    }
    if role == "eye-impact-flash" || role.starts_with("demon-death-fire-burst-") {
        return "direct-post-world";
    }
    if matches!(
        kind,
        "fire-array"
            | "late-splat"
            | "sprite-array"
            | "move-fade-perspective"
            | "fade-scale-perspective"
    ) || (kind == "move-fade-sin" && role == "zombie-rotten-particle")
        || (kind == "fade-additive" && role == "wraith-soul-wisp")
        || matches!(
            role,
            "blightning-source-smoke" | "blightning-path-smoke" | "discorporeal-death-branch"
        )
    {
        return "pre-world-queue";
    }
    "world-sorted"
}

fn mage_pulses(enemies: &Value, version: u64) -> Result<Vec<Value>, String> {
    let actors = array(&enemies["actors"], "game save Boneyard enemy actors")?;
    for (index, actor) in actors.iter().enumerate() {
        integer_within(
            &actor["id"],
            &format!("game save Boneyard enemy actor {index} id"),
            1,
            MAX_SAFE_INTEGER,
        )?;
    }
    let last_tick = finite(&enemies["lastStepTick"], "game save Boneyard enemy tick")?;
    if last_tick < -1.0 || last_tick.fract() != 0.0 || last_tick > MAX_SAFE_INTEGER as f64 {
        return Err("game save Boneyard enemy tick is invalid".into());
    }
    let mut result = Vec::new();
    let mut previous_id = 0;
    let mut previous_tick = -1.0;
    for (index, value) in array(&enemies["mageLightningPulses"], "game save Mage lightning pulses")?
        .iter()
        .enumerate()
    {
        let field = format!("game save Mage lightning pulse {index}");
        record(value, &field)?;
        let id = integer_within(&value["id"], &format!("{field} id"), 1, MAX_SAFE_INTEGER)?;
        let owner_id =
            integer_within(&value["ownerActorId"], &format!("{field} owner"), 1, MAX_SAFE_INTEGER)?;
        let tick = finite(&value["tick"], &format!("{field} tick"))?;
        if tick < 0.0 || tick > last_tick || tick.fract() != 0.0 {
            return Err(format!("{field} tick is out of range"));
        }
        if id <= previous_id || tick < previous_tick {
            return Err("game save Mage lightning pulse order is invalid".into());
        }
        // `NATIVE_MAGE_LIGHTNING_MAX_PULSE_AGES` is five enemy epochs.
        if last_tick - tick >= 5.0 {
            return Err(format!("{field} exceeds the live pulse age limit"));
        }
        previous_id = id;
        previous_tick = tick;
        record(&value["contact"], &format!("{field} contact"))?;
        let count = match value["contact"]["kind"].as_str() {
            Some("world") => 3,
            Some("target-attached") => 2,
            _ => return Err(format!("{field} contact kind is invalid")),
        };
        let painters =
            array(&value["painterRegistrations"], &format!("{field} painter registrations"))?;
        let ordinals: BTreeSet<_> =
            painters.iter().map(|value| value["registrationOrdinal"].as_u64()).collect();
        if painters.len() != count
            || ordinals.len() != count
            || painters.iter().any(|value| !valid_registration(value, WorldManagerLane::Actor))
        {
            return Err(format!("{field} painter registrations are invalid"));
        }
        let owner = actors.iter().find(|actor| actor["id"].as_u64() == Some(owner_id));
        let light = if version < 37 {
            owner.map(|value| &value["lightRegistration"]).unwrap_or(&Value::Null)
        } else {
            &value["lightRegistration"]
        };
        if !valid_registration(light, WorldManagerLane::Actor) {
            return Err(if version < 37 && owner.is_none() {
                format!(
                    "{field} cannot recover its creator registration from an absent legacy owner"
                )
            } else {
                format!("{field} creator registration is invalid")
            });
        }
        if painters
            .iter()
            .any(|painter| painter["registrationOrdinal"] == light["registrationOrdinal"])
        {
            return Err(format!("{field} creator and painter registrations must be distinct"));
        }
        if owner.is_some_and(|owner| {
            !valid_registration(&owner["lightRegistration"], WorldManagerLane::Actor)
                || owner["lightRegistration"]["registrationOrdinal"] != light["registrationOrdinal"]
        }) {
            return Err(format!("{field} creator registration does not match its owner"));
        }
        let mut pulse = value.clone();
        pulse["lightRegistration"] = light.clone();
        result.push(pulse);
    }
    Ok(result)
}
