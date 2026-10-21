//! Versioned primary and secondary actor migrations; both retain their canonical RNG.

use serde_json::{Value, json};
use solomon_sim::{NativeRng, Vec2};

use super::super::contract::{MAX_SAFE_INTEGER, record};
use super::super::value::{decode, encode};
use super::{array, finite, integer_within};

pub(super) fn primary(value: &Value, version: u64) -> Result<Value, String> {
    let mut source = record(value, "game save primary spells")?.clone();
    let mut transients = Vec::new();
    for (index, value) in
        array(&value["transients"], "game save primary spell transients")?.iter().enumerate()
    {
        let mut actor =
            record(value, &format!("game save primary spell transient {index}"))?.clone();
        let kind = value["kind"].as_str();
        if (version < 29 && kind == Some("water-hail"))
            || (version < 40 && kind == Some("water-aura"))
        {
            continue;
        }
        if version <= 50
            && value["buildId"] == 1003
            && matches!(kind, Some("weld-channel" | "weld-flame-lash-fade"))
            && !actor.contains_key("flameVisual")
        {
            actor.insert("flameVisual".into(), json!({"kind":"legacy-suppressed"}));
        }
        if version < 46 {
            match kind {
                Some("harden-shard" | "player-staff-pike-break") => {
                    actor.insert("enhancedShadow".into(), json!(true));
                }
                Some("air") => {
                    actor.insert("enhancedEffects".into(), json!(true));
                    actor.insert("chained".into(), json!(false));
                }
                Some("weld-channel" | "fire" | "earth-impact") => {
                    actor.insert("enhancedEffects".into(), json!(true));
                }
                _ => {}
            }
        }
        if version < 32 && kind == Some("fire-patch") {
            actor.insert("horizontalSign".into(), json!(1));
            actor.remove("shapeSample");
        }
        if version < 34 && kind == Some("weld-meteor") {
            let position: Vec2 = decode(value["position"].clone())?;
            let height = solomon_sim::float::fround(
                finite(&value["fallHeight"], "saved Meteor height")?
                    + finite(&value["fallStep"], "saved Meteor fall step")?,
            );
            let heading = finite(&value["fallHeadingDegrees"], "saved Meteor heading")?;
            let root = if value["ageTicks"].as_f64() == Some(0.0) {
                Vec2::ZERO
            } else {
                solomon_sim::spells::weld::meteor::native_weld_meteor_root_position(
                    position, heading, height,
                )
            };
            actor.insert("landingPosition".into(), encode(&position)?);
            actor.insert("position".into(), encode(&root)?);
        }
        if kind == Some("water") {
            let speed = if version < 19 && !actor.contains_key("speed") {
                4.0
            } else {
                finite(&value["speed"], &format!("game save Water transient {index} speed"))?
            };
            if !(4.0..=10.0).contains(&speed) {
                return Err(format!("game save Water transient {index} speed is invalid"));
            }
            actor.insert("speed".into(), json!(speed));
        }
        transients.push(Value::Object(actor));
    }
    source.insert("transients".into(), Value::Array(transients));
    Ok(Value::Object(source))
}

pub(super) fn secondary(value: &Value, version: u64, saved_tick: u64) -> Result<Value, String> {
    let mut source = record(value, "game save secondary abilities")?.clone();
    let camera = value.get("cameraDisplacements").cloned().unwrap_or_else(|| json!([]));
    camera_displacements(&camera, "game save secondary camera displacements")?;
    source.insert("cameraDisplacements".into(), camera);
    let mut rng: NativeRng = decode(value["rng"].clone())?;
    let mut actors = Vec::new();
    let legacy_debris: Vec<_> = array(&value["actors"], "game save secondary actors")?
        .iter()
        .filter(|actor| version < 48 && actor["kind"] == "ether-drain-debris")
        .collect();
    for (index, value) in array(&value["actors"], "game save secondary actors")?.iter().enumerate()
    {
        let mut actor = record(value, &format!("game save secondary actor {index}"))?.clone();
        if version < 48 && value["kind"] == "ether-drain-debris" {
            continue;
        }
        if value["kind"] == "ether-drain" {
            let field = format!("game save secondary actor {}.etherDrain", actors.len());
            let drain = if version < 48 {
                let mut animations = Vec::new();
                for child in &legacy_debris {
                    if child["worldKey"] != value["worldKey"] {
                        continue;
                    }
                    if array(&child["hitTargetIds"], "legacy field parent")?.first()
                        != Some(&value["id"])
                    {
                        continue;
                    }
                    animations.push(json!({"kind":"debris", "position":child["position"],
                        "direction":child["velocity"], "remainingDistance":child["quantity"],
                        "speed":child["slowFactor"], "oscillationDegrees":child["phase"],
                        "rotationDegrees":finite(&child["rotationRadians"], "legacy debris rotation")? * 180.0 / std::f64::consts::PI,
                        "variant":child["variant"]}));
                }
                actor.insert("hitTargetIds".into(), json!([]));
                actor.insert("quantity".into(), json!(0));
                json!({"animations":animations, "targetsInitialized":false, "lastLootRegistrationOrdinal":-1,
                    "loose":[], "queried":[], "scenery":[], "worldAnimationRefs":[],
                    "lastWorldAnimationIds":{"enemy-death-effect":0,"primary-transient":0,"secondary-actor":0,"death-weapon":0,"golem-fragment":0}})
            } else {
                value["etherDrain"].clone()
            };
            if !drain.is_null() {
                super::ether_drain::validate(&drain, &field)?;
            }
            actor.insert("etherDrain".into(), drain);
        }
        if version < 46 && value["kind"] == "storm-strike" {
            actor.insert("enhanced".into(), json!(true));
        }
        if value["kind"] == "golem-death" {
            use solomon_sim::death_animations::{
                NATIVE_GOLEM_DEATH_MAX_AGE_BASIC, NATIVE_GOLEM_DEATH_MAX_AGE_ENHANCED,
                create_native_golem_death_animation, step_native_golem_death_animation,
            };
            let enhanced = if version < 46 {
                true
            } else {
                value["enhanced"].as_bool().ok_or("saved Golem death quality is invalid")?
            };
            let lifetime = if enhanced {
                NATIVE_GOLEM_DEATH_MAX_AGE_ENHANCED
            } else {
                NATIVE_GOLEM_DEATH_MAX_AGE_BASIC
            };
            if version >= 48
                && (!value["presentationRng"].is_null() || value["lifetimeTicks"] != lifetime)
            {
                return Err(
                    "saved Golem death must retain its mutable owner and native lifetime envelope"
                        .into(),
                );
            }
            let animation = if version < 48 {
                let mut legacy_rng: NativeRng = decode(value["presentationRng"].clone())?;
                let age = integer_within(&value["ageTicks"], "saved Golem death age", 0, 133)?;
                let position: Vec2 = decode(value["position"].clone())?;
                let mut animation =
                    create_native_golem_death_animation(&mut legacy_rng, position, enhanced);
                for step in 1..=age {
                    step_native_golem_death_animation(
                        &mut animation,
                        (saved_tick + step).saturating_sub(age),
                        &mut legacy_rng,
                    );
                }
                encode(&animation)?
            } else {
                super::death::golem(
                    &value["golemDeath"],
                    &format!("game save secondary actor {index}.golemDeath"),
                    enhanced,
                )?;
                let key = format!("game save secondary actor {index}.painterRegistrations");
                let roots = super::protocol_values::limited_array(
                    &value["painterRegistrations"],
                    &key,
                    31,
                )?;
                let mut ordinals = std::collections::BTreeSet::new();
                for (root_index, root) in roots.iter().enumerate() {
                    ordinals.insert(super::protocol_values::registration(
                        root,
                        &format!("{key}[{root_index}]"),
                        "actor",
                    )?);
                }
                if roots.len() != 31 {
                    return Err(format!("{key} must contain exactly 31 roots"));
                }
                if ordinals.len() != roots.len() {
                    return Err(format!("{key} contains duplicate manager registrations"));
                }
                value["golemDeath"].clone()
            };
            actor.insert("enhanced".into(), json!(enhanced));
            actor.insert("golemDeath".into(), animation);
            actor.insert("lifetimeTicks".into(), json!(lifetime));
            actor.insert("presentationRng".into(), Value::Null);
        }
        if matches!(value["kind"].as_str(), Some("leviathan" | "leviathan-appendage")) {
            let rank = finite(&value["rank"], "saved Leviathan rank")?;
            let definition = &solomon_sim::profile::NATIVE_SKILL_CATALOG[11];
            if rank.fract() != 0.0
                || rank < 1.0
                || rank > f64::from(definition.maximum_level.max(2))
            {
                return Err(format!("secondary skill 11 has invalid effective rank {rank}"));
            }
            let damage = solomon_sim::secondary::native_secondary_ability_rank_stats(
                solomon_sim::secondary::NativeSecondaryAbilityId::CallLeviathan,
                rank as u32,
            )
            .value("mDamage")
            .expect("Leviathan defines damage");
            actor.insert("damage".into(), json!(damage));
        }
        if value["kind"].as_str() == Some("golem") {
            let mut golem = record(&value["golem"], "saved Golem")?.clone();
            let ticks = if version < 34 {
                0
            } else {
                integer_within(
                    &value["golem"]["circleSlowTicks"],
                    "saved Golem CircleSlow timer",
                    0,
                    20,
                )?
            };
            golem.insert("circleSlowTicks".into(), json!(ticks));
            actor.insert("golem".into(), Value::Object(golem));
        } else if version < 33 && value["kind"].as_str() == Some("dampened-projectile") {
            let position: Vec2 = decode(value["position"].clone())?;
            let velocity: Vec2 = decode(value["velocity"].clone())?;
            let origin = Vec2::new(position.x - velocity.x, position.y - velocity.y);
            let flyout = solomon_sim::secondary::dampened::create_native_dampened_spell(
                origin, position, &mut rng,
            );
            actor.extend([
                ("ageTicks".into(), json!(0)),
                ("frame".into(), json!(0)),
                ("lifetimeTicks".into(), json!(MAX_SAFE_INTEGER)),
                ("phase".into(), json!(flyout.phase_degrees)),
                ("rotationRadians".into(), json!(-std::f64::consts::PI / 180.0)),
                ("scale".into(), json!(1)),
                ("velocity".into(), encode(&flyout.velocity)?),
            ]);
        }
        actors.push(Value::Object(actor));
    }
    source.insert("actors".into(), Value::Array(actors));
    source.insert("rng".into(), encode(&rng)?);
    if version < 46 {
        let has_stoneskin =
            record(&value["players"], "game save secondary players")?.values().any(|player| {
                player["stoneskinTicksRemaining"].as_f64().is_some_and(|ticks| ticks > 0.0)
            });
        source.insert(
            "stoneskinWarp".into(),
            if has_stoneskin {
                encode(&solomon_sim::secondary::motion::create_native_stoneskin_warp(
                    &mut rng.clone(),
                ))?
            } else {
                Value::Null
            },
        );
    }
    if version < 30 {
        let mut events = array(&value["events"], "game save secondary events")?.clone();
        for event in &mut events {
            record(event, "game save secondary event")?;
            event["gain"] = json!(1);
        }
        source.insert("events".into(), Value::Array(events));
    }
    let mut players = record(&value["players"], "game save secondary players")?.clone();
    for (id, value) in &mut players {
        let mut player = record(value, &format!("game save secondary player {id}"))?.clone();
        let firewalker = player.get("firewalker").and_then(Value::as_bool) == Some(true);
        player.remove("staffCastTicksRemaining");
        player.extend([
            ("castSpinTicksRemaining".into(), json!(0)),
            ("heldSlot".into(), Value::Null),
            ("lastSkillId".into(), Value::Null),
            ("mindstar".into(), json!(false)),
            ("planeOrbHeld".into(), json!(false)),
            ("regenerate".into(), json!(false)),
            ("reservedMana".into(), json!(if firewalker { 50 } else { 0 })),
            ("castAction".into(), Value::Null),
        ]);
        *value = Value::Object(player);
    }
    source.insert("players".into(), Value::Object(players));
    Ok(Value::Object(source))
}

fn camera_displacements(value: &Value, field: &str) -> Result<(), String> {
    use super::protocol_values::{integer, limited_array, object, string, vector};
    let mut worlds = std::collections::BTreeSet::new();
    for (index, proposal) in limited_array(value, field, 32768)?.iter().enumerate() {
        let key = format!("{field}[{index}]");
        object(proposal, &key, &["displacement", "tick", "worldKey"])?;
        let world = string(&proposal["worldKey"], &format!("{key}.worldKey"), 256)?;
        if !worlds.insert(world) {
            return Err(format!("{field} has duplicate Region proposals"));
        }
        vector(&proposal["displacement"], &format!("{key}.displacement"))?;
        integer(&proposal["tick"], &format!("{key}.tick"), false)?;
    }
    Ok(())
}
