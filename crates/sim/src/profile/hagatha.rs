//! Purchased Hagatha effects from Website `core-kernels/native-hagatha-effects.ts`.
//! Runtime charges and until-hurt flags remain in the canonical progression state.

use crate::float::fround;

use super::NativeHagathaRuntimeState;

pub const NATIVE_HAGATHA_LAST_WORD_DEATH_TICK: u64 = 200;

/// Native constants have already been narrowed to float32 before any multiplication.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHagathaFactors {
    pub bare_hands_damage: f64,
    pub bare_hands_mana: f64,
    pub brute_melee: f64,
    pub brute_push: f64,
    pub cheat_death_recovery: f64,
    pub curing_poison: f64,
    pub focus_recharge: f64,
    pub glass_cannon: f64,
    pub life: f64,
    pub mana: f64,
    pub serendipity_damage: f64,
    pub speed: f64,
    pub war_mana: f64,
}

pub const NATIVE_HAGATHA_FACTORS: NativeHagathaFactors = NativeHagathaFactors {
    bare_hands_damage: 1.15_f32 as f64,
    bare_hands_mana: 0.85_f32 as f64,
    brute_melee: 3.0,
    brute_push: 2.0,
    cheat_death_recovery: 0.5,
    curing_poison: 0.5,
    focus_recharge: 1.25,
    glass_cannon: 2.0,
    life: 1.25,
    mana: 1.25,
    serendipity_damage: 3.0,
    speed: 1.1_f32 as f64,
    war_mana: 0.75,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHagathaDerivedModifiers {
    pub cast_speed_factor: f64,
    pub incoming_damage_factor: f64,
    pub melee_damage_factor: f64,
    pub movement_factor: f64,
    pub offensive_mana_factor: f64,
    pub poison_damage_factor: f64,
    pub push_strength_factor: f64,
    pub recharge_factor: f64,
    pub spell_damage_factor: f64,
}

#[must_use]
pub fn native_hagatha_derived_modifiers(
    owned_selectors: &[u32],
    runtime: NativeHagathaRuntimeState,
    weapon_equipped: bool,
) -> NativeHagathaDerivedModifiers {
    let factors = NATIVE_HAGATHA_FACTORS;
    let glass = owned_selectors.contains(&16);
    let bare_hands = !weapon_equipped && owned_selectors.contains(&20);
    let speed = if owned_selectors.contains(&2) { factors.speed } else { 1.0 };
    let glass_damage = if glass { factors.glass_cannon } else { 1.0 };
    let melee_damage = if owned_selectors.contains(&26) { factors.brute_melee } else { 1.0 };
    let war_mana = if owned_selectors.contains(&10) { factors.war_mana } else { 1.0 };
    let bare_hands_mana = if bare_hands { factors.bare_hands_mana } else { 1.0 };
    let bare_hands_damage = if bare_hands { factors.bare_hands_damage } else { 1.0 };
    let until_hurt_damage =
        if runtime.serendipity_active { factors.serendipity_damage } else { 1.0 };
    let until_hurt_mana = if runtime.reverie_active { 0.0 } else { 1.0 };

    // Keep the source multiplication order, especially the float32 store after combining
    // Bare Hands, Glass Cannon and the two temporary until-hurt effects.
    NativeHagathaDerivedModifiers {
        cast_speed_factor: speed,
        incoming_damage_factor: glass_damage,
        melee_damage_factor: fround(glass_damage * melee_damage),
        movement_factor: speed,
        offensive_mana_factor: fround(until_hurt_mana * war_mana * bare_hands_mana),
        poison_damage_factor: if owned_selectors.contains(&11) {
            factors.curing_poison
        } else {
            1.0
        },
        push_strength_factor: if owned_selectors.contains(&26) { factors.brute_push } else { 1.0 },
        recharge_factor: if owned_selectors.contains(&18) { factors.focus_recharge } else { 1.0 },
        spell_damage_factor: fround(until_hurt_damage * bare_hands_damage * glass_damage),
    }
}

#[must_use]
pub fn native_hagatha_revelation_rank(rank: u32, owned_selectors: &[u32]) -> u32 {
    if owned_selectors.contains(&6) { rank.max(2) } else { rank }
}

#[must_use]
pub fn native_hagatha_boss_damage_factor(owned_selectors: &[u32], native_type_id: u32) -> f64 {
    if owned_selectors.contains(&22) && (1008..=1011).contains(&native_type_id) { 3.0 } else { 1.0 }
}

#[must_use]
pub fn native_hagatha_drinker_should_use_health_potion(
    owned_selectors: &[u32],
    current_health: f64,
) -> bool {
    owned_selectors.contains(&15) && current_health <= -10.0
}

#[must_use]
pub fn native_hagatha_drinker_should_use_mana_potion(
    owned_selectors: &[u32],
    current_mana: f64,
    maximum_mana: f64,
    cost: f64,
) -> bool {
    if !current_mana.is_finite() || !maximum_mana.is_finite() || !cost.is_finite() {
        return false;
    }
    owned_selectors.contains(&15) && cost > current_mana && cost < maximum_mana
}
