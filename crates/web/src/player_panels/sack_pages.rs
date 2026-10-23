//! Stock Sack page navigation. Accepted items keep their addresses while the
//! source's stationary clip exposes incoming and outgoing GPU-painted pages.
use super::chrome::PanelCanvas;
use super::{PlayerPanel, PlayerPanelSnapshot, PlayerPanels, Target};
use solomon_sim::audio::AudioCue;
use solomon_sim::economy::{
    HubItemKind, find_inventory_item, inventory_items_at_sack_path, project_inventory_root_slots,
};
use solomon_sim::hud::HudRect;
use solomon_sim::player_panels::sack_pages::{
    SACK_PAGE_CLIP, SACK_PAGE_DURATION_MS, SackPageDirection, hub_sack_page_offsets,
};
use solomon_sim::player_panels::{INVENTORY_CAPACITY, inventory_slot_rect};
pub(super) struct SackPageTransition {
    direction: SackPageDirection,
    from_path: Vec<u64>,
    to_path: Vec<u64>,
    started_at_ms: f64,
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
    }
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

        true
    }
    pub(super) fn expire_sack_transition(&mut self, now_ms: f64) -> Result<(), String> {
        if self
            .sack_transition
            .as_ref()
            .is_some_and(|transition| now_ms - transition.started_at_ms >= SACK_PAGE_DURATION_MS)
        {
            self.clear_sack_transition()?;
        }
        Ok(())
    }
    pub(super) fn inventory_owner_active(&self) -> bool {
        matches!(self.panel, Some(PlayerPanel::Inventory | PlayerPanel::Service(_)))
    }
    pub(super) fn clear_sack_transition(&mut self) -> Result<(), String> {
        self.sack_transition = None;
        Ok(())
    }
    pub(super) fn draw_sack_pages(
        &self,
        c: &PanelCanvas<'_, '_>,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let sack = self
            .sack_path
            .last()
            .and_then(|id| find_inventory_item(&snapshot.economy.backpack, *id));
        if self.sack_transition.is_none() {
            self.section_header(
                c,
                sack.map_or("Backpack", |sack| sack.name.as_str()),
                800.0,
                460.0,
                489.0,
                sack.is_some(),
            )?;
        }
        let clip = c.clip(SACK_PAGE_CLIP);
        if let Some(transition) = &self.sack_transition {
            let offsets =
                hub_sack_page_offsets(transition.direction, transition.started_at_ms, self.now);
            self.draw_sack_grid(
                &clip.at(0.0, offsets.outgoing_y).admitted(false).namespace("panel:sack-outgoing"),
                &transition.from_path,
                false,
                snapshot,
            )?;
            self.draw_sack_grid(
                &clip.at(0.0, offsets.incoming_y).admitted(false).namespace("panel:sack-incoming"),
                &transition.to_path,
                false,
                snapshot,
            )?;
        } else {
            self.draw_sack_grid(
                &clip,
                &self.sack_path,
                !self.inventory_transition_locked(),
                snapshot,
            )?;
            clip.semantic_value(
                "panel:sack-page",
                "Current inventory page",
                "group",
                SACK_PAGE_CLIP,
                &serde_json::json!({"path":self.sack_path,"phase":"current"}).to_string(),
            );
        }
        Ok(())
    }
    fn draw_sack_grid(
        &self,
        c: &PanelCanvas<'_, '_>,
        path: &[u64],
        interactive: bool,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let source = inventory_items_at_sack_path(&snapshot.economy.backpack, path).unwrap_or(&[]);
        let items = project_inventory_root_slots(source);
        let parent =
            path.last().and_then(|id| find_inventory_item(&snapshot.economy.backpack, *id));
        for visible in 0..INVENTORY_CAPACITY {
            let r = inventory_slot_rect(visible);
            if visible == 0
                && let Some(item) = parent
            {
                let cell = c.admitted(interactive).button("back", "Return to parent inventory", r);
                cell.faded(0.4).raw(
                    &self.inventory,
                    10,
                    HudRect::new(0.0, 0.0, 72.0, 72.0),
                    0xffffff,
                )?;
                cell.fill(HudRect::new(6.0, 6.0, 60.0, 60.0), 0xd9ba70, 0.25);
                cell.art(&self.ui, 1, [36.0, 24.0], [0.65, 0.65], 0.0, 0xffffff)?;
                let holder = cell.at(1.0, 1.0).clip(HudRect::new(0.0, 0.0, 70.0, 67.0));
                self.draw_item(&holder, item, 35.0, 53.0, 1)?;
                continue;
            }
            let slot = visible - usize::from(parent.is_some());
            let item = items.iter().find(|item| item.slot == slot).map(|item| item.item);
            let action =
                item.map_or_else(|| format!("cell:{slot}"), |item| format!("item:{}", item.id));
            let label = item.map_or_else(
                || format!("Empty backpack slot {}", slot + 1),
                |item| format!("{}, quantity {}", item.name, item.quantity),
            );
            let cell = c.admitted(interactive).button(&action, &label, r);
            cell.faded(0.4).raw(
                &self.inventory,
                10,
                HudRect::new(0.0, 0.0, 72.0, 72.0),
                0xffffff,
            )?;
            if let Some(item) = item.filter(|item| !self.inventory_item_hidden(item.id)) {
                let bounds = self.draw_item(&cell, item, 36.0, 36.0, item.quantity as u64)?;
                self.record_mod_item(&cell, item, 36.0, 36.0, bounds);
            }
            if interactive && item.is_some_and(|item| self.selected == Some(Target::Item(item.id)))
            {
                cell.stroke(HudRect::new(1.0, 1.0, 70.0, 70.0), 0xd9ba70, 2.0);
            }
        }
        Ok(())
    }
}
