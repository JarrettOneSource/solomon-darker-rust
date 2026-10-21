use solomon_sim::modding::definition::{CompiledWebLuaMod, WebLuaScopeKind};
use solomon_sim::modding::runtime::*;
use solomon_sim::modding::{LuaObject, LuaValue};

use super::content_owner;

pub fn decode_prepared_mod_save_state(
    mods: &[CompiledWebLuaMod],
    state: &LuaObject,
) -> Result<PreparedModHostCheckpoint, String> {
    let first_mod = mods.first().ok_or("cannot restore mod state without prepared mods")?;
    let first = record(state.get(&first_mod.identity.id), "mod runtime state")?;
    let runtime = record(first.get("runtime"), "mod runtime metadata")?;
    let mut cells = Vec::new();
    let mut statuses = Vec::new();
    let mut powerups = Vec::new();
    let mut cooldowns = Vec::new();
    let mut timers = Vec::new();
    let mut semantic_values = Vec::new();
    let mut spell_effects = Vec::new();
    for (index, module) in mods.iter().enumerate() {
        let owner = &module.identity.id;
        let entry = record(state.get(owner), &format!("mod state {owner}"))?;
        if entry.get("api").and_then(LuaValue::as_str) != Some("1.0.0")
            || entry.get("graph_sha256").and_then(LuaValue::as_str)
                != Some(module.graph_sha256.as_str())
        {
            return Err(format!("mod state graph does not match {owner}"));
        }
        for value in array(entry.get("state_cells"), &format!("{owner}.state_cells"))? {
            let row = record(Some(value), "mod state cell")?;
            let kind = scope_kind(row, "mod state scope kind")?;
            cells.push(ModStateCellSnapshot {
                key: text(row.get("key"), "mod state key")?,
                mod_id: owner.clone(),
                schema_version: positive(row.get("schema_version"), "mod state schema version")?,
                scope: ModStateScope { id: text(row.get("scope_id"), "mod state scope id")?, kind },
                value: row.get("value").cloned().unwrap_or_default(),
            });
        }
        for value in array(entry.get("timers"), &format!("{owner}.timers"))? {
            let row = record(Some(value), "mod rule timer")?;
            let kind = scope_kind(row, "mod timer scope kind")?;
            timers.push(ModRuleTimerCheckpoint {
                context: record(row.get("context"), "mod timer context")?.clone(),
                due_tick: nonnegative(row.get("due_tick"), "mod timer due tick")?,
                event: text(row.get("event"), "mod timer event")?,
                id: positive(row.get("id"), "mod timer id")?,
                interval_ticks: if row.get("interval_ticks") == Some(&LuaValue::Null) {
                    None
                } else {
                    Some(positive(row.get("interval_ticks"), "mod timer interval")?)
                },
                mod_id: owner.clone(),
                node_id: text(row.get("node_id"), "mod timer node")?,
                payload: row.get("payload").cloned().unwrap_or_default(),
                remaining: positive(row.get("remaining"), "mod timer remaining")?,
                scope: ModStateScope { id: text(row.get("scope_id"), "mod timer scope id")?, kind },
            });
        }
        for value in array(entry.get("statuses"), &format!("{owner}.statuses"))? {
            let row = record(Some(value), "mod status")?;
            let content_id = content(row.get("content_id"), owner, mods)?;
            statuses.push(ActiveModStatus {
                content_id,
                expires_tick: positive(row.get("expires_tick"), "mod status expiry")?,
                instance_id: positive(row.get("instance_id"), "mod status instance")?,
                mod_id: owner.clone(),
                started_tick: nonnegative(row.get("started_tick"), "mod status start")?,
                target_id: text(row.get("target_id"), "mod status target")?,
            });
        }
        for value in array(entry.get("powerups"), &format!("{owner}.powerups"))? {
            let row = record(Some(value), "mod powerup")?;
            powerups.push(ActiveModPowerup {
                content_id: content(row.get("content_id"), owner, mods)?,
                id: positive(row.get("id"), "mod powerup id")?,
                mod_id: owner.clone(),
                spawned_tick: nonnegative(row.get("spawned_tick"), "mod powerup spawn")?,
                x: finite(row.get("x"), "mod powerup x")?,
                y: finite(row.get("y"), "mod powerup y")?,
            });
        }
        for value in array(entry.get("spell_cooldowns"), &format!("{owner}.spell_cooldowns"))? {
            let row = record(Some(value), "mod spell cooldown")?;
            cooldowns.push(ModSpellCooldown {
                content_id: content(row.get("content_id"), owner, mods)?,
                player_id: text(row.get("player_id"), "mod spell player")?,
                ready_tick: nonnegative(row.get("ready_tick"), "mod spell ready tick")?,
            });
        }
        for value in array(entry.get("spell_effects"), &format!("{owner}.spell_effects"))? {
            let row = record(Some(value), "mod spell effect")?;
            let scope = scope_kind(row, "mod spell effect scope kind")?;
            let kind = match text(row.get("kind"), "mod spell effect kind")?.as_str() {
                "area" => ModSpellEffectKind::Area,
                "channel" => ModSpellEffectKind::Channel,
                "projectile" => ModSpellEffectKind::Projectile,
                _ => return Err("mod spell effect kind is invalid".into()),
            };
            // The content check precedes template validation in the Source object
            // expression. Preserve that first-error boundary as well as its data.
            let content_id = content(row.get("content_id"), owner, mods)?;
            let mut effects = Vec::new();
            for value in array(row.get("effects"), "mod spell effect templates")? {
                let template = record(Some(value), "mod spell effect template")?;
                effects.push(ModEffectTemplate {
                    fields: record(template.get("fields"), "mod spell effect fields")?.clone(),
                    kind: text(template.get("kind"), "mod spell effect template kind")?,
                });
            }
            spell_effects.push(ActiveModSpellEffect {
                content_id,
                effects,
                expires_tick: positive(row.get("expires_tick"), "mod spell effect expiry")?,
                hit_targets: texts(
                    row.get("hit_targets"),
                    "mod spell hit targets",
                    "mod spell hit target",
                )?,
                id: positive(row.get("id"), "mod spell effect id")?,
                interval_ticks: positive(row.get("interval_ticks"), "mod spell effect interval")?,
                kind,
                last_tick: nonnegative(row.get("last_tick"), "mod spell effect last tick")?,
                mod_id: owner.clone(),
                next_pulse_tick: nonnegative(
                    row.get("next_pulse_tick"),
                    "mod spell effect next pulse",
                )?,
                owner_player_id: text(row.get("owner_player_id"), "mod spell effect owner")?,
                radius: finite(row.get("radius"), "mod spell effect radius")?,
                scope: ModStateScope {
                    id: text(row.get("scope_id"), "mod spell effect scope id")?,
                    kind: scope,
                },
                speed_per_tick: finite(row.get("speed_per_tick"), "mod spell effect speed")?,
                started_tick: nonnegative(row.get("started_tick"), "mod spell effect start")?,
                target_x: finite(row.get("target_x"), "mod spell effect target x")?,
                target_y: finite(row.get("target_y"), "mod spell effect target y")?,
                x: finite(row.get("x"), "mod spell effect x")?,
                y: finite(row.get("y"), "mod spell effect y")?,
            });
        }
        for value in array(entry.get("state_values"), &format!("{owner}.state_values"))? {
            let row = record(Some(value), "mod semantic state")?;
            let kind = scope_kind(row, "mod semantic state scope kind")?;
            semantic_values.push(ModSemanticValue {
                key: text(row.get("key"), "mod semantic state key")?,
                mod_id: owner.clone(),
                scope: ModStateScope {
                    id: text(row.get("scope_id"), "mod semantic state scope id")?,
                    kind,
                },
                value: row.get("value").cloned().unwrap_or_default(),
            });
        }
        if index > 0 && entry.get("runtime").is_some() {
            return Err("mod runtime metadata is duplicated".into());
        }
    }
    let mut enemies = Vec::new();
    for value in array(runtime.get("enemies"), "mod enemies")? {
        let row = record(Some(value), "mod enemy")?;
        enemies.push(ActiveModEnemy {
            content_id: text(row.get("content_id"), "mod enemy content")?,
            current_health: finite(row.get("current_health"), "mod enemy health")?,
            death_tick: optional_nonnegative(row.get("death_tick"), "mod enemy death")?,
            heading_index: nonnegative(row.get("heading_index"), "mod enemy heading")?,
            id: positive(row.get("id"), "mod enemy id")?,
            last_attack_tick: optional_nonnegative(
                row.get("last_attack_tick"),
                "mod enemy last attack",
            )?,
            last_damaged_by_player_id: optional_text(
                row.get("last_damaged_by_player_id"),
                "mod enemy damage owner",
            )?,
            life_state: ModEnemyLifeState::from_source(text(
                row.get("life_state"),
                "mod enemy life",
            )?),
            maximum_health: finite(row.get("maximum_health"), "mod enemy maximum health")?,
            moving: boolean(row.get("moving"), "mod enemy moving")?,
            next_attack_tick: nonnegative(row.get("next_attack_tick"), "mod enemy next attack")?,
            spawned_tick: nonnegative(row.get("spawned_tick"), "mod enemy spawn")?,
            target_player_id: optional_text(row.get("target_player_id"), "mod enemy target")?,
            x: finite(row.get("x"), "mod enemy x")?,
            y: finite(row.get("y"), "mod enemy y")?,
        });
    }
    let mut scenes = Vec::new();
    for value in array(runtime.get("scenes"), "mod scenes")? {
        let row = record(Some(value), "mod scene")?;
        scenes.push(ActiveModScene {
            epoch: positive(row.get("epoch"), "mod scene epoch")?,
            owner_id: text(row.get("owner_id"), "mod scene owner")?,
            parent_content_id: optional_text(row.get("parent_content_id"), "mod scene parent")?,
            room_index: nonnegative(row.get("room_index"), "mod scene room")?,
            scene_content_id: text(row.get("scene_content_id"), "mod scene content")?,
        });
    }
    let mut stacks = Vec::new();
    for value in array(runtime.get("scene_stacks"), "mod scene stacks")? {
        let row = record(Some(value), "mod scene stack")?;
        stacks.push(ModSceneStack {
            owner_id: text(row.get("owner_id"), "mod scene stack owner")?,
            room_indexes: array(row.get("room_indexes"), "mod scene stack rooms")?
                .iter()
                .map(|value| nonnegative(Some(value), "mod scene stack room"))
                .collect::<Result<_, _>>()?,
            scene_content_ids: texts(
                row.get("scene_content_ids"),
                "mod scene stack ids",
                "mod scene stack id",
            )?,
        });
    }
    let mut stock = Vec::new();
    for value in array(runtime.get("shop_stock"), "mod shop stock")? {
        let row = record(Some(value), "mod shop stock row")?;
        stock.push(ModShopStockState {
            player_id: text(row.get("player_id"), "mod shop player")?,
            remaining: nonnegative(row.get("remaining"), "mod shop remaining")?,
            restock_tick: optional_nonnegative(row.get("restock_tick"), "mod shop restock tick")?,
            row: nonnegative(row.get("row"), "mod shop row")?,
            shop_content_id: text(row.get("shop_content_id"), "mod shop content")?,
        });
    }
    let mut ranks = Vec::new();
    for value in array(runtime.get("skill_ranks"), "mod skill ranks")? {
        let row = record(Some(value), "mod skill rank")?;
        ranks.push(ModSkillRank {
            content_id: text(row.get("content_id"), "mod skill content")?,
            player_id: text(row.get("player_id"), "mod skill player")?,
            rank: positive(row.get("rank"), "mod skill rank")?,
        });
    }
    let mut offers = Vec::new();
    for value in array(runtime.get("skill_offers"), "mod skill offers")? {
        let row = record(Some(value), "mod skill offer")?;
        offers.push(ModSkillOffer {
            content_ids: texts(
                row.get("content_ids"),
                "mod skill offer ids",
                "mod skill offer id",
            )?,
            player_id: text(row.get("player_id"), "mod skill offer player")?,
            sequence: positive(row.get("sequence"), "mod skill offer sequence")?,
        });
    }
    let mut reducer_health = Vec::new();
    for value in array(runtime.get("reducer_health"), "mod reducer health")? {
        let row = record(Some(value), "mod reducer health row")?;
        reducer_health.push(ModReducerHealthCheckpoint {
            disabled: boolean(row.get("disabled"), "mod reducer disabled")?,
            failures: nonnegative(row.get("failures"), "mod reducer failures")?,
            key: text(row.get("key"), "mod reducer key")?,
            mod_id: text(row.get("mod_id"), "mod reducer owner")?,
        });
    }
    let mut bindings = Vec::new();
    for value in array(runtime.get("skill_bindings"), "mod skill bindings")? {
        let row = record(Some(value), "mod skill binding")?;
        bindings.push(ModSkillBinding {
            content_id: text(row.get("content_id"), "mod skill binding content")?,
            player_id: text(row.get("player_id"), "mod skill binding player")?,
            slot: nonnegative(row.get("slot"), "mod skill binding slot")?,
        });
    }
    Ok(PreparedModHostCheckpoint {
        enemies: ModEnemyCheckpoint {
            enemies,
            next_id: positive(runtime.get("enemy_next_id"), "mod enemy next id")?,
            revision: nonnegative(runtime.get("enemy_revision"), "mod enemy revision")?,
        },
        powerups: ModPowerupCheckpoint {
            instances: powerups,
            next_id: positive(runtime.get("powerup_next_id"), "mod powerup next id")?,
            revision: nonnegative(runtime.get("powerup_revision"), "mod powerup revision")?,
        },
        session: PreparedModCheckpoint {
            graph_sha256: mods.iter().map(|module| module.graph_sha256.clone()).collect(),
            rules: ModRuleEngineCheckpoint {
                next_intent_sequence: positive(
                    runtime.get("next_intent_sequence"),
                    "mod next intent sequence",
                )?,
                next_timer_id: positive(runtime.get("next_timer_id"), "mod next timer id")?,
                reducer_health,
                state: ModStateCheckpoint {
                    cells,
                    revision: nonnegative(runtime.get("state_revision"), "mod state revision")?,
                },
                timers,
            },
        },
        scenes: ModSceneCheckpoint {
            next_epoch: positive(runtime.get("scene_next_epoch"), "mod scene next epoch")?,
            scenes,
            stacks,
        },
        semantic_state: ModSemanticStateCheckpoint {
            revision: nonnegative(
                runtime.get("semantic_state_revision"),
                "mod semantic state revision",
            )?,
            values: semantic_values,
        },
        shops: ModShopCheckpoint {
            revision: nonnegative(runtime.get("shop_revision"), "mod shop revision")?,
            stock,
        },
        skills: ModSkillCheckpoint {
            bindings,
            offers,
            ranks,
            revision: nonnegative(runtime.get("skill_revision"), "mod skill revision")?,
        },
        spells: ModSpellCheckpoint {
            cooldowns,
            revision: nonnegative(runtime.get("spell_revision"), "mod spell revision")?,
        },
        spell_effects: ModSpellEffectCheckpoint {
            effects: spell_effects,
            next_id: positive(runtime.get("spell_effect_next_id"), "mod spell effect next id")?,
            next_sequence: positive(
                runtime.get("spell_effect_next_sequence"),
                "mod spell effect next sequence",
            )?,
            revision: nonnegative(
                runtime.get("spell_effect_revision"),
                "mod spell effect revision",
            )?,
        },
        statuses: ModStatusCheckpoint {
            instances: statuses,
            next_instance_id: positive(runtime.get("status_next_id"), "mod status next id")?,
            revision: nonnegative(runtime.get("status_revision"), "mod status revision")?,
        },
    })
}

fn record<'a>(value: Option<&'a LuaValue>, field: &str) -> Result<&'a LuaObject, String> {
    value.and_then(LuaValue::as_object).ok_or_else(|| format!("{field} must be an object"))
}

fn array<'a>(value: Option<&'a LuaValue>, field: &str) -> Result<&'a [LuaValue], String> {
    value.and_then(LuaValue::as_array).ok_or_else(|| format!("{field} must be an array"))
}

fn text(value: Option<&LuaValue>, field: &str) -> Result<String, String> {
    value
        .and_then(LuaValue::as_str)
        .filter(|value| !value.is_empty() && value.encode_utf16().count() <= 256)
        .map(str::to_owned)
        .ok_or_else(|| format!("{field} is invalid"))
}

fn boolean(value: Option<&LuaValue>, field: &str) -> Result<bool, String> {
    value.and_then(LuaValue::as_bool).ok_or_else(|| format!("{field} is invalid"))
}

fn finite(value: Option<&LuaValue>, field: &str) -> Result<f64, String> {
    value
        .and_then(LuaValue::as_number)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{field} is invalid"))
}

fn nonnegative(value: Option<&LuaValue>, field: &str) -> Result<u64, String> {
    let value = finite(value, field)?;
    if value.fract() != 0.0 || !(0.0..=9_007_199_254_740_991.0).contains(&value) {
        return Err(format!("{field} is invalid"));
    }
    Ok(value as u64)
}

fn positive(value: Option<&LuaValue>, field: &str) -> Result<u64, String> {
    let value = nonnegative(value, field)?;
    if value == 0 {
        return Err(format!("{field} is invalid"));
    }
    Ok(value)
}

fn optional_nonnegative(value: Option<&LuaValue>, field: &str) -> Result<Option<u64>, String> {
    if value == Some(&LuaValue::Null) {
        return Ok(None);
    }
    nonnegative(value, field).map(Some)
}

fn optional_text(value: Option<&LuaValue>, field: &str) -> Result<Option<String>, String> {
    if value == Some(&LuaValue::Null) {
        return Ok(None);
    }
    text(value, field).map(Some)
}

fn texts(
    value: Option<&LuaValue>,
    array_field: &str,
    item_field: &str,
) -> Result<Vec<String>, String> {
    array(value, array_field)?.iter().map(|value| text(Some(value), item_field)).collect()
}

fn scope_kind(row: &LuaObject, field: &str) -> Result<WebLuaScopeKind, String> {
    let value = text(row.get("scope_kind"), field)?;
    WebLuaScopeKind::parse(&value).ok_or_else(|| format!("{field} is invalid"))
}

fn content(
    value: Option<&LuaValue>,
    owner: &str,
    mods: &[CompiledWebLuaMod],
) -> Result<String, String> {
    let id = text(value, "mod content id")?;
    if content_owner(mods, &id) != Some(owner) {
        return Err(format!("mod content is not owned by {owner}"));
    }
    Ok(id)
}
