//! Wizard layer offsets, ported from `player-character-presentation.ts`.
//!
//! The head bobs against the gait phase and sways across the facing; the fixed robe (hands)
//! and the front staff attachment sway with half the gait phase.

use crate::float;
use std::f64::consts::PI;

use crate::collision::Vec2;
use crate::combat::PlayerCombatState;
use crate::float::fround;
use crate::secondary::NativeSecondaryPlayerState;

/// `renderer/player-material.ts`: modifier insertion order changes overlapping
/// poison/cold colours; Stoneskin composes afterwards, before equipment dye.
#[must_use]
pub fn native_player_material_tint(
    world_tint: u32,
    secondary: Option<&NativeSecondaryPlayerState>,
    status: &PlayerCombatState,
) -> u32 {
    let poison = ([0.1_f32 as f64, 0.5, 0.1_f32 as f64], status.poison_ticks_remaining > 0);
    let cold = ([0.5, 1.0, 1.0], status.cold_slow_ticks_remaining > 0);
    let modifiers = if status.poison_before_cold { [poison, cold] } else { [cold, poison] };
    let mut material = [1.0; 3];
    for (colour, active) in modifiers {
        if !active {
            continue;
        }
        for index in 0..3 {
            material[index] = fround((material[index] + colour[index]) * 0.5);
        }
    }
    let stoneskin = if secondary.is_some_and(|secondary| secondary.stoneskin_ticks_remaining > 0) {
        0.5
    } else {
        1.0
    };
    let channel = |index: usize, shift: u32| {
        (f64::from((world_tint >> shift) & 255) * material[index] * stoneskin).round() as u32
    };
    (channel(0, 16) << 16) | (channel(1, 8) << 8) | channel(2, 0)
}

/// Native packed material colours round each multiplied byte before submission.
#[must_use]
pub fn multiply_native_tints(first: u32, second: u32) -> u32 {
    let channel = |shift: u32| {
        (f64::from((first >> shift) & 255) * f64::from((second >> shift) & 255) / 255.0).round()
            as u32
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

/// `native-texture-color.ts`: the hit pass selects diffuse red while retaining
/// texture alpha. Its lighting scale is distinct from the ordinary material tint.
#[must_use]
pub fn native_puppet_hit_tint(complex_lighting: bool, main_tint: u32) -> u32 {
    let red = if complex_lighting { 0.65_f32 as f64 } else { 1.0 };
    ((f64::from((main_tint >> 16) & 255) * red).trunc() as u32) << 16
}

/// `selectedPrimaryId === -1` uses generic attachment pose 4 and the native robe's pose 13
/// (`NATIVE_UNSELECTED_PRIMARY_*` in `player-character-presentation.ts`).
pub const NATIVE_UNSELECTED_PRIMARY_ATTACHMENT_POSE: usize = 4;
pub const NATIVE_UNSELECTED_PRIMARY_ROBE_FIXED_POSE: usize = 13;

/// The living College wizard's layer selection before a primary spell is selected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WizardDrawPlan {
    pub attachment_pose: usize,
    pub fixed_robe_pose: usize,
    pub robe_pose: usize,
    pub fixed_robe_offset: Vec2,
    pub front_attachment_offset: Vec2,
    pub head_offset: Vec2,
}

/// The unselected-primary branch of `createPlayerCharacterDrawPlan`, pinned across all
/// facings and representative gait/walk phases by `wizard-attachments-goldens.json`.
#[must_use]
pub fn wizard_draw_plan(
    heading_index: usize,
    walk_cycle_primary: f64,
    gait_degrees: f64,
) -> WizardDrawPlan {
    WizardDrawPlan {
        attachment_pose: NATIVE_UNSELECTED_PRIMARY_ATTACHMENT_POSE,
        fixed_robe_pose: NATIVE_UNSELECTED_PRIMARY_ROBE_FIXED_POSE,
        robe_pose: walk_cycle_primary.trunc() as usize,
        fixed_robe_offset: fixed_robe_offset(gait_degrees),
        front_attachment_offset: front_attachment_offset(gait_degrees),
        head_offset: head_offset(heading_index, gait_degrees),
    }
}

/// Offset of the hat and face layers for a facing and gait phase, in world units.
#[must_use]
pub fn head_offset(heading_index: usize, gait_degrees: f64) -> Vec2 {
    let gait_radians = gait_degrees * PI / 180.0;
    let perpendicular_radians = (heading_index as f64 * 15.0 + 90.0) * PI / 180.0;
    let lateral = -float::cos(gait_radians) * 0.5;
    Vec2::new(
        float::sin(perpendicular_radians) * lateral,
        -float::cos(perpendicular_radians) * lateral - float::sin(gait_radians).abs() * 1.5,
    )
}

/// Offset of the fixed robe layers (hands and sleeves) for a gait phase.
#[must_use]
pub fn fixed_robe_offset(gait_degrees: f64) -> Vec2 {
    let half_gait_radians = gait_degrees * 0.5 * PI / 180.0;
    Vec2::new(float::sin(half_gait_radians).abs(), 0.0)
}

/// Offset of the staff drawn in front of the robe.
#[must_use]
pub fn front_attachment_offset(gait_degrees: f64) -> Vec2 {
    Vec2::new(fixed_robe_offset(gait_degrees).x, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_standing_wizard_facing_north_carries_its_head_half_a_unit_left() {
        let offset = head_offset(0, 0.0);
        assert!((offset.x + 0.5).abs() < 1e-12, "{offset:?}");
        assert!(offset.y.abs() < 1e-12, "{offset:?}");
    }

    #[test]
    fn mid_stride_the_head_drops_and_the_hands_sway() {
        let offset = head_offset(6, 90.0);
        assert!((offset.y + 1.5).abs() < 1e-12, "{offset:?}");
        assert!(offset.x.abs() < 1e-12, "{offset:?}");
        assert!((fixed_robe_offset(180.0).x - 1.0).abs() < 1e-12);
        assert_eq!(front_attachment_offset(0.0), Vec2::new(0.0, 1.0));
    }
}
