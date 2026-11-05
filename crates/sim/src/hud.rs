//! Screen-space HUD rules from Website `native-hud-layout.ts` and
//! `native-hud-presentation.ts`. `tests/hud_input_goldens.rs` replays their oracle.

pub mod belt_pull_off;
pub mod mobile;
pub mod quickbar;
pub mod skill_selector;

pub const HUD_BASE_HEALTH: f64 = 50.0;
pub const HUD_BASE_MANA: f64 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl HudRect {
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudControlLayout {
    pub backpack: HudRect,
    pub belt: [HudRect; 8],
    pub tome: HudRect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudHealthPresentation {
    pub core_width: f64,
    pub fill_progress: f64,
    pub fill_width: f64,
    pub shield_progress: f64,
    pub shield_width: f64,
    pub track_width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudManaPresentation {
    pub core_width: f64,
    pub fill_progress: f64,
    pub fill_width: f64,
    pub reserve_progress: f64,
    pub reserve_width: f64,
    pub track_width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudHealthLayerKind {
    Health,
    Shield,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudHealthLayer {
    pub kind: HudHealthLayerKind,
    pub progress: f64,
    pub record: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HudBeltEntry {
    Skill { record: usize, unavailable: bool },
    HealthPotion { count: u32 },
    ManaPotion { count: u32 },
    Item { icon: HudItemIcon, count: u32 },
}

/// The stock item artwork needed by a belt painter, without inventory ownership or contents.
#[derive(Clone, Debug, PartialEq)]
pub struct HudItemIcon {
    pub records: Vec<usize>,
    pub tints: [u32; 2],
    pub equipment_type: Option<crate::economy::EquipmentType>,
}

impl HudItemIcon {
    #[must_use]
    pub fn from_item(
        item: &crate::economy::HubInventoryItem,
        element: crate::profile::WizardElement,
    ) -> Self {
        Self {
            records: item.icon_records.clone(),
            tints: inventory_item_icon_tints(item, element),
            equipment_type: item.equipment_type,
        }
    }
}

/// `NativeBeltItemIcon` keeps explicit garment colors before recipe and element
/// defaults. A missing tint lane is untinted texture, including Combinator trim.
#[must_use]
pub fn inventory_item_icon_tints(
    item: &crate::economy::HubInventoryItem,
    element: crate::profile::WizardElement,
) -> [u32; 2] {
    use crate::economy::{DOWSING_EQUIPMENT_RECIPES, EquipmentType};
    if !matches!(item.equipment_type, Some(EquipmentType::Hat | EquipmentType::Robe)) {
        return [0xffffff; 2];
    }
    item.icon_tints
        .or_else(|| {
            item.recipe_index
                .and_then(|index| DOWSING_EQUIPMENT_RECIPES.get(index))
                .map(|recipe| recipe.icon_tints)
        })
        .unwrap_or([
            Some(crate::appearance::equipment::wizard_icon_primary_tint(element)),
            Some(0xffffff),
        ])
        .map(|tint| tint.unwrap_or(0xffffff))
}

/// Inventory icon transforms from Website `HUB_ITEM_ICON_TRANSFORMS`, shared by
/// the gameplay belt, Inventory and its companions, and the mobile HUD preview.
#[must_use]
pub const fn inventory_item_icon_transform(
    equipment_type: Option<crate::economy::EquipmentType>,
) -> ([f64; 2], f64) {
    use crate::economy::EquipmentType;
    match equipment_type {
        Some(EquipmentType::Amulet) => ([0.0, -5.0], 0.0),
        Some(EquipmentType::Staff) => ([-22.94306, 32.76608], 35.0),
        Some(EquipmentType::Wand) => ([0.0, 0.0], 45.0),
        _ => ([0.0, 0.0], 0.0),
    }
}

/// Values supplied by the local simulation or an authoritative runtime, never invented by
/// the view. Icon records are stock Skills/Inventory records, not skill identifiers.
#[derive(Clone, Debug, PartialEq)]
pub struct HudState {
    pub current_health: f64,
    pub maximum_health: f64,
    pub current_mana: f64,
    pub maximum_mana: f64,
    pub shield_current: f64,
    pub shield_maximum: f64,
    pub reserved_mana: f64,
    pub poisoned: bool,
    pub experience_progress: f64,
    pub primary_icon_record: usize,
    pub concentration_icon_records: [Option<usize>; 2],
    pub belt: [Option<HudBeltEntry>; 8],
    pub gold: u32,
}

impl HudState {
    /// Website `createPlayerCombat`, `createPlayerSkillBook` (Ether), `createNativePlayerBelt`
    /// and `createHubEconomy`: level one, Magic Missile/Call Leviathan, one potion of each, 500 gold.
    #[must_use]
    pub const fn new_game() -> Self {
        Self {
            current_health: HUD_BASE_HEALTH,
            maximum_health: HUD_BASE_HEALTH,
            current_mana: HUD_BASE_MANA,
            maximum_mana: HUD_BASE_MANA,
            shield_current: 0.0,
            shield_maximum: 0.0,
            reserved_mana: 0.0,
            poisoned: false,
            experience_progress: 0.0,
            primary_icon_record: 35,
            concentration_icon_records: [None; 2],
            belt: [
                Some(HudBeltEntry::Skill { record: 38, unavailable: true }),
                None,
                None,
                Some(HudBeltEntry::HealthPotion { count: 1 }),
                Some(HudBeltEntry::ManaPotion { count: 1 }),
                None,
                None,
                None,
            ],
            gold: 500,
        }
    }
}

/// `nativeHudControlLayout` and `nativeHudModalSlideLayout`. `None` is the resting writer;
/// a modal writer at progress zero starts the belt half a pixel lower than the resting HUD.
#[must_use]
pub fn hud_control_layout(
    width: f64,
    height: f64,
    modal_progress: Option<f64>,
) -> HudControlLayout {
    assert!(
        width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0,
        "HUD needs a positive back-buffer size"
    );
    let slide = modal_progress.map_or(0.0, |progress| {
        assert!((0.0..=1.0).contains(&progress), "HUD modal progress must be within [0, 1]");
        15.0 * progress
    });
    let control_top = height - 75.0 + slide;
    let belt_top = control_top + if modal_progress.is_some() { 8.0 } else { 7.5 };
    let backpack = HudRect::new(width / 2.0 - 69.5, control_top, 58.0, 62.0);
    let tome = HudRect::new(width / 2.0 + 10.5, control_top, 58.0, 62.0);
    let belt = std::array::from_fn(|index| {
        let center = if index < 4 {
            backpack.x + 29.0 - 265.0 + 60.0 * index as f64
        } else {
            tome.x + 29.0 + 85.0 + 60.0 * (index - 4) as f64
        };
        HudRect::new(center - 26.5, belt_top, 53.0, 53.0)
    });
    HudControlLayout { backpack, belt, tome }
}

#[must_use]
pub fn health_hud_presentation(
    current: f64,
    maximum: f64,
    shield_current: f64,
    shield_maximum: f64,
) -> HudHealthPresentation {
    let core_width = 2.0 * (HUD_BASE_HEALTH + 0.25 * (maximum - HUD_BASE_HEALTH));
    let fill_progress = clamp_unit(current / maximum).powi(2);
    let shield_progress =
        if shield_maximum > 0.0 { clamp_unit(shield_current / shield_maximum) } else { 0.0 };
    HudHealthPresentation {
        core_width,
        fill_progress,
        fill_width: core_width * fill_progress,
        shield_progress,
        shield_width: core_width * shield_progress,
        track_width: core_width + 10.0,
    }
}

#[must_use]
pub fn health_hud_layers(health: f64, shield: f64, poisoned: bool) -> Vec<HudHealthLayer> {
    let record = if poisoned { 52 } else { 26 };
    let health =
        HudHealthLayer { kind: HudHealthLayerKind::Health, progress: clamp_unit(health), record };
    let shield =
        HudHealthLayer { kind: HudHealthLayerKind::Shield, progress: clamp_unit(shield), record };
    if shield.progress == 0.0 {
        return vec![health];
    }
    if shield.progress <= health.progress { vec![shield, health] } else { vec![health, shield] }
}

#[must_use]
pub fn mana_hud_presentation(current: f64, maximum: f64, reserved: f64) -> HudManaPresentation {
    let core_width = HUD_BASE_MANA + 0.25 * (maximum - HUD_BASE_MANA);
    let fill_progress = clamp_unit(current / maximum);
    let reserve_progress = clamp_unit(reserved / maximum);
    HudManaPresentation {
        core_width,
        fill_progress,
        fill_width: core_width * fill_progress,
        reserve_progress,
        reserve_width: core_width * reserve_progress,
        track_width: core_width + 10.0,
    }
}

#[must_use]
pub fn clamp_unit(value: f64) -> f64 {
    if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::{
        DOWSING_EQUIPMENT_RECIPES, EquipmentType, create_equipment_inventory_item,
    };
    use crate::profile::WizardElement;

    #[test]
    fn half_health_uses_quarter_fill_while_half_mana_uses_half_fill() {
        assert_eq!(health_hud_presentation(25.0, 50.0, 0.0, 0.0).fill_width, 25.0);
        assert_eq!(mana_hud_presentation(50.0, 100.0, 0.0).fill_width, 50.0);
    }

    #[test]
    fn belt_item_art_retains_every_authored_equipment_layer_and_tint() {
        for recipe in &DOWSING_EQUIPMENT_RECIPES {
            let item = create_equipment_inventory_item(recipe, 100);
            let icon = HudItemIcon::from_item(&item, WizardElement::Ether);
            assert_eq!(icon.records, recipe.icon_records, "{} records", recipe.name);
            assert_eq!(
                icon.equipment_type,
                Some(recipe.equipment_type),
                "{} transform",
                recipe.name
            );
            let expected_tints =
                if matches!(recipe.equipment_type, EquipmentType::Hat | EquipmentType::Robe) {
                    recipe.icon_tints.map(|tint| tint.unwrap_or(0xffffff))
                } else {
                    [0xffffff; 2]
                };
            assert_eq!(icon.tints, expected_tints, "{} tints", recipe.name);
        }
    }

    #[test]
    fn belt_garment_art_retains_dye_before_recipe_and_element_defaults() {
        for kind in [EquipmentType::Hat, EquipmentType::Robe] {
            let recipe = DOWSING_EQUIPMENT_RECIPES
                .iter()
                .find(|recipe| recipe.equipment_type == kind)
                .unwrap();
            let mut item = create_equipment_inventory_item(recipe, 100);
            item.icon_tints = Some([Some(0x123456), Some(0xabcdef)]);
            assert_eq!(
                HudItemIcon::from_item(&item, WizardElement::Ether).tints,
                [0x123456, 0xabcdef]
            );
            for (source, expected) in [
                ([Some(0x123456), None], [0x123456, 0xffffff]),
                ([None, Some(0xabcdef)], [0xffffff, 0xabcdef]),
            ] {
                item.icon_tints = Some(source);
                assert_eq!(HudItemIcon::from_item(&item, WizardElement::Ether).tints, expected);
            }
            item.icon_tints = None;
            item.recipe_index = None;
            for (element, tint) in [
                (WizardElement::Air, 0xa0c3c3),
                (WizardElement::Earth, 0x90b390),
                (WizardElement::Ether, 0x886688),
                (WizardElement::Fire, 0x998077),
                (WizardElement::Water, 0x5e6e81),
            ] {
                assert_eq!(HudItemIcon::from_item(&item, element).tints, [tint, 0xffffff]);
            }
        }
    }

    #[test]
    fn item_icon_transforms_cover_every_equipment_family_and_plain_items() {
        for (kind, offset, rotation) in [
            (Some(EquipmentType::Amulet), [0.0, -5.0], 0.0),
            (Some(EquipmentType::Hat), [0.0, 0.0], 0.0),
            (Some(EquipmentType::Ring), [0.0, 0.0], 0.0),
            (Some(EquipmentType::Robe), [0.0, 0.0], 0.0),
            (Some(EquipmentType::Staff), [-22.94306, 32.76608], 35.0),
            (Some(EquipmentType::Wand), [0.0, 0.0], 45.0),
            (None, [0.0, 0.0], 0.0),
        ] {
            assert_eq!(inventory_item_icon_transform(kind), (offset, rotation));
        }
    }
}
