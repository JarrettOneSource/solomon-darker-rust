//! Native loot selection, materialization and item identity.
//!
//! An enemy's private stream selects a candidate category; constructing the candidate advances
//! the shared loot stream. `loot-goldens.json` pins drops, item trees and both RNG streams.

pub mod items;
mod materialize;
mod selection;

pub use items::dire_boss_item_recipe_pool;
pub use materialize::{materialize_loot_script_action, resolve_loot_placement, roll_enemy_loot};
pub use selection::select_loot_candidate;

use crate::economy::HubInventoryItem;
use crate::float::fround;
use crate::{Rng, Vec2};

pub const LOOT_ACTOR_SEED_BOUND: u32 = 10_000_000;
pub const LOOT_CARRIER_PLACEMENT_RADIUS: f64 = 15.0;
pub const LOOT_PLACEMENT_VERTICAL_SCALE: f64 = 0.8_f32 as f64;

/// A category of enemy drop: key, orb, gold, item, potion or powerup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LootCategory {
    Key,
    Orb,
    Gold,
    Item,
    Potion,
    Powerup,
}

/// A drop policy for one loot category. `GoldScatter` also adds a scattered Gold drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LootPolicy {
    Ordinary,
    Reduced,
    Increased,
    Forced,
    Disabled,
    GoldScatter,
}

impl LootPolicy {
    #[must_use]
    pub fn from_id(id: u32) -> Option<Self> {
        Some(match id {
            0 => Self::Ordinary,
            1 => Self::Reduced,
            2 => Self::Increased,
            3 => Self::Forced,
            4 => Self::Disabled,
            5 => Self::GoldScatter,
            _ => return None,
        })
    }
}

/// Whether an orb restores health or mana.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum OrbKind {
    Health,
    Mana,
}

/// A Bonus drop's reward: a skill choice, a skill rank or quadruple damage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BonusKind {
    SkillChoice,
    SkillRank,
    DamageX4,
}

impl BonusKind {
    #[must_use]
    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            0 => Some(Self::SkillChoice),
            1 => Some(Self::SkillRank),
            2 => Some(Self::DamageX4),
            _ => None,
        }
    }
}

/// A player's derived gold amount, orb pull and pickup range factors, before perks apply.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LootAttractionModifiers {
    pub gold_amount: f64,
    pub orb_pull: f64,
    pub pickup_factor: f64,
}

impl Default for LootAttractionModifiers {
    fn default() -> Self {
        Self { gold_amount: 1.0, orb_pull: 1.0, pickup_factor: 1.25 }
    }
}

/// A player's loot factors after perks: drop chances, gold amount, orb pull and value, and
/// pickup range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LootModifiers {
    pub gold_amount: f64,
    pub gold_chance: f64,
    pub item_chance: f64,
    pub orb_chance: f64,
    pub orb_pull: f64,
    pub orb_value_bonus: bool,
    pub pickup_factor: f64,
    pub powerup_chance: f64,
}

impl Default for LootModifiers {
    fn default() -> Self {
        Self {
            gold_amount: 1.0,
            gold_chance: 1.0,
            item_chance: 1.0,
            orb_chance: 1.0,
            orb_pull: 1.0,
            orb_value_bonus: false,
            pickup_factor: 1.25,
            powerup_chance: 1.0,
        }
    }
}

#[must_use]
pub fn loot_modifiers(
    owned_perk_selectors: &[u32],
    attraction: LootAttractionModifiers,
) -> LootModifiers {
    for value in [attraction.gold_amount, attraction.orb_pull, attraction.pickup_factor] {
        assert!(
            value.is_finite() && value >= 0.0,
            "loot attraction factors are finite and non-negative"
        );
    }
    let owns = |selector| owned_perk_selectors.contains(&selector);
    LootModifiers {
        gold_amount: fround(attraction.gold_amount * if owns(4) { 1.25 } else { 1.0 }),
        gold_chance: if owns(4) { 0.75 } else { 1.0 },
        item_chance: if owns(3) { 0.75 } else { 1.0 },
        orb_chance: if owns(9) { 0.5 } else { 1.0 },
        orb_pull: fround(attraction.orb_pull),
        orb_value_bonus: owns(9),
        pickup_factor: fround(attraction.pickup_factor),
        powerup_chance: if owns(23) { 0.8_f32 as f64 } else { 1.0 },
    }
}

/// The drop policy for each loot category, usually from an enemy's recipe, and the
/// specific-item mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LootPolicies {
    pub gold: LootPolicy,
    pub item: LootPolicy,
    pub orb: LootPolicy,
    pub potion: LootPolicy,
    pub powerup: LootPolicy,
    pub specific_item: u32,
}

impl LootPolicies {
    pub const DISABLED: Self = Self {
        gold: LootPolicy::Disabled,
        item: LootPolicy::Disabled,
        orb: LootPolicy::Disabled,
        potion: LootPolicy::Disabled,
        powerup: LootPolicy::Disabled,
        specific_item: 0,
    };
}

/// The arena's loot state: disable mask, item level range, last successful item level, level,
/// mode and special suppression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LootArenaInput {
    pub disable_mask: u32,
    pub item_level_maximum: i32,
    pub item_level_minimum: i32,
    pub last_successful_item_level: i32,
    pub level: i32,
    pub mode: i32,
    pub special_suppression: bool,
}

/// The player a drop is rolled for: level, loot modifiers, owned recipes, advanced unlocks and
/// slot.
#[derive(Clone, Copy, Debug)]
pub struct LootParticipantInput<'a> {
    pub advanced_unlocks: &'a [bool; 8],
    pub level: u32,
    pub modifiers: LootModifiers,
    pub owned_recipe_indexes: &'a [usize],
    pub slot: usize,
}

/// Key drop state: the level at which the next key drops, the current level and the keys left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LootKeyInput {
    pub current: u32,
    pub level: u32,
    pub remaining: usize,
}

/// Answers whether a loot carrier of a given radius fits at a position.
pub trait LootPlacement {
    fn can_place(&self, position: Vec2, radius: f64) -> bool;
}

impl<F: Fn(Vec2, f64) -> bool> LootPlacement for F {
    fn can_place(&self, position: Vec2, radius: f64) -> bool {
        self(position, radius)
    }
}

/// Placement that accepts every position, for fixtures and authored scripts that request it.
pub struct LootOpenPlacement;

impl LootPlacement for LootOpenPlacement {
    fn can_place(&self, _: Vec2, _: f64) -> bool {
        true
    }
}

/// The ground loot actor a drop creates: a Bonus, Gold, an Orb or a Sack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum LootDropKind {
    Bonus,
    Gold,
    Orb,
    Sack,
}

/// What produced a drop: an enemy, a Goodie or a script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum LootDropSource {
    Enemy,
    Goodie,
    Script,
}

/// A ground loot actor to create: its kind, position, contents, appearance and activation delay.
#[derive(Clone, Debug, PartialEq)]
pub struct LootDropSpec {
    pub activation_delay_ticks: i64,
    pub amount: Option<u64>,
    pub bonus_kind: Option<BonusKind>,
    pub id: u64,
    pub item: Option<HubInventoryItem>,
    pub kind: LootDropKind,
    pub type_id: u32,
    pub orb_kind: Option<OrbKind>,
    pub phase: f64,
    pub position: Vec2,
    pub rotation_deg: Option<f64>,
    pub scatter_seed: Option<u32>,
    pub source: LootDropSource,
    pub tier: Option<u32>,
    pub value: Option<f64>,
}

impl LootDropSpec {
    #[must_use]
    pub fn new(kind: LootDropKind, id: u64, position: Vec2, source: LootDropSource) -> Self {
        let type_id = match kind {
            LootDropKind::Bonus => 2038,
            LootDropKind::Gold => 2012,
            LootDropKind::Orb => 2011,
            LootDropKind::Sack => 2013,
        };
        Self {
            activation_delay_ticks: 0,
            amount: None,
            bonus_kind: None,
            id,
            item: None,
            kind,
            type_id,
            orb_kind: None,
            phase: 0.0,
            position,
            rotation_deg: None,
            scatter_seed: None,
            source,
            tier: None,
            value: None,
        }
    }
}

/// Everything an enemy loot roll reads: the enemy's seed and position, the arena, the player,
/// drop policies, placement and world counts.
#[derive(Clone, Copy)]
pub struct LootSelectionInput<'a> {
    pub actor_seed: u32,
    pub ground_recipe_indexes: &'a [usize],
    /// Approved enemy Item identity override; category rolls and scripts remain stock.
    pub item_recipe_pool: Option<&'a [usize]>,
    pub arena: LootArenaInput,
    pub explicit_gold_amount: Option<u64>,
    pub drop_delay_context: i64,
    pub inventory_has_health_potion: bool,
    pub key: LootKeyInput,
    pub nearby_mask_two_count: usize,
    pub participant: LootParticipantInput<'a>,
    pub placement: &'a dyn LootPlacement,
    pub policies: LootPolicies,
    pub scene_forces_health_potion: bool,
    pub source_position: Vec2,
    pub world_badguy_count: usize,
    pub world_has_health_potion_sack: bool,
}

/// The drops a roll created, with the updated last item level and next key drop level.
#[derive(Clone, Debug, PartialEq)]
pub struct LootMaterialization {
    pub drops: Vec<LootDropSpec>,
    pub last_successful_item_level: i32,
    pub next_key_drop_level: u32,
}

impl LootMaterialization {
    fn empty(input: &LootSelectionInput<'_>) -> Self {
        Self {
            drops: Vec::new(),
            last_successful_item_level: input.arena.last_successful_item_level,
            next_key_drop_level: input.key.current,
        }
    }
}

/// An enemy loot roll's result: its drops, the selected category and whether the emergency
/// potion was attempted.
#[derive(Clone, Debug, PartialEq)]
pub struct LootSelection {
    pub materialization: LootMaterialization,
    pub emergency_potion_attempted: bool,
    pub selected_category: Option<LootCategory>,
}

/// A scripted loot drop: gold, a named item, a key, a potion, random gold or a random item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LootScriptAction {
    DropGold { amount: i64 },
    DropItem { recipe_index: usize },
    DropKey,
    DropPotion { subtype: u32 },
    DropRandomGold { minimum: i32, maximum: i32 },
    DropRandomItem { mode: u32 },
}

/// Allocates loot item IDs in increasing order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LootItemIds {
    pub next_id: u64,
}

impl LootItemIds {
    #[must_use]
    pub fn new(first: u64) -> Self {
        assert!(first > 0, "native loot item ids start at a positive integer");
        Self { next_id: first }
    }
    pub fn allocate(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[must_use]
pub fn gold_tier(amount: u64) -> u32 {
    assert!(amount > 0, "a native Gold actor has a positive amount");
    if amount < 3 {
        0
    } else if amount < 5 {
        1
    } else if amount < 8 {
        2
    } else {
        3
    }
}

pub fn initial_key_drop_level(rng: &mut Rng) -> u32 {
    rng.draw_integer(8) as u32 + 5
}

pub fn advance_key_drop_level(rng: &mut Rng, current_level: u32) -> u32 {
    if current_level < 13 {
        rng.draw_integer(11) as u32 + 15
    } else if current_level < 26 {
        rng.draw_integer(11) as u32 + 30
    } else if current_level <= 40 {
        rng.draw_integer(21) as u32 + 50
    } else {
        current_level
    }
}

/// `enable_operand == 0` clears the supplied disable bits; every other operand sets them.
#[must_use]
pub fn loot_disable_mask(current: u32, enable_operand: i32, mask: u32) -> u32 {
    let disabled = current | mask;
    if enable_operand == 0 { disabled ^ mask } else { disabled }
}

/// An arena's item level range for drops, and its mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LootArenaDropLimits {
    pub item_level_maximum: i32,
    pub item_level_minimum: i32,
    pub mode: i32,
}

#[must_use]
pub fn loot_arena_drop_limits(
    mode: i32,
    range_mode: u32,
    first: i32,
    second: i32,
) -> LootArenaDropLimits {
    match range_mode {
        0 => LootArenaDropLimits { item_level_maximum: 9999, item_level_minimum: -9999, mode },
        1 => LootArenaDropLimits { item_level_maximum: first, item_level_minimum: first, mode },
        2 => LootArenaDropLimits { item_level_maximum: second, item_level_minimum: first, mode },
        _ => panic!("native item range mode is in0..3"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perk_factors_and_arena_masks_preserve_native_meaning() {
        let modifiers = loot_modifiers(&[3, 4, 9, 23], LootAttractionModifiers::default());
        assert_eq!(modifiers.gold_amount, 1.25);
        assert_eq!(modifiers.item_chance, 0.75);
        assert_eq!(modifiers.orb_chance, 0.5);
        assert_eq!(modifiers.powerup_chance, 0.8_f32 as f64);
        assert!(modifiers.orb_value_bonus);
        assert_eq!([1, 2, 3, 4, 5, 7, 8, 25].map(gold_tier), [0, 0, 1, 1, 2, 2, 3, 3]);
        assert_eq!(loot_disable_mask(0b1010, 0, 0b0110), 0b1000);
        assert_eq!(loot_disable_mask(0b1010, 1, 0b0110), 0b1110);
        assert_eq!(
            loot_arena_drop_limits(1, 2, 5, 12),
            LootArenaDropLimits { item_level_maximum: 12, item_level_minimum: 5, mode: 1 }
        );
    }
}
