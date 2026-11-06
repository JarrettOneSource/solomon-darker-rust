//! Legacy enemy body/controller migrations. Native draws stay in saved actor order.

use serde_json::{Map, Value, json};
use solomon_sim::boneyard::waves::survival_programs;
use solomon_sim::enemies::config::{AuthoredEnemyFamily, EnemyLootPolicies, desaturate_color};
use solomon_sim::enemies::model::FlightPhase;
use solomon_sim::rng::next_xorshift_random;
use solomon_sim::{Rng, Vec2};

use super::super::super::contract::{MAX_SAFE_INTEGER, record};
use super::super::super::value::{decode, encode};
use super::super::{array, finite, integer_within, puppet_hit, signed_integer};

/// One saved enemy actor as its migrations read it: the saved record, its place in the saved
/// actor order (named in errors) and the save's schema version.
struct SavedEnemy<'a> {
    value: &'a Value,
    index: usize,
    version: u64,
}

pub(super) fn normalize(
    value: &Value,
    version: u64,
    tick: u64,
    last_step_tick: &Value,
    source_sha: &str,
    private_rng: &mut u32,
    rng: &mut Rng,
) -> Result<Vec<Value>, String> {
    let mut result = Vec::new();
    for (index, value) in array(value, "game save Boneyard enemy actors")?.iter().enumerate() {
        let enemy = SavedEnemy { value, index, version };
        let mut actor = record(value, &format!("game save Boneyard enemy actor {index}"))?.clone();
        let mut config =
            record(&value["config"], &format!("game save Boneyard enemy config {index}"))?.clone();
        check_ether_drain_capture(&actor, &config, &enemy)?;
        let lethal = if version < 34 {
            false
        } else {
            value["lethalMagicDamage"]
                .as_bool()
                .ok_or("saved enemy lethal magic provenance is invalid")?
        };
        let mut brain = if version < 26 && value["config"]["enemyToken"].as_str() == Some("WRAITH")
        {
            legacy_wraith_brain(&enemy, private_rng)?
        } else {
            value["brain"].clone()
        };
        record(&brain, &format!("game save Boneyard enemy brain {index}"))?;
        let family = brain["family"].as_str().unwrap_or("").to_owned();
        if family == "wraith" {
            migrate_wraith_brain(&mut brain, &enemy, last_step_tick)?;
        }
        if family == "demon" {
            brain["articulation"] = demon_articulation(&brain, value, index, version)?;
        }
        if matches!(family.as_str(), "portal" | "coffin") {
            migrate_stationary_anchor(&mut brain, &family, &enemy)?;
        }
        if family == "mage" {
            migrate_mage_casting_delay(&mut brain, &enemy)?;
        }
        if matches!(family.as_str(), "skeleton" | "archer" | "mage") {
            migrate_skeleton_recoil(&mut brain, version)?;
        }
        if family == "skeleton" {
            migrate_skeleton_pike(&mut brain, &enemy)?;
        }
        if family == "demon-skull" {
            super::discorporeal::validate(&brain)?;
        }
        if version < 34 && family == "archer" {
            brain["strafe"] = json!({"direction":rng.draw_sign(1.0),
                "limbHeadingDeg":finite(&value["headingDeg"], &format!("game save Archer {index} heading"))?,
                "movementRamp":0,"turnBlend":0});
        }
        migrate_config(&mut config, &enemy, source_sha)?;
        actor.insert("config".into(), Value::Object(config));
        actor.insert("brain".into(), brain);
        actor.insert("lethalMagicDamage".into(), json!(lethal));
        insert_hit_feedback_and_shadow(&mut actor, &enemy, tick)?;
        result.push(Value::Object(actor));
    }
    Ok(result)
}

/// Only a dying enemy whose token Ether Drain can capture may carry a capture, and it must be
/// `true`.
fn check_ether_drain_capture(
    actor: &Map<String, Value>,
    config: &Map<String, Value>,
    enemy: &SavedEnemy,
) -> Result<(), String> {
    if let Some(captured) = actor.get("etherDrainCaptured")
        && (captured != true
            || enemy.value["lifeState"] != "dying"
            || !matches!(
                config["enemyToken"].as_str(),
                Some(
                    "SKELETON"
                        | "SKELETONARCHER"
                        | "SKELETONMAGE"
                        | "ZOMBIE"
                        | "DEMON"
                        | "SPIDER"
                        | "IMP"
                        | "WRAITH"
                        | "MAGGOT"
                )
            ))
    {
        return Err(format!(
            "game save Boneyard enemy actor {} has an invalid Ether Drain capture",
            enemy.index
        ));
    }
    Ok(())
}

/// A Wraith saved before schema 26 gets a new flight brain from four draws of the enemies'
/// private stream.
fn legacy_wraith_brain(enemy: &SavedEnemy, private_rng: &mut u32) -> Result<Value, String> {
    next_xorshift_random(private_rng); // Historical visual-phase draw is consumed.
    let resting = next_xorshift_random(private_rng);
    let flyby = (next_xorshift_random(private_rng) * 601.0).floor() as u64;
    let initial = next_xorshift_random(private_rng);
    let speed = finite(
        &enemy.value["config"]["chaseSpeed"],
        &format!("game save Wraith {} chase speed", enemy.index),
    )?;
    if speed <= 0.0 {
        return Err("Wraith chase speed must be positive".into());
    }
    let mut flight =
        solomon_sim::enemies::wraith::create_wraith_flight_state(speed, resting, initial, flyby);
    if enemy.value["lifeState"].as_str() != Some("alive") {
        flight.phase = FlightPhase::Death;
    }
    let mut brain = encode(&flight)?;
    brain["family"] = json!("wraith");
    Ok(brain)
}

/// A Wraith's collision-recipient flag, and for schemas 26 to 43 its repaired cruise speed and
/// turn gains.
fn migrate_wraith_brain(
    brain: &mut Value,
    enemy: &SavedEnemy,
    last_step_tick: &Value,
) -> Result<(), String> {
    if brain.get("collisionRecipient").is_none() {
        let spawn = integer_within(
            &enemy.value["spawnTick"],
            "saved Wraith spawn tick",
            0,
            MAX_SAFE_INTEGER,
        )?;
        let last =
            signed_integer(last_step_tick, "saved enemy tick", -1.0, MAX_SAFE_INTEGER as f64)?;
        brain["collisionRecipient"] = json!(spawn as f64 >= last);
    } else if !brain["collisionRecipient"].is_boolean() {
        return Err("saved Wraith collision recipient is invalid".into());
    }
    if (26..44).contains(&enemy.version) {
        let base = finite(&brain["baseFlybySpeed"], "legacy Wraith base speed")?;
        if base <= 0.0 {
            return Err("legacy Wraith base speed must be positive".into());
        }
        let old_cruise = finite(&brain["restingSpeed"], "legacy Wraith cruise speed")?;
        if old_cruise < 0.0 {
            return Err("legacy Wraith cruise speed must be non-negative".into());
        }
        let resting =
            solomon_sim::float::fround(base * solomon_sim::float::fround(20.0 + old_cruise / base));
        if !resting.is_finite() {
            return Err("repaired Wraith cruise speed is invalid".into());
        }
        brain["restingSpeed"] = json!(resting);
        brain["currentSpeed"] =
            json!(resting.max(finite(&brain["currentSpeed"], "legacy Wraith current speed")?));
        brain["currentTurnGain"] = json!(1.5);
        brain["targetTurnGain"] = json!(3);
    }
    Ok(())
}

/// A Portal or Coffin's anchor position.
fn migrate_stationary_anchor(
    brain: &mut Value,
    family: &str,
    enemy: &SavedEnemy,
) -> Result<(), String> {
    // Native Portal +0x218/+0x21C starts after its ten-tick placement body.
    let materializing =
        family == "portal" && finite(&brain["ageTicks"], "saved Portal age")? < 10.0;
    let anchor = if enemy.version < 38 {
        if materializing { &Value::Null } else { &enemy.value["position"] }
    } else {
        &brain["anchorPosition"]
    };
    brain["anchorPosition"] = if materializing {
        if !anchor.is_null() {
            return Err("saved Portal anchor must be null before materialization".into());
        }
        Value::Null
    } else {
        record(anchor, "saved stationary enemy anchor")?;
        json!({"x":finite(&anchor["x"], "saved stationary enemy anchor X")?,
            "y":finite(&anchor["y"], "saved stationary enemy anchor Y")?})
    };
    Ok(())
}

/// A Mage's primary casting delay: zero before schema 34, otherwise the saved delay of at most
/// 600 ticks.
fn migrate_mage_casting_delay(brain: &mut Value, enemy: &SavedEnemy) -> Result<(), String> {
    brain["disabledPrimaryTicks"] = if enemy.version < 34 {
        json!(0)
    } else {
        json!(integer_within(
            &brain["disabledPrimaryTicks"],
            &format!("game save Mage {} casting delay", enemy.index),
            0,
            600
        )?)
    };
    Ok(())
}

/// The recoil offset and velocity shared by Skeletons, Archers and Mages.
fn migrate_skeleton_recoil(brain: &mut Value, version: u64) -> Result<(), String> {
    let (offset, velocity) = if version < 35 {
        (0.0, 0.0)
    } else {
        (
            finite(&brain["verticalOffset"], "saved Skeleton recoil offset")?,
            finite(&brain["verticalVelocity"], "saved Skeleton recoil velocity")?,
        )
    };
    if offset > 0.0 {
        return Err("saved Skeleton recoil offset is invalid".into());
    }
    brain["verticalOffset"] = json!(offset);
    brain["verticalVelocity"] = json!(velocity);
    Ok(())
}

/// A Skeleton's Pike target: none before schema 35, otherwise held only by a living Pike
/// Skeleton during its Pike attack.
fn migrate_skeleton_pike(brain: &mut Value, enemy: &SavedEnemy) -> Result<(), String> {
    if enemy.version < 35 {
        brain["pike"] = Value::Null;
    } else if !brain["pike"].is_null() {
        record(&brain["pike"], "saved Skeleton Pike")?;
        let pike = &brain["pike"];
        let distance = finite(&pike["distance"], "saved Skeleton Pike distance")?;
        if pike["playerId"].as_str().is_none_or(str::is_empty)
            || distance <= 0.0
            || enemy.value["lifeState"].as_str() != Some("alive")
            || enemy.value["config"]["family"]["weapon"].as_str() != Some("pike")
            || brain["action"].as_str() != Some("pike")
            || brain["phase"].as_str() != Some("attack")
        {
            return Err("saved Skeleton Pike target is invalid".into());
        }
        let _: Vec2 = decode(pike["position"].clone())?;
    }
    Ok(())
}

/// Legacy Dire Faculty colors and Archer strafing, then defaults for config fields that older
/// saves leave out.
fn migrate_config(
    config: &mut Map<String, Value>,
    enemy: &SavedEnemy,
    source_sha: &str,
) -> Result<(), String> {
    if enemy.version < 35 && enemy.value["config"]["enemyToken"].as_str() == Some("DIREFACULTY") {
        config.insert("family".into(), faculty_color(&enemy.value["config"], source_sha)?);
    }
    if enemy.version < 34 && enemy.value["config"]["enemyToken"].as_str() == Some("SKELETONARCHER")
    {
        let mut family =
            record(&config["family"], &format!("game save Archer {} config", enemy.index))?.clone();
        family.insert("strafing".into(), json!(false));
        config.insert("family".into(), Value::Object(family));
    }
    for (key, default) in [
        ("classification", json!("normal")),
        ("lootPolicies", encode(&EnemyLootPolicies::default())?),
        ("onDeathProgram", Value::Null),
        ("recipeName", Value::Null),
        ("recipeUid", Value::Null),
    ] {
        if config.get(key).is_none_or(Value::is_null) {
            config.insert(key.into(), default);
        }
    }
    Ok(())
}

/// The hit reaction, hit feedback and shadow gait offset, with defaults for older schemas.
fn insert_hit_feedback_and_shadow(
    actor: &mut Map<String, Value>,
    enemy: &SavedEnemy,
    tick: u64,
) -> Result<(), String> {
    let reaction = if enemy.version < 41 {
        0.0
    } else {
        finite(
            &enemy.value["hitReactionTimer"],
            &format!("game save enemy {} hit reaction", enemy.index),
        )?
    };
    if !(0.0..=1.0).contains(&reaction) {
        return Err(format!("game save enemy {} hit reaction must be within [0,1]", enemy.index));
    }
    actor.insert("hitReactionTimer".into(), json!(reaction));
    actor.insert(
        "hitFeedback".into(),
        puppet_hit(
            &enemy.value["hitFeedback"],
            &enemy.value["lastDamageTick"],
            tick,
            enemy.version,
            &format!("game save enemy {} hit feedback", enemy.index),
        )?,
    );
    actor.insert(
        "shadowLateralOffset".into(),
        json!(if enemy.version < 34 {
            0.0
        } else {
            finite(&enemy.value["shadowLateralOffset"], "saved enemy shadow gait offset")?
        }),
    );
    Ok(())
}

fn demon_articulation(
    brain: &Value,
    actor: &Value,
    index: usize,
    version: u64,
) -> Result<Value, String> {
    if let Some(value) = brain.get("articulation") {
        let source = record(value, "Demon articulation state")?;
        if super::super::integer_within(
            &value["completedSteps"],
            "Demon completed steps",
            0,
            MAX_SAFE_INTEGER,
        )
        .is_err()
            || integer_within(&value["stepIntervalTicks"], "Demon step interval", 76, 150).is_err()
            || source.get("frontBaseRotationDeg").and_then(Value::as_f64).is_none()
            || source.get("rearBaseRotationDeg").and_then(Value::as_f64).is_none()
        {
            return Err("Demon articulation state is invalid".into());
        }
        for extremity in ["front", "rear"] {
            let value = &value[extremity];
            for point in ["current", "start", "target"] {
                let _: Vec2 = decode(value[point].clone())?;
            }
            if value["phase"].as_f64().is_none_or(|phase| !(0.0..=1.0).contains(&phase)) {
                return Err("Demon extremity phase must be within [0,1]".into());
            }
            if value["liftY"].as_f64().is_none_or(|lift| lift > 0.0) {
                return Err("Demon extremity lift must be finite and non-positive".into());
            }
        }
        return Ok(value.clone());
    }
    if version >= 27 {
        return Err(format!("game save Boneyard Demon {index} articulation is missing"));
    }
    let id = integer_within(
        &actor["id"],
        &format!("game save Boneyard Demon {index} id"),
        1,
        MAX_SAFE_INTEGER,
    )?;
    let tick = integer_within(
        &actor["spawnTick"],
        &format!("game save Boneyard Demon {index} spawn tick"),
        0,
        MAX_SAFE_INTEGER,
    )?;
    let position: Vec2 = decode(actor["position"].clone())?;
    let heading =
        finite(&actor["headingDeg"], &format!("game save Boneyard Demon {index} heading"))?;
    let scale =
        finite(&actor["config"]["scale"], &format!("game save Boneyard Demon {index} scale"))?;
    if scale <= 0.0 {
        return Err("Demon actor scale must be finite and positive".into());
    }
    encode(&solomon_sim::enemies::demon::create_demon_articulation(
        id as usize,
        tick,
        position,
        heading,
        scale,
    ))
}

fn faculty_color(config: &Value, source_sha: &str) -> Result<Value, String> {
    let mut family = record(&config["family"], "saved Faculty appearance")?.clone();
    let name = config["recipeName"].as_str().unwrap_or("");
    if matches!(name, "Dire Sirmin" | "Dire Lucritius" | "Dire Aliss")
        && let Some(programs) = survival_programs(source_sha)
        && let Some(recipe) = programs.bosses.iter().flat_map(|boss| &boss.recipes).find(|recipe| {
            recipe.name == name && config["recipeUid"].as_u64() == Some(recipe.uid as u64)
        })
        && let AuthoredEnemyFamily::Faculty(appearance) = &recipe.family
    {
        for (key, color) in
            [("bodyColor", appearance.body_color), ("headColor", appearance.head_color)]
        {
            if config["family"][key] == encode(&desaturate_color(color, 1.0 - 0.7_f32 as f64))? {
                family.insert(key.into(), encode(&desaturate_color(color, 0.7_f32 as f64))?);
            }
        }
        return Ok(Value::Object(family));
    }
    let orange = [1.0, 0.5, 0.0, 1.0];
    if config["family"]["bodyColor"] == encode(&desaturate_color(orange, 1.0 - 0.6_f32 as f64))? {
        family.insert("bodyColor".into(), encode(&desaturate_color(orange, 0.6_f32 as f64))?);
    }
    Ok(Value::Object(family))
}
