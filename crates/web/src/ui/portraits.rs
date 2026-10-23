//! Stock starter-element preview layers from `HallOfFamePrimitives.tsx` / `AllyHud.tsx`.
//! The sync script retains the Website's packed first-pose frames and fixed staff sockets.

use std::collections::BTreeMap;

use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::profile::WizardElement;
use web_sys::{Document, HtmlElement};

use crate::ui::{append, element, style};
use solomon_graphics::assets;

#[derive(Deserialize)]
pub(crate) struct StockPortraits {
    frames: BTreeMap<String, Vec<Option<[i32; 7]>>>,
    staff_orbs: [[f64; 2]; 24],
    staff_front: [bool; 24],
}

pub(crate) struct StockWizardPortrait {
    pub root: HtmlElement,
    pub orb: HtmlElement,
}

impl StockPortraits {
    pub async fn load() -> Result<Self, String> {
        let bytes = assets::fetch_bytes("assets/stock/player-portraits.json").await?;
        serde_json::from_slice(&bytes).map_err(|error| format!("stock portrait metadata: {error}"))
    }

    pub fn wizard(
        &self,
        document: &Document,
        element_id: WizardElement,
        heading: usize,
        position: Vec2,
        scale: f64,
    ) -> Result<StockWizardPortrait, String> {
        let heading = heading % 24;
        let front = self.staff_front[heading];
        let root = element(document, "span", "stock-wizard-portrait")?;
        root.set_attribute("aria-hidden", "true").map_err(|_| "portrait decorative state")?;
        style(
            &root,
            &format!(
                "left:{}px;top:{}px;transform:scale({scale})",
                position.x - 85.0,
                position.y - 85.0
            ),
        )?;
        let element_name = element_id.as_str();
        for (layer, depth) in [
            (
                if front { "staff-front".to_string() } else { "staff-back".to_string() },
                if front { 5 } else { 1 },
            ),
            (format!("robe-dynamic-{element_name}"), 3),
            (format!("robe-fixed-{element_name}"), 4),
            (format!("head-{element_name}"), 7),
        ] {
            self.append_layer(document, &root, &layer, heading, depth)?;
        }
        let [x, y] = self.staff_orbs[heading];
        let orb = element(document, "span", "stock-portrait-orb")?;
        style(
            &orb,
            &format!(
                "left:{}px;top:{}px;z-index:{}",
                85.0 + x,
                85.0 + y,
                if front { 6 } else { 2 }
            ),
        )?;
        append(&root, &orb)?;
        Ok(StockWizardPortrait { root, orb })
    }
    /// AllyHud intentionally uses only the body/head, without the weapon or orb.
    pub fn body(
        &self,
        document: &Document,
        element_id: WizardElement,
        heading: usize,
    ) -> Result<HtmlElement, String> {
        let root = element(document, "span", "stock-portrait-body")?;
        for (layer, depth) in [
            (format!("robe-dynamic-{}", element_id.as_str()), 3),
            (format!("robe-fixed-{}", element_id.as_str()), 4),
            (format!("head-{}", element_id.as_str()), 7),
        ] {
            self.append_layer(document, &root, &layer, heading % 24, depth)?;
        }
        Ok(root)
    }

    fn append_layer(
        &self,
        document: &Document,
        root: &HtmlElement,
        layer: &str,
        heading: usize,
        depth: usize,
    ) -> Result<(), String> {
        let name = format!("player-character-{layer}");
        let frames =
            self.frames.get(&name).ok_or_else(|| format!("stock portraits have no {name}"))?;
        let frame = frames.get(heading).ok_or_else(|| format!("{name} has no facing {heading}"))?;
        if let Some([page, x, y, width, height, trim_x, trim_y]) = frame {
            let crop = element(document, "span", "stock-portrait-layer")?;
            style(
                &crop,
                &format!(
                    "left:{trim_x}px;top:{trim_y}px;width:{width}px;height:{height}px;z-index:{depth};background-image:url('assets/stock/player-character-atlas-{page}.png');background-position:-{x}px -{y}px"
                ),
            )?;
            append(root, &crop)?;
        }
        Ok(())
    }
}
