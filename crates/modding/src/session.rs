//! Verified live mod VMs and atomic event batches from `prepared-mod-session.ts`.

use solomon_sim::modding::definition::{CompiledWebLuaMod, WebLuaContentKind, WebLuaModIdentity};
use solomon_sim::modding::runtime::*;
use solomon_sim::modding::{LuaObject, LuaValue};

use crate::definition::compiler::{
    CompileWebLuaDefinitionOptions, WebLuaDefinitionDependency, WebLuaDefinitionDependencyContent,
    compile_web_lua_definition,
};
use crate::definition::runtime::{WebLuaDefinitionRuntime, WebLuaDefinitionRuntimeOptions};
use crate::intents::{ModIntentAdapter, ModIntentExecutionContext, execute_mod_intents};
use crate::rules::{ModRuleEngine, ModRuleEngineOptions};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedModSource {
    pub compiled: CompiledWebLuaMod,
    pub entry_script: String,
    pub entry_script_path: String,
    pub identity: WebLuaModIdentity,
}

pub struct PreparedModSession {
    rules: ModRuleEngine,
    verified: Vec<CompiledWebLuaMod>,
    closed: bool,
}

impl PreparedModSession {
    pub fn new(sources: &[PreparedModSource]) -> Result<Self, String> {
        let rules = ModRuleEngine::new(ModRuleEngineOptions::default())?;
        Self::prepare(sources, rules)
    }

    pub fn with_clock(
        sources: &[PreparedModSource],
        now: impl FnMut() -> f64 + 'static,
    ) -> Result<Self, String> {
        let rules = ModRuleEngine::with_clock(ModRuleEngineOptions::default(), now)?;
        Self::prepare(sources, rules)
    }

    fn prepare(sources: &[PreparedModSource], mut rules: ModRuleEngine) -> Result<Self, String> {
        let mut verified: Vec<CompiledWebLuaMod> = Vec::new();
        for source in sources {
            let mut runtime = WebLuaDefinitionRuntime::new(WebLuaDefinitionRuntimeOptions {
                entry_script: source.entry_script_path.clone(),
                identity: source.identity.clone(),
                scripts: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
            let definition =
                runtime.run(&source.entry_script).map_err(|error| error.to_string())?;
            let dependencies = verified
                .iter()
                .map(|package| WebLuaDefinitionDependency {
                    id: package.identity.id.clone(),
                    content: package
                        .content
                        .iter()
                        .map(|content| WebLuaDefinitionDependencyContent {
                            content_id: content.content_id.clone(),
                            content_kind: content.content_kind,
                            key: content.key.clone(),
                        })
                        .collect(),
                })
                .collect();
            let compiled = compile_web_lua_definition(
                &source.identity,
                &definition,
                &CompileWebLuaDefinitionOptions { dependencies, ..Default::default() },
            )
            .map_err(|error| error.to_string())?;
            if compiled.graph_sha256 != source.compiled.graph_sha256 {
                return Err(format!("compiled Web Lua graph changed for {}", source.identity.id));
            }
            verified.push(compiled.clone());
            rules.register(compiled, runtime)?;
        }
        Ok(Self { rules, verified, closed: false })
    }

    pub fn step(
        &mut self,
        input: &PreparedModStepInput,
        adapter: &mut dyn ModIntentAdapter,
    ) -> Result<PreparedModStepResult, String> {
        self.require_open()?;
        if input.tick > 9_007_199_254_740_991 {
            return Err("mod session tick is invalid".into());
        }
        let mut result = PreparedModStepResult {
            accepted: true,
            budget_exceeded: false,
            errors: Vec::new(),
            intents: Vec::new(),
            invocations: 0,
        };
        for timer_id in self.rules.due_timer_ids(input.tick)? {
            let checkpoint = self.rules.checkpoint()?;
            let scheduled = self.rules.fire_timer(timer_id, input.tick)?;
            result.intents.extend(scheduled.result.intents.clone());
            result.errors.extend(scheduled.result.errors);
            result.invocations += scheduled.result.invocations;
            result.budget_exceeded |= scheduled.result.budget_exceeded;
            let error = if scheduled.result.budget_exceeded {
                Some("scheduled mod budget exceeded".into())
            } else {
                execute_batch(
                    adapter,
                    &scheduled.result.intents,
                    scheduled.context,
                    scheduled.scope,
                    input.tick,
                )
            };
            if let Some(error) = error {
                self.rules.rollback(&checkpoint)?;
                self.rules.cancel_timer(timer_id);
                result.errors.push(error);
                result.accepted = false;
            }
            if result.budget_exceeded {
                break;
            }
        }
        let checkpoint = self.rules.checkpoint()?;
        let mut event_intents = Vec::new();
        for event in &input.events {
            if result.budget_exceeded {
                break;
            }
            let dispatched = self.rules.dispatch(&ModRuleDispatchInput {
                context: event.context.clone(),
                event: event.event.clone(),
                payload: event.payload.clone(),
                scope: event.scope.clone(),
                tick: input.tick,
            })?;
            event_intents.extend(dispatched.intents);
            result.errors.extend(dispatched.errors);
            result.invocations += dispatched.invocations;
            result.budget_exceeded |= dispatched.budget_exceeded;
        }
        // Source checks common scope after dispatch, even for an empty batch.
        let scope = common_scope(&input.events)?;
        let error = if result.budget_exceeded && !input.events.is_empty() {
            Some("mod event budget exceeded".into())
        } else {
            execute_batch(
                adapter,
                &event_intents,
                input.events.first().map(|event| event.context.clone()).unwrap_or_default(),
                scope,
                input.tick,
            )
        };
        if let Some(error) = error {
            self.rules.rollback(&checkpoint)?;
            result.errors.push(error);
            result.accepted = false;
        }
        result.intents.extend(event_intents);
        Ok(result)
    }

    pub fn act(
        &mut self,
        input: &PreparedModActionInput,
        adapter: &mut dyn ModIntentAdapter,
    ) -> Result<PreparedModStepResult, String> {
        let event = content_action_event(&input.action, &input.payload, &self.verified)?;
        let generic_event = format!("action.{}", input.action);
        let mut names = vec![event];
        if names[0] != generic_event {
            names.push(generic_event);
        }
        let mut context = input.context.clone();
        context.insert("action_kind".into(), LuaValue::String(input.action.clone()));
        context.insert("request_id".into(), LuaValue::Number(input.request_id as f64));
        let events = names
            .into_iter()
            .map(|event| PreparedModEvent {
                context: context.clone(),
                event,
                payload: input.payload.clone(),
                scope: input.scope.clone(),
            })
            .collect();
        self.step(&PreparedModStepInput { events, tick: input.tick }, adapter)
    }

    pub fn catalog(&self) -> Result<&[CompiledWebLuaMod], String> {
        self.require_open()?;
        Ok(&self.verified)
    }

    pub fn checkpoint(&self) -> Result<PreparedModCheckpoint, String> {
        self.require_open()?;
        Ok(PreparedModCheckpoint {
            rules: self.rules.checkpoint()?,
            graph_sha256: self
                .verified
                .iter()
                .map(|package| package.graph_sha256.clone())
                .collect(),
        })
    }

    pub fn project(&self, viewer_id: &str) -> Result<PreparedModProjection, String> {
        self.require_open()?;
        // Source exposes all reducer cells here; semantic/private content is a
        // separate projection with its own viewer filtering.
        Ok(PreparedModProjection {
            catalog: self.verified.clone(),
            state: self.rules.checkpoint()?.state,
            viewer_id: viewer_id.into(),
        })
    }

    pub fn restore(&mut self, checkpoint: &PreparedModCheckpoint) -> Result<(), String> {
        self.require_open()?;
        let expected: Vec<_> =
            self.verified.iter().map(|package| package.graph_sha256.clone()).collect();
        if checkpoint.graph_sha256 != expected {
            return Err("mod checkpoint graph does not match the prepared session".into());
        }
        self.rules.restore(&checkpoint.rules)
    }

    pub fn close_run(&mut self, run_id: &str) -> Result<(), String> {
        self.require_open()?;
        self.rules.close_run(run_id)
    }

    pub fn reducer_diagnostics(&self) -> Result<Vec<ModReducerHealthCheckpoint>, String> {
        self.require_open()?;
        Ok(self.rules.reducer_diagnostics())
    }

    pub fn reducer_traces(&self) -> Result<&[ModReducerTrace], String> {
        self.require_open()?;
        Ok(self.rules.traces())
    }

    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.rules.close();
        }
    }

    fn require_open(&self) -> Result<(), String> {
        if self.closed { Err("prepared mod session is closed".into()) } else { Ok(()) }
    }
}

fn execute_batch(
    adapter: &mut dyn ModIntentAdapter,
    intents: &[ModIntent],
    context: LuaObject,
    scope: ModStateScope,
    tick: u64,
) -> Option<String> {
    if intents.is_empty() {
        return None;
    }
    let execution =
        execute_mod_intents(adapter, intents, &ModIntentExecutionContext { context, scope, tick });
    if execution.accepted {
        None
    } else {
        Some(execution.error.unwrap_or_else(|| "mod intent transaction failed".into()))
    }
}

fn common_scope(events: &[PreparedModEvent]) -> Result<ModStateScope, String> {
    let scope = events.first().map(|event| event.scope.clone()).unwrap_or(ModStateScope {
        id: "session".into(),
        kind: solomon_sim::modding::definition::WebLuaScopeKind::Session,
    });
    if events.iter().any(|event| event.scope != scope) {
        return Err("one mod session step may not mix state scopes".into());
    }
    Ok(scope)
}

fn content_action_event(
    action: &str,
    payload: &LuaValue,
    mods: &[CompiledWebLuaMod],
) -> Result<String, String> {
    let kinds: &[WebLuaContentKind] = match action {
        "content.use" => &[WebLuaContentKind::Item, WebLuaContentKind::Potion],
        "content.pickup" => &[WebLuaContentKind::Powerup],
        "content.cast" => &[WebLuaContentKind::Spell],
        _ => return Ok(format!("action.{action}")),
    };
    let fields =
        payload.as_object().ok_or_else(|| format!("{action} requires a content payload"))?;
    let content_id = fields
        .get("content_id")
        .and_then(LuaValue::as_str)
        .filter(|value| {
            (1..=19).contains(&value.len())
                && value.as_bytes()[0] != b'0'
                && value.bytes().all(|byte| byte.is_ascii_digit())
        })
        .ok_or_else(|| format!("{action} content identity is invalid"))?;
    let available = mods
        .iter()
        .flat_map(|package| &package.content)
        .any(|content| content.content_id == content_id && kinds.contains(&content.content_kind));
    if !available {
        return Err(format!("{action} content is unavailable: {content_id}"));
    }
    Ok(format!("action.{action}.{content_id}"))
}
