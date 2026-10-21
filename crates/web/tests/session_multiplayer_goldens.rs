//! Actual public Game input maps preserve one shared authority and source entity order.
#[path = "../../sim/tests/support/player_combat.rs"]
#[allow(dead_code)]
mod player_combat;
#[path = "../../sim/tests/support/player_snapshot.rs"]
mod player_snapshot;
#[path = "../../sim/tests/support/secondary_state.rs"]
#[allow(dead_code)]
mod secondary_state;
#[path = "../../sim/tests/support/session.rs"]
mod session_snapshot;
#[path = "../../sim/tests/support/session_tick.rs"]
mod session_tick_rows;
#[path = "../../sim/tests/support/mod.rs"]
mod support;
#[path = "../../sim/tests/support/tutorial_snapshot.rs"]
mod tutorial_snapshot;

use serde_json::Value;
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_assets::navigation::decode_navigation;
use solomon_sim::Vec2;
use solomon_sim::belt::{NativeBeltCommand, PlayerBeltComponent};
use solomon_sim::enemies::model::EnemyDamageRequest;
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::profile::{NewGameSelection, PlayerSkillChoice, WizardDiscipline, WizardElement};
use solomon_sim::session::{
    GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn snapshot(session: &GameSession) -> Value {
    session_tick_rows::current_source_snapshot_all(session)
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn selection(config: &Value) -> NewGameSelection {
    NewGameSelection {
        name: config["displayName"].as_str().unwrap().into(),
        element: WizardElement::ALL
            .into_iter()
            .find(|element| element.as_str() == config["element"].as_str().unwrap())
            .unwrap(),
        discipline: WizardDiscipline::ALL
            .into_iter()
            .find(|discipline| discipline.as_str() == config["discipline"].as_str().unwrap())
            .unwrap(),
    }
}

fn compare(actual: &Value, expected: &Value, label: &str) {
    let mut trig_paths = Vec::new();
    for id in expected["ids"].as_array().unwrap() {
        let id = id.as_str().unwrap();
        for field in ["position.x", "position.y", "velocity.x", "velocity.y"] {
            trig_paths.push(format!("participants.{id}.character.{field}"));
        }
    }
    if let Some(students) = expected["shared"]["world"]["students"].as_array() {
        for index in 0..students.len() {
            for axis in 0..2 {
                trig_paths.push(format!("shared.world.students[{index}][1][{axis}]"));
            }
        }
    }
    if let Some(actors) = expected["shared"]["world"]["actors"].as_array() {
        for index in 0..actors.len() {
            for axis in 0..2 {
                trig_paths.push(format!("shared.world.actors[{index}][2][{axis}]"));
            }
            trig_paths.push(format!("shared.world.actors[{index}][3]"));
        }
    }
    let paths: Vec<_> = trig_paths.iter().map(String::as_str).collect();
    support::assert_website_json(actual, expected, label, &paths);
}

#[test]
fn shared_participants_match_public_game_and_checkpoint_continuation() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/session-multiplayer-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let spec = &case["spec"];
        let name = spec["name"].as_str().unwrap();
        let configurations = case["configurations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (row[0].as_str().unwrap().into(), selection(&row[1])))
            .collect();
        let mut session = GameSession::new_participants(
            configurations,
            GameSessionOptions { gameplay_rng_seed: 17, combat_rng_seed: 91, ..Default::default() },
        )
        .unwrap();
        let prepared = if spec["hub"] == true {
            None
        } else {
            let mut loaded = catalog.materialize([0; 16]);
            loaded.run_id = format!("multiplayer-{name}");
            loaded.seed = loaded.run_id.clone();
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../web/assets/navigation")
                .join(format!("{}.json", loaded.source_sha256));
            let prepared =
                decode_navigation(&std::fs::read_to_string(path).unwrap(), &loaded).unwrap();
            session.install_boneyard(loaded).unwrap();
            let world = session.boneyard.as_mut().unwrap();
            world.encounter = None;
            world.waves = None;
            world.arena_transition = None;
            for (index, id) in session.participant_order.iter().enumerate() {
                session.participants.get_mut(id).unwrap().wizard.position =
                    Vec2::new(1000.0 + index as f64 * 15.0, 1000.0);
            }
            Some(prepared)
        };
        if spec["secondary"] == true {
            let participant = session.participants.get_mut("zeta").unwrap();
            participant.player.book.permanent_ranks[72] = 1;
            participant.player.book.learned_skill_order.push(72);
            participant
                .player
                .skill_runtime
                .refresh(&mut participant.player.book, &participant.player.economy);
            participant.player.belt = PlayerBeltComponent::new(&participant.player.book);
            assert!(participant.player.belt.apply(
                NativeBeltCommand::BindSkill { skill_id: Some(72), slot: 2 },
                &participant.player.book,
                &participant.player.economy
            ));
        }
        let frames = case["frames"].as_array().unwrap();
        compare(&snapshot(&session), &frames[0]["expected"], &format!("{name} initial"));
        let mut next_frame = 1;
        for (index, transaction) in case["transactions"].as_array().unwrap().iter().enumerate() {
            for command in transaction["commands"].as_array().unwrap() {
                match command["action"].as_str().unwrap() {
                    "experience" => {
                        session.grant_experience(
                            command["playerId"].as_str().unwrap(),
                            command["amount"].as_f64().unwrap(),
                        );
                    }
                    "choice" => {
                        let choice = &command["choice"];
                        assert!(session.choose_skill(
                            command["playerId"].as_str().unwrap(),
                            PlayerSkillChoice {
                                choice_index: choice["choiceIndex"].as_u64().unwrap() as usize,
                                offer_sequence: choice["offerSequence"].as_u64().unwrap(),
                                skill_id: choice["skillId"].as_u64().unwrap() as usize,
                            }
                        ));
                    }
                    "damage" => {
                        session.boneyard.as_mut().unwrap().enemies.damage(
                            EnemyDamageRequest {
                                enhanced_effects: None,
                                actor_id: command["actorId"].as_u64().unwrap() as usize,
                                amount: command["amount"].as_f64().unwrap(),
                                tick: command["tick"].as_u64().unwrap(),
                                source_player_id: Some(
                                    command["sourcePlayerId"].as_str().unwrap().into(),
                                ),
                                ether_drain_capture: false,
                                has_magic_damage: false,
                                hit_strength: 1.0,
                                magic: false,
                                suppress_hit_reaction: false,
                                suppress_hurt_sound: false,
                            },
                            Some(&mut session.painter_order),
                        );
                    }
                    action => panic!("unknown fixture command {action}"),
                }
            }
            let inputs: BTreeMap<_, _> = transaction["inputs"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(id, input)| {
                    (
                        id.clone(),
                        GameSessionInput {
                            aim: input["aim"].as_object().map(|_| point(&input["aim"])),
                            movement: point(&input["movement"]),
                            primary: input["cast"]["primary"].as_bool().unwrap(),
                            quickbar: input["cast"]["quickbar"]
                                .as_u64()
                                .map(|value| value as usize),
                            viewport: Vec2::new(
                                input["viewportWidth"].as_f64().unwrap(),
                                input["viewportHeight"].as_f64().unwrap(),
                            ),
                        },
                    )
                })
                .collect();
            let spawns: Vec<_> = transaction["enemySpawnIntents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|spawn| {
                    EnemySpawnIntent::new(
                        EnemyToken::from_id(spawn["enemyToken"].as_str().unwrap()).unwrap(),
                        point(&spawn["position"]),
                        spawn["id"].as_u64().unwrap() as usize,
                        spawn["spawnTick"].as_u64().unwrap(),
                        spawn["waveOrdinal"].as_u64().unwrap(),
                    )
                })
                .collect();
            let context = || GameSessionTickContext {
                navigation: prepared.as_ref(),
                enemy_spawn_intents: &spawns,
                ..Default::default()
            };
            let mut continuation =
                if frames[next_frame - 1]["frame"].as_u64().unwrap() == index as u64 {
                    Some(
                        GameSession::restore(
                            session.checkpoint(),
                            session.boneyard.as_ref().map(|world| world.loaded.clone()),
                        )
                        .unwrap(),
                    )
                } else {
                    None
                };
            session.step(&inputs, context());
            if let Some(restored) = &mut continuation {
                restored.step(&inputs, context());
                assert_eq!(
                    serde_json::to_value(restored.snapshot()).unwrap(),
                    serde_json::to_value(session.snapshot()).unwrap(),
                    "{name} restored frame{}",
                    index + 1
                );
            }
            if let Some(frame) = frames.get(next_frame)
                && frame["frame"].as_u64().unwrap() == index as u64 + 1
            {
                compare(
                    &snapshot(&session),
                    &frame["expected"],
                    &format!("{name} frame{}", index + 1),
                );
                next_frame += 1;
            }
        }
        assert_eq!(next_frame, frames.len());
    }
}
