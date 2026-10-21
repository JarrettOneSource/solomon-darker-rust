//! `save/discorporeal-save.ts`: retain the complete boss action program, validate its
//! source signed counters, and reject unknown action/visual fields before typed decoding.

use serde_json::Value;

use super::super::super::contract::{MAX_SAFE_INTEGER, only_keys, record};
use super::super::super::value::decode;
use super::super::{array, finite, integer_within, signed_integer};

pub(super) fn validate(value: &Value) -> Result<(), String> {
    let field = "saved Discorporeal";
    let source = record(value, field)?;
    only_keys(
        source,
        field,
        &[
            "bodyHeadingDeg",
            "bodyOffset",
            "bodyPhaseDeg",
            "bodyPose",
            "chargeGlow",
            "eyeCharge",
            "flairGlow",
            "flickerPhaseDeg",
            "jitter",
            "lightIntensity",
            "spin",
            "actions",
            "attackTicksRemaining",
            "capabilities",
            "deathCountdown",
            "family",
            "headingDelayTicks",
            "pendingAttack",
            "phase",
            "recoil",
            "screamActive",
            "seen",
            "speed",
            "targetSpeed",
        ],
    )?;
    if value["family"].as_str() != Some("demon-skull") {
        return Err(format!("{field} family is invalid"));
    }
    if !matches!(value["phase"].as_str(), Some("range-control" | "cast" | "death")) {
        return Err(format!("{field} phase is invalid"));
    }
    if !value["pendingAttack"].is_null()
        && !matches!(value["pendingAttack"].as_str(), Some("eyes" | "mouth" | "spit"))
    {
        return Err(format!("{field} pending attack is invalid"));
    }
    for key in ["bodyHeadingDeg", "bodyPhaseDeg", "flickerPhaseDeg", "spin", "speed", "targetSpeed"]
    {
        finite(&value[key], &format!("{field}.{key}"))?;
    }
    for key in ["bodyOffset", "jitter", "recoil"] {
        let _: solomon_sim::Vec2 = decode(value[key].clone())?;
    }
    for key in ["chargeGlow", "eyeCharge", "flairGlow", "lightIntensity"] {
        let number = finite(&value[key], &format!("{field}.{key}"))?;
        if !(0.0..=1.0).contains(&number) {
            return Err(format!("{field}.{key} is invalid"));
        }
    }
    integer_within(&value["bodyPose"], &format!("{field}.bodyPose"), 0, 2)?;
    integer_within(&value["capabilities"], &format!("{field} capabilities"), 0, 63)?;
    for key in ["screamActive", "seen"] {
        boolean(&value[key], &format!("{field} {key}"))?;
    }
    for key in ["attackTicksRemaining", "deathCountdown", "headingDelayTicks"] {
        counter(&value[key], &format!("{field} {key}"))?;
    }
    for (index, action) in array(&value["actions"], &format!("{field} actions"))?.iter().enumerate()
    {
        saved_action(action, &format!("{field} action {index}"))?;
    }
    Ok(())
}

fn saved_action(value: &Value, field: &str) -> Result<(), String> {
    let source = record(value, field)?;
    boolean(&value["retired"], &format!("{field} retired"))?;
    let (numbers, counters, booleans): (&[&str], &[&str], &[&str]) = match value["kind"].as_str() {
        Some("scream") => (&[], &[], &[]),
        Some("bite") => (&["progress", "rate"], &[], &["markerEmitted"]),
        Some("eyes") => {
            (&["attackSpeed"], &["warmupTicks", "shotsRemaining", "recoveryTicks"], &[])
        }
        Some("mouth") => (
            &["warmupJitter", "warmupTurnSpeed", "beamPower", "trackingRamp"],
            &["warmupTicks", "trackingDelayTicks", "recoveryTicks"],
            &[],
        ),
        Some("spit") => {
            (&["targetHeadingDeg"], &["shotsRemaining", "cooldownTicks", "mouthTicks"], &[])
        }
        Some("flair") => {
            (&["glow", "originalHeadingDeg"], &["remainingTicks", "settlingTicks"], &[])
        }
        _ => return Err(format!("{field} kind is invalid")),
    };
    let mut keys = vec!["kind", "retired"];
    keys.extend_from_slice(numbers);
    keys.extend_from_slice(counters);
    keys.extend_from_slice(booleans);
    only_keys(source, field, &keys)?;
    for key in numbers {
        finite(&value[*key], &format!("{field} {key}"))?;
    }
    for key in counters {
        counter(&value[*key], &format!("{field} {key}"))?;
    }
    for key in booleans {
        boolean(&value[*key], &format!("{field} {key}"))?;
    }
    Ok(())
}

fn counter(value: &Value, field: &str) -> Result<(), String> {
    signed_integer(value, field, -(MAX_SAFE_INTEGER as f64), MAX_SAFE_INTEGER as f64)?;
    Ok(())
}

fn boolean(value: &Value, field: &str) -> Result<(), String> {
    value.as_bool().ok_or_else(|| format!("{field} is invalid"))?;
    Ok(())
}
