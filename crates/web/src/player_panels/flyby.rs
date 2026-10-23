//! Source inventory flybys. The real swap command leaves at travel completion;
//! its authority receipt retires the held main icons while afterimages finish.
use super::chrome::PanelCanvas;
use super::{PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels};
use solomon_sim::economy::HubInventoryAction;
use solomon_sim::player_panels::flyby::{
    INVENTORY_FLYBY_TRAVEL_MS, InventoryFlybyRequest, hub_inventory_flyby_frame,
    hub_inventory_flyby_point,
};
pub(super) struct InventoryFlyby {
    request: InventoryFlybyRequest,
    started_at_ms: f64,
    action_dispatched: bool,
    trailing: bool,
}
impl PlayerPanels {
    pub(super) fn inventory_flyby_active(&self) -> bool {
        self.inventory_flybys.iter().any(|flyby| !flyby.trailing)
    }
    pub(super) fn inventory_item_flying(&self, item_id: u64) -> bool {
        self.inventory_flybys.iter().any(|flyby| {
            !flyby.trailing && flyby.request.lanes.iter().any(|lane| lane.item.id == item_id)
        })
    }
    pub(super) fn inventory_item_hidden(&self, item_id: u64) -> bool {
        self.inventory_item_flying(item_id)
            || self
                .press
                .is_some_and(|press| press.dragging && press.inventory_item_id == Some(item_id))
    }
    pub(super) fn inventory_flyby_feedback(&mut self, action: &HubInventoryAction) {
        for flyby in &mut self.inventory_flybys {
            if flyby.trailing || !flyby.action_dispatched {
                continue;
            }
            // Root delivers each actual authority receipt once. Source compares its
            // newer feedback sequence and action kind; it does not require acceptance.
            if flyby.request.action.as_ref().is_some_and(|pending| {
                std::mem::discriminant(pending) == std::mem::discriminant(action)
            }) {
                flyby.trailing = true;
            }
        }
    }
    pub(super) fn clear_inventory_flybys(&mut self) -> Result<(), String> {
        self.inventory_flybys.clear();
        Ok(())
    }
    pub(super) fn start_inventory_flyby(
        &mut self,
        request: InventoryFlybyRequest,
        _snapshot: &PlayerPanelSnapshot<'_>,
        now_ms: f64,
    ) -> Result<(), String> {
        if !self.inventory_flyby_active() {
            self.inventory_flybys.push(InventoryFlyby {
                request,
                started_at_ms: now_ms,
                action_dispatched: false,
                trailing: false,
            });
        }
        Ok(())
    }
    pub(super) fn update_inventory_flybys(&mut self, now: f64) -> Result<(), String> {
        for flyby in &mut self.inventory_flybys {
            if !flyby.trailing
                && !flyby.action_dispatched
                && now - flyby.started_at_ms >= INVENTORY_FLYBY_TRAVEL_MS
            {
                flyby.action_dispatched = true;
                if let Some(action) = &flyby.request.action {
                    self.actions.push(PlayerPanelAction::Inventory(action.clone()));
                } else {
                    flyby.trailing = true;
                }
            }
        }
        self.inventory_flybys.retain(|flyby| {
            !flyby.trailing || !hub_inventory_flyby_frame(flyby.started_at_ms, now).complete
        });
        Ok(())
    }
    pub(super) fn paint_inventory_flybys(
        &self,
        c: &PanelCanvas<'_, '_>,
        _snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let mut bounds = egui::Rect::NOTHING;
        let mut tick = 0;
        let mut flying = false;
        let mut main_visible = false;
        for flyby in &self.inventory_flybys {
            let frame = hub_inventory_flyby_frame(flyby.started_at_ms, self.now);
            tick = frame.tick;
            flying |= !flyby.trailing;
            main_visible |= !flyby.trailing && frame.main_visible;
            for lane in &flyby.request.lanes {
                for copy in &frame.afterimages {
                    let point = hub_inventory_flyby_point(lane.from, lane.to, copy.progress);
                    bounds = bounds.union(self.draw_item(
                        &c.faded(copy.alpha as f32),
                        &lane.item,
                        point.x,
                        point.y,
                        1,
                    )?);
                }
                if !flyby.trailing && frame.main_visible {
                    let point = hub_inventory_flyby_point(lane.from, lane.to, frame.main_progress);
                    bounds = bounds.union(self.draw_item(c, &lane.item, point.x, point.y, 1)?);
                }
            }
        }
        if bounds.is_positive() {
            let transform =
                c.frame.ctx.layer_transform_to_global(c.painter.layer_id()).unwrap_or_default();
            c.frame.record_rect(
                "panel:inventory-flyby",
                "Inventory item flyby",
                "image",
                transform * bounds,
                false,
            );
            c.frame.record_value("panel:inventory-flyby",&serde_json::json!({"phase":if flying{"flying"}else{"trailing"},"tick":tick,"mainVisible":main_visible}).to_string(),None);
        }
        Ok(())
    }
}
