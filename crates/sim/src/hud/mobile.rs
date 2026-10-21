//! Source touch quickbar banks from Website `mobile-quickbar-layout.ts`.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobileQuickbarBankLayout {
    pub inset: f64,
    pub size: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobileQuickbarBank {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobileQuickbarSlotPlacement {
    pub bank: MobileQuickbarBank,
    pub bottom: f64,
    pub inset: f64,
    pub size: f64,
}

#[must_use]
pub fn mobile_quickbar_bank_layout(logical_width: f64, ui_scale: f64) -> MobileQuickbarBankLayout {
    assert!(
        logical_width.is_finite() && logical_width > 0.0,
        "logical viewport width is positive and finite"
    );
    assert!(ui_scale.is_finite() && ui_scale > 0.0, "UI scale is positive and finite");
    let available = logical_width / ui_scale / 2.0 - 230.0 - 16.0;
    let full_width = 2.0 * 100.0 + 8.0;
    if 310.0 + full_width <= available {
        return MobileQuickbarBankLayout { inset: 310.0, size: 100.0 };
    }
    let yielded_inset = available - full_width;
    let minimum_inset = 48.0 + 237.5 + 8.0;
    if yielded_inset >= minimum_inset {
        return MobileQuickbarBankLayout { inset: yielded_inset, size: 100.0 };
    }
    let fitted = (available - minimum_inset - 8.0) / 2.0;
    MobileQuickbarBankLayout { inset: minimum_inset, size: fitted.max(52.0) }
}

#[must_use]
pub fn mobile_quickbar_slot_placement(
    slot: usize,
    layout: MobileQuickbarBankLayout,
) -> MobileQuickbarSlotPlacement {
    assert!(slot < 8, "the belt has eight slots");
    assert!(layout.inset.is_finite() && layout.inset > 0.0, "bank inset is positive and finite");
    assert!(layout.size.is_finite() && layout.size > 0.0, "slot size is positive and finite");
    let bank = if slot < 4 { MobileQuickbarBank::Left } else { MobileQuickbarBank::Right };
    let index = slot % 4;
    let column = index % 2;
    let row = index / 2;
    let pitch = layout.size + 8.0;
    let outboard_column = if bank == MobileQuickbarBank::Left { column } else { 1 - column };
    MobileQuickbarSlotPlacement {
        bank,
        bottom: 62.0 + (1 - row) as f64 * pitch,
        inset: layout.inset + outboard_column as f64 * pitch,
        size: layout.size,
    }
}
