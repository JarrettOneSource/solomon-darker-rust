//! Website `hub-inventory-render-contract.ts` InventoryFlyby: twenty discrete steps
//! and ten independently fading copies. These are display clocks, not inventory authority.

use crate::Vec2;
use crate::economy::{HubInventoryAction, HubInventoryItem};

pub const INVENTORY_FLYBY_TRAVEL_MS: f64 = 200.0;
pub const INVENTORY_FLYBY_AFTERIMAGE_BIRTH_TICKS: [u64; 10] = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19];

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryFlybyLane {
    pub from: Vec2,
    pub to: Vec2,
    pub item: HubInventoryItem,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryFlybyRequest {
    /// Source delays only Move/Unequip swaps; a restoration has no action.
    pub action: Option<HubInventoryAction>,
    pub lanes: Vec<InventoryFlybyLane>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InventoryFlybyAfterimageFrame {
    pub alpha: f64,
    pub progress: f64,
    pub spawn_tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryFlybyFrame {
    pub afterimages: Vec<InventoryFlybyAfterimageFrame>,
    pub complete: bool,
    pub main_progress: f64,
    pub main_visible: bool,
    pub tick: u64,
    pub travel_complete: bool,
}

#[must_use]
pub fn hub_inventory_flyby_frame(started_at_ms: f64, now_ms: f64) -> InventoryFlybyFrame {
    let tick = ((now_ms - started_at_ms) / 10.0).floor().max(0.0) as u64;
    let travel_complete = tick >= 20;
    let mut afterimages = Vec::new();
    for spawn_tick in INVENTORY_FLYBY_AFTERIMAGE_BIRTH_TICKS {
        if spawn_tick > tick {
            continue;
        }
        // Source rounds each copy's alpha to a tenth with JavaScript Math.round.
        let alpha = (((1.0 - (tick - spawn_tick) as f64 * 0.1) * 10.0) + 0.5).floor() / 10.0;
        if alpha > 0.0 {
            afterimages.push(InventoryFlybyAfterimageFrame {
                alpha,
                progress: spawn_tick as f64 / 20.0,
                spawn_tick,
            });
        }
    }
    InventoryFlybyFrame {
        afterimages,
        complete: tick >= 29,
        main_progress: tick.min(20) as f64 / 20.0,
        main_visible: !travel_complete,
        tick,
        travel_complete,
    }
}

#[must_use]
pub fn hub_inventory_flyby_point(from: Vec2, to: Vec2, progress: f64) -> Vec2 {
    let bounded = progress.clamp(0.0, 1.0);
    Vec2::new(from.x + (to.x - from.x) * bounded, from.y + (to.y - from.y) * bounded)
}
