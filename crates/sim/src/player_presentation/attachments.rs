//! The staff/wand pose precedence and orb passes in `player-character-presentation.ts`.

use crate::float::fround;
use crate::profile::WizardElement;
use crate::secondary::NativeSecondaryCastAction;
use crate::spells::casting::{
    NativePlayerWeaponKind, PlayerPrimaryCastState, player_wand_primary_pose,
    primary_cast_presentation_pose,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerSecondaryCastPresentation {
    Action(NativeSecondaryCastAction),
    Spin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerAttachmentPoses {
    pub staff: usize,
    pub wand: usize,
    pub bare: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerStaffOrbPasses {
    pub front_base: bool,
    pub front_overlay: bool,
}

#[must_use]
pub fn player_character_attachment_poses(
    cast: &PlayerPrimaryCastState,
    element: WizardElement,
    staff_action_pose: Option<usize>,
    secondary_cast: Option<PlayerSecondaryCastPresentation>,
) -> PlayerAttachmentPoses {
    let staff = if secondary_cast.is_some() {
        9
    } else {
        staff_action_pose.unwrap_or_else(|| primary_cast_presentation_pose(cast, element))
    };
    let mut wand = player_wand_primary_pose(cast.action_tick, cast.channel_active, element);
    match secondary_cast {
        Some(PlayerSecondaryCastPresentation::Spin) => wand = 0,
        Some(PlayerSecondaryCastPresentation::Action(action)) => {
            if action.weapon_kind != Some(NativePlayerWeaponKind::Wand) {
                wand = 0;
            } else if action.progress > 0.0 {
                wand = if action.progress < 1.0 { 1 } else { 2 };
            }
        }
        None => {}
    }
    PlayerAttachmentPoses {
        staff,
        wand,
        bare: cast.selected_primary_id.is_some()
            && staff_action_pose.is_none()
            && secondary_cast.is_none()
            && cast.action_tick.is_none()
            && !cast.channel_active
            && !cast.one_shot_attack_pose_held,
    }
}

#[must_use]
pub fn player_character_staff_is_front(heading_index: usize, attachment_pose: usize) -> bool {
    let heading = heading_index % 24;
    // The four source banks are contiguous ranges, including their boundary facings.
    match attachment_pose {
        4..=6 | 9 => (7..=18).contains(&heading),
        7 | 8 => (7..=17).contains(&heading),
        _ => (5..=16).contains(&heading),
    }
}

#[must_use]
pub fn player_character_staff_orb_passes(
    heading_index: usize,
    attachment_pose: usize,
    overlay_effect_phase: f64,
) -> PlayerStaffOrbPasses {
    let heading_degrees = (heading_index % 24) * 15;
    let back_angle = heading_degrees <= 90 || heading_degrees > 270;
    PlayerStaffOrbPasses {
        front_base: attachment_pose != 9 && (90..=270).contains(&heading_degrees),
        front_overlay: attachment_pose == 9
            || back_angle
            || overlay_effect_phase > f64::from(0.1_f32),
    }
}

#[must_use]
pub fn player_equipped_element_effect_scale(overlay_effect_phase: f64) -> f64 {
    fround(1.0 + 10.0 * overlay_effect_phase)
}

/// The two BadGuys record-7 passes in `renderer/player-damage-x4-vfx-view.ts`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeDamageX4VfxLayer {
    pub alpha: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
}

#[must_use]
pub fn native_damage_x4_vfx_plan(
    remaining_ticks: f64,
    tick: f64,
    emitter_scale: f64,
) -> Vec<NativeDamageX4VfxLayer> {
    if remaining_ticks <= 0.0 {
        return Vec::new();
    }
    let alpha = remaining_ticks.min(100.0) / 100.0;
    let fixed_tick = tick.floor();
    vec![
        NativeDamageX4VfxLayer { alpha, rotation_degrees: fixed_tick, scale: 2.5 * emitter_scale },
        NativeDamageX4VfxLayer {
            alpha,
            rotation_degrees: -0.5 * fixed_tick,
            scale: 2.0 * emitter_scale,
        },
    ]
}
