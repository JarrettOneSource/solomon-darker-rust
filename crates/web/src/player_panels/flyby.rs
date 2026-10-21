//! Source `use-hub-inventory-flybys.ts` / renderer `pages.ts`: display-only lanes.
//! A swap command leaves at travel completion; its real receipt retires the held icons.

use solomon_sim::economy::HubInventoryAction;
use solomon_sim::player_panels::flyby::{
    INVENTORY_FLYBY_AFTERIMAGE_BIRTH_TICKS, INVENTORY_FLYBY_TRAVEL_MS, InventoryFlybyRequest,
    hub_inventory_flyby_frame, hub_inventory_flyby_point,
};
use web_sys::HtmlElement;

use super::{PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels};
use crate::ui::{append, element, style};

struct LaneView {
    main: HtmlElement,
    afterimages: Vec<HtmlElement>,
}

pub(super) struct InventoryFlyby {
    request: InventoryFlybyRequest,
    root: HtmlElement,
    views: Vec<LaneView>,
    started_at_ms: f64,
    action_dispatched: bool,
    trailing: bool,
    painted_tick: Option<u64>,
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

    pub(super) fn clear_inventory_flybys(&mut self) -> Result<(), String> {
        self.inventory_flybys.clear();
        self.flyby_layer.set_text_content(None);
        self.clear_flyby_diagnostics()
    }

    pub(super) fn start_inventory_flyby(
        &mut self,
        request: InventoryFlybyRequest,
        snapshot: &PlayerPanelSnapshot<'_>,
        now_ms: f64,
    ) -> Result<(), String> {
        if self.inventory_flyby_active() {
            return Ok(());
        }
        let root = element(&self.document, "div", "panel-inventory-flyby")?;
        root.set_attribute("aria-hidden", "true").map_err(|_| "inventory flyby art")?;
        let mut views = Vec::new();
        for lane in &request.lanes {
            let mut afterimages = Vec::new();
            for spawn_tick in INVENTORY_FLYBY_AFTERIMAGE_BIRTH_TICKS {
                let copy = element(&self.document, "div", "panel-inventory-flyby-copy")?;
                copy.set_attribute("data-flyby-spawn-tick", &spawn_tick.to_string())
                    .map_err(|_| "inventory afterimage birth")?;
                self.draw_item(&copy, &lane.item, 0.0, 0.0, 1, snapshot.mod_assets)?;
                append(&root, &copy)?;
                copy.set_hidden(true);
                afterimages.push(copy);
            }
            let main = element(&self.document, "div", "panel-inventory-flyby-main")?;
            self.draw_item(&main, &lane.item, 0.0, 0.0, 1, snapshot.mod_assets)?;
            append(&root, &main)?;
            views.push(LaneView { main, afterimages });
        }
        append(&self.flyby_layer, &root)?;
        self.inventory_flybys.push(InventoryFlyby {
            request,
            root,
            views,
            started_at_ms: now_ms,
            action_dispatched: false,
            trailing: false,
            painted_tick: None,
        });
        self.dirty = true;
        Ok(())
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
                flyby.painted_tick = None;
                self.dirty = true;
            }
        }
    }

    pub(super) fn update_inventory_flybys(&mut self, now_ms: f64) -> Result<(), String> {
        for flyby in &mut self.inventory_flybys {
            if !flyby.trailing
                && !flyby.action_dispatched
                && now_ms - flyby.started_at_ms >= INVENTORY_FLYBY_TRAVEL_MS
            {
                flyby.action_dispatched = true;
                if let Some(action) = &flyby.request.action {
                    self.actions.push(PlayerPanelAction::Inventory(action.clone()));
                } else {
                    flyby.trailing = true;
                    self.dirty = true;
                }
            }
        }
        self.inventory_flybys.retain(|flyby| {
            if flyby.trailing && hub_inventory_flyby_frame(flyby.started_at_ms, now_ms).complete {
                flyby.root.remove();
                false
            } else {
                true
            }
        });
        let mut copy_count = 0;
        let mut main_count = 0;
        let mut representative_tick = 0;
        let mut flying_tick = None;
        for flyby in &mut self.inventory_flybys {
            let frame = hub_inventory_flyby_frame(flyby.started_at_ms, now_ms);
            copy_count += frame.afterimages.len() * flyby.request.lanes.len();
            if !flyby.trailing {
                flying_tick = Some(frame.tick);
                if frame.main_visible {
                    main_count += flyby.request.lanes.len();
                }
            }
            representative_tick = frame.tick;
            if flyby.painted_tick == Some(frame.tick) {
                continue;
            }
            flyby.painted_tick = Some(frame.tick);
            for (lane, view) in flyby.request.lanes.iter().zip(&flyby.views) {
                let point = hub_inventory_flyby_point(lane.from, lane.to, frame.main_progress);
                style(
                    &view.main,
                    &format!("position:absolute;left:{}px;top:{}px", point.x, point.y),
                )?;
                view.main.set_hidden(flyby.trailing || !frame.main_visible);
                for (spawn_tick, node) in
                    INVENTORY_FLYBY_AFTERIMAGE_BIRTH_TICKS.iter().zip(&view.afterimages)
                {
                    let copy = frame.afterimages.iter().find(|copy| copy.spawn_tick == *spawn_tick);
                    node.set_hidden(copy.is_none());
                    if let Some(copy) = copy {
                        let point = hub_inventory_flyby_point(lane.from, lane.to, copy.progress);
                        style(
                            node,
                            &format!(
                                "position:absolute;left:{}px;top:{}px;opacity:{}",
                                point.x, point.y, copy.alpha
                            ),
                        )?;
                    }
                }
            }
        }
        if self.inventory_flybys.is_empty() {
            return self.clear_flyby_diagnostics();
        }
        for (name, value) in [
            (
                "data-native-inventory-flyby-phase",
                if flying_tick.is_some() { "flying".to_owned() } else { "trailing".to_owned() },
            ),
            ("data-native-inventory-flyby-afterimages", copy_count.to_string()),
            ("data-native-inventory-flyby-main-items", main_count.to_string()),
            (
                "data-native-inventory-flyby-ticks",
                flying_tick.unwrap_or(representative_tick).to_string(),
            ),
        ] {
            self.stage.set_attribute(name, &value).map_err(|_| "inventory flyby frame")?;
        }
        Ok(())
    }

    fn clear_flyby_diagnostics(&self) -> Result<(), String> {
        for name in [
            "data-native-inventory-flyby-phase",
            "data-native-inventory-flyby-afterimages",
            "data-native-inventory-flyby-main-items",
            "data-native-inventory-flyby-ticks",
        ] {
            self.stage.remove_attribute(name).map_err(|_| "inventory flyby retirement")?;
        }
        Ok(())
    }
}
