//! Stock Dark Cloud scene, panels, list and tab artwork from
//! `NativeDarkCloudPresentation.tsx`; all coordinates are actual GPU quads.
use super::super::widgets;
use crate::gpu_ui::{StockAtlas, UiFrame};
use egui::{Color32, Rect, Ui, Vec2};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign};
pub(super) struct CloudArt {
    pub ui: StockAtlas,
    pub controls: StockAtlas,
}
impl CloudArt {
    pub async fn load() -> Result<Self, String> {
        Ok(Self {
            ui: StockAtlas::load("UI").await?,
            controls: StockAtlas::load("ControlPanel").await?,
        })
    }
    pub fn scene(&self, frame: &UiFrame<'_>, ui: &Ui, view: Rect) -> Result<(), String> {
        let w = view.width();
        let h = view.height();
        for (index, x, y, width, height) in [
            (30, 0.0, 65.0, w, 108.0),
            (30, 0.0, h - 100.0, w, 108.0),
            (33, -74.0, 0.0, 124.0, h),
            (33, w - 60.0, 0.0, 124.0, h),
        ] {
            self.ui.paint_tiles(
                frame,
                ui.painter(),
                index,
                Rect::from_min_size(view.min + Vec2::new(x, y), Vec2::new(width, height)),
                1.0,
                Color32::WHITE,
            )?;
        }
        for (index, x, y, rotation) in [
            (29, 135.0, 83.0, std::f32::consts::PI),
            (29, w - 135.0, 83.0, 0.0),
            (31, w - 112.0, 18.0, 0.0),
            (32, 104.0, 26.5, 0.0),
            (32, w + 44.0, h - 126.5, 0.0),
            (31, -57.0, h - 135.0, 0.0),
        ] {
            self.ui.paint(
                frame,
                ui.painter(),
                index,
                view.min + Vec2::new(x, y),
                1.0,
                Color32::WHITE,
            )?;
            if rotation != 0.0 {
                self.ui.paint_centered(
                    frame,
                    ui.painter(),
                    index,
                    view.min + Vec2::new(x, y),
                    1.0,
                    rotation,
                    Color32::WHITE,
                )?;
            }
        }
        for (left, positions) in [
            (0.0, [[-42.5, 162.5], [-82.5, (h - 8.0) / 2.0]]),
            (w - 55.0, [[w - 152.5, 162.5], [w - 112.5, (h - 8.0) / 2.0]]),
        ] {
            let clip = ui.painter().with_clip_rect(Rect::from_min_size(
                view.min + Vec2::new(left, 0.0),
                Vec2::new(55.0, h),
            ));
            for [x, y] in positions {
                self.ui.paint(frame, &clip, 20, view.min + Vec2::new(x, y), 1.0, Color32::WHITE)?;
            }
        }
        widgets::gradient(
            ui,
            Rect::from_min_size(view.min, Vec2::new(w, 150.0)),
            Color32::BLACK,
            Color32::TRANSPARENT,
        );
        Ok(())
    }
    pub fn list(&self, frame: &UiFrame<'_>, ui: &Ui, rect: Rect, scale: f32) -> Result<(), String> {
        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
        self.ui.paint_tiles(
            frame,
            ui.painter(),
            49,
            rect.shrink(20.0 * scale),
            scale,
            Color32::WHITE,
        )?;
        self.ui.paint_native_frame(frame, ui.painter(), 17, rect, true, Color32::WHITE)?;
        for (index, dest) in [
            (
                10,
                Rect::from_min_size(
                    rect.min + Vec2::new(-5.0, -14.0) * scale,
                    Vec2::new(rect.width() + 10.0 * scale, 19.0 * scale),
                ),
            ),
            (
                10,
                Rect::from_min_size(
                    egui::pos2(rect.left() - 5.0 * scale, rect.bottom() - 3.0 * scale),
                    Vec2::new(rect.width() + 10.0 * scale, 19.0 * scale),
                ),
            ),
            (
                79,
                Rect::from_min_size(
                    rect.min + Vec2::new(-15.0, -2.0) * scale,
                    Vec2::new(21.0 * scale, rect.height() + 4.0 * scale),
                ),
            ),
            (
                79,
                Rect::from_min_size(
                    egui::pos2(rect.right() - 2.0 * scale, rect.top() - 2.0 * scale),
                    Vec2::new(21.0 * scale, rect.height() + 4.0 * scale),
                ),
            ),
        ] {
            self.ui.paint_tiles(frame, ui.painter(), index, dest, scale, Color32::WHITE)?;
        }
        for (index, pos) in [
            (107, rect.min + Vec2::new(22.0, 25.0) * scale),
            (108, rect.right_top() + Vec2::new(-22.0, 25.0) * scale),
            (109, rect.left_bottom() + Vec2::new(22.0, -25.0) * scale),
            (110, rect.max - Vec2::new(22.0, 25.0) * scale),
        ] {
            self.ui.paint_centered(frame, ui.painter(), index, pos, scale, 0.0, Color32::WHITE)?;
        }
        Ok(())
    }
    pub fn panel(&self, frame: &UiFrame<'_>, ui: &Ui, rect: Rect) -> Result<(), String> {
        self.ui.paint_tiles(frame, ui.painter(), 49, rect, 1.0, Color32::WHITE)?;
        self.ui.paint_native_frame(
            frame,
            ui.painter(),
            17,
            rect.expand(20.0),
            true,
            Color32::WHITE,
        )?;
        for x in [rect.left() - 65.0, rect.right() + 65.0] {
            self.ui.paint_centered(
                frame,
                ui.painter(),
                18,
                egui::pos2(x, rect.center().y),
                1.0,
                0.0,
                Color32::WHITE,
            )?;
        }
        Ok(())
    }
    pub fn tab(
        &self,
        frame: &UiFrame<'_>,
        ui: &Ui,
        rect: Rect,
        scale: f32,
        label: &str,
        selected: bool,
    ) -> Result<(), String> {
        let chrome = (rect.height() - 69.0 * scale).max(0.0);
        let top = chrome + if selected { 0.0 } else { 8.0 * scale };
        let h = if selected { 65.0 } else { 51.0 };
        let bw = 34.0 * scale;
        let left = Rect::from_min_size(rect.min + Vec2::new(0.0, top), Vec2::new(bw, h * scale));
        self.ui.paint_slice(
            frame,
            ui.painter(),
            13,
            Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(34.0, h)),
            left,
            Color32::WHITE,
        )?;
        if rect.width() > bw * 2.0 {
            self.ui.paint_slice(
                frame,
                ui.painter(),
                13,
                Rect::from_min_size(egui::pos2(34.0 * 0.95, 0.0), Vec2::new(34.0 * 0.05, h)),
                Rect::from_min_size(
                    left.right_top(),
                    Vec2::new(rect.width() - bw * 2.0, h * scale),
                ),
                Color32::WHITE,
            )?;
        }
        let record = self.ui.record(13)?;
        let size = frame.image_size(self.ui.path()).ok_or("UI atlas not ready")?;
        let right = Rect::from_min_size(egui::pos2(rect.right() - bw, left.top()), left.size());
        let uv = Rect::from_min_max(
            egui::pos2(record.x as f32 / size.x, record.y as f32 / size.y),
            egui::pos2((record.x as f32 + 34.0) / size.x, (record.y as f32 + h) / size.y),
        );
        frame.image_quad(
            ui.painter(),
            self.ui.path(),
            [right.right_top(), right.left_top(), right.left_bottom(), right.right_bottom()],
            uv,
            Color32::WHITE,
        );
        widgets::native(
            frame,
            ui,
            label,
            NativeUiFontName::Menu,
            egui::pos2(
                rect.center().x,
                rect.top() + chrome + (45.0 + if selected { 0.0 } else { 8.0 }) * scale,
            ),
            scale,
            0xd9ba70,
            NativeUiTextAlign::Center,
        );
        Ok(())
    }
    pub fn tool(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &Ui,
        id: &str,
        label: &str,
        icon: Option<usize>,
        rect: Rect,
        enabled: bool,
    ) -> Result<bool, String> {
        let response = widgets::interact(frame, ui, id, label, "button", rect, enabled);
        let scale = rect.height() / 52.0;
        self.ui.paint_raw(
            frame,
            ui.painter(),
            if response.is_pointer_button_down_on() { 104 } else { 103 },
            rect,
            Color32::WHITE,
        )?;
        if let Some(icon) = icon {
            self.ui.paint_centered(
                frame,
                ui.painter(),
                icon,
                rect.center(),
                scale,
                0.0,
                Color32::WHITE,
            )?;
        } else {
            widgets::native(
                frame,
                ui,
                &label.to_lowercase(),
                NativeUiFontName::Menu,
                rect.center() + Vec2::new(0.0, 8.0 * scale),
                scale,
                0xd9ba70,
                NativeUiTextAlign::Center,
            );
        }
        self.ui.paint_native_frame(
            frame,
            ui.painter(),
            53,
            rect.expand(6.0 * scale),
            false,
            Color32::WHITE,
        )?;
        Ok(response.clicked() && enabled)
    }
}
