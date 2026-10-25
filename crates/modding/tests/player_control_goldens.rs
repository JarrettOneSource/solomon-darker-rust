//! Source Lua-controller leases use the actual Session and shared entrance navigator.

mod common;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::intents::ModIntentExecutionContext;
use solomon_modding::player_control::{
    ModPlayerControl, select_mod_player_skill, uses_player_control_rule,
};
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};
use solomon_sim::modding::runtime::ModIntent;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionInput, GameSessionOptions};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Fixture {
    loaded: Loaded,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Loaded {
    scene: Value,
    source_sha256: String,
    geometry_sha256: String,
    seed: String,
    run_id: String,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    leases: Value,
    offer: Value,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Input {
    Begin { now: f64 },
    Accept { intent: ModIntent, context: Option<ModIntentExecutionContext> },
    Apply,
    Observe { player_id: String },
    Save,
    Restore,
    Release { player_id: String },
    Clear,
    Xp { amount: f64 },
    Select { fields: LuaObject, context: Option<ModIntentExecutionContext> },
    Uses { value: LuaValue },
}
fn input_value(input: GameSessionInput) -> Value {
    json!({"aim":input.aim,"cast":{"primary":input.primary,"quickbar":input.quickbar},"movement":input.movement,"viewportHeight":input.viewport.y,"viewportWidth":input.viewport.x})
}

#[test]
fn lua_controller_admission_leases_expiry_observation_and_skill_choices_match_source() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-player-control-goldens.json"
    ))
    .unwrap();
    let mut game = GameSession::new(
        "local".into(),
        NewGameSelection {
            name: "Controller".into(),
            element: WizardElement::Fire,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    game.install_boneyard(LoadedBoneyard {
        choice_id: "default-random".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: "Controller".into(),
        run_id: fixture.loaded.run_id,
        seed: fixture.loaded.seed,
        source_sha256: fixture.loaded.source_sha256,
        geometry_sha256: fixture.loaded.geometry_sha256,
        scene: solomon_assets::boneyard::decode_scene(&fixture.loaded.scene.to_string())
            .unwrap()
            .into(),
    })
    .unwrap();
    let mut controller = ModPlayerControl::default();
    let human = GameSessionInput {
        viewport: solomon_sim::Vec2::new(1234.0, 768.0),
        ..GameSessionInput::default()
    };
    let default_context:ModIntentExecutionContext=serde_json::from_value(json!({"context":{"participant_id":"local"},"scope":{"kind":"participant-run","id":"local:controller-run"},"tick":0})).unwrap();
    let mut saved = None;
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let path = format!("transaction{index}");
        let result: Result<Value, String> = match case.input {
            Input::Begin { now } => controller
                .begin_frame(&game, &BTreeMap::from([("local".into(), human)]), now)
                .map(|value| json!(value)),
            Input::Accept { intent, context } => controller
                .accept(&game, &intent, context.as_ref().unwrap_or(&default_context))
                .map(|()| Value::Null),
            Input::Apply => {
                let mut inputs = BTreeMap::from([("local".into(), human)]);
                controller.apply(&mut inputs);
                Ok(json!({"local":input_value(inputs["local"])}))
            }
            Input::Observe { player_id } => {
                controller.observe(&game, &player_id).map(|value| json!(value))
            }
            Input::Save => {
                saved = Some(controller.checkpoint());
                Ok(Value::Null)
            }
            Input::Restore => {
                controller.restore(saved.clone().unwrap());
                Ok(Value::Null)
            }
            Input::Release { player_id } => {
                controller.release(&player_id);
                Ok(Value::Null)
            }
            Input::Clear => {
                controller.clear();
                Ok(Value::Null)
            }
            Input::Xp { amount } => {
                game.grant_experience("local", amount);
                Ok(Value::Null)
            }
            Input::Select { fields, context } => select_mod_player_skill(
                &mut game,
                &fields,
                context.as_ref().unwrap_or(&default_context),
            )
            .map(|()| Value::Null),
            Input::Uses { value } => Ok(json!(uses_player_control_rule(&value))),
        };
        match result {
            Ok(output) => {
                assert!(case.error.is_none(), "{path} unexpectedly accepted");
                common::assert_json_values(&output, &case.output, &path);
            }
            Err(error) => assert_eq!(Some(error), case.error, "{path} rejection"),
        }
        let leases:Vec<_>=controller.checkpoint().iter().map(|(player_id,lease)|json!({"playerId":player_id,"expiresAtMs":lease.expires_at_ms,"input":input_value(lease.input),"modId":lease.mod_id,"owner":lease.owner})).collect();
        common::assert_json_values(&json!(leases), &case.leases, &format!("{path}.leases"));
        let offer =
            game.participants["local"].player.progression.pending_offer.as_ref().map(|offer| {
                let options: Vec<_> = offer
                    .options
                    .iter()
                    .map(|option| {
                        let mut row =
                            json!({"skillId":option.skill_id,"targetRank":option.target_rank});
                        if option.insight {
                            row["insight"] = json!(true);
                        }
                        if let Some(id) = option.weld_build_id {
                            row["weldBuildId"] = json!(id);
                        }
                        row
                    })
                    .collect();
                let mut value =
                    json!({"level":offer.level,"options":options,"sequence":offer.sequence});
                if let Some(index) = offer.automatic_choice_index {
                    value["automaticChoiceIndex"] = json!(index);
                }
                value
            });
        common::assert_json_values(&json!(offer), &case.offer, &format!("{path}.offer"));
    }
}
