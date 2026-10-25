use super::*;
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::modding::{LuaValue, identity::GameContentManifest};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionInput, GameSessionOptions};
use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

fn loaded(id: &str) -> LoadedBoneyard {
    LoadedBoneyard {
        choice_id: "default-random".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: "Random Boneyard".into(),
        run_id: id.into(),
        seed: "archive-fixture".into(),
        source_sha256: "c".repeat(64),
        geometry_sha256: "d".repeat(64),
        scene: BoneyardScene {
            name: "Archive fixture".into(),
            environment_mode: 2,
            bounds: BoneyardBounds::new(0.0, 0.0, 500.0, 500.0),
            spawn: BoneyardSpawn { position: Vec2::new(250.0, 250.0), facing_degrees: 180.0 },
            objects: Vec::new(),
            sprites: Vec::new(),
            roads: Vec::new(),
            fences: Vec::new(),
            terrain: Vec::new(),
            solomon_dig: None,
        }
        .into(),
    }
}
fn base_state() -> GameSession {
    let mut state = GameSession::new_participants(
        [("alpha", "Alpha"), ("beta", "Beta")]
            .into_iter()
            .map(|(id, name)| {
                (
                    id.into(),
                    NewGameSelection {
                        name: name.into(),
                        element: WizardElement::Fire,
                        discipline: WizardDiscipline::Body,
                    },
                )
            })
            .collect(),
        GameSessionOptions::default(),
    )
    .unwrap();
    state.install_boneyard(loaded("first")).unwrap();
    state
}
fn snapshot(base: &Arc<ArchivedState>, spec: &Value) -> Arc<ArchivedState> {
    let mut checkpoint = base.checkpoint.clone();
    checkpoint.ticks = spec["tick"].as_u64().unwrap();
    checkpoint.run.phase = match spec["phase"].as_str() {
        Some("hub") => GameRunPhase::Hub,
        Some("game-over") => GameRunPhase::GameOver,
        _ => GameRunPhase::Active,
    };
    if spec["kind"] == "hub" {
        checkpoint.boneyard = None;
    }
    for (index, id) in ["alpha", "beta"].into_iter().enumerate() {
        checkpoint.participants.get_mut(id).unwrap().player.combat.life_state =
            match spec["lives"][index].as_str() {
                Some("lethal-pending") => PlayerLifeState::LethalPending,
                Some("dying") => PlayerLifeState::Dying,
                _ => PlayerLifeState::Alive,
            };
    }
    Arc::new(ArchivedState { checkpoint, inventory_dye_sessions: BTreeMap::new() })
}
fn now() -> String {
    "2026-10-21T00:00:00.000Z".into()
}
fn recorder() -> RunArchiveRecorder {
    RunArchiveRecorder::with_clock(
        "archive-owner".into(),
        "b".repeat(40),
        GameContentManifest { manifest_sha256: "a".repeat(64), mods: Vec::new() },
        now,
    )
}
fn checkpoint_projection(row: &RunArchiveCheckpoint) -> Value {
    json!({"tick":row.state.checkpoint.ticks,"movement":row.inputs.get("alpha").map(|input| input.movement),"tickMs":row.tick_ms,"spawns":row.enemy_spawn_intents.len()})
}
fn projection(archive: &RunArchive) -> Value {
    json!({"runId":archive.run_id,"sessionId":archive.session_id,"revision":archive.revision,
        "startedAtUtc":archive.started_at_utc,"endedAtUtc":archive.ended_at_utc,"endReason":archive.end_reason,
        "lastAlive":archive.last_alive.as_ref().map(checkpoint_projection),
        "byPlayer":archive.last_alive_by_player.iter().map(|(id, row)| json!([id,checkpoint_projection(row)])).collect::<Vec<_>>(),
        "worst":checkpoint_projection(&archive.worst_tick),"finalTick":archive.final_state.checkpoint.ticks,
        "players":super::encoding::archive_players(archive),"performance":archive.performance})
}

#[test]
fn source_run_archive_living_retention_lifecycle_and_performance_windows_are_exact() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../sim/tests/fixtures/website/run-archive-goldens.json"
    ))
    .unwrap();
    let base = ArchivedState::capture(&base_state());
    let mut recorder = recorder();
    let mut archives = Vec::new();
    for operation in fixture["operations"].as_array().unwrap() {
        match operation["op"].as_str().unwrap() {
            "checkpoint" => {
                // The recorder's public capture consumes this actual canonical
                // Session. Fixture edits are input states, never gameplay results.
                let mut state = base_state();
                let recorded = snapshot(&base, &operation["state"]);
                state.ticks = recorded.checkpoint.ticks;
                for id in &state.participant_order {
                    state.participants.get_mut(id).unwrap().player.combat.life_state =
                        recorded.checkpoint.participants[id].player.combat.life_state;
                }
                recorder.checkpoint(&state, &loaded(operation["run"].as_str().unwrap()));
            }
            "observe" => {
                let before = RunArchiveCheckpoint::new(
                    snapshot(&base, &operation["before"]),
                    BTreeMap::from([(
                        "alpha".into(),
                        GameSessionInput {
                            movement: Vec2::new(operation["input"].as_f64().unwrap(), 0.0),
                            ..Default::default()
                        },
                    )]),
                    Vec::new(),
                    operation["tickMs"].as_f64().unwrap(),
                );
                if let Some(archive) = recorder.observe(
                    before,
                    snapshot(&base, &operation["after"]),
                    &loaded(operation["run"].as_str().unwrap()),
                    operation["behindMs"].as_f64().unwrap(),
                ) {
                    archives.push(archive);
                }
            }
            "retain" => archives.extend(recorder.retain(
                &serde_json::from_value::<Vec<String>>(operation["active"].clone()).unwrap(),
            )),
            "close" => archives.extend(
                recorder.close(serde_json::from_value(operation["reason"].clone()).unwrap()),
            ),
            _ => unreachable!(),
        }
    }
    let actual: LuaValue =
        serde_json::from_value(json!(archives.iter().map(projection).collect::<Vec<_>>())).unwrap();
    let expected: LuaValue = serde_json::from_value(fixture["expected"].clone()).unwrap();
    assert_eq!(actual, expected);
    for tick in 0..6001 {
        let before = RunArchiveCheckpoint::new(
            snapshot(&base, &json!({"tick":tick})),
            BTreeMap::new(),
            Vec::new(),
            if tick % 3 == 0 { 10.5 } else { 2.0 },
        );
        assert!(
            recorder
                .observe(
                    before,
                    snapshot(&base, &json!({"tick":tick+1})),
                    &loaded("window-retention"),
                    (tick % 9) as f64
                )
                .is_none()
        );
    }
    let archive = recorder.close(RunArchiveEndReason::HostClosed).remove(0);
    let actual: LuaValue = serde_json::from_value(projection(&archive)).unwrap();
    let expected: LuaValue = serde_json::from_value(fixture["windows"].clone()).unwrap();
    assert_eq!(actual, expected);
}

fn directory() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "solomon-run-archive-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()
    ))
}
fn sample_archive() -> RunArchive {
    let state = base_state();
    let mut recorder = recorder();
    recorder.checkpoint(&state, &loaded("first"));
    recorder.close(RunArchiveEndReason::HostClosed).remove(0)
}
#[test]
fn native_archives_roundtrip_shared_checkpoints_publish_privately_and_never_overwrite() {
    let directory = directory();
    let archive = sample_archive();
    let id = "00000000-0000-4000-8000-000000000001";
    let receipt = write_archive(&directory, id, &archive).unwrap();
    let path = directory.join(&receipt.file);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.len(), receipt.bytes);
    assert_eq!(solomon_modding::assets::sha256(&bytes), receipt.sha256);
    let decoded = read_run_archive(&path).unwrap();
    assert_eq!(
        decoded.states.len(),
        1,
        "one Source checkpoint backs worst/final/both living players"
    );
    assert_eq!(decoded.archive.last_alive_by_player.len(), 2);
    assert_eq!(
        decoded.states[decoded.archive.worst_tick.state].checkpoint.participant_order,
        ["alpha", "beta"]
    );
    assert!(write_archive(&directory, id, &archive).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(
        !fs::read_dir(&directory).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".writing-"))
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&directory).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(directory.join(format!("{}.json", receipt.file)))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn source_archive_prune_honors_keep_pairs_temporary_names_and_worker_drain() {
    let directory = directory();
    let mut store = RunArchiveStore::new(directory.clone(), None).unwrap();
    let sink = store.sink();
    sink.save(sample_archive()).unwrap();
    sink.save(sample_archive()).unwrap();
    store.close();
    assert_eq!(
        fs::read_dir(&directory)
            .unwrap()
            .filter(|entry| entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".sdrrun.gz"))
            .count(),
        2
    );
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
    let expired = "00000000-0000-4000-8000-000000000002.sdrrun.gz";
    let kept = "00000000-0000-4000-8000-000000000003.sdrrun.gz";
    for name in [expired, kept, ".writing-A1b2C3", "untouched.sdrrun.gz"] {
        let path = directory.join(name);
        fs::write(&path, b"private-old").unwrap();
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(old))
            .unwrap();
    }
    fs::write(directory.join(format!("{expired}.json")), b"summary").unwrap();
    fs::write(directory.join(format!("{kept}.keep")), b"").unwrap();
    assert_eq!(
        prune_directory(&directory, old + Duration::from_secs(31 * 24 * 60 * 60)).unwrap(),
        2
    );
    assert!(!directory.join(expired).exists());
    assert!(!directory.join(format!("{expired}.json")).exists());
    assert!(directory.join(kept).exists());
    assert!(directory.join("untouched.sdrrun.gz").exists());
    assert!(!directory.join(".writing-A1b2C3").exists());
    fs::remove_dir_all(directory).unwrap();
}
