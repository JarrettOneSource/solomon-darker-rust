//! Each player's secondary state: cast actions, cooldown clocks, toggles, mana reserve and
//! Focus recharge.

use crate::Rng;
use crate::float::fround;
use crate::profile::offensive::OffensiveSpellFactors;
use crate::profile::{SkillBookState, skill_category, skill_root};
use crate::spells::casting::{PlayerWeaponKind, forward_action_marker_crossed};

use super::{
    SecondaryAbilityId, effective_secondary_ability_rank_stats, effective_skill_value,
    secondary_cooldown_capacity_ticks,
};

pub const SECONDARY_GLOBAL_COOLDOWN_TICKS: f64 = 150.0;

/// A secondary cast animation in progress: the weapon used, its progress and any bare-hand
/// pose.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(try_from = "saved_cast::Document"))]
pub struct SecondaryCastAction {
    pub weapon_kind: Option<PlayerWeaponKind>,
    pub progress: f64,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub hand: Option<HandCast2>,
}

/// The bare-hand cast animation of a weaponless secondary cast: its pose and whether it
/// has finished.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HandCast2 {
    pub pose: usize,
    pub done: bool,
}

impl SecondaryCastAction {
    #[must_use]
    pub fn new(weapon_kind: Option<PlayerWeaponKind>) -> Self {
        Self {
            weapon_kind,
            progress: 0.0,
            hand: weapon_kind.is_none().then_some(HandCast2 { pose: 0, done: false }),
        }
    }
}

/// A secondary cast action after one step, and whether it crossed its release marker.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SecondaryCastStep {
    pub action: Option<SecondaryCastAction>,
    pub marker_crossed: bool,
}

#[must_use]
pub fn step_secondary_cast_action(
    action: Option<SecondaryCastAction>,
    faster_caster_percent: f64,
    action_time_factor: f64,
) -> SecondaryCastStep {
    let Some(mut action) = action else { return SecondaryCastStep::default() };
    let previous = action.progress;
    if action.weapon_kind.is_none() {
        let hand = action.hand.get_or_insert(HandCast2 {
            pose: if action.progress == 0.0 {
                0
            } else if action.progress < 1.0 {
                1
            } else {
                3
            },
            done: action.progress >= 6.0,
        });
        if hand.done {
            return SecondaryCastStep::default();
        }
        // Bare Cast2 has a fixed born .095 rate. The native admission helper
        // 0x005297D0 applies no Faster Caster construction multiplier.
        action.progress = fround(action.progress + fround(action_time_factor) * 0.095_f32 as f64);
        hand.pose = if action.progress < 1.0 {
            1
        } else if action.progress < 7.0 {
            3
        } else {
            0
        };
        hand.done = action.progress >= 6.0;
        return SecondaryCastStep {
            action: Some(action),
            marker_crossed: forward_action_marker_crossed(previous, action.progress, 1.0),
        };
    }
    let staff = action.weapon_kind == Some(PlayerWeaponKind::Staff);
    let rate = if staff { 0.1_f32 as f64 } else { 0.095_f32 as f64 };
    let factor = fround(1.0 + faster_caster_percent / 100.0);
    action.progress = fround(action.progress + factor * rate);
    let end = if staff { 5.0 } else { 6.0 };
    SecondaryCastStep {
        marker_crossed: forward_action_marker_crossed(
            previous,
            action.progress,
            if staff { 0.0 } else { 1.0 },
        ),
        action: (action.progress <= end).then_some(action),
    }
}

#[cfg(feature = "persistence")]
mod saved_cast {
    use super::{HandCast2, PlayerWeaponKind, SecondaryCastAction};

    /// A saved cast action, validated before it becomes a `SecondaryCastAction`.
    #[derive(serde::Deserialize)]
    #[serde(rename = "SecondaryCastAction")]
    pub(super) struct Document {
        weapon_kind: Option<PlayerWeaponKind>,
        progress: f64,
        #[serde(default)]
        hand: Option<HandCast2>,
    }

    impl TryFrom<Document> for SecondaryCastAction {
        type Error = &'static str;
        fn try_from(value: Document) -> Result<Self, Self::Error> {
            if !value.progress.is_finite()
                || value.progress < 0.0
                || value.hand.is_some_and(|hand| {
                    value.weapon_kind.is_some()
                        || hand.pose > 3
                        || hand.done != (value.progress >= 6.0)
                })
            {
                return Err("secondary hand action has invalid progress, pose or terminal state");
            }
            Ok(Self { weapon_kind: value.weapon_kind, progress: value.progress, hand: value.hand })
        }
    }
}

/// A player's secondary ability state: cast action, cooldowns, toggles, Magic Shield,
/// Planewalker and Stoneskin timers, and reserved mana.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SecondaryPlayerState {
    pub cast_action: Option<SecondaryCastAction>,
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
    pub last_skill_id: Option<SecondaryAbilityId>,
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

impl Default for SecondaryPlayerState {
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

impl SecondaryPlayerState {
    pub fn start_cast(&mut self, weapon_kind: Option<PlayerWeaponKind>) {
        self.cast_action = Some(SecondaryCastAction::new(weapon_kind));
    }

    /// Fractional recharge factors stay in doubles; these timers are never narrowed.
    pub fn step(
        &mut self,
        skill_book: &SkillBookState,
        secondary_recharge_factor: f64,
        offensive_factors: &OffensiveSpellFactors,
        action_time_factor: f64,
    ) -> bool {
        for ability in SecondaryAbilityId::ALL {
            self.cooldown_maximum_ticks_by_skill[ability.skill_id()] =
                secondary_cooldown_capacity_ticks(skill_book, ability);
        }
        for skill_id in 0..self.cooldown_ticks_by_skill.len() {
            let ticks = self.cooldown_ticks_by_skill[skill_id];
            if ticks <= 0.0 {
                self.cooldown_ticks_by_skill[skill_id] = 0.0;
                continue;
            }
            let mut recharge = secondary_recharge_factor;
            if let Some(equipment) = &offensive_factors.equipment {
                let class_id = skill_root(skill_id).expect("cooling secondary has a native class");
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
        let step = step_secondary_cast_action(
            self.cast_action,
            effective_skill_value(skill_book, 70, "mValue"),
            action_time_factor,
        );
        self.cast_action = step.action;
        step.marker_crossed
    }

    /// Focus may leave the existing cooldown untouched after consuming its native chance draw.
    pub fn begin_cooldown(
        &mut self,
        ability: SecondaryAbilityId,
        ticks: f64,
        focus_instant_recharge_chance_percent: f64,
        rng: &mut Rng,
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
            if *cooldown < SECONDARY_GLOBAL_COOLDOWN_TICKS {
                *cooldown = 0.0;
            }
        }
        self.global_cooldown_ticks = SECONDARY_GLOBAL_COOLDOWN_TICKS;
    }

    pub fn cooldown_rejuvenation(&mut self) {
        for (skill_id, ticks) in self.cooldown_ticks_by_skill.iter_mut().enumerate() {
            if skill_category(skill_id) == Some(2) {
                *ticks = 0.0;
            }
        }
        self.global_cooldown_ticks = 0.0;
    }

    #[must_use]
    pub fn mana_reserve(&self, skill_book: &SkillBookState, maximum_mana: f64) -> f64 {
        let mut reserved_mana = 0.0;
        if self.firewalker {
            let stats =
                effective_secondary_ability_rank_stats(skill_book, SecondaryAbilityId::Firewalker);
            reserved_mana = stats.value("mHoard").unwrap_or(0.0);
        }
        for (active, ability) in [
            (self.mindstar, SecondaryAbilityId::Mindstar),
            (self.regenerate, SecondaryAbilityId::Regenerate),
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
pub fn plane_orb_damage(skill_book: &SkillBookState) -> f64 {
    let mut total = 0;
    for skill_id in [8, 10, 9, 13, 14, 15, 12] {
        total += skill_book.effective_ranks[skill_id];
    }
    2.0 * f64::from(total) / 100.0
}
