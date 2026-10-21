//! Current Website `hub-sack-rename.ts` and `hub-sack-rename-dialog-layout.ts`.
//! Selection is borrowed; only the inventory authority validates and commits a new name.

use crate::economy::{
    HubEconomyState, HubInventoryItem, HubItemKind, find_inventory_item,
    inventory_items_at_sack_path,
};
use crate::hud::HudRect;
use crate::native_ui_text::{NativeUiFontName, native_ui_font};

pub const INVENTORY_SACK_RENAME_BUTTON: HudRect = HudRect::new(1310.0, 440.0, 250.0, 44.0);
pub const INVENTORY_SACK_RENAME_BUTTON_SCALE: f64 = 44.0 / 69.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventorySackRenameOwner {
    Backpack,
    Storage,
    Equipment,
    Unowned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventorySackRenameSelection {
    pub item_id: u64,
    pub owner: InventorySackRenameOwner,
}

#[must_use]
pub fn inventory_sack_rename_target<'a>(
    economy: &'a HubEconomyState,
    selection: Option<InventorySackRenameSelection>,
    sack_path: &[u64],
) -> Option<&'a HubInventoryItem> {
    let item = if let Some(selection) = selection {
        let inventory = match selection.owner {
            InventorySackRenameOwner::Backpack => &economy.backpack,
            InventorySackRenameOwner::Storage => &economy.storage,
            InventorySackRenameOwner::Equipment | InventorySackRenameOwner::Unowned => return None,
        };
        find_inventory_item(inventory, selection.item_id)
    } else if let Some((sack_id, parents)) = sack_path.split_last() {
        inventory_items_at_sack_path(&economy.backpack, parents)?
            .iter()
            .find(|item| item.id == *sack_id)
    } else {
        None
    };
    item.filter(|item| item.kind == HubItemKind::Sack && item.native_type_id == 7008)
}

/// A user name must not silently lose characters absent from the stock Menu atlas.
#[must_use]
pub fn inventory_sack_name_needs_browser_font(name: &str) -> bool {
    let font = native_ui_font(NativeUiFontName::Menu);
    name.chars().any(|character| character != ' ' && font.glyph(character as u32).is_none())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InventorySackRenameDialogLayout {
    pub art_scale: f64,
    pub body: HudRect,
    pub button_scale: f64,
    pub cancel: HudRect,
    pub field: HudRect,
    pub save: HudRect,
    pub title_baseline: f64,
    pub limit_baseline: f64,
}

/// UI.18's logical width and UI.8's logical height come from the same retained stock atlas.
#[must_use]
pub fn inventory_sack_rename_dialog_layout(
    width: f64,
    height: f64,
    header_width: f64,
    arrow_height: f64,
) -> InventorySackRenameDialogLayout {
    let crown = 37.0 + header_width / 2.0;
    let footer = 50.0 + arrow_height / 2.0;
    let panel_width = 600.0_f64.min(width - 32.0);
    let panel_height = 400.0_f64.min((height - 32.0 - crown - footer).max(250.0));
    let art_scale = ((height - 32.0 - panel_height) / (crown + footer)).clamp(0.01, 1.0);
    let left = (width - panel_width) / 2.0;
    let top = (height - panel_height - (crown + footer) * art_scale) / 2.0 + crown * art_scale;
    let compact = panel_height < 340.0;
    let button_width = (panel_width - 72.0) / 2.0;
    InventorySackRenameDialogLayout {
        art_scale,
        body: HudRect::new(left, top, panel_width, panel_height),
        button_scale: (button_width / 260.0).min(1.0),
        cancel: HudRect::new(
            left + panel_width - button_width - 24.0,
            top + panel_height - 84.0,
            button_width,
            56.0,
        ),
        field: HudRect::new(
            left + 32.0,
            top + if compact { 92.0 } else { 165.0 },
            panel_width - 64.0,
            44.0,
        ),
        save: HudRect::new(left + 24.0, top + panel_height - 84.0, button_width, 56.0),
        title_baseline: top + if compact { 44.0 } else { 95.0 },
        limit_baseline: top + if compact { 70.0 } else { 130.5 },
    }
}
