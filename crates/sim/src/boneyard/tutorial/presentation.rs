//! Tutorial instructions, HUD access, forced entrance movement and authored reward items.
//! Reference: `core-kernels/native-tutorial.ts`.

use crate::Vec2;
use crate::economy::{EquipmentType, HubInventoryItem, HubItemKind, base_item};
use crate::equipment::NativeEquipmentEffect;
use crate::float::fround;

use super::{
    NativeTutorialStage as Stage, NativeTutorialState, advance_native_tutorial_intro_blend,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeTutorialHudAccess {
    pub combat: bool,
    pub inventory: bool,
    pub quickbar: bool,
    pub skills: bool,
    pub spell: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTutorialInputMode {
    Desktop,
    Mobile,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeTutorialBindingLabels<'a> {
    pub inventory: &'a str,
    pub move_down: &'a str,
    pub move_left: &'a str,
    pub move_right: &'a str,
    pub move_up: &'a str,
    pub potion: &'a str,
    pub secondary: &'a str,
    pub skills: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeTutorialPresentation {
    pub heading: Option<String>,
    pub hud: NativeTutorialHudAccess,
    pub subheading: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeTutorialInstructionBaselines {
    pub heading: f64,
    pub subheading: Option<f64>,
}

#[must_use]
pub fn native_tutorial_hud_access(state: &NativeTutorialState) -> NativeTutorialHudAccess {
    let stage = state.stage.id();
    NativeTutorialHudAccess {
        combat: !state.active || stage == 14 || stage >= 15,
        inventory: !state.active || stage >= 9,
        quickbar: !state.active || stage >= 5,
        skills: !state.active || stage >= 12,
        spell: !state.active || stage >= 5,
    }
}

#[must_use]
pub fn native_tutorial_presentation(
    state: &NativeTutorialState,
    bindings: NativeTutorialBindingLabels<'_>,
    input_mode: NativeTutorialInputMode,
) -> NativeTutorialPresentation {
    let hud = native_tutorial_hud_access(state);
    let present = |heading: Option<&str>, subheading: Option<String>| NativeTutorialPresentation {
        heading: heading.map(str::to_owned),
        subheading,
        hud,
    };
    if state.intro_active {
        return present(None, None);
    }
    let mobile = input_mode == NativeTutorialInputMode::Mobile;
    match state.stage {
        Stage::Movement => {
            if state.movement_instruction_acknowledged {
                return present(None, None);
            }
            if mobile {
                present(
                    Some("USE THE LEFT JOYSTICK\nTO MOVE THE WIZARD"),
                    Some("Find and confront Solomon Dark".to_owned()),
                )
            } else {
                present(
                    Some("USE YOUR KEYBOARD\nTO MOVE THE WIZARD"),
                    Some(format!(
                        "Move with {}, {}, {}, and {}",
                        bindings.move_up,
                        bindings.move_left,
                        bindings.move_down,
                        bindings.move_right
                    )),
                )
            }
        }
        Stage::ThrowMissiles => present(
            Some(if mobile {
                "USE THE RIGHT JOYSTICK\nTO THROW MAGIC MISSILES"
            } else {
                "POINT AND CLICK YOUR MOUSE\nTO THROW MAGIC MISSILES"
            }),
            Some("Defeat all evil emanations".to_owned()),
        ),
        Stage::CastAcidRain => present(
            Some("A SECONDARY SPELL IS READY"),
            Some(format!("Click here or press '{}' to cast 'ACID RAIN'", bindings.secondary)),
        ),
        Stage::OpenInventory => present(
            Some("ACCESS YOUR INVENTORY"),
            Some(format!(
                "Click here or press '{}' to open the inventory screen",
                bindings.inventory
            )),
        ),
        Stage::ClubEnemies => present(
            Some("WALK INTO ENEMIES TO CLUB THEM"),
            Some("This requires an equipped staff".to_owned()),
        ),
        Stage::OpenSkills => present(
            Some("ACCESS YOUR SKILLS"),
            Some(format!("Click here or press '{}' to open the skill screen", bindings.skills)),
        ),
        Stage::DrinkPotion => present(
            Some("DRINK POTION"),
            Some(format!(
                "Click here or press '{}' to drink\nthis potion and restore your health.",
                bindings.potion
            )),
        ),
        Stage::Survive => present(state.active.then_some("SURVIVE"), None),
        _ => present(None, None),
    }
}

#[must_use]
pub fn native_tutorial_instruction_baselines(
    stage: Stage,
    viewport_height: f64,
) -> Option<NativeTutorialInstructionBaselines> {
    let height =
        if viewport_height.is_finite() && viewport_height > 0.0 { viewport_height } else { 900.0 };
    let (heading, subheading) = match stage {
        Stage::Movement | Stage::ThrowMissiles => (100.0, Some(170.0)),
        Stage::CastAcidRain | Stage::OpenInventory | Stage::OpenSkills | Stage::DrinkPotion => {
            (height - 170.0, Some(height - 140.0))
        }
        Stage::ClubEnemies => (80.0, Some(110.0)),
        Stage::Survive => (200.0, None),
        _ => return None,
    };
    Some(NativeTutorialInstructionBaselines { heading, subheading })
}

#[must_use]
pub fn native_tutorial_forced_velocity(state: &NativeTutorialState) -> Option<Vec2> {
    // `player-character.ts::PLAYER_CHARACTER_STEADY_SPEED`, distinct from the lane cap.
    const STEADY_SPEED: f64 = 100.0;
    if state.intro_active {
        let delay_after_tick = state.intro_delay_ticks_remaining.saturating_sub(1);
        let blend_after_tick = if delay_after_tick == 0 {
            advance_native_tutorial_intro_blend(state.intro_blend)
        } else {
            state.intro_blend
        };
        return (blend_after_tick > 0.8).then_some(Vec2::new(0.0, -STEADY_SPEED));
    }
    let remaining = state.intro_movement_ticks_remaining.saturating_sub(1);
    if remaining == 0 {
        return None;
    }
    let native_accumulator = fround(-(remaining as f64) / 250.0);
    Some(Vec2::new(0.0, fround(native_accumulator * STEADY_SPEED)))
}

#[must_use]
pub fn native_tutorial_amulet_item() -> HubInventoryItem {
    let mut item = base_item(0, HubItemKind::Equipment, "Sorceror's Amulet", 7003, None, &[30, 18]);
    item.equipment_type = Some(EquipmentType::Amulet);
    item.native_selector = Some(0);
    item.icon_tints = Some([Some(0xff_ffff), Some(0xff_ffff)]);
    item.native_effects =
        Some(vec![NativeEquipmentEffect { kind: 2, magnitude: 10.0, operator: 2, target: 0 }]);
    item
}

#[must_use]
pub fn native_tutorial_health_potion_item() -> HubInventoryItem {
    base_item(0, HubItemKind::HealthPotion, "Health Potion", 7001, Some(0), &[46])
}

#[must_use]
pub fn native_tutorial_amulet_identity_matches(item: &HubInventoryItem) -> bool {
    let identity = native_tutorial_amulet_item();
    item.kind == identity.kind
        && item.equipment_type == identity.equipment_type
        && item.name == identity.name
        && item.native_type_id == identity.native_type_id
        && item.native_selector == identity.native_selector
        && item.native_subtype.is_none()
        && item.quantity == 1
        && item.rarity.is_none()
        && item.recipe_index.is_none()
        && item.generated_level.is_none()
        && item.icon_records == identity.icon_records
        && item.icon_tints == identity.icon_tints
        && item.native_effects == identity.native_effects
}
