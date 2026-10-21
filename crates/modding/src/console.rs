//! Enabled game Console from `host/lua/web-lua-runtime.ts` and `web-lua-api.ts`.
//! The actual Lua 5.4 VM retains globals, callbacks and timers. Host operations
//! borrow the canonical authority only during a call; queued game mutations use
//! `sim::modding::console::apply_web_lua_commands`.

mod api;
mod scheduler;
mod values;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::{Function, HookTriggers, Lua, MultiValue, Table, Value, VmState};
use serde::Serialize;
use solomon_sim::modding::console::{WebLuaCommand, WebLuaFrameState};
use solomon_sim::modding::definition::WebLuaModIdentity;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::profile::{WizardDiscipline, WizardElement};
use values::{bounded_error, json_value, object};

const PROVIDER: &str = "solomon.console.host";
const VM_MEMORY_BYTES: usize = 16 * 1_024 * 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebLuaBotConfig {
    pub discipline: WizardDiscipline,
    pub element: WizardElement,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WebLuaBotResult {
    pub display_name: String,
    pub player_id: String,
}

/// Source's live frame/authority readers and optional developer operation.
/// The developer namespace is installed only when its capability is enabled.
/// A bot summon may change the actual roster before the first lazy frame read.
pub trait WebLuaRuntimeBindings {
    fn authority_player_id(&self) -> Option<String>;
    fn frame(&self) -> WebLuaFrameState;
    fn summon_bot(&mut self, config: WebLuaBotConfig) -> Result<WebLuaBotResult, String>;
}

#[derive(Clone, Debug)]
pub struct WebLuaExecutionRequest {
    pub request_id: u64,
    pub player_id: String,
    pub code: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WebLuaExecutionResult {
    pub error: Option<String>,
    pub ok: bool,
    pub output: Vec<String>,
    pub values: Vec<LuaValue>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebLuaExecutionCompletion {
    pub request_id: u64,
    pub result: WebLuaExecutionResult,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WebLuaRuntimeLog {
    pub level: &'static str,
    pub event: &'static str,
    pub detail: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebLuaRuntimeMetrics {
    pub budget_exceeded_count: u64,
    pub callback_count: usize,
    pub initialized_at_ms: f64,
    pub last_tick_work_ms: f64,
    pub maximum_tick_work_ms: f64,
    pub memory_bytes: usize,
    pub pending_executions: usize,
    pub state_bytes: usize,
    pub timer_count: usize,
}

pub struct WebLuaRuntime {
    lua: Option<Lua>,
    state: Rc<RefCell<RuntimeState>>,
    deadline: Rc<Cell<Option<Instant>>>,
    pending: VecDeque<WebLuaExecutionRequest>,
    completions: Vec<WebLuaExecutionCompletion>,
    initialized_at_ms: f64,
}

#[derive(Clone)]
struct RegisteredCallback {
    name: String,
    handle: u64,
    callback: Function,
}
#[derive(Clone)]
struct TimerStep {
    callback: Function,
    delay_ticks: u64,
}
struct LuaTimer {
    callback: Function,
    due_tick: u64,
    handle: u64,
    interval_ticks: Option<u64>,
    sequence_steps: Vec<TimerStep>,
    sequence_index: usize,
}
struct RuntimeState {
    identity: WebLuaModIdentity,
    clock: Box<dyn FnMut() -> f64>,
    current_tick: u64,
    next_handle: u64,
    next_spawn_request_id: u64,
    frame: Option<WebLuaFrameState>,
    post_simulation_frame: bool,
    active_player_id: Option<String>,
    active_output: Option<Vec<String>>,
    active_output_bytes: usize,
    callbacks: Vec<RegisteredCallback>,
    timers: Vec<Rc<RefCell<LuaTimer>>>,
    state: LuaObject,
    state_revision: u64,
    selected_run_seed: Option<u32>,
    selected_run_id: Option<String>,
    commands: Vec<WebLuaCommand>,
    logs: Vec<WebLuaRuntimeLog>,
    tick_work_ms: f64,
    last_tick_work_ms: f64,
    maximum_tick_work_ms: f64,
    callback_invocations: usize,
    callback_warning_tick: Option<u64>,
    budget_warning_tick: Option<u64>,
    budget_exceeded_count: u64,
}

impl WebLuaRuntime {
    pub fn new(developer_enabled: bool) -> Result<Self, String> {
        let started = Instant::now();
        Self::with_clock(developer_enabled, None, move || started.elapsed().as_secs_f64() * 1_000.0)
    }

    pub fn with_clock(
        developer_enabled: bool,
        identity: Option<WebLuaModIdentity>,
        mut clock: impl FnMut() -> f64 + 'static,
    ) -> Result<Self, String> {
        let started = clock();
        let identity = identity.unwrap_or(WebLuaModIdentity {
            id: "web.dev-console".into(),
            name: "Browser Dev Console".into(),
            version: "1.0.0".into(),
        });
        for (field, value) in [
            ("mod id", &identity.id),
            ("mod name", &identity.name),
            ("mod version", &identity.version),
        ] {
            if value.is_empty() || value.len() > 128 {
                return Err(format!("{field} must be nonempty text of at most 128 bytes"));
            }
        }
        let state = Rc::new(RefCell::new(RuntimeState {
            identity,
            clock: Box::new(clock),
            current_tick: 0,
            next_handle: 1,
            next_spawn_request_id: 1,
            frame: None,
            post_simulation_frame: false,
            active_player_id: None,
            active_output: None,
            active_output_bytes: 0,
            callbacks: Vec::new(),
            timers: Vec::new(),
            state: LuaObject::new(),
            state_revision: 0,
            selected_run_seed: None,
            selected_run_id: None,
            commands: Vec::new(),
            logs: Vec::new(),
            tick_work_ms: 0.0,
            last_tick_work_ms: 0.0,
            maximum_tick_work_ms: 0.0,
            callback_invocations: 0,
            callback_warning_tick: None,
            budget_warning_tick: None,
            budget_exceeded_count: 0,
        }));
        let lua = Lua::new();
        lua.set_memory_limit(VM_MEMORY_BYTES).map_err(|error| error.to_string())?;
        let deadline = Rc::new(Cell::new(None));
        let hook = Rc::clone(&deadline);
        lua.set_hook(HookTriggers::new().every_nth_instruction(1_000), move |_, _| {
            check_deadline(&hook)
        })
        .map_err(|error| error.to_string())?;
        let hook = Rc::clone(&deadline);
        lua.set_global_hook(HookTriggers::new().every_nth_instruction(1_000), move |_, _| {
            check_deadline(&hook)
        })
        .map_err(|error| error.to_string())?;
        lua.set_named_registry_value(
            PROVIDER,
            lua.create_table().map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        api::install(&lua, Rc::clone(&state), developer_enabled)
            .map_err(|error| error.to_string())?;
        lua.load("__sd_submit_filter_result=nil;io=nil;os=nil;package=nil;module=nil;require=nil;load=nil;loadfile=nil;dofile=nil;debug=nil;collectgarbage=nil;coroutine=nil")
            .exec().map_err(|error| error.to_string())?;
        let initialized_at_ms = (state.borrow_mut().clock)() - started;
        Ok(Self {
            lua: Some(lua),
            state,
            deadline,
            pending: VecDeque::new(),
            completions: Vec::new(),
            initialized_at_ms,
        })
    }

    pub fn enqueue_execution(&mut self, request: WebLuaExecutionRequest) -> bool {
        if self.lua.is_none()
            || self.pending.len() >= 8
            || request.code.is_empty()
            || request.code.len() > 48 * 1_024
            || request.code.encode_utf16().count() > 48 * 1_024
        {
            return false;
        }
        self.pending.push_back(request);
        true
    }

    pub fn begin_tick(
        &mut self,
        tick: u64,
        bindings: &mut dyn WebLuaRuntimeBindings,
    ) -> Result<(), String> {
        if self.lua.is_none() {
            return Ok(());
        }
        if tick > 9_007_199_254_740_991 {
            return Err("runtime tick must be a safe integer".into());
        }
        self.with_bindings(bindings, |runtime, lua| {
            {
                let mut state = runtime.state.borrow_mut();
                state.current_tick = tick;
                state.last_tick_work_ms = state.tick_work_ms;
                state.tick_work_ms = 0.0;
                state.callback_invocations = 0;
                state.post_simulation_frame = false;
                state.frame = None;
            }
            if let Some(request) = runtime.pending.pop_front() {
                let result = if authority(lua)?.as_deref() == Some(request.player_id.as_str()) {
                    let started = runtime.now();
                    let result = runtime.execute(lua, &request.code, &request.player_id);
                    runtime.record_work(started);
                    result
                } else {
                    failed("Lua execution authority changed before the request ran.")
                };
                runtime
                    .completions
                    .push(WebLuaExecutionCompletion { request_id: request.request_id, result });
            }
            runtime.run_due_timers(lua)?;
            if runtime.state.borrow().callbacks.iter().any(|entry| entry.name == "runtime.tick") {
                let payload = object([
                    ("monotonic_milliseconds", LuaValue::Number(runtime.now().floor())),
                    ("tick_count", LuaValue::Number(tick as f64)),
                    ("tick_interval_ms", LuaValue::Number(10.0)),
                ]);
                runtime.dispatch_callbacks(lua, "runtime.tick", &payload, None)?;
            }
            Ok(())
        })
    }

    pub fn dispatch(
        &mut self,
        name: &str,
        payload: &LuaValue,
        active_player_id: Option<&str>,
        bindings: &mut dyn WebLuaRuntimeBindings,
    ) -> Result<(), String> {
        if self.lua.is_none() || name == "runtime.tick" {
            return Ok(());
        }
        self.with_bindings(bindings, |runtime, lua| {
            {
                let mut state = runtime.state.borrow_mut();
                if !state.post_simulation_frame {
                    state.post_simulation_frame = true;
                    state.frame = None;
                }
                if name == "run.started" && state.selected_run_seed.is_some() {
                    if let Some(id) = payload.get("run_id").and_then(LuaValue::as_str) {
                        state.selected_run_id = Some(id.into());
                    }
                } else if name == "run.ended" && state.selected_run_id.is_some() {
                    state.selected_run_seed = None;
                    state.selected_run_id = None;
                }
            }
            runtime.dispatch_callbacks(lua, name, payload, active_player_id)
        })
    }

    pub fn wants_event(&self, name: &str) -> bool {
        let state = self.state.borrow();
        state.selected_run_seed.is_some() && matches!(name, "run.started" | "run.ended")
            || state.callbacks.iter().any(|entry| entry.name == name)
    }
    pub fn drain_commands(&mut self) -> Vec<WebLuaCommand> {
        std::mem::take(&mut self.state.borrow_mut().commands)
    }
    pub fn drain_completions(&mut self) -> Vec<WebLuaExecutionCompletion> {
        std::mem::take(&mut self.completions)
    }
    pub fn drain_logs(&mut self) -> Vec<WebLuaRuntimeLog> {
        std::mem::take(&mut self.state.borrow_mut().logs)
    }
    pub fn snapshot_state(&self) -> LuaObject {
        self.state.borrow().state.clone()
    }
    pub fn restore_state(&mut self, source: &LuaObject) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        state.clear_state();
        for (key, value) in source.iter() {
            // Source restore clears first and may retain a valid prefix if a
            // later entry fails. Validate keys before visiting their values.
            if key.is_empty() || key.len() > 128 {
                return Err("state key must be nonempty text of at most 128 bytes".into());
            }
            if matches!(value, LuaValue::Null) {
                return Err("Lua state values may not be nil; use sd.state.delete".into());
            }
            let value = values::normalize_saved(value, &format!("state.{key}"))?;
            state.set_state(key, value)?;
        }
        Ok(())
    }
    pub fn metrics(&self) -> WebLuaRuntimeMetrics {
        let state = self.state.borrow();
        WebLuaRuntimeMetrics {
            budget_exceeded_count: state.budget_exceeded_count,
            callback_count: state.callbacks.len(),
            initialized_at_ms: self.initialized_at_ms,
            last_tick_work_ms: state.last_tick_work_ms,
            maximum_tick_work_ms: state.maximum_tick_work_ms,
            memory_bytes: self.lua.as_ref().map_or(0, Lua::used_memory),
            pending_executions: self.pending.len(),
            state_bytes: state.state_bytes(),
            timer_count: state.timers.len(),
        }
    }
    pub fn close(&mut self) {
        if self.lua.is_none() {
            return;
        }
        for request in self.pending.drain(..) {
            self.completions.push(WebLuaExecutionCompletion {
                request_id: request.request_id,
                result: failed("Lua runtime closed before the request executed."),
            });
        }
        let mut state = self.state.borrow_mut();
        state.callbacks.clear();
        state.timers.clear();
        state.state = LuaObject::new();
        state.selected_run_seed = None;
        state.selected_run_id = None;
        state.commands.clear();
        self.lua = None;
    }

    fn with_bindings(
        &mut self,
        bindings: &mut dyn WebLuaRuntimeBindings,
        action: impl FnOnce(&mut Self, &Lua) -> mlua::Result<()>,
    ) -> Result<(), String> {
        let lua = self.lua.as_ref().ok_or("Web Lua runtime is closed")?.clone();
        let host = RefCell::new(bindings);
        lua.scope(|scope| {
            let provider: Table = lua.named_registry_value(PROVIDER)?;
            provider.set(
                "authority",
                scope.create_function(|_, ()| Ok(host.borrow().authority_player_id()))?,
            )?;
            provider.set(
                "frame",
                scope.create_function(|_, ()| {
                    serde_json::to_string(&host.borrow().frame())
                        .map_err(|error| mlua::Error::RuntimeError(error.to_string()))
                })?,
            )?;
            provider.set(
                "summon",
                scope.create_function(|lua, (discipline, element): (String, String)| {
                    let discipline = match discipline.as_str() {
                        "arcane" => WizardDiscipline::Arcane,
                        "body" => WizardDiscipline::Body,
                        "mind" => WizardDiscipline::Mind,
                        _ => unreachable!("SDK validates discipline"),
                    };
                    let element = match element.as_str() {
                        "air" => WizardElement::Air,
                        "earth" => WizardElement::Earth,
                        "ether" => WizardElement::Ether,
                        "fire" => WizardElement::Fire,
                        "water" => WizardElement::Water,
                        _ => unreachable!("SDK validates element"),
                    };
                    let result = host
                        .borrow_mut()
                        .summon_bot(WebLuaBotConfig { discipline, element })
                        .map_err(mlua::Error::RuntimeError)?;
                    let value = json_value(result).map_err(mlua::Error::RuntimeError)?;
                    crate::definition::values::lua_payload(lua, &value)
                })?,
            )?;
            let result = action(self, &lua);
            provider.clear()?;
            result
        })
        .map_err(|error| bounded_error(&error))
    }

    fn execute(&mut self, lua: &Lua, code: &str, player_id: &str) -> WebLuaExecutionResult {
        {
            let mut state = self.state.borrow_mut();
            state.active_output = Some(Vec::new());
            state.active_output_bytes = 0;
            state.active_player_id = Some(player_id.into());
        }
        self.deadline.set(Some(Instant::now() + Duration::from_millis(4)));
        let returned = lua.load(code).set_name("=browser-dev-console").call::<MultiValue>(());
        self.deadline.set(None);
        let normalized = returned.and_then(|returned| {
            let mut budget = values::ValueBudget::default();
            let mut values = Vec::new();
            for (index, value) in returned.into_iter().take(16).enumerate() {
                values.push(
                    budget
                        .normalize(&value, &format!("return value {}", index + 1), 0)
                        .map_err(mlua::Error::RuntimeError)?,
                );
            }
            let encoded = crate::json::stringify(&LuaValue::Array(values.clone()))
                .map_err(mlua::Error::RuntimeError)?;
            if encoded.len() > 24 * 1_024 {
                return Err(mlua::Error::RuntimeError(
                    "Lua return values exceed their aggregate byte limit".into(),
                ));
            }
            Ok(values)
        });
        let mut state = self.state.borrow_mut();
        let output = state.active_output.take().unwrap_or_default();
        state.active_output_bytes = 0;
        state.active_player_id = None;
        match normalized {
            Ok(values) => WebLuaExecutionResult { error: None, ok: true, output, values },
            Err(error) => WebLuaExecutionResult {
                error: Some(bounded_error(&error)),
                ok: false,
                output,
                values: Vec::new(),
            },
        }
    }
    fn now(&self) -> f64 {
        (self.state.borrow_mut().clock)()
    }
    fn record_work(&mut self, started: f64) {
        let elapsed = (self.now() - started).max(0.0);
        let mut state = self.state.borrow_mut();
        state.tick_work_ms += elapsed;
        state.maximum_tick_work_ms = state.maximum_tick_work_ms.max(state.tick_work_ms);
        state.note_budget();
    }
}
impl Drop for WebLuaRuntime {
    fn drop(&mut self) {
        self.close();
    }
}

impl RuntimeState {
    fn state_bytes(&self) -> usize {
        crate::json::stringify(&LuaValue::Object(self.state.clone()))
            .expect("Console state contains only normalized JSON values")
            .len()
    }
    fn set_state(&mut self, key: &str, value: LuaValue) -> Result<u64, String> {
        let mut candidate = self.state.clone();
        candidate.insert(key.into(), value);
        if candidate.len() > 256
            || crate::json::stringify(&LuaValue::Object(candidate.clone()))?.len() > 64 * 1_024
        {
            return Err("Lua state exceeds its size limit".into());
        }
        self.state = candidate;
        self.state_revision += 1;
        Ok(self.state_revision)
    }
    fn clear_state(&mut self) -> bool {
        let changed = !self.state.is_empty();
        self.state = LuaObject::new();
        if changed {
            self.state_revision += 1;
        }
        changed
    }
    fn queue(&mut self, command: WebLuaCommand) -> Result<(), String> {
        if self.active_player_id.is_none() {
            return Err("Lua mutation requires active authority execution".into());
        }
        if self.commands.len() >= 256 {
            return Err("Lua command limit reached for this tick".into());
        }
        self.commands.push(command);
        Ok(())
    }
    fn log(&mut self, level: &'static str, event: &'static str, detail: String) {
        self.logs.push(WebLuaRuntimeLog { level, event, detail });
    }
    fn note_budget(&mut self) {
        if self.tick_work_ms > 4.0 && self.budget_warning_tick != Some(self.current_tick) {
            self.budget_warning_tick = Some(self.current_tick);
            self.budget_exceeded_count += 1;
            self.log(
                "warning",
                "lua.tick_budget_exceeded",
                format!("Lua callback work exceeded 4 ms at tick {}.", self.current_tick),
            );
        }
    }
}
fn failed(message: &str) -> WebLuaExecutionResult {
    WebLuaExecutionResult {
        error: Some(message.into()),
        ok: false,
        output: Vec::new(),
        values: Vec::new(),
    }
}
fn check_deadline(deadline: &Cell<Option<Instant>>) -> mlua::Result<VmState> {
    if deadline.get().is_some_and(|deadline| Instant::now() > deadline) {
        return Err(mlua::Error::RuntimeError("thread timeout exceeded".into()));
    }
    Ok(VmState::Continue)
}
fn authority(lua: &Lua) -> mlua::Result<Option<String>> {
    let provider: Table = lua.named_registry_value(PROVIDER)?;
    provider.get::<Function>("authority")?.call(())
}
fn frame(lua: &Lua, shared: &Rc<RefCell<RuntimeState>>) -> Result<WebLuaFrameState, String> {
    if let Some(frame) = &shared.borrow().frame {
        return Ok(frame.clone());
    }
    let provider: Table =
        lua.named_registry_value(PROVIDER).map_err(|error| bounded_error(&error))?;
    let encoded = provider
        .get::<Function>("frame")
        .and_then(|function| function.call::<String>(()))
        .map_err(|error| bounded_error(&error))?;
    let frame: WebLuaFrameState =
        serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
    shared.borrow_mut().frame = Some(frame.clone());
    Ok(frame)
}
