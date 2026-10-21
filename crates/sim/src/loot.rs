//! Native loot selection, materialization and item identity.
//!
//! References: Website `core-kernels/native-loot.ts`, `native-loot-selection.ts` and
//! `native-loot-items.ts`. `loot-goldens.json` pins drops, item trees and both RNG streams.

pub mod items;
mod materialize;
mod selection;

pub use items::dire_boss_item_recipe_pool;
pub use materialize::{
    materialize_native_loot_script_action, resolve_native_loot_placement, roll_native_enemy_loot,
};
pub use selection::select_native_loot_candidate;

use crate::economy::HubInventoryItem;
use crate::float::fround;
use crate::{NativeRng, Vec2};

pub const NATIVE_LOOT_ACTOR_SEED_BOUND: u32 = 10_000_000;
pub const NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS: f64 = 15.0;
pub const NATIVE_LOOT_PLACEMENT_VERTICAL_SCALE: f64 = 0.8_f32 as f64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeLootCategory {
    Key,
    Orb,
    Gold,
    Item,
    Potion,
    Powerup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeLootPolicy {
    Ordinary,
    Reduced,
    Increased,
    Forced,
    Disabled,
    GoldScatter,
}

impl NativeLootPolicy {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeOrbKind {
    Health,
    Mana,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeBonusKind {
    SkillChoice,
    SkillRank,
    DamageX4,
}

impl NativeBonusKind {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeLootAttractionModifiers {
    pub gold_amount: f64,
    pub orb_pull: f64,
    pub pickup_factor: f64,
}

impl Default for NativeLootAttractionModifiers {
    fn default() -> Self {
        Self { gold_amount: 1.0, orb_pull: 1.0, pickup_factor: 1.25 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeLootModifiers {
    pub gold_amount: f64,
    pub gold_chance: f64,
    pub item_chance: f64,
    pub orb_chance: f64,
    pub orb_pull: f64,
    pub orb_value_bonus: bool,
    pub pickup_factor: f64,
    pub powerup_chance: f64,
}

impl Default for NativeLootModifiers {
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
pub fn native_loot_modifiers(
    owned_perk_selectors: &[u32],
    attraction: NativeLootAttractionModifiers,
) -> NativeLootModifiers {
    for value in [attraction.gold_amount, attraction.orb_pull, attraction.pickup_factor] {
        assert!(
            value.is_finite() && value >= 0.0,
            "loot attraction factors are finite and non-negative"
        );
    }
    let owns = |selector| owned_perk_selectors.contains(&selector);
    NativeLootModifiers {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeLootPolicies {
    pub gold: NativeLootPolicy,
    pub item: NativeLootPolicy,
    pub orb: NativeLootPolicy,
    pub potion: NativeLootPolicy,
    pub powerup: NativeLootPolicy,
    pub specific_item: u32,
}

impl NativeLootPolicies {
    pub const DISABLED: Self = Self {
        gold: NativeLootPolicy::Disabled,
        item: NativeLootPolicy::Disabled,
        orb: NativeLootPolicy::Disabled,
        potion: NativeLootPolicy::Disabled,
        powerup: NativeLootPolicy::Disabled,
        specific_item: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeLootArenaInput {
    pub disable_mask: u32,
    pub item_level_maximum: i32,
    pub item_level_minimum: i32,
    pub last_successful_item_level: i32,
    pub level: i32,
    pub mode: i32,
    pub special_suppression: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeLootParticipantInput<'a> {
    pub advanced_unlocks: &'a [bool; 8],
    pub level: u32,
    pub modifiers: NativeLootModifiers,
    pub owned_recipe_indexes: &'a [usize],
    pub slot: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeLootKeyInput {
    pub current: u32,
    pub level: u32,
    pub remaining: usize,
}

pub trait NativeLootPlacement {
    fn can_place(&self, position: Vec2, radius: f64) -> bool;
}

impl<F: Fn(Vec2, f64) -> bool> NativeLootPlacement for F {
    fn can_place(&self, position: Vec2, radius: f64) -> bool {
        self(position, radius)
    }
}

/// Explicitly open geometry for source fixtures and authored scripts that request it.
pub struct NativeLootOpenPlacement;

impl NativeLootPlacement for NativeLootOpenPlacement {
    fn can_place(&self, _: Vec2, _: f64) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeLootDropKind {
    Bonus,
    Gold,
    Orb,
    Sack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeLootDropSource {
    Enemy,
    Goodie,
    Script,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLootDropSpec {
    pub activation_delay_ticks: i64,
    pub amount: Option<u64>,
    pub bonus_kind: Option<NativeBonusKind>,
    pub id: u64,
    pub item: Option<HubInventoryItem>,
    pub kind: NativeLootDropKind,
    pub native_type_id: u32,
    pub orb_kind: Option<NativeOrbKind>,
    pub phase: f64,
    pub position: Vec2,
    pub rotation_deg: Option<f64>,
    pub scatter_seed: Option<u32>,
    pub source: NativeLootDropSource,
    pub tier: Option<u32>,
    pub value: Option<f64>,
}

impl NativeLootDropSpec {
    #[must_use]
    pub fn new(
        kind: NativeLootDropKind,
        id: u64,
        position: Vec2,
        source: NativeLootDropSource,
    ) -> Self {
        let native_type_id = match kind {
            NativeLootDropKind::Bonus => 2038,
            NativeLootDropKind::Gold => 2012,
            NativeLootDropKind::Orb => 2011,
            NativeLootDropKind::Sack => 2013,
        };
        Self {
            activation_delay_ticks: 0,
            amount: None,
            bonus_kind: None,
            id,
            item: None,
            kind,
            native_type_id,
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

#[derive(Clone, Copy)]
pub struct NativeLootSelectionInput<'a> {
    pub actor_seed: u32,
    pub ground_recipe_indexes: &'a [usize],
    /// Approved enemy Item identity override; category rolls and scripts remain stock.
    pub item_recipe_pool: Option<&'a [usize]>,
    pub arena: NativeLootArenaInput,
    pub explicit_gold_amount: Option<u64>,
    pub drop_delay_context: i64,
    pub inventory_has_health_potion: bool,
    pub key: NativeLootKeyInput,
    pub nearby_mask_two_count: usize,
    pub participant: NativeLootParticipantInput<'a>,
    pub placement: &'a dyn NativeLootPlacement,
    pub policies: NativeLootPolicies,
    pub scene_forces_health_potion: bool,
    pub source_position: Vec2,
    pub world_badguy_count: usize,
    pub world_has_health_potion_sack: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLootMaterialization {
    pub drops: Vec<NativeLootDropSpec>,
    pub last_successful_item_level: i32,
    pub next_key_drop_level: u32,
}

impl NativeLootMaterialization {
    fn empty(input: &NativeLootSelectionInput<'_>) -> Self {
        Self {
            drops: Vec::new(),
            last_successful_item_level: input.arena.last_successful_item_level,
            next_key_drop_level: input.key.current,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLootSelection {
    pub materialization: NativeLootMaterialization,
    pub emergency_potion_attempted: bool,
    pub selected_category: Option<NativeLootCategory>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeLootScriptAction {
    DropGold { amount: i64 },
    DropItem { recipe_index: usize },
    DropKey,
    DropPotion { subtype: u32 },
    DropRandomGold { minimum: i32, maximum: i32 },
    DropRandomItem { mode: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeLootItemIds {
    pub next_id: u64,
}

impl NativeLootItemIds {
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
pub fn native_gold_tier(amount: u64) -> u32 {
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

pub fn initial_native_key_drop_level(rng: &mut NativeRng) -> u32 {
    rng.draw_integer(8) as u32 + 5
}

pub fn advance_native_key_drop_level(rng: &mut NativeRng, current_level: u32) -> u32 {
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

/// `enableOperand == 0` clears the supplied disable bits; every other operand sets them.
#[must_use]
pub fn native_loot_disable_mask(current: u32, enable_operand: i32, mask: u32) -> u32 {
    let disabled = current | mask;
    if enable_operand == 0 { disabled ^ mask } else { disabled }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeLootArenaDropLimits {
    pub item_level_maximum: i32,
    pub item_level_minimum: i32,
    pub mode: i32,
}

#[must_use]
pub fn native_loot_arena_drop_limits(
    mode: i32,
    range_mode: u32,
    first: i32,
    second: i32,
) -> NativeLootArenaDropLimits {
    match range_mode {
        0 => {
            NativeLootArenaDropLimits { item_level_maximum: 9999, item_level_minimum: -9999, mode }
        }
        1 => {
            NativeLootArenaDropLimits { item_level_maximum: first, item_level_minimum: first, mode }
        }
        2 => NativeLootArenaDropLimits {
            item_level_maximum: second,
            item_level_minimum: first,
            mode,
        },
        _ => panic!("native item range mode is in0..3"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perk_factors_and_arena_masks_preserve_native_meaning() {
        let modifiers =
            native_loot_modifiers(&[3, 4, 9, 23], NativeLootAttractionModifiers::default());
        assert_eq!(modifiers.gold_amount, 1.25);
        assert_eq!(modifiers.item_chance, 0.75);
        assert_eq!(modifiers.orb_chance, 0.5);
        assert_eq!(modifiers.powerup_chance, 0.8_f32 as f64);
        assert!(modifiers.orb_value_bonus);
        assert_eq!([1, 2, 3, 4, 5, 7, 8, 25].map(native_gold_tier), [0, 0, 1, 1, 2, 2, 3, 3]);
        assert_eq!(native_loot_disable_mask(0b1010, 0, 0b0110), 0b1000);
        assert_eq!(native_loot_disable_mask(0b1010, 1, 0b0110), 0b1110);
        assert_eq!(
            native_loot_arena_drop_limits(1, 2, 5, 12),
            NativeLootArenaDropLimits { item_level_maximum: 12, item_level_minimum: 5, mode: 1 }
        );
    }
}
