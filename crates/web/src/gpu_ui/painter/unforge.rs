//! One retained scratch target for the typed InventoryScreen paint operation.
use solomon_graphics::{BlendMode, Quad, Renderer, Texture, TextureColorMode, TextureSampling};

use super::{GpuPainter, Images, clip_scissor, groups::View};
use crate::gpu_ui::unforge::{CAPTURE_SIDE, Paint};

impl GpuPainter {
    pub(super) fn draw_unforge(
        &mut self,
        renderer: &mut Renderer,
        images: &Images,
        paint: &Paint,
        clip: egui::Rect,
        view: View,
    ) -> Result<(usize, usize), String> {
        let Some(clip) = clip_scissor(
            clip.intersect(view.clip).translate(-view.origin),
            view.physical,
            view.logical,
        ) else {
            return Ok((0, 0));
        };
        if paint.opacity == 0.0 {
            return Ok((0, 0));
        }
        let atlas = images.cached_texture(&paint.path).ok_or("Unforge UI atlas is not ready")?;
        let target = if let Some(target) = &self.unforge_target {
            target.clone()
        } else {
            renderer.flush();
            let target = renderer.create_render_target(CAPTURE_SIDE)?;
            self.unforge_target = Some(target.clone());
            target
        };
        renderer.draw_sampled(TextureSampling::Texture, |renderer| {
            renderer.render_native_ui_capture(&target, [CAPTURE_SIDE as f32; 2], |renderer| {
                let [left, top, width, height] = paint.frame.clip;
                renderer.draw_clipped(
                    [left, CAPTURE_SIDE as i32 - top - height, width, height],
                    1.0,
                    |renderer| {
                        for center in paint.frame.image_centers {
                            draw_record(
                                renderer,
                                &atlas,
                                &paint.records[0],
                                center,
                                BlendMode::Normal,
                            );
                        }
                        draw_record(
                            renderer,
                            &atlas,
                            &paint.records[1],
                            [128.0; 2],
                            BlendMode::Multiply,
                        );
                    },
                );
            });
            renderer.draw_clipped(clip, 1.0, |renderer| {
                let positions = |points: [egui::Pos2; 4]| {
                    points.map(|point| [point.x - view.origin.x, point.y - view.origin.y])
                };
                // RenderToSprite subtracts half a pixel from the quad. The raw
                // native target is sampled as straight RGBA, including alpha².
                renderer.draw_quad_with_color_mode(
                    &target.texture,
                    [0.0, CAPTURE_SIDE as f32, CAPTURE_SIDE as f32, -(CAPTURE_SIDE as f32)],
                    positions(paint.target_positions),
                    [[1.0, 1.0, 1.0, paint.opacity]; 4],
                    BlendMode::Normal,
                    TextureColorMode::Texture,
                );
                let marker = &paint.records[2];
                renderer.draw_quad(
                    &atlas,
                    record_source(marker),
                    positions(paint.marker_positions),
                    [[paint.frame.marker_red, 1.0, 1.0, paint.opacity]; 4],
                    BlendMode::Normal,
                );
            });
        });
        Ok((5, 10))
    }
}

fn draw_record(
    renderer: &mut Renderer,
    atlas: &Texture,
    record: &solomon_assets::SpriteRecord,
    center: [f32; 2],
    blend: BlendMode,
) {
    let [left, top] = record.registered_origin();
    renderer.draw(
        atlas,
        Quad {
            source: record_source(record),
            destination: [
                center[0] + left - record.logical_width as f32 / 2.0,
                center[1] + top - record.logical_height as f32 / 2.0,
                record.width as f32,
                record.height as f32,
            ],
            tint: [1.0; 4],
            blend,
        },
    );
}

// Sprite 0x00413DE0; generated target UVs above deliberately remain full0..1.
fn record_source(record: &solomon_assets::SpriteRecord) -> [f32; 4] {
    [
        record.x as f32 + 0.5,
        record.y as f32 + 0.5,
        record.width as f32 - 0.25,
        record.height as f32 - 0.25,
    ]
}
