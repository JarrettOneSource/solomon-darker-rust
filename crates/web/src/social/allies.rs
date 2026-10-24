//! GPU ally health rows from the host's canonical projection.

use egui::{Color32, FontFamily, FontId, Rect, Stroke, pos2, vec2};
use solomon_sim::social::{
    AllyHudIdentity, AllyHudRow, GameChatWorldKind, clamp_ally_hud_health_ratio,
};

use super::SocialUi;
use crate::gpu_ui::UiFrame;

#[derive(Default)]
pub(super) struct AllyMotion {
    rows: Vec<AllyRowMotion>,
    visible: bool,
    reduced_motion: bool,
}
struct AllyRowMotion {
    model: AllyHudRow,
    health: super::motion::EaseTransition,
    appeared_at: f64,
}
impl AllyRowMotion {
    fn health(&self, now: f64) -> f64 {
        self.health.value(now)
    }
    fn stripe_offset(&self, now: f64, reduced_motion: bool) -> f32 {
        if reduced_motion {
            0.0
        } else {
            (((now - self.appeared_at).max(0.0) % 900.0) / 900.0 * 20.0) as f32
        }
    }
}
impl AllyMotion {
    pub(super) fn hide(&mut self) {
        self.visible = false;
    }
    fn motion_preference(&mut self, reduced: bool, now: f64) {
        if self.reduced_motion != reduced {
            for row in &mut self.rows {
                row.appeared_at = now;
            }
            self.reduced_motion = reduced;
        }
    }
    fn sync(&mut self, models: &[AllyHudRow], now: f64, visible: bool) {
        if !visible {
            self.hide();
            return;
        }
        let recreated = !self.visible
            || self.rows.len() != models.len()
            || self.rows.iter().zip(models).any(|(row, model)| {
                row.model.id != model.id
                    || row.model.identity != model.identity
                    || row.model.connected != model.connected
                    || row.model.dead != model.dead
            });
        if recreated {
            self.rows = models
                .iter()
                .map(|model| {
                    let health = clamp_ally_hud_health_ratio(model.health_ratio);
                    AllyRowMotion {
                        model: model.clone(),
                        health: super::motion::EaseTransition::new(health, 220.0),
                        appeared_at: now,
                    }
                })
                .collect();
        } else {
            for (row, model) in self.rows.iter_mut().zip(models) {
                let health = clamp_ally_hud_health_ratio(model.health_ratio);
                row.health.set(health, now);
                row.model = model.clone();
            }
        }
        self.visible = true;
    }
}

impl SocialUi {
    pub(super) fn paint_allies(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let in_hub = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.world_kind == GameChatWorldKind::Hub);
        let visible =
            !self.context.input_suspended && !(self.coarse && in_hub && self.party_expanded);
        self.ally_motion.sync(&self.ally_rows, self.now, visible);
        if !visible {
            return Ok(());
        }
        let reduced_motion = self
            .document
            .default_view()
            .ok_or("ally window")?
            .match_media("(prefers-reduced-motion: reduce)")
            .map_err(|_| "ally motion preference")?
            .is_some_and(|query| query.matches());
        self.ally_motion.motion_preference(reduced_motion, self.now);
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
                for (model, motion) in self.ally_rows.iter().zip(&self.ally_motion.rows) {
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
                    let corners = egui::CornerRadius { nw: 3, ne: 14, sw: 3, se: 14 };
                    painter.rect_filled(
                        bounds,
                        corners,
                        Color32::from_rgba_unmultiplied(10, 9, 6, 190),
                    );
                    painter.rect_stroke(
                        bounds,
                        corners,
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(216, 188, 114, 77)),
                        egui::StrokeKind::Inside,
                    );
                    painter.line_segment(
                        [bounds.left_top(), bounds.left_bottom()],
                        Stroke::new(2.0, accent),
                    );
                    let saturated = frame.with_saturation(if model.connected { 1.0 } else { 0.35 });
                    let content_frame =
                        saturated.with_brightness(if model.connected { 1.0 } else { 0.72 });
                    content_frame.paint_group(
                        painter,
                        crate::gpu_ui::GroupEffect::default(),
                        |frame, painter| {
                            let chip =
                                Rect::from_min_size(bounds.min + vec2(3.0, 3.0), vec2(27.0, 27.0));
                            painter.circle_filled(
                                chip.center(),
                                13.5,
                                Color32::from_rgb(25, 26, 28),
                            );
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
                            let bar = Rect::from_min_size(
                                bounds.min + vec2(38.0, 22.0),
                                vec2(width - 50.0, 7.0),
                            );
                            painter.rect_filled(bar, 4.0, Color32::from_rgb(25, 10, 9));
                            let ratio = motion.health(self.now) as f32;
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
                            Ok(())
                        },
                    )?;
                    if model.dead {
                        painter.rect_filled(
                            bounds,
                            corners,
                            Color32::from_rgba_unmultiplied(117, 0, 0, 130),
                        );
                    }
                    if !model.connected {
                        paint_signal_loss(
                            painter,
                            bounds,
                            motion.stripe_offset(self.now, reduced_motion),
                        );
                    }
                    if !model.connected || model.dead {
                        let status = if !model.connected { "DISCONNECTED" } else { "DEAD" };
                        let status_run = crate::gpu_ui::font_run_with_options(
                            frame.ctx,
                            FontId::new(12.0, FontFamily::Name("Georgia Bold".into())),
                            if !model.connected {
                                Color32::from_rgb(212, 237, 242)
                            } else {
                                Color32::from_rgb(255, 209, 203)
                            },
                            status,
                            crate::gpu_ui::FontRunOptions {
                                line_height: Some(12.0),
                                letter_spacing: Some(1.2),
                                ..Default::default()
                            },
                        )?;
                        frame.paint_group(
                            painter,
                            crate::gpu_ui::GroupEffect {
                                shadows: vec![
                                    crate::gpu_ui::ImageShadow {
                                        offset: vec2(0.0, 1.0),
                                        blur: 1.0,
                                        color: Color32::BLACK,
                                    },
                                    crate::gpu_ui::ImageShadow {
                                        offset: egui::Vec2::ZERO,
                                        blur: 2.5,
                                        color: Color32::BLACK,
                                    },
                                ],
                                ..Default::default()
                            },
                            |frame, painter| {
                                status_run.paint(
                                    frame,
                                    painter,
                                    bounds.center() + vec2(13.5, 0.0) - status_run.size() / 2.0,
                                    1.0,
                                )
                            },
                        )?;
                    }
                    frame.record_rect(
                        &format!("social/ally:{}", model.id),
                        &model.accessible_status(),
                        "progressbar",
                        ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default() * bounds,
                        true,
                    );
                    frame.record_value(
                        &format!("social/ally:{}", model.id),
                        &format!(
                            "{{\"health\":{},\"displayHealth\":{},\"stripeOffset\":{}}}",
                            clamp_ally_hud_health_ratio(model.health_ratio),
                            motion.health(self.now),
                            motion.stripe_offset(self.now, reduced_motion)
                        ),
                        None,
                    );
                    ui.add_space(5.0);
                }
                Ok(())
            })
            .inner?;
        Ok(())
    }
}

// The source pseudo-element is a repeating115deg gradient inside the row clip.
fn paint_signal_loss(painter: &egui::Painter, bounds: Rect, offset: f32) {
    let mut clip = Vec::with_capacity(36);
    for (center, radius, angle) in [
        (bounds.left_top() + vec2(3.0, 3.0), 3.0, std::f32::consts::PI),
        (bounds.right_top() + vec2(-14.0, 14.0), 14.0, std::f32::consts::PI * 1.5),
        (bounds.right_bottom() - vec2(14.0, 14.0), 14.0, 0.0),
        (bounds.left_bottom() + vec2(3.0, -3.0), 3.0, std::f32::consts::FRAC_PI_2),
    ] {
        for step in 0..=8 {
            let angle = angle + std::f32::consts::FRAC_PI_2 * step as f32 / 8.0;
            clip.push(center + vec2(angle.cos(), angle.sin()) * radius);
        }
    }
    let normal = vec2(115.0_f32.to_radians().sin(), -115.0_f32.to_radians().cos());
    let tangent = vec2(-normal.y, normal.x);
    let content = Rect::from_min_max(bounds.min + vec2(2.0, 1.0), bounds.max - vec2(1.0, 1.0));
    let extent = content.size().length();
    let half_gradient_length =
        (content.width() * normal.x.abs() + content.height() * normal.y.abs()) / 2.0;
    let shift = normal.x * offset;
    let mut mesh = egui::Mesh::default();
    for stripe in -((extent / 10.0).ceil() as i32 + 3)..=((extent / 10.0).ceil() as i32 + 3) {
        for (start, end, color) in [
            (0.0, 4.0, Color32::from_rgba_unmultiplied(3, 8, 12, 143)),
            (4.0, 6.0, Color32::from_rgba_unmultiplied(104, 155, 166, 133)),
            (6.0, 10.0, Color32::from_rgba_unmultiplied(3, 8, 12, 143)),
        ] {
            let origin = content.center().to_vec2() - normal * half_gradient_length;
            let start = normal * (stripe as f32 * 10.0 + start + shift);
            let end = normal * (stripe as f32 * 10.0 + end + shift);
            let mut polygon = vec![
                (origin + start - tangent * extent).to_pos2(),
                (origin + end - tangent * extent).to_pos2(),
                (origin + end + tangent * extent).to_pos2(),
                (origin + start + tangent * extent).to_pos2(),
            ];
            for edge in 0..clip.len() {
                if polygon.is_empty() {
                    break;
                }
                let a = clip[edge];
                let b = clip[(edge + 1) % clip.len()];
                let side = |point: egui::Pos2| {
                    let edge = b - a;
                    let relative = point - a;
                    edge.x * relative.y - edge.y * relative.x
                };
                let input = std::mem::take(&mut polygon);
                let mut previous = *input.last().expect("nonempty clipped stripe");
                let mut previous_side = side(previous);
                for point in input {
                    let current_side = side(point);
                    if (previous_side >= 0.0) != (current_side >= 0.0) {
                        polygon.push(
                            previous.lerp(point, previous_side / (previous_side - current_side)),
                        );
                    }
                    if current_side >= 0.0 {
                        polygon.push(point);
                    }
                    previous = point;
                    previous_side = current_side;
                }
            }
            if polygon.len() < 3 {
                continue;
            }
            let base = mesh.vertices.len() as u32;
            for point in &polygon {
                mesh.colored_vertex(*point, color);
            }
            for index in 1..polygon.len() - 1 {
                mesh.add_triangle(base, base + index as u32, base + index as u32 + 1);
            }
        }
    }
    painter.with_clip_rect(content.intersect(painter.clip_rect())).add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod motion_tests {
    use super::*;
    fn row(health: f64, connected: bool) -> AllyHudRow {
        AllyHudRow {
            id: "ally".into(),
            identity: AllyHudIdentity::Golem,
            connected,
            dead: false,
            health_ratio: health,
        }
    }
    #[test]
    fn health_retargets_from_displayed_width_and_finishes_after_220ms() {
        let mut motion = AllyMotion::default();
        motion.sync(&[row(1.0, true)], 1000.0, true);
        motion.sync(&[row(0.0, true)], 1100.0, true);
        assert_eq!(motion.rows[0].health(1100.0), 1.0);
        let halfway = motion.rows[0].health(1210.0);
        assert!(halfway > 0.19 && halfway < 0.21, "CSS ease midpoint: {halfway}");
        motion.sync(&[row(0.7, true)], 1210.0, true);
        assert_eq!(motion.rows[0].health(1210.0), halfway);
        assert_eq!(motion.rows[0].health(1430.0), 0.7);
    }
    #[test]
    fn recreated_rows_snap_health_and_restart_signal_loss() {
        let mut motion = AllyMotion::default();
        motion.sync(&[row(1.0, true)], 0.0, true);
        motion.sync(&[row(0.3, false)], 500.0, true);
        assert_eq!(motion.rows[0].health(500.0), 0.3);
        assert_eq!(motion.rows[0].stripe_offset(950.0, false), 10.0);
        assert_eq!(motion.rows[0].stripe_offset(1400.0, false), 0.0);
        assert_eq!(motion.rows[0].stripe_offset(950.0, true), 0.0);
        motion.motion_preference(true, 950.0);
        motion.motion_preference(false, 1000.0);
        assert_eq!(motion.rows[0].stripe_offset(1000.0, false), 0.0);
        assert_eq!(motion.rows[0].health(1000.0), 0.3);
    }
    #[test]
    fn hidden_rows_reappear_at_current_health_without_old_tween() {
        let mut motion = AllyMotion::default();
        motion.sync(&[row(1.0, false)], 0.0, true);
        motion.sync(&[row(0.0, false)], 10.0, true);
        motion.hide();
        motion.sync(&[row(0.4, false)], 20.0, true);
        assert_eq!(motion.rows[0].health(20.0), 0.4);
        assert_eq!(motion.rows[0].stripe_offset(20.0, false), 0.0);
    }
}
