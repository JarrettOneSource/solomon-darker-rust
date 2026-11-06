//! InventoryScreen root 0x0056A7D7..0x0056AD5F: UI77 twice, UI76 multiply,
//! completed 256-square target, then pulsing UI75. Website RE ledger127 owns
//! the complete caller census and the corrected native integer-raster oracle.

use egui::{Pos2, Rect, pos2, vec2};
use solomon_assets::SpriteRecord;

use super::{StockAtlas, UiFrame, effects};

pub(super) const CAPTURE_SIDE: u32 = 256;

#[derive(Clone, Debug)]
pub(crate) struct Frame {
    pub image_centers: [[f32; 2]; 2],
    pub clip: [i32; 4],
    pub marker_red: f32,
}

impl Frame {
    pub fn new(tick: i32, image: &SpriteRecord, mask: &SpriteRecord) -> Self {
        let center = CAPTURE_SIDE as f32 / 2.0;
        let scroll = ((tick / 8) % image.logical_height as i32) as f32;
        let red = ((f64::from(tick).to_radians().sin() * 0.2 + 0.6) * 255.0).round();
        Self {
            image_centers: [
                [center, center - scroll],
                [center, center + image.logical_height as f32 - scroll - 1.0],
            ],
            clip: [
                (center - mask.logical_width as f32 / 2.0 + 0.5) as i32,
                (center - mask.logical_height as f32 / 2.0 + 0.5) as i32,
                mask.logical_width,
                mask.logical_height as i32,
            ],
            marker_red: red as f32 / 255.0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Paint {
    pub path: String,
    /// UI77, UI76, UI75, borrowing their tracked atlas at GPU submission.
    pub records: [SpriteRecord; 3],
    pub frame: Frame,
    pub target_positions: [Pos2; 4],
    pub marker_positions: [Pos2; 4],
    pub opacity: f32,
}

impl Paint {
    pub fn bounds(&self) -> Rect {
        self.target_positions
            .iter()
            .fold(Rect::NOTHING, |bounds, &point| bounds.union(Rect::from_min_max(point, point)))
    }

    #[cfg(test)]
    pub(crate) fn from_shape(shape: &egui::Shape) -> Option<Self> {
        let egui::Shape::Callback(callback) = shape else { return None };
        let effects::Boundary::Unforge(paint) = callback.callback.downcast_ref()? else {
            return None;
        };
        Some((**paint).clone())
    }
}

impl UiFrame<'_> {
    pub(crate) fn paint_unforge_target(
        &self,
        painter: &egui::Painter,
        atlas: &StockAtlas,
        application_time_ms: f64,
        reveal: f32,
        origin: egui::Vec2,
    ) -> Result<(), String> {
        let records =
            [atlas.record(77)?.clone(), atlas.record(76)?.clone(), atlas.record(75)?.clone()];
        let frame =
            Frame::new((application_time_ms / 10.0).trunc() as i32, &records[0], &records[1]);
        let anchor = pos2(1562.0 + (1.0 - reveal) * 25.0, 868.0) + origin;
        let layer = self.ctx.layer_transform_to_global(painter.layer_id()).unwrap_or_default();
        let corners = |min: Pos2, size: egui::Vec2| {
            let rect = Rect::from_min_size(min, size);
            [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()]
                .map(|point| layer * self.paint_position(point))
        };
        let marker = &records[2];
        let [left, top] = marker.registered_origin();
        let paint = Paint {
            path: atlas.path().to_owned(),
            target_positions: corners(anchor - vec2(128.5, 128.5), vec2(256.0, 256.0)),
            marker_positions: corners(
                anchor
                    + vec2(
                        left - marker.logical_width as f32 / 2.0,
                        top - marker.logical_height as f32 / 2.0,
                    ),
                vec2(marker.width as f32, marker.height as f32),
            ),
            opacity: reveal * painter.opacity(),
            records,
            frame,
        };
        let mut marker_painter = painter.clone();
        marker_painter.set_opacity(1.0);
        marker_painter.add(egui::Shape::Callback(egui::epaint::PaintCallback {
            rect: painter.clip_rect(),
            callback: std::sync::Arc::new(effects::Boundary::Unforge(Box::new(paint))),
        }));
        Ok(())
    }
}
