//! Actual Lua 5.4 definition VM from `web-lua-definition-runtime.ts`.
//! The VM retains reducer/migration functions; its compiled graph and saved
//! state remain ordinary values owned by the canonical prepared host.

mod bridge;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::{Function, HookTriggers, Lua, MultiValue, VmState};
use solomon_sim::modding::LuaValue;
use solomon_sim::modding::definition::*;

use super::assembly::{WebLuaAssemblyInput, WebLuaExplicitLists, assemble_web_lua_definition};
use super::error::{
    WebLuaDefinitionError, WebLuaDefinitionErrorCode as Code, WebLuaDefinitionIssue as Issue,
};
use super::scripts::{read_web_lua_script_bundle, validate_web_lua_script_set};
use super::values::{clone_lua_value, lua_payload};

const DEFINITION_TIMEOUT_MS: u64 = 250;
const CALLBACK_TIMEOUT_MS: u64 = 2;
const VM_MEMORY_BYTES: usize = 16 * 1_024 * 1_024;

#[derive(Clone, Debug)]
pub struct WebLuaDefinitionRuntimeOptions {
    /// Package entry path, normally `scripts/main.lua`, rather than its text.
    pub entry_script: String,
    pub identity: WebLuaModIdentity,
    pub scripts: Vec<(String, String)>,
}

#[derive(Debug)]
pub enum WebLuaDefinitionRuntimeError {
    Definition(WebLuaDefinitionError),
    Message(String),
}

impl fmt::Display for WebLuaDefinitionRuntimeError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Definition(error) => error.fmt(output),
            Self::Message(message) => output.write_str(message),
        }
    }
}

impl std::error::Error for WebLuaDefinitionRuntimeError {}

#[derive(Clone, Debug, PartialEq)]
pub struct WebLuaReducerResult {
    pub intents: LuaValue,
    pub state: LuaValue,
}

pub struct WebLuaDefinitionRuntime {
    lua: Option<Lua>,
    registry: Rc<RefCell<Registry>>,
    deadline: Rc<Cell<Option<Instant>>>,
}

struct RegisteredReducer {
    definition: WebLuaReducerDefinition,
    callback: Function,
    migrations: BTreeMap<u64, Function>,
}

struct Registry {
    identity: WebLuaModIdentity,
    entry_script: String,
    scripts: Vec<(String, String)>,
    assets: Vec<WebLuaAssetDefinition>,
    content: Vec<WebLuaContentDefinition>,
    rules: Vec<WebLuaRuleDefinition>,
    reducers: Vec<RegisteredReducer>,
    explicit: Option<WebLuaExplicitLists>,
    definition: Option<WebLuaModDefinition>,
    issues: Vec<Issue>,
    include_loaded: HashSet<String>,
    include_loading: HashSet<String>,
    nodes: u32,
    site: WebLuaDefinitionSource,
    reducer_running: bool,
    reducer_result_submitted: bool,
    reducer_result: Option<(mlua::Value, mlua::Value)>,
    logs: Vec<String>,
}

impl WebLuaDefinitionRuntime {
    pub fn new(options: WebLuaDefinitionRuntimeOptions) -> Result<Self, String> {
        validate_web_lua_script_set(&options.scripts)?;
        let entry_script = options.entry_script;
        let registry = Rc::new(RefCell::new(Registry {
            identity: options.identity,
            entry_script: entry_script.clone(),
            scripts: options
                .scripts
                .into_iter()
                .filter(|(path, _)| *path != entry_script)
                .collect(),
            assets: Vec::new(),
            content: Vec::new(),
            rules: Vec::new(),
            reducers: Vec::new(),
            explicit: None,
            definition: None,
            issues: Vec::new(),
            include_loaded: HashSet::new(),
            include_loading: HashSet::new(),
            nodes: 0,
            site: WebLuaDefinitionSource {
                column: Some(0),
                file: entry_script,
                line: Some(0),
                node: None,
            },
            reducer_running: false,
            reducer_result_submitted: false,
            reducer_result: None,
            logs: Vec::new(),
        }));
        let lua = Lua::new();
        lua.set_memory_limit(VM_MEMORY_BYTES).map_err(|error| error.to_string())?;
        let deadline = Rc::new(Cell::new(None));
        install_deadline(&lua, Rc::clone(&deadline)).map_err(|error| error.to_string())?;
        bridge::install(&lua, Rc::clone(&registry)).map_err(|error| error.to_string())?;
        lua.load(include_str!("bootstrap.lua"))
            .set_name("@web-lua-definition-bootstrap.lua")
            .exec()
            .map_err(|error| error.to_string())?;
        Ok(Self { lua: Some(lua), registry, deadline })
    }

    pub fn run(&mut self, code: &str) -> Result<WebLuaModDefinition, WebLuaDefinitionRuntimeError> {
        let lua = self.lua.as_ref().ok_or_else(|| {
            WebLuaDefinitionRuntimeError::Message("Web Lua definition runtime is closed".into())
        })?;
        if self.registry.borrow().definition.is_some() {
            return Err(WebLuaDefinitionRuntimeError::Message(
                "Web Lua definition entrypoint already ran".into(),
            ));
        }
        self.registry.borrow_mut().reset();
        let bundle = self.registry.borrow_mut().load_bundle(code);
        if let Err(message) = bundle {
            let mut registry = self.registry.borrow_mut();
            let source = registry.entry_source();
            registry.issues.push(Issue::new(Code::EScript, "sd.include", message, Some(&source)));
        }
        if self.registry.borrow().issues.is_empty() {
            let path = self.registry.borrow().entry_script.clone();
            let function = lua.load(code).set_name(format!("@{path}")).into_function();
            let returned = match function {
                Ok(function) => {
                    self.deadline
                        .set(Some(Instant::now() + Duration::from_millis(DEFINITION_TIMEOUT_MS)));
                    let returned = function.call::<MultiValue>(());
                    self.deadline.set(None);
                    returned
                }
                Err(error) => Err(error),
            };
            match returned {
                Ok(returned) => {
                    let meaningful: Vec<_> = returned
                        .into_iter()
                        .filter(|value| !matches!(value, mlua::Value::Nil))
                        .collect();
                    let receipt = meaningful.len() == 1
                        && meaningful[0].as_table().is_some_and(|table| {
                            table
                                .raw_get::<String>("kind")
                                .is_ok_and(|kind| kind == "mod-definition-receipt")
                        });
                    if !meaningful.is_empty() && !receipt {
                        let mut registry = self.registry.borrow_mut();
                        let source = registry.entry_source();
                        registry.issues.push(Issue::new(Code::EGraph, "definition", "the entry script returned an unexpected value; end it with return sd.mod({...}) or leave the return out", Some(&source)));
                    }
                }
                Err(error) => self.registry.borrow_mut().record_script_error(&error),
            }
        }
        if self.registry.borrow().issues.is_empty() {
            self.registry.borrow_mut().assemble();
        }
        let mut registry = self.registry.borrow_mut();
        if !registry.issues.is_empty() || registry.definition.is_none() {
            let issues = registry.issues.clone();
            registry.reset();
            return Err(WebLuaDefinitionRuntimeError::Definition(WebLuaDefinitionError::new(
                issues,
            )));
        }
        Ok(registry.definition.as_ref().expect("successful assembly owns a definition").clone())
    }

    pub fn reducer(&self, key: &str) -> Option<WebLuaReducerDefinition> {
        self.registry
            .borrow()
            .reducers
            .iter()
            .find(|entry| entry.definition.key == key)
            .map(|entry| entry.definition.clone())
    }

    pub fn invoke_reducer(
        &mut self,
        key: &str,
        state: &LuaValue,
        event: &LuaValue,
        context: &LuaValue,
    ) -> Result<WebLuaReducerResult, String> {
        self.invoke_reducer_call(key, |lua, callback| {
            callback.call::<()>((
                lua_payload(lua, state)?,
                lua_payload(lua, event)?,
                lua_payload(lua, context)?,
            ))
        })
    }

    /// The Source rule host adds one named random function to each context.
    /// Its owned closure may remain in a Lua upvalue after this invocation.
    pub(crate) fn invoke_reducer_with_random(
        &mut self,
        key: &str,
        state: &LuaValue,
        event: &LuaValue,
        context: &LuaValue,
        mut random: impl FnMut(&str) -> Result<f64, String> + 'static,
    ) -> Result<WebLuaReducerResult, String> {
        self.invoke_reducer_call(key, move |lua, callback| {
            let context = lua_payload(lua, context)?;
            let table = context.as_table().ok_or_else(|| {
                mlua::Error::RuntimeError("mod reducer context must be an object".into())
            })?;
            let random = lua.create_function_mut(move |_, name: mlua::Value| {
                let name = match name {
                    mlua::Value::String(value) => value.to_string_lossy(),
                    _ => {
                        return Err(mlua::Error::RuntimeError(
                            "named random stream requires 1..128 text characters".into(),
                        ));
                    }
                };
                if name.is_empty() || name.encode_utf16().count() > 128 {
                    return Err(mlua::Error::RuntimeError(
                        "named random stream requires 1..128 text characters".into(),
                    ));
                }
                random(&name).map_err(mlua::Error::RuntimeError)
            })?;
            table.raw_set("random", random)?;
            callback.call::<()>((lua_payload(lua, state)?, lua_payload(lua, event)?, context))
        })
    }

    fn invoke_reducer_call(
        &mut self,
        key: &str,
        invoke: impl FnOnce(&Lua, &Function) -> mlua::Result<()>,
    ) -> Result<WebLuaReducerResult, String> {
        let lua = self.lua.as_ref().ok_or("Web Lua definition runtime is closed")?;
        let callback = {
            let mut registry = self.registry.borrow_mut();
            if registry.reducer_running {
                return Err("advanced reducers may not run recursively".into());
            }
            let callback = registry
                .reducers
                .iter()
                .find(|entry| entry.definition.key == key)
                .map(|entry| entry.callback.clone())
                .ok_or_else(|| format!("unknown advanced reducer: {key}"))?;
            registry.reducer_running = true;
            registry.reducer_result = None;
            registry.reducer_result_submitted = false;
            callback
        };
        self.deadline.set(Some(Instant::now() + Duration::from_millis(CALLBACK_TIMEOUT_MS)));
        let returned = invoke(lua, &callback);
        self.deadline.set(None);
        let mut registry = self.registry.borrow_mut();
        registry.reducer_running = false;
        let result = registry.reducer_result.take();
        let submitted = std::mem::replace(&mut registry.reducer_result_submitted, false);
        returned.map_err(|error| source_error_message(&error))?;
        if !submitted {
            return Err(format!("advanced reducer {key} returned no captured result"));
        }
        let (state, intents) =
            result.ok_or_else(|| format!("advanced reducer {key} returned no captured result"))?;
        drop(registry);
        Ok(WebLuaReducerResult {
            intents: clone_lua_value(&intents, &format!("{key} intents"))?,
            state: clone_lua_value(&state, &format!("{key} state"))?,
        })
    }

    pub fn invoke_migration(
        &mut self,
        key: &str,
        version: u64,
        value: &LuaValue,
    ) -> Result<LuaValue, String> {
        let lua = self.lua.as_ref().ok_or("Web Lua definition runtime is closed")?;
        let migration = {
            let mut registry = self.registry.borrow_mut();
            if registry.reducer_running {
                return Err("advanced reducer callbacks may not run recursively".into());
            }
            let migration = registry
                .reducers
                .iter()
                .find(|entry| entry.definition.key == key)
                .and_then(|entry| entry.migrations.get(&version))
                .cloned()
                .ok_or_else(|| {
                    format!("advanced reducer {key} has no migration from version {version}")
                })?;
            registry.reducer_running = true;
            migration
        };
        self.deadline.set(Some(Instant::now() + Duration::from_millis(CALLBACK_TIMEOUT_MS)));
        let returned =
            lua_payload(lua, value).and_then(|value| migration.call::<mlua::Value>(value));
        self.deadline.set(None);
        self.registry.borrow_mut().reducer_running = false;
        let returned = returned.map_err(|error| source_error_message(&error))?;
        clone_lua_value(&returned, &format!("{key} migration {version}"))
    }

    pub fn memory_bytes(&self) -> usize {
        self.lua.as_ref().map_or(0, Lua::used_memory)
    }

    pub fn drain_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.registry.borrow_mut().logs)
    }

    pub fn close(&mut self) {
        if self.lua.is_none() {
            return;
        }
        let mut registry = self.registry.borrow_mut();
        registry.definition = None;
        registry.reducers.clear();
        registry.reducer_result = None;
        self.lua = None;
    }
}

impl Drop for WebLuaDefinitionRuntime {
    fn drop(&mut self) {
        self.close();
    }
}

impl Registry {
    fn reset(&mut self) {
        self.definition = None;
        self.explicit = None;
        self.reducers.clear();
        self.assets.clear();
        self.content.clear();
        self.rules.clear();
        self.issues.clear();
        self.include_loaded.clear();
        self.include_loading.clear();
        self.nodes = 0;
        self.site = self.entry_source();
    }

    fn load_bundle(&mut self, code: &str) -> Result<(), String> {
        let Some(bundle) = read_web_lua_script_bundle(code)? else { return Ok(()) };
        for (path, text) in bundle {
            if let Some((_, existing)) = self.scripts.iter().find(|(existing, _)| *existing == path)
            {
                if *existing != text {
                    return Err(format!(
                        "{path} differs between the package files and the packed script bundle"
                    ));
                }
            } else {
                self.scripts.push((path, text));
            }
        }
        Ok(())
    }

    fn source(&self) -> WebLuaDefinitionSource {
        self.site.clone()
    }

    fn entry_source(&self) -> WebLuaDefinitionSource {
        WebLuaDefinitionSource {
            column: Some(0),
            file: self.entry_script.clone(),
            line: Some(0),
            node: None,
        }
    }

    fn node_source(&mut self) -> Result<WebLuaDefinitionSource, String> {
        if self.nodes as usize >= WebLuaDefinitionLimits::default().maximum_nodes {
            return Err(format!(
                "a mod may create at most {} definitions",
                WebLuaDefinitionLimits::default().maximum_nodes
            ));
        }
        self.nodes += 1;
        let mut source = self.source();
        source.node = Some(self.nodes);
        Ok(source)
    }

    fn assemble(&mut self) {
        if self.explicit.is_none()
            && self.assets.is_empty()
            && self.content.is_empty()
            && self.rules.is_empty()
            && self.reducers.is_empty()
        {
            self.issues.push(Issue::new(Code::EGraph, "definition", "the script defined nothing; create content such as sd.item({key = \"my_item\", name = \"My Item\"}) or call sd.mod({...})", Some(&self.entry_source())));
            return;
        }
        let input = WebLuaAssemblyInput {
            assets: self.assets.clone(),
            content: self.content.clone(),
            identity: self.identity.clone(),
            rules: self.rules.clone(),
        };
        let output = assemble_web_lua_definition(&input, self.explicit.as_ref(), &mut self.issues);
        let mut reducers = Vec::new();
        let mut listed = HashSet::new();
        if let Some(explicit) = &self.explicit {
            for key in &explicit.systems {
                if listed.insert(key)
                    && let Some(entry) =
                        self.reducers.iter().find(|entry| entry.definition.key == *key)
                {
                    reducers.push(entry.definition.clone());
                }
            }
        }
        for reducer in &self.reducers {
            if !listed.contains(&reducer.definition.key) {
                reducers.push(reducer.definition.clone());
            }
        }
        self.definition = Some(WebLuaModDefinition {
            api: WEB_LUA_DEFINITION_API_VERSION.into(),
            assets: output.assets,
            content: output.content,
            reducers,
            rules: output.rules,
        });
    }

    fn record_script_error(&mut self, error: &mlua::Error) {
        let message = source_error_message(error);
        if self.issues.iter().any(|issue| issue.message == message) {
            return;
        }
        if message.contains("thread timeout exceeded") {
            self.issues.push(Issue::new(Code::EBudget, "definition", format!("the definition script ran longer than {DEFINITION_TIMEOUT_MS} ms; definitions must finish quickly, so keep loops small and leave gameplay to rules"), Some(&self.entry_source())));
            return;
        }
        if message.starts_with("scripts/") {
            for (offset, _) in message.match_indices(".lua:") {
                let (line, detail) = message[offset + 5..].split_once(": ").unwrap_or(("", ""));
                if let Ok(line) = line.parse::<u32>() {
                    let source = WebLuaDefinitionSource {
                        column: Some(0),
                        file: message[..offset + 4].to_owned(),
                        line: Some(line),
                        node: None,
                    };
                    self.issues.push(Issue::new(
                        Code::EScript,
                        "script",
                        format!("{detail}{}", bridge::script_hint(detail)),
                        Some(&source),
                    ));
                    return;
                }
            }
        }
        self.issues.push(Issue::new(
            Code::EScript,
            "script",
            format!("{message}{}", bridge::script_hint(&message)),
            Some(&self.source()),
        ));
    }
}

fn install_deadline(lua: &Lua, deadline: Rc<Cell<Option<Instant>>>) -> mlua::Result<()> {
    let current = Rc::clone(&deadline);
    lua.set_hook(HookTriggers::new().every_nth_instruction(1_000), move |_, _| {
        check_deadline(&current)
    })?;
    lua.set_global_hook(HookTriggers::new().every_nth_instruction(1_000), move |_, _| {
        check_deadline(&deadline)
    })
}

fn check_deadline(deadline: &Cell<Option<Instant>>) -> mlua::Result<VmState> {
    if deadline.get().is_some_and(|deadline| Instant::now() > deadline) {
        return Err(mlua::Error::RuntimeError("thread timeout exceeded".into()));
    }
    Ok(VmState::Continue)
}

fn source_error_message(error: &mlua::Error) -> String {
    match error {
        mlua::Error::CallbackError { cause, .. } => source_error_message(cause),
        mlua::Error::RuntimeError(message) | mlua::Error::MemoryError(message) => {
            message.split("\nstack traceback:").next().unwrap_or(message).to_owned()
        }
        mlua::Error::SyntaxError { message, .. } => message.clone(),
        _ => error.to_string(),
    }
}
