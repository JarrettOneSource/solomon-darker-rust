//! The owned content engines, restore transaction and gameplay filters from
//! `host/prepared-mod-host.ts`. The live Lua session is borrowed only while its
//! checkpoint is read/restored; actual stock players stay in `GameSession`.

use std::sync::Arc;

use solomon_sim::economy::{BoastSelection, HubEquipmentState, HubInventoryItem};
use solomon_sim::enemies::EnemyToken;
use solomon_sim::hub::interaction::BoastRules;
use solomon_sim::modding::content::PreparedModContentCatalog;
use solomon_sim::modding::definition::WebLuaContentKind;
use solomon_sim::modding::extensions::*;
use solomon_sim::modding::inventory::ModModifierOperation;
use solomon_sim::modding::runtime::PreparedModHostCheckpoint;

use crate::assets::{ModAssetSource, PreparedModAssetCatalog, compile_mod_assets};
use crate::content::compile_mod_content_catalog;
use crate::engines::affix::ModAffixEngine;
use crate::engines::effects::ModSpellEffectEngine;
use crate::engines::enemies::ModEnemyEngine;
use crate::engines::powerups::ModPowerupEngine;
use crate::engines::scenes::{ModPortalEngine, ModSceneEngine};
use crate::engines::semantic::ModSemanticStateEngine;
use crate::engines::shops::ModShopEngine;
use crate::engines::skills::ModSkillEngine;
use crate::engines::spells::ModSpellEngine;
use crate::engines::statuses::ModStatusEngine;
use crate::package::MaterializedWebSessionContent;
use crate::session::PreparedModSession;

/// One mutable collection supports both live host transactions and borrowed
/// Session extension calls. No engine is reconstructed for a filter or frame.
pub struct PreparedModRuntime {
    pub assets: PreparedModAssetCatalog,
    pub content: Arc<PreparedModContentCatalog>,
    pub affixes: ModAffixEngine,
    pub enemies: ModEnemyEngine,
    pub portals: ModPortalEngine,
    pub powerups: ModPowerupEngine,
    pub scenes: ModSceneEngine,
    pub semantic_state: ModSemanticStateEngine,
    pub shops: ModShopEngine,
    pub skills: ModSkillEngine,
    pub spells: ModSpellEngine,
    pub spell_effects: ModSpellEffectEngine,
    pub statuses: ModStatusEngine,
}

impl PreparedModRuntime {
    pub fn prepare(package: &MaterializedWebSessionContent) -> Result<Self, String> {
        if package.mod_sources.len() != package.compiled_mods.len() {
            return Err("admitted Web Lua source and compiled graph counts disagree".into());
        }
        let sources: Vec<_> = package
            .mod_sources
            .iter()
            .map(|source| ModAssetSource {
                files: source.files.clone(),
                identity: source.identity.clone(),
            })
            .collect();
        let assets = compile_mod_assets(&package.assets, &package.compiled_mods, &sources)?;
        let content = Arc::new(compile_mod_content_catalog(&package.compiled_mods, &assets)?);
        for definition in content.all() {
            if definition.kind() != WebLuaContentKind::Boneyard {
                continue;
            }
            let definition = definition.common();
            let source = definition.fields.get("source").and_then(|value| value.as_str());
            if source.is_none_or(|path| {
                !package.assets.iter().any(|asset| {
                    asset.mod_id == definition.mod_id
                        && asset.path == path
                        && asset.kind == "boneyard"
                })
            }) {
                return Err(format!(
                    "{}:{} Boneyard source is not packaged",
                    definition.mod_id, definition.key
                ));
            }
        }
        Ok(Self {
            affixes: ModAffixEngine::new(Arc::clone(&content)),
            enemies: ModEnemyEngine::new(Arc::clone(&content), 100)?,
            portals: ModPortalEngine::new(&content)?,
            powerups: ModPowerupEngine::new(Arc::clone(&content)),
            scenes: ModSceneEngine::new(Arc::clone(&content)),
            semantic_state: ModSemanticStateEngine::new(
                package.compiled_mods.iter().map(|package| package.identity.id.clone()),
            ),
            shops: ModShopEngine::new(Arc::clone(&content), 100)?,
            skills: ModSkillEngine::new(Arc::clone(&content)),
            spells: ModSpellEngine::new(Arc::clone(&content), 100)?,
            spell_effects: ModSpellEffectEngine::new(100)?,
            statuses: ModStatusEngine::new(Arc::clone(&content), 100)?,
            assets,
            content,
        })
    }

    pub fn checkpoint(
        &self,
        session: &PreparedModSession,
    ) -> Result<PreparedModHostCheckpoint, String> {
        Ok(PreparedModHostCheckpoint {
            enemies: self.enemies.checkpoint(),
            powerups: self.powerups.checkpoint(),
            scenes: self.scenes.checkpoint(),
            semantic_state: self.semantic_state.checkpoint(),
            shops: self.shops.checkpoint(),
            skills: self.skills.checkpoint(),
            spells: self.spells.checkpoint(),
            spell_effects: self.spell_effects.checkpoint(),
            session: session.checkpoint()?,
            statuses: self.statuses.checkpoint(),
        })
    }

    pub fn restore(
        &mut self,
        session: &mut PreparedModSession,
        checkpoint: &PreparedModHostCheckpoint,
    ) -> Result<(), String> {
        let previous = self.checkpoint(session)?;
        if let Err(error) = self.restore_engines(session, checkpoint) {
            self.restore_engines(session, &previous)?;
            return Err(error);
        }
        Ok(())
    }

    fn restore_engines(
        &mut self,
        session: &mut PreparedModSession,
        checkpoint: &PreparedModHostCheckpoint,
    ) -> Result<(), String> {
        if checkpoint
            .spell_effects
            .effects
            .iter()
            .any(|effect| self.content.spell(&effect.content_id).is_none())
        {
            return Err("mod spell effect checkpoint references unavailable content".into());
        }
        self.enemies.restore(checkpoint.enemies.clone())?;
        self.powerups.restore(checkpoint.powerups.clone())?;
        self.scenes.restore(checkpoint.scenes.clone())?;
        self.semantic_state.restore(checkpoint.semantic_state.clone())?;
        self.shops.restore(checkpoint.shops.clone())?;
        self.skills.restore(checkpoint.skills.clone())?;
        self.spells.restore(checkpoint.spells.clone())?;
        self.spell_effects.restore(checkpoint.spell_effects.clone())?;
        self.statuses.restore(checkpoint.statuses.clone())?;
        session.restore(&checkpoint.session)
    }
}

impl GameSimulationExtensions for PreparedModRuntime {
    fn create_loot_items(&self, input: GameSimulationLootInput) -> Vec<HubInventoryItem> {
        // Source enemy loot supplies its native semantic word, at most 30 bits.
        // This cannot fail the catalog's safe-integer seed contract.
        self.content
            .create_loot_items(input.actor_seed, input.enemy_token == EnemyToken::Demon)
            .expect("enemy loot supplies a native semantic seed")
    }

    fn filter_damage(
        &self,
        input: GameSimulationDamageFilterInput<'_>,
        equipment: &HubEquipmentState,
    ) -> Result<f64, String> {
        let status =
            self.statuses.filter_damage(input.target_player_id, input.amount, input.tick)?;
        let skill = self.skills.filter(input.target_player_id, "incoming_damage", status)?;
        Ok(filter_equipment_modifiers(equipment, "incoming_damage", skill))
    }

    fn filter_mana(
        &self,
        input: GameSimulationManaFilterInput<'_>,
        equipment: &HubEquipmentState,
    ) -> Result<f64, String> {
        let status = self.statuses.filter_mana(input.player_id, input.delta, input.tick)?;
        if status >= 0.0 {
            return Ok(status);
        }
        let skill = self.skills.filter(input.player_id, "mana_spend", -status)?;
        Ok(-filter_equipment_modifiers(equipment, "mana_spend", skill))
    }

    fn has_consumable(&self, content_id: &str) -> bool {
        self.content.potion(content_id).is_some()
    }

    fn resolve_boast(&self, selection: &BoastSelection) -> Option<BoastRules<'_>> {
        if matches!(selection, BoastSelection::Native(_)) {
            return None;
        }
        self.content.boast_rules(selection)
    }
}

/// Source slot and affix iteration order is observable for mixed add/set/multiply.
pub fn filter_equipment_modifiers(equipment: &HubEquipmentState, key: &str, source: f64) -> f64 {
    let mut value = source;
    let items = [
        equipment.amulet.as_ref(),
        equipment.hat.as_ref(),
        equipment.rings[0].as_ref(),
        equipment.rings[1].as_ref(),
        equipment.rings[2].as_ref(),
        equipment.robe.as_ref(),
        equipment.weapon.as_ref(),
    ];
    for item in items.into_iter().flatten() {
        for affix in item.mod_affixes.as_deref().unwrap_or(&[]) {
            for modifier in &affix.modifiers {
                if modifier.key == key {
                    value = match modifier.operation {
                        ModModifierOperation::Add => value + modifier.value,
                        ModModifierOperation::Multiply => value * modifier.value,
                        ModModifierOperation::Set => modifier.value,
                    };
                }
            }
        }
    }
    if value.is_nan() { value } else { value.max(0.0) }
}
