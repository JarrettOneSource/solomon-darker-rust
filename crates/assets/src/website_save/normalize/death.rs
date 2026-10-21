//! Persisted mutable death owners from Source `protocol/codecs/death-animations.ts`.

use std::collections::BTreeSet;

use serde_json::Value;

use super::protocol_values::{
    bounded_integer, finite, integer, limited_array, member, object, player_id, positive,
    registration, vector,
};

pub(super) const MAX_DEATH_EFFECTS: usize = 8_192 + 3 * (125 * 140 + 488);

fn motion(value: &Value, field: &str) -> Result<(), String> {
    object(
        value,
        field,
        &[
            "bounceVelocity",
            "height",
            "position",
            "rotationDegrees",
            "rotationStepDegrees",
            "velocity",
            "verticalVelocity",
        ],
    )?;
    let bounce = finite(&value["bounceVelocity"], &format!("{field}.bounceVelocity"))?;
    let height = finite(&value["height"], &format!("{field}.height"))?;
    let vertical = finite(&value["verticalVelocity"], &format!("{field}.verticalVelocity"))?;
    let rotation_step =
        finite(&value["rotationStepDegrees"], &format!("{field}.rotationStepDegrees"))?;
    vector(&value["velocity"], &format!("{field}.velocity"))?;
    let x = value["velocity"]["x"].as_f64().expect("validated velocity");
    let y = value["velocity"]["y"].as_f64().expect("validated velocity");
    if !(-5.0..=0.0).contains(&bounce)
        || !(-64.0..=0.0).contains(&height)
        || !(-5.0..=7.0).contains(&vertical)
        || rotation_step.abs() > 20.0
        || x.abs() > 2.25
        || y.abs() > 2.25
    {
        return Err(format!("{field} exceeds its native death Bouncer domains"));
    }
    if bounce == 0.0
        && (height != 0.0 || vertical != 0.0 || x != 0.0 || y != 0.0 || rotation_step != 0.0)
    {
        return Err(format!("{field} has motion after native settling"));
    }
    vector(&value["position"], &format!("{field}.position"))?;
    finite(&value["rotationDegrees"], &format!("{field}.rotationDegrees"))?;
    Ok(())
}

pub(super) fn golem(value: &Value, field: &str, enhanced: bool) -> Result<(), String> {
    object(value, field, &["fragments", "starRotationDegrees", "starStepDegrees"])?;
    let fragments = limited_array(&value["fragments"], &format!("{field}.fragments"), 30)?;
    for (index, fragment) in fragments.iter().enumerate() {
        if fragment.is_null() {
            continue;
        }
        let key = format!("{field}.fragments[{index}]");
        let mut fields = object(
            fragment,
            &key,
            &[
                "bounceVelocity",
                "height",
                "life",
                "position",
                "rotationDegrees",
                "rotationStepDegrees",
                "velocity",
                "verticalVelocity",
            ],
        )?
        .clone();
        let life = positive(&fields.remove("life").unwrap_or(Value::Null), &format!("{key}.life"))?;
        if life > if enhanced { 10.0 } else { 2.0 } {
            return Err(format!("{key}.life exceeds its born quality"));
        }
        motion(&Value::Object(fields), &key)?;
    }
    if fragments.len() != 30 {
        return Err(format!("{field}.fragments must retain30 indexed slots"));
    }
    let rotation = finite(&value["starRotationDegrees"], &format!("{field}.starRotationDegrees"))?;
    let step = finite(&value["starStepDegrees"], &format!("{field}.starStepDegrees"))?;
    if !(0.0..=360.0).contains(&rotation) || !(0.0..=7.0).contains(&step) {
        return Err(format!("{field} exceeds its native Unbind constructor"));
    }
    Ok(())
}

pub(super) fn weapons(value: &Value, field: &str, tick: u64) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    let mut epochs = BTreeSet::new();
    let mut registrations = BTreeSet::new();
    let actors = limited_array(value, field, MAX_DEATH_EFFECTS)?;
    for (index, actor) in actors.iter().enumerate() {
        let key = format!("{field}[{index}]");
        object(
            actor,
            &key,
            &[
                "ageTicks",
                "birthTick",
                "deathEpoch",
                "id",
                "life",
                "motion",
                "ownerId",
                "painterRegistration",
                "weapon",
            ],
        )?;
        let age = integer(&actor["ageTicks"], &format!("{key}.ageTicks"), false)?;
        let birth = integer(&actor["birthTick"], &format!("{key}.birthTick"), false)?;
        let life = positive(&actor["life"], &format!("{key}.life"))?;
        if birth > tick || age > tick - birth || life > 99999.0 {
            return Err(format!("{key} has invalid death-weapon clocks"));
        }
        object(&actor["weapon"], &format!("{key}.weapon"), &["kind", "selector"])?;
        let epoch = integer(&actor["deathEpoch"], &format!("{key}.deathEpoch"), true)?;
        ids.insert(integer(&actor["id"], &format!("{key}.id"), true)?);
        motion(&actor["motion"], &format!("{key}.motion"))?;
        let owner = player_id(&actor["ownerId"], &format!("{key}.ownerId"))?;
        epochs.insert(format!("{owner}:{epoch}"));
        registrations.insert(registration(
            &actor["painterRegistration"],
            &format!("{key}.painterRegistration"),
            "actor",
        )?);
        member(&actor["weapon"]["kind"], &format!("{key}.weapon.kind"), &["staff", "wand"])?;
        bounded_integer(&actor["weapon"]["selector"], &format!("{key}.weapon.selector"), 0, 5)?;
    }
    if ids.len() != actors.len()
        || epochs.len() != actors.len()
        || registrations.len() != actors.len()
    {
        return Err(format!("{field} duplicates a death-weapon id, epoch or registration"));
    }
    Ok(())
}
