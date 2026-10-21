//! Native inventory-to-sprite projection from `core-kernels/player-equipment-appearance.ts`.
//! Inventory owns the items; this projection preserves their selector and independent tints.

use crate::economy::{EquipmentType, HubEquipmentState, HubInventoryItem};
use crate::modding::inventory::{ModItemContent, ModWearableSlot};
use crate::profile::WizardElement;
use crate::spells::casting::NativePlayerWeaponKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerEquipmentTintedSelector {
    pub primary_tint: u32,
    pub secondary_tint: u32,
    pub selector: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerWeaponAppearance {
    pub kind: NativePlayerWeaponKind,
    pub selector: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerLivingEquipmentAppearance {
    pub hat: Option<PlayerEquipmentTintedSelector>,
    pub robe: Option<PlayerEquipmentTintedSelector>,
    pub weapon: Option<PlayerWeaponAppearance>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerDeathEquipmentAppearance {
    pub hat: PlayerEquipmentTintedSelector,
    pub robe: PlayerEquipmentTintedSelector,
    pub weapon: PlayerWeaponAppearance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerModTintedEquipmentAppearance {
    pub content: ModItemContent,
    pub primary_tint: u32,
    pub secondary_tint: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerRenderableGarmentAppearance {
    Native(PlayerEquipmentTintedSelector),
    Mod(Box<PlayerModTintedEquipmentAppearance>),
}

impl PlayerRenderableGarmentAppearance {
    #[must_use]
    pub const fn tints(&self) -> [u32; 2] {
        match self {
            Self::Native(garment) => [garment.primary_tint, garment.secondary_tint],
            Self::Mod(garment) => [garment.primary_tint, garment.secondary_tint],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerRenderableWeaponAppearance {
    Native(PlayerWeaponAppearance),
    ModStaff(Box<ModItemContent>),
}

impl PlayerRenderableWeaponAppearance {
    #[must_use]
    pub const fn kind(&self) -> NativePlayerWeaponKind {
        match self {
            Self::Native(weapon) => weapon.kind,
            Self::ModStaff(_) => NativePlayerWeaponKind::Staff,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerRenderableEquipmentAppearance {
    pub hat: Option<PlayerRenderableGarmentAppearance>,
    pub robe: Option<PlayerRenderableGarmentAppearance>,
    pub weapon: Option<PlayerRenderableWeaponAppearance>,
}

/// Living custom art retains its package identity, while death uses authored stock shapes.
#[must_use]
pub fn player_living_equipment_appearance(
    element: WizardElement,
    equipment: &HubEquipmentState,
) -> PlayerRenderableEquipmentAppearance {
    let fallback = default_garment(element);
    PlayerRenderableEquipmentAppearance {
        hat: equipment
            .hat
            .as_ref()
            .map(|item| living_tinted_appearance(item, EquipmentType::Hat, fallback)),
        robe: equipment
            .robe
            .as_ref()
            .map(|item| living_tinted_appearance(item, EquipmentType::Robe, fallback)),
        weapon: equipment.weapon.as_ref().map(living_weapon_appearance),
    }
}

/// `playerLivingNativeEquipmentAppearance`: native-only consumers use deathShape
/// for custom wearables. This is also the source memorial portrait contract.
#[must_use]
pub fn player_living_native_equipment_appearance(
    element: WizardElement,
    equipment: &HubEquipmentState,
) -> PlayerLivingEquipmentAppearance {
    let fallback = default_garment(element);
    PlayerLivingEquipmentAppearance {
        hat: equipment
            .hat
            .as_ref()
            .map(|item| tinted_appearance(item, EquipmentType::Hat, fallback)),
        robe: equipment
            .robe
            .as_ref()
            .map(|item| tinted_appearance(item, EquipmentType::Robe, fallback)),
        weapon: equipment.weapon.as_ref().map(weapon_appearance),
    }
}

#[must_use]
pub fn player_death_equipment_appearance(
    element: WizardElement,
    equipment: &HubEquipmentState,
) -> PlayerDeathEquipmentAppearance {
    let living = player_living_native_equipment_appearance(element, equipment);
    PlayerDeathEquipmentAppearance {
        hat: living.hat.unwrap_or_else(|| default_garment(element)),
        robe: living.robe.unwrap_or_else(|| default_garment(element)),
        weapon: living
            .weapon
            .unwrap_or(PlayerWeaponAppearance { kind: NativePlayerWeaponKind::Staff, selector: 0 }),
    }
}

fn default_garment(element: WizardElement) -> PlayerEquipmentTintedSelector {
    PlayerEquipmentTintedSelector {
        primary_tint: native_wizard_icon_primary_tint(element),
        secondary_tint: 0xffffff,
        selector: 0,
    }
}

/// `HUB_STARTER_EQUIPMENT_PRIMARY_TINT`: shared stock sprite/icon fallback palette.
#[must_use]
pub const fn native_wizard_icon_primary_tint(element: WizardElement) -> u32 {
    match element {
        WizardElement::Air => 0xa0c3c3,
        WizardElement::Earth => 0x90b390,
        WizardElement::Ether => 0x886688,
        WizardElement::Fire => 0x998077,
        WizardElement::Water => 0x5e6e81,
    }
}

fn tinted_appearance(
    item: &HubInventoryItem,
    kind: EquipmentType,
    fallback: PlayerEquipmentTintedSelector,
) -> PlayerEquipmentTintedSelector {
    if let Some(content) = &item.mod_item_content
        && let Some(wearable) = &content.wearable
    {
        assert_eq!(wearable.slot, garment_slot(kind), "mod garment death slot");
        let (primary_tint, secondary_tint) =
            required_tints(item.icon_tints.expect("mod garment has two tint lanes"));
        return PlayerEquipmentTintedSelector {
            primary_tint,
            secondary_tint,
            selector: wearable.death_shape,
        };
    }
    if let Some(recipe) = item.recipe_index {
        let (primary_tint, secondary_tint, selector) = match (kind, recipe) {
            (EquipmentType::Robe, 1) => (0x191919, 0x80ffff, 1),
            (EquipmentType::Robe, 7) => (0xc0c0c0, 0xffffff, 2),
            (EquipmentType::Robe, 12) => (0xff19ff, 0xffffff, 0),
            (EquipmentType::Robe, 17) => (0x19ffff, 0xffffff, 2),
            (EquipmentType::Robe, 21) => (0xff0000, 0xffffff, 0),
            (EquipmentType::Robe, 25) => (0x19ff19, 0xc8ffc8, 2),
            (EquipmentType::Robe, 46) => (0xffffff, 0xffffff, 0),
            (EquipmentType::Hat, 5) => (0x191919, 0xff80ff, 0),
            (EquipmentType::Hat, 6) => (0xc0c0c0, 0xffffff, 2),
            (EquipmentType::Hat, 11) => (0xff19ff, 0xffffff, 0),
            (EquipmentType::Hat, 16) => (0x19ffff, 0xffffff, 3),
            (EquipmentType::Hat, 20) => (0xff0000, 0xffffff, 2),
            (EquipmentType::Hat, 40) => (0xffffff, 0xffffff, 3),
            _ => panic!("unsupported native {kind:?} appearance recipe {recipe}"),
        };
        assert_eq!(item.equipment_type, Some(kind), "recipe equipment type");
        let (primary_tint, secondary_tint) =
            item.icon_tints.map(required_tints).unwrap_or((primary_tint, secondary_tint));
        return PlayerEquipmentTintedSelector { primary_tint, secondary_tint, selector };
    }
    if item.native_selector.is_none() && item.icon_tints.is_none() {
        return fallback;
    }
    assert_eq!(item.equipment_type, Some(kind), "native equipment type");
    let (primary_tint, secondary_tint) =
        required_tints(item.icon_tints.expect("native garment has two tint lanes"));
    PlayerEquipmentTintedSelector {
        primary_tint,
        secondary_tint,
        selector: item.native_selector.unwrap_or(0) as usize,
    }
}

fn required_tints(tints: [Option<u32>; 2]) -> (u32, u32) {
    (
        tints[0].expect("native garment primary tint"),
        tints[1].expect("native garment secondary tint"),
    )
}

fn weapon_appearance(item: &HubInventoryItem) -> PlayerWeaponAppearance {
    if let Some(content) = &item.mod_item_content
        && let Some(wearable) = &content.wearable
    {
        assert_eq!(wearable.slot, ModWearableSlot::Staff, "mod death weapon slot");
        assert_eq!(item.equipment_type, Some(EquipmentType::Staff), "mod death weapon type");
        return PlayerWeaponAppearance {
            kind: NativePlayerWeaponKind::Staff,
            selector: wearable.death_shape,
        };
    }
    if let Some(recipe) = item.recipe_index {
        let (kind, selector) = match recipe {
            2 => (NativePlayerWeaponKind::Wand, 2),
            8 => (NativePlayerWeaponKind::Staff, 1),
            13 => (NativePlayerWeaponKind::Wand, 4),
            18 => (NativePlayerWeaponKind::Staff, 3),
            28 => (NativePlayerWeaponKind::Wand, 3),
            33 => (NativePlayerWeaponKind::Staff, 0),
            34 => (NativePlayerWeaponKind::Staff, 2),
            41..=45 => (NativePlayerWeaponKind::Wand, 5),
            _ => panic!("unsupported native weapon appearance recipe {recipe}"),
        };
        let expected = if kind == NativePlayerWeaponKind::Staff {
            EquipmentType::Staff
        } else {
            EquipmentType::Wand
        };
        assert_eq!(item.equipment_type, Some(expected), "recipe weapon type");
        return PlayerWeaponAppearance { kind, selector };
    }
    let Some(selector) = item.native_selector else {
        return PlayerWeaponAppearance { kind: NativePlayerWeaponKind::Staff, selector: 0 };
    };
    let kind = match item.equipment_type {
        Some(EquipmentType::Staff) => NativePlayerWeaponKind::Staff,
        Some(EquipmentType::Wand) => NativePlayerWeaponKind::Wand,
        _ => panic!("native weapon has Staff or Wand type"),
    };
    PlayerWeaponAppearance { kind, selector: selector as usize }
}

fn living_tinted_appearance(
    item: &HubInventoryItem,
    kind: EquipmentType,
    fallback: PlayerEquipmentTintedSelector,
) -> PlayerRenderableGarmentAppearance {
    if let Some(content) = &item.mod_item_content
        && let Some(wearable) = &content.wearable
    {
        assert_eq!(wearable.slot, garment_slot(kind), "mod living garment slot");
        let (primary_tint, secondary_tint) =
            required_tints(item.icon_tints.expect("mod garment has two tint lanes"));
        return PlayerRenderableGarmentAppearance::Mod(Box::new(
            PlayerModTintedEquipmentAppearance {
                content: content.clone(),
                primary_tint,
                secondary_tint,
            },
        ));
    }
    PlayerRenderableGarmentAppearance::Native(tinted_appearance(item, kind, fallback))
}

fn living_weapon_appearance(item: &HubInventoryItem) -> PlayerRenderableWeaponAppearance {
    if let Some(content) = &item.mod_item_content
        && let Some(wearable) = &content.wearable
    {
        assert_eq!(wearable.slot, ModWearableSlot::Staff, "mod living weapon slot");
        assert_eq!(item.equipment_type, Some(EquipmentType::Staff), "mod living weapon type");
        return PlayerRenderableWeaponAppearance::ModStaff(Box::new(content.clone()));
    }
    PlayerRenderableWeaponAppearance::Native(weapon_appearance(item))
}

fn garment_slot(kind: EquipmentType) -> ModWearableSlot {
    match kind {
        EquipmentType::Hat => ModWearableSlot::Hat,
        EquipmentType::Robe => ModWearableSlot::Robe,
        _ => panic!("garment appearance is a hat or robe"),
    }
}
