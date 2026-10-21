//! The complete standalone tick against the actual pinned public Website Game API.

#[path = "../../sim/tests/support/player_combat.rs"]
#[allow(dead_code)] // The shared projection also contains other fixture constructors.
mod player_combat;
#[path = "../../sim/tests/support/player_snapshot.rs"]
mod player_snapshot;
#[path = "../../sim/tests/support/secondary_state.rs"]
#[allow(dead_code)] // Reuse the complete canonical secondary-state projection.
mod secondary_state;
#[path = "../../sim/tests/support/session.rs"]
mod session_snapshot;
#[path = "../../sim/tests/support/mod.rs"]
mod support;
#[path = "../../sim/tests/support/tutorial_snapshot.rs"]
mod tutorial_snapshot;

use std::path::PathBuf;

use serde_json::{Value, json};
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_assets::navigation::decode_navigation;
use solomon_sim::Vec2;
use solomon_sim::belt::{NativeBeltCommand, PlayerBeltComponent};
use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::loot::{NativeLootDropKind, NativeLootDropSource, NativeLootDropSpec};
use solomon_sim::profile::{NewGameSelection, PlayerSkillChoice, WizardDiscipline, WizardElement};
use solomon_sim::session::{
    GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};

#[path = "../../sim/tests/support/session_tick.rs"]
#[allow(dead_code)] // The shared projection also exports its multi-participant view.
mod session_tick_rows;

fn snapshot(session: &GameSession) -> Value {
    session_tick_rows::current_source_snapshot(session, "local")
}

fn selection(element: &Value, discipline: &Value) -> NewGameSelection {
    NewGameSelection {
        name: "Oracle".into(),
        element: WizardElement::ALL
            .into_iter()
            .find(|element_id| element_id.as_str() == element.as_str().unwrap())
            .unwrap(),
        discipline: WizardDiscipline::ALL
            .into_iter()
            .find(|discipline_id| discipline_id.as_str() == discipline.as_str().unwrap())
            .unwrap(),
    }
}

fn navigation(loaded: &LoadedBoneyard) -> NavigationSet {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/assets/navigation")
        .join(format!("{}.json", loaded.source_sha256));
    decode_navigation(&std::fs::read_to_string(path).unwrap(), loaded).unwrap()
}

fn read_point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn compare(actual: &Value, expected: &Value, label: &str) {
    // Only positions/headings that feed native trigonometry use the documented 1e-9
    // platform-libm exception. RNG words, clocks, phases, resources and IDs stay exact.
    let mut trig_paths = vec![
        "character.position.x".to_owned(),
        "character.position.y".to_owned(),
        "character.velocity.x".to_owned(),
        "character.velocity.y".to_owned(),
    ];
    if let Some(students) = expected["world"]["students"].as_array() {
        for index in 0..students.len() {
            trig_paths.push(format!("world.students[{index}][1][0]"));
            trig_paths.push(format!("world.students[{index}][1][1]"));
        }
    }
    if let Some(actors) = expected["world"]["actors"].as_array() {
        for index in 0..actors.len() {
            trig_paths.push(format!("world.actors[{index}][2][0]"));
            trig_paths.push(format!("world.actors[{index}][2][1]"));
            trig_paths.push(format!("world.actors[{index}][3]"));
        }
    }
    let paths: Vec<_> = trig_paths.iter().map(String::as_str).collect();
    support::assert_website_json(actual, expected, label, &paths);
}

#[test]
fn complete_session_tick_matches_public_game_and_restored_continuation() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/session-tick-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let spec = &case["spec"];
        let name = spec["name"].as_str().unwrap();
        let mut session = GameSession::new(
            "local".into(),
            selection(&spec["element"], &spec["discipline"]),
            GameSessionOptions {
                gameplay_rng_seed: 17,
                combat_rng_seed: 91,
                ..GameSessionOptions::default()
            },
        );
        let prepared = if spec["hub"] == true {
            None
        } else {
            let mut loaded = if spec["tutorial"] == true {
                catalog.materialize_tutorial([0; 16])
            } else {
                catalog.materialize([0; 16])
            };
            loaded.run_id = format!("session-{name}");
            loaded.seed = loaded.run_id.clone();
            let prepared = navigation(&loaded);
            session.install_boneyard(loaded).unwrap();
            if spec["tutorial"] != true {
                let world = session.boneyard.as_mut().unwrap();
                world.encounter = None;
                world.waves = None;
                world.arena_transition = None;
                session.participants.get_mut("local").unwrap().wizard.position =
                    Vec2::new(1000.0, 1000.0);
            }
            Some(prepared)
        };
        {
            let participant = session.participants.get_mut("local").unwrap();
            if let Some(ranks) = spec["ranks"].as_array() {
                for rank in ranks {
                    let id = rank[0].as_u64().unwrap() as usize;
                    participant.player.book.permanent_ranks[id] = rank[1].as_u64().unwrap() as u32;
                    if !participant.player.book.learned_skill_order.contains(&id) {
                        participant.player.book.learned_skill_order.push(id);
                    }
                }
                participant
                    .player
                    .skill_runtime
                    .refresh(&mut participant.player.book, &participant.player.economy);
                participant.player.belt = PlayerBeltComponent::new(&participant.player.book);
            }
            if spec["secondary"] == true {
                assert!(participant.player.belt.apply(
                    NativeBeltCommand::BindSkill { skill_id: Some(72), slot: 2 },
                    &participant.player.book,
                    &participant.player.economy
                ));
            }
        }
        if let Some(amount) = spec["gold"].as_u64() {
            let mut gold = NativeLootDropSpec::new(
                NativeLootDropKind::Gold,
                1,
                Vec2::new(1000.0, 1000.0),
                NativeLootDropSource::Script,
            );
            gold.amount = Some(amount);
            gold.scatter_seed = Some(17);
            gold.tier = Some(2);
            assert_eq!(
                session.boneyard.as_mut().unwrap().loot.spawn_specs(
                    vec![gold],
                    0,
                    &mut session.painter_order
                ),
                0
            );
        }
        if spec["lethal"] == true {
            session
                .participants
                .get_mut("local")
                .unwrap()
                .player
                .damage_with_result(1000.0, 0, true, true, 1.0, 0.0);
        }
        if let Some(experience) = spec["experience"].as_f64() {
            session.grant_experience("local", experience);
        }
        if spec["college"] == true {
            assert!(session.arm_college_intro("local"));
        }
        let frames = case["frames"].as_array().unwrap();
        compare(&snapshot(&session), &frames[0]["expected"], &format!("{name} initial"));
        let mut next_snapshot = 1;
        for (index, transaction) in case["inputs"].as_array().unwrap().iter().enumerate() {
            let frame = index + 1;
            for command in case["commands"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|command| command["frame"].as_u64().unwrap() == frame as u64)
            {
                match command["action"].as_str().unwrap() {
                    "continue" => {
                        let run_id = session.run.run_id.clone().unwrap();
                        assert!(
                            session.continue_game_over(&run_id, session.run.game_over_event_id)
                        );
                    }
                    "ack-college" => assert!(session.acknowledge_college_intro_dialogue("local")),
                    "loadout" => {
                        assert!(
                            session.confirm_loadout(
                                "local",
                                selection(&json!("water"), &json!("mind"))
                            )
                        )
                    }
                    action => panic!("unknown command {action}"),
                }
            }
            for choice in case["choices"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|choice| choice["frame"].as_u64().unwrap() == frame as u64)
            {
                let choice = &choice["choice"];
                assert!(session.choose_skill(
                    "local",
                    PlayerSkillChoice {
                        choice_index: choice["choiceIndex"].as_u64().unwrap() as usize,
                        offer_sequence: choice["offerSequence"].as_u64().unwrap(),
                        skill_id: choice["skillId"].as_u64().unwrap() as usize,
                    }
                ));
            }
            let value = &transaction["input"];
            let input = GameSessionInput {
                aim: value["aim"].as_object().map(|_| read_point(&value["aim"])),
                movement: read_point(&value["movement"]),
                primary: value["cast"]["primary"].as_bool().unwrap(),
                quickbar: value["cast"]["quickbar"].as_u64().map(|slot| slot as usize),
                viewport: Vec2::new(
                    value["viewportWidth"].as_f64().unwrap(),
                    value["viewportHeight"].as_f64().unwrap(),
                ),
            };
            let spawns: Vec<_> = transaction["enemySpawnIntents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| {
                    EnemySpawnIntent::new(
                        EnemyToken::from_id(value["enemyToken"].as_str().unwrap()).unwrap(),
                        read_point(&value["position"]),
                        value["id"].as_u64().unwrap() as usize,
                        value["spawnTick"].as_u64().unwrap(),
                        value["waveOrdinal"].as_u64().unwrap(),
                    )
                })
                .collect();
            let ready_players = if transaction["collegeIntroReady"].as_bool().unwrap() {
                None
            } else {
                Some(std::collections::BTreeSet::new())
            };
            let inputs = std::collections::BTreeMap::from([("local".into(), input)]);
            let context = || GameSessionTickContext {
                navigation: prepared.as_ref(),
                college_intro_ready_player_ids: ready_players.as_ref(),
                enemy_spawn_intents: &spawns,
            };
            // Restore at every retained checkpoint, then drive the next actual input on
            // both states. Compare complete checkpoint debug values to cover owned fields
            // beyond the deliberately compact source projection.
            let mut continuation =
                if frames[next_snapshot - 1]["frame"].as_u64().unwrap() == index as u64 {
                    let loaded = session.boneyard.as_ref().map(|world| world.loaded.clone());
                    Some(GameSession::restore(session.checkpoint(), loaded).unwrap())
                } else {
                    None
                };
            session.step(&inputs, context());
            if let Some(restored) = &mut continuation {
                restored.step(&inputs, context());
                assert_eq!(
                    format!("{:?}", restored.checkpoint()),
                    format!("{:?}", session.checkpoint()),
                    "{name} checkpoint continuation at frame{frame}"
                );
            }
            if let Some(expected) = frames.get(next_snapshot)
                && expected["frame"].as_u64().unwrap() == frame as u64
            {
                compare(
                    &snapshot(&session),
                    &expected["expected"],
                    &format!("{name} frame{frame}"),
                );
                next_snapshot += 1;
            }
        }
        assert_eq!(next_snapshot, frames.len(), "{name}: every source checkpoint was visited");
    }
}
