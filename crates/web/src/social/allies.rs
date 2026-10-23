//! GPU ally health rows from the host's canonical projection.

use egui::{Color32, FontFamily, FontId, Rect, Stroke, pos2, vec2};
use solomon_sim::social::{AllyHudIdentity, GameChatWorldKind, clamp_ally_hud_health_ratio};

use super::SocialUi;
use crate::gpu_ui::UiFrame;

impl SocialUi {
    pub(super) fn paint_allies(&self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let in_hub = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.world_kind == GameChatWorldKind::Hub);
        if self.context.input_suspended || (self.coarse && in_hub && self.party_expanded) {
            return Ok(());
        }
        let ctx = frame.ctx.clone();
        let layer = Self::allies_layer();
        let area = egui::Area::new(layer.id)
            .fade_in(false)
            .background_interactable(false)
            .order(layer.order)
            .constrain(false);
        ctx.set_transform_layer(
            area.layer(),
            egui::emath::TSTransform {
                scaling: if self.coarse { 0.72 } else { 1.0 },
                translation: if self.coarse {
                    vec2(6.0, if in_hub { 88.0 } else { 54.0 })
                } else {
                    egui::Vec2::ZERO
                },
            },
        );
        let width = if self.coarse { 164.0 } else { 196.0 };
        area.fixed_pos(if self.coarse { egui::Pos2::ZERO } else { pos2(11.0, 60.0) })
            .show(&ctx, |ui| -> Result<(), String> {
                ui.spacing_mut().item_spacing.y = 0.0;
                for model in &self.ally_rows {
                    let (bounds, _) =
                        ui.allocate_exact_size(vec2(width, 34.0), egui::Sense::hover());
                    let painter = ui.painter();
                    let accent = match &model.identity {
                        AllyHudIdentity::Player { element, .. } => match element.as_str() {
                            "ether" => Color32::from_rgb(185, 143, 230),
                            "fire" => Color32::from_rgb(224, 122, 69),
                            "water" => Color32::from_rgb(95, 176, 220),
                            "air" => Color32::from_rgb(165, 203, 228),
                            _ => Color32::from_rgb(168, 191, 98),
                        },
                        AllyHudIdentity::Golem => Color32::from_rgb(216, 188, 114),
                    };
                    painter.rect_filled(
                        bounds,
                        3.0,
                        Color32::from_rgba_unmultiplied(10, 9, 6, 190),
                    );
                    painter.rect_stroke(
                        bounds,
                        3.0,
                        Stroke::new(1.0, accent.gamma_multiply(0.5)),
                        egui::StrokeKind::Inside,
                    );
                    painter.line_segment(
                        [bounds.left_top(), bounds.left_bottom()],
                        Stroke::new(2.0, accent),
                    );
                    let chip = Rect::from_min_size(bounds.min + vec2(3.0, 3.0), vec2(27.0, 27.0));
                    painter.circle_filled(chip.center(), 13.5, Color32::from_rgb(25, 26, 28));
                    match &model.identity {
                        AllyHudIdentity::Player { element, .. } => {
                            let clipped = painter.with_clip_rect(chip);
                            self.portraits.paint_body_clipped(
                                frame,
                                &clipped,
                                *element,
                                12,
                                chip.min + vec2(12.85, 26.55),
                                0.45,
                                chip,
                            )?;
                        }
                        AllyHudIdentity::Golem => {
                            frame.image(
                                painter,
                                "assets/stock/skull-white.png",
                                Rect::from_center_size(chip.center(), vec2(17.0, 17.0)),
                                None,
                                Color32::WHITE,
                            );
                        }
                    }
                    painter.circle_stroke(chip.center(), 13.5, Stroke::new(1.0, accent));
                    super::gpu::browser_text(
                        frame,
                        painter,
                        bounds.min + vec2(38.0, 10.0),
                        egui::Align2::LEFT_CENTER,
                        model.accessible_name(),
                        FontId::new(12.0, FontFamily::Name("Georgia".into())),
                        Color32::from_rgb(240, 226, 182),
                    )?;
                    let bar =
                        Rect::from_min_size(bounds.min + vec2(38.0, 22.0), vec2(width - 50.0, 7.0));
                    painter.rect_filled(bar, 4.0, Color32::from_rgb(25, 10, 9));
                    let ratio = clamp_ally_hud_health_ratio(model.health_ratio) as f32;
                    painter.rect_filled(
                        Rect::from_min_size(bar.min, vec2(bar.width() * ratio, 7.0)),
                        3.0,
                        Color32::from_rgb(192, 57, 43),
                    );
                    painter.rect_stroke(
                        bar,
                        4.0,
                        Stroke::new(1.0, Color32::BLACK),
                        egui::StrokeKind::Inside,
                    );
                    if model.dead {
                        painter.rect_filled(
                            bounds,
                            3.0,
                            Color32::from_rgba_unmultiplied(117, 0, 0, 130),
                        );
                    }
                    if !model.connected {
                        painter.rect_filled(bounds, 3.0, Color32::from_black_alpha(110));
                    }
                    if !model.connected || model.dead {
                        painter.text(
                            bounds.right_center() - vec2(7.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            if !model.connected { "DISCONNECTED" } else { "DEAD" },
                            FontId::proportional(9.0),
                            Color32::WHITE,
                        );
                    }
                    frame.record_rect(
                        &format!("social/ally:{}", model.id),
                        &model.accessible_status(),
                        "progressbar",
                        ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default() * bounds,
                        true,
                    );
                    ui.add_space(5.0);
                }
                Ok(())
            })
            .inner?;
        Ok(())
    }
}
