//! Shared stock art and native text placements for the two player panels.

use solomon_sim::belt::{
    NativeBeltEntry, NativeBeltPotionKind, native_belt_entry_item, native_belt_potion_projection,
};
use solomon_sim::economy::{EquipmentType, HubInventoryItem};
use solomon_sim::hud::{HudRect, native_hud_control_layout};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use web_sys::{Document, HtmlElement};

use super::{PlayerPanel, PlayerPanelSnapshot, PlayerPanels, Target};
use crate::ui::{StockAtlas, append, element, style};

pub(crate) fn place(node: &HtmlElement, rect: HudRect) -> Result<(), String> {
    let css = node.style();
    for (property, value) in
        [("left", rect.x), ("top", rect.y), ("width", rect.width), ("height", rect.height)]
    {
        css.set_property(property, &format!("{value}px")).map_err(|_| "panel geometry")?;
    }
    Ok(())
}

pub(crate) fn art(
    document: &Document,
    parent: &HtmlElement,
    atlas: &StockAtlas,
    record: usize,
    center: [f64; 2],
    scale: [f64; 2],
    rotation: f64,
) -> Result<HtmlElement, String> {
    let [x, y] = center;
    // Native textures retain their logical box and trim origin. UI.3, for example,
    // contains only an arc near the edge of a 1024×768 logical sprite; centering the
    // crop itself would collapse Skillbook's eight rotating seals into a small cluster.
    let node = atlas.sprite(document, record, "panel-art")?;
    node.style().set_property("position", "absolute").map_err(|_| "art positioning")?;
    node.style().set_property("left", &format!("{x}px")).map_err(|_| "art x")?;
    node.style().set_property("top", &format!("{y}px")).map_err(|_| "art y")?;
    node.style()
        .set_property(
            "transform",
            &format!("translate(-50%,-50%) rotate({rotation}deg) scale({},{})", scale[0], scale[1]),
        )
        .map_err(|_| "art transform")?;
    append(parent, &node)?;
    Ok(node)
}

pub(crate) fn text(
    document: &Document,
    parent: &HtmlElement,
    text: &str,
    x: f64,
    baseline: f64,
    class: &str,
    color: &str,
) -> Result<HtmlElement, String> {
    let node = element(document, "span", &format!("panel-text {class}"))?;
    node.set_text_content(Some(text));
    style(&node, &format!("left:{x}px;top:{baseline}px;color:{color}"))?;
    append(parent, &node)?;
    Ok(node)
}

pub(crate) fn bitmap_text(
    document: &Document,
    parent: &HtmlElement,
    spec: &NativeUiTextSpec<'_>,
) -> Result<HtmlElement, String> {
    let node = crate::ui::text::native_text(document, spec, "panel-bitmap-text")?;
    append(parent, &node)?;
    Ok(node)
}

pub(crate) fn button(
    document: &Document,
    parent: &HtmlElement,
    action: &str,
    label: &str,
    rect: HudRect,
) -> Result<HtmlElement, String> {
    let node = element(document, "button", "panel-action")?;
    node.set_attribute("type", "button").map_err(|_| "panel button type")?;
    node.set_attribute("data-panel-action", action).map_err(|_| "panel button action")?;
    node.set_attribute("aria-label", label).map_err(|_| "panel button label")?;
    place(&node, rect)?;
    append(parent, &node)?;
    Ok(node)
}

pub(crate) use crate::ui::tint;

impl PlayerPanels {
    pub(crate) fn draw_item(
        &self,
        parent: &HtmlElement,
        item: &HubInventoryItem,
        x: f64,
        y: f64,
        quantity: u64,
        mod_assets: Option<&crate::mod_presentation_assets::ModPresentationAssets>,
    ) -> Result<(), String> {
        if let Some(icon) = crate::ui::mod_icon::mod_item_icon(
            &self.document,
            mod_assets,
            item,
            solomon_sim::Vec2::new(x, y),
        )? {
            append(parent, &icon)?;
        } else {
            let (translation, rotation) = match item.equipment_type {
                Some(EquipmentType::Amulet) => ([0.0, -5.0], 0.0),
                Some(EquipmentType::Staff) => ([-22.94306, 32.76608], 35.0),
                Some(EquipmentType::Wand) => ([0.0, 0.0], 45.0),
                _ => ([0.0, 0.0], 0.0),
            };
            let tints = item.clothing_tints();
            for (layer, record) in item.icon_records.iter().enumerate() {
                let icon = art(
                    &self.document,
                    parent,
                    &self.inventory,
                    *record,
                    [x + translation[0], y + translation[1]],
                    [1.0, 1.0],
                    rotation,
                )?;
                if matches!(item.equipment_type, Some(EquipmentType::Hat | EquipmentType::Robe))
                    && let Some(color) = tints.and_then(|colors| colors.get(layer).copied())
                {
                    tint(&self.document, &icon, color)?;
                }
            }
        }
        if quantity > 1 {
            bitmap_text(
                &self.document,
                parent,
                &NativeUiTextSpec {
                    tint: 0xf4e5b4,
                    ..NativeUiTextSpec::new(
                        NativeUiFontName::Medium,
                        &quantity.to_string(),
                        x + 25.0,
                        y + 18.0,
                    )
                },
            )?;
        }
        Ok(())
    }

    pub(crate) fn draw_footer(&self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        let hud = native_hud_control_layout(1600.0, 900.0, Some(1.0));
        for (record, rect, action, label) in [
            (
                47,
                hud.backpack,
                if self.panel == Some(PlayerPanel::Inventory) { "close" } else { "inventory" },
                "Open or close inventory",
            ),
            (
                48,
                hud.tome,
                if self.panel == Some(PlayerPanel::Skills) { "close" } else { "skills" },
                "Open or close skills",
            ),
        ] {
            art(
                &self.document,
                &self.content,
                &self.ui,
                record,
                [rect.x + 29.0, rect.y + 31.0],
                [1.0, 1.0],
                0.0,
            )?;
            button(&self.document, &self.content, action, label, rect)?;
        }
        art(&self.document, &self.content, &self.ui, 82, [800.5, 872.0], [1.0, 1.0], 0.0)?;
        for (visible_slot, rect) in hud.belt.iter().enumerate() {
            let slot = self.hotbar.slot(visible_slot);
            let center_x = rect.x + rect.width / 2.0;
            let center_y = rect.y + rect.height / 2.0;
            art(&self.document, &self.content, &self.ui, 2, [center_x, center_y], [1.0, 1.0], 0.0)?;
            let entry = snapshot.belt.entries[slot];
            if let Some(NativeBeltEntry::Skill { skill_id }) = entry {
                art(
                    &self.document,
                    &self.content,
                    &self.skills,
                    snapshot.book.skill_icon_record(skill_id),
                    [center_x, center_y],
                    [1.0, 1.0],
                    0.0,
                )?;
            } else if let Some(entry) = entry {
                let item = native_belt_entry_item(entry, snapshot.economy);
                let quantity = match entry {
                    NativeBeltEntry::HealthPotion => {
                        native_belt_potion_projection(
                            &snapshot.economy.backpack,
                            NativeBeltPotionKind::Health,
                        )
                        .count
                    }
                    NativeBeltEntry::ManaPotion => {
                        native_belt_potion_projection(
                            &snapshot.economy.backpack,
                            NativeBeltPotionKind::Mana,
                        )
                        .count
                    }
                    _ => item.map_or(0, |item| u64::from(item.quantity)),
                };
                if let Some(item) = item {
                    self.draw_item(
                        &self.content,
                        item,
                        center_x,
                        center_y,
                        1,
                        snapshot.mod_assets,
                    )?;
                    if quantity > 1 {
                        bitmap_text(
                            &self.document,
                            &self.content,
                            &NativeUiTextSpec {
                                align: NativeUiTextAlign::Left,
                                tint: 0xf4e5b4,
                                ..NativeUiTextSpec::new(
                                    NativeUiFontName::Medium,
                                    &quantity.to_string(),
                                    center_x + 20.0,
                                    center_y + 22.0,
                                )
                            },
                        )?;
                    }
                }
            }
            let action = button(
                &self.document,
                &self.content,
                &format!("belt:{slot}"),
                &format!(
                    "Belt {}, {}",
                    visible_slot + 1,
                    if entry.is_some() {
                        "assign or drag away to remove"
                    } else {
                        "empty, assign selected skill or item"
                    }
                ),
                *rect,
            )?;
            action
                .set_attribute(
                    "data-target",
                    if self.target_belt == Some(slot) { "true" } else { "false" },
                )
                .map_err(|_| "belt target")?;
            action
                .set_attribute("data-belt-index", &slot.to_string())
                .map_err(|_| "modal belt authority index")?;
            action
                .set_attribute("data-visible-belt-slot", &visible_slot.to_string())
                .map_err(|_| "modal belt visible slot")?;
        }
        if self.notice.is_none() && self.dye.is_none() && self.rename_sack.is_none() {
            append(&self.content, &self.hotbar_controls.root)?;
        }
        art(&self.document, &self.content, &self.ui, 21, [38.0, 868.0], [1.0, 1.0], 0.0)?;
        text(
            &self.document,
            &self.content,
            &snapshot.economy.gold.to_string(),
            48.0,
            870.0,
            "panel-left panel-gold",
            "#d9ba70",
        )?;
        if matches!(
            self.panel,
            Some(
                PlayerPanel::Inventory
                    | PlayerPanel::Service(solomon_sim::economy::HubTraderId::Luthacus)
            )
        ) {
            art(&self.document, &self.content, &self.ui, 75, [1562.0, 868.0], [1.0, 1.0], 0.0)?;
            button(
                &self.document,
                &self.content,
                "unforge",
                "Unforge selected item",
                HudRect::new(1500.0, 800.0, 100.0, 100.0),
            )?;
        }
        Ok(())
    }

    pub(crate) fn update_dragger(&self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        self.dragger.set_text_content(None);
        let Some(press) = self.press.filter(|press| press.dragging) else {
            return Ok(());
        };
        let target = press.target;
        style(&self.dragger, &format!("left:{}px;top:{}px", self.pointer.x, self.pointer.y))?;
        if let Target::Skill(id) = target {
            art(
                &self.document,
                &self.dragger,
                &self.skills,
                snapshot.book.skill_icon_record(id),
                [0.0, 0.0],
                [1.25, 1.25],
                0.0,
            )?;
        } else if let Target::Item(id) = target
            && let Some(item) = solomon_sim::belt::native_belt_owned_item(snapshot.economy, id)
        {
            self.draw_item(&self.dragger, item, 0.0, 0.0, 1, snapshot.mod_assets)?;
        }
        Ok(())
    }
}
