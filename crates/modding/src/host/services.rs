//! Authored skill, shop, scene and UI actions from `prepared-mod-host.ts`.

use solomon_sim::economy::find_inventory_item;
use solomon_sim::modding::content::PreparedModUiDefinition;
use solomon_sim::modding::inventory::{
    ModEquipmentAffix, ModEquipmentModifier, ModModifierOperation, mod_consumable_inventory_item,
    mod_item_inventory_item,
};

use crate::engines::scenes::ModPortalActivation;
use crate::host_intents::grant_item;

use super::*;

pub struct PreparedModUiActionInput<'a> {
    pub action: &'a str,
    pub arguments: LuaObject,
    pub content_id: &'a str,
    pub player_id: &'a str,
    pub request_id: u64,
}

impl PreparedModHost {
    pub fn choose_skill(
        &mut self,
        player_id: &str,
        content_id: &str,
        mod_offer_sequence: u64,
        native_offer_sequence: u64,
        game: &mut GameSession,
    ) -> Result<(), String> {
        self.require_open()?;
        let source = game.clone();
        let checkpoint = self.runtime.skills.checkpoint();
        let chosen = (|| {
            self.runtime.skills.choose(player_id, content_id, mod_offer_sequence)?;
            if !game.replace_skill_with_mod(player_id, native_offer_sequence) {
                return Err("native skill offer is stale".into());
            }
            let progression = &game.participants[player_id].player.progression;
            if let Some(offer) = &progression.pending_offer {
                self.runtime.skills.offer(
                    player_id,
                    u64::from(progression.level),
                    offer.sequence,
                    3,
                )?;
            }
            Ok(())
        })();
        if let Err(error) = chosen {
            self.runtime.skills.restore(checkpoint)?;
            *game = source;
            return Err(error);
        }
        Ok(())
    }

    pub fn purchase_shop(
        &mut self,
        player_id: &str,
        shop_content_id: &str,
        row: usize,
        game: &mut GameSession,
    ) -> Result<(), String> {
        self.require_open()?;
        let source = game.clone();
        let checkpoint = self.runtime.shops.checkpoint();
        let gold =
            game.participants.get(player_id).ok_or("player is unavailable")?.player.economy.gold;
        let purchase =
            self.runtime.shops.purchase(player_id, shop_content_id, row, gold, game.ticks)?;
        let item = if let Some(definition) = self.runtime.content.item(&purchase.item_content_id) {
            mod_item_inventory_item(&definition.catalog, purchase.quantity)?
        } else if let Some(definition) = self.runtime.content.potion(&purchase.item_content_id) {
            let mut item = mod_consumable_inventory_item(&definition.catalog);
            item.quantity = purchase.quantity;
            item
        } else {
            self.runtime.shops.restore(checkpoint)?;
            return Err("mod shop item is unavailable".into());
        };
        let mut candidate = source.clone();
        let economy = &mut candidate.participants.get_mut(player_id).unwrap().player.economy;
        economy.gold -= purchase.price;
        economy.revision += 1;
        if !grant_item(&mut candidate, player_id, item) {
            self.runtime.shops.restore(checkpoint)?;
            return Err("inventory cannot accept the mod shop purchase".into());
        }
        *game = candidate;
        // In Source only an explicitly rejected rule result rolls the purchase
        // back. A fatal VM/dispatch error propagates after this committed grant.
        let result = self.act_raw(
            &PreparedModActionInput {
                action: "shop.purchase".into(),
                context: object([
                    ("participant_id", LuaValue::String(player_id.into())),
                    ("shop_content_id", LuaValue::String(shop_content_id.into())),
                ]),
                payload: object_value([
                    ("item_content_id", LuaValue::String(purchase.item_content_id)),
                    ("participant_id", LuaValue::String(player_id.into())),
                    ("price", LuaValue::Number(purchase.price as f64)),
                    ("quantity", LuaValue::Number(f64::from(purchase.quantity))),
                    ("row", LuaValue::Number(purchase.row as f64)),
                    ("shop_content_id", LuaValue::String(shop_content_id.into())),
                ]),
                request_id: self.runtime.shops.revision(),
                scope: ModStateScope {
                    id: player_id.into(),
                    kind: WebLuaScopeKind::ParticipantProfile,
                },
                tick: source.ticks,
            },
            game,
        )?;
        if !result.accepted {
            self.runtime.shops.restore(checkpoint)?;
            *game = source;
            return Err(rule_failure(&result, "mod shop purchase rule failed"));
        }
        Ok(())
    }

    pub fn reforge_shop(
        &mut self,
        player_id: &str,
        shop_content_id: &str,
        service_index: usize,
        item_id: u64,
        game: &mut GameSession,
    ) -> Result<(), String> {
        self.require_open()?;
        let economy =
            &game.participants.get(player_id).ok_or("player is unavailable")?.player.economy;
        let service = self
            .runtime
            .content
            .shop(shop_content_id)
            .and_then(|shop| shop.services.get(service_index));
        let item = find_inventory_item(&economy.backpack, item_id);
        let Some((service, equipment_type)) = service
            .zip(item.and_then(|item| item.equipment_type))
            .filter(|(service, _)| economy.gold >= service.price)
        else {
            return Err("mod reforge service is unavailable".into());
        };
        let pool_id = service
            .pool
            .content_id
            .parse::<u64>()
            .expect("compiled pool content identity is numeric");
        let seed = game.ticks as u32 ^ item_id as u32 ^ pool_id as u32;
        let rolls = self.runtime.affixes.roll(
            &service.pool.content_id,
            equipment_type.as_str(),
            u64::from(seed),
        )?;
        let mut affixes = Vec::new();
        for roll in rolls {
            let definition = self
                .runtime
                .content
                .affix(&roll.content_id)
                .expect("accepted pool rolls reference an accepted affix");
            let mut modifiers = Vec::new();
            for (key, value) in roll.modifiers.iter() {
                if let Some(amount) = value.as_number().filter(|number| number.is_finite()) {
                    modifiers.push(ModEquipmentModifier {
                        key: key.into(),
                        operation: ModModifierOperation::Multiply,
                        value: amount,
                    });
                } else if let Some(operations) = value.as_object() {
                    for (operation, value) in operations.iter() {
                        let operation = match operation {
                            "add" => ModModifierOperation::Add,
                            "multiply" => ModModifierOperation::Multiply,
                            "set" => ModModifierOperation::Set,
                            _ => continue,
                        };
                        if let Some(amount) = value.as_number().filter(|number| number.is_finite())
                        {
                            modifiers.push(ModEquipmentModifier {
                                key: key.into(),
                                operation,
                                value: amount,
                            });
                        }
                    }
                }
            }
            affixes.push(ModEquipmentAffix {
                content_id: roll.content_id,
                mod_id: definition.common.mod_id.clone(),
                modifiers,
                name: roll.name,
            });
        }
        if affixes.is_empty() {
            return Err("mod reforge produced no applicable affix".into());
        }
        let price = service.price;
        let mut economy = economy.clone();
        if !economy.reforge_mod_equipment(item_id, &affixes).accepted {
            return Err("mod reforge item is invalid".into());
        }
        economy.gold -= price;
        economy.revision += 1;
        game.participants.get_mut(player_id).unwrap().player.economy = economy;
        Ok(())
    }

    pub fn return_scene(&mut self, owner_id: &str) -> Result<Option<ActiveModScene>, String> {
        self.require_open()?;
        Ok(self.runtime.scenes.return_scene(owner_id))
    }

    pub fn select_scene_room(
        &mut self,
        owner_id: &str,
        room_index: usize,
        game: &mut GameSession,
    ) -> Result<ActiveModScene, String> {
        self.require_open()?;
        let checkpoint = self.runtime.scenes.checkpoint();
        let active = self.runtime.scenes.select_room(owner_id, room_index)?;
        let result = self.act_raw(
            &PreparedModActionInput {
                action: "scene.room".into(),
                context: object([
                    ("scene_content_id", LuaValue::String(active.scene_content_id.clone())),
                    ("scene_owner_id", LuaValue::String(owner_id.into())),
                ]),
                payload: object_value([
                    ("room_index", LuaValue::Number(active.room_index as f64)),
                    ("scene_content_id", LuaValue::String(active.scene_content_id.clone())),
                ]),
                request_id: active.epoch,
                scope: ModStateScope { id: owner_id.into(), kind: WebLuaScopeKind::Scene },
                tick: game.ticks,
            },
            game,
        )?;
        if !result.accepted {
            self.runtime.scenes.restore(checkpoint)?;
            return Err(rule_failure(&result, "mod scene room rule failed"));
        }
        Ok(active)
    }

    pub fn enter_portal(
        &mut self,
        input: ModPortalActivation<'_>,
        player_id: &str,
        game: &mut GameSession,
    ) -> Result<(), String> {
        self.require_open()?;
        let checkpoint = self.runtime.scenes.checkpoint();
        let owner = input.owner_id;
        let portal = input.portal_id;
        let active = self.runtime.portals.activate(&mut self.runtime.scenes, input)?;
        let result = self.act_raw(
            &PreparedModActionInput {
                action: "portal.enter".into(),
                context: object([
                    ("participant_id", LuaValue::String(player_id.into())),
                    ("portal_id", LuaValue::String(portal.into())),
                    ("scene_owner_id", LuaValue::String(owner.into())),
                ]),
                payload: object_value([
                    ("portal_id", LuaValue::String(portal.into())),
                    ("scene_content_id", LuaValue::String(active.scene_content_id)),
                ]),
                request_id: active.epoch,
                scope: ModStateScope { id: owner.into(), kind: WebLuaScopeKind::PartyRun },
                tick: game.ticks,
            },
            game,
        )?;
        if !result.accepted {
            self.runtime.scenes.restore(checkpoint)?;
            return Err(rule_failure(&result, "mod portal rule failed"));
        }
        Ok(())
    }

    pub fn ui_action(
        &mut self,
        input: PreparedModUiActionInput<'_>,
        game: &mut GameSession,
    ) -> Result<PreparedModStepResult, String> {
        self.require_open()?;
        let available = self.runtime.content.ui(input.content_id).is_some_and(|definition| {
            definition.actions.iter().any(|action| action == input.action)
                && self.ui_visible(definition, Some(input.player_id), game)
        });
        if !available {
            return Err("mod UI action is unavailable".into());
        }
        self.act_raw(
            &PreparedModActionInput {
                action: "ui.action".into(),
                context: object([
                    ("action", LuaValue::String(input.action.into())),
                    ("participant_id", LuaValue::String(input.player_id.into())),
                    ("ui_content_id", LuaValue::String(input.content_id.into())),
                ]),
                payload: object_value([
                    ("action", LuaValue::String(input.action.into())),
                    ("arguments", LuaValue::Object(input.arguments)),
                    ("content_id", LuaValue::String(input.content_id.into())),
                    ("participant_id", LuaValue::String(input.player_id.into())),
                ]),
                request_id: input.request_id,
                scope: participant_scope(game, input.player_id),
                tick: game.ticks,
            },
            game,
        )
    }

    pub(super) fn ui_visible(
        &self,
        definition: &PreparedModUiDefinition,
        viewer_id: Option<&str>,
        game: &GameSession,
    ) -> bool {
        let owner = game.run.run_id.as_deref().or(viewer_id);
        let in_room = self
            .runtime
            .scenes
            .project()
            .iter()
            .any(|scene| Some(scene.owner_id.as_str()) == owner);
        let scene = if in_room {
            "room"
        } else if game.boneyard.is_some() {
            "boneyard"
        } else {
            "hub"
        };
        if let Some(LuaValue::Array(allowed)) = definition.visible.get("scenes")
            && !allowed.is_empty()
            && !allowed.iter().any(|value| value.as_str() == Some(scene))
        {
            return false;
        }
        let Some(condition) = definition.visible.get("state").and_then(LuaValue::as_object) else {
            return true;
        };
        let key = condition
            .get("key")
            .and_then(LuaValue::as_str)
            .or_else(|| condition.get("state").and_then(LuaValue::as_str));
        let Some(key) = key.filter(|key| !key.is_empty()) else {
            return true;
        };
        let values = self.runtime.semantic_state.project(viewer_id);
        let value = values
            .iter()
            .find(|row| row.mod_id == definition.common.mod_id && row.key == key)
            .map(|row| &row.value);
        match condition.get("equals") {
            None => match value {
                None | Some(LuaValue::Null) | Some(LuaValue::Boolean(false)) => false,
                Some(LuaValue::Number(value)) => *value != 0.0 && !value.is_nan(),
                Some(LuaValue::String(value)) => !value.is_empty(),
                _ => true,
            },
            Some(
                expected @ (LuaValue::Null
                | LuaValue::Boolean(_)
                | LuaValue::Number(_)
                | LuaValue::String(_)),
            ) => value == Some(expected),
            Some(_) => false,
        }
    }

    pub(super) fn ui_bindings(
        &self,
        definition: &PreparedModUiDefinition,
        viewer_id: Option<&str>,
    ) -> LuaObject {
        let values = self.runtime.semantic_state.project(viewer_id);
        let mut fields = LuaObject::new();
        for (name, binding) in definition.bindings.iter() {
            let key = binding
                .as_object()
                .and_then(|binding| binding.get("state"))
                .and_then(LuaValue::as_str);
            let value = key.and_then(|key| {
                values.iter().find(|row| row.mod_id == definition.common.mod_id && row.key == key)
            });
            fields.insert(name.into(), value.map_or(LuaValue::Null, |row| row.value.clone()));
        }
        fields
    }
}

fn rule_failure(result: &PreparedModStepResult, fallback: &str) -> String {
    if result.errors.is_empty() { fallback.into() } else { result.errors.join("; ") }
}
