//! InventoryScreen page motion from renderer/hub-inventory-render-contract.ts.
//! The clip stays in the native stage while incoming and outgoing pages move.

use crate::hud::HudRect;

pub const SACK_PAGE_HEIGHT: f64 = 365.0;
pub const SACK_PAGE_TICKS: u32 = 37;
pub const SACK_PAGE_DURATION_MS: f64 = 370.0;
pub const SACK_PAGE_CLIP: HudRect = HudRect::new(0.0, 492.0, 1600.0, 305.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SackPageDirection {
    Open,
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SackPageOffsets {
    pub incoming_y: f64,
    pub outgoing_y: f64,
    pub settled: bool,
    pub ticks: u32,
}

/// Native BuildPage selects grid height from the actual screen height.
#[must_use]
pub fn native_inventory_sack_page_height(screen_height: f64) -> f64 {
    assert!(screen_height.is_finite() && screen_height > 0.0, "inventory height must be positive");
    if screen_height > 320.0 && screen_height <= 600.0 {
        220.0
    } else if screen_height > 800.0 {
        365.0
    } else {
        295.0
    }
}

/// hubSackPageOffsets: ten pixels per native tick, with a final five-pixel
/// snap to settle the 365px incoming page on tick37.
#[must_use]
pub fn hub_sack_page_offsets(
    direction: SackPageDirection,
    started_at_ms: f64,
    now_ms: f64,
) -> SackPageOffsets {
    let ticks = ((now_ms - started_at_ms) / 10.0).floor().clamp(0.0, f64::from(SACK_PAGE_TICKS));
    let travel = ticks * 10.0;
    let settled = ticks == f64::from(SACK_PAGE_TICKS);
    let remaining = if settled { 0.0 } else { SACK_PAGE_HEIGHT - travel };
    let (incoming_y, outgoing_y) = match direction {
        SackPageDirection::Open => (remaining, if travel == 0.0 { 0.0 } else { -travel }),
        SackPageDirection::Back => (if remaining == 0.0 { 0.0 } else { -remaining }, travel),
    };
    SackPageOffsets { incoming_y, outgoing_y, settled, ticks: ticks as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incoming_page_snaps_only_on_the_final_native_tick() {
        let open = hub_sack_page_offsets(SackPageDirection::Open, 100.0, 469.0);
        assert_eq!(open.incoming_y, 5.0);
        assert_eq!(open.outgoing_y, -360.0);
        assert!(!open.settled);
        let back = hub_sack_page_offsets(SackPageDirection::Back, 100.0, 470.0);
        assert_eq!(back.incoming_y, 0.0);
        assert_eq!(back.outgoing_y, 370.0);
        assert!(back.settled);
    }
}
