//! Semantic projections from the Source SDK, over its one lazily sampled frame.
use super::*;
use serde_json::json;

pub(super) fn invoke(
    lua: &Lua,
    shared: &Rc<RefCell<RuntimeState>>,
    operation: &str,
    first: Value,
) -> Result<MultiValue, String> {
    let value = match operation {
        "enemies.get" => stock_enemy(&first).map_or(LuaValue::Null, enemy_descriptor),
        "enemies.list" => {
            LuaValue::Array(WEB_LUA_STOCK_ENEMIES.into_iter().map(enemy_descriptor).collect())
        }
        "events.names" => json_value(WEB_LUA_EVENT_NAMES)?,
        "rng.get_seed" => shared
            .borrow()
            .selected_run_seed
            .map_or(LuaValue::Null, |seed| LuaValue::Number(f64::from(seed))),
        "runtime.get_capabilities" => json_value(WEB_LUA_CAPABILITIES)?,
        "runtime.get_mod" => {
            let identity = &shared.borrow().identity;
            json_value(json!({
                "api_version": WEB_LUA_API_VERSION,
                "id": identity.id,
                "name": identity.name,
                "version": identity.version,
            }))?
        }
        "runtime.has_capability" => LuaValue::Boolean(
            WEB_LUA_CAPABILITIES.contains(&require_text(&first, "capability", 128)?.as_str()),
        ),
        "dev.list_items" => {
            let mut rows = Vec::new();
            for item in web_lua_developer_items() {
                let mut row =
                    json_value(&item)?.as_object().expect("catalog row is an object").clone();
                if item.native_subtype.is_none() {
                    row.remove("native_subtype");
                }
                if item.recipe_index.is_none() {
                    row.remove("recipe_index");
                }
                rows.push(LuaValue::Object(row));
            }
            LuaValue::Array(rows)
        }
        "dev.list_skills" => json_value(web_lua_developer_skills())?,
        "dev.list_welds" => json_value(web_lua_developer_welds())?,
        "player.get_state" => player_state(&resolve_player(lua, shared, &first)?)?,
        _ => frame_read(lua, shared, operation)?,
    };
    reply(lua, value)
}
fn frame_read(
    lua: &Lua,
    shared: &Rc<RefCell<RuntimeState>>,
    operation: &str,
) -> Result<LuaValue, String> {
    let frame = frame(lua, shared)?;
    match operation {
        "gameplay.get_state" => json_value(json!({
            "phase": frame.phase,
            "run_id": frame.run_id,
            "tick": frame.tick,
        })),
        "gameplay.get_combat_state" => {
            if frame.world != "boneyard" {
                return Ok(LuaValue::Null);
            }
            let wave_index = frame
                .waves
                .as_ref()
                .and_then(|waves| waves.get("wave_ordinal"))
                .and_then(LuaValue::as_number)
                .unwrap_or(0.0);
            json_value(json!({
                "active": frame.phase == "active",
                "run_id": frame.run_id,
                "tick": frame.tick,
                "wave_index": wave_index,
            }))
        }
        "hub.get_surface_state" => {
            let hub = frame.world == "hub";
            json_value(json!({
                "active": hub,
                "player_count": frame.player_count,
                "scene": frame.scene,
                "shared_hub": hub && frame.scene == "hub.courtyard",
                "surface_active": hub,
                "valid": hub,
            }))
        }
        "player.list" => {
            Ok(LuaValue::Array(frame.players.iter().map(player_state).collect::<Result<_, _>>()?))
        }
        "runtime.get_frame_state" => {
            let mut value =
                json_value(&frame)?.as_object().expect("typed frame is an object").clone();
            value.insert(
                "frame_count".into(),
                LuaValue::Number(shared.borrow().current_tick as f64),
            );
            value.insert(
                "observed_ms".into(),
                LuaValue::Number((shared.borrow_mut().clock)().floor()),
            );
            Ok(LuaValue::Object(value))
        }
        "runtime.get_multiplayer_state" => {
            let players = frame.players.iter().map(player_state).collect::<Result<Vec<_>, _>>()?;
            json_value(json!({
                "authority_player_id": frame.authority_player_id,
                "is_authority": shared.borrow().active_player_id == frame.authority_player_id,
                "participants": players,
                "player_count": frame.player_count,
            }))
        }
        "scene.get_state" | "world.get_scene" => json_value(json!({
            "can_enter_run": frame.world == "hub" && frame.phase == "hub",
            "can_switch_region": false,
            "is_authority": frame.authority_player_id.is_some(),
            "kind": if frame.world == "boneyard" { "arena" } else { "hub" },
            "name": frame.scene,
            "run_id": frame.run_id,
            "scene_id": frame.run_id.as_ref().unwrap_or(&frame.scene),
            "scene_key": frame.scene,
            "seed": frame.boneyard_seed,
            "tick": frame.tick,
            "transitioning": false,
            "web_world": frame.world,
        })),
        "waves.get_state" => Ok(frame.waves.map_or(LuaValue::Null, LuaValue::Object)),
        "world.get_state" => json_value(json!({
            "enemy_count": frame.enemies.len(),
            "player_count": frame.player_count,
            "run_id": frame.run_id,
            "tick": frame.tick,
            "world": frame.world,
        })),
        "world.list_actors" => {
            let mut actors = Vec::new();
            for player in &frame.players {
                actors.push(json_value(json!({
                    "dead": player.life_state != "alive",
                    "hp": player.current_health,
                    "id": player.id,
                    "kind": "player",
                    "life_state": player.life_state,
                    "max_hp": player.maximum_health,
                    "position": player.position,
                    "tracked_enemy": false,
                    "x": player.position.x,
                    "y": player.position.y,
                }))?);
            }
            for enemy in &frame.enemies {
                let descriptor = stock_enemy(&Value::String(
                    lua.create_string(&enemy.token).map_err(|error| bounded_error(&error))?,
                ))
                .ok_or_else(|| format!("unknown stock enemy: {}", enemy.token))?;
                let native_id = descriptor.token.native_type_id();
                actors.push(json_value(json!({
                    "dead": enemy.life_state != "alive",
                    "enemy_type": native_id,
                    "hp": enemy.health,
                    "id": enemy.id,
                    "kind": "enemy",
                    "life_state": enemy.life_state,
                    "max_hp": enemy.maximum_health,
                    "network_actor_id": enemy.id,
                    "object_type_id": native_id,
                    "position": enemy.position,
                    "token": enemy.token,
                    "tracked_enemy": true,
                    "x": enemy.position.x,
                    "y": enemy.position.y,
                }))?);
            }
            Ok(LuaValue::Array(actors))
        }
        _ => Err(format!("unknown Console SDK operation: {operation}")),
    }
}
fn player_state(player: &WebLuaPlayerState) -> Result<LuaValue, String> {
    json_value(json!({
        "currentHealth": player.current_health,
        "currentMana": player.current_mana,
        "dead": player.life_state != "alive",
        "death_tick": player.death_tick,
        "discipline": player.discipline,
        "display_name": player.display_name,
        "element": player.element,
        "experience": player.experience,
        "gold": player.gold,
        "hp": player.current_health,
        "id": player.id,
        "level": player.level,
        "life_state": player.life_state,
        "max_hp": player.maximum_health,
        "max_mp": player.maximum_mana,
        "mp": player.current_mana,
        "pending_level_up": player.pending_level_up,
        "position": player.position,
        "x": player.position.x,
        "xp": player.experience,
        "y": player.position.y,
    }))
}
