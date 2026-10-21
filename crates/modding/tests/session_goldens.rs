//! Real prepared Lua sessions replay Source adapter admission and rollback order.

mod common;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::intents::{
    ModIntentAdapter, ModIntentExecutionContext, ModIntentTransaction, execute_mod_intents,
};
use solomon_modding::session::{PreparedModSession, PreparedModSource};
use solomon_sim::modding::runtime::{
    ModIntent, PreparedModActionInput, PreparedModCheckpoint, PreparedModStepInput,
};

#[derive(Deserialize)]
struct Fixture {
    source: PreparedModSource,
    scenarios: Vec<Scenario>,
    preparation: Vec<Preparation>,
}
#[derive(Deserialize)]
struct Preparation {
    input: PreparedModSource,
    error: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Scenario {
    name: String,
    clock_step: f64,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    checkpoint: Option<PreparedModCheckpoint>,
    checkpoint_error: Option<String>,
    calls: Vec<Value>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Input {
    Behavior { value: Behavior },
    Step { input: PreparedModStepInput },
    Act { input: PreparedModActionInput },
    Restore { checkpoint: PreparedModCheckpoint },
    Project { viewer_id: String },
    Catalog,
    CloseRun { run_id: String },
    Close,
    Execute { intents: Vec<ModIntent>, context: ModIntentExecutionContext },
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Behavior {
    prepare_error: Option<String>,
    commit_error: Option<String>,
    rollback_error: Option<String>,
}
#[derive(Default)]
struct Adapter {
    behavior: Behavior,
    calls: Vec<Value>,
}
struct Transaction<'a>(&'a mut Adapter);

impl ModIntentAdapter for Adapter {
    fn prepare<'a>(
        &'a mut self,
        intents: &'a [ModIntent],
        context: &'a ModIntentExecutionContext,
    ) -> Result<Box<dyn ModIntentTransaction + 'a>, String> {
        self.calls.push(json!({"phase":"prepare","intents":intents,"context":context}));
        if let Some(error) = &self.behavior.prepare_error {
            return Err(error.clone());
        }
        Ok(Box::new(Transaction(self)))
    }
}
impl ModIntentTransaction for Transaction<'_> {
    fn commit(&mut self) -> Result<(), String> {
        self.0.calls.push(json!({"phase":"commit"}));
        match &self.0.behavior.commit_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    fn rollback(&mut self, reason: &str) -> Result<(), String> {
        self.0.calls.push(json!({"phase":"rollback","reason":reason}));
        match &self.0.behavior.rollback_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

#[test]
fn verified_prepared_sessions_commit_or_restore_actual_reducer_cells_and_timers() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-session-goldens.json"
    ))
    .unwrap();
    for case in fixture.preparation {
        let result = PreparedModSession::new(&[case.input]);
        assert_eq!(result.err(), Some(case.error));
    }
    for scenario in fixture.scenarios {
        let mut now = 0.0;
        let step = scenario.clock_step;
        let mut session =
            PreparedModSession::with_clock(std::slice::from_ref(&fixture.source), move || {
                let value = now;
                now += step;
                value
            })
            .unwrap();
        let mut adapter = Adapter::default();
        for (index, case) in scenario.cases.into_iter().enumerate() {
            let path = format!("{}.transaction{index}", scenario.name);
            let result: Result<Value, String> = match case.input {
                Input::Behavior { value } => {
                    adapter.behavior = value;
                    Ok(Value::Null)
                }
                Input::Step { input } => {
                    session.step(&input, &mut adapter).map(|value| json!(value))
                }
                Input::Act { input } => session.act(&input, &mut adapter).map(|value| json!(value)),
                Input::Restore { checkpoint } => session.restore(&checkpoint).map(|()| Value::Null),
                Input::Project { viewer_id } => {
                    session.project(&viewer_id).map(|value| json!(value))
                }
                Input::Catalog => session.catalog().map(|value| json!(value)),
                Input::CloseRun { run_id } => session.close_run(&run_id).map(|()| Value::Null),
                Input::Close => {
                    session.close();
                    Ok(Value::Null)
                }
                Input::Execute { intents, context } => {
                    Ok(json!(execute_mod_intents(&mut adapter, &intents, &context)))
                }
            };
            match result {
                Ok(output) => {
                    assert!(case.error.is_none(), "{path} unexpectedly accepted");
                    common::assert_json_values(&output, &case.output, &path);
                }
                Err(error) => assert_eq!(Some(error), case.error, "{path} rejection"),
            }
            match session.checkpoint() {
                Ok(checkpoint) => {
                    assert!(case.checkpoint_error.is_none(), "{path}.checkpoint");
                    common::assert_json_values(
                        &json!(checkpoint),
                        &json!(case.checkpoint.expect("open Source prepared session")),
                        &format!("{path}.checkpoint"),
                    );
                }
                Err(error) => {
                    assert_eq!(Some(error), case.checkpoint_error, "{path}.checkpoint error")
                }
            }
            common::assert_json_values(
                &json!(adapter.calls),
                &json!(case.calls),
                &format!("{path}.adapter"),
            );
        }
    }
}
