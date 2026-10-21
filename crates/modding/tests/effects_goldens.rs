//! Continuous custom spell actors produce actual Source ordered effect batches.

mod common;
use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::engines::effects::{ModSpellEffectEngine, ModSpellEffectSpawn};
use solomon_sim::Vec2;
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::runtime::{
    ModActorPlayer, ModSpellEffectCheckpoint, ModSpellEffectTarget, ModStateScope,
};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    checkpoint: ModSpellEffectCheckpoint,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Input {
    Spawn { input: Spawn },
    Tick { tick: u64, targets: Vec<ModSpellEffectTarget>, players: Vec<ModActorPlayer> },
    Retire { id: u64 },
    Restore { checkpoint: ModSpellEffectCheckpoint },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Spawn {
    content_id: String,
    fields: LuaObject,
    mod_id: String,
    origin: Point,
    owner_player_id: String,
    scope: ModStateScope,
    target: Point,
    tick: u64,
}
#[derive(Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

#[test]
fn custom_area_projectile_and_channel_actors_keep_source_targets_and_sequences() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-effects-goldens.json"
    ))
    .unwrap();
    let mut engine = ModSpellEffectEngine::new(100).unwrap();
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let result: Result<Value, String> = match case.input {
            Input::Spawn { input } => engine
                .spawn(ModSpellEffectSpawn {
                    content_id: &input.content_id,
                    fields: &input.fields,
                    mod_id: &input.mod_id,
                    origin: Vec2::new(input.origin.x, input.origin.y),
                    owner_player_id: &input.owner_player_id,
                    scope: input.scope,
                    target: Vec2::new(input.target.x, input.target.y),
                    tick: input.tick,
                })
                .map(|value| json!(value)),
            Input::Tick { tick, targets, players } => {
                engine.tick(tick, &players, &targets).map(|value| json!(value))
            }
            Input::Retire { id } => Ok(json!(engine.retire(id))),
            Input::Restore { checkpoint } => engine.restore(checkpoint).map(|()| Value::Null),
        };
        match result {
            Ok(output) => {
                assert!(case.error.is_none(), "transaction{index} unexpectedly accepted");
                common::assert_json_values(&output, &case.output, &format!("transaction{index}"));
            }
            Err(error) => assert_eq!(Some(error), case.error, "transaction{index} rejection"),
        }
        common::assert_json_values(
            &json!(engine.checkpoint()),
            &json!(case.checkpoint),
            &format!("transaction{index}.checkpoint"),
        );
    }
}
