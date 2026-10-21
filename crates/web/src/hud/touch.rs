//! The source adaptive two-bank quickbar from `mobile-quickbar-layout.ts`.
//! Saved layout transforms remain separate from these natural control dimensions.

use solomon_sim::Vec2;
use solomon_sim::hud::mobile::{
    MobileQuickbarBank, MobileQuickbarBankLayout, mobile_quickbar_bank_layout,
    mobile_quickbar_slot_placement,
};
use solomon_sim::viewport::game_viewport;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use super::Hud;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TouchBeltLayout {
    bank: MobileQuickbarBankLayout,
}

impl Hud {
    pub(super) fn update_touch_layout(&mut self, width: i32, height: i32) -> Result<(), String> {
        let window = self.document.default_view().ok_or("HUD window")?;
        if !window
            .match_media("(hover:none) and (pointer:coarse)")
            .map_err(|_| "HUD pointer query")?
            .is_some_and(|query| query.matches())
        {
            self.touch_layout = None;
            return Ok(());
        }
        let viewport = game_viewport(Vec2::new(f64::from(width), f64::from(height)));
        let bank = mobile_quickbar_bank_layout(viewport.logical_size.x, self.ui_scale);
        let layout = TouchBeltLayout { bank };
        if self.touch_layout == Some(layout) {
            return Ok(());
        }
        self.touch_layout = Some(layout);
        self.root
            .style()
            .set_property("--touch-slot-size", &format!("{}px", bank.size))
            .map_err(|_| "touch slot size")?;
        let children = self.belt.children();
        for index in 0..children.length() {
            if let Some(node) = children.item(index) {
                let slot = node
                    .get_attribute("data-slot")
                    .ok_or("belt node has no slot")?
                    .parse()
                    .map_err(|_| "belt slot is not an index")?;
                let node = node
                    .dyn_into::<HtmlElement>()
                    .map_err(|_| "belt slot is not an HTML element")?;
                self.place_touch_belt_slot(&node, slot)?;
            }
        }
        Ok(())
    }

    pub(super) fn place_touch_belt_slot(
        &self,
        button: &HtmlElement,
        slot: usize,
    ) -> Result<(), String> {
        let Some(layout) = self.touch_layout else {
            return Ok(());
        };
        let placement = mobile_quickbar_slot_placement(slot, layout.bank);
        button
            .set_attribute(
                "data-quickbar-bank",
                if placement.bank == MobileQuickbarBank::Left { "left" } else { "right" },
            )
            .map_err(|_| "touch bank")?;
        button
            .style()
            .set_property("--touch-slot-inset", &format!("{}px", placement.inset))
            .map_err(|_| "touch slot inset")?;
        button
            .style()
            .set_property("--touch-slot-bottom", &format!("{}px", placement.bottom))
            .map_err(|_| "touch slot bottom")?;
        Ok(())
    }
}
