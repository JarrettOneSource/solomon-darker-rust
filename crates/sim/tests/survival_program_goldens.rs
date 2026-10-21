//! Every retained generated survival program, with exact authored recipes and script epochs.

#[path = "support/enemy_snapshots.rs"]
mod enemy_snapshots;
mod support;

use enemy_snapshots::{policy_id, recipe_snapshot};
use serde_json::{Value, json};
use solomon_sim::boneyard::waves::native_survival_programs;
use solomon_sim::boneyard::waves::programs::{
    BossBirth, BossEncounter, SkeletonBossState, SkeletonStep, SpiderState,
};
use support::assert_website_json;

fn skeleton_births(step: &SkeletonStep) -> Value {
    Value::Array(step.births.iter().map(|birth| {
        let mut value = json!({ "enemyToken": birth.enemy_token.id(), "flags": birth.flags.iter().map(|flag| flag.id()).collect::<Vec<_>>() });
        if birth.archer_strafing { value["archerStrafing"] = true.into(); }
        if let Some(recipe) = &birth.authored_recipe { value["authoredRecipe"] = recipe_snapshot(recipe); }
        value
    }).collect())
}

#[test]
fn every_generated_survival_program_and_recipe_matches_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/survival-program-goldens.json"))
            .unwrap();
    assert_eq!(fixture["data"].as_array().unwrap().len(), 12);
    for source in fixture["data"].as_array().unwrap() {
        let sha = source["sourceSha256"].as_str().unwrap();
        let programs = native_survival_programs(sha).unwrap();
        assert_eq!(programs.bosses.len(), 3);
        for (definition, expected) in
            programs.bosses.iter().zip(source["bosses"].as_array().unwrap())
        {
            assert_eq!(
                definition.wave_ordinal,
                expected["definition"]["waveOrdinal"].as_u64().unwrap()
            );
            assert_eq!(
                definition.completion_skeletons,
                expected["definition"]["completionSkeletons"].as_u64().unwrap() as usize
            );
            let recipes = Value::Array(definition.recipes.iter().map(recipe_snapshot).collect());
            assert_website_json(&recipes, &expected["recipes"], sha, &[]);
        }
        for (definition, expected) in
            [(&programs.ironmaw, &source["ironmaw"]), (&programs.foulshaft, &source["foulshaft"])]
        {
            assert_eq!(definition.wave_ordinal, expected["waveOrdinal"].as_u64().unwrap());
            assert_website_json(
                &recipe_snapshot(&definition.recipe),
                &expected["recipe"],
                sha,
                &[],
            );
        }
        assert_website_json(
            &recipe_snapshot(&programs.slumpgut_recipe),
            &source["slumpgutRecipe"],
            sha,
            &[],
        );
        assert_eq!(
            programs.portal_timeline_uid,
            source["portals"]["timelineUid"].as_u64().map(|value| value as usize)
        );
        assert_eq!(programs.portals.len(), source["portals"]["phases"].as_array().unwrap().len());
        for (phase, expected) in
            programs.portals.iter().zip(source["portals"]["phases"].as_array().unwrap())
        {
            let actual = json!({ "frequency": phase.frequency, "maximumHealth": phase.maximum_health,
                "name": phase.name, "recipeUid": phase.recipe_uid, "placementPolicy": policy_id(phase.placement_policy),
                "scriptUid": phase.script_uid, "spawnCount": phase.spawn_count, "startWave": phase.start_wave,
                "triggerUid": phase.trigger_uid });
            assert_website_json(&actual, expected, sha, &[]);
        }
    }

    for case in fixture["bossCases"].as_array().unwrap() {
        let sha = case["sourceSha256"].as_str().unwrap();
        let programs = native_survival_programs(sha).unwrap();
        let definition = programs
            .bosses
            .iter()
            .find(|definition| definition.kind.id() == case["kind"].as_str().unwrap())
            .unwrap();
        let mut state = BossEncounter::new(definition.kind);
        let mut tick = 0;
        for record in case["records"].as_array().unwrap() {
            loop {
                let result =
                    state.step(definition, definition.wave_ordinal, if tick < 450 { 1 } else { 0 });
                if tick == record["tick"].as_u64().unwrap() {
                    let actual = json!({ "kind": state.kind.id(), "phase": state.phase.id(),
                        "skeletonsRemaining": state.skeletons_remaining, "sourceSha256": sha,
                        "ticksRemaining": state.ticks_remaining });
                    assert_website_json(
                        &actual,
                        &record["state"],
                        &format!("{sha} boss tick {tick}"),
                        &[],
                    );
                    let birth = result.birth.map(|birth| match birth {
                        BossBirth::Boss => "boss",
                        BossBirth::Skeleton => "skeleton",
                    });
                    assert_eq!(json!(birth), record["birth"]);
                    assert_eq!(result.release_wave, record["releaseWave"].as_bool().unwrap());
                    tick += 1;
                    break;
                }
                tick += 1;
            }
        }
    }

    for case in fixture["skeletonCases"].as_array().unwrap() {
        let sha = case["sourceSha256"].as_str().unwrap();
        let programs = native_survival_programs(sha).unwrap();
        let mut state = SkeletonBossState::default();
        let mut tick = 0;
        for record in case["records"].as_array().unwrap() {
            loop {
                let wave = if tick == 0 {
                    programs.ironmaw.wave_ordinal
                } else {
                    programs.foulshaft.wave_ordinal
                };
                let result = state.step(
                    &programs.ironmaw,
                    &programs.foulshaft,
                    wave,
                    if tick < 1200 { 1 } else { 0 },
                );
                if tick == record["tick"].as_u64().unwrap() {
                    let actual = json!({ "foulshaftArchersRemaining": state.foulshaft_archers_remaining,
                        "foulshaftPhase": state.foulshaft_phase.id(), "foulshaftTicksRemaining": state.foulshaft_ticks_remaining,
                        "ironmawSpawned": state.ironmaw_spawned, "sourceSha256": sha });
                    assert_website_json(
                        &actual,
                        &record["state"],
                        &format!("{sha} skeleton tick {tick}"),
                        &[],
                    );
                    assert_website_json(&skeleton_births(&result), &record["births"], sha, &[]);
                    assert_eq!(result.release_wave, record["releaseWave"].as_bool().unwrap());
                    tick += 1;
                    break;
                }
                tick += 1;
            }
        }
    }

    for case in fixture["spiderCases"].as_array().unwrap() {
        let sha = case["sourceSha256"].as_str().unwrap();
        let programs = native_survival_programs(sha).unwrap();
        let index = case["definitionIndex"].as_u64().unwrap() as usize;
        let definition = &programs.spiders[index];
        let mut state = SpiderState::default();
        let mut tick = 0;
        for record in case["records"].as_array().unwrap() {
            loop {
                let result = state.step(
                    std::slice::from_ref(definition),
                    definition.start_wave,
                    if tick < 450 { 2 } else { 0 },
                );
                if tick == record["tick"].as_u64().unwrap() {
                    let actual = json!({ "active": state.active, "phaseIndex": state.phase_index,
                        "cursor": state.cursor, "repeatStart": state.repeat_start, "repeatRemaining": state.repeat_remaining,
                        "sleepTicksRemaining": state.sleep_ticks_remaining, "timelinePaused": state.timeline_paused,
                        "positionPolicy": policy_id(state.position_policy) });
                    assert_website_json(
                        &actual,
                        &record["state"],
                        &format!("{sha} spider tick {tick}"),
                        &[],
                    );
                    let births = json!(result.births.iter().map(|birth| json!({ "flags": birth.flags.iter().map(|flag| flag.id()).collect::<Vec<_>>(), "positionPolicy": policy_id(birth.position_policy) })).collect::<Vec<_>>());
                    assert_website_json(&births, &record["births"], sha, &[]);
                    assert_eq!(result.advance_wave, record["advanceWave"].as_bool().unwrap());
                    tick += 1;
                    break;
                }
                tick += 1;
            }
        }
    }
}
