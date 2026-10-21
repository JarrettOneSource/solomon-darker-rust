//! Complete Tutorial progression, retained cursors and authored tables against the Website.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::encounter::SolomonPhase;
use solomon_sim::boneyard::tutorial::*;
use solomon_sim::economy::HubInventoryItem;
use solomon_sim::enemies::EnemySpawnIntent;

#[path = "support/enemy_snapshots.rs"]
mod enemy_snapshots;
mod support;
#[path = "support/tutorial_snapshot.rs"]
mod tutorial_snapshot;
use enemy_snapshots::{policy_id, recipe_snapshot};
use support::assert_website_json;
use tutorial_snapshot::{narration_row, state_row};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/tutorial-goldens.json")).unwrap()
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}
fn integer(value: &Value) -> u64 {
    value.as_u64().unwrap()
}
fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}
fn point_row(point: Vec2) -> Value {
    json!({ "x": point.x, "y": point.y })
}
fn bounds_row(bounds: BoneyardBounds) -> Value {
    json!({ "x": bounds.x, "y": bounds.y, "w": bounds.width, "h": bounds.height })
}
fn compare(actual: &Value, expected: &Value, label: &str) {
    assert_website_json(actual, expected, label, &[]);
}

fn initial(fixture: &Value) -> NativeTutorialState {
    NativeTutorialState::new(point(&fixture["position"]), 3, fixture["seed"].as_str().unwrap())
}

fn after_intro(state: &mut NativeTutorialState) {
    state.intro_active = false;
    state.intro_blend = 1.0;
    state.intro_delay_ticks_remaining = 0;
    state.intro_fade = 0.0;
    state.intro_movement_ticks_remaining = 250;
}

fn patch_state(state: &mut NativeTutorialState, patch: &Value) {
    for (field, value) in patch.as_object().unwrap() {
        match field.as_str() {
            "stage" => state.stage = NativeTutorialStage::from_id(integer(value) as u8).unwrap(),
            "stageTicks" => state.stage_ticks = integer(value),
            "inventoryOpened" => state.inventory_opened = value.as_bool().unwrap(),
            "inventorySeen" => state.inventory_seen = value.as_bool().unwrap(),
            "skillsOpened" => state.skills_opened = value.as_bool().unwrap(),
            "skillsSeen" => state.skills_seen = value.as_bool().unwrap(),
            "cameraLockTriggered" => state.camera_lock_triggered = value.as_bool().unwrap(),
            "cameraLockAgeTicks" => state.camera_lock_age_ticks = number(value),
            "cameraLockTicksRemaining" => state.camera_lock_ticks_remaining = integer(value),
            _ => panic!("unhandled retained Tutorial field {field}"),
        }
    }
}

fn input(value: &Value) -> NativeTutorialTickInput {
    let phase = match value["solomonPhase"].as_str() {
        None => None,
        Some("speaking") => Some(SolomonPhase::Speaking),
        Some("retreat-accelerating") => Some(SolomonPhase::RetreatAccelerating),
        Some(value) => panic!("unhandled source Solomon input {value}"),
    };
    NativeTutorialTickInput {
        acid_rain_cast_sequence: integer(&value["acidRainCastSequence"]),
        acid_rain_last_skill_id: value["acidRainLastSkillId"].as_u64().map(|id| id as usize),
        camera_lock_safety_clear: value["cameraLockSafetyClear"].as_bool().unwrap(),
        current_health: number(&value["currentHealth"]),
        enemy_count: integer(&value["enemyCount"]) as usize,
        ground_sack_count: integer(&value["groundSackCount"]) as usize,
        has_top_level_non_potion_item: value["hasTopLevelNonPotionItem"].as_bool().unwrap(),
        health_potion_count: integer(&value["healthPotionCount"]) as usize,
        level: integer(&value["level"]) as u32,
        level_up_pending: value["levelUpPending"].as_bool().unwrap(),
        maximum_health: number(&value["maximumHealth"]),
        player_action_idle: value["playerActionIdle"].as_bool().unwrap(),
        player_position: point(&value["playerPosition"]),
        primary_cast_sequence: integer(&value["primaryCastSequence"]),
        solomon_phase: phase,
        solomon_run_event_id: integer(&value["solomonRunEventId"]),
        tick: integer(&value["tick"]),
    }
}

fn spawn_row(spawn: &EnemySpawnIntent) -> Value {
    let mut recipe = recipe_snapshot(spawn.authored_recipe.as_ref().unwrap());
    recipe["enemyToken"] = json!(spawn.enemy_token.id());
    recipe["flanking"] = json!(spawn.flanking);
    recipe["pathfindingMode"] = json!(spawn.pathfinding_mode);
    let mut row = json!({ "authoredRecipe": recipe, "enemyToken": spawn.enemy_token.id(),
        "flags": [], "flanking": spawn.flanking, "id": spawn.id,
        "locationPolicy": "near-player", "nativeTypeId": spawn.native_type_id,
        "pathfindingMode": spawn.pathfinding_mode, "position": point_row(spawn.position),
        "positionPolicy": policy_id(spawn.position_policy), "spawnTick": spawn.spawn_tick,
        "waveOrdinal": spawn.wave_ordinal });
    if let Some(id) = spawn.placement_group_id {
        row["placementGroupId"] = json!(id);
    }
    row
}

fn result_row(state: &NativeTutorialState, result: &NativeTutorialTickResult) -> Value {
    json!({ "forceOfferSkillIds": result.force_offer_skill_ids, "grantExperience": result.grant_experience,
        "spawnIntents": result.spawn_intents.iter().map(spawn_row).collect::<Vec<_>>(), "state": state_row(state) })
}

fn presentation_row(state: &NativeTutorialState) -> Value {
    let bindings = NativeTutorialBindingLabels {
        inventory: "I",
        move_down: "S",
        move_left: "A",
        move_right: "D",
        move_up: "W",
        potion: "Q",
        secondary: "SPACE",
        skills: "K",
    };
    let row = |mode| {
        let plan = native_tutorial_presentation(state, bindings, mode);
        json!({ "heading": plan.heading, "subheading": plan.subheading,
            "hud": { "combat": plan.hud.combat, "inventory": plan.hud.inventory,
                "quickbar": plan.hud.quickbar, "skills": plan.hud.skills, "spell": plan.hud.spell } })
    };
    json!({ "desktop": row(NativeTutorialInputMode::Desktop), "mobile": row(NativeTutorialInputMode::Mobile),
        "baselines": native_tutorial_instruction_baselines(state.stage, 900.0)
            .map(|baseline|json!({ "heading": baseline.heading, "subheading": baseline.subheading })),
        "forcedVelocity": native_tutorial_forced_velocity(state).map(point_row),
        "hostilePaused": native_tutorial_hostile_scene_paused(state),
        "camera": native_tutorial_camera_bounds(state).map(bounds_row) })
}

#[test]
fn tutorial_intro_stages_surfaces_and_instructions_match_the_source() {
    let fixture = fixture();
    compare(&state_row(&initial(&fixture)), &fixture["initial"], "initial");
    for case in fixture["steps"].as_array().unwrap() {
        let mut state = initial(&fixture);
        after_intro(&mut state);
        patch_state(&mut state, &case["patch"]);
        let result = state.step(input(&case["input"]));
        compare(&result_row(&state, &result), &case["expected"], case["name"].as_str().unwrap());
        compare(&presentation_row(&state), &case["presentation"], "stage presentation");
    }
    let mut state = initial(&fixture);
    let mut frames = fixture["intro"].as_array().unwrap().iter().peekable();
    for tick in 0..510 {
        let mut context = input(&fixture["baseInput"]);
        context.tick = tick;
        state.step(context);
        if frames.peek().is_some_and(|frame| frame["tick"] == tick) {
            let frame = frames.next().unwrap();
            compare(&state_row(&state), &frame["state"], "intro state");
            compare(&presentation_row(&state), &frame["presentation"], "intro presentation");
        }
        if !state.intro_active {
            break;
        }
    }
    assert!(frames.next().is_none());
    for case in fixture["surfaces"].as_array().unwrap() {
        let mut state = initial(&fixture);
        after_intro(&mut state);
        state.stage = NativeTutorialStage::from_id(integer(&case["stage"]) as u8).unwrap();
        state.inventory_seen = true;
        state.skills_seen = true;
        let action = NativeTutorialSurfaceAction::ALL
            .into_iter()
            .find(|action| action.id() == case["action"].as_str().unwrap())
            .unwrap();
        state.apply_surface_action(action);
        compare(&state_row(&state), &case["state"], "surface state");
        compare(&presentation_row(&state), &case["presentation"], "surface presentation");
    }
    for case in fixture["acknowledgements"].as_array().unwrap() {
        let mut state = initial(&fixture);
        after_intro(&mut state);
        state.stage = NativeTutorialStage::from_id(integer(&case["stage"]) as u8).unwrap();
        state.intro_active = case["introActive"].as_bool().unwrap();
        state.acknowledge_movement_instruction(
            case["input"]["movementEpochActive"].as_bool().unwrap(),
            case["input"]["userMovementRequested"].as_bool().unwrap(),
        );
        compare(&state_row(&state), &case["state"], "movement acknowledgement");
    }
}

#[test]
fn tutorial_waves_and_all_narration_cues_preserve_future_cursors() {
    let fixture = fixture();
    for case in fixture["waves"].as_array().unwrap() {
        let mut state = initial(&fixture);
        after_intro(&mut state);
        state.active = false;
        state.stage = NativeTutorialStage::Survive;
        state.wave_ordinal = integer(&case["waveOrdinal"]);
        state.survival_enabled = state.wave_ordinal == 6;
        let mut frames = case["frames"].as_array().unwrap().iter().peekable();
        for tick in 1..=1600 {
            let mut context = input(&fixture["baseInput"]);
            context.tick = tick;
            context.player_position = Vec2::new(1000.0, 500.0);
            context.enemy_count = if tick < 600 {
                20
            } else if tick < 900 {
                150
            } else {
                0
            };
            context.level = if tick < 1000 { 1 } else { 4 };
            let result = state.step(context);
            if frames.peek().is_some_and(|frame| frame["tick"] == tick) {
                compare(
                    &result_row(&state, &result),
                    &frames.next().unwrap()["expected"],
                    &format!("wave{} tick{tick}", state.wave_ordinal),
                );
            } else {
                assert!(result.spawn_intents.is_empty(), "unrecorded source birth at {tick}");
            }
        }
        assert!(frames.next().is_none());
    }
    let mut state = initial(&fixture);
    after_intro(&mut state);
    state.active = false;
    state.narration.pending = NativeTutorialCue::ALL.to_vec();
    let mut frames = fixture["narration"].as_array().unwrap().iter().peekable();
    for tick in 1..15000 {
        let mut context = input(&fixture["baseInput"]);
        context.tick = tick;
        state.step(context);
        if frames.peek().is_some_and(|frame| frame["tick"] == tick) {
            compare(
                &narration_row(&state.narration),
                &frames.next().unwrap()["state"],
                "narration",
            );
        }
        if state.narration.idle() {
            break;
        }
    }
    assert!(frames.next().is_none());
}

#[test]
fn tutorial_camera_and_spawn_domain_match_the_authored_fence() {
    let fixture = fixture();
    for case in fixture["cameras"].as_array().unwrap() {
        let mut state = initial(&fixture);
        state.camera_lock_triggered = true;
        state.camera_lock_age_ticks = number(&case["age"]);
        compare(
            &native_tutorial_camera_bounds(&state).map(bounds_row).unwrap(),
            &case["bounds"],
            "camera interpolation",
        );
    }
    for case in fixture["domains"].as_array().unwrap() {
        let position = point(&case["point"]);
        let radius = number(&case["radius"]);
        assert_eq!(
            native_tutorial_enemy_camera_position_is_allowed(position, radius),
            case["camera"].as_bool().unwrap()
        );
        assert_eq!(
            native_tutorial_enemy_spawn_position_is_allowed(position, radius),
            case["spawn"].as_bool().unwrap()
        );
    }
}

fn item_row(item: &HubInventoryItem) -> Value {
    let mut row = json!({ "equipmentType": item.equipment_type.map(|kind|kind.as_str()),
        "iconRecords": item.icon_records, "id": item.id, "kind": item.kind.as_str(),
        "name": item.name, "nativeSubtype": item.native_subtype, "nativeTypeId": item.native_type_id,
        "quantity": item.quantity, "rarity": null, "recipeIndex": item.recipe_index });
    if let Some(selector) = item.native_selector {
        row["nativeSelector"] = json!(selector);
    }
    if let Some(tints) = item.icon_tints {
        row["iconTints"] = json!(tints);
    }
    if let Some(effects) = &item.native_effects {
        row["nativeEffects"] = json!(
            effects
                .iter()
                .map(|effect| json!({ "kind": effect.kind,
            "magnitude": effect.magnitude, "operator": effect.operator, "target": effect.target }))
                .collect::<Vec<_>>()
        );
    }
    row
}

#[test]
fn tutorial_authored_recipes_groups_cues_and_reward_items_are_complete() {
    let fixture = fixture();
    let tables = &fixture["tables"];
    for uid in NATIVE_TUTORIAL_MONSTER_UIDS {
        let definition = native_tutorial_monster_recipe(uid).unwrap();
        let mut actual = recipe_snapshot(&definition.recipe);
        actual["enemyToken"] = json!(definition.enemy_token.id());
        actual["flanking"] = json!(definition.flanking);
        actual["pathfindingMode"] = json!(definition.pathfinding_mode);
        compare(&actual, &tables["recipes"][uid.to_string()], "authored recipe");
    }
    for uid in NATIVE_TUTORIAL_GROUP_UIDS {
        let group = native_tutorial_uid_group(uid).unwrap();
        compare(
            &json!({ "memberUids": group.member_uids, "positionCacheDword": group.position_cache_dword,
            "shareFinalRoot": group.share_final_root }),
            &tables["groups"][uid.to_string()],
            "UID group",
        );
    }
    for wave in 1..=6 {
        let rows: Vec<_> = native_tutorial_wave_batches(wave).iter().map(|batch|json!({
            "count": batch.count, "groupUid": batch.group_uid, "positionPolicy": policy_id(batch.position_policy),
            "recipeUid": batch.recipe_uid, "tick": batch.tick })).collect();
        compare(&json!(rows), &tables["batches"][wave.to_string()], "wave batches");
    }
    for cue in NativeTutorialCue::ALL {
        let definition = cue.definition();
        compare(
            &json!({ "durationTicks": definition.duration_ticks, "speaker": definition.speaker.id(),
            "text": definition.text }),
            &tables["cues"][cue.id()],
            "narration definition",
        );
    }
    let fires: Vec<_> = NATIVE_TUTORIAL_FIRES.iter().map(|fire|json!({ "damage": fire.damage,
        "lifetimeTicks": fire.lifetime_ticks, "position": point_row(fire.position), "radius": fire.radius })).collect();
    compare(&json!(fires), &tables["fires"], "authored fire");
    compare(&item_row(&native_tutorial_amulet_item()), &tables["amulet"], "amulet");
    compare(&item_row(&native_tutorial_health_potion_item()), &tables["potion"], "potion");
    assert!(native_tutorial_amulet_identity_matches(&native_tutorial_amulet_item()));
    assert_eq!(native_tutorial_dialogue_ticks(), integer(&tables["dialogueTicks"]));
}
