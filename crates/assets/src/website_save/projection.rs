//! Source component and immutable-world JSON for Website save interchange.

use serde_json::{Value, json};
use solomon_sim::boneyard::waves::director::WaveDirector;
use solomon_sim::boneyard::{BoneyardWorld, LoadedBoneyard};
use solomon_sim::session::GameSession;
use solomon_sim::spells::boneyard::boneyard_primary_scenery_targets;

use super::value::encode;

pub(super) fn loaded_boneyard(loaded: &LoadedBoneyard) -> Result<Value, String> {
    let mut choice =
        json!({"id":loaded.choice_id,"name":loaded.name,"source":encode(&loaded.choice_source)?});
    if let Some(value) = &loaded.mod_id {
        choice["modId"] = json!(value);
    }
    if let Some(value) = &loaded.mod_name {
        choice["modName"] = json!(value);
    }
    Ok(json!({"choice":choice,"geometrySha256":loaded.geometry_sha256,"runId":loaded.run_id,
        "scene":encode(&loaded.scene)?,"seed":loaded.seed,"sourceSha256":loaded.source_sha256}))
}

pub(super) fn simulation(state: &GameSession, owner_id: &str) -> Result<Value, String> {
    let participant = &state.participants[owner_id];
    let player = &participant.player;
    let mut progression = encode(&player.progression)?;
    progression
        .as_object_mut()
        .unwrap()
        .extend(encode(&player.combat)?.as_object().unwrap().clone());
    let mut locomotion = encode(&participant.wizard)?;
    for key in ["primaryCast", "region", "transition"] {
        locomotion.as_object_mut().unwrap().remove(key);
    }
    let players = json!({
        "belts":[encode(&player.belt)?],"configs":[encode(&player.selection)?],
        "economies":[encode(&player.economy)?],"entityIds":[participant.entity_id],
        "identities":[{"playerId":owner_id}],"lightings":[encode(&participant.lighting)?],
        "locomotions":[locomotion],"nextEntityId":state.next_entity_id,
        "primaryCasts":[encode(&participant.wizard.primary_cast)?],"progressions":[progression],
        "skillBooks":[encode(&player.book)?],"skillRuntimes":[encode(&player.skill_runtime)?],
        "statBooks":[encode(&player.stat_book)?],
    });
    let world = if let Some(world) = &state.boneyard {
        let mut result = boneyard(world)?;
        result["hallOfFameRuns"] = json!({owner_id:encode(&participant.score)?});
        result["tutorialProfileEconomy"] = encode(&participant.tutorial_profile_economy)?;
        result
    } else {
        let mut students = encode(&state.npcs.students)?;
        students.as_object_mut().unwrap().remove("nextRegistrationOrdinal");
        students["routeEndBehavior"] = json!("retire");
        json!({"ambient":encode(&state.ambient)?,"collisionRngState":state.collision_rng,
            "kind":"hub","participants":{owner_id:{"collegeIntro":encode(&participant.college_intro)?,
                "region":encode(&participant.wizard.region)?,"transition":encode(&participant.wizard.transition)?}},
            "skorcha":encode(&state.npcs.skorcha.skorcha)?,"studentPopulation":students,
            "traderAnimationSeed":state.hub_trader_animation_seed})
    };
    Ok(json!({"accumulatorSeconds":0,"combatRng":encode(&state.combat_rng)?,
        "enhancedEffects":state.enhanced_effects,"screenFlashes":encode(&state.screen_flashes)?,
        "gameRng":encode(&state.gameplay_rng)?,"hallOfFameClockStartedAtTick":state.hall_of_fame_clock_started_at_tick,
        "levelUpBarrier":encode(&state.level_up_barrier)?,"worldManagerOrder":encode(&state.painter_order)?,
        "modEffects":[],"nextLevelUpBarrierId":state.next_level_up_barrier_id,"nextModConsumableUseId":1,
        "playerEntities":players,"primarySpells":encode(&state.spells)?,"run":encode(&state.run)?,
        "secondaryAbilities":encode(&state.secondary)?,"tick":state.ticks,"world":world}))
}

pub(super) fn boneyard(world: &BoneyardWorld) -> Result<Value, String> {
    let mut result = encode(&world.checkpoint())?;
    let fields = result.as_object_mut().unwrap();
    for key in [
        "choiceId",
        "choiceSource",
        "modId",
        "modName",
        "seed",
        "sourceSha256",
        "geometrySha256",
        "ticks",
        "sceneryRetainedBounds",
        "sceneryTargetRetention",
    ] {
        fields.remove(key);
    }
    fields.insert("kind".into(), json!("boneyard"));
    fields.insert("bounds".into(), encode(&world.loaded.scene.bounds)?);
    fields.insert("spawn".into(), encode(&world.loaded.scene.spawn)?);
    fields.insert(
        "waves".into(),
        world.waves.as_ref().map(waves).transpose()?.unwrap_or(Value::Null),
    );
    let collision = &world.collision;
    fields.insert("collision".into(),json!({
        "circles":collision.circles.iter().map(|circle| collision_shape(json!({"center":circle.center,"radius":circle.radius}), circle.native_line_mask, circle.source_id.as_deref())).collect::<Vec<_>>(),
        "polygons":collision.polygons.iter().map(|polygon| collision_shape(json!({"points":polygon.points}), polygon.native_line_mask, polygon.source_id.as_deref())).collect::<Vec<_>>(),
        "segments":collision.segments.iter().map(|segment| collision_shape(json!({"start":segment.start,"end":segment.end,"radius":segment.radius}), segment.native_line_mask, segment.source_id.as_deref())).collect::<Vec<_>>()
    }));
    let inside = |position: solomon_sim::Vec2| {
        world.cleanup_bounds().is_none_or(|bounds| {
            position.x >= bounds.x
                && position.y >= bounds.y
                && position.x <= bounds.x + bounds.width
                && position.y <= bounds.y + bounds.height
        })
    };
    let mut earthquake_targets = Vec::new();
    let mut spell_targets = Vec::new();
    for (index, object) in world.loaded.scene.objects.iter().enumerate() {
        if !inside(object.position) {
            continue;
        }
        if world.retains_earthquake_scenery_target(index) {
            let mut target = json!({
                "id":index, "position":object.position, "typeId":object.type_id,
            });
            if let Some(variant) = world.earthquake_scenery_variant(index) {
                target["secondaryVariant"] = json!(variant);
            }
            earthquake_targets.push(target);
        }
        let id = format!("scenery:{}", object.eid);
        if object.type_id == 2029 && world.retains_scenery_spell_target(&id) {
            spell_targets.push(json!({
                "active":true,"actorFlags":4,"attachment":{"x":0,"y":0},"bodyRadius":0,
                "cellBindingOrder":index,"id":id,"kind":"gravestone",
                "nativePriority":1000,"queryLane":"grid","pendingRemove":false,
                "position":object.position,"registrationOrder":index,
            }));
        }
    }
    fields.insert("earthquakeSceneryTargets".into(), Value::Array(earthquake_targets));
    fields.insert("primarySceneryTargets".into(),json!(boneyard_primary_scenery_targets(world).into_iter().map(|target| json!({
        "active":target.active,"actorFlags":target.actor_flags,"attachment":target.attachment,
        "bodyRadius":target.body_radius,"cellBindingOrder":target.cell_binding_order,"id":target.id,
        "kind":if target.kind == solomon_sim::spells::targeting::PrimarySpellTargetKind::Gravestone {"gravestone"} else {"scenery"},"nativePriority":target.native_priority,"queryLane":"grid",
        "pendingRemove":target.pending_remove,"position":target.position,"registrationOrder":target.registration_order,
    })).collect::<Vec<_>>()));
    fields.insert("scenerySpellTargets".into(), Value::Array(spell_targets));
    Ok(result)
}

fn waves(director: &WaveDirector) -> Result<Value, String> {
    let mut result = encode(&director.state)?;
    let fields = result.as_object_mut().unwrap();
    fields.remove("activeSection");
    fields.insert("activeBursts".into(), encode(&director.active_bursts)?);
    fields.insert("compiledSchedule".into(), encode(&director.compiled_schedule)?);
    fields.insert("openingBursts".into(), encode(&director.opening_bursts)?);
    fields.insert("openingReleaseThreshold".into(), json!(director.opening_release_threshold));
    fields.insert("schedule".into(), encode(&director.schedule)?);
    let mut portal_program = Value::Null;
    let mut spiders = json!([]);
    if let Some(programs) = &director.programs {
        let mut phases = encode(&programs.portals)?;
        for phase in phases.as_array_mut().unwrap() {
            phase.as_object_mut().unwrap().remove("recipe");
        }
        portal_program = json!({"timelineUid":programs.portal_timeline_uid,"phases":phases});
        spiders = encode(&programs.spiders)?;
        for boss in fields["bossEncounters"].as_array().unwrap().clone().into_iter().enumerate() {
            fields.get_mut("bossEncounters").unwrap()[boss.0]["sourceSha256"] =
                json!(programs.source_sha256);
        }
        if let Some(bosses) = fields.get_mut("skeletonBosses").and_then(Value::as_object_mut) {
            bosses.insert("sourceSha256".into(), json!(programs.source_sha256));
        }
    }
    fields.insert("portalProgram".into(), portal_program);
    fields.insert("spiderWaves".into(), spiders);
    Ok(Value::Object(std::mem::take(fields)))
}

fn collision_shape(mut value: Value, mask: u32, source_id: Option<&str>) -> Value {
    if mask != 0 {
        value["nativeLineMask"] = json!(mask);
    }
    if let Some(id) = source_id {
        value["sourceId"] = json!(id);
    }
    value
}
