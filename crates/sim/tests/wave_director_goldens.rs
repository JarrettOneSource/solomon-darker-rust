//! Complete wave execution through the public director/population boundary.

#[path = "support/enemy_snapshots.rs"]
mod enemy_snapshots;
mod support;

use enemy_snapshots::{policy_id, recipe_snapshot};
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::waves::director::{
    PortalScriptPhase, SlumpgutPhase, WaveDirector, WavePhase, WaveTickContext,
};
use solomon_sim::boneyard::waves::programs::{BossKind, SkeletonBossState};
use solomon_sim::enemies::{EnemySpawnIntent, SpawnLocationPolicy};
use support::assert_website_json;

fn director_snapshot(director: &WaveDirector) -> Value {
    let sha = director.programs.as_ref().map(|programs| programs.source_sha256.as_str());
    let bosses: Vec<_> = director.state.boss_encounters.iter().map(|boss| json!({
        "kind": boss.kind.id(), "phase": boss.phase.id(), "skeletonsRemaining": boss.skeletons_remaining,
        "sourceSha256": sha, "ticksRemaining": boss.ticks_remaining,
    })).collect();
    let skeletons = director.state.skeleton_bosses.as_ref().map(|state| json!({
        "foulshaftArchersRemaining": state.foulshaft_archers_remaining,
        "foulshaftPhase": state.foulshaft_phase.id(), "foulshaftTicksRemaining": state.foulshaft_ticks_remaining,
        "ironmawSpawned": state.ironmaw_spawned, "sourceSha256": sha,
    }));
    let spider = &director.state.spider_state;
    json!({
        "activeBurstIndex": director.state.active_burst_index, "activeGroupIndex": director.state.active_group_index,
        "activeGroupMemberIndex": director.state.active_group_member_index, "burstStarted": director.state.burst_started,
        "burstSpawnRemaining": director.state.burst_spawn_remaining, "burstSpreadTicksRemaining": director.state.burst_spread_ticks_remaining,
        "bossEncounters": bosses, "interwaveDelayTicks": director.state.interwave_delay_ticks,
        "lullThreshold": director.state.lull_threshold, "lowPopulationTicks": director.state.low_population_ticks,
        "nextSpawnIntentId": director.state.next_spawn_intent_id, "nextScheduleIndex": director.state.next_schedule_index,
        "openingReleaseThreshold": director.opening_release_threshold, "pendingSpawnBudget": director.state.pending_spawn_budget,
        "phase": director.state.phase.id(), "portalPhaseIndex": director.state.portal_phase_index,
        "portalScriptPhase": director.state.portal_script_phase.id(), "portalSpawnRemaining": director.state.portal_spawn_remaining,
        "portalTicksRemaining": director.state.portal_ticks_remaining, "portalTimelinePaused": director.state.portal_timeline_paused,
        "populationThreshold": director.state.population_threshold,
        "rngState": { "indexA": director.state.rng.index_a, "indexB": director.state.rng.index_b, "words": director.state.rng.words.as_slice() },
        "scheduleIndex": director.state.schedule_index, "skeletonBosses": skeletons,
        "slumpgutPhase": director.state.slumpgut_phase.id(), "slumpgutPollCursor": director.state.slumpgut_poll_cursor,
        "slumpgutRecipeUid": director.state.slumpgut_recipe_uid, "slumpgutTicksRemaining": director.state.slumpgut_ticks_remaining,
        "spawnCountdown": director.state.spawn_countdown, "spawnDelayTicks": director.state.spawn_delay_ticks,
        "spiderState": { "active": spider.active, "phaseIndex": spider.phase_index, "cursor": spider.cursor,
            "repeatStart": spider.repeat_start, "repeatRemaining": spider.repeat_remaining,
            "sleepTicksRemaining": spider.sleep_ticks_remaining, "timelinePaused": spider.timeline_paused,
            "positionPolicy": policy_id(spider.position_policy) },
        "waveEventId": director.state.wave_event_id, "waveOrdinal": director.state.wave_ordinal,
    })
}

fn spawn_snapshot(spawn: &EnemySpawnIntent) -> Value {
    let location = match spawn.location_policy {
        SpawnLocationPolicy::Anywhere => "anywhere",
        SpawnLocationPolicy::NearPlayer => "near-player",
    };
    json!({
        "enemyToken": spawn.enemy_token.id(), "flags": spawn.flags.iter().map(|flag| flag.id()).collect::<Vec<_>>(),
        "id": spawn.id, "locationPolicy": location, "nativeTypeId": spawn.native_type_id,
        "position": { "x": spawn.position.x, "y": spawn.position.y }, "positionPolicy": policy_id(spawn.position_policy),
        "spawnTick": spawn.spawn_tick, "waveOrdinal": spawn.wave_ordinal,
        "authoredRecipe": spawn.authored_recipe.as_ref().map(recipe_snapshot), "flanking": spawn.flanking,
        "pathfindingMode": spawn.pathfinding_mode, "archerStrafing": spawn.archer_strafing,
        "placementRadius": spawn.placement_radius, "navigationClearance": spawn.navigation_clearance,
        "reachabilityRadius": spawn.reachability_radius, "zombieBodyType": spawn.zombie_body_type,
        "enableDiscorporealHealthGates": spawn.enable_discorporeal_health_gates,
    })
}

fn make_director(setup: &Value) -> WaveDirector {
    let mut director =
        WaveDirector::new(setup["seed"].as_str().unwrap(), setup["sourceSha256"].as_str());
    director.start();
    let kind = setup["kind"].as_str().unwrap();
    if kind == "ordinary" {
        return director;
    }
    let boss_kind = match setup["bossKind"].as_str() {
        Some("heartmonger") => Some(BossKind::Heartmonger),
        Some("faculty") => Some(BossKind::Faculty),
        Some("discorporeal") => Some(BossKind::Discorporeal),
        _ => None,
    };
    director.state.boss_encounters.retain(|boss| Some(boss.kind) == boss_kind);
    director.state.skeleton_bosses =
        if kind == "skeleton" { Some(SkeletonBossState::default()) } else { None };
    director.state.portal_script_phase =
        if kind == "portal" { PortalScriptPhase::Idle } else { PortalScriptPhase::Retired };
    if kind == "portal" {
        director.state.portal_phase_index = setup["phaseIndex"].as_u64().unwrap() as usize;
    }
    let programs = director.programs.as_mut().unwrap();
    if kind == "spider" {
        let index = setup["definitionIndex"].as_u64().unwrap() as usize;
        programs.spiders = vec![programs.spiders[index].clone()];
    } else {
        programs.spiders.clear();
    }
    director.state.slumpgut_phase =
        if kind == "slumpgut" { SlumpgutPhase::Eligible } else { SlumpgutPhase::Retired };
    director.state.phase = WavePhase::WaveThreshold;
    director.state.population_threshold = 0;
    director.state.lull_threshold = 0;
    director.state.active_burst_index = None;
    director.state.active_group_index = None;
    director.state.burst_started = false;
    director.state.burst_spawn_remaining = 0;
    director.state.burst_spread_ticks_remaining = 0.0;
    director.state.pending_spawn_budget = 0;
    director.state.spawn_countdown = 0.0;
    director.state.spawn_delay_ticks = 0;
    director.state.wave_ordinal = setup["waveOrdinal"].as_u64().unwrap();
    director
}

#[test]
fn native_wave_execution_and_all_generated_script_births_match_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-wave-director-goldens.json"))
            .unwrap();
    for (case_index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let setup = &case["setup"];
        let kind = setup["kind"].as_str().unwrap();
        let mut director = make_director(setup);
        assert_website_json(
            &director_snapshot(&director),
            &case["initial"],
            &format!("case {case_index} initial"),
            &[],
        );
        let mut record_index = 0;
        let positions = if setup["noPlayers"].as_bool().unwrap_or(false) {
            Vec::new()
        } else {
            vec![Vec2::new(700.0, 800.0), Vec2::new(900.0, 1000.0)]
        };
        for tick in 0..=case["lastTick"].as_u64().unwrap() {
            let mut context = WaveTickContext {
                bounds: BoneyardBounds::new(0.0, 0.0, 2000.0, 3000.0),
                live_boss_count: 0,
                live_enemy_count: 0,
                live_zombie_count: 0,
                players: &positions,
                tick,
            };
            match kind {
                "ordinary" => context.live_enemy_count = if tick % 1400 < 600 { 20 } else { 0 },
                "boss" | "portal" => context.live_boss_count = usize::from(tick < 450),
                "skeleton" => {
                    director.state.wave_ordinal = if tick == 0 {
                        setup["waveOrdinal"].as_u64().unwrap()
                    } else {
                        setup["foulshaftWaveOrdinal"].as_u64().unwrap()
                    };
                    context.live_boss_count = usize::from(tick < 1200);
                }
                "spider" => context.live_enemy_count = if tick < 450 { 2 } else { 0 },
                "slumpgut" => {
                    context.live_enemy_count = 76;
                    context.live_zombie_count = 76;
                }
                kind => panic!("unrecognized director oracle kind {kind}"),
            }
            let births = director.step(context);
            let records = case["records"].as_array().unwrap();
            if records
                .get(record_index)
                .is_some_and(|record| record["tick"].as_u64().unwrap() == tick)
            {
                let record = &records[record_index];
                let path = format!("case {case_index} {kind} tick {tick}");
                assert_website_json(&director_snapshot(&director), &record["state"], &path, &[]);
                let expected = record["spawnIntents"].as_array().unwrap();
                assert_eq!(births.len(), expected.len(), "{path} birth count");
                for (actual, expected) in births.iter().zip(expected) {
                    let actual = spawn_snapshot(actual);
                    let expected = expected.as_object().unwrap();
                    let actual: serde_json::Map<String, Value> =
                        expected.keys().map(|key| (key.clone(), actual[key].clone())).collect();
                    assert_website_json(
                        &Value::Object(actual),
                        &Value::Object(expected.clone()),
                        &path,
                        &[],
                    );
                }
                record_index += 1;
            } else {
                assert!(
                    births.is_empty(),
                    "case {case_index} tick {tick} emitted an unrecorded birth"
                );
            }
        }
        assert_eq!(record_index, case["records"].as_array().unwrap().len());
    }
}

#[test]
fn restoring_a_wave_checkpoint_preserves_future_births_and_every_random_word() {
    let source = "2118053783606f5ef9dc848671d6eecd8e87aa0a3610c8c2119f08452e15a22f";
    let positions = [Vec2::new(700.0, 800.0), Vec2::new(900.0, 1000.0)];
    let context = |tick| WaveTickContext {
        bounds: BoneyardBounds::new(0.0, 0.0, 2000.0, 3000.0),
        live_boss_count: 0,
        live_enemy_count: if tick % 1400 < 600 { 20 } else { 0 },
        live_zombie_count: 0,
        players: &positions,
        tick,
    };
    for saved_tick in [0, 400, 550, 2100, 9500, 14000] {
        let mut original = WaveDirector::new("checkpoint-continuation", Some(source));
        original.start();
        for tick in 0..saved_tick {
            original.step(context(tick));
        }
        let checkpoint = original.checkpoint();
        let mut restored = WaveDirector::restore(checkpoint.clone());
        assert_eq!(restored.checkpoint(), checkpoint);
        for tick in saved_tick..saved_tick + 1500 {
            assert_eq!(
                original.step(context(tick)),
                restored.step(context(tick)),
                "births after checkpoint at tick {saved_tick}, continuation tick {tick}"
            );
            assert_eq!(
                original.checkpoint(),
                restored.checkpoint(),
                "state after checkpoint at tick {saved_tick}, continuation tick {tick}"
            );
        }
    }
}

#[test]
fn applying_a_retained_historical_graph_replaces_the_same_active_schedule_section() {
    let positions = [Vec2::new(700.0, 800.0)];
    let context = |tick| WaveTickContext {
        bounds: BoneyardBounds::new(0.0, 0.0, 2000.0, 3000.0),
        live_boss_count: 0,
        live_enemy_count: 0,
        live_zombie_count: 0,
        players: &positions,
        tick,
    };
    let seed = "historical-graph-continuation";
    let mut checkpoint = WaveDirector::new(seed, None).checkpoint();
    checkpoint.active_bursts = None;
    checkpoint.compiled_schedule = None;
    checkpoint.opening_bursts = None;
    checkpoint.opening_release_threshold = None;
    let mut historical = WaveDirector::restore(checkpoint);
    historical.start();
    let mut current = WaveDirector::new(seed, None);
    current.start();
    let mut saved_tick = None;
    for tick in 0..10_000 {
        historical.step(context(tick));
        current.step(context(tick));
        if historical.state.active_section == current.state.active_section
            && historical.active_bursts != current.active_bursts
        {
            saved_tick = Some(tick);
            break;
        }
    }
    let saved_tick = saved_tick.expect("both compiler generations enter the first wave");
    let checkpoint = historical.checkpoint();
    current.apply_checkpoint(checkpoint.clone()).unwrap();
    let mut restored = WaveDirector::restore(checkpoint.clone());
    assert_eq!(current.checkpoint(), checkpoint);
    assert_eq!(restored.checkpoint(), checkpoint);
    for tick in saved_tick + 1..saved_tick + 2_000 {
        let births = historical.step(context(tick));
        assert_eq!(current.step(context(tick)), births, "reapplied tick {tick}");
        assert_eq!(restored.step(context(tick)), births, "restored tick {tick}");
        assert_eq!(current.checkpoint(), historical.checkpoint(), "reapplied tick {tick}");
        assert_eq!(restored.checkpoint(), historical.checkpoint(), "restored tick {tick}");
    }
}

#[cfg(feature = "persistence")]
#[test]
fn borrowed_wave_snapshot_and_owned_checkpoint_retain_the_same_tables() {
    let mut director = WaveDirector::new("borrowed-wave-tables", None);
    director.start();
    let owned = serde_json::to_value(director.checkpoint()).unwrap();
    let borrowed = serde_json::to_value(director.snapshot()).unwrap();
    assert_eq!(borrowed, owned);
    let decoded = serde_json::from_value(borrowed).unwrap();
    assert_eq!(WaveDirector::restore(decoded), director);
}
