//! GPU stock starter-element portraits from `HallOfFamePrimitives.tsx` / `AllyHud.tsx`.
//! Packed first poses and staff sockets retain their authored registration and order.

use std::collections::BTreeMap;

use egui::{Color32, Painter, Pos2, Rect, pos2, vec2};
use serde::Deserialize;
use solomon_sim::profile::WizardElement;

use crate::gpu_ui::UiFrame;
use solomon_graphics::assets;

#[derive(Deserialize)]
pub(crate) struct StockPortraits {
    frames: BTreeMap<String, Vec<Option<[i32; 7]>>>,
    staff_orbs: [[f64; 2]; 24],
    staff_front: [bool; 24],
}

impl StockPortraits {
    pub async fn load() -> Result<Self, String> {
        let bytes = assets::fetch_bytes("assets/stock/player-portraits.json").await?;
        serde_json::from_slice(&bytes).map_err(|error| format!("stock portrait metadata: {error}"))
    }

    /// Paint the whole wizard and return the staff socket for the caller's live orb VFX.
    pub fn paint_wizard(
        &self,
        gpu: &UiFrame<'_>,
        painter: &Painter,
        element: WizardElement,
        heading: usize,
        position: Pos2,
        scale: f32,
    ) -> Result<Pos2, String> {
        self.paint_wizard_with_orb(
            gpu,
            painter,
            element,
            heading,
            position,
            scale,
            |_, _, _| Ok(()),
        )
    }

    /// The orb paints at its original depth between the staff/robe/head layers.
    #[allow(clippy::too_many_arguments)] // The callback inserts live VFX into the stock layer order.
    pub fn paint_wizard_with_orb(
        &self,
        gpu: &UiFrame<'_>,
        painter: &Painter,
        element: WizardElement,
        heading: usize,
        position: Pos2,
        scale: f32,
        paint_orb: impl FnOnce(&UiFrame<'_>, &Painter, Pos2) -> Result<(), String>,
    ) -> Result<Pos2, String> {
        let heading = heading % 24;
        let front = self.staff_front[heading];
        let [x, y] = self.staff_orbs[heading];
        let socket = position + vec2(x as f32, y as f32) * scale;
        if front {
            for layer in [
                format!("robe-dynamic-{}", element.as_str()),
                format!("robe-fixed-{}", element.as_str()),
            ] {
                self.paint_layer(gpu, painter, &layer, heading, position, scale)?;
            }
            self.paint_layer(gpu, painter, "staff-front", heading, position, scale)?;
            paint_orb(gpu, painter, socket)?;
        } else {
            self.paint_layer(gpu, painter, "staff-back", heading, position, scale)?;
            paint_orb(gpu, painter, socket)?;
            for layer in [
                format!("robe-dynamic-{}", element.as_str()),
                format!("robe-fixed-{}", element.as_str()),
            ] {
                self.paint_layer(gpu, painter, &layer, heading, position, scale)?;
            }
        }
        self.paint_layer(
            gpu,
            painter,
            &format!("head-{}", element.as_str()),
            heading,
            position,
            scale,
        )?;
        Ok(socket)
    }

    /// AllyHud omits the weapon and orb.
    pub fn paint_body(
        &self,
        gpu: &UiFrame<'_>,
        painter: &Painter,
        element: WizardElement,
        heading: usize,
        position: Pos2,
        scale: f32,
    ) -> Result<(), String> {
        for layer in [
            format!("robe-dynamic-{}", element.as_str()),
            format!("robe-fixed-{}", element.as_str()),
            format!("head-{}", element.as_str()),
        ] {
            self.paint_layer(gpu, painter, &layer, heading % 24, position, scale)?;
        }
        Ok(())
    }

    fn paint_layer(
        &self,
        gpu: &UiFrame<'_>,
        painter: &Painter,
        layer: &str,
        heading: usize,
        position: Pos2,
        scale: f32,
    ) -> Result<(), String> {
        let name = format!("player-character-{layer}");
        let frames =
            self.frames.get(&name).ok_or_else(|| format!("stock portraits have no {name}"))?;
        let frame = frames.get(heading).ok_or_else(|| format!("{name} has no facing {heading}"))?;
        if let Some([page, x, y, width, height, trim_x, trim_y]) = frame {
            let path = format!("assets/stock/player-character-atlas-{page}.png");
            let Some(size) = gpu.image_size(&path) else {
                return Err(format!("portrait image is not loaded: {path}"));
            };
            let target = Rect::from_min_size(
                position + vec2((*trim_x - 85) as f32, (*trim_y - 85) as f32) * scale,
                vec2(*width as f32, *height as f32) * scale,
            );
            let uv = Rect::from_min_max(
                pos2(*x as f32 / size.x, *y as f32 / size.y),
                pos2((*x + *width) as f32 / size.x, (*y + *height) as f32 / size.y),
            );
            gpu.image(painter, &path, target, Some(uv), Color32::WHITE);
        }
        Ok(())
    }
}
