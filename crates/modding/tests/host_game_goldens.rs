//! Real Source host rules mutate one canonical Session and roll back rejected batches.

mod common;
#[path = "common/inventory.rs"]
mod inventory;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::host::{PreparedModCastInput, PreparedModDerivedEvent, PreparedModHost};
use solomon_modding::package::{
    compile_web_session_content_definitions, materialize_web_session_content_value,
};
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};
use solomon_sim::modding::inventory::GameSimulationModConsumption;
use solomon_sim::modding::runtime::PreparedModHostCheckpoint;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

#[derive(Deserialize)]
struct Fixture {
    payload: LuaValue,
    loaded: Value,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    game: Value,
    checkpoint: PreparedModHostCheckpoint,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Input {
    Step { events: Vec<Event>, tick: u64, scope_id: Option<String>, context: Option<LuaObject> },
    Tick { tick: u64 },
    Mana { amount: f64 },
    Consume { consumption: Box<GameSimulationModConsumption> },
    Enter,
    Activate { content_id: Option<String>, restore_existing: bool },
    Cast { input: Cast },
    Restore { checkpoint: Box<PreparedModHostCheckpoint> },
    Drain,
}
#[derive(Deserialize)]
struct Event {
    name: String,
    payload: LuaValue,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cast {
    content_id: String,
    player_id: String,
    request_id: u64,
    context: Option<LuaObject>,
}

#[test]
fn prepared_host_actions_filters_contacts_and_failed_batches_use_actual_game_authority() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-host-game-goldens.json"
    ))
    .unwrap();
    let package = compile_web_session_content_definitions(
        materialize_web_session_content_value(&fixture.payload).unwrap(),
    )
    .unwrap();
    let mut game = GameSession::new(
        "player-1".into(),
        NewGameSelection {
            name: "Tester".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let mut host = PreparedModHost::with_clock(&package, &mut game, || 0.0).unwrap();
    let context: LuaObject = serde_json::from_value(json!({"participant_id":"player-1"})).unwrap();
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let path = format!("transaction{index}");
        let result: Result<Value, String> = match case.input {
            Input::Step { events, tick, scope_id, context: fields } => {
                let events: Vec<_> = events
                    .into_iter()
                    .map(|event| PreparedModDerivedEvent {
                        name: event.name,
                        payload: event.payload,
                    })
                    .collect();
                host.step(
                    &events,
                    tick,
                    scope_id.as_deref().unwrap_or("run-1"),
                    fields.as_ref().unwrap_or(&context),
                    &mut game,
                )
                .map(|value| json!(value))
            }
            Input::Tick { tick } => host.tick(tick, &mut game).map(|value| json!(value)),
            Input::Mana { amount } => {
                game.participants.get_mut("player-1").unwrap().player.combat.set_mana(amount);
                Ok(Value::Null)
            }
            Input::Consume { consumption } => {
                host.consume(&consumption, &mut game).map(|value| json!(value))
            }
            Input::Enter => {
                let loaded = &fixture.loaded;
                game.install_boneyard(LoadedBoneyard {
                    choice_id: "mod-host-game".into(),
                    choice_source: BoneyardSource::Mod,
                    mod_id: Some("example.host-content".into()),
                    mod_name: Some("Host Content".into()),
                    name: "Host Game".into(),
                    run_id: loaded["runId"].as_str().unwrap().into(),
                    seed: loaded["seed"].as_str().unwrap().into(),
                    source_sha256: loaded["sourceSha256"].as_str().unwrap().into(),
                    geometry_sha256: loaded["geometrySha256"].as_str().unwrap().into(),
                    scene: solomon_assets::boneyard::decode_scene(&loaded["scene"].to_string())
                        .unwrap(),
                })
                .map(|()| Value::Null)
            }
            Input::Activate { content_id, restore_existing } => host
                .activate_boneyard(content_id.as_deref(), restore_existing, &game)
                .map(|()| Value::Null),
            Input::Cast { input } => host
                .cast(
                    PreparedModCastInput {
                        content_id: &input.content_id,
                        player_id: &input.player_id,
                        request_id: input.request_id,
                        context: input.context.unwrap_or_default(),
                    },
                    &mut game,
                )
                .map(|value| json!(value)),
            Input::Restore { checkpoint } => host.restore(&checkpoint).map(|()| Value::Null),
            Input::Drain => {
                let spawns = host.drain_enemy_spawns().unwrap();
                assert!(spawns.is_empty(), "this source journey emits only custom enemies");
                Ok(json!({"spawns":[],"presentation":host.drain_presentation().unwrap()}))
            }
        };
        match result {
            Ok(output) => {
                assert_eq!(case.error, None, "{path}");
                common::assert_json_values(&output, &case.output, &format!("{path}.output"));
            }
            Err(error) => assert_eq!(Some(error), case.error, "{path}.error"),
        }
        let player = &game.participants["player-1"].player;
        let state = json!({
            "health":player.combat.current_health,"mana":player.combat.current_mana,
            "gold":player.economy.gold,"experience":player.progression.experience,
            "level":player.progression.level,"backpack":player.economy.backpack.iter().map(inventory::item_value).collect::<Vec<_>>(),
        });
        common::assert_json_values(&state, &case.game, &format!("{path}.game"));
        common::assert_json_values(
            &json!(host.checkpoint().unwrap()),
            &json!(case.checkpoint),
            &format!("{path}.checkpoint"),
        );
    }
}
