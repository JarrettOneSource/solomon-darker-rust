//! Public Game API entry/return replay, using real retained Boneyard assets.
#[path = "../../sim/tests/support/player_snapshot.rs"]
mod player_snapshot;
#[path = "../../sim/tests/support/secondary.rs"]
#[allow(dead_code)] // Shared complete actor projection also contains other fixture readers.
mod secondary_snapshot;
#[path = "../../sim/tests/support/session.rs"]
mod session_snapshot;
#[path = "../../sim/tests/support/mod.rs"]
mod support;
#[path = "../../sim/tests/support/tutorial_snapshot.rs"]
mod tutorial_snapshot;

use serde_json::{Value, json};
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

fn registration(value: PainterRegistration) -> Value {
    json!({"managerLane":match value.manager_lane { WorldManagerLane::Actor => "actor", WorldManagerLane::Transient => "transient" },
        "registrationOrdinal":value.registration_ordinal})
}

fn snapshot(session: &GameSession) -> Value {
    let secondary = &session.secondary;
    let mut players = json!({});
    for (id, player) in &secondary.players {
        players[id] = secondary_snapshot::player_snapshot(player);
    }
    let score = &session.participants["local"].score;
    let world = if let Some(world) = &session.boneyard {
        let baseline = session.participants["local"].tutorial_profile_economy.as_ref().map(|economy| json!({
            "gold":economy.gold, "revision":economy.revision,"collegeIntroPending":economy.college_intro_pending,
            "inventory":economy.backpack.iter().map(|item|[item.id,u64::from(item.native_type_id),u64::from(item.quantity)]).collect::<Vec<_>>(),
            "equippedTints":[economy.equipment.hat.as_ref().and_then(|item|item.icon_tints),economy.equipment.robe.as_ref().and_then(|item|item.icon_tints)],
            "rng":player_snapshot::rng_snapshot(&economy.rng)
        }));
        json!({"kind":"boneyard","runId":world.loaded.run_id,
            "lanternLightRegistration":world.lantern_light_registration.map(registration),
            "solomonPainterRegistration":world.solomon_painter_registration.map(registration),
            "tutorial":world.tutorial.as_ref().map(tutorial_snapshot::state_row),
            "tutorialProfileEconomy":baseline,
            "score":session_snapshot::score(score)})
    } else {
        json!({"kind":"hub","traderAnimationSeed":session.hub_trader_animation_seed,
            "collisionRngState":session.collision_rng,"participant":session_snapshot::hub_participant(&session.participants["local"]),
            "studentIds":session.npcs.students.students.iter().map(|student|student.id).collect::<Vec<_>>(),
            "studentRegistrations":session.npcs.students.students.iter().map(|student|student.registration_ordinal).collect::<Vec<_>>()})
    };
    json!({"tick":session.ticks,"player":player_snapshot::snapshot(&session.participants["local"].player,&session.gameplay_rng),
        "combatRng":player_snapshot::rng_snapshot(&session.combat_rng),
        "character":session_snapshot::character(&session.participants["local"]),
        "lighting":session_snapshot::lighting(&session.participants["local"].lighting),
        "secondary":{"actors":secondary.actors.iter().map(|actor| secondary_snapshot::actor_snapshot(&actor)).collect::<Vec<_>>(),
            "nextActorId":secondary.next_actor_id,"nextEventId":secondary.next_event_id,"players":players,"rng":player_snapshot::rng_snapshot(&secondary.rng)},
        "run":session_snapshot::run(&session.run),
        "order":{"nextRegistrationOrdinal":{"actor":session.painter_order.next_actor_ordinal,"transient":session.painter_order.next_transient_ordinal}},"world":world})
}

#[test]
fn canonical_session_world_resets_match_public_website_game() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/session-lifecycle-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let config = &case["config"];
        let selection = NewGameSelection {
            name: "Oracle".into(),
            element: match config["element"].as_str().unwrap() {
                "ether" => WizardElement::Ether,
                "fire" => WizardElement::Fire,
                _ => unreachable!(),
            },
            discipline: match config["discipline"].as_str().unwrap() {
                "arcane" => WizardDiscipline::Arcane,
                "body" => WizardDiscipline::Body,
                _ => unreachable!(),
            },
        };
        let mut session = GameSession::new(
            "local".into(),
            selection,
            GameSessionOptions {
                gameplay_rng_seed: case["seed"].as_i64().unwrap(),
                combat_rng_seed: case["combatSeed"].as_i64().unwrap(),
                ..GameSessionOptions::default()
            },
        );
        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            match step["action"].as_str().unwrap() {
                "create" => {}
                "enter" => {
                    let mut loaded = if step["choice"] == "stock-tutorial" {
                        catalog.materialize_tutorial([0; 16])
                    } else {
                        catalog.materialize([0; 16])
                    };
                    loaded.run_id = step["expected"]["world"]["runId"].as_str().unwrap().into();
                    loaded.seed = loaded.run_id.clone();
                    session.install_boneyard(loaded).unwrap();
                }
                "return" => session.return_to_hub(),
                "pending-offer" => {
                    session.grant_experience("local", step["experience"].as_f64().unwrap());
                    assert!(!session.can_enter_boneyard());
                    assert_eq!(
                        session.install_boneyard(catalog.materialize([0; 16])).unwrap_err(),
                        "cannot enter a Boneyard during a level-up barrier"
                    );
                }
                _ => unreachable!(),
            }
            support::assert_website_json(
                &snapshot(&session),
                &step["expected"],
                &format!("session seed{} step{index}", case["seed"]),
                &[],
            );
            let scene = session.boneyard.as_ref().map(|world| world.loaded.clone());
            let restored = GameSession::restore(session.checkpoint(), scene).unwrap();
            support::assert_website_json(
                &snapshot(&restored),
                &step["expected"],
                "session restore",
                &[],
            );
            session = restored;
        }
    }
}
