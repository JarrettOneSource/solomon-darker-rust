//! Game mutations from `prepared-mod-host.ts::applyIntent` and its target helpers.
//! The caller prepares these against one canonical Session candidate, then
//! commits the complete batch. This module never owns or steps another world.

use solomon_sim::Vec2;
use solomon_sim::boneyard::spawn::SpawnPlacement;
use solomon_sim::economy::HubInventoryItem;
use solomon_sim::enemies::model::EnemyDamageRequest;
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::modding::definition::WebLuaContentKind;
use solomon_sim::modding::extensions::{
    GameSimulationDamageFilterInput, GameSimulationDamageKind, GameSimulationExtensions,
};
use solomon_sim::modding::inventory::{mod_consumable_inventory_item, mod_item_inventory_item};
use solomon_sim::modding::runtime::{
    ModEnemyLifeState, ModIntent, PreparedModPresentationIntent, PreparedModPresentationKind,
};
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::session::GameSession;

use crate::engines::effects::ModSpellEffectSpawn;
use crate::host_runtime::PreparedModRuntime;
use crate::intents::ModIntentExecutionContext;

#[derive(Default)]
pub struct AppliedModGameIntent {
    pub spawn: Option<EnemySpawnIntent>,
    pub presentation: Option<PreparedModPresentationIntent>,
}

pub fn apply_game_intent(
    game: &mut GameSession,
    runtime: &mut PreparedModRuntime,
    intent: &ModIntent,
    context: &ModIntentExecutionContext,
) -> Result<AppliedModGameIntent, String> {
    let fields = &intent.fields;
    let mut output = AppliedModGameIntent::default();
    match intent.kind.as_str() {
        "damage" => {
            exact_fields(intent, &["amount", "modifier", "target"], "damage intent")?;
            apply_damage(game, runtime, fields, context)?;
        }
        "resource" => {
            exact_fields(
                intent,
                &["experience", "gold", "health", "mana", "target"],
                "resource intent",
            )?;
            let id = target_player(game, fields.get("target"), context)?.to_owned();
            let player = &mut game.participants.get_mut(&id).unwrap().player;
            if let Some(value) = fields.get("health") {
                let amount = if value.as_str() == Some("full") {
                    player.combat.maximum_health - player.combat.current_health
                } else {
                    nonnegative(value, "resource health")?
                };
                player.combat.restore_health(amount);
            }
            if let Some(value) = fields.get("mana") {
                if value.as_str() == Some("full") {
                    player.combat.set_mana(player.combat.maximum_mana);
                } else {
                    player.combat.restore_mana(nonnegative(value, "resource mana")?);
                }
            }
            if let Some(value) = fields.get("gold") {
                let amount = integer(value, 0, 10_000_000, "resource gold")?;
                let gold = (player.economy.gold + amount).min(10_000_000);
                player.economy.revision += u64::from(gold != player.economy.gold);
                player.economy.gold = gold;
            }
            if let Some(value) = fields.get("experience") {
                game.grant_experience_with_extensions(
                    &id,
                    nonnegative(value, "resource experience")?,
                    Some(runtime),
                );
            }
        }
        "grant" => {
            exact_fields(intent, &["item", "quantity", "target"], "grant intent")?;
            let id = target_player(game, fields.get("target"), context)?.to_owned();
            let reference = resolved_reference(fields.get("item"), "grant item")?;
            let quantity = fields
                .get("quantity")
                .map(|value| integer(value, 1, 99, "grant quantity"))
                .transpose()?
                .unwrap_or(1) as u32;
            let (item, owner, key) = match reference.target_kind {
                "potion" => {
                    let definition =
                        runtime.content.potion(reference.content_id).ok_or_else(|| {
                            format!("grant item is unavailable: {}", reference.content_id)
                        })?;
                    let mut item = mod_consumable_inventory_item(&definition.catalog);
                    item.quantity = quantity;
                    (item, &definition.common.mod_id, &definition.common.key)
                }
                "item" => {
                    let definition =
                        runtime.content.item(reference.content_id).ok_or_else(|| {
                            format!("grant item is unavailable: {}", reference.content_id)
                        })?;
                    (
                        mod_item_inventory_item(&definition.catalog, quantity)?,
                        &definition.common.mod_id,
                        &definition.common.key,
                    )
                }
                _ => return Err(format!("grant item is unavailable: {}", reference.content_id)),
            };
            if !grant_item(game, &id, item) {
                return Err(format!("inventory cannot accept {owner}:{key}"));
            }
        }
        "status" => {
            exact_fields(intent, &["status", "target"], "status intent")?;
            let id = target_player(game, fields.get("target"), context)?.to_owned();
            let reference = content_reference(
                fields.get("status"),
                WebLuaContentKind::Status,
                &format!("{}:{} status", intent.mod_id, intent.owner),
            )?;
            runtime.statuses.apply(reference.content_id, &id, context.tick, None)?;
        }
        "spawn" => {
            exact_fields(intent, &["content", "enemy", "token", "x", "y"], "spawn intent")?;
            if let Some(value) = fields.get("content") {
                let reference = resolved_reference(Some(value), "spawn content")?;
                if reference.target_kind != "powerup" {
                    return Err("spawn content must reference a powerup".into());
                }
                let position = intent_position(game, fields, context)?;
                runtime.powerups.spawn(reference.content_id, position, context.tick)?;
            } else if fields
                .get("enemy")
                .is_some_and(|value| matches!(value, LuaValue::Object(_) | LuaValue::Array(_)))
            {
                let reference = resolved_reference(fields.get("enemy"), "spawn enemy")?;
                if reference.target_kind != "enemy" {
                    return Err("spawn enemy must reference an enemy".into());
                }
                let world = game
                    .boneyard
                    .as_ref()
                    .ok_or("custom enemy spawn requires an active Boneyard")?;
                let position = intent_position(game, fields, context)?;
                let radius = runtime.enemies.collision_radius(reference.content_id)?;
                let position = resolve_spawn(world, position, radius)?;
                runtime.enemies.spawn(reference.content_id, position, context.tick)?;
            } else {
                let world =
                    game.boneyard.as_ref().ok_or("enemy spawn requires an active Boneyard")?;
                let value = fields
                    .get("token")
                    .filter(|value| !matches!(value, LuaValue::Null))
                    .or_else(|| fields.get("enemy"));
                let token = value
                    .and_then(LuaValue::as_str)
                    .and_then(EnemyToken::from_id)
                    .filter(|token| *token != EnemyToken::Portal)
                    .ok_or("enemy spawn requires a stock token or resolved enemy reference")?;
                let position = intent_position(game, fields, context)?;
                output.spawn = Some(EnemySpawnIntent::new(
                    token,
                    position,
                    0x5000_0000 + (intent.sequence % 0x0fff_ffff) as usize,
                    game.ticks + 1,
                    world.waves.as_ref().map_or(0, |waves| waves.state.wave_ordinal),
                ));
            }
        }
        "spell-effect" => {
            let id =
                target_player(game, Some(&LuaValue::String("caster".into())), context)?.to_owned();
            let origin = game.participants[&id].wizard.position;
            let content_id = text(fields.get("spell_content_id"), "spell effect content id")?;
            if runtime.content.spell(content_id).is_none() {
                return Err("spell effect content is unavailable".into());
            }
            runtime.spell_effects.spawn(ModSpellEffectSpawn {
                content_id,
                fields,
                mod_id: &intent.mod_id,
                origin,
                owner_player_id: &id,
                scope: intent.scope.clone(),
                target: Vec2::new(
                    finite(context.context.get("target_x"), origin.x, "spell target x")?,
                    finite(context.context.get("target_y"), origin.y, "spell target y")?,
                ),
                tick: context.tick,
            })?;
        }
        "state" => {
            exact_fields(intent, &["clear", "key", "value"], "state intent")?;
            let key = text(fields.get("key"), "state key")?;
            let clear = fields.get("clear") == Some(&LuaValue::Boolean(true));
            if clear == fields.get("value").is_some() {
                return Err("state intent requires exactly one of value or clear".into());
            }
            if clear {
                runtime.semantic_state.clear(&intent.mod_id, &intent.scope, key);
            } else {
                runtime.semantic_state.set(
                    &intent.mod_id,
                    intent.scope.clone(),
                    key,
                    fields.get("value").unwrap().clone(),
                )?;
            }
        }
        "present" => {
            exact_fields(intent, &["sound"], "present intent")?;
            if fields.get("sound").is_none() {
                return Err("present intent requires sound".into());
            }
            output.presentation = Some(PreparedModPresentationIntent {
                fields: fields.clone(),
                kind: PreparedModPresentationKind::Present,
                mod_id: intent.mod_id.clone(),
                sequence: intent.sequence,
                tick: context.tick,
            });
        }
        _ => return Err(format!("unsupported Web Lua intent: {}", intent.kind)),
    }
    Ok(output)
}

fn apply_damage(
    game: &mut GameSession,
    runtime: &mut PreparedModRuntime,
    fields: &LuaObject,
    context: &ModIntentExecutionContext,
) -> Result<(), String> {
    let amount = fields
        .get("amount")
        .and_then(LuaValue::as_number)
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1_000_000_000.0)
        .ok_or("damage amount must be finite within 0..1000000000")?;
    if let Some(id) = damage_player_target(game, fields.get("target"), context) {
        let player = &game.participants[&id].player;
        let filtered = runtime.filter_damage(
            GameSimulationDamageFilterInput {
                amount,
                damage_kind: GameSimulationDamageKind::Physical,
                source_actor_id: None,
                target_player_id: &id,
                tick: context.tick,
            },
            &player.economy.equipment,
        )?;
        game.damage_player(&id, filtered, context.tick);
        return Ok(());
    }
    let (id, custom) = damage_enemy_target(fields.get("target"), context)?;
    let source_player_id = context
        .context
        .get("participant_id")
        .and_then(LuaValue::as_str)
        .filter(|id| game.participants.contains_key(*id));
    let modifier = fields
        .get("modifier")
        .and_then(LuaValue::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("outgoing_damage");
    let amount = if let Some(id) = source_player_id {
        runtime.skills.filter(id, modifier, amount)?
    } else {
        amount
    };
    if custom {
        let damaged = runtime
            .enemies
            .damage(id, amount, context.tick, source_player_id)?
            .ok_or("mod damage target is unavailable")?;
        if damaged.life_state != ModEnemyLifeState::Dying || source_player_id.is_none() {
            return Ok(());
        }
        let definition = runtime.content.enemy(&damaged.content_id).unwrap();
        let span = definition.gold_maximum - definition.gold_minimum + 1;
        let gold = definition.gold_minimum + (damaged.id as u32).wrapping_mul(0x9e37_79b1) % span;
        let id = source_player_id.unwrap().to_owned();
        let player = &mut game.participants.get_mut(&id).unwrap().player;
        if gold == 0 {
            return Err("loot Gold credit must be a positive safe integer".into());
        }
        player.economy.credit_loot_gold(u64::from(gold));
        if definition.experience != 0.0 {
            game.grant_experience_with_extensions(&id, definition.experience, Some(runtime));
        }
    } else {
        let source_player_id = source_player_id.map(str::to_owned);
        let world = game.boneyard.as_mut().ok_or("enemy damage requires an active Boneyard")?;
        if !amount.is_finite() || amount <= 0.0 {
            return Err("enemy damage must be finite and positive".into());
        }
        let result = world.enemies.damage(
            EnemyDamageRequest {
                enhanced_effects: Some(game.enhanced_effects),
                actor_id: usize::try_from(id).map_err(|_| "enemy damage target is unavailable")?,
                amount,
                ether_drain_capture: false,
                magic: false,
                has_magic_damage: false,
                hit_strength: 1.0,
                source_player_id,
                suppress_hit_reaction: false,
                suppress_hurt_sound: false,
                tick: context.tick,
            },
            Some(&mut game.painter_order),
        );
        if !result.accepted {
            return Err("enemy damage target is unavailable".into());
        }
    }
    Ok(())
}

pub fn grant_item(game: &mut GameSession, id: &str, item: HubInventoryItem) -> bool {
    let economy =
        &mut game.participants.get_mut(id).expect("resolved inventory participant").player.economy;
    economy.grant_inventory_items(&[item])
}

pub fn resolve_spawn(
    world: &solomon_sim::boneyard::BoneyardWorld,
    position: Vec2,
    radius: f64,
) -> Result<Vec2, String> {
    let mut collision = world.collision.clone();
    collision.set_gate_leaves(&world.gate_leaves);
    SpawnPlacement { position, bounds: world.loaded.scene.bounds, collision: &collision, radius }
        .resolve(None)
        .map_err(|_| {
            format!(
                "Boneyard has no collision-safe spawn placement for radius {radius} from ({}, {})",
                position.x, position.y
            )
        })
}

pub fn intent_position(
    game: &GameSession,
    fields: &LuaObject,
    context: &ModIntentExecutionContext,
) -> Result<Vec2, String> {
    let player = context
        .context
        .get("participant_id")
        .and_then(LuaValue::as_str)
        .and_then(|id| game.participants.get(id));
    let origin = player.map_or(Vec2::ZERO, |player| player.wizard.position);
    Ok(Vec2::new(
        finite(fields.get("x"), origin.x, "spawn x")?,
        finite(fields.get("y"), origin.y, "spawn y")?,
    ))
}

pub fn target_player<'a>(
    game: &'a GameSession,
    value: Option<&'a LuaValue>,
    context: &'a ModIntentExecutionContext,
) -> Result<&'a str, String> {
    let candidate = match value {
        None => context.context.get("participant_id").and_then(LuaValue::as_str),
        Some(LuaValue::String(value))
            if matches!(value.as_str(), "user" | "collector" | "caster") =>
        {
            context.context.get("participant_id").and_then(LuaValue::as_str)
        }
        Some(LuaValue::String(value)) => Some(value.as_str()),
        Some(LuaValue::Object(fields)) => fields
            .get("participant_id")
            .filter(|value| !matches!(value, LuaValue::Null))
            .or_else(|| fields.get("id"))
            .and_then(LuaValue::as_str),
        _ => None,
    };
    candidate
        .filter(|id| game.participants.contains_key(*id))
        .ok_or_else(|| "mod intent target player is unavailable".into())
}

fn damage_player_target(
    game: &GameSession,
    value: Option<&LuaValue>,
    context: &ModIntentExecutionContext,
) -> Option<String> {
    let candidate = match value {
        None => context.context.get("participant_id").and_then(LuaValue::as_str),
        Some(LuaValue::String(value))
            if matches!(value.as_str(), "user" | "collector" | "caster") =>
        {
            context.context.get("participant_id").and_then(LuaValue::as_str)
        }
        Some(LuaValue::String(value)) if value == "target_player" => {
            context.context.get("target_player_id").and_then(LuaValue::as_str)
        }
        Some(LuaValue::String(value)) => Some(value.as_str()),
        Some(LuaValue::Object(fields)) => fields.get("participant_id").and_then(LuaValue::as_str),
        _ => None,
    };
    candidate.filter(|id| game.participants.contains_key(*id)).map(str::to_owned)
}

fn damage_enemy_target(
    value: Option<&LuaValue>,
    context: &ModIntentExecutionContext,
) -> Result<(u64, bool), String> {
    let fields = value.and_then(LuaValue::as_object);
    let name = value.and_then(LuaValue::as_str);
    let contextual_custom = context.context.get("mod_enemy_id").and_then(LuaValue::as_number);
    let explicit_custom = name == Some("mod_enemy");
    let target_custom = name == Some("target_enemy") && contextual_custom.is_some();
    let number = value
        .and_then(LuaValue::as_number)
        .or_else(|| fields.and_then(|fields| fields.get("id")).and_then(LuaValue::as_number))
        .or_else(|| {
            if explicit_custom || target_custom {
                contextual_custom
            } else {
                context
                    .context
                    .get("enemy_id")
                    .filter(|value| !matches!(value, LuaValue::Null))
                    .or_else(|| context.context.get("actor_id"))
                    .and_then(LuaValue::as_number)
            }
        });
    let id = number
        .filter(|value| {
            value.is_finite()
                && value.fract() == 0.0
                && (1.0..=9_007_199_254_740_991.0).contains(value)
        })
        .ok_or("mod damage target is unavailable")? as u64;
    let custom = fields.and_then(|fields| fields.get("kind")).and_then(LuaValue::as_str)
        == Some("mod-enemy")
        || explicit_custom
        || target_custom;
    Ok((id, custom))
}

/// A borrowed validation view over an intent field. Source only requires these
/// two strings here; creation-site/key metadata belongs to the compiled catalog.
pub struct IntentContentReference<'a> {
    pub content_id: &'a str,
    pub target_kind: &'a str,
}

pub fn resolved_reference<'a>(
    value: Option<&'a LuaValue>,
    field: &str,
) -> Result<IntentContentReference<'a>, String> {
    let invalid = || format!("{field} must be a resolved content reference");
    let fields = value.and_then(LuaValue::as_object).ok_or_else(invalid)?;
    if fields.get("kind").and_then(LuaValue::as_str) != Some("resolved-content-reference") {
        return Err(invalid());
    }
    let target_kind = fields.get("targetKind").and_then(LuaValue::as_str).ok_or_else(invalid)?;
    let content_id = fields.get("contentId").and_then(LuaValue::as_str).ok_or_else(invalid)?;
    Ok(IntentContentReference { content_id, target_kind })
}

fn content_reference<'a>(
    value: Option<&'a LuaValue>,
    kind: WebLuaContentKind,
    field: &str,
) -> Result<IntentContentReference<'a>, String> {
    let reference = resolved_reference(value, field)
        .map_err(|_| format!("{field} must be a resolved {} reference", kind.as_str()))?;
    if reference.target_kind != kind.as_str() {
        return Err(format!("{field} must be a resolved {} reference", kind.as_str()));
    }
    Ok(reference)
}
fn exact_fields(intent: &ModIntent, fields: &[&str], label: &str) -> Result<(), String> {
    let extra = intent.owner.starts_with("spell-effect.");
    let unknown: Vec<_> = intent
        .fields
        .iter()
        .map(|(key, _)| key)
        .filter(|key| !fields.contains(key) && !(extra && ["effect_id", "x", "y"].contains(key)))
        .collect();
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(format!("{label} contains unknown fields: {}", unknown.join(", ")))
    }
}
pub fn text<'a>(value: Option<&'a LuaValue>, field: &str) -> Result<&'a str, String> {
    value
        .and_then(LuaValue::as_str)
        .filter(|value| (1..=128).contains(&value.encode_utf16().count()))
        .ok_or_else(|| format!("{field} must be 1..128 text characters"))
}
pub fn finite(value: Option<&LuaValue>, fallback: f64, field: &str) -> Result<f64, String> {
    match value {
        None => Ok(fallback),
        Some(value) => value
            .as_number()
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("{field} must be finite")),
    }
}
pub fn nonnegative(value: &LuaValue, field: &str) -> Result<f64, String> {
    value
        .as_number()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or_else(|| format!("{field} must be a nonnegative finite number"))
}
pub fn integer(value: &LuaValue, minimum: u64, maximum: u64, field: &str) -> Result<u64, String> {
    value
        .as_number()
        .filter(|value| {
            value.is_finite()
                && value.fract() == 0.0
                && *value >= minimum as f64
                && *value <= maximum as f64
        })
        .map(|value| value as u64)
        .ok_or_else(|| format!("{field} must be an integer within {minimum}..{maximum}"))
}
