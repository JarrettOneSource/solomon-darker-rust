//! Console SDK installation. Ordinary Lua tables expose the current borrowed provider.

mod mutations;
mod reads;

use super::*;
use crate::definition::values::{LuaTableEntries, lua_payload, lua_table_entries};
use solomon_sim::modding::console::*;
use values::{finite_within, integer_within, nonnegative_integer, normalize, require_text};

pub(super) fn install(
    lua: &Lua,
    state: Rc<RefCell<RuntimeState>>,
    developer: bool,
) -> mlua::Result<()> {
    let sd = lua.create_table()?;
    for namespace in [
        "enemies", "events", "gameplay", "hub", "player", "rng", "runtime", "scene", "state",
        "timer", "waves", "world",
    ] {
        sd.set(namespace, lua.create_table()?)?;
    }
    if developer {
        sd.set("dev", lua.create_table()?)?;
        sd.set("bots", lua.create_table()?)?;
    }
    let functions = [
        "enemies.get",
        "enemies.list",
        "enemies.spawn",
        "events.names",
        "events.on",
        "gameplay.get_combat_state",
        "gameplay.get_state",
        "hub.get_surface_state",
        "player.get_state",
        "player.grant_experience",
        "player.list",
        "player.restore_health",
        "player.restore_mana",
        "player.set_gold",
        "player.set_mana",
        "rng.get_seed",
        "rng.set_seed",
        "runtime.get_capabilities",
        "runtime.get_frame_state",
        "runtime.get_mod",
        "runtime.get_multiplayer_state",
        "runtime.has_capability",
        "scene.get_state",
        "state.clear",
        "state.delete",
        "state.get",
        "state.get_revision",
        "state.is_authority",
        "state.set",
        "state.snapshot",
        "timer.after",
        "timer.cancel",
        "timer.clear",
        "timer.every",
        "timer.sequence",
        "waves.get_state",
        "waves.spawn_next",
        "world.get_scene",
        "world.get_state",
        "world.list_actors",
        "dev.grant_gold",
        "dev.grant_item",
        "dev.grant_skill",
        "dev.grant_weld",
        "dev.list_items",
        "dev.list_skills",
        "dev.list_welds",
        "bots.summon",
    ];
    for operation in functions {
        let (namespace, name) = operation.split_once('.').expect("SDK path has a namespace");
        if !developer && matches!(namespace, "dev" | "bots") {
            continue;
        }
        let table: Table = sd.get(namespace)?;
        let shared = Rc::clone(&state);
        table.set(
            name,
            lua.create_function(move |lua, args: MultiValue| {
                invoke(lua, &shared, operation, args).map_err(mlua::Error::RuntimeError)
            })?,
        )?;
    }
    sd.get::<Table>("runtime")?.set("api_version", WEB_LUA_API_VERSION)?;
    lua.globals().set("sd", sd)?;
    lua.globals().set(
        "print",
        lua.create_function(move |_, args: MultiValue| {
            print(&state, args).map_err(mlua::Error::RuntimeError)
        })?,
    )
}

fn invoke(
    lua: &Lua,
    shared: &Rc<RefCell<RuntimeState>>,
    operation: &str,
    args: MultiValue,
) -> Result<MultiValue, String> {
    let first = args.front().cloned().unwrap_or(Value::Nil);
    let second = args.get(1).cloned().unwrap_or(Value::Nil);
    match operation {
        "events.on" => {
            shared.borrow_mut().add_callback(&first, &second)?;
            reply(lua, LuaValue::Boolean(true))
        }
        "state.clear" => {
            let changed = shared.borrow_mut().clear_state();
            reply(lua, LuaValue::Boolean(changed))
        }
        "state.delete" => {
            let key = require_text(&first, "state key", 128)?;
            let mut state = shared.borrow_mut();
            let removed = state.state.remove(&key).is_some();
            if removed {
                state.state_revision += 1;
            }
            reply(lua, LuaValue::Boolean(removed))
        }
        "state.get" => {
            let key = require_text(&first, "state key", 128)?;
            if let Some(value) = shared.borrow().state.get(&key) {
                return reply(lua, value.clone());
            }
            if matches!(second, Value::Nil) {
                return reply(lua, LuaValue::Null);
            }
            Ok(MultiValue::from_vec(vec![second]))
        }
        "state.get_revision" => reply(lua, LuaValue::Number(shared.borrow().state_revision as f64)),
        "state.is_authority" => {
            let actual = authority(lua).map_err(|error| bounded_error(&error))?;
            let is_authority = actual.is_some() && actual == shared.borrow().active_player_id;
            reply(lua, LuaValue::Boolean(is_authority))
        }
        "state.set" => {
            let key = require_text(&first, "state key", 128)?;
            if matches!(second, Value::Nil) {
                return Err("Lua state values may not be nil; use sd.state.delete".into());
            }
            let value = normalize(&second, &format!("state.{key}"))?;
            let revision = shared.borrow_mut().set_state(&key, value)?;
            reply(lua, LuaValue::Number(revision as f64))
        }
        "state.snapshot" => reply(lua, LuaValue::Object(shared.borrow().state.clone())),
        "timer.after" | "timer.every" => {
            let handle =
                shared.borrow_mut().add_timer(&first, &second, operation == "timer.every")?;
            reply(lua, LuaValue::Number(handle as f64))
        }
        "timer.sequence" => {
            let handle = shared.borrow_mut().add_sequence(&first)?;
            reply(lua, LuaValue::Number(handle as f64))
        }
        "timer.cancel" => {
            let handle = nonnegative_integer(&first, "handle")?;
            if handle == 0 {
                return Err("handle must be positive".into());
            }
            let mut state = shared.borrow_mut();
            let before = state.timers.len();
            state.timers.retain(|timer| timer.borrow().handle != handle);
            reply(lua, LuaValue::Boolean(state.timers.len() < before))
        }
        "timer.clear" => {
            let mut state = shared.borrow_mut();
            let count = state.timers.len();
            state.timers.clear();
            reply(lua, LuaValue::Number(count as f64))
        }
        _ if operation.starts_with("dev.grant_")
            || operation.starts_with("player.")
                && !matches!(operation, "player.get_state" | "player.list")
            || matches!(
                operation,
                "bots.summon" | "enemies.spawn" | "rng.set_seed" | "waves.spawn_next"
            ) =>
        {
            mutations::invoke(lua, shared, operation, args)
        }
        _ => reads::invoke(lua, shared, operation, first),
    }
}

fn reply(lua: &Lua, value: LuaValue) -> Result<MultiValue, String> {
    let value = lua_payload(lua, &value).map_err(|error| bounded_error(&error))?;
    Ok(MultiValue::from_vec(vec![value]))
}

fn print(shared: &Rc<RefCell<RuntimeState>>, args: MultiValue) -> Result<(), String> {
    let mut printed = Vec::new();
    for value in args {
        let text = if let Value::String(text) = &value {
            text.to_string_lossy()
        } else {
            let value = normalize(&value, "print value")?;
            match value {
                LuaValue::Null => "null".into(),
                LuaValue::Boolean(value) => value.to_string(),
                LuaValue::String(value) => value,
                value => crate::json::stringify(&value)?,
            }
        };
        printed.push(text);
    }
    let line = printed.join("\t");
    if line.len() > 4096 {
        return Err("Lua print line exceeds its limit".into());
    }
    let mut state = shared.borrow_mut();
    if state.active_output.is_none() {
        state.log("info", "lua.print", line);
        return Ok(());
    }
    let count = state.active_output.as_ref().expect("active output exists").len();
    if count >= 64 {
        return Err("Lua print output exceeds its line limit".into());
    }
    let next_entry_bytes =
        state.active_output_bytes + crate::json::stringify(&LuaValue::String(line.clone()))?.len();
    if 2 + next_entry_bytes + count > 16 * 1_024 {
        return Err("Lua print output exceeds its aggregate byte limit".into());
    }
    state.active_output_bytes = next_entry_bytes;
    state.active_output.as_mut().expect("active output exists").push(line);
    Ok(())
}

fn resolve_player(
    lua: &Lua,
    shared: &Rc<RefCell<RuntimeState>>,
    candidate: &Value,
) -> Result<WebLuaPlayerState, String> {
    let id = if matches!(candidate, Value::Nil) {
        shared.borrow().active_player_id.clone()
    } else {
        Some(require_text(candidate, "player id", 128)?)
    };
    let id = id.ok_or("Lua player operation has no active player")?;
    frame(lua, shared)?
        .players
        .into_iter()
        .find(|player| player.id == id)
        .ok_or_else(|| format!("unknown player: {id}"))
}
fn stock_enemy(value: &Value) -> Option<WebLuaStockEnemy> {
    WEB_LUA_STOCK_ENEMIES.into_iter().find(|enemy| match value {
        Value::Integer(value) => *value == i64::from(enemy.token.native_type_id()),
        Value::Number(value) => *value == f64::from(enemy.token.native_type_id()),
        Value::String(value) => {
            let value = value.to_string_lossy().to_lowercase();
            value == enemy.key || value == enemy.token.id().to_lowercase()
        }
        _ => false,
    })
}
fn enemy_descriptor(enemy: WebLuaStockEnemy) -> LuaValue {
    object([
        ("base", LuaValue::String(enemy.key.into())),
        ("key", LuaValue::String(enemy.key.into())),
        ("native_type_id", LuaValue::Number(f64::from(enemy.token.native_type_id()))),
    ])
}
fn object_options(value: &Value, message: &str) -> Result<Vec<(String, Value)>, String> {
    if let Value::Table(table) = value
        && let LuaTableEntries::Object(fields) = lua_table_entries(table)?
    {
        return Ok(fields);
    }
    Err(message.into())
}
