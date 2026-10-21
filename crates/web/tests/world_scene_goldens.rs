//! Source-ordered world phases and checkpoint continuation at the scene/runtime boundary.

#[path = "../../sim/tests/support/mod.rs"]
mod support;

use std::path::PathBuf;

use serde_json::{Value, json};
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_assets::navigation::decode_navigation;
use solomon_sim::boneyard::arena::{ArenaPhase, ArenaTransition, EntrySide};
use solomon_sim::boneyard::loot::{BoneyardGoodiePhase, BoneyardLootParticipant};
use solomon_sim::boneyard::waves::director::WaveTickContext;
use solomon_sim::boneyard::world::BoneyardPlayerBody;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardMovementInput, BoneyardMovementStep, BoneyardPlayerStatus,
    BoneyardWorld, LoadedBoneyard,
};
use solomon_sim::enemies::model::WebbedState;
use solomon_sim::loot::{
    NativeLootDropKind, NativeLootDropSource, NativeLootDropSpec, NativeLootModifiers,
    NativeOrbKind,
};
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::{HubWizard, Vec2};

fn bounds(value: BoneyardBounds) -> Value {
    json!({ "x": value.x, "y": value.y, "w": value.width, "h": value.height })
}

fn arena(value: ArenaTransition) -> Value {
    json!({ "blendFactor": value.blend_factor, "cameraBounds": bounds(value.camera_bounds),
        "combatBounds": bounds(value.combat_bounds), "fullBounds": bounds(value.full_bounds),
        "entrySide": match value.entry_side { EntrySide::North => "north", EntrySide::South => "south" },
        "phase": match value.phase { ArenaPhase::Open => "open", ArenaPhase::Locking => "locking", ArenaPhase::Sealed => "sealed" },
        "sealTicksRemaining": value.seal_ticks_remaining })
}

fn point(value: Vec2) -> Value {
    json!({ "x": value.x, "y": value.y })
}

fn snapshot(world: &BoneyardWorld, wizard: &HubWizard, movement: &BoneyardMovementStep) -> Value {
    let encounter = world.encounter.as_ref().unwrap();
    let waves = &world.waves.as_ref().unwrap().state;
    json!({
        "wizard": { "position": point(wizard.position), "velocity": point(wizard.velocity),
            "headingIndex": wizard.heading_index, "gaitDegrees": wizard.gait_degrees,
            "walkCyclePrimary": wizard.walk_cycle_primary },
        "lanternPosition": world.lantern_position.map(point), "arenaTransition": world.arena_transition.map(arena),
        "encounter": { "phase": encounter.phase.id(), "position": point(encounter.position),
            "headingDeg": encounter.heading_degrees, "runEventId": encounter.run_event_id,
            "voiceTicksRemaining": encounter.voice_ticks_remaining, "phaseTicksRemaining": encounter.phase_ticks_remaining,
            "rngState": { "indexA": encounter.rng.index_a, "indexB": encounter.rng.index_b, "words": encounter.rng.words.as_slice() },
            "targetPlayerId": encounter.target_player_id },
        "waves": { "phase": waves.phase.id(), "waveOrdinal": waves.wave_ordinal,
            "slumpgutPollCursor": waves.slumpgut_poll_cursor, "populationThreshold": waves.population_threshold,
            "lowPopulationTicks": waves.low_population_ticks, "activeBurstIndex": waves.active_burst_index,
            "activeGroupIndex": waves.active_group_index, "spawnCountdown": waves.spawn_countdown,
            "rngState": { "indexA": waves.rng.index_a, "indexB": waves.rng.index_b, "words": waves.rng.words.as_slice() } },
        "contacts": movement.contacts.iter().map(|contact| json!({ "bodyId": contact.body_id, "staffHostile": contact.staff_hostile })).collect::<Vec<_>>(),
        "enemyWorldFeedback": { "accumulator": world.enemy_world_feedback.accumulator,
            "magnitude": world.enemy_world_feedback.magnitude },
        "loot": { "lastStepTick": world.loot.last_step_tick.map_or(-1, |tick|tick as i64),
            "lastSuccessfulItemLevel": world.loot.last_successful_item_level,
            "nextKeyDropLevel": world.loot.next_key_drop_level,"nextItemId":world.loot.next_item_id,
            "sharedRng": { "indexA":world.loot.shared_rng.index_a,"indexB":world.loot.shared_rng.index_b,"words":world.loot.shared_rng.words.as_slice() },
            "goodies":world.loot.goodies.iter().map(|goodie|json!({"eid":goodie.eid,"id":goodie.id,"rewardSeed":goodie.reward_seed,
                "phase":match goodie.phase { BoneyardGoodiePhase::Intact => 0, BoneyardGoodiePhase::Opening => 1, BoneyardGoodiePhase::Broken => 2 },
                "timer":goodie.timer,"active":goodie.active,"exhausted":goodie.exhausted,"sceneryRegistrationOrdinal":goodie.scenery_registration_ordinal})).collect::<Vec<_>>() },
        "lightCameraBounds": bounds(movement.light_camera_bounds),
    })
}

fn step_ground_loot(
    world: &mut BoneyardWorld,
    wizard: &HubWizard,
    player: BoneyardPlayerStatus<'_>,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    let advanced = [false; 8];
    let participants = [BoneyardLootParticipant {
        advanced_unlocks: &advanced,
        alive: player.alive,
        connected: true,
        heading_index: wizard.heading_index,
        level: 1,
        modifiers: NativeLootModifiers::default(),
        owned_recipe_indexes: &[],
        player_id: player.id,
        position: wizard.position,
    }];
    world.step_ground_loot(&participants, tick, order);
}

fn loaded(catalog: &BoneyardCatalog, index: usize, seed_text: &str) -> LoadedBoneyard {
    let mut seed = [0; 16];
    seed[..4].copy_from_slice(&(index as u32).to_be_bytes());
    let mut loaded = catalog.materialize(seed);
    loaded.seed = seed_text.into();
    loaded
}

fn navigation(loaded: &LoadedBoneyard) -> solomon_sim::boneyard::navigation::NavigationSet {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/assets/navigation")
        .join(format!("{}.json", loaded.source_sha256));
    decode_navigation(&std::fs::read_to_string(path).unwrap(), loaded).unwrap()
}

#[test]
fn movement_contacts_facing_locks_and_native_release_follow_the_real_world_step() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/world-scene-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let index = case["templateIndex"].as_u64().unwrap() as usize;
        let loaded = loaded(&catalog, index, case["seed"].as_str().unwrap());
        let navigation = navigation(&loaded);
        let mut world = BoneyardWorld::new(loaded);
        let mut wizard = world.spawn_wizard();
        let mut order = WorldPainterOrder { next_actor_ordinal: 3, next_transient_ordinal: 0 };
        wizard.primary_cast.channel_active = case["castOwnsFacing"].as_bool().unwrap();
        let dig = world.loaded.scene.solomon_dig.as_ref().unwrap();
        wizard.position = Vec2::new(dig.position.x + 30.0, dig.position.y + 50.0);
        if !case["webbed"].is_null() {
            world.enemies.webbed_players.insert(
                "local".into(),
                WebbedState { severity: 1.25, strength: 12.0, cocoon_health: 0.0, hit_pulse: 0.0 },
            );
        }
        assert_eq!(
            world.enemies.next_native_registration_order,
            case["nativeRegistrationBase"].as_u64().unwrap() as usize
        );
        let alive = case["alive"].as_bool().unwrap();
        let player = BoneyardPlayerStatus {
            id: "local",
            alive,
            eligible: alive,
            collision_enabled: alive,
            movement_scale: 0.8,
        };
        let paused = case["paused"].as_bool().unwrap();
        let records = case["records"].as_array().unwrap();
        let mut record_index = 0;
        let last_tick = records.last().unwrap()["tick"].as_u64().unwrap();
        let mut movement = BoneyardMovementStep {
            movement_active: false,
            contacts: Vec::new(),
            light_camera_bounds: world.camera_bounds(),
        };
        for tick in 0..=last_tick {
            if tick > 0 {
                let hostile_paused = paused || world.encounter.as_ref().unwrap().run_event_id > 0;
                movement = world.step_player_movement(
                    &mut wizard,
                    BoneyardMovementInput {
                        player,
                        movement: Vec2::new(if tick % 200 < 100 { 1.0 } else { -1.0 }, 0.0),
                        force_movement_facing: false,
                        tutorial_movement_paused: false,
                        hostile_scene_paused: hostile_paused,
                    },
                );
                step_ground_loot(&mut world, &wizard, player, tick, &mut order);
                let encounter = world.step_encounter_and_arena(
                    &[BoneyardPlayerBody { status: player, position: wizard.position }],
                    tick,
                    &navigation,
                );
                let players = if alive { vec![wizard.position] } else { Vec::new() };
                let births = world.resolve_spawn_intents(
                    WaveTickContext {
                        tick,
                        bounds: encounter.spawn_bounds,
                        live_enemy_count: 0,
                        live_zombie_count: 0,
                        live_boss_count: 0,
                        players: &players,
                    },
                    encounter.waves_started,
                    hostile_paused,
                    Vec::new(),
                );
                assert!(
                    births.is_empty(),
                    "source scene fixture pauses enemy births after release"
                );
            }
            if records
                .get(record_index)
                .is_some_and(|record| record["tick"].as_u64().unwrap() == tick)
            {
                support::assert_website_json(
                    &snapshot(&world, &wizard, &movement),
                    &records[record_index]["state"],
                    &format!("map {index},scene tick {tick}"),
                    &["encounter.position.x", "encounter.position.y", "encounter.headingDeg"],
                );
                record_index += 1;
            }
        }
        assert_eq!(record_index, records.len());
        assert_eq!(world.ticks, last_tick);
    }
}

#[test]
fn restoring_scene_state_preserves_future_movement_encounter_wave_rng_and_gate_motion() {
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    let loaded = loaded(&catalog, 0, "scene-checkpoint");
    let navigation = navigation(&loaded);
    let mut world = BoneyardWorld::new(loaded.clone());
    let mut wizard = world.spawn_wizard();
    let mut order = WorldPainterOrder { next_actor_ordinal: 3, next_transient_ordinal: 0 };
    let dig = world.loaded.scene.solomon_dig.as_ref().unwrap();
    wizard.position = Vec2::new(dig.position.x + 30.0, dig.position.y + 50.0);
    let player = BoneyardPlayerStatus {
        id: "local",
        alive: true,
        eligible: true,
        collision_enabled: true,
        movement_scale: 1.0,
    };
    for tick in 1..=500 {
        if tick == 450 {
            let eid = world
                .loot
                .goodies
                .first()
                .expect("the retained scene contains native Goodies")
                .eid
                .clone();
            world.loot.activate_goodie(&eid, player.id);
        }
        if tick == 499 {
            let position = Vec2::new(wizard.position.x + 65.0, wizard.position.y);
            let mut orb = NativeLootDropSpec::new(
                NativeLootDropKind::Orb,
                0,
                position,
                NativeLootDropSource::Enemy,
            );
            orb.orb_kind = Some(NativeOrbKind::Mana);
            orb.value = Some(0.7);
            world.loot.spawn_specs(vec![orb], tick, &mut order);
        }
        world.step_player_movement(
            &mut wizard,
            BoneyardMovementInput {
                player,
                movement: Vec2::ZERO,
                force_movement_facing: false,
                tutorial_movement_paused: false,
                hostile_scene_paused: true,
            },
        );
        step_ground_loot(&mut world, &wizard, player, tick, &mut order);
        world.step_encounter_and_arena(
            &[BoneyardPlayerBody { status: player, position: wizard.position }],
            tick,
            &navigation,
        );
    }
    let checkpoint = world.checkpoint();
    let mut mismatched = loaded.clone();
    mismatched.geometry_sha256 = "different geometry".into();
    assert!(BoneyardWorld::restore(mismatched, checkpoint.clone()).is_err());
    let mut wrong_wave = checkpoint.clone();
    wrong_wave.waves.as_mut().unwrap().seed = "different wave seed".into();
    assert!(BoneyardWorld::restore(loaded.clone(), wrong_wave).is_err());
    let mut restored = BoneyardWorld::restore(loaded, checkpoint).unwrap();
    let mut restored_wizard = wizard.clone();
    let mut restored_order = order;
    let mut birth_count = 0;
    for tick in 501..=1800 {
        let movement = Vec2::new(if tick % 150 < 75 { 0.5 } else { -0.5 }, 0.0);
        let input = BoneyardMovementInput {
            player,
            movement,
            force_movement_facing: false,
            tutorial_movement_paused: false,
            hostile_scene_paused: true,
        };
        let first = world.step_player_movement(&mut wizard, input);
        let second = restored.step_player_movement(&mut restored_wizard, input);
        assert_eq!(first, second);
        step_ground_loot(&mut world, &wizard, player, tick, &mut order);
        step_ground_loot(&mut restored, &restored_wizard, player, tick, &mut restored_order);
        let first_encounter = world.step_encounter_and_arena(
            &[BoneyardPlayerBody { status: player, position: wizard.position }],
            tick,
            &navigation,
        );
        let second_encounter = restored.step_encounter_and_arena(
            &[BoneyardPlayerBody { status: player, position: restored_wizard.position }],
            tick,
            &navigation,
        );
        assert_eq!(first_encounter, second_encounter);
        let positions = [wizard.position];
        let context = WaveTickContext {
            tick,
            bounds: first_encounter.spawn_bounds,
            live_enemy_count: world.enemies.actors.len(),
            live_zombie_count: 0,
            live_boss_count: 0,
            players: &positions,
        };
        let first_births =
            world.resolve_spawn_intents(context, first_encounter.waves_started, false, Vec::new());
        let second_births = restored.resolve_spawn_intents(
            context,
            second_encounter.waves_started,
            false,
            Vec::new(),
        );
        assert_eq!(first_births, second_births);
        birth_count += first_births.len();
        assert_eq!(wizard, restored_wizard);
        assert_eq!(world.gate_leaves, restored.gate_leaves);
        assert_eq!(world.dynamic_collision, restored.dynamic_collision);
        assert_eq!(world.encounter, restored.encounter);
        assert_eq!(world.waves, restored.waves);
        assert_eq!(world.enemy_world_feedback, restored.enemy_world_feedback);
        assert_eq!(world.loot, restored.loot);
        assert_eq!(order, restored_order);
    }
    assert!(birth_count > 0, "continuation must advance the actual wave RNG and emit births");
}
