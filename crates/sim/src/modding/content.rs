//! Prepared mod content: one definition type per authored family, and their catalog.
//! Definitions are immutable. The native catalog validates and indexes one
//! `PreparedModContentDefinition`; clients borrow the same accepted values.

use std::collections::BTreeMap;

use crate::hub::interaction::BoastFailureProducer;

use super::assets::ModSpriteFrame;
use super::definition::{
    ResolvedWebLuaContentReference, WebLuaAssetKind, WebLuaContentKind, WebLuaRuleDefinition,
};
use super::inventory::{ModConsumableCatalogEntry, ModItemCatalogEntry};
use super::{LuaObject, LuaValue};

/// What reapplying an active status does: ignore it, refresh it, replace it or stack it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModStatusStacking {
    Ignore,
    Refresh,
    Replace,
    Stack,
}

/// One named art binding of a content entry: the bound asset's ID, kind, key and path.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModArtBinding {
    pub asset_id: String,
    pub asset_kind: WebLuaAssetKind,
    pub key: String,
    pub path: String,
}

/// The family discriminant is stored once, in the enclosing prepared enum.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModContentEntry {
    pub art: BTreeMap<String, PreparedModArtBinding>,
    pub content_id: String,
    pub description: String,
    pub fields: LuaObject,
    pub key: String,
    pub mod_id: String,
    pub name: String,
}

/// An authored status effect: its duration, modifiers and stacking rule.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModStatusDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub duration_ms: f64,
    pub modifiers: LuaObject,
    pub stacking: ModStatusStacking,
}

/// A mod potion's drop chance from ordinary enemies and from bosses.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModPotionLoot {
    pub boss: f64,
    pub ordinary: f64,
}

/// An authored potion: its catalog entry, loot chances, use rule and the status it applies.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModPotionDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub catalog: ModConsumableCatalogEntry,
    pub loot: Option<ModPotionLoot>,
    pub on_use: WebLuaRuleDefinition,
    pub status: Option<ResolvedWebLuaContentReference>,
}

/// An authored item: its catalog entry, stack limit and optional use rule.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModItemDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub catalog: ModItemCatalogEntry,
    pub stack_maximum: u32,
    #[cfg_attr(feature = "persistence", serde(rename = "use"))]
    pub use_rule: Option<WebLuaRuleDefinition>,
}

/// An authored powerup: its pickup radius and effect rule.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModPowerupDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub effect: WebLuaRuleDefinition,
    pub pickup_radius: f64,
}

/// An authored equipment affix: what it applies to and its modifiers.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModAffixDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub applies_to: Vec<String>,
    pub modifiers: LuaObject,
}

/// One weighted affix in an affix pool.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PreparedModAffixPoolEntry {
    pub affix: ResolvedWebLuaContentReference,
    pub weight: f64,
}

/// An authored affix pool: its weighted affixes, roll count, what it applies to and its RNG
/// domain.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModAffixPoolDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub applies_to: Vec<String>,
    pub entries: Vec<PreparedModAffixPoolEntry>,
    pub rng_domain: String,
    pub rolls: u32,
}

/// An authored skill: its ranks and their grants, prerequisites, parent, minimum level and
/// offer weight.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModSkillDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub grants: Vec<ResolvedWebLuaContentReference>,
    pub maximum_rank: u32,
    pub minimum_level: u32,
    pub offer_weight: f64,
    pub parent: Option<ResolvedWebLuaContentReference>,
    pub prerequisites: Vec<ResolvedWebLuaContentReference>,
    pub rank_grants: Vec<Vec<ResolvedWebLuaContentReference>>,
    pub ranks: Vec<LuaObject>,
}

/// Whether an authored spell is a primary or a secondary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModSpellSlot {
    Primary,
    Secondary,
}

/// An authored spell: its slot, mana cost, cooldown and behavior rule.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModSpellDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub behavior: WebLuaRuleDefinition,
    pub cooldown_ms: f64,
    pub mana: f64,
    pub slot: ModSpellSlot,
}

/// An authored enemy: its health, attack, speed, size, experience and gold drop.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModEnemyDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub attack_cooldown_ms: f64,
    pub attack_damage: f64,
    pub attack_range: f64,
    pub health: f64,
    pub collision_radius: f64,
    pub gold_maximum: u32,
    pub gold_minimum: u32,
    pub experience: f64,
    pub scale: f64,
    pub speed: f64,
}

/// An authored Boneyard: its source, environment, anchors, roster, waves and triggers.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModBoneyardDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub anchors: LuaObject,
    pub environment: LuaObject,
    pub roster: Vec<LuaValue>,
    pub source: String,
    pub triggers: Vec<WebLuaRuleDefinition>,
    pub waves: Vec<LuaObject>,
}

/// A boast's icon: a frame of a mod image, or a stock icon record and style.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "persistence",
    serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")
)]
pub enum PreparedModBoastIcon {
    Mod { frame: ModSpriteFrame, image_height: u32, image_path: String, image_width: u32 },
    Stock { record: usize, style: usize },
}

/// The persisted identity of a selected mod boast: its content ID and owning mod.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename = "mod", rename_all = "camelCase"))]
pub struct ModBoastSelection {
    pub content_id: String,
    pub mod_id: String,
}

impl ModBoastSelection {
    /// Validates a persisted identity: a decimal content ID of at most 19 digits with no
    /// leading zero, and a valid mod ID.
    pub fn new(content_id: String, mod_id: String) -> Result<Self, String> {
        if content_id.is_empty()
            || content_id.len() > 19
            || content_id.starts_with('0')
            || !content_id.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(
                "mod Boast content ID must be a positive decimal string of at most 19 digits"
                    .into(),
            );
        }
        let valid_end = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
        if mod_id.is_empty()
            || mod_id.len() > 128
            || !valid_end(mod_id.as_bytes()[0])
            || !valid_end(*mod_id.as_bytes().last().expect("owner ID is nonempty"))
            || !mod_id.bytes().all(|byte| valid_end(byte) || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err("mod Boast owner ID is invalid".into());
        }
        Ok(Self { content_id, mod_id })
    }
}

/// An authored boast: its text, icon, failure producers, success wave and score multiplier.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModBoastDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub failure_producers: Vec<BoastFailureProducer>,
    pub icon: PreparedModBoastIcon,
    pub instruction: String,
    pub label: String,
    pub random_skill_choices: bool,
    pub response: String,
    pub score_multiplier: f64,
    pub selection: ModBoastSelection,
    pub statement: String,
    pub success_wave: u32,
}

impl PreparedModBoastDefinition {
    #[must_use]
    pub fn rules(&self) -> crate::hub::interaction::BoastRules<'_> {
        crate::hub::interaction::BoastRules {
            failure_producers: &self.failure_producers,
            label: &self.label,
            random_skill_choices: self.random_skill_choices,
            score_multiplier: self.score_multiplier,
            success_wave: self.success_wave,
        }
    }
}

/// Who shares a mod shop's stock: the party, each player or the whole session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModShopStockScope {
    Party,
    Player,
    Session,
}

/// One stocked shop item: the item, its price and quantity.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PreparedModShopStock {
    pub item: ResolvedWebLuaContentReference,
    pub price: u64,
    pub quantity: u32,
}

/// A shop's reforge service: the affix pool it draws from and its price.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "type", rename = "reforge"))]
pub struct PreparedModShopService {
    pub pool: ResolvedWebLuaContentReference,
    pub price: u64,
}

/// An authored shop: its stock and stock scope, restock time, services, NPC and mount.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModShopDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub mount: Option<LuaObject>,
    pub npc: Option<LuaObject>,
    pub restock_ms: f64,
    pub services: Vec<PreparedModShopService>,
    pub stock: Vec<PreparedModShopStock>,
    pub stock_scope: ModShopStockScope,
}

/// An authored UI panel: its mount, accessible name, view rule, bindings, actions and
/// visibility.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModUiDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub accessible_name: String,
    pub actions: Vec<String>,
    pub bindings: LuaObject,
    pub mount: String,
    pub visible: LuaObject,
    pub view: WebLuaRuleDefinition,
}

/// An authored room and its geometry.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PreparedModRoomDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub geometry: LuaObject,
}

/// An authored scene and the rooms it contains.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PreparedModSceneDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub rooms: Vec<ResolvedWebLuaContentReference>,
}

/// Authored features added to an existing scene.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PreparedModSceneExtensionDefinition {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub common: PreparedModContentEntry,
    pub features: Vec<WebLuaRuleDefinition>,
    pub scene: String,
}

/// One prepared mod content definition, by family.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "contentKind", rename_all = "kebab-case"))]
pub enum PreparedModContentDefinition {
    Affix(PreparedModAffixDefinition),
    AffixPool(PreparedModAffixPoolDefinition),
    Boneyard(PreparedModBoneyardDefinition),
    Boast(PreparedModBoastDefinition),
    Enemy(PreparedModEnemyDefinition),
    Item(PreparedModItemDefinition),
    Potion(PreparedModPotionDefinition),
    Powerup(PreparedModPowerupDefinition),
    Room(PreparedModRoomDefinition),
    Scene(PreparedModSceneDefinition),
    SceneExtension(PreparedModSceneExtensionDefinition),
    Shop(PreparedModShopDefinition),
    Skill(PreparedModSkillDefinition),
    Spell(PreparedModSpellDefinition),
    Status(PreparedModStatusDefinition),
    Ui(PreparedModUiDefinition),
}

impl PreparedModContentDefinition {
    pub fn common(&self) -> &PreparedModContentEntry {
        match self {
            Self::Affix(value) => &value.common,
            Self::AffixPool(value) => &value.common,
            Self::Boneyard(value) => &value.common,
            Self::Boast(value) => &value.common,
            Self::Enemy(value) => &value.common,
            Self::Item(value) => &value.common,
            Self::Potion(value) => &value.common,
            Self::Powerup(value) => &value.common,
            Self::Room(value) => &value.common,
            Self::Scene(value) => &value.common,
            Self::SceneExtension(value) => &value.common,
            Self::Shop(value) => &value.common,
            Self::Skill(value) => &value.common,
            Self::Spell(value) => &value.common,
            Self::Status(value) => &value.common,
            Self::Ui(value) => &value.common,
        }
    }

    pub const fn kind(&self) -> WebLuaContentKind {
        match self {
            Self::Affix(_) => WebLuaContentKind::Affix,
            Self::AffixPool(_) => WebLuaContentKind::AffixPool,
            Self::Boneyard(_) => WebLuaContentKind::Boneyard,
            Self::Boast(_) => WebLuaContentKind::Boast,
            Self::Enemy(_) => WebLuaContentKind::Enemy,
            Self::Item(_) => WebLuaContentKind::Item,
            Self::Potion(_) => WebLuaContentKind::Potion,
            Self::Powerup(_) => WebLuaContentKind::Powerup,
            Self::Room(_) => WebLuaContentKind::Room,
            Self::Scene(_) => WebLuaContentKind::Scene,
            Self::SceneExtension(_) => WebLuaContentKind::SceneExtension,
            Self::Shop(_) => WebLuaContentKind::Shop,
            Self::Skill(_) => WebLuaContentKind::Skill,
            Self::Spell(_) => WebLuaContentKind::Spell,
            Self::Status(_) => WebLuaContentKind::Status,
            Self::Ui(_) => WebLuaContentKind::Ui,
        }
    }
}

/// One accepted immutable catalog. The native compiler supplies sorted rows;
/// this index keeps every family lookup on those same definitions.
#[derive(Clone, Debug)]
pub struct PreparedModContentCatalog {
    content: Vec<PreparedModContentDefinition>,
    by_id: BTreeMap<String, usize>,
}

impl PreparedModContentCatalog {
    /// Mod potion loot uses a content-addressed SplitMix64 sample and never
    /// advances the world's gameplay or loot RNG streams.
    pub fn create_loot_items(
        &self,
        actor_seed: u64,
        boss: bool,
    ) -> Result<Vec<crate::economy::HubInventoryItem>, String> {
        if actor_seed > 9_007_199_254_740_991 {
            return Err("mod loot actor seed must be a non-negative safe integer".into());
        }
        let mut items = Vec::new();
        let mut index = 0_u64;
        for definition in &self.content {
            let PreparedModContentDefinition::Potion(potion) = definition else { continue };
            let Some(loot) = potion.loot else { continue };
            let id = potion
                .common
                .content_id
                .parse::<u64>()
                .expect("prepared content identity is numeric");
            let mut mixed = actor_seed ^ id ^ (index + 1);
            mixed = mixed.wrapping_add(0x9e37_79b9_7f4a_7c15);
            mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            mixed ^= mixed >> 31;
            let roll = (mixed >> 11) as f64 / 9_007_199_254_740_992.0;
            if roll < if boss { loot.boss } else { loot.ordinary } {
                items.push(super::inventory::mod_consumable_inventory_item(&potion.catalog));
            }
            index += 1;
        }
        Ok(items)
    }
    #[must_use]
    pub fn boast_rules(
        &self,
        selection: &crate::economy::BoastSelection,
    ) -> Option<crate::hub::interaction::BoastRules<'_>> {
        match selection {
            crate::economy::BoastSelection::Native(_) => {
                crate::hub::interaction::boast_rules_for_selection(selection)
            }
            crate::economy::BoastSelection::Mod(selection) => {
                let definition = self.boast(&selection.content_id)?;
                (definition.common.mod_id == selection.mod_id).then(|| definition.rules())
            }
        }
    }
    pub fn from_prepared(content: Vec<PreparedModContentDefinition>) -> Result<Self, String> {
        let mut by_id = BTreeMap::new();
        for (index, definition) in content.iter().enumerate() {
            let id = &definition.common().content_id;
            if by_id.insert(id.clone(), index).is_some() {
                return Err(format!("prepared mod content is duplicated: {id}"));
            }
        }
        Ok(Self { content, by_id })
    }

    pub fn all(&self) -> &[PreparedModContentDefinition] {
        &self.content
    }

    pub fn content(&self, content_id: &str) -> Option<&PreparedModContentDefinition> {
        self.by_id.get(content_id).map(|&index| &self.content[index])
    }

    pub fn status(&self, content_id: &str) -> Option<&PreparedModStatusDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Status(value) => Some(value),
            _ => None,
        }
    }

    pub fn potion(&self, content_id: &str) -> Option<&PreparedModPotionDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Potion(value) => Some(value),
            _ => None,
        }
    }

    pub fn item(&self, content_id: &str) -> Option<&PreparedModItemDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Item(value) => Some(value),
            _ => None,
        }
    }

    pub fn powerup(&self, content_id: &str) -> Option<&PreparedModPowerupDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Powerup(value) => Some(value),
            _ => None,
        }
    }

    pub fn affix(&self, content_id: &str) -> Option<&PreparedModAffixDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Affix(value) => Some(value),
            _ => None,
        }
    }

    pub fn affix_pool(&self, content_id: &str) -> Option<&PreparedModAffixPoolDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::AffixPool(value) => Some(value),
            _ => None,
        }
    }

    pub fn skill(&self, content_id: &str) -> Option<&PreparedModSkillDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Skill(value) => Some(value),
            _ => None,
        }
    }

    pub fn spell(&self, content_id: &str) -> Option<&PreparedModSpellDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Spell(value) => Some(value),
            _ => None,
        }
    }

    pub fn enemy(&self, content_id: &str) -> Option<&PreparedModEnemyDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Enemy(value) => Some(value),
            _ => None,
        }
    }

    pub fn boneyard(&self, content_id: &str) -> Option<&PreparedModBoneyardDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Boneyard(value) => Some(value),
            _ => None,
        }
    }

    pub fn boast(&self, content_id: &str) -> Option<&PreparedModBoastDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Boast(value) => Some(value),
            _ => None,
        }
    }

    pub fn room(&self, content_id: &str) -> Option<&PreparedModRoomDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Room(value) => Some(value),
            _ => None,
        }
    }

    pub fn scene(&self, content_id: &str) -> Option<&PreparedModSceneDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Scene(value) => Some(value),
            _ => None,
        }
    }

    pub fn shop(&self, content_id: &str) -> Option<&PreparedModShopDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Shop(value) => Some(value),
            _ => None,
        }
    }

    pub fn ui(&self, content_id: &str) -> Option<&PreparedModUiDefinition> {
        match self.content(content_id)? {
            PreparedModContentDefinition::Ui(value) => Some(value),
            _ => None,
        }
    }

    pub fn skills(&self) -> impl Iterator<Item = &PreparedModSkillDefinition> {
        self.content.iter().filter_map(|entry| match entry {
            PreparedModContentDefinition::Skill(value) => Some(value),
            _ => None,
        })
    }

    pub fn spells(&self) -> impl Iterator<Item = &PreparedModSpellDefinition> {
        self.content.iter().filter_map(|entry| match entry {
            PreparedModContentDefinition::Spell(value) => Some(value),
            _ => None,
        })
    }

    pub fn enemies(&self) -> impl Iterator<Item = &PreparedModEnemyDefinition> {
        self.content.iter().filter_map(|entry| match entry {
            PreparedModContentDefinition::Enemy(value) => Some(value),
            _ => None,
        })
    }

    pub fn boasts(&self) -> impl Iterator<Item = &PreparedModBoastDefinition> {
        self.content.iter().filter_map(|entry| match entry {
            PreparedModContentDefinition::Boast(value) => Some(value),
            _ => None,
        })
    }

    pub fn scene_extensions(&self) -> impl Iterator<Item = &PreparedModSceneExtensionDefinition> {
        self.content.iter().filter_map(|entry| match entry {
            PreparedModContentDefinition::SceneExtension(value) => Some(value),
            _ => None,
        })
    }

    pub fn uis(&self) -> impl Iterator<Item = &PreparedModUiDefinition> {
        self.content.iter().filter_map(|entry| match entry {
            PreparedModContentDefinition::Ui(value) => Some(value),
            _ => None,
        })
    }

    pub fn items(&self) -> impl Iterator<Item = &ModItemCatalogEntry> {
        self.content.iter().filter_map(|entry| match entry {
            PreparedModContentDefinition::Item(value) => Some(&value.catalog),
            _ => None,
        })
    }

    pub fn consumables(&self) -> impl Iterator<Item = &ModConsumableCatalogEntry> {
        self.content.iter().filter_map(|entry| match entry {
            PreparedModContentDefinition::Potion(value) => Some(&value.catalog),
            _ => None,
        })
    }
}
