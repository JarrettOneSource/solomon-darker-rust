//! The source enemy rows flatten family enums, shared projectile state and boss owners.
//! These translations change JSON shape only; the canonical actor/controller types remain
//! the sole state owners used by the next simulation tick.

use serde_json::{Value, json};

pub(super) fn prepare(name: &str, value: &mut Value) -> Result<(), String> {
    if has_unstepped_clock(name) && value["lastStepTick"].as_i64() == Some(-1) {
        value["lastStepTick"] = Value::Null;
    }
    match name {
        "EnemyActor" | "EnemyMaggotActor" => {
            if value.get("etherDrainCaptured").is_none() {
                value["etherDrainCaptured"] = json!(false);
            }
            value["registrationOrdinal"] =
                value["lightRegistration"]["registrationOrdinal"].clone();
        }
        "EnemyConfig" => {
            let family = match value["enemyToken"].as_str() {
                Some("SKELETON") => "skeleton",
                Some("SKELETONARCHER") => "archer",
                Some("SKELETONMAGE") => "mage",
                Some("IMP") => "imp",
                Some("ZOMBIE") => "zombie",
                Some("WRAITH") => "wraith",
                Some("DEMON") => "demon",
                Some("COFFIN") => "coffin",
                Some("PORTAL") => "portal",
                Some("HEARTMONGER") => "heartmonger",
                Some("DIREFACULTY") => "faculty",
                Some("DEMONSKULL") => "demon-skull",
                Some("SPIDER") => "spider",
                Some("COCOON") => "cocoon",
                _ => return Err("saved enemy configuration has an unknown family".into()),
            };
            if matches!(family, "wraith" | "cocoon") {
                value["family"] = json!(family);
            } else {
                if !value["family"].is_object() {
                    return Err("saved enemy family must be an object".into());
                }
                value["family"]["kind"] = json!(family);
            }
            value["minibossDieProgram"] =
                json!(value["onDeathProgram"].as_str() == Some("miniboss-die"));
        }
        "EnemyProjectileEffect" => {
            let kind = value["kind"].as_str().ok_or("saved projectile effect requires its kind")?;
            let payload = match kind {
                "demon-fire" => json!({"kind":kind,"fire":value["fire"]}),
                "poison-bubble" => {
                    json!({"kind":kind,"growthPerTick":value["growthPerTick"],"maximumScale":value["maximumScale"]})
                }
                _ => json!(kind),
            };
            *value = json!({"kind":payload,"state":value});
        }
        "NativeBossSpell" => {
            *value = json!({"owner":value,"kind":value});
        }
        "DemonSkullAction" => {
            *value = json!({"retired":value["retired"],"program":value});
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn finish(name: &str, value: &mut Value) {
    if has_unstepped_clock(name) && value["lastStepTick"].is_null() {
        value["lastStepTick"] = json!(-1);
    }
    match name {
        "EnemyActor" | "EnemyMaggotActor" => {
            let fields = value.as_object_mut().expect("serialized actor is an object");
            if fields.get("etherDrainCaptured") == Some(&Value::Bool(false)) {
                fields.remove("etherDrainCaptured");
            }
            let ordinal =
                fields.remove("registrationOrdinal").expect("actor retains its registration");
            fields.insert(
                "lightRegistration".into(),
                json!({"managerLane":"actor","registrationOrdinal":ordinal}),
            );
        }
        "EnemyConfig" => {
            let fields = value.as_object_mut().expect("serialized config is an object");
            if let Some(family) = fields.get_mut("family").and_then(Value::as_object_mut)
                && family.get("kind").and_then(Value::as_str) != Some("cocoon")
            {
                family.remove("kind");
            }
            let program =
                fields.remove("minibossDieProgram").expect("config owns its death policy");
            fields.insert(
                "onDeathProgram".into(),
                if program.as_bool() == Some(true) { json!("miniboss-die") } else { Value::Null },
            );
            // The stock constructor rejects dormant Skeleton policies. Every admitted
            // canonical configuration therefore retains this source-owned default.
            fields.insert("skeletonPolicy".into(), json!("default"));
        }
        "EnemyFamilyConfig" => {
            value.as_object_mut().expect("serialized family is an object").remove("kind");
        }
        "DemonSkullAction" => {
            flatten(value, "program");
        }
        "NativeBossSpell" => {
            flatten(value, "owner");
            flatten(value, "kind");
        }
        "EnemyProjectileEffect" => {
            let kind = value["kind"].clone();
            value.as_object_mut().expect("serialized effect is an object").remove("kind");
            flatten(value, "state");
            match kind {
                Value::String(tag) => value["kind"] = Value::String(tag),
                Value::Object(fields) => value.as_object_mut().unwrap().extend(fields),
                _ => unreachable!("serialized effect has its enum kind"),
            }
        }
        "EnemyDeathEffect" => {
            for field in ["painterSortBias", "scrapOscillation"] {
                if value[field].is_null() {
                    value.as_object_mut().unwrap().remove(field);
                }
            }
        }
        "EnemyProjectile" => {
            flatten(value, "state");
        }
        _ => {}
    }
}

pub(super) fn flatten(value: &mut Value, field: &str) {
    let fields = value.as_object_mut().expect("serialized owner is an object");
    let nested = fields.remove(field).expect("serialized owner retains its payload");
    fields.extend(nested.as_object().expect("serialized payload is an object").clone());
}

fn has_unstepped_clock(name: &str) -> bool {
    matches!(name, "EnemyWorld" | "EnemyDeathEffect" | "BoneyardLootStore" | "BoneyardLootActor")
}
