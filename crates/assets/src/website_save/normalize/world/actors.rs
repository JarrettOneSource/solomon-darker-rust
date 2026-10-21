//! Legacy enemy body/controller migrations. Native draws stay in saved actor order.

use serde_json::{Value, json};
use solomon_sim::boneyard::waves::native_survival_programs;
use solomon_sim::enemies::config::{AuthoredEnemyFamily, EnemyLootPolicies, desaturate_color};
use solomon_sim::enemies::model::FlightPhase;
use solomon_sim::rng::next_xorshift_random;
use solomon_sim::{NativeRng, Vec2};

use super::super::super::contract::{MAX_SAFE_INTEGER, record};
use super::super::super::value::{decode, encode};
use super::super::{array, finite, integer_within, puppet_hit, signed_integer};

pub(super) fn normalize(
    value: &Value,
    version: u64,
    tick: u64,
    last_step_tick: &Value,
    source_sha: &str,
    private_rng: &mut u32,
    native_rng: &mut NativeRng,
) -> Result<Vec<Value>, String> {
    let mut result = Vec::new();
    for (index, value) in array(value, "game save Boneyard enemy actors")?.iter().enumerate() {
        let mut actor = record(value, &format!("game save Boneyard enemy actor {index}"))?.clone();
        let mut config =
            record(&value["config"], &format!("game save Boneyard enemy config {index}"))?.clone();
        if let Some(captured) = actor.get("etherDrainCaptured")
            && (captured != true
                || value["lifeState"] != "dying"
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
                "game save Boneyard enemy actor {index} has an invalid Ether Drain capture"
            ));
        }
        let lethal = if version < 34 {
            false
        } else {
            value["lethalMagicDamage"]
                .as_bool()
                .ok_or("saved enemy lethal magic provenance is invalid")?
        };
        let mut brain = if version < 26 && value["config"]["enemyToken"].as_str() == Some("WRAITH")
        {
            next_xorshift_random(private_rng); // Historical visual-phase draw is consumed.
            let resting = next_xorshift_random(private_rng);
            let flyby = (next_xorshift_random(private_rng) * 601.0).floor() as u64;
            let initial = next_xorshift_random(private_rng);
            let speed = finite(
                &value["config"]["chaseSpeed"],
                &format!("game save Wraith {index} chase speed"),
            )?;
            if speed <= 0.0 {
                return Err("Wraith chase speed must be positive".into());
            }
            let mut flight = solomon_sim::enemies::wraith::create_native_wraith_flight_state(
                speed, resting, initial, flyby,
            );
            if value["lifeState"].as_str() != Some("alive") {
                flight.phase = FlightPhase::Death;
            }
            let mut brain = encode(&flight)?;
            brain["family"] = json!("wraith");
            brain
        } else {
            value["brain"].clone()
        };
        record(&brain, &format!("game save Boneyard enemy brain {index}"))?;
        let family = brain["family"].as_str().unwrap_or("").to_owned();
        if family == "wraith" {
            if brain.get("collisionRecipient").is_none() {
                let spawn = integer_within(
                    &value["spawnTick"],
                    "saved Wraith spawn tick",
                    0,
                    MAX_SAFE_INTEGER,
                )?;
                let last = signed_integer(
                    last_step_tick,
                    "saved enemy tick",
                    -1.0,
                    MAX_SAFE_INTEGER as f64,
                )?;
                brain["collisionRecipient"] = json!(spawn as f64 >= last);
            } else if !brain["collisionRecipient"].is_boolean() {
                return Err("saved Wraith collision recipient is invalid".into());
            }
            if (26..44).contains(&version) {
                let base = finite(&brain["baseFlybySpeed"], "legacy Wraith base speed")?;
                if base <= 0.0 {
                    return Err("legacy Wraith base speed must be positive".into());
                }
                let old_cruise = finite(&brain["restingSpeed"], "legacy Wraith cruise speed")?;
                if old_cruise < 0.0 {
                    return Err("legacy Wraith cruise speed must be non-negative".into());
                }
                let resting = solomon_sim::float::fround(
                    base * solomon_sim::float::fround(20.0 + old_cruise / base),
                );
                if !resting.is_finite() {
                    return Err("repaired Wraith cruise speed is invalid".into());
                }
                brain["restingSpeed"] = json!(resting);
                brain["currentSpeed"] = json!(
                    resting.max(finite(&brain["currentSpeed"], "legacy Wraith current speed")?)
                );
                brain["currentTurnGain"] = json!(1.5);
                brain["targetTurnGain"] = json!(3);
            }
        }
        if family == "demon" {
            brain["articulation"] = demon_articulation(&brain, value, index, version)?;
        }
        if matches!(family.as_str(), "portal" | "coffin") {
            // Native Portal +0x218/+0x21C starts after its ten-tick placement body.
            let materializing =
                family == "portal" && finite(&brain["ageTicks"], "saved Portal age")? < 10.0;
            let anchor = if version < 38 {
                if materializing { &Value::Null } else { &value["position"] }
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
        }
        if family == "mage" {
            brain["disabledPrimaryTicks"] = if version < 34 {
                json!(0)
            } else {
                json!(integer_within(
                    &brain["disabledPrimaryTicks"],
                    &format!("game save Mage {index} casting delay"),
                    0,
                    600
                )?)
            };
        }
        if matches!(family.as_str(), "skeleton" | "archer" | "mage") {
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
        }
        if family == "skeleton" {
            if version < 35 {
                brain["pike"] = Value::Null;
            } else if !brain["pike"].is_null() {
                record(&brain["pike"], "saved Skeleton Pike")?;
                let pike = &brain["pike"];
                let distance = finite(&pike["distance"], "saved Skeleton Pike distance")?;
                if pike["playerId"].as_str().is_none_or(str::is_empty)
                    || distance <= 0.0
                    || value["lifeState"].as_str() != Some("alive")
                    || value["config"]["family"]["weapon"].as_str() != Some("pike")
                    || brain["action"].as_str() != Some("pike")
                    || brain["phase"].as_str() != Some("attack")
                {
                    return Err("saved Skeleton Pike target is invalid".into());
                }
                let _: Vec2 = decode(pike["position"].clone())?;
            }
        }
        if family == "demon-skull" {
            super::discorporeal::validate(&brain)?;
        }
        if version < 34 && family == "archer" {
            brain["strafe"] = json!({"direction":native_rng.draw_sign(1.0),
                "limbHeadingDeg":finite(&value["headingDeg"], &format!("game save Archer {index} heading"))?,
                "movementRamp":0,"turnBlend":0});
        }
        if version < 35 && value["config"]["enemyToken"].as_str() == Some("DIREFACULTY") {
            config.insert("family".into(), faculty_color(&value["config"], source_sha)?);
        }
        if version < 34 && value["config"]["enemyToken"].as_str() == Some("SKELETONARCHER") {
            let mut family =
                record(&config["family"], &format!("game save Archer {index} config"))?.clone();
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
        actor.insert("config".into(), Value::Object(config));
        actor.insert("brain".into(), brain);
        actor.insert("lethalMagicDamage".into(), json!(lethal));
        let reaction = if version < 41 {
            0.0
        } else {
            finite(&value["hitReactionTimer"], &format!("game save enemy {index} hit reaction"))?
        };
        if !(0.0..=1.0).contains(&reaction) {
            return Err(format!("game save enemy {index} hit reaction must be within [0,1]"));
        }
        actor.insert("hitReactionTimer".into(), json!(reaction));
        actor.insert(
            "hitFeedback".into(),
            puppet_hit(
                &value["hitFeedback"],
                &value["lastDamageTick"],
                tick,
                version,
                &format!("game save enemy {index} hit feedback"),
            )?,
        );
        actor.insert(
            "shadowLateralOffset".into(),
            json!(if version < 34 {
                0.0
            } else {
                finite(&value["shadowLateralOffset"], "saved enemy shadow gait offset")?
            }),
        );
        result.push(Value::Object(actor));
    }
    Ok(result)
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
        && let Some(programs) = native_survival_programs(source_sha)
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
