//! HubInventoryUi's local Sack navigation and pages.ts stationary viewport.
//! Paths borrow the accepted backpack; this view never moves inventory items.

use solomon_sim::audio::AudioCue;
use solomon_sim::economy::{
    HubItemKind, find_inventory_item, inventory_items_at_sack_path, project_inventory_root_slots,
};
use solomon_sim::player_panels::sack_pages::{
    SACK_PAGE_CLIP, SACK_PAGE_DURATION_MS, SackPageDirection, hub_sack_page_offsets,
};
use solomon_sim::player_panels::{INVENTORY_CAPACITY, inventory_slot_rect};
use web_sys::HtmlElement;

use super::chrome::{art, button};
use super::{PlayerPanel, PlayerPanelSnapshot, PlayerPanels, Target};
use crate::ui::{append, element, style};

pub(super) struct SackPageTransition {
    direction: SackPageDirection,
    from_path: Vec<u64>,
    to_path: Vec<u64>,
    started_at_ms: f64,
}

pub(super) struct SackPageViews {
    caption: HtmlElement,
    incoming: HtmlElement,
    outgoing: Option<HtmlElement>,
    painted_tick: Option<u32>,
}

impl PlayerPanels {
    pub(super) fn inventory_transition_locked(&self) -> bool {
        self.sack_transition.is_some() || self.inventory_flyby_active()
    }

    pub(super) fn open_inventory_sack(
        &mut self,
        item_id: u64,
        snapshot: &PlayerPanelSnapshot<'_>,
        now_ms: f64,
    ) {
        if self.sack_transition.is_some() {
            return;
        }
        let exists = self.active_items(snapshot).iter().any(|item| {
            item.id == item_id && item.kind == HubItemKind::Sack && item.native_type_id == 7008
        });
        if !exists {
            return;
        }
        let from_path = self.sack_path.clone();
        self.sack_path.push(item_id);
        self.sack_transition = Some(SackPageTransition {
            direction: SackPageDirection::Open,
            from_path,
            to_path: self.sack_path.clone(),
            started_at_ms: now_ms,
        });
        self.selected = None;
        self.press = None;
        self.sound(AudioCue::BackpackOpen);
        self.dirty = true;
    }

    /// A Back during the moving page is consumed, including while returning to
    /// root where the new path is already empty. It must not close the book.
    pub(super) fn return_from_inventory_sack(&mut self, now_ms: f64) -> bool {
        if self.sack_transition.is_some() {
            return true;
        }
        if self.sack_path.is_empty() {
            return false;
        }
        let from_path = self.sack_path.clone();
        self.sack_path.pop();
        self.sack_transition = Some(SackPageTransition {
            direction: SackPageDirection::Back,
            from_path,
            to_path: self.sack_path.clone(),
            started_at_ms: now_ms,
        });
        self.selected = None;
        self.press = None;
        self.sound(AudioCue::BackpackClose);
        self.dirty = true;
        true
    }

    pub(super) fn expire_sack_transition(&mut self, now_ms: f64) -> Result<(), String> {
        if self
            .sack_transition
            .as_ref()
            .is_some_and(|transition| now_ms - transition.started_at_ms >= SACK_PAGE_DURATION_MS)
        {
            self.clear_sack_transition()?;
            self.dirty = true;
        }
        Ok(())
    }

    pub(super) fn clear_sack_transition(&mut self) -> Result<(), String> {
        self.sack_transition = None;
        self.sack_page_views = None;
        for name in [
            "data-native-sack-page-state",
            "data-native-sack-page-ticks",
            "data-native-sack-incoming-x",
            "data-native-sack-outgoing-x",
            "data-native-sack-incoming-y",
            "data-native-sack-outgoing-y",
            "data-native-sack-clip",
            "data-native-sack-caption-visible",
        ] {
            self.stage.remove_attribute(name).map_err(|_| "Sack page disposal")?;
        }
        Ok(())
    }

    pub(super) fn draw_sack_pages(
        &mut self,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let caption = element(&self.document, "div", "panel-sack-caption")?;
        let sack = self
            .sack_path
            .last()
            .and_then(|id| find_inventory_item(&snapshot.economy.backpack, *id));
        let label = sack.map_or("Backpack", |sack| sack.name.as_str());
        caption.set_attribute("aria-label", label).map_err(|_| "Sack page caption")?;
        self.section_header(&caption, label, 800.0, 460.0, 489.0, sack.is_some())?;
        append(&self.content, &caption)?;
        let viewport = element(&self.document, "div", "panel-sack-page-viewport")?;
        style(
            &viewport,
            &format!(
                "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;overflow:hidden",
                SACK_PAGE_CLIP.x, SACK_PAGE_CLIP.y, SACK_PAGE_CLIP.width, SACK_PAGE_CLIP.height,
            ),
        )?;
        append(&self.content, &viewport)?;
        let outgoing = if let Some(transition) = &self.sack_transition {
            let page = self.draw_sack_grid(&viewport, &transition.from_path, false, snapshot)?;
            page.set_attribute("data-native-sack-page", "outgoing")
                .map_err(|_| "outgoing Sack page")?;
            Some(page)
        } else {
            None
        };
        let path = self
            .sack_transition
            .as_ref()
            .map_or(self.sack_path.as_slice(), |row| row.to_path.as_slice());
        let incoming = self.draw_sack_grid(&viewport, path, outgoing.is_none(), snapshot)?;
        incoming
            .set_attribute(
                "data-native-sack-page",
                if outgoing.is_some() { "incoming" } else { "current" },
            )
            .map_err(|_| "incoming Sack page")?;
        caption.set_hidden(self.sack_transition.is_some());
        self.sack_page_views =
            Some(SackPageViews { caption, incoming, outgoing, painted_tick: None });
        Ok(())
    }

    fn draw_sack_grid(
        &self,
        viewport: &HtmlElement,
        path: &[u64],
        interactive: bool,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<HtmlElement, String> {
        let page = element(&self.document, "div", "panel-sack-page")?;
        style(
            &page,
            &format!(
                "position:absolute;left:0;top:-{}px;width:1600px;height:900px;pointer-events:{}",
                SACK_PAGE_CLIP.y,
                if interactive { "auto" } else { "none" },
            ),
        )?;
        append(viewport, &page)?;
        let source = inventory_items_at_sack_path(&snapshot.economy.backpack, path).unwrap_or(&[]);
        let items = project_inventory_root_slots(source);
        let parent =
            path.last().and_then(|id| find_inventory_item(&snapshot.economy.backpack, *id));
        for visible_slot in 0..INVENTORY_CAPACITY {
            let rect = inventory_slot_rect(visible_slot);
            if visible_slot == 0
                && let Some(item) = parent
            {
                let back =
                    button(&self.document, &page, "back", "Return to parent inventory", rect)?;
                back.toggle_attribute_with_force("disabled", !interactive)
                    .map_err(|_| "Sack parent admission")?;
                let frame = self.inventory.raw_sprite(&self.document, 10)?;
                frame.style().set_property("opacity", "0.4").map_err(|_| "parent frame alpha")?;
                append(&back, &frame)?;
                let pad = element(&self.document, "span", "")?;
                style(
                    &pad,
                    "position:absolute;left:6px;top:6px;width:60px;height:60px;background:rgba(217,186,112,.25);pointer-events:none",
                )?;
                append(&back, &pad)?;
                art(&self.document, &back, &self.ui, 1, [36.0, 24.0], [0.65, 0.65], 0.0)?;
                let holder = element(&self.document, "span", "panel-parent-holder")?;
                style(
                    &holder,
                    "position:absolute;left:1px;top:1px;width:70px;height:67px;overflow:hidden;pointer-events:none",
                )?;
                self.draw_item(&holder, item, 35.0, 53.0, 1, snapshot.mod_assets)?;
                append(&back, &holder)?;
                continue;
            }
            let slot = visible_slot - usize::from(parent.is_some());
            let item = items.iter().find(|item| item.slot == slot).map(|item| item.item);
            let action =
                item.map_or_else(|| format!("cell:{slot}"), |item| format!("item:{}", item.id));
            let label = item.map_or_else(
                || format!("Empty backpack slot {}", slot + 1),
                |item| format!("{}, quantity {}", item.name, item.quantity),
            );
            let cell = button(&self.document, &page, &action, &label, rect)?;
            cell.set_attribute("data-inventory-slot", &visible_slot.to_string())
                .map_err(|_| "backpack slot")?;
            cell.toggle_attribute_with_force("disabled", !interactive)
                .map_err(|_| "Sack page admission")?;
            let frame = self.inventory.raw_sprite(&self.document, 10)?;
            frame.style().set_property("opacity", "0.4").map_err(|_| "backpack frame alpha")?;
            append(&cell, &frame)?;
            if let Some(item) = item.filter(|item| !self.inventory_item_hidden(item.id)) {
                self.draw_item(
                    &cell,
                    item,
                    36.0,
                    36.0,
                    u64::from(item.quantity),
                    snapshot.mod_assets,
                )?;
            }
            if interactive && item.is_some_and(|item| self.selected == Some(Target::Item(item.id)))
            {
                cell.set_attribute("data-selected", "true")
                    .map_err(|_| "selected backpack item")?;
            }
        }
        Ok(page)
    }

    pub(super) fn paint_sack_pages(&mut self, now_ms: f64) -> Result<(), String> {
        let Some(views) = &mut self.sack_page_views else {
            return Ok(());
        };
        let Some(transition) = &self.sack_transition else {
            self.stage
                .set_attribute("data-native-sack-caption-visible", "true")
                .map_err(|_| "Sack caption visibility")?;
            return Ok(());
        };
        let offsets = hub_sack_page_offsets(transition.direction, transition.started_at_ms, now_ms);
        if views.painted_tick == Some(offsets.ticks) {
            return Ok(());
        }
        views.painted_tick = Some(offsets.ticks);
        views
            .incoming
            .style()
            .set_property("transform", &format!("translateY({}px)", offsets.incoming_y))
            .map_err(|_| "incoming Sack page offset")?;
        if let Some(outgoing) = &views.outgoing {
            outgoing
                .style()
                .set_property("transform", &format!("translateY({}px)", offsets.outgoing_y))
                .map_err(|_| "outgoing Sack page offset")?;
        }
        views.caption.set_hidden(!offsets.settled);
        for (name, value) in [
            (
                "data-native-sack-page-state",
                if offsets.settled { "settled".to_owned() } else { "moving".to_owned() },
            ),
            ("data-native-sack-page-ticks", offsets.ticks.to_string()),
            ("data-native-sack-incoming-x", "0".to_owned()),
            ("data-native-sack-outgoing-x", "0".to_owned()),
            ("data-native-sack-incoming-y", offsets.incoming_y.to_string()),
            ("data-native-sack-outgoing-y", offsets.outgoing_y.to_string()),
            ("data-native-sack-clip", "0,492,1600,305".to_owned()),
            ("data-native-sack-caption-visible", offsets.settled.to_string()),
        ] {
            self.stage.set_attribute(name, &value).map_err(|_| "Sack page diagnostics")?;
        }
        Ok(())
    }

    pub(super) fn inventory_owner_active(&self) -> bool {
        matches!(self.panel, Some(PlayerPanel::Inventory | PlayerPanel::Service(_)))
    }
}
