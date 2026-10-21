use solomon_sim::modding::definition::CompiledWebLuaMod;
use solomon_sim::modding::runtime::PreparedModHostCheckpoint;
use solomon_sim::modding::{LuaObject, LuaValue};

use super::{
    content_owner, integer, integer_array, object, object_value, optional_integer, optional_string,
    string, string_array,
};

pub fn encode_prepared_mod_save_state(
    mods: &[CompiledWebLuaMod],
    checkpoint: &PreparedModHostCheckpoint,
) -> Result<LuaObject, String> {
    if checkpoint.session.graph_sha256.len() != mods.len() {
        return Err("prepared mod checkpoint graph count is invalid".into());
    }
    let mut state = LuaObject::new();
    for (index, module) in mods.iter().enumerate() {
        let owner = &module.identity.id;
        let mut entry = LuaObject::new();
        entry.insert("api".into(), string("1.0.0"));
        entry.insert("graph_sha256".into(), string(&checkpoint.session.graph_sha256[index]));
        entry.insert(
            "powerups".into(),
            LuaValue::Array(
                checkpoint
                    .powerups
                    .instances
                    .iter()
                    .filter(|instance| instance.mod_id == *owner)
                    .map(|instance| {
                        object([
                            ("content_id", string(&instance.content_id)),
                            ("id", integer(instance.id)),
                            ("spawned_tick", integer(instance.spawned_tick)),
                            ("x", LuaValue::Number(instance.x)),
                            ("y", LuaValue::Number(instance.y)),
                        ])
                    })
                    .collect(),
            ),
        );
        entry.insert(
            "spell_cooldowns".into(),
            LuaValue::Array(
                checkpoint
                    .spells
                    .cooldowns
                    .iter()
                    .filter(|cooldown| {
                        content_owner(mods, &cooldown.content_id) == Some(owner.as_str())
                    })
                    .map(|cooldown| {
                        object([
                            ("content_id", string(&cooldown.content_id)),
                            ("player_id", string(&cooldown.player_id)),
                            ("ready_tick", integer(cooldown.ready_tick)),
                        ])
                    })
                    .collect(),
            ),
        );
        entry.insert("spell_effects".into(), LuaValue::Array(checkpoint.spell_effects.effects.iter()
            .filter(|effect| effect.mod_id == *owner)
            .map(|effect| object([
                ("content_id",string(&effect.content_id)),
                ("effects",LuaValue::Array(effect.effects.iter().map(|template| object([
                    ("fields",object_value(&template.fields)),("kind",string(&template.kind)),
                ])).collect())),
                ("expires_tick",integer(effect.expires_tick)),("hit_targets",string_array(&effect.hit_targets)),
                ("id",integer(effect.id)),("interval_ticks",integer(effect.interval_ticks)),
                ("kind", string(match effect.kind {
                    solomon_sim::modding::runtime::ModSpellEffectKind::Area=>"area",
                    solomon_sim::modding::runtime::ModSpellEffectKind::Channel=>"channel",
                    solomon_sim::modding::runtime::ModSpellEffectKind::Projectile=>"projectile",
                })),
                ("last_tick",integer(effect.last_tick)),("next_pulse_tick",integer(effect.next_pulse_tick)),
                ("owner_player_id",string(&effect.owner_player_id)),("radius",LuaValue::Number(effect.radius)),
                ("scope_id",string(&effect.scope.id)),("scope_kind",string(effect.scope.kind.as_str())),
                ("speed_per_tick",LuaValue::Number(effect.speed_per_tick)),("started_tick",integer(effect.started_tick)),
                ("target_x",LuaValue::Number(effect.target_x)),("target_y",LuaValue::Number(effect.target_y)),
                ("x",LuaValue::Number(effect.x)),("y",LuaValue::Number(effect.y)),
            ])).collect()));
        entry.insert(
            "state_values".into(),
            LuaValue::Array(
                checkpoint
                    .semantic_state
                    .values
                    .iter()
                    .filter(|row| row.mod_id == *owner)
                    .map(|row| {
                        object([
                            ("key", string(&row.key)),
                            ("scope_id", string(&row.scope.id)),
                            ("scope_kind", string(row.scope.kind.as_str())),
                            ("value", row.value.clone()),
                        ])
                    })
                    .collect(),
            ),
        );
        entry.insert(
            "state_cells".into(),
            LuaValue::Array(
                checkpoint
                    .session
                    .rules
                    .state
                    .cells
                    .iter()
                    .filter(|cell| cell.mod_id == *owner)
                    .map(|cell| {
                        object([
                            ("key", string(&cell.key)),
                            ("schema_version", integer(cell.schema_version)),
                            ("scope_id", string(&cell.scope.id)),
                            ("scope_kind", string(cell.scope.kind.as_str())),
                            ("value", cell.value.clone()),
                        ])
                    })
                    .collect(),
            ),
        );
        entry.insert(
            "timers".into(),
            LuaValue::Array(
                checkpoint
                    .session
                    .rules
                    .timers
                    .iter()
                    .filter(|timer| timer.mod_id == *owner)
                    .map(|timer| {
                        object([
                            ("context", object_value(&timer.context)),
                            ("due_tick", integer(timer.due_tick)),
                            ("event", string(&timer.event)),
                            ("id", integer(timer.id)),
                            ("interval_ticks", optional_integer(timer.interval_ticks)),
                            ("node_id", string(&timer.node_id)),
                            ("payload", timer.payload.clone()),
                            ("remaining", integer(timer.remaining)),
                            ("scope_id", string(&timer.scope.id)),
                            ("scope_kind", string(timer.scope.kind.as_str())),
                        ])
                    })
                    .collect(),
            ),
        );
        entry.insert(
            "statuses".into(),
            LuaValue::Array(
                checkpoint
                    .statuses
                    .instances
                    .iter()
                    .filter(|status| status.mod_id == *owner)
                    .map(|status| {
                        object([
                            ("content_id", string(&status.content_id)),
                            ("expires_tick", integer(status.expires_tick)),
                            ("instance_id", integer(status.instance_id)),
                            ("started_tick", integer(status.started_tick)),
                            ("target_id", string(&status.target_id)),
                        ])
                    })
                    .collect(),
            ),
        );
        // All shared runtime rows/counters live under the first mod only. The
        // remaining per-mod records preserve their own source insertion order.
        if index == 0 {
            entry.insert("runtime".into(), runtime(checkpoint));
        }
        state.insert(owner.clone(), LuaValue::Object(entry));
    }
    Ok(state)
}

fn runtime(checkpoint: &PreparedModHostCheckpoint) -> LuaValue {
    object([
        (
            "enemies",
            LuaValue::Array(
                checkpoint
                    .enemies
                    .enemies
                    .iter()
                    .map(|enemy| {
                        object([
                            ("content_id", string(&enemy.content_id)),
                            ("current_health", LuaValue::Number(enemy.current_health)),
                            ("death_tick", optional_integer(enemy.death_tick)),
                            ("heading_index", integer(enemy.heading_index)),
                            ("id", integer(enemy.id)),
                            ("last_attack_tick", optional_integer(enemy.last_attack_tick)),
                            (
                                "last_damaged_by_player_id",
                                optional_string(enemy.last_damaged_by_player_id.as_deref()),
                            ),
                            ("life_state", string(enemy.life_state.as_str())),
                            ("maximum_health", LuaValue::Number(enemy.maximum_health)),
                            ("moving", LuaValue::Boolean(enemy.moving)),
                            ("next_attack_tick", integer(enemy.next_attack_tick)),
                            ("spawned_tick", integer(enemy.spawned_tick)),
                            (
                                "target_player_id",
                                optional_string(enemy.target_player_id.as_deref()),
                            ),
                            ("x", LuaValue::Number(enemy.x)),
                            ("y", LuaValue::Number(enemy.y)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("enemy_next_id", integer(checkpoint.enemies.next_id)),
        ("enemy_revision", integer(checkpoint.enemies.revision)),
        ("next_intent_sequence", integer(checkpoint.session.rules.next_intent_sequence)),
        ("next_timer_id", integer(checkpoint.session.rules.next_timer_id)),
        ("powerup_next_id", integer(checkpoint.powerups.next_id)),
        ("powerup_revision", integer(checkpoint.powerups.revision)),
        (
            "reducer_health",
            LuaValue::Array(
                checkpoint
                    .session
                    .rules
                    .reducer_health
                    .iter()
                    .map(|row| {
                        object([
                            ("disabled", LuaValue::Boolean(row.disabled)),
                            ("failures", integer(row.failures)),
                            ("key", string(&row.key)),
                            ("mod_id", string(&row.mod_id)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("scene_next_epoch", integer(checkpoint.scenes.next_epoch)),
        (
            "scenes",
            LuaValue::Array(
                checkpoint
                    .scenes
                    .scenes
                    .iter()
                    .map(|scene| {
                        object([
                            ("epoch", integer(scene.epoch)),
                            ("owner_id", string(&scene.owner_id)),
                            (
                                "parent_content_id",
                                optional_string(scene.parent_content_id.as_deref()),
                            ),
                            ("room_index", integer(scene.room_index)),
                            ("scene_content_id", string(&scene.scene_content_id)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "scene_stacks",
            LuaValue::Array(
                checkpoint
                    .scenes
                    .stacks
                    .iter()
                    .map(|stack| {
                        object([
                            ("owner_id", string(&stack.owner_id)),
                            ("room_indexes", integer_array(&stack.room_indexes)),
                            ("scene_content_ids", string_array(&stack.scene_content_ids)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("semantic_state_revision", integer(checkpoint.semantic_state.revision)),
        (
            "shop_stock",
            LuaValue::Array(
                checkpoint
                    .shops
                    .stock
                    .iter()
                    .map(|stock| {
                        object([
                            ("player_id", string(&stock.player_id)),
                            ("remaining", integer(stock.remaining)),
                            ("restock_tick", optional_integer(stock.restock_tick)),
                            ("row", integer(stock.row)),
                            ("shop_content_id", string(&stock.shop_content_id)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("shop_revision", integer(checkpoint.shops.revision)),
        (
            "skill_ranks",
            LuaValue::Array(
                checkpoint
                    .skills
                    .ranks
                    .iter()
                    .map(|rank| {
                        object([
                            ("content_id", string(&rank.content_id)),
                            ("player_id", string(&rank.player_id)),
                            ("rank", integer(rank.rank)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "skill_offers",
            LuaValue::Array(
                checkpoint
                    .skills
                    .offers
                    .iter()
                    .map(|offer| {
                        object([
                            ("content_ids", string_array(&offer.content_ids)),
                            ("player_id", string(&offer.player_id)),
                            ("sequence", integer(offer.sequence)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "skill_bindings",
            LuaValue::Array(
                checkpoint
                    .skills
                    .bindings
                    .iter()
                    .map(|binding| {
                        object([
                            ("content_id", string(&binding.content_id)),
                            ("player_id", string(&binding.player_id)),
                            ("slot", integer(binding.slot)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("skill_revision", integer(checkpoint.skills.revision)),
        ("spell_revision", integer(checkpoint.spells.revision)),
        ("spell_effect_next_id", integer(checkpoint.spell_effects.next_id)),
        ("spell_effect_next_sequence", integer(checkpoint.spell_effects.next_sequence)),
        ("spell_effect_revision", integer(checkpoint.spell_effects.revision)),
        ("state_revision", integer(checkpoint.session.rules.state.revision)),
        ("status_next_id", integer(checkpoint.statuses.next_instance_id)),
        ("status_revision", integer(checkpoint.statuses.revision)),
    ])
}
