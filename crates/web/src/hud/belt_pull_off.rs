//! Gameplay belt placement around the shared stock pull-off burst.

use super::Hud;
use crate::ui::{append, belt_pull_off::belt_pull_off_burst, element};

impl Hud {
    /// Pair the source input edge with the actual unbind command and Poof cue in
    /// Game. The slot is the authority belt index, not its visible bank offset.
    pub fn show_belt_pull_off(&self, slot: usize) -> Result<(), String> {
        let Some(button) = self
            .belt
            .query_selector(&format!("[data-belt-index='{slot}']"))
            .map_err(|_| "belt burst target")?
        else {
            return Ok(());
        };
        let anchor = element(&self.document, "span", "hud-belt-slot hud-belt-burst-anchor")?;
        // A sibling keeps the exact responsive/custom control transform while
        // accepted cooldown/item changes replace the button's displayed art.
        for name in [
            "style",
            "data-quickbar-bank",
            "data-touch-potion",
            "data-mobile-ui-custom",
            "data-mobile-ui-element",
        ] {
            if let Some(value) = button.get_attribute(name) {
                anchor.set_attribute(name, &value).map_err(|_| "belt burst placement")?;
            }
        }
        anchor.set_attribute("aria-hidden", "true").map_err(|_| "belt burst decoration")?;
        anchor.set_attribute("data-belt-burst-anchor", "true").map_err(|_| "belt burst owner")?;
        append(&anchor, &belt_pull_off_burst(&self.document, &self.ui)?)?;
        append(&self.root, &anchor)
    }
}
