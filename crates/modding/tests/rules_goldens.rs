//! Actual Lua callbacks replay Source events, timers, named RNG and reducer circuits.

mod common;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::definition::runtime::{
    WebLuaDefinitionRuntime, WebLuaDefinitionRuntimeOptions,
};
use solomon_modding::rules::{ModRuleEngine, ModRuleEngineOptions};
use solomon_sim::modding::definition::{CompiledWebLuaMod, WebLuaModIdentity};
use solomon_sim::modding::runtime::{
    ModReducerTrace, ModRuleDispatchInput, ModRuleEngineCheckpoint,
};

#[derive(Deserialize)]
struct Fixture {
    scenarios: Vec<Scenario>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Scenario {
    name: String,
    mods: Vec<Package>,
    clock_step: f64,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Package {
    identity: WebLuaModIdentity,
    code: String,
    compiled: CompiledWebLuaMod,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    checkpoint: Option<ModRuleEngineCheckpoint>,
    checkpoint_error: Option<String>,
    traces: Vec<ModReducerTrace>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Input {
    Dispatch { input: ModRuleDispatchInput },
    Due { tick: u64 },
    Fire { id: u64, tick: u64 },
    Cancel { id: u64 },
    Restore { checkpoint: ModRuleEngineCheckpoint },
    Rollback { checkpoint: ModRuleEngineCheckpoint },
    CloseRun { run_id: String },
    CloseMod { mod_id: String },
    Close,
}

#[test]
fn actual_lua_rules_timers_named_random_and_reducer_circuits_match_source() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-rule-goldens.json"
    ))
    .unwrap();
    for scenario in fixture.scenarios {
        let mut now = 0.0;
        let step = scenario.clock_step;
        let mut engine = ModRuleEngine::with_clock(ModRuleEngineOptions::default(), move || {
            let value = now;
            now += step;
            value
        })
        .unwrap();
        for package in scenario.mods {
            let mut runtime = WebLuaDefinitionRuntime::new(WebLuaDefinitionRuntimeOptions {
                entry_script: "scripts/main.lua".into(),
                identity: package.identity,
                scripts: Vec::new(),
            })
            .unwrap();
            runtime.run(&package.code).unwrap();
            // Use the actual accepted Source graph so timer IDs refer to its
            // occurrences. Native definition compilation is independently
            // replayed; Lua table iteration is not a stable execution order.
            engine.register(package.compiled, runtime).unwrap();
        }
        for (index, case) in scenario.cases.into_iter().enumerate() {
            let path = format!("{}.transaction{index}", scenario.name);
            let result: Result<Value, String> = match case.input {
                Input::Dispatch { input } => engine.dispatch(&input).map(|value| json!(value)),
                Input::Due { tick } => engine.due_timer_ids(tick).map(|value| json!(value)),
                Input::Fire { id, tick } => engine.fire_timer(id, tick).map(|value| json!(value)),
                Input::Cancel { id } => {
                    engine.cancel_timer(id);
                    Ok(Value::Null)
                }
                Input::Restore { checkpoint } => engine.restore(&checkpoint).map(|()| Value::Null),
                Input::Rollback { checkpoint } => {
                    engine.rollback(&checkpoint).map(|()| Value::Null)
                }
                Input::CloseRun { run_id } => engine.close_run(&run_id).map(|()| Value::Null),
                Input::CloseMod { mod_id } => Ok(json!(engine.close_mod(&mod_id))),
                Input::Close => {
                    engine.close();
                    Ok(Value::Null)
                }
            };
            match result {
                Ok(output) => {
                    assert!(case.error.is_none(), "{path} unexpectedly accepted");
                    common::assert_json_values(&output, &case.output, &path);
                }
                Err(error) => assert_eq!(Some(error), case.error, "{path} rejection"),
            }
            match engine.checkpoint() {
                Ok(checkpoint) => {
                    assert!(case.checkpoint_error.is_none(), "{path}.checkpoint");
                    common::assert_json_values(
                        &json!(checkpoint),
                        &json!(case.checkpoint.expect("open Source state store")),
                        &format!("{path}.checkpoint"),
                    );
                }
                Err(error) => {
                    assert_eq!(Some(error), case.checkpoint_error, "{path}.checkpoint error")
                }
            }
            common::assert_json_values(
                &json!(engine.traces()),
                &json!(case.traces),
                &format!("{path}.traces"),
            );
        }
    }
}
