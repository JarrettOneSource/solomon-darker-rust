//! Cosmetic stock belt pull-off members. The input owner emits the canonical
//! unbind command; these GPU triangles only follow the native cosmetic plan.
use crate::gpu_ui::{StockAtlas, UiFrame};
use solomon_sim::hud::belt_pull_off::{
    NativeBeltPullOffBurstMember, NativeBeltPullOffStep, native_belt_pull_off_burst_members,
};
pub(crate) struct BeltPullOffBurst {
    started_at: f64,
    members: Vec<NativeBeltPullOffBurstMember>,
}
impl BeltPullOffBurst {
    pub fn new(now: f64) -> Self {
        let step = if js_sys::Math::random() < 0.5 {
            NativeBeltPullOffStep::Degrees90
        } else {
            NativeBeltPullOffStep::Degrees120
        };
        Self {
            started_at: now,
            members: native_belt_pull_off_burst_members(step, &mut js_sys::Math::random),
        }
    }
    pub fn sequence(&self) -> u64 {
        self.started_at.to_bits()
    }
    pub fn finished(&self, now: f64) -> bool {
        self.members.iter().all(|member| now - self.started_at >= member.duration_ms)
    }
    pub fn paint(
        &self,
        frame: &UiFrame<'_>,
        painter: &egui::Painter,
        atlas: &StockAtlas,
        center: egui::Pos2,
        now: f64,
        scale: f32,
        id: &str,
    ) -> Result<(), String> {
        let mut bounds = egui::Rect::NOTHING;
        let mut smoke = 0;
        let mut spokes = 0;
        for member in &self.members {
            let progress = ((now - self.started_at) / member.duration_ms).clamp(0.0, 1.0);
            if progress >= 1.0 {
                continue;
            }
            let point = center
                + egui::vec2(
                    (member.start.x + (member.end.x - member.start.x) * progress) as f32,
                    (member.start.y + (member.end.y - member.start.y) * progress) as f32,
                ) * scale;
            let record = atlas.record(member.record)?;
            let destination = egui::Rect::from_center_size(
                point,
                egui::vec2(
                    record.width as f32 * member.scale.x as f32 * scale,
                    record.height as f32 * member.scale.y as f32 * scale,
                ),
            );
            let size = frame.image_size(atlas.path()).ok_or("Belt pull-off atlas is not ready")?;
            let rotation = egui::emath::Rot2::from_angle(
                (member.rotation_degrees.to_radians() * progress) as f32,
            );
            for corner in [
                destination.left_top(),
                destination.right_top(),
                destination.right_bottom(),
                destination.left_bottom(),
            ] {
                bounds.extend_with(point + rotation * (corner - point));
            }
            if member.record == 65 {
                smoke += 1;
            } else if member.record == 69 {
                spokes += 1;
            }
            frame.image_rotated(
                painter,
                atlas.path(),
                destination,
                Some(egui::Rect::from_min_max(
                    egui::pos2(record.x as f32 / size.x, record.y as f32 / size.y),
                    egui::pos2(
                        (record.x + record.width) as f32 / size.x,
                        (record.y + record.height) as f32 / size.y,
                    ),
                )),
                (member.rotation_degrees.to_radians() * progress) as f32,
                point,
                egui::Color32::from_gray((member.brightness * 255.0).round() as u8)
                    .gamma_multiply((1.0 - progress) as f32),
            );
        }
        if bounds.is_positive() {
            let transform =
                frame.ctx.layer_transform_to_global(painter.layer_id()).unwrap_or_default();
            frame.record_rect(id, "Belt pull-off burst", "image", transform * bounds, false);
            frame.record_value(
                id,
                &serde_json::json!({"smoke":smoke,"spokes":spokes}).to_string(),
                None,
            );
        }
        Ok(())
    }
}
