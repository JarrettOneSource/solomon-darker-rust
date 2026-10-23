//! Saved Hub admission: validate retained rows before the source deliberately reconstructs
//! its ambient population, and restart the superseded pre-schema-15 College walk.

use serde_json::{Value, json};
use solomon_sim::hub::college_intro::NativeCollegeIntroState;
use solomon_sim::hub::npc::HUB_SKORCHA_PLACEMENTS;
use solomon_sim::hub::regions::HubRegion;

use super::super::contract::{MAX_SAFE_INTEGER, only_keys, record};
use super::super::value::{decode, encode};
use super::{array, finite, integer_within};

pub(crate) fn participant(
    world: &Value,
    owner: &str,
    version: u64,
) -> Result<(Value, bool), String> {
    let mut keys = vec![
        "ambient",
        "collisionRngState",
        "kind",
        "participants",
        "studentPopulation",
        "traderAnimationSeed",
    ];
    if version >= 8 {
        keys.push("skorcha");
    }
    only_keys(record(world, "game save Hub world")?, "game save Hub world", &keys)?;
    let participants = record(&world["participants"], "game save Hub participants")?;
    if participants.len() != 1 || !participants.contains_key(owner) {
        return Err("game save owner is not the sole Hub participant".into());
    }
    let value = &participants[owner];
    let keys: &[&str] = if version >= 15 {
        &["collegeIntro", "region", "transition"]
    } else {
        &["region", "transition"]
    };
    only_keys(record(value, "game save Hub participant")?, "game save Hub participant", keys)?;
    let region = region(&value["region"], "game save Hub participant region")?;
    if !value["transition"].is_null() {
        transition(&value["transition"], region)?;
    }
    if version >= 15 && !value["collegeIntro"].is_null() {
        college_intro(&value["collegeIntro"], region)?;
    }
    let population = record(&world["studentPopulation"], "game save Hub students")?;
    only_keys(
        population,
        "game save Hub students",
        &[
            "nextId",
            "rarePathDenominator",
            "rngState",
            "routeEndBehavior",
            "spawningEnabled",
            "spawnRequestPending",
            "spawnTickerCounter",
            "students",
        ],
    )?;
    array(&world["studentPopulation"]["students"], "game save Hub students")?;
    if let Some(value) = world.get("skorcha") {
        skorcha(value)?;
    }
    let legacy_intro =
        version < 15 && value["transition"]["phase"].as_str() == Some("college-intro");
    if legacy_intro {
        return Ok((
            json!({"collegeIntro":encode(&NativeCollegeIntroState::new())?, "region":"courtyard","transition":null}),
            true,
        ));
    }
    Ok((
        json!({"collegeIntro":if version >= 15 { value["collegeIntro"].clone() } else { Value::Null },
        "region":value["region"],"transition":value["transition"]}),
        false,
    ))
}

fn region(value: &Value, field: &str) -> Result<HubRegion, String> {
    value.as_str().and_then(HubRegion::from_id).ok_or_else(|| format!("{field} is invalid"))
}

fn transition(value: &Value, region: HubRegion) -> Result<(), String> {
    only_keys(
        record(value, "game save Hub transition")?,
        "game save Hub transition",
        &["alpha", "destination", "phase", "scriptedSpeed", "scriptedTarget", "sourceRegion"],
    )?;
    let phase = value["phase"].as_str().unwrap_or("");
    if !matches!(phase, "college-intro" | "college-loadout" | "outgoing" | "incoming") {
        return Err("game save Hub transition phase is invalid".into());
    }
    let destination = self::region(&value["destination"], "game save Hub destination")?;
    let source = self::region(&value["sourceRegion"], "game save Hub source")?;
    let alpha = finite(&value["alpha"], "game save Hub transition alpha")?;
    let speed = finite(&value["scriptedSpeed"], "game save Hub transition speed")?;
    let target = record(&value["scriptedTarget"], "game save Hub transition target")?;
    only_keys(target, "game save Hub transition target", &["x", "y"])?;
    let edge = if source == HubRegion::Courtyard {
        destination != HubRegion::Courtyard
    } else {
        destination == HubRegion::Courtyard
    };
    let outgoing = matches!(phase, "college-intro" | "outgoing");
    if !(0.0..=1.0).contains(&alpha)
        || speed <= 0.0
        || !edge
        || (outgoing && region != source)
        || (!outgoing && region != destination)
    {
        return Err("game save Hub transition is invalid".into());
    }
    finite(&target["x"], "game save Hub transition x")?;
    finite(&target["y"], "game save Hub transition y")?;
    Ok(())
}

fn college_intro(value: &Value, region: HubRegion) -> Result<(), String> {
    only_keys(
        record(value, "game save College intro")?,
        "game save College intro",
        &[
            "contactCounter",
            "coverAlpha",
            "dialogueSequence",
            "officeSpeed",
            "pathCursor",
            "phase",
            "titleCursor",
        ],
    )?;
    let phase = value["phase"].as_str().unwrap_or("");
    if !matches!(phase, "courtyard-walk" | "office-walk" | "arch-dialogue") {
        return Err("game save College intro phase is invalid".into());
    }
    if (phase == "courtyard-walk" && region != HubRegion::Courtyard)
        || (phase != "courtyard-walk" && region != HubRegion::Office)
    {
        return Err("game save College intro region is invalid".into());
    }
    let path = finite(&value["pathCursor"], "game save College path cursor")?;
    let limit = if phase == "courtyard-walk" { 9.0 } else { 6.0 };
    if !(0.0..=limit).contains(&path) {
        return Err("game save College path cursor is invalid".into());
    }
    let title = finite(&value["titleCursor"], "game save College title cursor")?;
    let cover = finite(&value["coverAlpha"], "game save College cover alpha")?;
    let speed = finite(&value["officeSpeed"], "game save College Office speed")?;
    let contact =
        integer_within(&value["contactCounter"], "game save College contact counter", 0, 10)?;
    if !(0.0..=5.0).contains(&title)
        || !(0.0..=1.0).contains(&cover)
        || !(0.5..=1.0).contains(&speed)
        || !contact.is_multiple_of(2)
    {
        return Err("game save College intro state is invalid".into());
    }
    integer_within(
        &value["dialogueSequence"],
        "game save College dialogue sequence",
        0,
        MAX_SAFE_INTEGER,
    )?;
    Ok(())
}

fn skorcha(value: &Value) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    only_keys(
        record(value, "game save Skorcha")?,
        "game save Skorcha",
        &[
            "dismissalIndex",
            "gesture",
            "gestureTicksRemaining",
            "hatActive",
            "hatPhaseDegrees",
            "hatRateDegreesPerTick",
            "position",
            "rng",
            "variant",
        ],
    )?;
    let variant = integer_within(&value["variant"], "game save Skorcha variant", 0, 2)? as usize;
    let point = record(&value["position"], "game save Skorcha position")?;
    only_keys(point, "game save Skorcha position", &["x", "y"])?;
    let x = finite(&point["x"], "game save Skorcha x")?;
    let y = finite(&point["y"], "game save Skorcha y")?;
    if HUB_SKORCHA_PLACEMENTS[variant] != solomon_sim::Vec2::new(x, y) {
        return Err("game save Skorcha placement drifted".into());
    }
    let active = value["hatActive"].as_bool().ok_or("game save Skorcha hat state is invalid")?;
    let phase = finite(&value["hatPhaseDegrees"], "game save Skorcha hat phase")?;
    let rate = finite(&value["hatRateDegreesPerTick"], "game save Skorcha hat rate")?;
    if !(0.0..180.0).contains(&phase)
        || !(0.0..=1.8).contains(&rate)
        || (!active && phase != 0.0)
        || (active && rate < 0.45)
    {
        return Err("game save Skorcha hat state is invalid".into());
    }
    integer_within(&value["dismissalIndex"], "game save Skorcha dismissal", 0, 2)?;
    integer_within(&value["gesture"], "game save Skorcha gesture", 0, 2)?;
    integer_within(&value["gestureTicksRemaining"], "game save Skorcha gesture timer", 0, 29)?;
    let _: solomon_sim::NativeRng = decode(value["rng"].clone())?;
    Ok(())
}
