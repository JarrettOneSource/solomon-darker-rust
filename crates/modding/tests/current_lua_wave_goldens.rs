//! Current Lua next-wave commands use the existing Session and native wave director.

mod common;

use serde_json::{Value, json};
use solomon_sim::boneyard::tutorial::NativeTutorialState;
use solomon_sim::boneyard::waves::director::{WaveDirector, WavePhase};
use solomon_sim::boneyard::waves::programs::{BossPhase, FoulshaftPhase};
use solomon_sim::boneyard::waves::{WaveDef, WaveGroup, WaveGroupEntry};
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};
use solomon_sim::enemies::EnemyToken;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::modding::LuaValue;
use solomon_sim::modding::console::{
    WebLuaCommand, apply_web_lua_commands, create_web_lua_frame_state,
};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

const SCHEDULE: [WaveDef; 2] = [
    WaveDef {
        groups: &[WaveGroup {
            entries: &[WaveGroupEntry { enemy: EnemyToken::Skeleton, flags: &[] }],
        }],
        max_enemies: 40,
        next: &[1],
        spawn: 3,
        spawn_delay: [0, 0],
        wave_delay: [0, 0],
        zombie_wave: false,
    },
    WaveDef {
        groups: &[WaveGroup {
            entries: &[WaveGroupEntry { enemy: EnemyToken::Zombie, flags: &[] }],
        }],
        max_enemies: 40,
        next: &[-1],
        spawn: 2,
        spawn_delay: [0, 0],
        wave_delay: [0, 0],
        zombie_wave: false,
    },
];

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/current-lua-wave-goldens.json"
    ))
    .unwrap()
}

fn fresh_game(fixture: &Value) -> GameSession {
    let mut game = college_game();
    let loaded = &fixture["loaded"];
    game.install_boneyard(LoadedBoneyard {
        choice_id: loaded["choice"]["id"].as_str().unwrap().into(),
        choice_source: BoneyardSource::Mod,
        mod_id: loaded["choice"]["modId"].as_str().map(str::to_owned),
        mod_name: loaded["choice"]["modName"].as_str().map(str::to_owned),
        name: loaded["choice"]["name"].as_str().unwrap().into(),
        run_id: loaded["runId"].as_str().unwrap().into(),
        seed: loaded["seed"].as_str().unwrap().into(),
        source_sha256: loaded["sourceSha256"].as_str().unwrap().into(),
        geometry_sha256: loaded["geometrySha256"].as_str().unwrap().into(),
        scene: solomon_assets::boneyard::decode_scene(&loaded["scene"].to_string()).unwrap().into(),
    })
    .unwrap();
    let mut waves =
        WaveDirector::with_schedule("report58", &SCHEDULE, fixture["sourceSha256"].as_str());
    waves.state.phase = WavePhase::OpeningThreshold;
    game.boneyard.as_mut().unwrap().waves = Some(waves);
    game
}

fn college_game() -> GameSession {
    GameSession::new(
        "player-1".into(),
        NewGameSelection {
            name: "Tester".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    )
}

#[test]
fn current_console_frame_exposes_actual_next_wave_availability() {
    let fixture = fixture();
    let game = fresh_game(&fixture);
    let frame = create_web_lua_frame_state(&game, Some("player-1"));
    let expected =
        fixture["cases"][0]["initial"]["frame"]["spawn_next_available"].as_bool().unwrap();
    assert_eq!(
        frame.waves.as_ref().and_then(|waves| waves.get("spawn_next_available")),
        Some(&LuaValue::Boolean(expected))
    );
}

fn snapshot(game: &GameSession) -> Value {
    let waves = game.boneyard.as_ref().and_then(|world| world.waves.as_ref()).map(|waves| {
        let state = &waves.state;
        json!({
            "phase": state.phase.id(), "scheduleIndex": state.schedule_index,
            "waveOrdinal": state.wave_ordinal, "waveEventId": state.wave_event_id,
            "pendingSpawnBudget": state.pending_spawn_budget,
            "spawnCountdown": state.spawn_countdown, "spawnDelayTicks": state.spawn_delay_ticks,
            "interwaveDelayTicks": state.interwave_delay_ticks, "nextScheduleIndex": state.next_schedule_index,
            "lullThreshold": state.lull_threshold, "lowPopulationTicks": state.low_population_ticks,
            "populationThreshold": state.population_threshold, "activeBurstIndex": state.active_burst_index,
            "activeGroupIndex": state.active_group_index, "activeGroupMemberIndex": state.active_group_member_index,
            "burstStarted": state.burst_started, "burstSpawnRemaining": state.burst_spawn_remaining,
            "burstSpreadTicksRemaining": state.burst_spread_ticks_remaining,
            "rngState": {"indexA": state.rng.index_a, "indexB": state.rng.index_b, "words": &state.rng.words[..]},
        })
    });
    let frame = create_web_lua_frame_state(game, Some("player-1"));
    json!({
        "waves": waves,
        "frame": frame.waves,
        "gameRng": {"indexA": game.gameplay_rng.index_a, "indexB": game.gameplay_rng.index_b, "words": &game.gameplay_rng.words[..]},
        "tick": game.ticks,
        "enemyCount": game.boneyard.as_ref().map_or(0, |world| world.enemies.actors.len()),
    })
}

#[test]
fn current_lua_wave_commands_preserve_holds_signed_edges_and_request_generations() {
    let fixture = fixture();
    for expected in fixture["cases"].as_array().unwrap() {
        let mode = expected["mode"].as_str().unwrap();
        let mut game = fresh_game(&fixture);
        let waves = game.boneyard.as_mut().unwrap().waves.as_mut().unwrap();
        if ["wave-threshold", "wave-lull-delay", "wave-lull", "interwave", "preselected"]
            .contains(&mode)
        {
            waves.state.phase = match mode {
                "wave-threshold" => WavePhase::WaveThreshold,
                "wave-lull-delay" => WavePhase::WaveLullDelay,
                "wave-lull" => WavePhase::WaveLull,
                _ => WavePhase::Interwave,
            };
            waves.state.schedule_index = 1;
            waves.state.wave_ordinal = 9;
            waves.state.wave_event_id = 9;
            waves.state.interwave_delay_ticks = 77;
            if mode == "preselected" {
                waves.state.next_schedule_index = Some(0);
            }
        }
        match mode {
            "dormant" => waves.state.phase = WavePhase::Dormant,
            "opening" => waves.state.phase = WavePhase::Opening,
            "spawning" => waves.state.phase = WavePhase::Spawning,
            "pending" => waves.state.pending_spawn_budget = 1,
            "portal-hold" => waves.state.portal_timeline_paused = true,
            "spider-hold" => waves.state.spider_state.timeline_paused = true,
            "heart-hold" => waves.state.boss_encounters[0].phase = BossPhase::BossWait,
            "faculty-hold" => waves.state.boss_encounters[1].phase = BossPhase::BossWait,
            "discorporeal-hold" => waves.state.boss_encounters[2].phase = BossPhase::BossWait,
            "skeleton-reinforcements" => {
                waves.state.skeleton_bosses.as_mut().unwrap().foulshaft_phase =
                    FoulshaftPhase::Reinforcements
            }
            "skeleton-boss-wait" => {
                waves.state.skeleton_bosses.as_mut().unwrap().foulshaft_phase =
                    FoulshaftPhase::BossWait
            }
            _ => {}
        }
        match mode {
            "barrier" => {
                let _ = game.grant_experience("player-1", 1000.0);
            }
            "tutorial" => {
                let world = game.boneyard.as_mut().unwrap();
                world.tutorial = Some(NativeTutorialState::new(
                    world.loaded.scene.spawn.position,
                    0,
                    &world.loaded.seed,
                ));
            }
            "game-over" => game.run.phase = GameRunPhase::GameOver,
            "hub" => game = college_game(),
            _ => {}
        }
        common::assert_json_values(
            &snapshot(&game),
            &expected["initial"],
            &format!("{mode}.initial"),
        );
        let commands: Vec<WebLuaCommand> =
            serde_json::from_value(expected["commands"].clone()).unwrap();
        let applied = apply_web_lua_commands(&mut game, &commands, None);
        assert!(applied.enemy_spawn_intents.is_empty(), "{mode} leaves actual world actors intact");
        common::assert_json_values(
            &json!({"enemySpawnIntents": [], "nextRunSeed": applied.next_run_seed}),
            &expected["output"],
            mode,
        );
        common::assert_json_values(&snapshot(&game), &expected["expected"], mode);
    }
}
