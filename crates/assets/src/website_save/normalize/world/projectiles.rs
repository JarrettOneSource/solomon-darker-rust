//! Legacy projectile steering, Arrow velocity and retired cosmetic effect migrations.

use serde_json::{Value, json};
use solomon_sim::{NativeRng, Vec2};

use super::super::super::contract::record;
use super::super::super::value::{decode, encode};
use super::super::{array, finite};

pub(super) fn normalize(
    value: &Value,
    version: u64,
    rng: &mut NativeRng,
) -> Result<Vec<Value>, String> {
    let mut result = Vec::new();
    for (index, value) in array(value, "game save Boneyard enemy projectiles")?.iter().enumerate() {
        let mut projectile =
            record(value, &format!("game save Boneyard enemy projectile {index}"))?.clone();
        let chill = if version < 19 && !projectile.contains_key("chillTumbleAccumulator") {
            0.0
        } else {
            finite(
                &value["chillTumbleAccumulator"],
                &format!("game save Boneyard enemy projectile {index} Chill accumulator"),
            )?
        };
        if !(0.0..=1.0).contains(&chill) {
            return Err(format!(
                "game save Boneyard enemy projectile {index} Chill accumulator is invalid"
            ));
        }
        let secondary = if version < 31 {
            0.0
        } else {
            finite(
                &value["secondaryDamage"],
                &format!("game save Boneyard enemy projectile {index} magic damage"),
            )?
        };
        if secondary < 0.0 {
            return Err(format!(
                "game save Boneyard enemy projectile {index} magic damage is invalid"
            ));
        }
        let guided = value["kind"].as_str() == Some("guided-missile");
        let turn = if version < 32 {
            if guided {
                solomon_sim::float::fround(0.5 + rng.draw_scaled_float(0.75, false))
            } else {
                0.0
            }
        } else {
            let turn = finite(
                &value["turnSpeed"],
                &format!("game save enemy projectile {index} turn speed"),
            )?;
            if if guided { !(0.5..=1.25).contains(&turn) } else { turn != 0.0 } {
                return Err(format!("game save enemy projectile {index} turn speed is invalid"));
            }
            turn
        };
        if value["kind"].as_str() == Some("arrow") {
            let velocity: Vec2 = if version < 32 {
                solomon_sim::enemies::ranged::native_enemy_projectile_velocity(
                    finite(&value["headingDeg"], "saved Arrow heading")?,
                    finite(&value["speed"], "saved Arrow speed")?,
                )
            } else {
                decode(value["velocity"].clone())?
            };
            projectile.insert("velocity".into(), encode(&velocity)?);
        }
        if version < 32 && guided {
            projectile.insert("lifetimeTicks".into(), json!(1300));
        }
        if version < 32 && value["kind"].as_str() == Some("poison-pool") {
            projectile.insert("lifetimeTicks".into(), json!(3200));
        }
        projectile.insert("chillTumbleAccumulator".into(), json!(chill));
        projectile.insert("secondaryDamage".into(), json!(secondary));
        projectile.insert("turnSpeed".into(), json!(turn));
        result.push(Value::Object(projectile));
    }
    Ok(result)
}

pub(super) fn effects(
    value: &Value,
    projectiles: &[Value],
    version: u64,
) -> Result<Vec<Value>, String> {
    let mut result = Vec::new();
    for value in array(value, "game save enemy projectile effects")? {
        record(value, "game save enemy projectile effect")?;
        let kind = value["kind"].as_str().unwrap_or("");
        if version < 32
            && matches!(
                kind,
                "demon-fire"
                    | "poison-pool-normal"
                    | "poison-pool-additive"
                    | "fire-burst-frame"
                    | "fire-burst-glow"
                    | "guided-impact-main"
                    | "guided-impact-aura-one"
                    | "guided-impact-aura-two"
            )
        {
            continue;
        }
        let mut effect = value.clone();
        if kind == "firebolt-trail" {
            let parent = projectiles.iter().find(|parent| {
                parent["id"] == value["ownerProjectileId"]
                    && parent["kind"].as_str() == Some("firebolt")
            });
            if version < 34 {
                let Some(parent) = parent else {
                    continue;
                };
                effect["painterRegistration"] = parent["painterRegistration"].clone();
            } else {
                let parent = parent.ok_or("saved Firebolt trail has no live parent")?;
                record(&effect["painterRegistration"], "saved Firebolt trail painter")?;
                record(&parent["painterRegistration"], "saved Firebolt painter")?;
                if effect["painterRegistration"]["managerLane"]
                    != parent["painterRegistration"]["managerLane"]
                    || effect["painterRegistration"]["registrationOrdinal"]
                        != parent["painterRegistration"]["registrationOrdinal"]
                {
                    return Err("saved Firebolt trail must inherit its parent painter".into());
                }
            }
        }
        result.push(effect);
    }
    Ok(result)
}
