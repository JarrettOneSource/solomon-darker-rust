//! Ether Drain's saved target cursors and animation owners from `codecs/secondary-actors.ts`.

use std::collections::BTreeSet;

use serde_json::Value;

use super::death::MAX_DEATH_EFFECTS;
use super::protocol_values::{
    boolean, bounded_integer, finite, integer, limited_array, member, object, player_id, positive,
    vector,
};

const ANIMATION_KINDS: &[&str] = &[
    "enemy-death-effect",
    "primary-transient",
    "secondary-actor",
    "death-weapon",
    "golem-fragment",
];

pub(super) fn validate(value: &Value, field: &str) -> Result<(), String> {
    object(
        value,
        field,
        &[
            "animations",
            "targetsInitialized",
            "lastLootRegistrationOrdinal",
            "loose",
            "queried",
            "scenery",
            "worldAnimationRefs",
            "lastWorldAnimationIds",
        ],
    )?;
    let loose = refs(&value["loose"], field, "loose", 2047)?;
    refs(&value["queried"], field, "queried", 512 + 2048 + 64 + 32768)?;
    let cursor = bounded_integer(
        &value["lastLootRegistrationOrdinal"],
        &format!("{field}.lastLootRegistrationOrdinal"),
        -1,
        9_007_199_254_740_991,
    )?;
    if loose.iter().any(|reference| {
        reference["registrationOrdinal"].as_f64().is_some_and(|ordinal| ordinal > cursor as f64)
    }) {
        return Err(format!("{field} has a loose target beyond its registration cursor"));
    }
    let animations =
        limited_array(&value["animations"], &format!("{field}.animations"), (8192 + 1) * 1061 + 1)?;
    let mut captured = 0;
    for (index, animation) in animations.iter().enumerate() {
        let key = format!("{field}.animations[{index}]");
        if !animation.is_object() {
            return Err(format!("{key} must be an object"));
        }
        if animation["kind"] == "debris" {
            object(
                animation,
                &key,
                &[
                    "kind",
                    "position",
                    "direction",
                    "remainingDistance",
                    "speed",
                    "oscillationDegrees",
                    "rotationDegrees",
                    "variant",
                ],
            )?;
            vector(&animation["position"], &format!("{key}.position"))?;
            vector(&animation["direction"], &format!("{key}.direction"))?;
            positive(&animation["remainingDistance"], &format!("{key}.remainingDistance"))?;
            positive(&animation["speed"], &format!("{key}.speed"))?;
            finite(&animation["oscillationDegrees"], &format!("{key}.oscillationDegrees"))?;
            finite(&animation["rotationDegrees"], &format!("{key}.rotationDegrees"))?;
            bounded_integer(&animation["variant"], &format!("{key}.variant"), 0, 2)?;
            continue;
        }
        if animation["kind"] != "captured" {
            return Err(format!("{key}.kind is not a field animation"));
        }
        captured += 1;
        object(animation, &key, &["kind", "alpha", "atlas", "entry", "tint", "bodyYOffset"])?;
        let atlas = member(&animation["atlas"], &format!("{key}.atlas"), &["BadGuys", "Demon"])?;
        let entry = integer(&animation["entry"], &format!("{key}.entry"), true)?;
        if (atlas == "BadGuys"
            && !((1477..=1584).contains(&entry) || (2293..=2346).contains(&entry)))
            || (atlas == "Demon" && !(80..=97).contains(&entry))
        {
            return Err(format!("{key}.entry is outside native capture art"));
        }
        if positive(&animation["alpha"], &format!("{key}.alpha"))? > 1.25 {
            return Err(format!("{key}.alpha exceeds its capture birth"));
        }
        let body_y = if atlas == "BadGuys" && entry <= 1584 { 23.0 } else { 0.0 };
        if animation["bodyYOffset"].as_f64() != Some(body_y) {
            return Err(format!("{key}.bodyYOffset does not match its native capture art"));
        }
        bounded_integer(&animation["tint"], &format!("{key}.tint"), 0, 0xffffff)?;
    }
    if captured > 1 {
        return Err(format!("{field}.animations has multiple captured images after a replacement"));
    }
    let scenery = limited_array(&value["scenery"], &format!("{field}.scenery"), 8192)?;
    let mut scenery_ids = BTreeSet::new();
    for (index, id) in scenery.iter().enumerate() {
        scenery_ids.insert(integer(id, &format!("{field}.scenery[{index}]"), false)?);
    }
    if scenery_ids.len() != scenery.len() {
        return Err(format!("{field}.scenery contains duplicate references"));
    }
    let saved_ids = &value["lastWorldAnimationIds"];
    object(saved_ids, &format!("{field}.lastWorldAnimationIds"), ANIMATION_KINDS)?;
    for kind in ANIMATION_KINDS {
        integer(&saved_ids[kind], &format!("{field}.lastWorldAnimationIds.{kind}"), false)?;
    }
    let world_refs = limited_array(
        &value["worldAnimationRefs"],
        &format!("{field}.worldAnimationRefs"),
        MAX_DEATH_EFFECTS * 2 + 16384 + 32768 * 31,
    )?;
    let mut keys = BTreeSet::new();
    for (index, reference) in world_refs.iter().enumerate() {
        let key = format!("{field}.worldAnimationRefs[{index}]");
        if !reference.is_object() {
            return Err(format!("{key} must be an object"));
        }
        let kind = member(&reference["kind"], &format!("{key}.kind"), ANIMATION_KINDS)?;
        object(
            reference,
            &key,
            if kind == "golem-fragment" { &["kind", "id", "index"] } else { &["kind", "id"] },
        )?;
        let id = integer(&reference["id"], &format!("{key}.id"), true)?;
        if id as f64 > saved_ids[kind].as_f64().expect("validated animation cursor") {
            return Err(format!("{key} is beyond its animation registration cursor"));
        }
        let index = if kind == "golem-fragment" {
            Some(bounded_integer(&reference["index"], &format!("{key}.index"), 0, 29)?)
        } else {
            None
        };
        keys.insert((kind, id, index));
    }
    if keys.len() != world_refs.len() {
        return Err(format!("{field}.worldAnimationRefs contains duplicate references"));
    }
    boolean(&value["targetsInitialized"], &format!("{field}.targetsInitialized"))
}

fn refs<'a>(
    value: &'a Value,
    field: &str,
    lane: &str,
    maximum: usize,
) -> Result<&'a Vec<Value>, String> {
    let references = limited_array(value, &format!("{field}.{lane}"), maximum)?;
    let mut keys = BTreeSet::new();
    for (index, reference) in references.iter().enumerate() {
        let key = format!("{field}.{lane}[{index}]");
        if !reference.is_object() {
            return Err(format!("{key} must be an object"));
        }
        let kind = member(
            &reference["kind"],
            &format!("{key}.kind"),
            &["enemy", "golem", "loot", "player"],
        )?;
        if (lane == "loose") != (kind == "loot") {
            return Err(format!("{key} is in the wrong field target lane"));
        }
        let registered = kind == "enemy" || kind == "loot";
        object(
            reference,
            &key,
            if registered { &["kind", "id", "registrationOrdinal"] } else { &["kind", "id"] },
        )?;
        let id = if kind == "player" {
            player_id(&reference["id"], &format!("{key}.id"))?.to_owned()
        } else {
            integer(&reference["id"], &format!("{key}.id"), true)?.to_string()
        };
        let ordinal = if registered {
            Some(integer(
                &reference["registrationOrdinal"],
                &format!("{key}.registrationOrdinal"),
                false,
            )?)
        } else {
            None
        };
        keys.insert((kind, id, ordinal));
    }
    if keys.len() != references.len() {
        return Err(format!("{field}.{lane} contains duplicate target references"));
    }
    Ok(references)
}
