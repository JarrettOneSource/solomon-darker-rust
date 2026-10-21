//! Ordered events, timers, reducer circuits and named RNG from `mod-rule-engine.ts`.
//! The native owner holds each real VM directly; closing/unloading a mod drops
//! its timer subscriptions and closes that VM before returning.

mod program;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use solomon_sim::modding::definition::CompiledWebLuaMod;
use solomon_sim::modding::runtime::*;
use solomon_sim::modding::{LuaObject, LuaValue};

use self::program::{EvaluationState, RuleProgram, evaluate_rule, intent};
use crate::definition::runtime::WebLuaDefinitionRuntime;
use crate::json::SourceCollator;
use crate::state::{ModStateStore, validate_schema_value};

pub struct ModRuleEngineOptions {
    pub failure_threshold: u64,
    pub tick_budget_ms: f64,
    pub ticks_per_second: u32,
}
impl Default for ModRuleEngineOptions {
    fn default() -> Self {
        Self { failure_threshold: 3, tick_budget_ms: 4.0, ticks_per_second: 100 }
    }
}

struct RegisteredMod {
    compiled: Arc<CompiledWebLuaMod>,
    runtime: WebLuaDefinitionRuntime,
    program: RuleProgram,
    reducers: Vec<ModReducerHealthCheckpoint>,
}

pub struct ModRuleEngine {
    options: ModRuleEngineOptions,
    mods: Vec<RegisteredMod>,
    now: Box<dyn FnMut() -> f64>,
    state: ModStateStore,
    next_intent_sequence: u64,
    next_timer_id: u64,
    timers: Vec<ModRuleTimerCheckpoint>,
    traces: Vec<ModReducerTrace>,
    collator: SourceCollator,
}

impl ModRuleEngine {
    pub fn new(options: ModRuleEngineOptions) -> Result<Self, String> {
        let started = Instant::now();
        Self::with_clock(options, move || started.elapsed().as_secs_f64() * 1_000.0)
    }

    pub fn with_clock(
        options: ModRuleEngineOptions,
        now: impl FnMut() -> f64 + 'static,
    ) -> Result<Self, String> {
        if options.ticks_per_second == 0 {
            return Err("mod rule tick rate is invalid".into());
        }
        Ok(Self {
            options,
            mods: Vec::new(),
            now: Box::new(now),
            state: ModStateStore::new(),
            next_intent_sequence: 1,
            next_timer_id: 1,
            timers: Vec::new(),
            traces: Vec::new(),
            collator: SourceCollator::default(),
        })
    }

    pub fn state(&mut self) -> &mut ModStateStore {
        &mut self.state
    }

    pub fn register(
        &mut self,
        compiled: CompiledWebLuaMod,
        mut runtime: WebLuaDefinitionRuntime,
    ) -> Result<(), String> {
        let mod_id = &compiled.identity.id;
        if self.mods.iter().any(|package| package.compiled.identity.id == *mod_id) {
            runtime.close();
            return Err(format!("mod rule engine already registered {mod_id}"));
        }
        let mut reducers = Vec::new();
        for reducer in &compiled.reducers {
            if runtime.reducer(&reducer.key).is_none() {
                return Err(format!(
                    "compiled reducer has no live callback: {mod_id}:{}",
                    reducer.key
                ));
            }
            reducers.push(ModReducerHealthCheckpoint {
                disabled: false,
                failures: 0,
                key: reducer.key.clone(),
                mod_id: mod_id.clone(),
            });
        }
        let program = RuleProgram::new(&compiled)?;
        self.mods.push(RegisteredMod { compiled: Arc::new(compiled), runtime, program, reducers });
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<ModRuleEngineCheckpoint, String> {
        Ok(ModRuleEngineCheckpoint {
            next_intent_sequence: self.next_intent_sequence,
            next_timer_id: self.next_timer_id,
            reducer_health: self.reducer_diagnostics(),
            state: self.state.snapshot(None)?,
            timers: self.timers.clone(),
        })
    }

    pub fn restore(&mut self, checkpoint: &ModRuleEngineCheckpoint) -> Result<(), String> {
        let mut definitions = Vec::new();
        for package in &self.mods {
            for reducer in &package.reducers {
                definitions.push(cell_definition(package, &reducer.key)?);
            }
        }
        let mods = &mut self.mods;
        self.state.restore(&checkpoint.state, &definitions, |definition, version, value| {
            mods.iter_mut()
                .find(|package| package.compiled.identity.id == definition.mod_id)
                .expect("registered reducer owns its migration VM")
                .runtime
                .invoke_migration(&definition.key, version, value)
        })?;
        self.restore_timers(checkpoint)?;
        self.restore_reducer_health(&checkpoint.reducer_health)
    }

    pub fn rollback(&mut self, checkpoint: &ModRuleEngineCheckpoint) -> Result<(), String> {
        self.state.rollback(&checkpoint.state)?;
        self.restore_timers(checkpoint)?;
        self.restore_reducer_health(&checkpoint.reducer_health)
    }

    pub fn due_timer_ids(&self, tick: u64) -> Result<Vec<u64>, String> {
        validate_tick(tick)?;
        let mut timers: Vec<_> =
            self.timers.iter().filter(|timer| timer.due_tick <= tick).collect();
        timers.sort_by_key(|timer| (timer.due_tick, timer.id));
        Ok(timers.into_iter().take(64).map(|timer| timer.id).collect())
    }

    pub fn cancel_timer(&mut self, id: u64) {
        self.timers.retain(|timer| timer.id != id);
    }

    pub fn fire_timer(&mut self, id: u64, tick: u64) -> Result<ModScheduledRuleResult, String> {
        validate_tick(tick)?;
        let index = self
            .timers
            .iter()
            .position(|timer| timer.id == id && timer.due_tick <= tick)
            .ok_or_else(|| format!("mod rule timer is unavailable: {id}"))?;
        let timer = self.timers.remove(index);
        let package = self
            .mods
            .iter()
            .find(|package| package.compiled.identity.id == timer.mod_id)
            .ok_or_else(|| format!("mod rule timer definition is unavailable: {id}"))?;
        let node = node_index(&timer.node_id)
            .filter(|index| *index < package.program.nodes.len())
            .ok_or_else(|| format!("mod rule timer definition is unavailable: {id}"))?;
        let input = ModRuleDispatchInput {
            context: timer.context.clone(),
            event: timer.event.clone(),
            payload: timer.payload.clone(),
            scope: timer.scope.clone(),
            tick,
        };
        let started = (self.now)();
        let mut errors = Vec::new();
        let mut intents = Vec::new();
        let result = evaluate_rule(
            &package.program,
            node,
            &input,
            &mut EvaluationState {
                mod_id: &timer.mod_id,
                next_intent_sequence: &mut self.next_intent_sequence,
                next_timer_id: &mut self.next_timer_id,
                timers: &mut self.timers,
                ticks_per_second: self.options.ticks_per_second,
            },
        );
        match result {
            Ok(values) => {
                intents = values;
                if let Some(interval) = timer.interval_ticks
                    && timer.remaining > 1
                {
                    self.timers.push(ModRuleTimerCheckpoint {
                        due_tick: timer.due_tick + interval,
                        remaining: timer.remaining - 1,
                        ..timer.clone()
                    });
                }
            }
            Err(error) => errors.push(format!("{}:timer.{}: {error}", timer.mod_id, timer.id)),
        }
        Ok(ModScheduledRuleResult {
            context: timer.context,
            result: ModRuleDispatchResult {
                budget_exceeded: (self.now)() - started > self.options.tick_budget_ms,
                errors,
                intents,
                invocations: 1,
            },
            scope: timer.scope,
        })
    }

    pub fn dispatch(
        &mut self,
        input: &ModRuleDispatchInput,
    ) -> Result<ModRuleDispatchResult, String> {
        validate_tick(input.tick)?;
        let started = (self.now)();
        let mut result = ModRuleDispatchResult::default();
        let mut order: Vec<_> = (0..self.mods.len()).collect();
        order.sort_by(|left, right| {
            self.collator.compare(
                &self.mods[*left].compiled.identity.id,
                &self.mods[*right].compiled.identity.id,
            )
        });
        for index in order {
            let compiled = Arc::clone(&self.mods[index].compiled);
            let rules: Result<Vec<ModIntent>, String> = (|| {
                let package = &self.mods[index];
                let mut values = Vec::new();
                let mut execution = EvaluationState {
                    mod_id: &compiled.identity.id,
                    next_intent_sequence: &mut self.next_intent_sequence,
                    next_timer_id: &mut self.next_timer_id,
                    timers: &mut self.timers,
                    ticks_per_second: self.options.ticks_per_second,
                };
                for &root in &package.program.roots {
                    values.extend(evaluate_rule(&package.program, root, input, &mut execution)?);
                }
                if result.intents.len() + values.len() > 256 {
                    return Err("mod intent limit reached for one dispatch".into());
                }
                Ok(values)
            })();
            match rules {
                Ok(values) => result.intents.extend(values),
                Err(error) => {
                    result.errors.push(format!("{}:rules: {error}", compiled.identity.id))
                }
            }
            for reducer_index in 0..self.mods[index].reducers.len() {
                let reducer = &self.mods[index].reducers[reducer_index];
                let definition = compiled
                    .reducers
                    .iter()
                    .find(|definition| definition.key == reducer.key)
                    .expect("registered reducer has compiled metadata");
                if reducer.disabled
                    || !definition.on.contains(&input.event)
                    || definition.scope != input.scope.kind
                {
                    continue;
                }
                if result.invocations >= 64 || (self.now)() - started > self.options.tick_budget_ms
                {
                    result.budget_exceeded = true;
                    break;
                }
                result.invocations += 1;
                let key = reducer.key.clone();
                let registration = self.mods[index]
                    .runtime
                    .reducer(&key)
                    .expect("registered reducer has a real callback");
                // Source initializes the cell outside the reducer catch. An
                // invalid schema/budget here is a host error, not a circuit hit.
                let cell = self
                    .state
                    .cell(cell_definition(&self.mods[index], &key)?, input.scope.clone())?;
                let invocation: Result<Vec<ModIntent>, String> = (|| {
                    let state = self.state.read(&cell)?;
                    let mut context = input.context.clone();
                    context.insert("tick".into(), LuaValue::Number(input.tick as f64));
                    let mut random = NamedRandom::new(
                        &compiled.identity.id,
                        &key,
                        &input.scope,
                        &input.event,
                        input.tick,
                    );
                    let value = self.mods[index].runtime.invoke_reducer_with_random(
                        &key,
                        &state,
                        &input.payload,
                        &LuaValue::Object(context),
                        move |name| random.draw(name),
                    )?;
                    let next_state = validate_schema_value(
                        &registration.state,
                        &value.state,
                        &format!("{}:{key} state", compiled.identity.id),
                    )?;
                    let next_intents = normalize_reducer_intents(
                        &compiled,
                        &self.mods,
                        &key,
                        &input.scope,
                        &value.intents,
                        &mut self.next_intent_sequence,
                    )?;
                    if result.intents.len() + next_intents.len() > 256 {
                        return Err("mod intent limit reached for one dispatch".into());
                    }
                    self.state.set(&cell, next_state)?;
                    Ok(next_intents)
                })();
                match invocation {
                    Ok(intents) => {
                        self.mods[index].reducers[reducer_index].failures = 0;
                        self.trace(ModReducerTrace {
                            disabled: false,
                            error: None,
                            event: input.event.clone(),
                            failures: 0,
                            intents: intents.len() as u32,
                            key,
                            mod_id: compiled.identity.id.clone(),
                            outcome: ModReducerTraceOutcome::Ok,
                            scope: input.scope.clone(),
                            tick: input.tick,
                        });
                        result.intents.extend(intents);
                    }
                    Err(error) => {
                        let reducer = &mut self.mods[index].reducers[reducer_index];
                        reducer.failures += 1;
                        if reducer.failures >= self.options.failure_threshold {
                            reducer.disabled = true
                        }
                        let disabled = reducer.disabled;
                        let failures = reducer.failures;
                        result.errors.push(format!("{}:{key}: {error}", compiled.identity.id));
                        self.trace(ModReducerTrace {
                            disabled,
                            error: Some(error),
                            event: input.event.clone(),
                            failures,
                            intents: 0,
                            key,
                            mod_id: compiled.identity.id.clone(),
                            outcome: ModReducerTraceOutcome::Error,
                            scope: input.scope.clone(),
                            tick: input.tick,
                        });
                    }
                }
            }
            if result.budget_exceeded {
                break;
            }
        }
        Ok(result)
    }

    pub fn diagnostic(&self, mod_id: &str, key: &str) -> Option<&ModReducerHealthCheckpoint> {
        self.mods
            .iter()
            .find(|package| package.compiled.identity.id == mod_id)?
            .reducers
            .iter()
            .find(|reducer| reducer.key == key)
    }

    pub fn reducer_diagnostics(&self) -> Vec<ModReducerHealthCheckpoint> {
        let mut rows: Vec<_> =
            self.mods.iter().flat_map(|package| package.reducers.iter().cloned()).collect();
        rows.sort_by(|left, right| {
            self.collator
                .compare(&left.mod_id, &right.mod_id)
                .then_with(|| self.collator.compare(&left.key, &right.key))
        });
        rows
    }
    pub fn traces(&self) -> &[ModReducerTrace] {
        &self.traces
    }

    pub fn close_run(&mut self, run_id: &str) -> Result<(), String> {
        if run_id.is_empty() {
            return Ok(());
        }
        let suffix = format!(":{run_id}");
        let matches = |scope: &ModStateScope| scope.id == run_id || scope.id.ends_with(&suffix);
        self.state.close_scopes(matches)?;
        self.timers.retain(|timer| !matches(&timer.scope));
        Ok(())
    }
    pub fn close_mod(&mut self, mod_id: &str) -> bool {
        let Some(index) =
            self.mods.iter().position(|package| package.compiled.identity.id == mod_id)
        else {
            return false;
        };
        let mut package = self.mods.remove(index);
        package.runtime.close();
        self.timers.retain(|timer| timer.mod_id != mod_id);
        true
    }
    pub fn close(&mut self) {
        for package in &mut self.mods {
            package.runtime.close();
        }
        self.mods.clear();
        self.timers.clear();
        self.state.close();
        self.traces.clear();
    }
    fn trace(&mut self, trace: ModReducerTrace) {
        self.traces.push(trace);
        if self.traces.len() > 256 {
            self.traces.remove(0);
        }
    }

    fn restore_timers(&mut self, checkpoint: &ModRuleEngineCheckpoint) -> Result<(), String> {
        if checkpoint.next_intent_sequence == 0
            || checkpoint.next_intent_sequence > 9_007_199_254_740_991
            || checkpoint.next_timer_id == 0
            || checkpoint.next_timer_id > 9_007_199_254_740_991
            || checkpoint.timers.len() > 256
        {
            return Err("mod rule checkpoint is invalid".into());
        }
        let mut ids = std::collections::HashSet::new();
        for timer in &checkpoint.timers {
            let valid_node = self
                .mods
                .iter()
                .find(|package| package.compiled.identity.id == timer.mod_id)
                .is_some_and(|package| {
                    node_index(&timer.node_id)
                        .is_some_and(|index| index < package.program.nodes.len())
                });
            if !valid_node
                || !ids.insert(timer.id)
                || timer.id == 0
                || timer.id > 9_007_199_254_740_991
                || timer.due_tick > 9_007_199_254_740_991
                || timer.remaining == 0
                || timer.remaining > 9_007_199_254_740_991
                || timer
                    .interval_ticks
                    .is_some_and(|tick| tick == 0 || tick > 9_007_199_254_740_991)
            {
                return Err("mod rule checkpoint contains an invalid timer".into());
            }
        }
        self.timers = checkpoint.timers.clone();
        self.next_intent_sequence = checkpoint.next_intent_sequence;
        self.next_timer_id = checkpoint.next_timer_id;
        Ok(())
    }

    fn restore_reducer_health(
        &mut self,
        checkpoint: &[ModReducerHealthCheckpoint],
    ) -> Result<(), String> {
        let expected: usize = self.mods.iter().map(|package| package.reducers.len()).sum();
        if checkpoint.len() != expected {
            return Err("mod reducer health checkpoint is invalid".into());
        }
        for package in &mut self.mods {
            for reducer in &mut package.reducers {
                let row = checkpoint
                    .iter()
                    .rev()
                    .find(|row| {
                        row.mod_id == package.compiled.identity.id && row.key == reducer.key
                    })
                    .ok_or("mod reducer health checkpoint is invalid")?;
                if row.failures > 9_007_199_254_740_991
                    || row.disabled != (row.failures >= self.options.failure_threshold)
                {
                    return Err("mod reducer health checkpoint is invalid".into());
                }
                reducer.failures = row.failures;
                reducer.disabled = row.disabled;
            }
        }
        Ok(())
    }
}

impl Drop for ModRuleEngine {
    fn drop(&mut self) {
        self.close();
    }
}

fn cell_definition(package: &RegisteredMod, key: &str) -> Result<ModStateCellDefinition, String> {
    let registration = package.runtime.reducer(key).ok_or_else(|| {
        format!("compiled reducer has no live callback: {}:{key}", package.compiled.identity.id)
    })?;
    Ok(ModStateCellDefinition {
        default_value: None,
        key: key.into(),
        migration_versions: (1..registration.schema_version).collect(),
        mod_id: package.compiled.identity.id.clone(),
        schema: registration.state,
        schema_version: registration.schema_version,
        scope: registration.scope,
    })
}
fn node_index(value: &str) -> Option<usize> {
    let suffix = value.strip_prefix('r')?;
    let index: usize = suffix.parse().ok()?;
    // Source looks up the exact generated ID in a Map; accepting an alternate
    // numeric spelling would admit a timer that its checkpoint rejects.
    (suffix == index.to_string()).then_some(index)
}
fn validate_tick(tick: u64) -> Result<(), String> {
    if tick > 9_007_199_254_740_991 { Err("mod rule tick is invalid".into()) } else { Ok(()) }
}

fn normalize_reducer_intents(
    compiled: &CompiledWebLuaMod,
    mods: &[RegisteredMod],
    reducer_key: &str,
    scope: &ModStateScope,
    value: &LuaValue,
    sequence: &mut u64,
) -> Result<Vec<ModIntent>, String> {
    let values = match value {
        LuaValue::Array(values) => values.iter().collect::<Vec<_>>(),
        LuaValue::Object(fields) if fields.is_empty() => Vec::new(),
        _ => vec![value],
    };
    if values.len() > 64 {
        return Err("advanced reducer returned more than 64 intents".into());
    }
    let mut output = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        let row = value
            .as_object()
            .filter(|fields| {
                fields.get("kind").and_then(LuaValue::as_str) == Some("intent-definition")
            })
            .ok_or_else(|| format!("advanced reducer intent {index} is invalid"))?;
        let kind = row
            .get("intentKind")
            .and_then(LuaValue::as_str)
            .ok_or_else(|| format!("advanced reducer intent {index} is invalid"))?;
        let fields = resolve_reducer_references(
            row.get("fields").unwrap_or(&LuaValue::Null),
            compiled,
            mods,
        )?;
        let current = *sequence;
        *sequence += 1;
        let fields = fields.as_object().ok_or_else(|| {
            format!("{}:{reducer_key} {kind} intent fields must be an object", compiled.identity.id)
        })?;
        output.push(intent(&compiled.identity.id, kind, fields, reducer_key, scope, current)?);
    }
    Ok(output)
}

fn resolve_reducer_references(
    value: &LuaValue,
    compiled: &CompiledWebLuaMod,
    mods: &[RegisteredMod],
) -> Result<LuaValue, String> {
    match value {
        LuaValue::Array(values) => {
            let mut output = Vec::new();
            for value in values {
                output.push(resolve_reducer_references(value, compiled, mods)?);
            }
            Ok(LuaValue::Array(output))
        }
        LuaValue::Object(fields) => {
            if fields.get("kind").and_then(LuaValue::as_str) == Some("content-reference") {
                let mod_id =
                    fields.get("modId").and_then(LuaValue::as_str).unwrap_or(&compiled.identity.id);
                let key = fields.get("key").and_then(LuaValue::as_str);
                let target_kind = fields.get("targetKind").and_then(LuaValue::as_str);
                let target = mods
                    .iter()
                    .find(|package| package.compiled.identity.id == mod_id)
                    .and_then(|package| {
                        package.compiled.content.iter().find(|row| {
                            Some(row.key.as_str()) == key
                                && Some(row.content_kind.as_str()) == target_kind
                        })
                    });
                let Some(target) = target else {
                    return Err(format!(
                        "advanced reducer reference is unavailable: {mod_id}:{}",
                        js_text(fields.get("key"))
                    ));
                };
                let mut reference = LuaObject::new();
                for (key, value) in [
                    ("contentId", LuaValue::String(target.content_id.clone())),
                    ("key", LuaValue::String(target.key.clone())),
                    ("kind", LuaValue::String("resolved-content-reference".into())),
                    ("modId", LuaValue::String(mod_id.into())),
                    ("targetKind", LuaValue::String(target.content_kind.as_str().into())),
                ] {
                    reference.insert(key.into(), value);
                }
                return Ok(LuaValue::Object(reference));
            }
            let mut output = LuaObject::new();
            for (key, value) in fields.iter() {
                output.insert(key.into(), resolve_reducer_references(value, compiled, mods)?);
            }
            Ok(LuaValue::Object(output))
        }
        _ => Ok(value.clone()),
    }
}

fn js_text(value: Option<&LuaValue>) -> String {
    match value {
        None => "undefined".into(),
        Some(LuaValue::Null) => "null".into(),
        Some(LuaValue::String(value)) => value.clone(),
        Some(LuaValue::Number(value)) => {
            crate::json::stringify(&LuaValue::Number(*value)).expect("runtime values are finite")
        }
        Some(LuaValue::Boolean(value)) => value.to_string(),
        Some(LuaValue::Array(values)) => {
            values.iter().map(|value| js_text(Some(value))).collect::<Vec<_>>().join(",")
        }
        Some(LuaValue::Object(_)) => "[object Object]".into(),
    }
}

struct NamedRandom {
    prefix: String,
    counters: BTreeMap<String, u64>,
}

impl NamedRandom {
    fn new(mod_id: &str, reducer_key: &str, scope: &ModStateScope, event: &str, tick: u64) -> Self {
        let prefix = format!(
            "{mod_id}\0{reducer_key}\0{}\0{}\0{event}\0{tick}\0",
            scope.kind.as_str(),
            scope.id,
        );
        Self { prefix, counters: BTreeMap::new() }
    }

    fn draw(&mut self, name: &str) -> Result<f64, String> {
        let counter = self.counters.entry(name.into()).or_default();
        let input = format!("{}{name}\0{counter}", self.prefix);
        *counter += 1;
        let mut hash = 0x811c_9dc5_u32;
        for byte in input.bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        Ok(f64::from(hash) / 4_294_967_296.0)
    }
}
