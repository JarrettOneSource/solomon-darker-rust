//! Canonical concentrations, equipment refresh and derived player stats from Website
//! `core-kernels/player-skill-runtime.ts`. Resource state remains in PlayerCombatState.

mod defenses;
pub use crate::hurricane::{NativeHurricaneChargeTick, native_hurricane_charge_tick};
pub use defenses::{
    NativeHardenChannel, NativeHardenState, interpolate_native_harden_coating,
    native_harden_break_angle_step,
};

use super::offensive::{NativeOffensiveSpellFactors, resolve_native_skill_damage_value};
use super::{
    NATIVE_HAGATHA_FACTORS, NATIVE_SKILL_CATALOG, NATIVE_WELD_BUILDS, NativeSkillBookState,
    PlayerProgressionState, PlayerSkillOffer, PlayerStatBookComponent, WizardElement,
    native_hagatha_derived_modifiers, native_skill_category, native_skill_root, native_weld_build,
};
use crate::NativeRng;
use crate::combat::{
    PLAYER_HEALTH_RECOVERY_PER_TICK, PLAYER_INITIAL_HEALTH, PLAYER_INITIAL_MANA,
    PLAYER_MANA_RECOVERY_PER_TICK, PlayerCombatState,
};
use crate::constants::TICK_RATE_HZ;
use crate::economy::{EquipmentType, HubEconomyState};
use crate::equipment::{NativeEquipmentModifiers, resolve_equipped_native_effects};
use crate::float::fround;

pub const CONCENTRATABLE_SKILL_IDS: [usize; 14] =
    [57, 58, 59, 60, 61, 62, 63, 65, 66, 67, 68, 69, 70, 71];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PlayerConcentrationSlot {
    A,
    B,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerSkillRuntimeState {
    pub concentration_skill_id_a: Option<usize>,
    pub concentration_skill_id_b: Option<usize>,
    pub equipment_modifiers: NativeEquipmentModifiers,
    pub harden: NativeHardenState,
    pub harden_armor_maximum: f64,
    pub harden_armor_per_tick: f64,
    pub hurricane_charge: f64,
    pub hurricane_enabled: bool,
    pub hurricane_refreshed: bool,
    pub meditation_activity_ramp_ticks: u64,
    pub meditation_idle_elapsed_ticks: u64,
    pub mindstar_active: bool,
    pub next_concentration_replacement_slot: PlayerConcentrationSlot,
    pub staff_melee_alternate: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerSkillDerivedStats {
    pub cast_progress_factor: f64,
    pub damage_resistance: f64,
    pub deflect_chance_percent: f64,
    pub experience_bonus: f64,
    pub flash_chance_percent: f64,
    pub flash_duration_ticks: u64,
    pub focus_instant_recharge_chance_percent: f64,
    pub flailing_chance_percent: f64,
    pub gold_amount_multiplier: f64,
    pub health_recovery_per_tick: f64,
    pub incoming_damage_factor: f64,
    pub magic_resistance: f64,
    pub mana_recovery_per_tick: f64,
    pub mana_recovery_per_second: f64,
    pub maximum_health: f64,
    pub maximum_mana: f64,
    pub meditation_concentrated: bool,
    pub meditation_idle_delay_ticks: i64,
    pub meditation_recovery_multiplier: f64,
    pub melee_damage_maximum: f64,
    pub melee_damage_minimum: f64,
    pub movement_factor: f64,
    pub offensive_damage_factor: f64,
    pub offensive_damage_flat: f64,
    pub offensive_mana_cost_factor: f64,
    pub offensive_mana_cost_reduction: f64,
    pub orb_pull_multiplier: f64,
    pub pickup_range_scalar: f64,
    pub poison_resistance: f64,
    pub poison_damage_factor: f64,
    pub push_strength_factor: f64,
    pub secondary_recharge_factor: f64,
    pub staff_action_timing_factor: f64,
    pub staff_damage_primary: f64,
    pub staff_damage_secondary: f64,
    pub staff_equipped: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerSkillActivity {
    pub acting: bool,
    pub moving: bool,
    pub primary_channel: Option<WizardElement>,
    pub primary_active: bool,
    pub primary_underpowered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerSkillRuntimeTickResult {
    pub base_mana_recovery_per_tick: f64,
    pub meditation_mana_recovery_per_tick: f64,
}

impl PlayerSkillRuntimeState {
    #[must_use]
    pub fn new(book: &mut NativeSkillBookState, economy: &HubEconomyState) -> Self {
        Self::new_with_stat_book(book, &PlayerStatBookComponent::default(), economy)
    }

    #[must_use]
    pub fn new_with_stat_book(
        book: &mut NativeSkillBookState,
        stat_book: &PlayerStatBookComponent,
        economy: &HubEconomyState,
    ) -> Self {
        let mut state = Self {
            concentration_skill_id_a: None,
            concentration_skill_id_b: None,
            equipment_modifiers: NativeEquipmentModifiers::default(),
            harden: NativeHardenState::default(),
            harden_armor_maximum: 0.0,
            harden_armor_per_tick: 0.0,
            hurricane_charge: 0.0,
            hurricane_enabled: false,
            hurricane_refreshed: false,
            meditation_activity_ramp_ticks: 0,
            meditation_idle_elapsed_ticks: 0,
            mindstar_active: false,
            next_concentration_replacement_slot: PlayerConcentrationSlot::A,
            staff_melee_alternate: false,
        };
        state.refresh_with_stat_book(book, stat_book, economy);
        state
    }

    pub fn refresh(&mut self, book: &mut NativeSkillBookState, economy: &HubEconomyState) {
        self.refresh_with_stat_book(book, &PlayerStatBookComponent::default(), economy);
    }

    pub fn refresh_with_stat_book(
        &mut self,
        book: &mut NativeSkillBookState,
        stat_book: &PlayerStatBookComponent,
        economy: &HubEconomyState,
    ) {
        let equipment = resolve_equipped_native_effects(
            &book.permanent_ranks,
            &economy.equipment,
            &economy.owned_perk_selectors,
        );
        let mut ranks = equipment.effective_ranks;
        if equipment.modifiers.feature_bits & 0x0800 != 0 {
            // Maximum Weld energizes only the modifier rows, leaving unlearned elemental
            // primaries untouched. Repeated build membership has no further effect.
            for build in NATIVE_WELD_BUILDS {
                for id in build.component_skill_ids {
                    if ![8, 16, 24, 32, 40].contains(&id) && ranks[id] == 0 {
                        ranks[id] = 1;
                    }
                }
            }
        }
        if self.mindstar_active {
            for id in 8..=77 {
                if ranks[id] > 0 {
                    ranks[id] = NATIVE_SKILL_CATALOG[id].maximum_level.min(ranks[id] + 1);
                }
            }
        }
        book.effective_ranks = ranks;
        // Visible acquisition order keeps a skill in place while any provider
        // still grants it. Only expired ranks leave; newly granted FX rows append.
        book.learned_skill_order.retain(|id| ranks[*id] != 0);
        for id in equipment.granted_skill_order {
            if !book.learned_skill_order.contains(&id) {
                book.learned_skill_order.push(id);
            }
        }
        if let Some(build) = book.weld_build_id.and_then(native_weld_build)
            && (book.weld_component_ranks.is_none() || economy.owned_perk_selectors.contains(&13))
        {
            let component_ranks = build.component_skill_ids.map(|id| ranks[id]);
            assert!(
                component_ranks.iter().all(|rank| *rank <= 255),
                "Weld component ranks fit the native byte lanes"
            );
            book.weld_component_ranks = Some(component_ranks);
        }
        self.concentration_skill_id_a = valid_concentration(self.concentration_skill_id_a, book);
        self.concentration_skill_id_b = if economy.owned_perk_selectors.contains(&21) {
            valid_concentration(self.concentration_skill_id_b, book)
        } else {
            None
        };
        self.equipment_modifiers = equipment.modifiers;
        self.harden_armor_maximum =
            effective_skill_numeric_value_with_stat_book(book, stat_book, 36, "mMaxArmor");
        self.harden_armor_per_tick =
            effective_skill_numeric_value_with_stat_book(book, stat_book, 36, "mArmorPlus") / 100.0;
        self.hurricane_enabled = book.effective_ranks[29] > 0;
        if !self.hurricane_enabled {
            self.hurricane_charge = 0.0;
            self.hurricane_refreshed = false;
        }
        let delay = meditation_idle_delay_ticks(book, stat_book);
        if delay < 0 {
            self.meditation_activity_ramp_ticks = 0;
            self.meditation_idle_elapsed_ticks = 0;
        } else {
            self.meditation_activity_ramp_ticks =
                self.meditation_activity_ramp_ticks.min(delay as u64);
            self.meditation_idle_elapsed_ticks =
                self.meditation_idle_elapsed_ticks.min(delay as u64);
        }
    }

    pub fn toggle_staff_melee_lane(&mut self) {
        self.staff_melee_alternate = !self.staff_melee_alternate;
    }

    pub fn set_mindstar_active(
        &mut self,
        active: bool,
        book: &mut NativeSkillBookState,
        economy: &HubEconomyState,
    ) {
        self.set_mindstar_active_with_stat_book(
            active,
            book,
            &PlayerStatBookComponent::default(),
            economy,
        );
    }

    pub fn set_mindstar_active_with_stat_book(
        &mut self,
        active: bool,
        book: &mut NativeSkillBookState,
        stat_book: &PlayerStatBookComponent,
        economy: &HubEconomyState,
    ) {
        self.mindstar_active = active;
        self.refresh_with_stat_book(book, stat_book, economy);
    }

    #[must_use]
    pub fn is_concentrated(&self, progression: &PlayerProgressionState, skill_id: usize) -> bool {
        progression.mind_chug_ticks_remaining > 0
            || self.concentration_skill_id_a == Some(skill_id)
            || self.concentration_skill_id_b == Some(skill_id)
    }

    pub fn set_concentration(
        &mut self,
        book: &mut NativeSkillBookState,
        economy: &HubEconomyState,
        skill_id: usize,
    ) -> bool {
        self.set_concentration_with_stat_book(
            book,
            &PlayerStatBookComponent::default(),
            economy,
            skill_id,
        )
    }

    pub fn set_concentration_with_stat_book(
        &mut self,
        book: &mut NativeSkillBookState,
        stat_book: &PlayerStatBookComponent,
        economy: &HubEconomyState,
        skill_id: usize,
    ) -> bool {
        if valid_concentration(Some(skill_id), book).is_none() {
            return false;
        }
        if self.concentration_skill_id_a == Some(skill_id)
            || self.concentration_skill_id_b == Some(skill_id)
        {
            return true;
        }
        let split = economy.owned_perk_selectors.contains(&21);
        let slot = if self.concentration_skill_id_a.is_none() {
            PlayerConcentrationSlot::A
        } else if split && self.concentration_skill_id_b.is_none() {
            PlayerConcentrationSlot::B
        } else if split {
            self.next_concentration_replacement_slot
        } else {
            PlayerConcentrationSlot::A
        };
        self.assign_concentration(slot, skill_id, split);
        self.refresh_with_stat_book(book, stat_book, economy);
        true
    }

    pub fn set_concentration_slot(
        &mut self,
        book: &mut NativeSkillBookState,
        economy: &HubEconomyState,
        skill_id: usize,
        slot: PlayerConcentrationSlot,
    ) -> bool {
        self.set_concentration_slot_with_stat_book(
            book,
            &PlayerStatBookComponent::default(),
            economy,
            skill_id,
            slot,
        )
    }

    pub fn set_concentration_slot_with_stat_book(
        &mut self,
        book: &mut NativeSkillBookState,
        stat_book: &PlayerStatBookComponent,
        economy: &HubEconomyState,
        skill_id: usize,
        slot: PlayerConcentrationSlot,
    ) -> bool {
        if valid_concentration(Some(skill_id), book).is_none() {
            return false;
        }
        let split = economy.owned_perk_selectors.contains(&21);
        if slot == PlayerConcentrationSlot::B && !split {
            return false;
        }
        let other = if slot == PlayerConcentrationSlot::A {
            self.concentration_skill_id_b
        } else {
            self.concentration_skill_id_a
        };
        if other == Some(skill_id) {
            return false;
        }
        self.assign_concentration(slot, skill_id, split);
        self.refresh_with_stat_book(book, stat_book, economy);
        true
    }

    fn assign_concentration(
        &mut self,
        slot: PlayerConcentrationSlot,
        skill_id: usize,
        split: bool,
    ) {
        match slot {
            PlayerConcentrationSlot::A => self.concentration_skill_id_a = Some(skill_id),
            PlayerConcentrationSlot::B => self.concentration_skill_id_b = Some(skill_id),
        }
        self.next_concentration_replacement_slot = if split && slot == PlayerConcentrationSlot::A {
            PlayerConcentrationSlot::B
        } else {
            PlayerConcentrationSlot::A
        };
    }

    pub fn autofill_selections(
        &mut self,
        book: &mut NativeSkillBookState,
        economy: &HubEconomyState,
        rng: &mut NativeRng,
    ) {
        self.autofill_selections_with_stat_book(
            book,
            &PlayerStatBookComponent::default(),
            economy,
            rng,
        );
    }

    pub fn autofill_selections_with_stat_book(
        &mut self,
        book: &mut NativeSkillBookState,
        stat_book: &PlayerStatBookComponent,
        economy: &HubEconomyState,
        rng: &mut NativeRng,
    ) {
        self.refresh_with_stat_book(book, stat_book, economy);
        if self.concentration_skill_id_a.is_none() {
            self.concentration_skill_id_a =
                draw_concentration(book, self.concentration_skill_id_b, rng);
        }
        if economy.owned_perk_selectors.contains(&21) && self.concentration_skill_id_b.is_none() {
            self.concentration_skill_id_b =
                draw_concentration(book, self.concentration_skill_id_a, rng);
        }
        if native_skill_category(book.primary_skill_id) != Some(1)
            || book.effective_ranks[book.primary_skill_id] < 1
        {
            let mut candidates = Vec::new();
            for (id, rank) in book.effective_ranks.iter().enumerate() {
                if *rank > 0 && native_skill_category(id) == Some(1) {
                    candidates.push(id);
                }
            }
            if !candidates.is_empty() {
                book.primary_skill_id =
                    candidates[rng.draw_integer(candidates.len() as i32) as usize];
            }
        }
        self.refresh_with_stat_book(book, stat_book, economy);
    }

    pub fn step(
        &mut self,
        derived: &PlayerSkillDerivedStats,
        activity: PlayerSkillActivity,
    ) -> PlayerSkillRuntimeTickResult {
        let active = activity.acting || activity.moving;
        if derived.meditation_idle_delay_ticks >= 0 {
            let delay = derived.meditation_idle_delay_ticks as u64;
            if active {
                self.meditation_activity_ramp_ticks =
                    (self.meditation_activity_ramp_ticks + 1).min(delay);
                if !derived.meditation_concentrated {
                    self.meditation_idle_elapsed_ticks = 0;
                }
            }
            self.meditation_idle_elapsed_ticks =
                (self.meditation_idle_elapsed_ticks + 1).min(delay);
        }
        let ready = derived.meditation_idle_delay_ticks >= 0
            && self.meditation_idle_elapsed_ticks >= derived.meditation_idle_delay_ticks as u64;
        let multiplier = if self.meditation_activity_ramp_ticks > 0 {
            1.0 + (derived.meditation_recovery_multiplier - 1.0) * 0.25
        } else {
            derived.meditation_recovery_multiplier
        };
        let meditation = if ready { derived.mana_recovery_per_tick * multiplier } else { 0.0 };
        self.meditation_activity_ramp_ticks = self.meditation_activity_ramp_ticks.saturating_sub(1);
        let hurricane = native_hurricane_charge_tick(
            self.hurricane_charge,
            self.hurricane_refreshed,
            self.hurricane_enabled,
            activity.primary_channel == Some(WizardElement::Air) && !activity.primary_underpowered,
        );
        self.hurricane_charge = hurricane.next_charge;
        self.hurricane_refreshed = hurricane.refreshed;
        let channel = if activity.primary_channel == Some(WizardElement::Water) {
            if activity.primary_underpowered {
                NativeHardenChannel::WeakWater
            } else {
                NativeHardenChannel::Water
            }
        } else if activity.primary_channel.is_some() || activity.primary_active {
            NativeHardenChannel::Other
        } else {
            NativeHardenChannel::Idle
        };
        self.harden.step(channel, self.harden_armor_per_tick, self.harden_armor_maximum);
        PlayerSkillRuntimeTickResult {
            base_mana_recovery_per_tick: derived.mana_recovery_per_tick,
            meditation_mana_recovery_per_tick: meditation,
        }
    }

    pub fn mark_creativity_insight(
        &self,
        offer: &mut PlayerSkillOffer,
        book: &NativeSkillBookState,
        rng: &mut NativeRng,
    ) {
        self.mark_creativity_insight_with_stat_book(
            offer,
            book,
            &PlayerStatBookComponent::default(),
            rng,
        );
    }

    pub fn mark_creativity_insight_with_stat_book(
        &self,
        offer: &mut PlayerSkillOffer,
        book: &NativeSkillBookState,
        stat_book: &PlayerStatBookComponent,
        rng: &mut NativeRng,
    ) {
        if (self.concentration_skill_id_a != Some(63) && self.concentration_skill_id_b != Some(63))
            || rng.draw_integer(5) != 1
        {
            return;
        }
        let mut candidates = Vec::new();
        for (index, option) in offer.options.iter().enumerate() {
            // Retail compares the option index to 52 rather than the skill identity.
            if index == 52 {
                continue;
            }
            let rank = book.effective_ranks[option.skill_id];
            if native_skill_category(option.skill_id) == Some(4) && rank == 0 {
                continue;
            }
            let maximum =
                stat_book.entries.get(option.skill_id).map_or(0, |entry| entry.maximum_level);
            if i64::from(rank) < i64::from(maximum) - 2 {
                candidates.push(option.skill_id);
            }
        }
        if candidates.is_empty() {
            return;
        }
        let selected = candidates[rng.draw_integer(candidates.len() as i32) as usize];
        for option in &mut offer.options {
            if option.skill_id == selected {
                option.insight = true;
            }
        }
    }
}

#[must_use]
pub fn player_skill_derived_stats(
    runtime: &PlayerSkillRuntimeState,
    book: &NativeSkillBookState,
    progression: &PlayerProgressionState,
    economy: &HubEconomyState,
) -> PlayerSkillDerivedStats {
    player_skill_derived_stats_with_stat_book(
        runtime,
        book,
        &PlayerStatBookComponent::default(),
        progression,
        economy,
    )
}

#[must_use]
pub fn player_skill_derived_stats_with_stat_book(
    runtime: &PlayerSkillRuntimeState,
    book: &NativeSkillBookState,
    stat_book: &PlayerStatBookComponent,
    progression: &PlayerProgressionState,
    economy: &HubEconomyState,
) -> PlayerSkillDerivedStats {
    let selected = |id| runtime.is_concentrated(progression, id);
    let value =
        |id, property| effective_skill_numeric_value_with_stat_book(book, stat_book, id, property);
    let channel_mana = value(57, "mValue");
    let battle_mage = value(59, "mValue");
    let focus = value(60, "mValue");
    let siege_mage = value(61, "mValue");
    let resist_magic = value(62, "mValue");
    let staff_damage = value(65, "mDamage");
    let telekinesis = value(66, "mValue");
    let rush = value(67, "mValue");
    let resist_poison = value(69, "mValue");
    let faster_caster = value(70, "mValue");
    let hagatha = native_hagatha_derived_modifiers(
        &economy.owned_perk_selectors,
        progression.hagatha_runtime,
        economy.equipment.weapon.is_some(),
    );
    let modifiers = &runtime.equipment_modifiers;
    let base_offensive_damage_factor = (1.0
        + (siege_mage + if selected(61) { value(61, "mConcentration") } else { 0.0 }) / 100.0)
        * if progression.damage_x4_ticks_remaining > 0 { 4.0 } else { 1.0 };
    let offensive_damage_factor = base_offensive_damage_factor * hagatha.spell_damage_factor;
    let offensive_mana_cost_factor = (1.0
        - (battle_mage + if selected(59) { value(59, "mConcentration") } else { 0.0 }) / 100.0)
        .max(0.0)
        * hagatha.offensive_mana_factor;
    let class = native_skill_root(book.primary_skill_id).expect("primary skill has a native class");
    let cast_base = 1.0
        + (faster_caster + if selected(70) { value(70, "mConcentration") } else { 0.0 }) / 100.0;
    let cast_progress = ((modifiers.cast_speed_multiplier * cast_base + modifiers.cast_speed_flat)
        * modifiers.class_cast_speed_multiplier[class]
        + modifiers.class_cast_speed_flat[class])
        .max(0.0);
    let mana_recovery = modifiers.mana_recovery.apply(
        PLAYER_MANA_RECOVERY_PER_TICK
            * f64::from(TICK_RATE_HZ)
            * (1.0 + channel_mana / 100.0)
            * if selected(57) { 1.0 + value(57, "mConcentration") / 100.0 } else { 1.0 },
    );
    let staff_equipped = economy
        .equipment
        .weapon
        .as_ref()
        .is_some_and(|item| item.equipment_type == Some(EquipmentType::Staff));
    let life_charm =
        if economy.owned_perk_selectors.contains(&0) { NATIVE_HAGATHA_FACTORS.life } else { 1.0 };
    let mana_charm =
        if economy.owned_perk_selectors.contains(&1) { NATIVE_HAGATHA_FACTORS.mana } else { 1.0 };
    // Native MELEEDAMAGE formats the two body values, separately from staff
    // contact's row-65 offensive resolution below. Preserve both f32 stores.
    let melee_multiplier = fround(
        modifiers.melee_damage_multiplier
            * if progression.damage_x4_ticks_remaining > 0 { 4.0 } else { 1.0 }
            * hagatha.melee_damage_factor,
    );
    let melee_damage = |base| {
        fround(fround(base + staff_damage) * melee_multiplier + modifiers.melee_damage_flat)
            .max(0.0)
    };
    PlayerSkillDerivedStats {
        cast_progress_factor: multiply_hagatha_factor(cast_progress, hagatha.cast_speed_factor),
        damage_resistance: modifiers.damage_resistance.clamp(0.0, 1.0),
        deflect_chance_percent: if staff_equipped { value(68, "mValue") } else { 0.0 },
        experience_bonus: economy.unforge_bonuses.experience,
        flash_chance_percent: value(53, "mChance"),
        flash_duration_ticks: (value(53, "mDuration") * 100.0 + 0.5).floor() as u64,
        focus_instant_recharge_chance_percent: if selected(60) {
            value(60, "mConcentration")
        } else {
            0.0
        },
        flailing_chance_percent: value(71, "mChance"),
        gold_amount_multiplier: modifiers.gold_multiplier,
        health_recovery_per_tick: modifiers.health_recovery.apply(PLAYER_HEALTH_RECOVERY_PER_TICK),
        incoming_damage_factor: hagatha.incoming_damage_factor,
        magic_resistance: ((resist_magic
            + if selected(62) { value(62, "mConcentration") } else { 0.0 })
            / 100.0
            + modifiers.magic_resistance)
            .clamp(0.0, 1.0),
        mana_recovery_per_second: mana_recovery,
        mana_recovery_per_tick: mana_recovery / f64::from(TICK_RATE_HZ),
        maximum_health: fround(
            modifiers.maximum_health.apply(
                PLAYER_INITIAL_HEALTH
                    + economy.unforge_bonuses.maximum_health
                    + value(64, "mValue"),
            ) * life_charm,
        ),
        maximum_mana: fround(
            modifiers.maximum_mana.apply(
                PLAYER_INITIAL_MANA + economy.unforge_bonuses.maximum_mana + value(56, "mValue"),
            ) * mana_charm,
        ),
        meditation_concentrated: selected(58),
        meditation_idle_delay_ticks: meditation_idle_delay_ticks(book, stat_book),
        meditation_recovery_multiplier: if book.effective_ranks[58] > 0 {
            value(58, "mValue")
        } else {
            1.0
        },
        melee_damage_maximum: melee_damage(1.0),
        melee_damage_minimum: melee_damage(0.5),
        movement_factor: multiply_hagatha_factor(
            modifiers.walk_speed.apply(
                (1.0 + rush / 100.0)
                    * if selected(67) { 1.0 + value(67, "mConcentration") / 100.0 } else { 1.0 },
            ),
            hagatha.movement_factor,
        ),
        offensive_damage_factor,
        offensive_damage_flat: economy.unforge_bonuses.offensive_damage,
        offensive_mana_cost_factor,
        offensive_mana_cost_reduction: economy.unforge_bonuses.mana_cost_reduction,
        orb_pull_multiplier: modifiers.orb_pull_multiplier,
        pickup_range_scalar: telekinesis * if selected(66) { 2.5 } else { 1.25 },
        poison_resistance: ((resist_poison
            + if selected(69) { value(69, "mConcentration") } else { 0.0 })
            / 100.0
            + modifiers.poison_resistance)
            .clamp(0.0, 1.0),
        poison_damage_factor: hagatha.poison_damage_factor,
        push_strength_factor: hagatha.push_strength_factor,
        secondary_recharge_factor: multiply_hagatha_factor(
            modifiers.recharge.apply(1.0 + focus / 100.0),
            hagatha.recharge_factor,
        ),
        staff_action_timing_factor: if selected(65) { 1.75 } else { 1.0 },
        staff_damage_primary: staff_damage,
        staff_damage_secondary: staff_damage,
        staff_equipped,
    }
}

impl PlayerSkillDerivedStats {
    #[must_use]
    pub fn offensive_factors(
        &self,
        runtime: &PlayerSkillRuntimeState,
    ) -> NativeOffensiveSpellFactors {
        NativeOffensiveSpellFactors {
            damage: self.offensive_damage_factor,
            mana_cost: self.offensive_mana_cost_factor,
            global_flat_damage: self.offensive_damage_flat,
            global_mana_reduction: self.offensive_mana_cost_reduction,
            equipment: Some(runtime.equipment_modifiers.clone()),
        }
    }
}

pub fn refresh_player_combat_from_skill_stats(
    combat: &mut PlayerCombatState,
    derived: &PlayerSkillDerivedStats,
) {
    let health_ratio = if combat.maximum_health > 0.0 {
        combat.current_health / combat.maximum_health
    } else {
        1.0
    };
    let mana_ratio =
        if combat.maximum_mana > 0.0 { combat.current_mana / combat.maximum_mana } else { 1.0 };
    combat.current_health =
        derived.maximum_health.min((derived.maximum_health * health_ratio).max(0.0));
    combat.current_mana = derived.maximum_mana.min((derived.maximum_mana * mana_ratio).max(0.0));
    combat.maximum_health = derived.maximum_health;
    combat.maximum_mana = derived.maximum_mana;
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerStaffDamageLane {
    Primary,
    Secondary,
}

impl PlayerStaffDamageLane {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerStaffAttackOutcome {
    Normal,
    Knockback,
    DisablingHit,
    CriticalHit,
    Whirl,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStaffAttackResult {
    pub action_timing_factor: f64,
    pub outcome: PlayerStaffAttackOutcome,
}

#[must_use]
pub fn resolve_player_staff_attack(
    derived: &PlayerSkillDerivedStats,
    rng: &mut NativeRng,
) -> PlayerStaffAttackResult {
    assert!(derived.staff_equipped, "staff attack requires an equipped staff");
    let chance = rng.draw_scaled_float(100.0, false);
    let outcome = if derived.flailing_chance_percent >= chance {
        [
            PlayerStaffAttackOutcome::Knockback,
            PlayerStaffAttackOutcome::DisablingHit,
            PlayerStaffAttackOutcome::CriticalHit,
            PlayerStaffAttackOutcome::Whirl,
        ][rng.draw_integer(4) as usize]
    } else {
        PlayerStaffAttackOutcome::Normal
    };
    PlayerStaffAttackResult { action_timing_factor: derived.staff_action_timing_factor, outcome }
}

#[must_use]
pub fn player_staff_damage(
    runtime: &PlayerSkillRuntimeState,
    derived: &PlayerSkillDerivedStats,
    progression: &PlayerProgressionState,
    outcome: PlayerStaffAttackOutcome,
) -> f64 {
    let factors = derived.offensive_factors(runtime);
    let base = resolve_native_skill_damage_value(65, derived.staff_damage_primary, &factors);
    base.max(1.0)
        * if outcome == PlayerStaffAttackOutcome::CriticalHit { 3.0 } else { 1.0 }
        * if outcome != PlayerStaffAttackOutcome::Normal && runtime.is_concentrated(progression, 71)
        {
            1.2
        } else {
            1.0
        }
}

#[must_use]
pub fn player_poison_duration_seconds(
    derived: &PlayerSkillDerivedStats,
    duration_seconds: f64,
) -> f64 {
    assert!(
        duration_seconds.is_finite() && duration_seconds >= 0.0,
        "poison duration is finite and non-negative"
    );
    let ticks = fround(duration_seconds * f64::from(TICK_RATE_HZ)).trunc();
    let resisted = fround(ticks * derived.poison_resistance).trunc();
    (ticks - resisted).max(0.0) / f64::from(TICK_RATE_HZ)
}

/// Runtime stat reads retain rank-zero entries; unlike cast admission, scalar properties
/// and authored row zero can contribute even before their skill is learned.
#[must_use]
pub fn effective_skill_numeric_value(
    book: &NativeSkillBookState,
    skill_id: usize,
    property: &str,
) -> f64 {
    effective_skill_numeric_value_with_stat_book(
        book,
        &PlayerStatBookComponent::default(),
        skill_id,
        property,
    )
}

#[must_use]
pub fn effective_skill_numeric_value_with_stat_book(
    book: &NativeSkillBookState,
    stat_book: &PlayerStatBookComponent,
    skill_id: usize,
    property: &str,
) -> f64 {
    let entry = stat_book
        .entries
        .get(skill_id)
        .filter(|entry| entry.id == skill_id)
        .expect("native skill stat row exists");
    entry.stat(property, book.effective_ranks[skill_id]).unwrap_or(0.0)
}

fn valid_concentration(selection: Option<usize>, book: &NativeSkillBookState) -> Option<usize> {
    selection.filter(|id| CONCENTRATABLE_SKILL_IDS.contains(id) && book.effective_ranks[*id] > 0)
}
fn draw_concentration(
    book: &NativeSkillBookState,
    excluded: Option<usize>,
    rng: &mut NativeRng,
) -> Option<usize> {
    let mut candidates = Vec::new();
    for id in CONCENTRATABLE_SKILL_IDS {
        if Some(id) != excluded && book.effective_ranks[id] > 0 {
            candidates.push(id);
        }
    }
    if candidates.is_empty() {
        None
    } else {
        Some(candidates[rng.draw_integer(candidates.len() as i32) as usize])
    }
}
fn meditation_idle_delay_ticks(
    book: &NativeSkillBookState,
    stat_book: &PlayerStatBookComponent,
) -> i64 {
    if book.effective_ranks[58] > 0 {
        (effective_skill_numeric_value_with_stat_book(book, stat_book, 58, "mSeconds") * 100.0)
            .trunc() as i64
    } else {
        -1
    }
}
fn multiply_hagatha_factor(value: f64, factor: f64) -> f64 {
    if factor == 1.0 { value } else { fround(value * factor) }
}
