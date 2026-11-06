//! Each participant's inventory, equipment and Gold, and the College services that use them:
//! Fomentius's shop, Hagatha's perks, Luthacus's storage, Shlorio's dowsing, dye and Unforge.
//!
//! Items preserve their addressed cells and nested sack ownership. Commands are atomic:
//! rejected moves retain the original inventory and native random stream.

mod archive;
mod catalog;
mod feedback;
mod inventory;
mod services;

pub use crate::equipment::EquipmentEffect;
pub use archive::{CompletedRunEconomyArchive, RETAINED_SACK_SUFFIXES};
pub use catalog::{
    DOWSING_EQUIPMENT_RECIPES, FOMENTIUS_STOCK_DEFINITIONS, HAGATHA_PERKS, SKILL_BOOK_DEFINITIONS,
};
pub use feedback::{
    HubActionFeedback, HubActionFeedbackAction, HubTransferGesture, SkillBookFeedbackCursor,
    SkillBookOutcome, SkillBookReceipt, skill_book_result_text,
};
pub(crate) use inventory::base_item;
pub use inventory::{
    INVENTORY_SACK_NAME_MAX_LENGTH, ProjectedInventoryItem, ProjectedInventoryRootSlot,
    create_equipment_inventory_item, create_fomentius_inventory_item,
    create_skill_book_inventory_item, equipment_meets_level_requirement, equipment_required_level,
    find_inventory_item, inventory_items_at_sack_path, inventory_items_share_stack,
    normalize_inventory_sack_name, project_inventory_items, project_inventory_root_slots,
    reconcile_inventory_sack_path,
};
pub use services::{
    dye_committed_tint, dye_mixed_tint, hagatha_bundle_state_is_valid,
    hagatha_outcome_state_is_valid,
};

use crate::Rng;
use crate::appearance::roll_starter_equipment_appearance;
use crate::modding::content::ModBoastSelection;
use crate::modding::inventory::{ModConsumableContent, ModEquipmentAffix, ModItemContent};
use crate::profile::WizardElement;

pub const FRESH_PROFILE_GOLD: u64 = 500;
pub const HUB_INVENTORY_SLOT_CAPACITY: usize = 88;
pub const HUB_STORAGE_SLOT_CAPACITY: usize = 28;
pub const DOWSING_MAX_OFFERS: usize = 6;
pub const LOOT_BACKPACK_REPLICATION_LIMIT: usize = 2048;
pub const HUB_SACK_CHILD_REPLICATION_LIMIT: usize = LOOT_BACKPACK_REPLICATION_LIMIT + 7;
pub const HUB_SACK_REPLICATION_DEPTH_LIMIT: usize = 32;

/// The kind of equipment an item is; staffs and wands are both weapons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EquipmentType {
    Amulet,
    Hat,
    Ring,
    Robe,
    Staff,
    Wand,
}

impl EquipmentType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Amulet => "amulet",
            Self::Hat => "hat",
            Self::Ring => "ring",
            Self::Robe => "robe",
            Self::Staff => "staff",
            Self::Wand => "wand",
        }
    }
}

/// A wizard's equipment slot: amulet, hat, three rings, robe and weapon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EquipmentSlot {
    Amulet,
    Hat,
    Ring0,
    Ring1,
    Ring2,
    Robe,
    Weapon,
}

impl EquipmentSlot {
    pub const ALL: [Self; 7] =
        [Self::Amulet, Self::Hat, Self::Ring0, Self::Ring1, Self::Ring2, Self::Robe, Self::Weapon];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Amulet => "amulet",
            Self::Hat => "hat",
            Self::Ring0 => "ring-0",
            Self::Ring1 => "ring-1",
            Self::Ring2 => "ring-2",
            Self::Robe => "robe",
            Self::Weapon => "weapon",
        }
    }

    #[must_use]
    pub fn accepts(self, equipment_type: EquipmentType) -> bool {
        match self {
            Self::Amulet => equipment_type == EquipmentType::Amulet,
            Self::Hat => equipment_type == EquipmentType::Hat,
            Self::Ring0 | Self::Ring1 | Self::Ring2 => equipment_type == EquipmentType::Ring,
            Self::Robe => equipment_type == EquipmentType::Robe,
            Self::Weapon => matches!(equipment_type, EquipmentType::Staff | EquipmentType::Wand),
        }
    }
}

/// A College NPC who runs an economy service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubTraderId {
    Fomentius,
    Hagatha,
    Luthacus,
    Shlorio,
}

impl HubTraderId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fomentius => "fomentius",
            Self::Hagatha => "hagatha",
            Self::Luthacus => "luthacus",
            Self::Shlorio => "shlorio",
        }
    }
}

/// The category of an inventory item, such as a potion, equipment, a Sack or a mod item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HubItemKind {
    Antidote,
    Dye,
    Equipment,
    HealthPotion,
    Key,
    ManaPotion,
    MindChug,
    ModItem,
    ModPotion,
    RejuvenationPotion,
    Sack,
    SkillBook,
    WizardChug,
}

impl HubItemKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Antidote => "antidote",
            Self::Dye => "dye",
            Self::Equipment => "equipment",
            Self::HealthPotion => "health-potion",
            Self::Key => "key",
            Self::ManaPotion => "mana-potion",
            Self::MindChug => "mind-chug",
            Self::ModItem => "mod-item",
            Self::ModPotion => "mod-potion",
            Self::RejuvenationPotion => "rejuvenation-potion",
            Self::Sack => "sack",
            Self::SkillBook => "skill-book",
            Self::WizardChug => "wizard-chug",
        }
    }
}

/// The rarity of special equipment: Rare or Epic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum ItemRarity {
    Epic,
    Rare,
}

/// One item in an inventory, Sack, equipment slot or shop. `None` in `contents` or
/// `native_effects` means the field is absent, which is not the same as an empty list.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubInventoryItem {
    pub contents: Option<Vec<HubInventoryItem>>,
    pub equipment_type: Option<EquipmentType>,
    pub generated_level: Option<u32>,
    pub icon_records: Vec<usize>,
    pub icon_tints: Option<[Option<u32>; 2]>,
    pub id: u64,
    pub inventory_slot: Option<usize>,
    pub kind: HubItemKind,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub mod_content: Option<ModConsumableContent>,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub mod_item_content: Option<ModItemContent>,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub mod_affixes: Option<Vec<ModEquipmentAffix>>,
    pub name: String,
    #[cfg_attr(feature = "persistence", serde(rename = "native_subtype"))]
    pub subtype: Option<u32>,
    #[cfg_attr(feature = "persistence", serde(rename = "native_selector"))]
    pub selector: Option<u32>,
    pub native_effects: Option<Vec<EquipmentEffect>>,
    #[cfg_attr(feature = "persistence", serde(rename = "native_type_id"))]
    pub type_id: u32,
    pub quantity: u32,
    pub rarity: Option<ItemRarity>,
    pub recipe_index: Option<usize>,
}

/// An item in Fomentius's shop and its price.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubShopItem {
    pub item: HubInventoryItem,
    pub price: u64,
}

/// An authored named equipment item, offered by dowsing and dropped as loot. `source_index` is
/// its position in `DOWSING_EQUIPMENT_RECIPES`; items store it as `recipe_index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EquipmentRecipe {
    pub icon_records: &'static [usize],
    pub icon_tints: [Option<u32>; 2],
    pub level: u32,
    pub name: &'static str,
    pub type_id: u32,
    pub rarity: ItemRarity,
    pub set_name: Option<&'static str>,
    pub source_index: usize,
    pub equipment_type: EquipmentType,
}

/// An authored row of Fomentius's shop: the item, its price, and how a restock rolls the
/// number to stock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FomentiusStockDefinition {
    pub gate_value: Option<u32>,
    pub icon_records: &'static [usize],
    pub kind: HubItemKind,
    pub name: &'static str,
    pub subtype: u32,
    pub type_id: u32,
    pub price: u64,
    pub quantity_offset: u32,
    pub roll_bound: u32,
}

/// An authored Book of Skill item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillBookDefinition {
    pub icon_records: &'static [usize],
    pub key: &'static str,
    pub name: &'static str,
    pub subtype: u32,
    pub type_id: u32,
}

/// An authored charm, curse or Tonic that Hagatha sells, identified by its selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HagathaPerkDefinition {
    pub base_price: u64,
    pub behavior_family: &'static str,
    pub description: &'static str,
    pub name: &'static str,
    pub selector: u32,
}

/// One of Hagatha's current offers: a single perk, or the Bargain Bundle (selector `-1`) of
/// several, with its price.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HagathaOffer {
    pub selector: i32,
    pub name: String,
    pub description: String,
    pub members: Vec<u32>,
    pub price: u64,
}

/// An equipment recipe that Shlorio's dowsing found, offered for sale at a price.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DowsingOffer {
    pub id: u64,
    pub price: u64,
    pub recipe_index: usize,
}

/// The items a wizard has equipped, one per equipment slot.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubEquipmentState {
    pub amulet: Option<HubInventoryItem>,
    pub hat: Option<HubInventoryItem>,
    pub rings: [Option<HubInventoryItem>; 3],
    pub robe: Option<HubInventoryItem>,
    pub weapon: Option<HubInventoryItem>,
}

impl HubEquipmentState {
    #[must_use]
    pub fn at(&self, slot: EquipmentSlot) -> &Option<HubInventoryItem> {
        match slot {
            EquipmentSlot::Amulet => &self.amulet,
            EquipmentSlot::Hat => &self.hat,
            EquipmentSlot::Ring0 => &self.rings[0],
            EquipmentSlot::Ring1 => &self.rings[1],
            EquipmentSlot::Ring2 => &self.rings[2],
            EquipmentSlot::Robe => &self.robe,
            EquipmentSlot::Weapon => &self.weapon,
        }
    }

    pub fn set(&mut self, slot: EquipmentSlot, item: Option<HubInventoryItem>) {
        let destination = match slot {
            EquipmentSlot::Amulet => &mut self.amulet,
            EquipmentSlot::Hat => &mut self.hat,
            EquipmentSlot::Ring0 => &mut self.rings[0],
            EquipmentSlot::Ring1 => &mut self.rings[1],
            EquipmentSlot::Ring2 => &mut self.rings[2],
            EquipmentSlot::Robe => &mut self.robe,
            EquipmentSlot::Weapon => &mut self.weapon,
        };
        *destination = item;
    }

    #[must_use]
    pub fn has_complete_set(&self, recipe_indices: &[usize]) -> bool {
        recipe_indices.iter().all(|index| {
            EquipmentSlot::ALL.iter().any(|slot| {
                self.at(*slot).as_ref().is_some_and(|item| item.recipe_index == Some(*index))
            })
        })
    }
}

/// A selected Boast: a stock Boast by number or a mod-authored one. Both forms share one
/// serialized field.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(untagged))]
pub enum BoastSelection {
    Native(u32),
    Mod(ModBoastSelection),
}

impl BoastSelection {
    #[must_use]
    pub const fn native_id(&self) -> Option<u32> {
        match self {
            Self::Native(id) => Some(*id),
            Self::Mod(_) => None,
        }
    }
}

impl From<u32> for BoastSelection {
    fn from(value: u32) -> Self {
        Self::Native(value)
    }
}

/// The wizard's selected Boast, whether it has failed or succeeded, and a sequence number
/// that advances on each failure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoastState {
    pub failed: bool,
    pub failure_sequence: u64,
    pub selected: Option<BoastSelection>,
    pub succeeded: bool,
}

/// A wizard's College NPC state: the Boast, which onboarding hints are still pending, and
/// whether the Librarian's one-time book has been read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeHubNpcState {
    pub boast: BoastState,
    pub help_flags: [bool; 10],
    pub librarian_lace_read: bool,
}

impl Default for NativeHubNpcState {
    fn default() -> Self {
        Self { boast: BoastState::default(), help_flags: [true; 10], librarian_lace_read: false }
    }
}

/// Permanent stat bonuses won by unforging items, and the count of recipe-backed attempts,
/// which changes the odds of later attempts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct UnforgeBonuses {
    pub experience: f64,
    pub mana_cost_reduction: f64,
    pub maximum_health: f64,
    pub maximum_mana: f64,
    pub offensive_damage: f64,
    pub recipe_attempt_count: u32,
}

/// What unforging an item produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum UnforgeOutcomeKind {
    Experience,
    Fizzle,
    FullRejuvenation,
    Gold,
    ManaCost,
    MaximumHealth,
    MaximumMana,
    MindDredge,
    OffensiveDamage,
}

impl UnforgeOutcomeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Experience => "experience",
            Self::Fizzle => "fizzle",
            Self::FullRejuvenation => "full-rejuvenation",
            Self::Gold => "gold",
            Self::ManaCost => "mana-cost",
            Self::MaximumHealth => "maximum-health",
            Self::MaximumMana => "maximum-mana",
            Self::MindDredge => "mind-dredge",
            Self::OffensiveDamage => "offensive-damage",
        }
    }
}

/// The result of unforging one item: the outcome, its amount if it has one, and the item's
/// name.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct UnforgeOutcome {
    pub amount: Option<f64>,
    pub item_name: String,
    pub kind: UnforgeOutcomeKind,
}

/// One participant's economy: backpack, storage, equipment and Gold, the College services'
/// state, and the random stream those services draw from.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubEconomyState {
    /// Receipt of the latest inventory or service command. Older native checkpoints lack it.
    #[cfg_attr(feature = "persistence", serde(default))]
    pub action_feedback: Option<HubActionFeedback>,
    pub backpack: Vec<HubInventoryItem>,
    pub charm_capacity: u32,
    pub college_intro_pending: bool,
    pub dowsing_fee: u64,
    pub dowsing_offers: Vec<DowsingOffer>,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub dowsing_rolled: bool,
    pub equipment: HubEquipmentState,
    pub first_mixed_selectors: Vec<u32>,
    pub fomentius_stock: Vec<HubShopItem>,
    pub gold: u64,
    pub hagatha_bundle_selectors: Vec<u32>,
    pub next_item_id: u64,
    pub next_offer_id: u64,
    pub npc: NativeHubNpcState,
    pub owned_perk_selectors: Vec<u32>,
    pub revision: u64,
    pub rng: Rng,
    pub storage: Vec<HubInventoryItem>,
    pub tonic_purchases: u32,
    pub tutorial_pending: bool,
    pub unforge_bonuses: UnforgeBonuses,
}

/// Why an economy command was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HubEconomyRejection {
    CapacityFull,
    IneligibleItem,
    InsufficientGold,
    InvalidInventory,
    InvalidName,
    InvalidOffer,
    InvalidSlot,
    InvalidTarget,
    ItemNotFound,
    OffersActive,
    PerkCapacityFull,
    RequiredClothing,
    SlotEmpty,
    SlotLocked,
    ServiceUnavailable,
}

impl HubEconomyRejection {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CapacityFull => "capacity-full",
            Self::IneligibleItem => "ineligible-item",
            Self::InsufficientGold => "insufficient-gold",
            Self::InvalidInventory => "invalid-inventory",
            Self::InvalidName => "invalid-name",
            Self::InvalidOffer => "invalid-offer",
            Self::InvalidSlot => "invalid-slot",
            Self::InvalidTarget => "invalid-target",
            Self::ItemNotFound => "item-not-found",
            Self::OffersActive => "offers-active",
            Self::PerkCapacityFull => "perk-capacity-full",
            Self::RequiredClothing => "required-clothing",
            Self::SlotEmpty => "slot-empty",
            Self::SlotLocked => "slot-locked",
            Self::ServiceUnavailable => "service-unavailable",
        }
    }
}

/// The result of one economy command: acceptance or the rejection reason, and any dowsing
/// pitch, dye session or Unforge outcome it produced.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubEconomyResult {
    pub accepted: bool,
    pub reason: Option<HubEconomyRejection>,
    pub dowsing_pitch: Option<f64>,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub dye_session: Option<InventoryDyeSession>,
    pub unforge_outcome: Option<UnforgeOutcome>,
}

impl HubEconomyResult {
    fn accepted() -> Self {
        Self {
            accepted: true,
            reason: None,
            dowsing_pitch: None,
            dye_session: None,
            unforge_outcome: None,
        }
    }

    fn rejected(reason: HubEconomyRejection) -> Self {
        Self {
            accepted: false,
            reason: Some(reason),
            dowsing_pitch: None,
            dye_session: None,
            unforge_outcome: None,
        }
    }
}

/// Which way a storage transfer moves an item: into the backpack or into storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum TransferDirection {
    ToBackpack,
    ToStorage,
}
/// Which of a hat's or robe's two colours a dye paints: the cloth or the trim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum DyeLayer {
    Cloth,
    Trim,
}

/// One open Fabric Dye painting, owned transiently by the Session rather than a save.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct InventoryDyeSession {
    pub id: String,
    pub dye_item_id: u64,
    pub kit_consumed: bool,
}

/// An inventory or College service command applied to one participant's economy.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HubInventoryAction {
    AcknowledgeNpcHint {
        interaction_id: crate::hub::interaction::HubInteractionId,
    },
    BuyFomentius {
        item_id: u64,
    },
    BuyHagatha {
        selector: i32,
    },
    CloseHagatha,
    RemoveHagatha {
        selector: u32,
    },
    BuyDowsing {
        offer_id: u64,
    },
    Dowse {
        reference_item_id: Option<u64>,
    },
    CloseDowsing,
    Equip {
        item_id: u64,
        slot: EquipmentSlot,
    },
    Unequip {
        slot: EquipmentSlot,
        destination_sack_id: Option<u64>,
        destination_slot: Option<usize>,
    },
    EquipSackContents {
        item_id: u64,
    },
    Transfer {
        item_id: u64,
        direction: TransferDirection,
        #[cfg_attr(feature = "persistence", serde(default))]
        gesture: HubTransferGesture,
    },
    Move {
        item_id: u64,
        destination_sack_id: Option<u64>,
        destination_slot: Option<usize>,
    },
    RenameSack {
        item_id: u64,
        name: String,
    },
    Consume {
        item_id: u64,
    },
    Discard {
        item_id: u64,
    },
    ReadSkillBook {
        item_id: u64,
    },
    OpenDye {
        dye_item_id: u64,
        session_id: String,
    },
    CloseDye {
        session_id: String,
    },
    Dye {
        session_id: String,
        target_item_id: u64,
        layer: DyeLayer,
        swatch_rows: Vec<usize>,
    },
    Unforge {
        item_id: u64,
    },
    SelectBoast {
        boast_id: BoastSelection,
    },
    ReadLibrarianBook {
        book_id: u32,
    },
    BuyTeacherSpell {
        skill_id: usize,
    },
}

impl HubInventoryAction {
    #[must_use]
    pub const fn trader(&self) -> Option<HubTraderId> {
        match self {
            Self::BuyFomentius { .. } => Some(HubTraderId::Fomentius),
            Self::BuyHagatha { .. } | Self::CloseHagatha | Self::RemoveHagatha { .. } => {
                Some(HubTraderId::Hagatha)
            }
            Self::Transfer { .. } => Some(HubTraderId::Luthacus),
            Self::Dowse { .. } | Self::BuyDowsing { .. } => Some(HubTraderId::Shlorio),
            _ => None,
        }
    }

    /// Whether the command needs a College service. The session admits such commands only in
    /// the Hub phase, with no Boneyard and no doorway transition in progress.
    #[must_use]
    pub fn requires_hub_service(&self) -> bool {
        self.trader().is_some()
            || matches!(
                self,
                Self::AcknowledgeNpcHint { .. }
                    | Self::BuyTeacherSpell { .. }
                    | Self::ReadLibrarianBook { .. }
                    | Self::SelectBoast { .. }
            )
    }
}

/// The wizard's current and maximum health and mana. Early Unforge attempts reroll a Full
/// Rejuvenation when both are already full.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnforgeVitals {
    pub current_health: f64,
    pub current_mana: f64,
    pub maximum_health: f64,
    pub maximum_mana: f64,
}

/// Values owned by progression/combat, sampled once for an economy command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubEconomyContext {
    pub player_level: u32,
    pub creativity_rank: u32,
    pub advanced_unlocks: [bool; 8],
    pub vitals: UnforgeVitals,
}

impl Default for HubEconomyContext {
    fn default() -> Self {
        Self {
            player_level: 1,
            creativity_rank: 0,
            advanced_unlocks: [false; 8],
            vitals: UnforgeVitals {
                current_health: 100.0,
                current_mana: 100.0,
                maximum_health: 100.0,
                maximum_mana: 100.0,
            },
        }
    }
}

impl HubEconomyState {
    /// A fresh profile's economy. The starter appearance and the shop stream each start from
    /// `seed`, independently; appearance draws do not advance the shop stream.
    #[must_use]
    pub fn new(seed: i64, element: WizardElement) -> Self {
        let appearance = roll_starter_equipment_appearance(&mut Rng::new(seed), element.into());
        let (backpack, equipment, next_item_id) = inventory::starter_loadout(1, appearance);
        let mut state = Self {
            action_feedback: None,
            backpack,
            charm_capacity: 3,
            college_intro_pending: true,
            dowsing_fee: 650,
            dowsing_offers: Vec::new(),
            dowsing_rolled: false,
            equipment,
            first_mixed_selectors: Vec::new(),
            fomentius_stock: Vec::new(),
            gold: FRESH_PROFILE_GOLD,
            hagatha_bundle_selectors: Vec::new(),
            next_item_id,
            next_offer_id: 1,
            npc: NativeHubNpcState::default(),
            owned_perk_selectors: Vec::new(),
            revision: 0,
            rng: Rng::new(seed),
            storage: Vec::new(),
            tonic_purchases: 0,
            tutorial_pending: true,
            unforge_bonuses: UnforgeBonuses::default(),
        };
        state.roll_fomentius_stock();
        state
    }

    /// A rejected transaction restores all items, ids and random words together.
    pub fn apply(
        &mut self,
        action: &HubInventoryAction,
        context: &HubEconomyContext,
    ) -> HubEconomyResult {
        self.apply_with_extensions(action, context, None)
    }

    pub fn apply_with_extensions(
        &mut self,
        action: &HubInventoryAction,
        context: &HubEconomyContext,
        extensions: Option<&dyn crate::modding::extensions::GameSimulationExtensions>,
    ) -> HubEconomyResult {
        self.apply_with_dye_session(action, context, extensions, None)
    }

    pub fn apply_with_dye_session(
        &mut self,
        action: &HubInventoryAction,
        context: &HubEconomyContext,
        extensions: Option<&dyn crate::modding::extensions::GameSimulationExtensions>,
        dye_session: Option<&InventoryDyeSession>,
    ) -> HubEconomyResult {
        let original = self.clone();
        let result = self.apply_command(action, context, extensions, dye_session);
        if !result.accepted {
            *self = original;
        }
        result
    }
}
