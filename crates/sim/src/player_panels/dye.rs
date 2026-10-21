//! Fabric Dye presentation from `renderer/hub-inventory-render-contract.ts`.
//! Session admission and color mixing remain in the canonical inventory owner.

use crate::hud::HudRect;

#[must_use]
pub fn native_dye_swatch_rect(index: usize) -> HudRect {
    assert!(index < 18, "native dye swatch index must be within [0,17]");
    let bank = index / 9;
    let bank_index = index % 9;
    HudRect::new(
        560.0 + bank as f64 * 200.0 + (bank_index % 3) as f64 * 40.0,
        185.0 + (bank_index / 3) as f64 * 50.0,
        32.0,
        32.0,
    )
}

#[must_use]
pub fn native_dye_modal_opacity(opened_at: f64, closing_at: Option<f64>, now: f64) -> f64 {
    let opened_until = now.min(closing_at.unwrap_or(now));
    let opened_opacity = (elapsed_ticks(opened_at, opened_until) * 0.01).min(1.0);
    let Some(closing_at) = closing_at.filter(|closing_at| now > *closing_at) else {
        return opened_opacity;
    };
    (opened_opacity - elapsed_ticks(closing_at, now) * 0.1).max(0.0)
}

#[must_use]
pub fn native_dye_selected_pulse(selected_at: Option<f64>, now: f64) -> f64 {
    selected_at.map_or(0.0, |selected_at| (1.0 - elapsed_ticks(selected_at, now) * 0.05).max(0.0))
}

fn elapsed_ticks(started_at: f64, now: f64) -> f64 {
    ((now - started_at) / 10.0).floor().max(0.0)
}
