//! Boneyard/Tutorial default reconstruction and versioned world migrations.

mod actors;
mod discorporeal;
mod effects;
mod projectiles;
mod validation;

use serde_json::{Value, json};
use solomon_sim::boneyard::waves::{native_survival_programs, programs::SpiderState};
use solomon_sim::{NativeRng, boneyard::BoneyardWorld};

use super::super::contract::{MAX_SAFE_INTEGER, record};
use super::super::value::{decode, encode};
use super::{array, finite, integer_within, puppet_hit};

pub(super) fn normalize(
    value: &Value,
    loaded: &Value,
    owner: &str,
    version: u64,
    tick: u64,
) -> Result<Value, String> {
    let mut world = record(value, "game save world")?.clone();
    if value["kind"].as_str() == Some("hub") {
        if version < 22 {
            record(&value["ambient"], "legacy game save Hub ambient state")?;
            let mut ambient = value["ambient"].clone();
            ambient["teacherTick"] = json!(0);
            ambient["teacherWorldRelease"] = Value::Null;
            world.insert("ambient".into(), ambient);
        }
        return Ok(Value::Object(world));
    }
    if value["kind"].as_str() != Some("boneyard") {
        return Ok(value.clone());
    }
    let loaded = super::super::restore::loaded_boneyard(loaded)?;
    let defaults = super::super::projection::boneyard(&BoneyardWorld::new(loaded.clone()))?;
    let weapons = if version < 48 {
        json!([])
    } else {
        super::death::weapons(&value["deathWeapons"], "game save deathWeapons", tick)?;
        value["deathWeapons"].clone()
    };
    let next_weapon = if version < 48 {
        1
    } else {
        integer_within(
            &value["nextDeathWeaponId"],
            "game save nextDeathWeaponId",
            1,
            MAX_SAFE_INTEGER,
        )?
    };
    if weapons
        .as_array()
        .expect("validated death weapons")
        .iter()
        .any(|actor| actor["id"].as_u64().is_some_and(|id| id >= next_weapon))
    {
        return Err("game save death weapon allocator is behind its actors".into());
    }
    world.insert("deathWeapons".into(), weapons);
    world.insert("nextDeathWeaponId".into(), json!(next_weapon));
    let mut enemies = record(&value["enemies"], "game save Boneyard enemies")?.clone();
    let mut private_rng = finite(&enemies["rngState"], "game save Boneyard enemy RNG")? as u32;
    let mut native_rng: NativeRng = decode(
        enemies
            .get("steeringRngState")
            .filter(|value| !value.is_null())
            .unwrap_or(&defaults["enemies"]["steeringRngState"])
            .clone(),
    )?;
    let actors = actors::normalize(
        &value["enemies"]["actors"],
        version,
        tick,
        &value["enemies"]["lastStepTick"],
        &loaded.source_sha256,
        &mut private_rng,
        &mut native_rng,
    )?;
    let projectiles =
        projectiles::normalize(&value["enemies"]["projectiles"], version, &mut native_rng)?;
    let mut featured = if version < 34 || value["enemies"]["featuredBossId"].is_null() {
        Value::Null
    } else {
        json!(integer_within(
            &value["enemies"]["featuredBossId"],
            "game save featuredBossId",
            1,
            MAX_SAFE_INTEGER
        )?)
    };
    if version < 34 {
        let candidates: Vec<_> = actors
            .iter()
            .filter(|actor| {
                actor["lifeState"].as_str() == Some("alive")
                    && matches!(
                        actor["config"]["classification"].as_str(),
                        Some("boss" | "multiple-boss")
                    )
            })
            .collect();
        if !candidates.is_empty() {
            featured =
                candidates[native_rng.draw_integer(candidates.len() as i32) as usize]["id"].clone();
        }
    }
    if !featured.is_null()
        && !actors.iter().any(|actor| {
            actor["id"] == featured
                && actor["lifeState"].as_str() == Some("alive")
                && actor["config"]["classification"].as_str() != Some("normal")
        })
    {
        return Err("game save featuredBossId must refer to a living boss or miniboss".into());
    }
    enemies.insert("featuredBossId".into(), featured);
    enemies.insert("actors".into(), Value::Array(actors));
    enemies.insert("projectiles".into(), Value::Array(projectiles.clone()));
    let mut maggots = array(&value["enemies"]["maggots"], "game save Maggots")?.clone();
    for (index, maggot) in maggots.iter_mut().enumerate() {
        record(maggot, &format!("game save Maggot {index}"))?;
        if let Some(captured) = maggot.get("etherDrainCaptured")
            && (captured != true || maggot["lifeState"] != "dying")
        {
            return Err(format!("game save Maggot {index} has an invalid Ether Drain capture"));
        }
        maggot["nativeTreeQueryMember"] = json!(false);
        maggot["hitFeedback"] = puppet_hit(
            &maggot["hitFeedback"],
            &maggot["lastDamageTick"],
            tick,
            version,
            &format!("game save Maggot {index} hit feedback"),
        )?;
    }
    enemies.insert("maggots".into(), Value::Array(maggots));
    enemies.insert(
        "deathEffects".into(),
        effects::normalize(&value["enemies"]["deathEffects"], version)?,
    );
    for (field, default) in [
        ("demonSkullEncounter", defaults["enemies"]["demonSkullEncounter"].clone()),
        ("bossNarration", defaults["enemies"]["bossNarration"].clone()),
        ("facultyVoiceController", Value::Null),
        ("bossSpells", json!([])),
        ("puppetHits", json!([])),
        ("detachedCrows", json!([])),
    ] {
        if version < 34 {
            enemies.insert(field.into(), default);
        }
    }
    if version >= 34 {
        validation::demon_skull_encounter(&value["enemies"]["demonSkullEncounter"])?;
        validation::puppet_hits(&value["enemies"]["puppetHits"], tick)?;
        if version < 46 {
            let mut spells = array(&value["enemies"]["bossSpells"], "saved boss spells")?.clone();
            for spell in &mut spells {
                record(spell, "saved boss spell")?;
                if spell["kind"] == "blightning" {
                    spell["enhancedEffects"] = json!(true);
                }
            }
            enemies.insert("bossSpells".into(), Value::Array(spells));
        }
    }
    if version < 33 {
        for field in ["silks", "silkFragments", "spiderRemains"] {
            enemies.insert(field.into(), json!([]));
        }
        enemies.insert("webbedPlayers".into(), json!({}));
        enemies.insert("spiderSpitTicksRemaining".into(), json!(0));
    }
    if enemies.get("locomotionRngState").is_none_or(Value::is_null) {
        enemies
            .insert("locomotionRngState".into(), defaults["enemies"]["locomotionRngState"].clone());
    }
    if version < 32 {
        enemies.insert("projectileKnockbacks".into(), json!([]));
        enemies.insert("targetCellBindings".into(), json!({}));
    } else {
        validation::knockbacks(
            &value["enemies"]["projectileKnockbacks"],
            finite(&value["enemies"]["lastStepTick"], "game save enemy tick")?,
        )?;
        validation::target_bindings(&value["enemies"]["targetCellBindings"])?;
    }
    enemies.insert(
        "projectileEffects".into(),
        Value::Array(projectiles::effects(
            &value["enemies"]["projectileEffects"],
            &projectiles,
            version,
        )?),
    );
    enemies.insert("rngState".into(), json!(private_rng));
    enemies.insert("steeringRngState".into(), encode(&native_rng)?);
    world.insert("enemies".into(), Value::Object(enemies));
    world.insert("encounter".into(), encounter(&value["encounter"], &defaults["encounter"])?);
    let tutorial = tutorial(value.get("tutorial").unwrap_or(&defaults["tutorial"]), version)?;
    let baseline = if value["tutorialProfileEconomy"].is_null() {
        Value::Null
    } else {
        encode(
            &super::super::profile::normalize_economy(&value["tutorialProfileEconomy"], version)?.0,
        )?
    };
    if tutorial.is_null() != baseline.is_null() {
        return Err("game save Tutorial profile baseline ownership is inconsistent".into());
    }
    world.insert("tutorial".into(), tutorial);
    world.insert("tutorialProfileEconomy".into(), baseline);
    for field in ["enemyWorldFeedback", "lanternPosition"] {
        if world.get(field).is_none_or(Value::is_null) {
            world.insert(field.into(), defaults[field].clone());
        }
    }
    if world.get("hallOfFameRuns").is_none_or(Value::is_null) {
        world.insert(
            "hallOfFameRuns".into(),
            json!({owner:encode(&solomon_sim::score::NativeHallOfFameRunState::new(0))?}),
        );
    }
    if version < 34 {
        let previous = array(
            value
                .get("primarySceneryTargets")
                .filter(|value| !value.is_null())
                .unwrap_or(&defaults["primarySceneryTargets"]),
            "legacy saved scenery targets",
        )?;
        let mut retained = Vec::new();
        for target in array(&defaults["primarySceneryTargets"], "default scenery targets")? {
            if target["id"].as_str().is_some_and(|id| id.starts_with("fencepost:"))
                || previous.iter().any(|old| old["id"] == target["id"])
            {
                retained.push(target.clone());
            }
        }
        world.insert("primarySceneryTargets".into(), Value::Array(retained));
    }
    world.insert(
        "waves".into(),
        waves(&value["waves"], &defaults["waves"], &loaded.source_sha256, version)?,
    );
    Ok(Value::Object(world))
}

fn encounter(value: &Value, defaults: &Value) -> Result<Value, String> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let mut source = record(value, "game save Boneyard Solomon encounter")?.clone();
    let dig_event = source.remove("digAudioEventId").unwrap_or(json!(0));
    source.remove("digAudioEvents");
    for (field, default) in [
        ("digBodyBobAmplitude", defaults.get("digBodyBobAmplitude").cloned().unwrap_or(json!(5))),
        ("digBodyOffsetY", json!(0)),
        ("digEventId", dig_event),
        ("digEvents", json!([])),
        ("dialogueMode", json!("ordinary")),
        ("escapeCollisionSourceIds", json!([])),
        ("escapeTarget", Value::Null),
        ("tutorialDialogueTicks", json!(0)),
    ] {
        if source.get(field).is_none_or(Value::is_null) {
            source.insert(field.into(), default);
        }
    }
    Ok(Value::Object(source))
}

fn tutorial(value: &Value, version: u64) -> Result<Value, String> {
    use solomon_sim::boneyard::tutorial::{
        NATIVE_TUTORIAL_CAMERA_CLEANUP_TICKS, NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS,
    };
    if value.is_null() {
        return Ok(Value::Null);
    }
    let mut source = record(value, "game save Tutorial")?.clone();
    if version >= 16 && value["movementInstructionAcknowledged"].as_bool().is_none() {
        return Err("game save Tutorial movementInstructionAcknowledged must be boolean".into());
    }
    if !source.contains_key("cameraLockAgeTicks") {
        let triggered = value["cameraLockTriggered"].as_bool() == Some(true);
        let age = match value["cameraLockTicksRemaining"]
            .as_u64()
            .filter(|ticks| *ticks > 0 && *ticks <= MAX_SAFE_INTEGER)
        {
            Some(remaining) if triggered => {
                NATIVE_TUTORIAL_CAMERA_CLEANUP_TICKS as f64 - remaining as f64
            }
            _ if triggered => NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS as f64,
            _ => 0.0,
        };
        source.insert("cameraLockAgeTicks".into(), json!(age));
    }
    source.insert(
        "movementInstructionAcknowledged".into(),
        json!(value["movementInstructionAcknowledged"].as_bool() == Some(true)),
    );
    source.insert(
        "selectedSkillHudAcknowledged".into(),
        json!(value["selectedSkillHudAcknowledged"].as_bool() == Some(true)),
    );
    Ok(Value::Object(source))
}

fn waves(value: &Value, defaults: &Value, source_sha: &str, version: u64) -> Result<Value, String> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let mut waves = record(value, "game save Boneyard waves")?.clone();
    if version < 33 {
        let definitions =
            native_survival_programs(source_sha).map(|program| program.spiders).unwrap_or_default();
        let wave = finite(&value["waveOrdinal"], "game save wave ordinal")?;
        let phase = definitions
            .iter()
            .position(|definition| definition.start_wave as f64 >= wave)
            .unwrap_or(definitions.len());
        waves.insert("spiderWaves".into(), encode(&definitions)?);
        waves.insert(
            "spiderState".into(),
            encode(&SpiderState { phase_index: phase, ..SpiderState::default() })?,
        );
    }
    if version < 34 {
        waves.insert(
            "bossEncounters".into(),
            defaults.get("bossEncounters").cloned().unwrap_or(json!([])),
        );
    }
    for (field, fallback) in [
        ("openingBursts", json!([])),
        ("openingReleaseThreshold", json!(0)),
        ("portalPhaseIndex", json!(0)),
        ("portalProgram", Value::Null),
        ("portalScriptPhase", json!("retired")),
        ("portalSpawnRemaining", json!(0)),
        ("portalTicksRemaining", json!(0)),
        ("portalTimelinePaused", json!(false)),
        ("slumpgutPhase", json!("eligible")),
        ("slumpgutPollCursor", json!(0)),
        ("slumpgutRecipeUid", Value::Null),
        ("slumpgutTicksRemaining", json!(0)),
    ] {
        if waves.get(field).is_none_or(Value::is_null) {
            let default = if matches!(
                field,
                "portalProgram" | "portalScriptPhase" | "slumpgutPhase" | "slumpgutRecipeUid"
            ) {
                defaults.get(field).cloned().unwrap_or(fallback)
            } else {
                fallback
            };
            waves.insert(field.into(), default);
        }
    }
    Ok(Value::Object(waves))
}
