//! Offensive damage and mana lanes from `core-kernels/native-offensive-resolution.ts`.
//!
//! The order matters: Siege Mage scales the completed damage product, while Battle Mage
//! scales mana before flat equipment costs. `primary-profile-goldens.json` pins both.

use crate::equipment::NativeEquipmentModifiers;

use super::native_skill_root;

pub const NATIVE_OFFENSIVE_SKILL_IDS: &[usize] = &[
    8, 9, 10, 11, 13, 16, 17, 18, 19, 20, 21, 23, 24, 25, 26, 27, 29, 31, 32, 33, 34, 35, 38, 40,
    41, 42, 43, 44, 47, 50, 52, 55, 65, 72, 73, 74, 76, 77, 78, 79,
];

/// The player's offensive bonuses at the instant a spell is cast.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeOffensiveSpellFactors {
    pub damage: f64,
    pub mana_cost: f64,
    pub global_flat_damage: f64,
    pub global_mana_reduction: f64,
    pub equipment: Option<NativeEquipmentModifiers>,
}

impl Default for NativeOffensiveSpellFactors {
    fn default() -> Self {
        Self {
            damage: 1.0,
            mana_cost: 1.0,
            global_flat_damage: 0.0,
            global_mana_reduction: 0.0,
            equipment: None,
        }
    }
}

impl NativeOffensiveSpellFactors {
    pub fn validate(&self) {
        for (name, value) in [
            ("offensive damage factor", self.damage),
            ("offensive mana factor", self.mana_cost),
            ("global flat damage", self.global_flat_damage),
            ("global mana reduction", self.global_mana_reduction),
        ] {
            assert!(value.is_finite() && value >= 0.0, "{name} must be finite and non-negative");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeDamageResolutionLanes {
    pub actor_base_damage: f64,
    pub base_damage: f64,
    pub global_flat_damage: f64,
    pub skill_flat_damage: f64,
    pub class_flat_damage: f64,
    pub global_multiplier: f64,
    pub skill_multiplier: f64,
    pub class_multiplier: f64,
    pub element_or_class_multiplier: f64,
}

impl Default for NativeDamageResolutionLanes {
    fn default() -> Self {
        Self {
            actor_base_damage: 0.0,
            base_damage: 0.0,
            global_flat_damage: 0.0,
            skill_flat_damage: 0.0,
            class_flat_damage: 0.0,
            global_multiplier: 1.0,
            skill_multiplier: 1.0,
            class_multiplier: 1.0,
            element_or_class_multiplier: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeManaCostResolutionLanes {
    pub base_mana_cost: f64,
    pub global_mana_reduction: f64,
    pub global_flat_mana_cost: f64,
    pub class_flat_mana_cost: f64,
    pub skill_flat_mana_cost: f64,
    pub global_multiplier: f64,
    pub class_multiplier: f64,
    pub skill_multiplier: f64,
    pub element_multiplier: f64,
}

impl Default for NativeManaCostResolutionLanes {
    fn default() -> Self {
        Self {
            base_mana_cost: 0.0,
            global_mana_reduction: 0.0,
            global_flat_mana_cost: 0.0,
            class_flat_mana_cost: 0.0,
            skill_flat_mana_cost: 0.0,
            global_multiplier: 1.0,
            class_multiplier: 1.0,
            skill_multiplier: 1.0,
            element_multiplier: 1.0,
        }
    }
}

#[must_use]
pub fn native_skill_is_offensive(skill_id: usize) -> bool {
    NATIVE_OFFENSIVE_SKILL_IDS.contains(&skill_id)
}

/// Native `0x0065FFF0`, including the final offensive-only Siege Mage multiplier.
#[must_use]
pub fn resolve_native_skill_damage(
    skill_id: usize,
    factors: &NativeOffensiveSpellFactors,
    lanes: NativeDamageResolutionLanes,
) -> f64 {
    factors.validate();
    for value in [
        lanes.actor_base_damage,
        lanes.base_damage,
        lanes.global_flat_damage,
        lanes.skill_flat_damage,
        lanes.class_flat_damage,
        lanes.global_multiplier,
        lanes.skill_multiplier,
        lanes.class_multiplier,
        lanes.element_or_class_multiplier,
    ] {
        assert!(value.is_finite(), "native damage lane must be finite");
    }
    let class_id = native_skill_root(skill_id).expect("damage skill has a native class");
    let identity = NativeEquipmentModifiers::default();
    let equipment = factors.equipment.as_ref().unwrap_or(&identity);
    let offensive = native_skill_is_offensive(skill_id);
    let unforge_damage = if offensive { factors.global_flat_damage } else { 0.0 };
    let damage = (lanes.actor_base_damage
        + lanes.base_damage
        + lanes.global_flat_damage
        + unforge_damage
        + lanes.skill_flat_damage
        + lanes.class_flat_damage
        + equipment.global_damage_flat
        + equipment.skill_damage_flat[skill_id]
        + equipment.class_damage_flat[class_id])
        * lanes.global_multiplier
        * lanes.skill_multiplier
        * lanes.class_multiplier
        * lanes.element_or_class_multiplier
        * equipment.global_damage_multiplier
        * equipment.skill_damage_multiplier[skill_id]
        * equipment.class_damage_multiplier[class_id];
    let damage = if offensive { damage * factors.damage } else { damage };
    damage.max(0.0)
}

/// Native `0x006600F0`: the minimum base cost precedes Battle Mage and later flat lanes.
#[must_use]
pub fn resolve_native_skill_mana_cost(
    skill_id: usize,
    factors: &NativeOffensiveSpellFactors,
    lanes: NativeManaCostResolutionLanes,
) -> f64 {
    factors.validate();
    for value in [
        lanes.base_mana_cost,
        lanes.global_mana_reduction,
        lanes.global_flat_mana_cost,
        lanes.class_flat_mana_cost,
        lanes.skill_flat_mana_cost,
        lanes.global_multiplier,
        lanes.class_multiplier,
        lanes.skill_multiplier,
        lanes.element_multiplier,
    ] {
        assert!(value.is_finite(), "native mana lane must be finite");
    }
    let class_id = native_skill_root(skill_id).expect("mana-cost skill has a native class");
    let identity = NativeEquipmentModifiers::default();
    let equipment = factors.equipment.as_ref().unwrap_or(&identity);
    let mut cost =
        (lanes.base_mana_cost - lanes.global_mana_reduction - factors.global_mana_reduction)
            .max(1.0);
    if native_skill_is_offensive(skill_id) {
        cost *= factors.mana_cost;
    }
    cost = (cost
        + lanes.global_flat_mana_cost
        + lanes.class_flat_mana_cost
        + lanes.skill_flat_mana_cost
        + equipment.global_mana_cost_flat
        + equipment.class_mana_cost_flat[class_id])
        * lanes.global_multiplier
        * lanes.class_multiplier
        * lanes.skill_multiplier
        * lanes.element_multiplier
        * equipment.global_mana_cost_multiplier
        * equipment.class_mana_cost_multiplier[class_id];
    cost.max(0.0)
}

#[must_use]
pub fn resolve_native_skill_damage_value(
    skill_id: usize,
    damage: f64,
    factors: &NativeOffensiveSpellFactors,
) -> f64 {
    resolve_native_skill_damage(
        skill_id,
        factors,
        NativeDamageResolutionLanes {
            base_damage: damage,
            ..NativeDamageResolutionLanes::default()
        },
    )
}

#[must_use]
pub fn resolve_native_skill_mana_cost_value(
    skill_id: usize,
    mana_cost: f64,
    factors: &NativeOffensiveSpellFactors,
) -> f64 {
    if mana_cost == 0.0 {
        factors.validate();
        return 0.0;
    }
    resolve_native_skill_mana_cost(
        skill_id,
        factors,
        NativeManaCostResolutionLanes {
            base_mana_cost: mana_cost,
            ..NativeManaCostResolutionLanes::default()
        },
    )
}
