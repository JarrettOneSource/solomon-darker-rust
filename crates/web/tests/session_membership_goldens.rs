//! Public add/remove/merge, shared XP and detached skill transactions.
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
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::economy::HubInventoryAction;
use solomon_sim::profile::{
    NewGameSelection, PlayerSkillChoice, SharedPlayerLevelMilestone, WizardDiscipline,
    WizardElement,
};
use solomon_sim::session::{
    DetachedSessionParticipant, GameCheckpoint, GameSession, GameSessionOptions,
    GameSessionTickContext,
};

fn snapshot(session: &GameSession) -> Value {
    let mut value = session_tick_rows::snapshot_all(session);
    value["entityIds"] = serde_json::json!(
        session
            .participant_order
            .iter()
            .map(|id| session.participants[id].entity_id)
            .collect::<Vec<_>>()
    );
    value["nextEntityId"] = serde_json::json!(session.next_entity_id);
    value
}

fn selection(element: &str) -> NewGameSelection {
    NewGameSelection {
        name: "Oracle".into(),
        element: WizardElement::ALL.into_iter().find(|id| id.as_str() == element).unwrap(),
        discipline: WizardDiscipline::Arcane,
    }
}
fn choice(value: &Value) -> PlayerSkillChoice {
    PlayerSkillChoice {
        choice_index: value["choiceIndex"].as_u64().unwrap() as usize,
        offer_sequence: value["offerSequence"].as_u64().unwrap(),
        skill_id: value["skillId"].as_u64().unwrap() as usize,
    }
}
fn compare(actual: &Value, expected: &Value, label: &str) {
    // No movement until the final three idle ticks. Constructor trigonometry and
    // those physical positions retain the existing, bounded platform-libm rule.
    let mut paths = Vec::new();
    for id in expected["ids"].as_array().unwrap() {
        for field in ["position.x", "position.y", "velocity.x", "velocity.y"] {
            paths.push(format!("participants.{}.character.{field}", id.as_str().unwrap()));
        }
    }
    if let Some(students) = expected["shared"]["world"]["students"].as_array() {
        for index in 0..students.len() {
            for axis in 0..2 {
                paths.push(format!("shared.world.students[{index}][1][{axis}]"));
            }
        }
    }
    let paths: Vec<_> = paths.iter().map(String::as_str).collect();
    support::assert_website_json(actual, expected, label, &paths);
}

#[test]
fn membership_and_detached_choices_match_public_game_without_world_staging_clones() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/session-membership-goldens.json"
    ))
    .unwrap();
    let options =
        GameSessionOptions { gameplay_rng_seed: 17, combat_rng_seed: 91, ..Default::default() };
    let mut session = GameSession::new_participants(
        vec![("zeta".into(), selection("fire")), ("alpha".into(), selection("water"))],
        options,
    )
    .unwrap();
    let mut detached: Option<DetachedSessionParticipant> = None;
    let mut navigation: Option<NavigationSet> = None;
    for (index, step) in fixture["steps"].as_array().unwrap().iter().enumerate() {
        let request = &step["request"];
        let accepted = match step["action"].as_str().unwrap() {
            "create" => true,
            "add" | "duplicate-add" => session.add_participant(
                request["id"].as_str().unwrap().into(),
                selection(request["config"]["element"].as_str().unwrap()),
            ),
            "buy-charm" => {
                let id = request["id"].as_str().unwrap();
                session.participants.get_mut(id).unwrap().player.economy.gold =
                    request["gold"].as_u64().unwrap();
                session
                    .apply_inventory(
                        id,
                        &HubInventoryAction::BuyHagatha {
                            selector: request["selector"].as_i64().unwrap() as i32,
                        },
                    )
                    .transaction
                    .accepted
            }
            "remove" => session.remove_participant(request["id"].as_str().unwrap()),
            "merge" => {
                let source = GameSession::new(
                    "omega".into(),
                    selection("air"),
                    GameSessionOptions { gameplay_rng_seed: 73, ..Default::default() },
                );
                session.merge_hub(&source).unwrap();
                true
            }
            "enter" => {
                let mut loaded = BoneyardCatalog::load_embedded().unwrap().materialize([0; 16]);
                loaded.run_id = request["runId"].as_str().unwrap_or("membership-run").into();
                loaded.seed = loaded.run_id.clone();
                let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../web/assets/navigation")
                    .join(format!("{}.json", loaded.source_sha256));
                navigation = Some(
                    decode_navigation(&std::fs::read_to_string(path).unwrap(), &loaded).unwrap(),
                );
                session.install_boneyard(loaded).unwrap();
                true
            }
            "return" => {
                session.return_to_hub();
                true
            }
            "reset" => session.reset_participant(
                request["id"].as_str().unwrap(),
                selection(request["config"]["element"].as_str().unwrap()),
            ),
            "damage-player" => {
                session
                    .participants
                    .get_mut(request["id"].as_str().unwrap())
                    .unwrap()
                    .player
                    .damage_with_result(
                        request["amount"].as_f64().unwrap(),
                        0,
                        true,
                        false,
                        1.0,
                        0.0,
                    );
                true
            }
            "detach-remove" => {
                let id = request["id"].as_str().unwrap();
                detached = Some(session.detach_participant(id).unwrap());
                session.remove_participant(id)
            }
            "experience" => {
                session.grant_experience(
                    request["id"].as_str().unwrap(),
                    request["amount"].as_f64().unwrap(),
                );
                true
            }
            "synchronize-detached" => {
                let milestone = SharedPlayerLevelMilestone {
                    crossed_levels: request["crossedLevels"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_u64().unwrap() as u32)
                        .collect(),
                    experience: request["experience"].as_f64().unwrap(),
                    level: request["level"].as_u64().unwrap() as u32,
                };
                session.synchronize_detached(detached.as_mut().unwrap(), &milestone).unwrap();
                true
            }
            "project" => true,
            "choice-detached" => {
                session.choose_detached_skill(detached.as_mut().unwrap(), choice(request)).unwrap()
            }
            "reroll-detached" => session
                .reroll_detached_skill_offer(
                    detached.as_mut().unwrap(),
                    request["sequence"].as_u64().unwrap(),
                )
                .unwrap(),
            "defer-detached" => session
                .defer_detached_skill_choice(
                    detached.as_mut().unwrap(),
                    request["sequence"].as_u64().unwrap(),
                )
                .unwrap(),
            "rejoin" => {
                session.rejoin_participant(detached.take().unwrap(), None).unwrap();
                true
            }
            "choice" => {
                session.choose_skill(request["id"].as_str().unwrap(), choice(&request["choice"]))
            }
            "idle-tick" => {
                session.step(
                    &Default::default(),
                    GameSessionTickContext {
                        navigation: navigation.as_ref(),
                        ..Default::default()
                    },
                );
                true
            }
            action => panic!("unknown membership action {action}"),
        };
        assert_eq!(accepted, step["accepted"].as_bool().unwrap(), "membership step {index}");
        let mut profiles = serde_json::Map::new();
        for (id, participant) in &session.participants {
            let economy = &participant.player.economy;
            let storage: Vec<_> = solomon_sim::economy::project_inventory_items(&economy.storage)
                .iter()
                .map(|row| {
                    serde_json::json!([
                        row.depth,
                        row.parent_sack_id,
                        row.slot,
                        row.item.id,
                        row.item.native_type_id,
                        row.item.quantity,
                        row.item.name
                    ])
                })
                .collect();
            profiles.insert(
                id.clone(),
                serde_json::json!({"nextItemId":economy.next_item_id,"storage":storage}),
            );
        }
        assert_eq!(
            Value::Object(profiles),
            step["profiles"],
            "membership profile archives step {index}"
        );
        let live = serde_json::to_value(session.snapshot()).unwrap();
        let detached_ids = detached.as_ref().map(|detached| {
            serde_json::json!({"id": detached.participant.entity_id, "next": detached.next_entity_id})
        });
        assert_eq!(
            serde_json::json!(detached_ids),
            step["detachedIds"],
            "detached entity IDs step {index}"
        );
        compare(
            &snapshot(&session),
            &step["expected"],
            &format!("membership step {index}:{}", step["action"]),
        );
        if !step["projected"].is_null() {
            let projected = session.project_detached(detached.as_ref().unwrap()).unwrap();
            let checkpoint: GameCheckpoint = serde_json::from_value(
                serde_json::to_value(session.snapshot_with_detached(&projected)).unwrap(),
            )
            .unwrap();
            let view = GameSession::restore(
                checkpoint,
                session.boneyard.as_ref().map(|world| world.loaded.clone()),
            )
            .unwrap();
            compare(
                &snapshot(&view),
                &step["projected"],
                &format!("membership projected step {index}"),
            );
            assert_eq!(
                serde_json::to_value(session.snapshot()).unwrap(),
                live,
                "read-only projection must not consume live authority"
            );
        }
    }
}
