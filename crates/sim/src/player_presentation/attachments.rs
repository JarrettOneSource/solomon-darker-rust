//! Staff, wand and bare-hand pose precedence, the equipped element orb and its draw
//! passes, and the Damage x4 effect layers.

use crate::float::fround;
use crate::profile::WizardElement;
use crate::secondary::SecondaryCastAction;
use crate::spells::casting::{
    PlayerPrimaryCastState, PlayerWeaponKind, player_staff_attachment_offset,
    player_wand_attachment_offset, player_wand_primary_pose, primary_cast_presentation_pose,
};

/// The secondary cast a wizard is showing: an ordinary cast action or a spin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerSecondaryCastPresentation {
    Action(SecondaryCastAction),
    Spin,
}

/// The staff, wand and bare-hand poses for one character frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerAttachmentPoses {
    pub staff: usize,
    pub wand: usize,
    pub bare: Option<usize>,
}

/// Whether the staff orb's base and overlay passes draw in front of the wizard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerStaffOrbPasses {
    pub front_base: bool,
    pub front_overlay: bool,
}

/// Where and how the equipped element orb draws: socket, scale, passes and attachment
/// offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerEquippedElementPlan {
    pub socket: crate::Vec2,
    pub scale: f64,
    pub passes: PlayerStaffOrbPasses,
    pub overlay_after_head: bool,
    pub attachment_offset: crate::Vec2,
}

/// PlayerWizard::Draw `0x0054BC5B..0x0054BDA1` admits the equipped helper
/// by socket angle. DrawSpecial calls it once outside the attachment transform.
#[must_use]
pub fn player_equipped_element_plan(
    heading: usize,
    poses: PlayerAttachmentPoses,
    weapon_kind: PlayerWeaponKind,
    secondary_cast: Option<PlayerSecondaryCastPresentation>,
    phase: f64,
    gait_degrees: f64,
) -> PlayerEquippedElementPlan {
    let wand = weapon_kind == PlayerWeaponKind::Wand;
    let staff_cast = matches!(secondary_cast, Some(PlayerSecondaryCastPresentation::Action(action))
        if action.weapon_kind == Some(PlayerWeaponKind::Staff));
    let effect_pose =
        if wand && secondary_cast != Some(PlayerSecondaryCastPresentation::Spin) && !staff_cast {
            14 + poses.wand
        } else {
            poses.staff
        };
    let socket = if wand {
        player_wand_attachment_offset(heading, poses.wand)
    } else {
        player_staff_attachment_offset(heading, poses.staff)
    };
    let angle = player_element_call_angle(socket, Some(weapon_kind));
    let mut attachment_offset = crate::presentation::front_attachment_offset(gait_degrees);
    if effect_pose == 9 {
        attachment_offset.x = 0.0;
    }
    PlayerEquippedElementPlan {
        socket,
        // Preserve the double product before its one final float store.
        scale: fround((if wand { 0.6_f32 as f64 } else { 1.0 }) * (1.0 + 10.0 * phase)),
        passes: equipped_orb_passes(angle, effect_pose, phase),
        overlay_after_head: effect_pose == 9 || phase > 0.1_f32 as f64,
        attachment_offset,
    }
}

/// Ordinary PlayerWizard call admission uses a separate emitter query. Its
/// no-item/Wand branch reuses the stored scaled X for Y (`0x0053B9DF`).
/// The selected-effect helper itself retains the authored full XY socket.
#[must_use]
pub fn player_element_call_angle(
    socket: crate::Vec2,
    weapon_kind: Option<PlayerWeaponKind>,
) -> usize {
    let query_y = if weapon_kind == Some(PlayerWeaponKind::Staff) { socket.y } else { socket.x };
    let reference = if weapon_kind.is_some() { 20.0 } else { 10.0 };
    let radians = fround(crate::float::atan2(socket.x, -(query_y + reference)));
    let mut degrees = fround(radians * 180.0 / crate::boneyard::terrain_surface::GROUND_PI);
    if degrees < 0.0 {
        degrees = fround(degrees + 360.0);
    }
    degrees.trunc() as usize
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
            if action.weapon_kind != Some(PlayerWeaponKind::Wand) {
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
        bare: match secondary_cast {
            Some(PlayerSecondaryCastPresentation::Spin) => Some(3),
            Some(PlayerSecondaryCastPresentation::Action(action)) => {
                if action.weapon_kind.is_none() {
                    Some(action.hand.map_or(0, |hand| hand.pose))
                } else {
                    None
                }
            }
            None if staff_action_pose.is_some() => None,
            None => Some(cast.hand_cast.as_ref().map_or_else(
                || {
                    if cast.channel_active && cast.action_tick.is_some_and(|tick| tick > 0.0) {
                        2
                    } else {
                        0
                    }
                },
                |hand| hand.pose(),
            )),
        },
    }
}

#[must_use]
pub fn player_character_staff_is_front(heading_index: usize, attachment_pose: usize) -> bool {
    let heading = heading_index % 24;
    // Each pose bank's front facings form one contiguous range, boundary facings included.
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
    equipped_orb_passes(heading_degrees, attachment_pose, overlay_effect_phase)
}

fn equipped_orb_passes(
    heading_degrees: usize,
    attachment_pose: usize,
    overlay_effect_phase: f64,
) -> PlayerStaffOrbPasses {
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

/// One of the two BadGuys record-7 passes drawn around a wizard while Damage x4 lasts.
/// Both fade out over the final 100 ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageX4VfxLayer {
    pub alpha: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
}

#[must_use]
pub fn damage_x4_vfx_plan(
    remaining_ticks: f64,
    tick: f64,
    emitter_scale: f64,
) -> Vec<DamageX4VfxLayer> {
    if remaining_ticks <= 0.0 {
        return Vec::new();
    }
    let alpha = remaining_ticks.min(100.0) / 100.0;
    let fixed_tick = tick.floor();
    vec![
        DamageX4VfxLayer { alpha, rotation_degrees: fixed_tick, scale: 2.5 * emitter_scale },
        DamageX4VfxLayer { alpha, rotation_degrees: -0.5 * fixed_tick, scale: 2.0 * emitter_scale },
    ]
}
