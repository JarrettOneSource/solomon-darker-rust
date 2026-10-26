//! Real Console Lua 5.4 differential journeys, including its enabled SDK boundary.
mod common;
use std::cell::Cell;
use std::rc::Rc;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::console::*;
use solomon_sim::modding::console::WebLuaFrameState;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    developer: bool,
    frame: WebLuaFrameState,
    clock_step: f64,
    actions: Vec<Value>,
    rows: Vec<Value>,
}
struct Binding {
    frame: WebLuaFrameState,
    bot_calls: Vec<Value>,
}
impl WebLuaRuntimeBindings for Binding {
    fn authority_player_id(&self) -> Option<String> {
        self.frame.authority_player_id.clone()
    }
    fn frame(&self) -> WebLuaFrameState {
        self.frame.clone()
    }
    fn summon_bot(&mut self, config: WebLuaBotConfig) -> Result<WebLuaBotResult, String> {
        // Source's runtime SDK test adapter records the actual delegate contract.
        // Production admission is the launcher's real bot controller, not this fixture.
        self.bot_calls.push(
            json!({"discipline":config.discipline.as_str(),"element":config.element.as_str()}),
        );
        let id = format!("bot-{}", self.bot_calls.len());
        let mut bot = self.frame.players[0].clone();
        bot.id = id.clone();
        bot.display_name = "Test Bot".into();
        self.frame.players.push(bot);
        self.frame.player_count += 1;
        Ok(WebLuaBotResult { display_name: "Test Bot".into(), player_id: id })
    }
}
#[test]
fn real_console_sdk_scheduling_authority_and_limits_match_source() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/web-lua-console-goldens.json"
    ))
    .unwrap();
    for case in fixture.cases {
        let clock = Rc::new(Cell::new(0.0));
        let current = Rc::clone(&clock);
        let mut runtime = WebLuaRuntime::with_clock(case.developer, None, move || {
            let now = current.get();
            current.set(now + case.clock_step);
            now
        })
        .unwrap();
        let mut binding = Binding { frame: case.frame, bot_calls: Vec::new() };
        for (index, (action, expected)) in case.actions.iter().zip(case.rows).enumerate() {
            let expected = expected_with_lua_nil(&case.name, index, expected);
            let mut result = Value::Null;
            match action["kind"].as_str().unwrap() {
                "execute" | "enqueue" => {
                    result = json!(runtime.enqueue_execution(WebLuaExecutionRequest {
                        request_id: index as u64,
                        player_id: action["playerId"].as_str().unwrap_or("player-1").into(),
                        code: action["code"].as_str().unwrap().into()
                    }));
                    if action["kind"] == "execute" {
                        let tick = action["tick"].as_u64().unwrap();
                        binding.frame.tick = tick;
                        runtime.begin_tick(tick, &mut binding).unwrap();
                    }
                }
                "tick" => {
                    let tick = action["tick"].as_u64().unwrap();
                    binding.frame.tick = tick;
                    runtime.begin_tick(tick, &mut binding).unwrap();
                }
                "frame" => {
                    let mut frame = json!(binding.frame);
                    for (key, value) in action["frame"].as_object().unwrap() {
                        frame[key] = value.clone();
                    }
                    binding.frame = serde_json::from_value(frame).unwrap();
                }
                "dispatch" => {
                    let payload: LuaValue =
                        serde_json::from_value(action["payload"].clone()).unwrap();
                    runtime
                        .dispatch(
                            action["name"].as_str().unwrap(),
                            &payload,
                            action["playerId"].as_str(),
                            &mut binding,
                        )
                        .unwrap();
                }
                "restore" => {
                    let state: LuaObject = serde_json::from_value(action["state"].clone()).unwrap();
                    if let Err(error) = runtime.restore_state(&state) {
                        result = json!(error);
                    }
                }
                "wants" => result = json!(runtime.wants_event(action["name"].as_str().unwrap())),
                "close" => runtime.close(),
                kind => panic!("unknown fixture action {kind}"),
            }
            let metrics = runtime.metrics();
            let actual = json!({"result":result,"completions":runtime.drain_completions(),"commands":runtime.drain_commands(),"logs":runtime.drain_logs(),"state":runtime.snapshot_state(),"bots":std::mem::take(&mut binding.bot_calls),"metrics":{"callbackCount":metrics.callback_count,"pendingExecutions":metrics.pending_executions,"stateBytes":metrics.state_bytes,"timerCount":metrics.timer_count,"budgetExceededCount":metrics.budget_exceeded_count}});
            common::assert_json_values(&actual, &expected, &format!("{}[{index}]", case.name));
        }
        assert!(runtime.metrics().memory_bytes <= 16 * 1024 * 1024);
    }
}

// Retain every recorded action and the immutable recording. These explicit
// expectations replace the retired JavaScript bridge's null exception and its
// consequences: normal Lua nil values, continued commands and surviving callbacks.
fn expected_with_lua_nil(case: &str, index: usize, mut expected: Value) -> Value {
    match (case, index) {
        ("boneyard-spawn-and-source-identities", 0) => {
            expected["completions"][0]["result"] = json!({
                "error": null, "ok": true, "output": [],
                "values": [
                    {"base":"skeleton", "key":"skeleton", "native_type_id":1001,
                     "request_id":1, "x":60, "y":70},
                    null, null, null, 2
                ]
            });
            expected["commands"].as_array_mut().unwrap().push(json!({
                "requestId":2, "token":"DEMON", "type":"spawn-enemy", "x":-1, "y":0
            }));
        }
        ("events-frame-boundary-and-current-authority", 0) => {
            expected["completions"][0]["result"] =
                json!({"error":null, "ok":true, "output":[], "values":[1]});
        }
        ("events-frame-boundary-and-current-authority", 3..=5) => {
            if index == 3 {
                expected["logs"].as_array_mut().unwrap().remove(0);
            }
            expected["state"] =
                json!({"run":if index == 3 { "run-a" } else { "run-b" }, "tick":99});
            expected["metrics"]["callbackCount"] = json!(1);
            expected["metrics"]["stateBytes"] = json!(25);
        }
        ("seed-selection-and-lifecycle", 4) => {
            expected["completions"][0]["result"] =
                json!({"error":null, "ok":true, "output":[], "values":[null]});
        }
        _ => {}
    }
    expected
}

#[test]
fn college_console_reads_optional_values_as_lua_nil() {
    let game = GameSession::new(
        "player-1".into(),
        NewGameSelection {
            name: "Native Wizard".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let mut binding = Binding {
        frame: solomon_sim::modding::console::create_web_lua_frame_state(&game, Some("player-1")),
        bot_calls: Vec::new(),
    };
    let mut runtime = WebLuaRuntime::with_clock(false, None, || 0.0).unwrap();
    assert!(
        runtime.enqueue_execution(WebLuaExecutionRequest {
            request_id: 1,
            player_id: "player-1".into(),
            code: "local frame = sd.runtime.get_frame_state(); \
               return frame.world, frame.players[1].displayName, \
               frame.runId == nil, frame.boneyardSeed == nil, frame.waves == nil, \
               sd.waves.get_state() == nil, sd.gameplay.get_combat_state() == nil, \
               sd.enemies.get('missing') == nil, sd.state.get('missing') == nil, \
               sd.rng.get_seed() == nil"
                .into(),
        })
    );
    runtime.begin_tick(1, &mut binding).unwrap();
    let result = runtime.drain_completions().remove(0).result;
    assert!(result.ok, "{:?}", result.error);
    let mut expected =
        vec![LuaValue::String("hub".into()), LuaValue::String("Native Wizard".into())];
    expected.extend(std::iter::repeat_n(LuaValue::Boolean(true), 8));
    assert_eq!(result.values, expected);
    assert!(runtime.drain_commands().is_empty());
}

#[test]
fn actual_lua_timeout_and_memory_failure_leave_the_vm_available() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/web-lua-console-goldens.json"
    ))
    .unwrap();
    let mut binding = Binding { frame: fixture.cases[0].frame.clone(), bot_calls: Vec::new() };
    let mut runtime = WebLuaRuntime::with_clock(false, None, || 0.0).unwrap();
    for (index, code) in [
        "persisted=42;while true do end",
        "return string.rep('x',32*1024*1024)",
        "return persisted",
    ]
    .into_iter()
    .enumerate()
    {
        assert!(runtime.enqueue_execution(WebLuaExecutionRequest {
            request_id: index as u64,
            player_id: "player-1".into(),
            code: code.into()
        }));
        runtime.begin_tick(index as u64 + 1, &mut binding).unwrap();
        let result = runtime.drain_completions().remove(0).result;
        if index == 0 {
            assert!(!result.ok);
            assert!(result.error.unwrap().contains("thread timeout exceeded"));
        } else if index == 1 {
            assert!(!result.ok);
            let error = result.error.unwrap();
            assert!(error.contains("memory") || error.contains("allocation"), "{error}");
        } else {
            assert!(result.ok);
            assert_eq!(result.values, vec![LuaValue::Number(42.0)]);
        }
    }
    assert!(runtime.metrics().memory_bytes <= 16 * 1024 * 1024);
}
