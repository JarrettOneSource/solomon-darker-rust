//! Run lifecycle checks at the public session boundary, using unmodified Website output.

use serde_json::{Value, json};
use solomon_sim::game_run::GameRunLifecycle;
use solomon_sim::game_run::presentation::{
    game_over_audio_events, game_over_presentation, solomon_riff_presentation,
};

fn strings(value: &Value) -> Vec<String> {
    value.as_array().unwrap().iter().map(|entry| entry.as_str().unwrap().into()).collect()
}

#[test]
fn stopped_world_clock_matches_current_source_at_fractional_frames() {
    use solomon_sim::game_run::{GameRunPhase, game_run_world_tick};
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/game-run-world-clock-goldens.json"))
            .unwrap();
    for row in fixture["rows"].as_array().unwrap() {
        let mut run = GameRunLifecycle::new();
        run.phase = match row["phase"].as_str().unwrap() {
            "hub" => GameRunPhase::Hub,
            "active" => GameRunPhase::Active,
            "game-over" => GameRunPhase::GameOver,
            "loadout" => GameRunPhase::Loadout,
            _ => unreachable!(),
        };
        run.game_over_ticks = row["gameOverTicks"].as_u64().unwrap();
        assert_eq!(
            game_run_world_tick(row["tick"].as_f64().unwrap(), &run),
            row["value"].as_f64().unwrap(),
            "{row}"
        );
    }
}

#[test]
fn terminal_run_fades_riff_animation_and_sound_edges_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/game-run-goldens.json")).unwrap();
    for record in fixture["presentations"].as_array().unwrap() {
        let kind = match record["exitKind"].as_str() {
            Some("input") => Some(solomon_sim::game_run::GameOverExitKind::Input),
            Some("automatic") => Some(solomon_sim::game_run::GameOverExitKind::Automatic),
            _ => None,
        };
        let actual = game_over_presentation(
            record["tick"].as_u64().unwrap(),
            record["exitTick"].as_u64(),
            kind,
        );
        let value = &record["value"];
        assert_eq!(actual.accepts_input, value["acceptsInput"].as_bool().unwrap());
        for (name, actual) in [
            ("entryFadeAlpha", actual.entry_fade_alpha),
            ("exitFadeAlpha", actual.exit_fade_alpha),
            ("promptAlpha", actual.prompt_alpha),
            ("titleAlpha", actual.title_alpha),
        ] {
            assert_eq!(actual, value[name].as_f64().unwrap(), "{name}: {record}");
        }
    }
    for record in fixture["riff"].as_array().unwrap() {
        let actual = solomon_riff_presentation(record["tick"].as_u64().unwrap());
        let value = &record["value"];
        assert_eq!(actual.frame_record, value["frameRecord"].as_u64().map(|value| value as usize));
        assert_eq!(actual.frame_record.is_some(), value["visible"].as_bool().unwrap());
        assert_eq!(
            actual.offset.x,
            value["xOffset"].as_f64().unwrap(),
            "riff tick {}",
            record["tick"]
        );
        assert_eq!(
            actual.offset.y,
            value["yOffset"].as_f64().unwrap(),
            "riff tick {}",
            record["tick"]
        );
    }
    let mut run = GameRunLifecycle::new();
    run.start("game-over-audio".into(), &["local".into()]);
    let mut events = Vec::new();
    for tick in 0..1202 {
        let previous = run.clone();
        run.step(&[]);
        let cues = game_over_audio_events(&previous, &run);
        if !cues.is_empty() {
            events.push(json!({"tick":tick,"cues":cues.iter().map(|cue|cue.definition().id).collect::<Vec<_>>()}));
        }
    }
    assert_eq!(json!(events), fixture["audioEvents"]);
}

#[test]
fn game_over_gates_and_post_run_loadout_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/game-run-goldens.json")).unwrap();
    for scenario in fixture["scenarios"].as_array().unwrap() {
        let mut run = GameRunLifecycle::new();
        for (index, record) in scenario["records"].as_array().unwrap().iter().enumerate() {
            let action = &record["action"];
            let accepted = match action["kind"].as_str().unwrap() {
                "start" => {
                    run.start(
                        action["runId"].as_str().unwrap().into(),
                        &strings(&action["players"]),
                    );
                    true
                }
                "synchronize" => {
                    run.synchronize_participants(&strings(&action["players"]));
                    true
                }
                "step" => {
                    let alive = strings(&action["alive"]);
                    for _ in 0..action["count"].as_u64().unwrap() {
                        run.step(&alive);
                    }
                    true
                }
                "continue" => run.continue_game_over(
                    action["runId"].as_str().unwrap(),
                    action["eventId"].as_u64().unwrap(),
                ),
                "confirm" => run.confirm_post_run_loadout(action["player"].as_str().unwrap()),
                "college-intro" => run.continue_post_run_to_college_intro(),
                kind => panic!("unrecognized lifecycle oracle action {kind}"),
            };
            assert_eq!(
                accepted,
                record["accepted"].as_bool().unwrap(),
                "{} record {index}",
                scenario["name"]
            );
            let actual = json!({
                "eligiblePlayerIds": run.eligible_player_ids,
                "gameOverEventId": run.game_over_event_id,
                "gameOverExitKind": run.game_over_exit_kind.map(|kind| kind.id()),
                "gameOverExitTicks": run.game_over_exit_ticks,
                "gameOverTicks": run.game_over_ticks,
                "lastCompletedRunId": run.last_completed_run_id,
                "loadoutReadyPlayerIds": run.loadout_ready_player_ids,
                "nextGameOverEventId": run.next_game_over_event_id,
                "phase": run.phase.id(),
                "runId": run.run_id,
            });
            assert_eq!(actual, record["state"], "{} record {index}", scenario["name"]);
        }
    }
}
