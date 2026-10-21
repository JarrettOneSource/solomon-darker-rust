//! Player action clocks and weapon emitters from `core-kernels/primary-spells.ts`.
//!
//! The source's -1 action/selection sentinels become `None`. Cast progress stays f64: its
//! native-rate thresholds must observe the same crossing tick as the Website.

use super::weapon_offsets::{BARE_OFFSETS, STAFF_OFFSETS, WAND_OFFSETS};
use crate::Vec2;
use crate::float::hypot;
use crate::profile::WizardElement;

/// The Website's stored single is 0.8999999761581421.
pub const NATIVE_PLAYER_WEAPON_PULSE_DECAY: f64 = 0.9_f32 as f64;
pub const NATIVE_ETHER_BLAST_WEAPON_PULSE: f64 = 0.25_f32 as f64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativePlayerWeaponKind {
    Staff,
    Wand,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerPrimaryCastState {
    pub action_tick: Option<f64>,
    pub aim_direction: Vec2,
    pub cast_sequence: u64,
    pub channel_active: bool,
    pub emission_sequence: u64,
    pub ether_blast_charge: f64,
    pub ether_blast_charge_cue_sequence: u64,
    pub fizzle_sequence: u64,
    pub held: bool,
    pub last_weld_playback_rate: Option<f64>,
    pub last_weld_sound_variant: Option<u32>,
    pub one_shot_attack_pose_held: bool,
    pub selected_primary_age_ticks: u64,
    pub selected_primary_id: Option<u32>,
    pub target_id: Option<String>,
    pub underpowered: bool,
    pub weapon_pulse: f64,
}

impl Default for PlayerPrimaryCastState {
    fn default() -> Self {
        Self {
            action_tick: None,
            aim_direction: Vec2::new(0.0, 1.0),
            cast_sequence: 0,
            channel_active: false,
            emission_sequence: 0,
            ether_blast_charge: 0.0,
            ether_blast_charge_cue_sequence: 0,
            fizzle_sequence: 0,
            held: false,
            last_weld_playback_rate: None,
            last_weld_sound_variant: None,
            one_shot_attack_pose_held: false,
            selected_primary_age_ticks: 0,
            selected_primary_id: None,
            target_id: None,
            underpowered: false,
            weapon_pulse: 0.0,
        }
    }
}

impl PlayerPrimaryCastState {
    #[must_use]
    pub fn owns_facing(&self) -> bool {
        self.action_tick.is_some() || self.channel_active
    }

    pub fn reset(&mut self) {
        let charge = self.ether_blast_charge;
        let cue_sequence = self.ether_blast_charge_cue_sequence;
        let pulse = self.weapon_pulse;
        *self = Self::default();
        self.ether_blast_charge = charge;
        self.ether_blast_charge_cue_sequence = cue_sequence;
        self.weapon_pulse = pulse;
    }

    /// Book selection clears occupancy while retaining the source's emission counters,
    /// aim direction, Ether charge and weapon pulse from the previous selection.
    pub fn reset_for_selection(&mut self, selected_primary_id: u32) {
        self.action_tick = None;
        self.channel_active = false;
        self.held = false;
        self.last_weld_playback_rate = None;
        self.last_weld_sound_variant = None;
        self.one_shot_attack_pose_held = false;
        self.selected_primary_age_ticks = 0;
        self.selected_primary_id = Some(selected_primary_id);
        self.target_id = None;
        self.underpowered = false;
    }

    /// `advancePrimaryCast`: retain an action on the tick that first crosses progress four;
    /// the following tick observes the free slot. Active channels instead cap progress at one.
    pub fn advance(
        &mut self,
        held: bool,
        accepted: bool,
        element: WizardElement,
        progress_factor: f64,
    ) {
        assert!(
            progress_factor.is_finite() && progress_factor >= 0.0,
            "primary cast progress factor must be finite and nonnegative"
        );
        if let Some(action_tick) = self.action_tick {
            self.action_tick = if self.channel_active {
                Some((action_tick + progress_factor).min(1.0))
            } else if action_tick > primary_cast_completion_progress(element) {
                None
            } else {
                Some(action_tick + progress_factor)
            };
        }
        if accepted {
            self.action_tick = Some(0.0);
            self.cast_sequence += 1;
            self.underpowered = false;
        }
        self.held = held;
        self.one_shot_attack_pose_held &= held || self.action_tick.is_some();
    }
}

#[must_use]
pub fn primary_cast_rate(element: WizardElement) -> f64 {
    let ether_rate = 0.075_f32 as f64;
    if element == WizardElement::Ether {
        ether_rate
    } else {
        crate::float::fround(ether_rate * 0.75)
    }
}

#[must_use]
pub fn primary_cast_completion_progress(element: WizardElement) -> f64 {
    4.0 / primary_cast_rate(element)
}

#[must_use]
pub fn primary_cast_emission_progress(element: WizardElement) -> f64 {
    1.0 / primary_cast_rate(element)
}

#[must_use]
pub fn primary_cast_pose(
    action_tick: Option<f64>,
    channel_active: bool,
    element: WizardElement,
) -> usize {
    let action = action_tick.unwrap_or(-1.0);
    if channel_active {
        return if action <= 0.0 { 0 } else { 7 };
    }
    let action_end = if element == WizardElement::Ether { 55.0 } else { 73.0 };
    if action <= 0.0 || action >= action_end {
        return 0;
    }
    if action < primary_cast_emission_progress(element) {
        return 1;
    }
    if action < 2.0 / primary_cast_rate(element) {
        return 8;
    }
    7
}

#[must_use]
pub fn primary_cast_presentation_pose(
    cast: &PlayerPrimaryCastState,
    element: WizardElement,
) -> usize {
    if cast.one_shot_attack_pose_held {
        8
    } else {
        primary_cast_pose(cast.action_tick, cast.channel_active, element)
    }
}

#[must_use]
pub fn primary_spell_emitter_offset(
    heading: usize,
    action_tick: Option<f64>,
    channel_active: bool,
    element: WizardElement,
    weapon_kind: Option<NativePlayerWeaponKind>,
) -> Vec2 {
    let heading = heading % 24;
    match weapon_kind {
        None => BARE_OFFSETS[heading],
        Some(NativePlayerWeaponKind::Staff) => player_staff_attachment_offset(
            heading,
            primary_cast_pose(action_tick, channel_active, element),
        ),
        Some(NativePlayerWeaponKind::Wand) => {
            let pose = player_wand_primary_pose(action_tick, channel_active, element);
            WAND_OFFSETS[pose][heading]
        }
    }
}

#[must_use]
pub fn player_staff_attachment_offset(heading: usize, pose: usize) -> Vec2 {
    STAFF_OFFSETS[pose][heading % 24]
}

#[must_use]
pub fn player_wand_primary_pose(
    action_tick: Option<f64>,
    channel_active: bool,
    element: WizardElement,
) -> usize {
    let rate = if element == WizardElement::Ether {
        0.1_f32 as f64
    } else {
        crate::float::fround(0.1_f32 as f64 * 0.75)
    };
    let action = action_tick.unwrap_or(-1.0);
    usize::from(action > 0.0 && !channel_active && crate::float::fround(action * rate) < 2.0)
}

#[must_use]
pub fn primary_spell_aim_direction(position: Vec2, world_aim: Vec2, view_scale: f64) -> Vec2 {
    let dx = world_aim.x - position.x;
    let dy = world_aim.y - (position.y - 25.0 / view_scale);
    let length = hypot(dx, dy);
    if length > 0.0001 { Vec2::new(dx / length, dy / length) } else { Vec2::new(0.0, -1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ether_retains_the_action_on_the_tick_that_crosses_progress_four() {
        let mut cast = PlayerPrimaryCastState::default();
        cast.advance(true, true, WizardElement::Ether, 1.0);
        for _ in 0..54 {
            cast.advance(true, false, WizardElement::Ether, 1.0);
        }
        assert_eq!(cast.action_tick, Some(54.0));
        cast.advance(true, false, WizardElement::Ether, 1.0);
        assert_eq!(cast.action_tick, None);
    }
}
