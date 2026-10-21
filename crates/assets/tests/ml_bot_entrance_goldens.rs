//! Source navigator outputs over all thirteen real scenes and lifecycle boundaries.

#[path = "../../sim/tests/support/mod.rs"]
mod support;

use serde_json::{Value, json};
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_assets::json_value::stringify_serializable;
use solomon_sim::Vec2;
use solomon_sim::boneyard::encounter::SolomonPhase;
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};
use solomon_sim::ml_bot::{BoneyardEntranceNavigator, MlBotEntranceNavigationError};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn point_json(point: Vec2) -> Value {
    json!({ "x": point.x, "y": point.y })
}

fn error_message(error: MlBotEntranceNavigationError) -> String {
    match error {
        MlBotEntranceNavigationError::NoPlayer(id) => {
            format!("Boneyard entrance navigation has no player {id}")
        }
        MlBotEntranceNavigationError::NoRoute { start, target } => {
            let target = target.map_or_else(
                || "Solomon".to_owned(),
                |target| stringify_serializable(&point_json(target)).unwrap(),
            );
            let start = stringify_serializable(&point_json(start)).unwrap();
            format!("ML bot found no collision-safe route to {target} from {start}")
        }
    }
}

#[test]
fn real_scene_entrance_inputs_and_navigation_lifetimes_match_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/ml-bot-entrance-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let tutorial = case["tutorial"].as_bool().unwrap();
        let template = if tutorial {
            &catalog.tutorial
        } else {
            &catalog.templates[case["index"].as_u64().unwrap() as usize]
        };
        let loaded = LoadedBoneyard {
            choice_id: if tutorial { "stock-tutorial".into() } else { "default-random".into() },
            choice_source: BoneyardSource::Default,
            mod_id: None,
            mod_name: None,
            name: template.scene.name.clone(),
            run_id: case["runId"].as_str().unwrap().into(),
            seed: case["seed"].as_str().unwrap().into(),
            source_sha256: template.source_sha256.clone(),
            geometry_sha256: template.geometry_sha256.clone(),
            scene: template.scene.clone(),
        };
        let mut session = GameSession::new(
            "local".into(),
            NewGameSelection {
                name: "Policy Bot".into(),
                element: WizardElement::Fire,
                discipline: WizardDiscipline::Arcane,
            },
            GameSessionOptions::default(),
        );
        session.install_boneyard(loaded).unwrap();
        let mut navigator = BoneyardEntranceNavigator::default();
        for (index, frame) in case["frames"].as_array().unwrap().iter().enumerate() {
            let input = &frame["input"];
            session.ticks = input["tick"].as_u64().unwrap();
            session.participants.get_mut("local").unwrap().wizard.position =
                point(&input["position"]);
            if let Some(encounter) = &mut session.boneyard.as_mut().unwrap().encounter {
                encounter.phase = if input["phase"] == "digging" {
                    SolomonPhase::Digging
                } else {
                    SolomonPhase::Speaking
                };
                encounter.run_event_id = input["runEventId"].as_u64().unwrap();
            }
            let hub = input["hub"].as_bool().unwrap();
            let saved_world = if hub { session.boneyard.take() } else { None };
            let result = navigator.input(&session, input["playerId"].as_str().unwrap());
            if hub {
                session.boneyard = saved_world;
            }
            let path = format!("scene {} frame {index}", case["index"]);
            if let Some(error) = frame["error"].as_str() {
                assert_eq!(error_message(result.unwrap_err()), error, "{path}");
                continue;
            }
            let actual = result.unwrap().map_or(Value::Null, |input| {
                json!({ "aim": input.aim.map(point_json),
                    "cast": { "primary": input.primary, "quickbar": input.quickbar },
                    "movement": point_json(input.movement),
                    "viewportWidth": input.viewport.x, "viewportHeight": input.viewport.y })
            });
            support::assert_website_json(&actual, &frame["output"], &path, &[]);
        }
    }
}
