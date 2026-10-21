//! Source argument admission precedes the one actual queued-mutation producer.
use super::*;
use values::js_string;

pub(super) fn invoke(
    lua: &Lua,
    shared: &Rc<RefCell<RuntimeState>>,
    operation: &str,
    args: MultiValue,
) -> Result<MultiValue, String> {
    let first = args.front().cloned().unwrap_or(Value::Nil);
    let second = args.get(1).cloned().unwrap_or(Value::Nil);
    let third = args.get(2).cloned().unwrap_or(Value::Nil);
    let command = match operation {
        "player.restore_mana" => {
            if !args.is_empty() {
                return Err("sd.player.restore_mana expects no arguments".into());
            }
            let player = resolve_player(lua, shared, &Value::Nil)?;
            shared.borrow_mut().queue(WebLuaCommand::RestoreMana {
                amount: (player.maximum_mana - player.current_mana).max(0.0),
                player_id: player.id,
            })?;
            return Ok(MultiValue::from_vec(vec![
                Value::Boolean(true),
                Value::Number(player.maximum_mana),
            ]));
        }
        "player.grant_experience"
        | "player.restore_health"
        | "player.set_gold"
        | "player.set_mana" => {
            let (field, integer) = match operation {
                "player.grant_experience" => ("experience amount", false),
                "player.restore_health" => ("health amount", false),
                "player.set_gold" => ("gold", true),
                _ => ("mana", false),
            };
            let value = if integer {
                integer_within(&first, field, 0, 10_000_000)? as f64
            } else {
                finite_within(&first, field, 0.0, 10_000_000.0)?
            };
            let id = resolve_player(lua, shared, &second)?.id;
            match operation {
                "player.grant_experience" => {
                    WebLuaCommand::GrantExperience { amount: value, player_id: id }
                }
                "player.restore_health" => {
                    WebLuaCommand::RestoreHealth { amount: value, player_id: id }
                }
                "player.set_gold" => WebLuaCommand::SetGold { value: value as u64, player_id: id },
                _ => WebLuaCommand::SetMana { value, player_id: id },
            }
        }
        "dev.grant_gold" => {
            let id = resolve_player(lua, shared, &second)?.id;
            let amount = integer_within(&first, "Gold grant", 1, 10_000_000)? as f64;
            WebLuaCommand::GrantGold { amount, player_id: id }
        }
        "dev.grant_item" => {
            let key = require_text(&first, "developer item key", 128)?;
            if !web_lua_developer_item_exists(&key) {
                return Err(format!("unknown developer item: {key}"));
            }
            let id = resolve_player(lua, shared, &third)?.id;
            let quantity = if matches!(second, Value::Nil) {
                1
            } else {
                integer_within(&second, "item quantity", 1, 100)?
            };
            WebLuaCommand::GrantItem { item_key: key, player_id: id, quantity: quantity as f64 }
        }
        "dev.grant_skill" => {
            let id = integer_within(&first, "skill id", 8, 79)? as u32;
            let skill = web_lua_developer_skill(id)
                .ok_or_else(|| format!("unknown developer skill: {id}"))?;
            if skill.weld_only {
                return Err("Spell Welding must be granted with sd.dev.grant_weld".into());
            }
            let player_id = resolve_player(lua, shared, &third)?.id;
            let ranks = if matches!(second, Value::Nil) {
                1
            } else {
                integer_within(&second, "skill ranks", 1, u64::from(skill.maximum_rank))?
            };
            WebLuaCommand::GrantSkill { player_id, ranks: ranks as f64, skill_id: id }
        }
        "dev.grant_weld" => {
            let id = integer_within(&first, "Weld build id", 1000, 1009)? as u32;
            if web_lua_developer_weld(id).is_none() {
                return Err(format!("unknown native Weld build: {id}"));
            }
            WebLuaCommand::GrantWeld {
                build_id: id,
                player_id: resolve_player(lua, shared, &second)?.id,
            }
        }
        "rng.set_seed" => {
            let frame = frame(lua, shared)?;
            if frame.world != "hub" || frame.phase != "hub" {
                return Err("sd.rng.set_seed may only be called in the Hub before a run".into());
            }
            let seed = nonnegative_integer(&first, "run seed")?;
            if !(1..=0x3fff_ffff).contains(&seed) {
                return Err("run seed must be within 1..0x3fffffff".into());
            }
            shared.borrow_mut().selected_run_seed = Some(seed as u32);
            shared.borrow_mut().queue(WebLuaCommand::SetNextRunSeed { seed: seed as u32 })?;
            return reply(lua, LuaValue::Number(seed as f64));
        }
        "waves.spawn_next" => {
            let frame = frame(lua, shared)?;
            let available = frame.waves.as_ref().is_some_and(|waves| {
                waves.get("spawn_next_available") == Some(&LuaValue::Boolean(true))
            });
            if frame.world != "boneyard"
                || frame.phase != "active"
                || frame.run_id.is_none()
                || !available
            {
                return Err("Next wave requires an active survival wave with no pending spawns, level-up barrier or boss hold.".into());
            }
            let value = match frame.waves.as_ref().and_then(|waves| waves.get("wave_event_id")) {
                Some(LuaValue::Number(value)) => Value::Number(*value),
                _ => Value::Nil,
            };
            let wave_event_id = nonnegative_integer(&value, "wave event id")?;
            WebLuaCommand::SpawnNextWave {
                run_id: frame.run_id.expect("admitted active run"),
                wave_event_id,
            }
        }
        "enemies.spawn" => {
            let frame = frame(lua, shared)?;
            if frame.world != "boneyard" || frame.phase != "active" {
                return Err("sd.enemies.spawn requires an active Boneyard run".into());
            }
            let descriptor = stock_enemy(&first)
                .ok_or_else(|| format!("unknown stock enemy: {}", js_string(&first)))?;
            let fields = object_options(&second, "sd.enemies.spawn options must be a table")?;
            for (key, _) in &fields {
                if !matches!(key.as_str(), "x" | "y") {
                    return Err(format!("unsupported sd.enemies.spawn option: {key}"));
                }
            }
            let get = |key: &str| {
                fields
                    .iter()
                    .find(|(name, _)| name == key)
                    .map_or(Value::Nil, |(_, value)| value.clone())
            };
            let x = finite_within(&get("x"), "enemy x", -1_000_000.0, 1_000_000.0)?;
            let y = finite_within(&get("y"), "enemy y", -1_000_000.0, 1_000_000.0)?;
            let mut state = shared.borrow_mut();
            let request_id = state.next_spawn_request_id;
            state.next_spawn_request_id += 1;
            state.queue(WebLuaCommand::SpawnEnemy { request_id, token: descriptor.token, x, y })?;
            let mut value =
                enemy_descriptor(descriptor).as_object().expect("descriptor is an object").clone();
            value.insert("request_id".into(), LuaValue::Number(request_id as f64));
            value.insert("x".into(), LuaValue::Number(x));
            value.insert("y".into(), LuaValue::Number(y));
            return reply(lua, LuaValue::Object(value));
        }
        "bots.summon" => {
            let fields = if matches!(first, Value::Nil) {
                Vec::new()
            } else {
                object_options(&first, "sd.bots.summon options must be a table")?
            };
            for (key, _) in &fields {
                if !matches!(key.as_str(), "discipline" | "element") {
                    return Err(format!("unsupported sd.bots.summon option: {key}"));
                }
            }
            let get = |key: &str, default: &str| -> Result<String, String> {
                match fields.iter().find(|(name, _)| name == key) {
                    Some((_, value)) => require_text(value, &format!("bot {key}"), 16),
                    None => Ok(default.into()),
                }
            };
            let discipline = get("discipline", "arcane")?;
            let element = get("element", "fire")?;
            if !["arcane", "body", "mind"].contains(&discipline.as_str()) {
                return Err(format!("unsupported bot discipline: {discipline}"));
            }
            if !["air", "earth", "ether", "fire", "water"].contains(&element.as_str()) {
                return Err(format!("unsupported bot element: {element}"));
            }
            let provider: Table =
                lua.named_registry_value(PROVIDER).map_err(|error| bounded_error(&error))?;
            let value = provider
                .get::<Function>("summon")
                .and_then(|function| function.call::<Value>((discipline, element)))
                .map_err(|error| bounded_error(&error))?;
            return Ok(MultiValue::from_vec(vec![value]));
        }
        _ => return Err(format!("unknown Console mutation: {operation}")),
    };
    shared.borrow_mut().queue(command)?;
    reply(lua, LuaValue::Boolean(true))
}
