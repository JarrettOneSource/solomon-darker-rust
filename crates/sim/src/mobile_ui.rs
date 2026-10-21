//! Website `mobile-ui-layout.ts`: the complete device-layout document and its
//! authored default geometry. The editor and browser storage use this one model.

mod geometry;

pub use geometry::*;

use std::collections::BTreeMap;

use crate::Vec2;
use crate::hud::mobile::{
    MobileQuickbarBank, mobile_quickbar_bank_layout, mobile_quickbar_slot_placement,
};

pub const MOBILE_UI_CANONICAL_WIDTH: f64 = 896.0;
pub const MOBILE_UI_CANONICAL_HEIGHT: f64 = 414.0;
pub const MOBILE_UI_LAYOUT_VERSION: u32 = 2;
pub const MOBILE_UI_SCALE_MIN: f64 = 0.4;
pub const MOBILE_UI_SCALE_MAX: f64 = 3.0;

/// Source `TouchJoystick.update` removes the rotated square's expanded DOM bounds
/// before sampling input. Rotation changes the control art, not the movement direction.
#[must_use]
pub fn mobile_ui_joystick_input_radius(bounds: MobileUiSize, rotation: f64) -> f64 {
    let radians = rotation * std::f64::consts::PI / 180.0;
    let expansion = radians.cos().abs() + radians.sin().abs();
    bounds.width.min(bounds.height) / expansion * 0.34
}

/// The knob counter-rotates within the control so it still points toward the actual
/// screen-space input vector. `render_size` is the untransformed DOM offset size.
#[must_use]
pub fn mobile_ui_joystick_knob_offset(
    movement: Vec2,
    render_size: MobileUiSize,
    rotation: f64,
) -> Vec2 {
    let radians = rotation * std::f64::consts::PI / 180.0;
    let radius = render_size.width.min(render_size.height) * 0.34;
    let cosine = (-radians).cos();
    let sine = (-radians).sin();
    Vec2::new(
        (movement.x * cosine - movement.y * sine) * radius,
        (movement.x * sine + movement.y * cosine) * radius,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
#[repr(usize)]
pub enum MobileUiElementId {
    Pause,
    Diagnostics,
    Meters,
    LeftJoystick,
    RightJoystick,
    Slot1,
    Slot2,
    Slot3,
    Slot4,
    Slot5,
    Slot6,
    Slot7,
    Slot8,
    Inventory,
    Skillbook,
    Xp,
    HealthPotion,
    ManaPotion,
}

impl MobileUiElementId {
    pub const SLOTS: [Self; 8] = [
        Self::Slot1,
        Self::Slot2,
        Self::Slot3,
        Self::Slot4,
        Self::Slot5,
        Self::Slot6,
        Self::Slot7,
        Self::Slot8,
    ];

    pub const ALL: [Self; 18] = [
        Self::Pause,
        Self::Diagnostics,
        Self::Meters,
        Self::LeftJoystick,
        Self::RightJoystick,
        Self::Slot1,
        Self::Slot2,
        Self::Slot3,
        Self::Slot4,
        Self::Slot5,
        Self::Slot6,
        Self::Slot7,
        Self::Slot8,
        Self::Inventory,
        Self::Skillbook,
        Self::Xp,
        Self::HealthPotion,
        Self::ManaPotion,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        [
            "pause",
            "diagnostics",
            "meters",
            "leftJoystick",
            "rightJoystick",
            "slot1",
            "slot2",
            "slot3",
            "slot4",
            "slot5",
            "slot6",
            "slot7",
            "slot8",
            "inventory",
            "skillbook",
            "xp",
            "healthPotion",
            "manaPotion",
        ][self as usize]
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|element| element.as_str() == id)
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        [
            "Pause",
            "FPS / Ping",
            "Health / Mana",
            "Movement Joystick",
            "Aim / Cast Joystick",
            "Slot 1",
            "Slot 2",
            "Slot 3",
            "Slot 4",
            "Slot 5",
            "Slot 6",
            "Slot 7",
            "Slot 8",
            "Inventory",
            "Skillbook",
            "XP Meter",
            "Health Potion",
            "Mana Potion",
        ][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(deny_unknown_fields))]
pub struct MobileUiElementTransform {
    pub rotation: f64,
    pub scale: f64,
    pub x: f64,
    pub y: f64,
}

impl Default for MobileUiElementTransform {
    fn default() -> Self {
        Self { rotation: 0.0, scale: 1.0, x: 50.0, y: 50.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobileUiSize {
    pub height: f64,
    pub width: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobileUiLayout {
    pub elements: [MobileUiElementTransform; 18],
}

impl MobileUiLayout {
    #[must_use]
    pub fn get(&self, id: MobileUiElementId) -> MobileUiElementTransform {
        self.elements[id as usize]
    }

    pub fn set(&mut self, id: MobileUiElementId, transform: MobileUiElementTransform) {
        self.elements[id as usize] = normalize_mobile_ui_transform(transform);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobileUiGeometry {
    pub layout: MobileUiLayout,
    pub sizes: [MobileUiSize; 18],
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobileUiLayoutState {
    pub customized: bool,
    pub layout: MobileUiLayout,
}

impl Default for MobileUiLayoutState {
    fn default() -> Self {
        Self {
            customized: false,
            layout: default_mobile_ui_geometry(
                MOBILE_UI_CANONICAL_WIDTH,
                MOBILE_UI_CANONICAL_HEIGHT,
                1.0,
            )
            .layout,
        }
    }
}

/// Only the two source root fields and the closed element IDs deserialize. The
/// version reader also accepts JSON `2.0`, which JavaScript parses as number 2.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(deny_unknown_fields))]
pub struct MobileUiLayoutDocument {
    pub elements: BTreeMap<MobileUiElementId, MobileUiElementTransform>,
    #[cfg_attr(feature = "persistence", serde(deserialize_with = "document_version"))]
    pub version: u32,
}

#[cfg(feature = "persistence")]
fn document_version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    use serde::Deserialize;
    let value = f64::deserialize(deserializer)?;
    if value == 1.0 {
        Ok(1)
    } else if value == 2.0 {
        Ok(2)
    } else {
        Err(serde::de::Error::custom("unsupported mobile UI document version"))
    }
}

#[must_use]
pub fn mobile_ui_layout_from_document(document: &MobileUiLayoutDocument) -> Option<MobileUiLayout> {
    if document.version != MOBILE_UI_LAYOUT_VERSION
        || document.elements.len() != MobileUiElementId::ALL.len()
    {
        return None;
    }
    read_transforms(document, false)
}

/// Version 1 is accepted only while reading the previous local storage schema;
/// the shared-layout API deliberately requires the complete version 2 document.
#[must_use]
pub fn legacy_mobile_ui_layout(document: &MobileUiLayoutDocument) -> Option<MobileUiLayout> {
    if document.version != 1
        || document.elements.len() != 17
        || document.elements.contains_key(&MobileUiElementId::Meters)
    {
        return None;
    }
    read_transforms(document, true)
}

fn read_transforms(document: &MobileUiLayoutDocument, legacy: bool) -> Option<MobileUiLayout> {
    let mut layout = MobileUiLayoutState::default().layout;
    for id in MobileUiElementId::ALL {
        if legacy && id == MobileUiElementId::Meters {
            continue;
        }
        let transform = *document.elements.get(&id)?;
        if !valid_mobile_ui_transform(transform) {
            return None;
        }
        // Admission retains +180 and signed zero exactly. Normalization happens
        // when an editor/set operation writes a new document, as in the source.
        layout.elements[id as usize] = transform;
    }
    Some(layout)
}

#[must_use]
pub fn mobile_ui_layout_document(layout: &MobileUiLayout) -> MobileUiLayoutDocument {
    let mut elements = BTreeMap::new();
    for id in MobileUiElementId::ALL {
        elements.insert(id, normalize_mobile_ui_transform(layout.get(id)));
    }
    MobileUiLayoutDocument { elements, version: MOBILE_UI_LAYOUT_VERSION }
}

#[must_use]
pub fn normalize_mobile_ui_transform(
    transform: MobileUiElementTransform,
) -> MobileUiElementTransform {
    let rotation = finite_or(transform.rotation, 0.0);
    MobileUiElementTransform {
        rotation: ((rotation + 180.0) % 360.0 + 360.0) % 360.0 - 180.0,
        scale: source_clamp(
            finite_or(transform.scale, 1.0),
            MOBILE_UI_SCALE_MIN,
            MOBILE_UI_SCALE_MAX,
        ),
        x: source_clamp(finite_or(transform.x, 50.0), 0.0, 100.0),
        y: source_clamp(finite_or(transform.y, 50.0), 0.0, 100.0),
    }
}

#[must_use]
pub fn valid_mobile_ui_transform(transform: MobileUiElementTransform) -> bool {
    transform.x.is_finite()
        && (0.0..=100.0).contains(&transform.x)
        && transform.y.is_finite()
        && (0.0..=100.0).contains(&transform.y)
        && transform.scale.is_finite()
        && (MOBILE_UI_SCALE_MIN..=MOBILE_UI_SCALE_MAX).contains(&transform.scale)
        && transform.rotation.is_finite()
        && (-180.0..=180.0).contains(&transform.rotation)
}

#[must_use]
pub fn default_mobile_ui_geometry(width: f64, height: f64, ui_scale: f64) -> MobileUiGeometry {
    assert!(width.is_finite() && width > 0.0, "viewportWidth must be positive and finite");
    assert!(height.is_finite() && height > 0.0, "viewportHeight must be positive and finite");
    let ui_scale = finite_or(ui_scale, 1.0).clamp(0.75, 1.5);
    let display_scale = (width / 1600.0).min(height / 900.0);
    let root_scale = display_scale * ui_scale;
    let bank = mobile_quickbar_bank_layout(width / display_scale, ui_scale);
    let mut result = MobileUiGeometry {
        layout: MobileUiLayout { elements: [MobileUiElementTransform::default(); 18] },
        sizes: [MobileUiSize { width: 0.0, height: 0.0 }; 18],
    };
    let mut add =
        |id: MobileUiElementId, x: f64, y: f64, element_width: f64, element_height: f64| {
            result.layout.elements[id as usize] = MobileUiElementTransform {
                rotation: 0.0,
                scale: 1.0,
                x: x / width * 100.0,
                y: y / height * 100.0,
            };
            result.sizes[id as usize] =
                MobileUiSize { width: element_width, height: element_height };
        };
    add(MobileUiElementId::Pause, 26.0, 26.0, 44.0, 44.0);
    add(MobileUiElementId::Diagnostics, 126.865, 20.1, 69.73, 10.2);
    add(
        MobileUiElementId::Meters,
        width / 2.0,
        24.5 * root_scale,
        320.0 * root_scale,
        20.0 * root_scale,
    );
    let joystick_center = 48.0 + 237.5 / 2.0;
    let joystick_y = height - (56.0 + 237.5 / 2.0) * root_scale;
    add(
        MobileUiElementId::LeftJoystick,
        joystick_center * root_scale,
        joystick_y,
        237.5 * root_scale,
        237.5 * root_scale,
    );
    add(
        MobileUiElementId::RightJoystick,
        width - joystick_center * root_scale,
        joystick_y,
        237.5 * root_scale,
        237.5 * root_scale,
    );
    for slot in 0..8 {
        let placement = mobile_quickbar_slot_placement(slot, bank);
        let size = placement.size * root_scale;
        let x = if placement.bank == MobileQuickbarBank::Left {
            placement.inset * root_scale + size / 2.0
        } else {
            width - placement.inset * root_scale - size / 2.0
        };
        let y = height - placement.bottom * root_scale - size / 2.0;
        add(MobileUiElementId::ALL[MobileUiElementId::Slot1 as usize + slot], x, y, size, size);
    }
    add(
        MobileUiElementId::HealthPotion,
        width / 2.0 - 180.0 * root_scale,
        height - 58.0 * root_scale,
        100.0 * root_scale,
        100.0 * root_scale,
    );
    add(
        MobileUiElementId::Inventory,
        width / 2.0 - 65.0 * root_scale,
        height - 65.0 * root_scale,
        130.0 * root_scale,
        130.0 * root_scale,
    );
    add(
        MobileUiElementId::Skillbook,
        width / 2.0 + 65.0 * root_scale,
        height - 65.0 * root_scale,
        130.0 * root_scale,
        130.0 * root_scale,
    );
    add(
        MobileUiElementId::Xp,
        width / 2.0 + 4.0 * root_scale,
        height - 44.0 * root_scale,
        12.0 * root_scale,
        56.0 * root_scale,
    );
    add(
        MobileUiElementId::ManaPotion,
        width / 2.0 + 180.0 * root_scale,
        height - 58.0 * root_scale,
        100.0 * root_scale,
        100.0 * root_scale,
    );
    result
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

fn source_clamp(value: f64, minimum: f64, maximum: f64) -> f64 {
    // Math.max(0, -0) chooses positive zero. A Rust comparison-only clamp
    // otherwise retains -0 and writes a different normalized document.
    if value <= minimum {
        minimum
    } else if value >= maximum {
        maximum
    } else {
        value
    }
}
