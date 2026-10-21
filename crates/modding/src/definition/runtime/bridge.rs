//! Native callbacks used by the unchanged Source Lua sandbox.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

use mlua::{Lua, MultiValue, Table, Value};
use solomon_sim::modding::definition::*;
use solomon_sim::modding::{LuaObject, LuaValue};

use super::{Code, Issue, RegisteredReducer, Registry};
use crate::definition::assembly::{WebLuaExplicitAsset, WebLuaExplicitLists};
use crate::definition::scripts::valid_script_path;
use crate::definition::suggestions::{did_you_mean, list_choices, suggest_web_lua_name};
use crate::definition::values::{clone_lua_value, lua_payload, typed_value};

#[derive(Clone, Copy)]
enum Operation {
    Asset(WebLuaAssetKind),
    AssetReference,
    Wearable,
    Content(WebLuaContentKind),
    Prefab(&'static str),
    Rule(&'static str),
    Effect(&'static str),
    Intent(&'static str),
    Schema(&'static str),
    Reducer,
    Mod,
    Reference,
    Include,
}

pub(super) fn install(lua: &Lua, registry: Rc<RefCell<Registry>>) -> mlua::Result<()> {
    let sd = lua.create_table()?;
    let art = lua.create_table()?;
    for kind in WebLuaAssetKind::ALL {
        install_operation(lua, &art, kind.as_str(), Operation::Asset(kind), &registry)?;
    }
    install_operation(lua, &art, "ref", Operation::AssetReference, &registry)?;
    install_operation(lua, &art, "wearable", Operation::Wearable, &registry)?;
    sd.set("art", art)?;

    let kit = lua.create_table()?;
    for kind in WebLuaContentKind::ALL {
        install_operation(lua, &kit, &member(kind.as_str()), Operation::Content(kind), &registry)?;
    }
    sd.set("kit", kit)?;
    for (namespace, names, operation) in [
        (
            "prefab",
            WEB_LUA_PREFAB_NAMES.as_slice(),
            Operation::Prefab as fn(&'static str) -> Operation,
        ),
        ("rules", WEB_LUA_RULE_NAMES.as_slice(), Operation::Rule),
        ("effect", WEB_LUA_EFFECT_NAMES.as_slice(), Operation::Effect),
        ("intent", WEB_LUA_EFFECT_NAMES.as_slice(), Operation::Intent),
        ("schema", WEB_LUA_SCHEMA_NAMES.as_slice(), Operation::Schema),
    ] {
        let table = lua.create_table()?;
        for &name in names {
            install_operation(lua, &table, name, operation(name), &registry)?;
        }
        sd.set(namespace, table)?;
    }
    let advanced = lua.create_table()?;
    install_operation(lua, &advanced, "reducer", Operation::Reducer, &registry)?;
    sd.set("advanced", advanced)?;
    install_operation(lua, &sd, "mod", Operation::Mod, &registry)?;
    install_operation(lua, &sd, "ref", Operation::Reference, &registry)?;
    lua.globals().set("sd", sd)?;

    let current = Rc::clone(&registry);
    lua.globals().set(
        "__sd_site",
        lua.create_function(move |_, args: MultiValue| {
            let file = match argument(&args, 0) {
                Value::String(value) => {
                    value.to_string_lossy().strip_prefix('@').map(str::to_owned)
                }
                _ => None,
            };
            let line = match argument(&args, 1) {
                Value::Integer(value) if *value > 0 => u32::try_from(*value).unwrap_or(0),
                Value::Number(value) if value.fract() == 0.0 && *value > 0.0 => *value as u32,
                _ => 0,
            };
            let mut registry = current.borrow_mut();
            registry.site = WebLuaDefinitionSource {
                column: Some(0),
                file: file.unwrap_or_else(|| registry.entry_script.clone()),
                line: Some(line),
                node: None,
            };
            Ok(())
        })?,
    )?;

    let current = Rc::clone(&registry);
    lua.globals().set(
        "__sd_submit_reducer_result",
        lua.create_function(move |_, args: MultiValue| {
            {
                let registry = current.borrow();
                if !registry.reducer_running {
                    return Err(lua_error(
                        "reducer results may only be submitted while a reducer runs",
                    ));
                }
                if registry.reducer_result_submitted {
                    return Err(lua_error("reducer results may only be submitted once"));
                }
            }
            let mut registry = current.borrow_mut();
            registry.reducer_result_submitted = true;
            registry.reducer_result =
                Some((argument(&args, 0).clone(), argument(&args, 1).clone()));
            Ok(())
        })?,
    )?;

    install_operation(lua, &lua.globals(), "__sd_include_begin", Operation::Include, &registry)?;
    let current = Rc::clone(&registry);
    lua.globals().set(
        "__sd_include_end",
        lua.create_function(move |_, args: MultiValue| {
            if let Value::String(path) = argument(&args, 0) {
                let path = path.to_string_lossy();
                let mut registry = current.borrow_mut();
                registry.include_loading.remove(&path);
                if matches!(argument(&args, 1), Value::Boolean(true)) {
                    registry.include_loaded.insert(path);
                }
            }
            Ok(())
        })?,
    )?;

    lua.globals().set(
        "__sd_unknown_member",
        lua.create_function(|_, args: MultiValue| {
            Ok(unknown_member(&js_string(argument(&args, 0))?, &js_string(argument(&args, 1))?))
        })?,
    )?;
    lua.globals().set(
        "__sd_unknown_global",
        lua.create_function(|_, args: MultiValue| {
            let globals = clone_lua_value(argument(&args, 1), "global names").map_err(lua_error)?;
            Ok(unknown_global(&js_string(argument(&args, 0))?, &globals))
        })?,
    )?;
    let current = Rc::clone(&registry);
    lua.globals().set(
        "print",
        lua.create_function(move |_, args: MultiValue| {
            let mut parts = Vec::with_capacity(args.len());
            for value in args.iter() {
                parts.push(js_string(value)?);
            }
            current.borrow_mut().logs.push(parts.join("\t"));
            Ok(())
        })?,
    )?;

    // Lua::new deliberately excludes the unsafe debug library. The Source
    // bootstrap only needs this read-only stack location before hiding debug.
    let debug = lua.create_table()?;
    debug.set(
        "getinfo",
        lua.create_function(|lua, (level, _): (usize, Option<String>)| {
            let Some((source, line)) = lua.inspect_stack(level, |debug| {
                (debug.source().source.map(|source| source.into_owned()), debug.current_line())
            }) else {
                return Ok(Value::Nil);
            };
            let info = lua.create_table()?;
            info.set("source", source)?;
            info.set("currentline", line)?;
            Ok(Value::Table(info))
        })?,
    )?;
    lua.globals().set("debug", debug)?;
    Ok(())
}

fn install_operation(
    lua: &Lua,
    table: &Table,
    name: &str,
    operation: Operation,
    registry: &Rc<RefCell<Registry>>,
) -> mlua::Result<()> {
    let registry = Rc::clone(registry);
    table.set(
        name,
        lua.create_function(move |lua, args: MultiValue| {
            let result = dispatch(lua, &registry, operation, &args);
            match result {
                Ok(value) => lua_payload(lua, &value),
                Err(message) => {
                    let (code, path) = operation.label(&args);
                    let mut registry = registry.borrow_mut();
                    let source = registry.source();
                    registry.issues.push(Issue::new(code, path, &message, Some(&source)));
                    Err(lua_error(message))
                }
            }
        })?,
    )
}

impl Operation {
    fn label(self, args: &MultiValue) -> (Code, String) {
        let path = match self {
            Self::Asset(kind) => format!("sd.art.{}", member(kind.as_str())),
            Self::AssetReference => "sd.art.ref".into(),
            Self::Wearable => "sd.art.wearable".into(),
            Self::Content(kind) => {
                let mut label = format!("sd.{}", member(kind.as_str()));
                if let Value::Table(table) = argument(args, 0) {
                    let key = match table.raw_get::<Value>("key") {
                        Ok(Value::String(key)) => Some(key.to_string_lossy()),
                        _ => match table.raw_get::<Value>("name") {
                            Ok(Value::String(name)) => Some(slug_key(&name.to_string_lossy())),
                            _ => None,
                        },
                    };
                    if let Some(key) = key.filter(|key| !key.is_empty()) {
                        label.push('(');
                        label.push_str(&key);
                        label.push(')');
                    }
                }
                label
            }
            Self::Prefab(name) => format!("sd.prefab.{name}"),
            Self::Rule(name) => format!("sd.{name}"),
            Self::Effect(name) => format!("sd.effect.{name}"),
            Self::Intent(name) => format!("sd.intent.{name}"),
            Self::Schema(name) => format!("sd.schema.{name}"),
            Self::Reducer => "sd.advanced.reducer".into(),
            Self::Mod => "sd.mod".into(),
            Self::Reference => "sd.ref".into(),
            Self::Include => "sd.include".into(),
        };
        let code = match self {
            Self::Mod => Code::EGraph,
            Self::Reference | Self::AssetReference => Code::EReference,
            Self::Include => Code::EScript,
            _ => Code::ESchema,
        };
        (code, path)
    }
}

fn dispatch(
    lua: &Lua,
    registry: &Rc<RefCell<Registry>>,
    operation: Operation,
    args: &MultiValue,
) -> Result<LuaValue, String> {
    let first = argument(args, 0);
    match operation {
        Operation::Asset(kind) => asset(lua, registry, kind, first, argument(args, 1)),
        Operation::AssetReference => Ok(object([
            ("key", LuaValue::String(text(first, "asset reference key")?)),
            ("kind", LuaValue::String("asset-reference".into())),
        ])),
        Operation::Wearable => {
            let options = optional_record(lua, argument(args, 1), "wearable options")?;
            options
                .raw_set(
                    "animations",
                    lua.create_table_from([(
                        "wearable",
                        lua.create_sequence_from([1]).map_err(error_text)?,
                    )])
                    .map_err(error_text)?,
                )
                .map_err(error_text)?;
            options
                .raw_set(
                    "frame",
                    lua.create_table_from([("height", 170), ("width", 170)]).map_err(error_text)?,
                )
                .map_err(error_text)?;
            options.raw_set("image", text(first, "wearable art path")?).map_err(error_text)?;
            asset(lua, registry, WebLuaAssetKind::Sheet, &Value::Table(options), &Value::Nil)
        }
        Operation::Content(kind) => content(lua, registry, kind, first),
        Operation::Prefab(name) => rule(registry, &format!("prefab.{name}"), first),
        Operation::Effect(name) => rule(registry, &format!("effect.{name}"), first),
        Operation::Rule(name) => {
            let fields = rule_arguments(lua, name, args)?;
            rule(registry, &format!("rules.{name}"), &Value::Table(fields))
        }
        Operation::Intent(name) => {
            let fields = definition_record(first, &format!("intent.{name} intent"))?;
            let source = registry.borrow().source();
            crate::json::lua_value(&WebLuaIntentDefinition {
                fields,
                intent_kind: name.into(),
                source,
            })
        }
        Operation::Schema(kind) => {
            let fields = if kind == "enum" {
                if !is_array(first)? {
                    return Err("enum values must be an array".into());
                }
                let values = clone_lua_value(first, "enum values")?;
                match object([("values", values)]) {
                    LuaValue::Object(fields) => fields,
                    _ => unreachable!(),
                }
            } else if kind == "object" && is_record(first)? {
                let fields = LuaValue::Object(definition_record(first, "object schema fields")?);
                match object([("fields", fields)]) {
                    LuaValue::Object(fields) => fields,
                    _ => unreachable!(),
                }
            } else {
                let value = if matches!(first, Value::Nil) {
                    Value::Table(lua.create_table().map_err(error_text)?)
                } else {
                    first.clone()
                };
                definition_record(&value, &format!("{kind} schema"))?
            };
            crate::json::lua_value(&WebLuaSchemaDefinition {
                fields,
                schema_kind: kind.into(),
                source: registry.borrow().source(),
            })
        }
        Operation::Reducer => reducer(registry, first),
        Operation::Mod => mod_definition(registry, first),
        Operation::Reference => {
            let kind = text(first, "reference kind")?;
            let Some(target_kind) = WebLuaContentKind::parse(&kind) else {
                return Err(format!(
                    "unknown content reference kind: {kind}{}",
                    did_you_mean(&kind, WebLuaContentKind::ALL.map(WebLuaContentKind::as_str))
                ));
            };
            let key = text(argument(args, 1), "reference key")?;
            let mod_id = if matches!(argument(args, 2), Value::Nil) {
                registry.borrow().identity.id.clone()
            } else {
                text(argument(args, 2), "reference mod id")?
            };
            Ok(object([
                ("key", LuaValue::String(key)),
                ("kind", LuaValue::String("content-reference".into())),
                ("modId", LuaValue::String(mod_id)),
                ("targetKind", LuaValue::String(target_kind.as_str().into())),
            ]))
        }
        Operation::Include => include_begin(registry, first),
    }
}

fn asset(
    lua: &Lua,
    registry: &Rc<RefCell<Registry>>,
    kind: WebLuaAssetKind,
    value: &Value,
    options: &Value,
) -> Result<LuaValue, String> {
    let label = format!("{} asset", kind.as_str());
    let fields = if let Value::String(path) = value {
        let fields = optional_record(lua, options, &format!("{label} options"))?;
        fields
            .raw_set(if kind == WebLuaAssetKind::Sheet { "image" } else { "path" }, path.clone())
            .map_err(error_text)?;
        fields
    } else {
        copy_record(lua, value, &format!("{label} definition"))?
    };
    let raw_key = fields.raw_get::<Value>("key").map_err(error_text)?;
    let key = if matches!(raw_key, Value::Nil) {
        String::new()
    } else {
        text(&raw_key, &format!("{label} key"))?
    };
    fields.raw_remove("key").map_err(error_text)?;
    let fields = definition_record(&Value::Table(fields), &format!("{label} definition"))?;
    let mut registry = registry.borrow_mut();
    let definition =
        WebLuaAssetDefinition { asset_kind: kind, fields, key, source: registry.node_source()? };
    let value = crate::json::lua_value(&definition)?;
    registry.assets.push(definition);
    Ok(value)
}

fn content(
    lua: &Lua,
    registry: &Rc<RefCell<Registry>>,
    kind: WebLuaContentKind,
    value: &Value,
) -> Result<LuaValue, String> {
    let name = member(kind.as_str());
    let label = format!("sd.{name}");
    let fields = copy_record(lua, value, &format!("{label} definition"))?;
    let raw_key = fields.raw_get::<Value>("key").map_err(error_text)?;
    let key = if matches!(raw_key, Value::Nil) {
        if let Value::String(raw_name) = fields.raw_get::<Value>("name").map_err(error_text)?
            && !crate::definition::scripts::trim_javascript(&raw_name.to_string_lossy()).is_empty()
        {
            let key = slug_key(&raw_name.to_string_lossy());
            if !valid_web_lua_content_key(&key) {
                return Err(format!(
                    "{label} needs a key because its name cannot become one; add key = \"my_{name}\""
                ));
            }
            key
        } else {
            return Err(format!(
                "{label} needs a key, for example key = \"my_{name}\"; keys are lowercase words joined by underscores"
            ));
        }
    } else {
        text(&raw_key, &format!("{name} key"))?
    };
    fields.raw_remove("key").map_err(error_text)?;
    let fields = definition_record(&Value::Table(fields), &format!("{label} definition"))?;
    let mut registry = registry.borrow_mut();
    let definition = WebLuaContentDefinition {
        content_kind: kind,
        fields,
        key,
        source: registry.node_source()?,
    };
    let value = crate::json::lua_value(&definition)?;
    registry.content.push(definition);
    Ok(value)
}

fn rule(
    registry: &Rc<RefCell<Registry>>,
    operation: &str,
    value: &Value,
) -> Result<LuaValue, String> {
    let fields = definition_record(value, &format!("{operation} definition"))?;
    let mut registry = registry.borrow_mut();
    let definition = WebLuaRuleDefinition {
        fields,
        operation: operation.into(),
        source: registry.node_source()?,
    };
    let value = crate::json::lua_value(&definition)?;
    registry.rules.push(definition);
    Ok(value)
}

fn rule_arguments(lua: &Lua, name: &str, args: &MultiValue) -> Result<Table, String> {
    let fields = if name == "every" {
        match argument(args, 2) {
            Value::Nil => lua.create_table().map_err(error_text)?,
            value @ (Value::Integer(_) | Value::Number(_)) => {
                let fields = lua.create_table().map_err(error_text)?;
                fields.raw_set("times", value.clone()).map_err(error_text)?;
                fields
            }
            value => copy_record(lua, value, "sd.every options")?,
        }
    } else {
        lua.create_table().map_err(error_text)?
    };
    match name {
        "after" | "on" => {
            fields
                .raw_set(
                    if name == "after" { "duration" } else { "event" },
                    argument(args, 0).clone(),
                )
                .map_err(error_text)?;
            let nodes: Vec<_> = args.iter().skip(1).cloned().collect();
            if nodes.is_empty() {
                return Err(format!("sd.{name} is missing the effect or rule to run"));
            }
            let node = if nodes.len() == 1 {
                nodes[0].clone()
            } else {
                Value::Table(lua.create_sequence_from(nodes).map_err(error_text)?)
            };
            fields.raw_set("node", node).map_err(error_text)?;
        }
        "all" | "first" => {
            if args.is_empty() {
                return Err(format!("sd.{name} needs at least one effect or rule"));
            }
            let nodes = if args.len() == 1 && is_array(argument(args, 0))? {
                argument(args, 0).clone()
            } else {
                Value::Table(lua.create_sequence_from(args.iter().cloned()).map_err(error_text)?)
            };
            fields.raw_set("nodes", nodes).map_err(error_text)?;
        }
        "every" => {
            fields.raw_set("interval", argument(args, 0).clone()).map_err(error_text)?;
            fields.raw_set("node", argument(args, 1).clone()).map_err(error_text)?;
        }
        "when" => {
            fields.raw_set("no", argument(args, 2).clone()).map_err(error_text)?;
            fields.raw_set("predicate", argument(args, 0).clone()).map_err(error_text)?;
            fields.raw_set("yes", argument(args, 1).clone()).map_err(error_text)?;
        }
        _ => unreachable!("installed rule names match the Source table"),
    }
    Ok(fields)
}

fn reducer(registry: &Rc<RefCell<Registry>>, value: &Value) -> Result<LuaValue, String> {
    let source = record(value, "advanced reducer")?;
    exact_keys(
        &source,
        &["key", "migrations", "on", "reduce", "schema_version", "scope", "state"],
        "advanced reducer",
    )?;
    let key = text(&source.raw_get::<Value>("key").map_err(error_text)?, "advanced reducer key")?;
    if registry.borrow().reducers.iter().any(|entry| entry.definition.key == key) {
        return Err(format!("advanced reducer is already registered: {key}"));
    }
    let scope =
        text(&source.raw_get::<Value>("scope").map_err(error_text)?, "advanced reducer scope")?;
    let Some(scope) = WebLuaScopeKind::parse(&scope) else {
        return Err(format!(
            "unknown advanced reducer scope: {scope}{}",
            did_you_mean(&scope, WebLuaScopeKind::ALL.map(WebLuaScopeKind::as_str))
        ));
    };
    let on_value = source.raw_get::<Value>("on").map_err(error_text)?;
    if !is_array(&on_value)? {
        return Err("advanced reducer on must be a nonempty array of at most 64 event names".into());
    }
    let raw_on = raw_entries(on_value.as_table().expect("array projection owns a table"))?;
    if raw_on.is_empty() || raw_on.len() > 64 {
        return Err("advanced reducer on must be a nonempty array of at most 64 event names".into());
    }
    let mut on = Vec::with_capacity(raw_on.len());
    for (index, (_, value)) in raw_on.into_iter().enumerate() {
        on.push(text(&value, &format!("advanced reducer on[{index}]"))?);
    }
    if on.iter().collect::<HashSet<_>>().len() != on.len() {
        return Err("advanced reducer on contains duplicates".into());
    }
    if let Some(event) = on.iter().find(|event| !WEB_LUA_RULE_EVENT_NAMES.contains(&event.as_str()))
    {
        return Err(format!(
            "unknown advanced reducer event: {event}{}",
            did_you_mean(event, WEB_LUA_RULE_EVENT_NAMES)
        ));
    }
    if on.iter().any(|event| event == "player.control") && scope != WebLuaScopeKind::ParticipantRun
    {
        return Err("player.control reducers require participant-run scope".into());
    }
    let version = source.raw_get::<Value>("schema_version").map_err(error_text)?;
    let version = match version {
        Value::Integer(value) => value as f64,
        Value::Number(value) => value,
        _ => f64::NAN,
    };
    if version < 1.0
        || version > 9_007_199_254_740_991.0
        || !version.is_finite()
        || version.fract() != 0.0
    {
        return Err("advanced reducer schema_version must be a positive safe integer".into());
    }
    let version = version as u64;
    let state = source.raw_get::<Value>("state").map_err(error_text)?;
    if !raw_token(&state, "schema-definition") {
        return Err("advanced reducer state must be an sd.schema definition".into());
    }
    let state = typed_value(&clone_lua_value(&state, "advanced reducer state")?)?;
    let callback = match source.raw_get::<Value>("reduce").map_err(error_text)? {
        Value::Function(function) => function,
        _ => return Err("advanced reducer reduce must be a function".into()),
    };
    let migration_source = source.raw_get::<Value>("migrations").map_err(error_text)?;
    let entries = match migration_source {
        Value::Nil => Vec::new(),
        Value::Table(table) => raw_entries(&table)?,
        _ => return Err("advanced reducer migrations must be a table".into()),
    };
    let mut migrations = BTreeMap::new();
    for migration_version in 1..version {
        let key = migration_version.to_string();
        let Some((_, Value::Function(migration))) = entries.iter().find(|(name, _)| *name == key)
        else {
            return Err(format!(
                "advanced reducer is missing migration from version {migration_version}"
            ));
        };
        migrations.insert(migration_version, migration.clone());
    }
    let mut registry = registry.borrow_mut();
    let maximum = WebLuaDefinitionLimits::default().maximum_reducers;
    if registry.reducers.len() >= maximum {
        return Err(format!("a mod may register at most {maximum} advanced reducers"));
    }
    let definition = WebLuaReducerDefinition {
        key: key.clone(),
        on,
        schema_version: version,
        scope,
        source: registry.source(),
        state,
    };
    registry.reducers.push(RegisteredReducer { definition, callback, migrations });
    Ok(object([("key", LuaValue::String(key)), ("kind", LuaValue::String("reducer-token".into()))]))
}

fn mod_definition(registry: &Rc<RefCell<Registry>>, value: &Value) -> Result<LuaValue, String> {
    if registry.borrow().explicit.is_some() {
        return Err("sd.mod may be called only once".into());
    }
    let source = record(value, "sd.mod definition")?;
    exact_keys(&source, &["api", "assets", "content", "rules", "systems"], "sd.mod definition")?;
    let api = source.raw_get::<Value>("api").map_err(error_text)?;
    if !matches!(api, Value::Nil) && text(&api, "sd.mod api")? != WEB_LUA_DEFINITION_API_VERSION {
        return Err(format!("sd.mod api must be \"{WEB_LUA_DEFINITION_API_VERSION}\""));
    }
    let assets =
        definition_list(&source.raw_get::<Value>("assets").map_err(error_text)?, "assets")?;
    let content =
        definition_list(&source.raw_get::<Value>("content").map_err(error_text)?, "content")?;
    let rules = definition_list(&source.raw_get::<Value>("rules").map_err(error_text)?, "rules")?;
    let systems =
        definition_list(&source.raw_get::<Value>("systems").map_err(error_text)?, "systems")?;
    let mut explicit = WebLuaExplicitLists::default();
    // These are actual created tokens, rather than deserialized caller-chosen
    // IDs. Validate each against its owner registry before assembly uses it.
    let registry_ref = registry.borrow();
    for (index, (map_key, value)) in assets.into_iter().enumerate() {
        let field = format!("assets[{}]", map_key.clone().unwrap_or_else(|| index.to_string()));
        let node = registry_node(
            &value,
            "asset-definition",
            |node| registry_ref.assets.iter().any(|entry| entry.source.node == Some(node)),
            &field,
            "sd.art",
        )?;
        let definition = registry_ref
            .assets
            .iter()
            .find(|entry| entry.source.node == Some(node))
            .expect("validated asset node");
        if let Some(map_key) = &map_key
            && !definition.key.is_empty()
            && *map_key != definition.key
        {
            return Err(format!(
                "assets.{map_key} was created with key = \"{}\"; use one name for it",
                definition.key
            ));
        }
        explicit.assets.push(WebLuaExplicitAsset {
            key: map_key.unwrap_or_else(|| definition.key.clone()),
            node,
        });
    }
    for (index, (map_key, value)) in content.into_iter().enumerate() {
        let field = format!("content[{}]", map_key.clone().unwrap_or_else(|| index.to_string()));
        let node = registry_node(
            &value,
            "content-definition",
            |node| registry_ref.content.iter().any(|entry| entry.source.node == Some(node)),
            &field,
            "sd.kit",
        )?;
        let definition = registry_ref
            .content
            .iter()
            .find(|entry| entry.source.node == Some(node))
            .expect("validated content node");
        if let Some(map_key) = map_key
            && map_key != definition.key
        {
            return Err(format!(
                "content.{map_key} was created with key = \"{}\"; use one name for it",
                definition.key
            ));
        }
        explicit.content.push(node);
    }
    for (index, (_, value)) in rules.into_iter().enumerate() {
        explicit.rules.push(registry_node(
            &value,
            "rule-definition",
            |node| registry_ref.rules.iter().any(|entry| entry.source.node == Some(node)),
            &format!("rules[{index}]"),
            "sd.on, sd.rules, sd.effect, or sd.prefab",
        )?);
    }
    for (index, (_, value)) in systems.into_iter().enumerate() {
        let key = match &value {
            LuaValue::Object(fields)
                if fields.get("kind").and_then(LuaValue::as_str) == Some("reducer-token") =>
            {
                fields.get("key").and_then(LuaValue::as_str)
            }
            _ => None,
        };
        let Some(key) = key
            .filter(|key| registry_ref.reducers.iter().any(|entry| entry.definition.key == *key))
        else {
            return Err(format!("systems[{index}] must be created by sd.advanced.reducer"));
        };
        explicit.systems.push(key.into());
    }
    if explicit.systems.iter().collect::<HashSet<_>>().len() != explicit.systems.len() {
        return Err("systems contains duplicates".into());
    }
    let count = registry_ref.content.len();
    let mod_id = registry_ref.identity.id.clone();
    drop(registry_ref);
    registry.borrow_mut().explicit = Some(explicit);
    Ok(object([
        ("api", LuaValue::String(WEB_LUA_DEFINITION_API_VERSION.into())),
        ("content_count", LuaValue::Number(count as f64)),
        ("kind", LuaValue::String("mod-definition-receipt".into())),
        ("mod_id", LuaValue::String(mod_id)),
    ]))
}

fn include_begin(registry: &Rc<RefCell<Registry>>, value: &Value) -> Result<LuaValue, String> {
    let path = match value {
        Value::String(path) if valid_script_path(&path.to_string_lossy()) => path.to_string_lossy(),
        _ => {
            return Err(
                "sd.include needs a package script path such as \"scripts/items.lua\"".into()
            );
        }
    };
    let mut registry = registry.borrow_mut();
    if path == registry.entry_script {
        return Err(format!("{path} is the entry script and cannot include itself"));
    }
    if registry.include_loading.contains(&path) {
        return Err(format!(
            "{path} is still being included; scripts may not include each other in a cycle"
        ));
    }
    if registry.include_loaded.contains(&path) {
        return Ok(object([("status", LuaValue::String("cached".into()))]));
    }
    let Some((_, text)) = registry.scripts.iter().find(|(name, _)| *name == path) else {
        let names: Vec<_> = registry.scripts.iter().map(|(name, _)| name.as_str()).collect();
        let hint = if names.is_empty() {
            "; add the file under scripts/ in the package".into()
        } else {
            let hint = did_you_mean(&path, names.iter().copied());
            if hint.is_empty() {
                format!("; the package has {}", list_choices(names))
            } else {
                hint
            }
        };
        return Err(format!("{path} is not in the package{hint}"));
    };
    let text = text.clone();
    registry.include_loading.insert(path);
    Ok(object([("status", LuaValue::String("load".into())), ("text", LuaValue::String(text))]))
}

fn registry_node(
    value: &LuaValue,
    kind: &str,
    contains: impl FnOnce(u32) -> bool,
    field: &str,
    constructors: &str,
) -> Result<u32, String> {
    let node = value
        .as_object()
        .filter(|fields| fields.get("kind").and_then(LuaValue::as_str) == Some(kind))
        .and_then(|fields| fields.get("source"))
        .and_then(LuaValue::as_object)
        .and_then(|fields| fields.get("node"))
        .and_then(LuaValue::as_number);
    if let Some(node) =
        node.filter(|node| node.fract() == 0.0 && *node >= 0.0 && *node <= u32::MAX as f64)
        && contains(node as u32)
    {
        return Ok(node as u32);
    }
    Err(format!("{field} must be created by {constructors}"))
}

fn definition_list(value: &Value, field: &str) -> Result<Vec<(Option<String>, LuaValue)>, String> {
    if matches!(value, Value::Nil) {
        return Ok(Vec::new());
    }
    if !matches!(value, Value::Table(_)) {
        return Err(format!("{field} must be a table"));
    }
    match clone_lua_value(value, field)? {
        LuaValue::Array(values) => Ok(values.into_iter().map(|value| (None, value)).collect()),
        LuaValue::Object(fields) => {
            Ok(fields.iter().map(|(key, value)| (Some(key.into()), value.clone())).collect())
        }
        _ => unreachable!("table bridge returns an object or array"),
    }
}

fn raw_entries(table: &Table) -> Result<Vec<(String, Value)>, String> {
    let mut entries = Vec::new();
    for entry in table.clone().pairs::<Value, Value>() {
        let (key, value) = entry.map_err(error_text)?;
        entries.push((key.to_string().map_err(error_text)?, value));
    }
    Ok(entries)
}

fn is_array(value: &Value) -> Result<bool, String> {
    let Value::Table(table) = value else { return Ok(false) };
    let entries = raw_entries(table)?;
    Ok(!entries.is_empty()
        && entries.iter().enumerate().all(|(index, (key, _))| *key == (index + 1).to_string()))
}

fn is_record(value: &Value) -> Result<bool, String> {
    Ok(matches!(value, Value::Table(_)) && !is_array(value)?)
}

fn record(value: &Value, field: &str) -> Result<Table, String> {
    if !is_record(value)? {
        return Err(format!("{field} must be a table"));
    }
    Ok(value.as_table().expect("record projection owns a table").clone())
}

fn copy_record(lua: &Lua, value: &Value, field: &str) -> Result<Table, String> {
    let source = record(value, field)?;
    let table = lua.create_table().map_err(error_text)?;
    for (key, value) in raw_entries(&source)? {
        table.raw_set(key, value).map_err(error_text)?;
    }
    Ok(table)
}

fn optional_record(lua: &Lua, value: &Value, field: &str) -> Result<Table, String> {
    if matches!(value, Value::Nil) {
        lua.create_table().map_err(error_text)
    } else {
        copy_record(lua, value, field)
    }
}

fn definition_record(value: &Value, field: &str) -> Result<LuaObject, String> {
    record(value, field)?;
    match clone_lua_value(value, field)? {
        LuaValue::Object(fields) => Ok(fields),
        _ => unreachable!("record projection owns an object"),
    }
}

fn exact_keys(table: &Table, keys: &[&str], field: &str) -> Result<(), String> {
    let mut fields = LuaObject::new();
    for (key, _) in raw_entries(table)? {
        fields.insert(key, LuaValue::Null);
    }
    let unknown: Vec<_> =
        fields.iter().map(|(key, _)| key).filter(|key| !keys.contains(key)).collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let hint = unknown
        .iter()
        .map(|key| did_you_mean(key, keys.iter().copied()))
        .find(|hint| !hint.is_empty())
        .unwrap_or_default();
    Err(format!("{field} contains unknown fields: {}{hint}", unknown.join(", ")))
}

fn raw_token(value: &Value, kind: &str) -> bool {
    match value {
        Value::Table(table) => table.raw_get::<String>("kind").is_ok_and(|value| value == kind),
        _ => false,
    }
}

fn text(value: &Value, field: &str) -> Result<String, String> {
    if let Value::String(value) = value {
        let value = value.to_string_lossy();
        if !value.is_empty() && value.encode_utf16().count() <= 256 && !value.contains('\0') {
            return Ok(value);
        }
    }
    Err(format!("{field} must be nonempty text of at most 256 characters"))
}

fn argument(args: &MultiValue, index: usize) -> &Value {
    args.get(index).unwrap_or(&Value::Nil)
}

fn member(value: &str) -> String {
    value.replace('-', "_")
}

fn slug_key(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in value.to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            if separator && !result.is_empty() {
                result.push('_')
            }
            result.push(character);
            separator = false;
        } else {
            separator = true
        }
    }
    result
}

fn object<const N: usize>(fields: [(&str, LuaValue); N]) -> LuaValue {
    let mut output = LuaObject::new();
    for (key, value) in fields {
        output.insert(key.into(), value);
    }
    LuaValue::Object(output)
}

fn lua_error(message: impl Into<String>) -> mlua::Error {
    mlua::Error::RuntimeError(message.into())
}
fn error_text(error: mlua::Error) -> String {
    super::source_error_message(&error)
}

fn js_string(value: &Value) -> mlua::Result<String> {
    Ok(match value {
        Value::Nil => "null".into(),
        Value::Boolean(value) => value.to_string(),
        Value::Integer(value) => {
            crate::json::stringify(&LuaValue::Number(*value as f64)).map_err(lua_error)?
        }
        Value::Number(value) => {
            crate::json::stringify(&LuaValue::Number(*value)).map_err(lua_error)?
        }
        Value::String(value) => value.to_string_lossy(),
        Value::Table(_) if is_array(value).map_err(lua_error)? => {
            let values = raw_entries(value.as_table().expect("array projection owns a table"))
                .map_err(lua_error)?;
            let mut parts = Vec::new();
            for (_, value) in values {
                parts.push(js_string(&value)?);
            }
            parts.join(",")
        }
        Value::Table(_) => "[object Object]".into(),
        Value::Function(_) => "function".into(),
        _ => value.to_string()?,
    })
}

fn members() -> Vec<(&'static str, Vec<String>)> {
    let kit: Vec<_> =
        WebLuaContentKind::ALL.into_iter().map(|kind| member(kind.as_str())).collect();
    let rules: Vec<_> = WEB_LUA_RULE_NAMES.into_iter().map(str::to_owned).collect();
    let mut sd = kit.clone();
    sd.extend(rules.iter().cloned());
    sd.extend(WEB_LUA_ART_ALIAS_NAMES.map(str::to_owned));
    sd.extend(
        [
            "advanced", "art", "effect", "include", "intent", "kit", "mod", "prefab", "ref",
            "rules", "schema",
        ]
        .map(str::to_owned),
    );
    let mut art: Vec<_> =
        WebLuaAssetKind::ALL.into_iter().map(|kind| member(kind.as_str())).collect();
    art.extend(["ref".into(), "wearable".into()]);
    vec![
        ("sd", sd),
        ("sd.advanced", vec!["reducer".into()]),
        ("sd.art", art),
        ("sd.effect", WEB_LUA_EFFECT_NAMES.map(str::to_owned).to_vec()),
        ("sd.intent", WEB_LUA_EFFECT_NAMES.map(str::to_owned).to_vec()),
        ("sd.kit", kit),
        ("sd.prefab", WEB_LUA_PREFAB_NAMES.map(str::to_owned).to_vec()),
        ("sd.rules", rules),
        ("sd.schema", WEB_LUA_SCHEMA_NAMES.map(str::to_owned).to_vec()),
    ]
}

fn unknown_member(namespace: &str, name: &str) -> String {
    let namespaces = members();
    let local: Vec<_> = namespaces
        .iter()
        .find(|(space, _)| *space == namespace)
        .map(|(_, names)| names.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let mut hint = String::new();
    if let Some((space, _)) = namespaces
        .iter()
        .find(|(space, names)| *space != namespace && names.iter().any(|member| member == name))
    {
        hint = format!("; did you mean {space}.{name}?");
    } else if let Some(suggestion) = suggest_web_lua_name(name, local.iter().copied()) {
        hint = format!("; did you mean {namespace}.{suggestion}?");
    } else {
        for (space, names) in &namespaces {
            if let Some(suggestion) = suggest_web_lua_name(name, names.iter().map(String::as_str)) {
                hint = format!("; did you mean {space}.{suggestion}?");
                break;
            }
        }
        if hint.is_empty() && !local.is_empty() && local.len() <= 12 {
            hint = format!("; {namespace} offers {}", list_choices(local));
        }
        if hint.is_empty() {
            hint = "; see REFERENCE.md for the full list of names".into()
        }
    }
    format!("{namespace}.{name} is not part of Web Lua 1.0{hint}")
}

fn unknown_global(name: &str, globals: &LuaValue) -> String {
    if ["dofile", "loadfile", "require"].contains(&name) {
        return format!(
            "{name} is not available inside Web Lua mods; use sd.include(\"scripts/file.lua\") to split a mod across files"
        );
    }
    if [
        "collectgarbage",
        "coroutine",
        "debug",
        "dofile",
        "io",
        "load",
        "loadfile",
        "module",
        "os",
        "package",
        "require",
        "rawset",
    ]
    .contains(&name)
    {
        return format!("{name} is not available inside Web Lua mods");
    }
    let members = members();
    let sd = &members[0].1;
    if sd.iter().any(|member| member == name) {
        return format!("'{name}' is not defined; did you mean sd.{name}?");
    }
    if let Some(suggestion) = suggest_web_lua_name(name, sd.iter().map(String::as_str)) {
        return format!("'{name}' is not defined; did you mean sd.{suggestion}?");
    }
    if let LuaValue::Array(globals) = globals
        && let Some(suggestion) =
            suggest_web_lua_name(name, globals.iter().filter_map(LuaValue::as_str))
    {
        return format!("'{name}' is not defined; did you mean {suggestion}?");
    }
    format!(
        "'{name}' is not defined; create it first with local {name} = ... or check the spelling"
    )
}

pub(super) fn script_hint(message: &str) -> &'static str {
    for (pattern, hint) in [
        (
            "')' expected",
            "; Lua tables use braces, so write sd.item({key = \"my_item\"}) or sd.item{key = \"my_item\"}",
        ),
        (
            "'}' expected",
            "; a table is missing its closing brace; check the commas between fields too",
        ),
        ("'end' expected", "; a function, if, or for block is missing its matching end"),
        (
            "unexpected symbol near <eof>",
            "; the file ended early; a closing brace, parenthesis, or end is missing",
        ),
        (
            "unexpected symbol near ')'",
            "; there is an extra parenthesis, or a comma right before a closing parenthesis",
        ),
        ("unfinished string", "; a string is missing its closing quote"),
        (
            "'=' expected",
            "; fields inside tables are written as name = value, for example key = \"my_item\"",
        ),
        (
            "attempt to index a nil value",
            "; a name used here has no value yet; check that it is spelled like where it was created",
        ),
        (
            "attempt to call a nil value",
            "; the function named here does not exist; check its spelling against the reference",
        ),
        (
            "attempt to call a table value",
            "; this name holds a table, not a function; calls look like sd.item({...})",
        ),
        ("attempt to concatenate", "; join text with .. and wrap numbers in tostring()"),
        ("attempt to compare", "; these two values have different types and cannot be compared"),
        (
            "attempt to perform arithmetic",
            "; one of these values is text or nil rather than a number",
        ),
    ] {
        if message.contains(pattern) {
            return hint;
        }
    }
    ""
}
