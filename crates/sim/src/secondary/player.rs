//! Secondary player clocks, reserve and Focus recharge from native-secondary-abilities.ts.

use crate::NativeRng;
use crate::float::fround;
use crate::profile::offensive::NativeOffensiveSpellFactors;
use crate::profile::{NativeSkillBookState, native_skill_category, native_skill_root};
use crate::spells::casting::NativePlayerWeaponKind;

use super::{
    NativeSecondaryAbilityId, effective_secondary_ability_rank_stats, effective_skill_value,
    native_secondary_cooldown_capacity_ticks,
};

pub const NATIVE_SECONDARY_GLOBAL_COOLDOWN_TICKS: f64 = 150.0;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondaryCastAction {
    pub weapon_kind: Option<NativePlayerWeaponKind>,
    pub progress: f64,
}

#[must_use]
pub fn step_native_secondary_cast_action(
    action: Option<NativeSecondaryCastAction>,
    faster_caster_percent: f64,
) -> Option<NativeSecondaryCastAction> {
    let mut action = action?;
    let staff = action.weapon_kind == Some(NativePlayerWeaponKind::Staff);
    let rate = if staff { 0.1_f32 as f64 } else { 0.095_f32 as f64 };
    let factor = fround(1.0 + faster_caster_percent / 100.0);
    action.progress = fround(action.progress + factor * rate);
    let end = if staff { 5.0 } else { 6.0 };
    if action.progress > end { None } else { Some(action) }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct NativeSecondaryPlayerState {
    pub cast_action: Option<NativeSecondaryCastAction>,
    pub cast_sequence: u64,
    pub cast_spin_ticks_remaining: u64,
    #[cfg_attr(feature = "persistence", serde(with = "crate::persistence::fixed_array"))]
    pub cooldown_maximum_ticks_by_skill: [f64; 83],
    #[cfg_attr(feature = "persistence", serde(with = "crate::persistence::fixed_array"))]
    pub cooldown_ticks_by_skill: [f64; 83],
    pub firewalker: bool,
    pub fizzle_sequence: u64,
    pub global_cooldown_ticks: f64,
    pub held_slot: Option<usize>,
    pub last_skill_id: Option<NativeSecondaryAbilityId>,
    pub magic_shield_absorb: f64,
    pub magic_shield_explosion_damage: f64,
    pub magic_shield_maximum: f64,
    pub magic_shield_pulse_ticks: u64,
    pub mindstar: bool,
    pub plane_orb_held: bool,
    pub planewalker_ticks_remaining: u64,
    pub regenerate: bool,
    pub reserved_mana: f64,
    pub stoneskin_ticks_remaining: u64,
}

impl Default for NativeSecondaryPlayerState {
    fn default() -> Self {
        Self {
            cast_action: None,
            cast_sequence: 0,
            cast_spin_ticks_remaining: 0,
            cooldown_maximum_ticks_by_skill: [0.0; 83],
            cooldown_ticks_by_skill: [0.0; 83],
            firewalker: false,
            fizzle_sequence: 0,
            global_cooldown_ticks: 0.0,
            held_slot: None,
            last_skill_id: None,
            magic_shield_absorb: 0.0,
            magic_shield_explosion_damage: 0.0,
            magic_shield_maximum: 0.0,
            magic_shield_pulse_ticks: 0,
            mindstar: false,
            plane_orb_held: false,
            planewalker_ticks_remaining: 0,
            regenerate: false,
            reserved_mana: 0.0,
            stoneskin_ticks_remaining: 0,
        }
    }
}

impl NativeSecondaryPlayerState {
    pub fn start_cast(&mut self, weapon_kind: Option<NativePlayerWeaponKind>) {
        self.cast_action = Some(NativeSecondaryCastAction { weapon_kind, progress: 0.0 });
    }

    /// Fractional recharge factors stay in doubles; the source does not narrow these timers.
    pub fn step(
        &mut self,
        skill_book: &NativeSkillBookState,
        secondary_recharge_factor: f64,
        offensive_factors: &NativeOffensiveSpellFactors,
    ) {
        for ability in NativeSecondaryAbilityId::ALL {
            self.cooldown_maximum_ticks_by_skill[ability.skill_id()] =
                native_secondary_cooldown_capacity_ticks(skill_book, ability);
        }
        for skill_id in 0..self.cooldown_ticks_by_skill.len() {
            let ticks = self.cooldown_ticks_by_skill[skill_id];
            if ticks <= 0.0 {
                self.cooldown_ticks_by_skill[skill_id] = 0.0;
                continue;
            }
            let mut recharge = secondary_recharge_factor;
            if let Some(equipment) = &offensive_factors.equipment {
                let class_id =
                    native_skill_root(skill_id).expect("cooling secondary has a native class");
                recharge = recharge.max(equipment.class_recharge[class_id].apply(1.0));
            }
            self.cooldown_ticks_by_skill[skill_id] =
                (ticks.min(self.cooldown_maximum_ticks_by_skill[skill_id]) - recharge).max(0.0);
        }
        self.cast_spin_ticks_remaining = self.cast_spin_ticks_remaining.saturating_sub(1);
        self.global_cooldown_ticks =
            (self.global_cooldown_ticks - secondary_recharge_factor).max(0.0);
        self.magic_shield_pulse_ticks = self.magic_shield_pulse_ticks.saturating_sub(1);
        self.planewalker_ticks_remaining = self.planewalker_ticks_remaining.saturating_sub(1);
        self.stoneskin_ticks_remaining = self.stoneskin_ticks_remaining.saturating_sub(1);
        self.cast_action = step_native_secondary_cast_action(
            self.cast_action,
            effective_skill_value(skill_book, 70, "mValue"),
        );
    }

    /// Focus may leave the existing cooldown untouched after consuming its native chance draw.
    pub fn begin_cooldown(
        &mut self,
        ability: NativeSecondaryAbilityId,
        ticks: f64,
        focus_instant_recharge_chance_percent: f64,
        rng: &mut NativeRng,
    ) {
        self.cooldown_maximum_ticks_by_skill[ability.skill_id()] = ticks;
        if ticks > 0.0 && focus_instant_recharge_chance_percent > 0.0 {
            let draw = f64::from(rng.draw_integer(100));
            if draw >= 100.0 - focus_instant_recharge_chance_percent {
                return;
            }
        }
        self.cooldown_ticks_by_skill[ability.skill_id()] = ticks;
        for cooldown in &mut self.cooldown_ticks_by_skill {
            if *cooldown < NATIVE_SECONDARY_GLOBAL_COOLDOWN_TICKS {
                *cooldown = 0.0;
            }
        }
        self.global_cooldown_ticks = NATIVE_SECONDARY_GLOBAL_COOLDOWN_TICKS;
    }

    pub fn cooldown_rejuvenation(&mut self) {
        for (skill_id, ticks) in self.cooldown_ticks_by_skill.iter_mut().enumerate() {
            if native_skill_category(skill_id) == Some(2) {
                *ticks = 0.0;
            }
        }
        self.global_cooldown_ticks = 0.0;
    }

    #[must_use]
    pub fn mana_reserve(&self, skill_book: &NativeSkillBookState, maximum_mana: f64) -> f64 {
        let mut reserved_mana = 0.0;
        if self.firewalker {
            let stats = effective_secondary_ability_rank_stats(
                skill_book,
                NativeSecondaryAbilityId::Firewalker,
            );
            reserved_mana = stats.value("mHoard").unwrap_or(0.0);
        }
        for (active, ability) in [
            (self.mindstar, NativeSecondaryAbilityId::Mindstar),
            (self.regenerate, NativeSecondaryAbilityId::Regenerate),
        ] {
            if active {
                let stats = effective_secondary_ability_rank_stats(skill_book, ability);
                reserved_mana += maximum_mana * stats.value("mHoard").unwrap_or(0.0) / 100.0;
            }
        }
        reserved_mana
    }

    #[must_use]
    pub fn mana_ceiling(&self, maximum_mana: f64) -> f64 {
        (maximum_mana - self.reserved_mana).max(0.0)
    }

    pub fn clear_toggles(&mut self) {
        self.firewalker = false;
        self.mindstar = false;
        self.regenerate = false;
        self.reserved_mana = 0.0;
    }
}

#[must_use]
pub fn native_plane_orb_damage(skill_book: &NativeSkillBookState) -> f64 {
    let mut total = 0;
    for skill_id in [8, 10, 9, 13, 14, 15, 12] {
        total += skill_book.effective_ranks[skill_id];
    }
    2.0 * f64::from(total) / 100.0
}
