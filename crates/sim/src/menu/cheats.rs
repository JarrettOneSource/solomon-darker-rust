//! Website `cheat-menu-contract.ts`: admitted availability, semantic Lua actions,
//! actual VM catalog decoding and console history. This module never executes Lua.

use crate::modding::value::LuaValue;
use crate::profile::{WizardDiscipline, WizardElement};

pub const CHEAT_MENU_EXPERIENCE_MAX: f64 = 10_000_000.0;
pub const CHEAT_MENU_GOLD_MAX: f64 = 10_000_000.0;
pub const CHEAT_MENU_RUN_SEED_MAX: f64 = 1_073_741_823.0;
pub const CHEAT_MENU_SPAWN_COUNT_MAX: f64 = 20.0;
pub const CHEAT_MENU_CONSOLE_HISTORY_LIMIT: usize = 50;
pub const MAX_LUA_CONSOLE_CODE_LENGTH: usize = 48 * 1024;
pub const CHEAT_MENU_CATALOG_QUERY: &str = "local developer = type(sd.dev) == 'table'\nreturn sd.enemies.list(),\n  developer and sd.dev.list_items() or {},\n  developer and sd.dev.list_skills() or {},\n  developer and sd.dev.list_welds() or {}";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheatMenuEnemyDescriptor {
    pub base: String,
    pub key: String,
    pub native_type_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheatMenuItemDescriptor {
    pub key: String,
    pub kind: String,
    pub name: String,
    pub native_type_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheatMenuSkillDescriptor {
    pub family: String,
    pub id: i64,
    pub maximum_rank: i64,
    pub name: String,
    pub weld_only: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CheatMenuWeldDescriptor {
    // Source requires Number.isInteger here, rather than Number.isSafeInteger.
    pub component_skill_ids: Vec<f64>,
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CheatMenuCatalogs {
    pub enemies: Vec<CheatMenuEnemyDescriptor>,
    pub items: Vec<CheatMenuItemDescriptor>,
    pub skills: Vec<CheatMenuSkillDescriptor>,
    pub welds: Vec<CheatMenuWeldDescriptor>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CheatMenuAction {
    RestoreHealth { player_id: String },
    RestoreMana { player_id: String },
    SetGold { gold: f64, player_id: String },
    GrantExperience { amount: f64, player_id: String },
    SetRunSeed { seed: f64 },
    SpawnNextWave,
    SpawnEnemy { count: f64, enemy_key: String, player_id: String },
    GrantItem { item_key: String, player_id: String, quantity: f64 },
    GrantSkill { player_id: String, ranks: f64, skill_id: f64 },
    GrantWeld { build_id: f64, player_id: String },
    SummonBot { discipline: WizardDiscipline, element: WizardElement },
}

#[must_use]
pub const fn game_cheat_menu_available(
    developer_access: bool,
    cheats_enabled: bool,
    is_host: bool,
) -> bool {
    developer_access || (cheats_enabled && is_host)
}

pub fn compile_cheat_menu_action(action: &CheatMenuAction) -> Result<String, String> {
    Ok(match action {
        CheatMenuAction::RestoreHealth { player_id } => {
            format!("return sd.player.restore_health(10000000, {})", lua_string_literal(player_id)?)
        }
        CheatMenuAction::RestoreMana { player_id } => {
            format!("return sd.player.set_mana(10000000, {})", lua_string_literal(player_id)?)
        }
        CheatMenuAction::SetGold { gold, player_id } => format!(
            "return sd.player.set_gold({}, {})",
            integer_within(*gold, "Gold", 0.0, CHEAT_MENU_GOLD_MAX)?,
            lua_string_literal(player_id)?
        ),
        CheatMenuAction::GrantExperience { amount, player_id } => format!(
            "return sd.player.grant_experience({}, {})",
            integer_within(*amount, "experience", 0.0, CHEAT_MENU_EXPERIENCE_MAX)?,
            lua_string_literal(player_id)?
        ),
        CheatMenuAction::SetRunSeed { seed } => format!(
            "return sd.rng.set_seed({})",
            integer_within(*seed, "run seed", 1.0, CHEAT_MENU_RUN_SEED_MAX)?
        ),
        CheatMenuAction::SpawnNextWave => "return sd.waves.spawn_next()".to_owned(),
        CheatMenuAction::SpawnEnemy { count, enemy_key, player_id } => {
            let count = integer_within(*count, "enemy count", 1.0, CHEAT_MENU_SPAWN_COUNT_MAX)?;
            format!(
                "local player = sd.player.get_state({})\nfor index = 1, {count} do\n  sd.enemies.spawn({}, {{\n    x = player.x + 80 + ((index - 1) % 5) * 32,\n    y = player.y + math.floor((index - 1) / 5) * 32\n  }})\nend\nreturn {count}",
                lua_string_literal(player_id)?,
                lua_string_literal(enemy_key)?
            )
        }
        CheatMenuAction::GrantItem { item_key, player_id, quantity } => format!(
            "return sd.dev.grant_item({}, {}, {})",
            lua_string_literal(item_key)?,
            integer_within(*quantity, "item quantity", 1.0, 100.0)?,
            lua_string_literal(player_id)?
        ),
        CheatMenuAction::GrantSkill { player_id, ranks, skill_id } => format!(
            "return sd.dev.grant_skill({}, {}, {})",
            integer_within(*skill_id, "skill id", 8.0, 79.0)?,
            integer_within(*ranks, "skill ranks", 1.0, 100.0)?,
            lua_string_literal(player_id)?
        ),
        CheatMenuAction::GrantWeld { build_id, player_id } => format!(
            "return sd.dev.grant_weld({}, {})",
            integer_within(*build_id, "Weld build id", 1000.0, 1009.0)?,
            lua_string_literal(player_id)?
        ),
        CheatMenuAction::SummonBot { discipline, element } => {
            if !WizardDiscipline::CHOICES.contains(discipline) {
                return Err(format!("unsupported bot discipline: {}", discipline.as_str()));
            }
            format!(
                "return sd.bots.summon({{discipline = {}, element = {}}})",
                lua_string_literal(discipline.as_str())?,
                lua_string_literal(element.as_str())?
            )
        }
    })
}

pub fn decode_cheat_menu_catalogs(values: &[LuaValue]) -> Result<CheatMenuCatalogs, String> {
    if values.len() != 4 {
        return Err("cheat catalog query must return four values".to_owned());
    }
    Ok(CheatMenuCatalogs {
        enemies: decode_catalog(&values[0], "enemy", false, |value, index| {
            let row = record(value, &format!("enemy catalog row {index}"))?;
            Ok(CheatMenuEnemyDescriptor {
                base: string_field(row, "base")?,
                key: string_field(row, "key")?,
                native_type_id: integer_field(row, "native_type_id")?,
            })
        })?,
        items: decode_catalog(&values[1], "item", true, |value, index| {
            let row = record(value, &format!("item catalog row {index}"))?;
            Ok(CheatMenuItemDescriptor {
                key: string_field(row, "key")?,
                kind: string_field(row, "kind")?,
                name: string_field(row, "name")?,
                native_type_id: integer_field(row, "native_type_id")?,
            })
        })?,
        skills: decode_catalog(&values[2], "skill", true, |value, index| {
            let row = record(value, &format!("skill catalog row {index}"))?;
            Ok(CheatMenuSkillDescriptor {
                family: string_field(row, "family")?,
                id: integer_field(row, "id")?,
                maximum_rank: integer_field(row, "maximum_rank")?,
                name: string_field(row, "name")?,
                weld_only: boolean_field(row, "weld_only")?,
            })
        })?,
        welds: decode_catalog(&values[3], "Weld", true, |value, index| {
            let row = record(value, &format!("Weld catalog row {index}"))?;
            let components =
                row.get("component_skill_ids").and_then(LuaValue::as_array).ok_or_else(|| {
                    format!("Weld catalog row {index}.component_skill_ids must be integers")
                })?;
            let mut component_skill_ids = Vec::with_capacity(components.len());
            for component in components {
                let value = component
                    .as_number()
                    .filter(|value| value.is_finite() && value.fract() == 0.0)
                    .ok_or_else(|| {
                        format!("Weld catalog row {index}.component_skill_ids must be integers")
                    })?;
                component_skill_ids.push(value);
            }
            Ok(CheatMenuWeldDescriptor {
                component_skill_ids,
                id: integer_field(row, "id")?,
                name: string_field(row, "name")?,
            })
        })?,
    })
}

pub fn append_cheat_console_history(history: &mut Vec<String>, code: &str) {
    let command = code.trim_matches(crate::native_ui_text::is_javascript_whitespace);
    if command.is_empty() || history.last().is_some_and(|last| last == command) {
        return;
    }
    history.push(command.to_owned());
    if history.len() > CHEAT_MENU_CONSOLE_HISTORY_LIMIT {
        history.drain(..history.len() - CHEAT_MENU_CONSOLE_HISTORY_LIMIT);
    }
}

fn integer_within(value: f64, label: &str, minimum: f64, maximum: f64) -> Result<i64, String> {
    if !safe_integer(value) || value < minimum || value > maximum {
        return Err(format!("{label} must be within {minimum}..{maximum}"));
    }
    Ok(value as i64)
}

fn lua_string_literal(value: &str) -> Result<String, String> {
    // JavaScript checks UTF-16 length, including both halves of an astral glyph.
    if value.is_empty() || value.encode_utf16().count() > 128 {
        return Err(
            "Lua action identity must be a nonempty string of at most 128 characters".to_owned()
        );
    }
    let escaped = value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    Ok(format!("'{escaped}'"))
}

fn safe_integer(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0
}

fn decode_catalog<T>(
    value: &LuaValue,
    label: &str,
    accepts_empty_object: bool,
    decode: impl Fn(&LuaValue, usize) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    let values = match value {
        LuaValue::Array(values) => values,
        LuaValue::Object(object) if accepts_empty_object && object.is_empty() => {
            return Ok(Vec::new());
        }
        _ => return Err(format!("{label} catalog must be an array")),
    };
    let mut decoded = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        decoded.push(decode(value, index)?);
    }
    Ok(decoded)
}

fn record<'a>(
    value: &'a LuaValue,
    label: &str,
) -> Result<&'a crate::modding::value::LuaObject, String> {
    value.as_object().ok_or_else(|| format!("{label} must be an object"))
}

fn string_field(row: &crate::modding::value::LuaObject, key: &str) -> Result<String, String> {
    row.get(key)
        .and_then(LuaValue::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} must be a nonempty string"))
}

fn integer_field(row: &crate::modding::value::LuaObject, key: &str) -> Result<i64, String> {
    row.get(key)
        .and_then(LuaValue::as_number)
        .filter(|value| safe_integer(*value))
        .map(|value| value as i64)
        .ok_or_else(|| format!("{key} must be an integer"))
}

fn boolean_field(row: &crate::modding::value::LuaObject, key: &str) -> Result<bool, String> {
    row.get(key).and_then(LuaValue::as_bool).ok_or_else(|| format!("{key} must be a Boolean"))
}
