//! Reused GPU-only group targets and nested composition. Nothing is read back.

use egui::{Rect, Vec2, pos2, vec2};
use solomon_graphics::{
    BlendMode, ColorFilter, RenderTarget, Renderer, Texture, TextureColorMode, Vertex,
};
use web_sys::WebGl2RenderingContext;

use super::{GpuPainter, Images, clip_scissor};
use crate::gpu_ui::{
    effects::{self, Node, Style},
    geometry::shadow_geometry,
};

#[derive(Clone, Copy)]
pub(super) struct View {
    pub physical: [i32; 2],
    pub logical: [f32; 2],
    pub origin: Vec2,
    pub clip: Rect,
}

impl View {
    pub fn screen(physical: [i32; 2], logical: [f32; 2]) -> Self {
        Self {
            physical,
            logical,
            origin: Vec2::ZERO,
            clip: Rect::from_min_size(egui::Pos2::ZERO, vec2(logical[0], logical[1])),
        }
    }
    fn scale(self) -> Vec2 {
        vec2(self.physical[0] as f32 / self.logical[0], self.physical[1] as f32 / self.logical[1])
    }
    fn target(target: &RenderTarget, bounds: Rect, scale: Vec2) -> Self {
        Self {
            physical: [target.texture.width as i32, target.texture.height as i32],
            logical: [target.texture.width / scale.x, target.texture.height / scale.y],
            origin: bounds.min.to_vec2(),
            clip: bounds,
        }
    }
}

#[derive(Default)]
struct Slot {
    source: Option<RenderTarget>,
    composite: Option<RenderTarget>,
}

#[derive(Default)]
pub(super) struct Targets {
    slots: Vec<Slot>,
    max_size: Option<u32>,
}

impl Targets {
    fn depth(&mut self, depth: usize) -> &mut Slot {
        if self.slots.len() <= depth {
            self.slots.resize_with(depth + 1, Slot::default)
        }
        &mut self.slots[depth]
    }
    pub fn texture_count(&self) -> usize {
        self.slots
            .iter()
            .map(|slot| usize::from(slot.source.is_some()) + usize::from(slot.composite.is_some()))
            .sum()
    }
    pub fn destroy(&mut self, gl: &WebGl2RenderingContext) {
        for slot in self.slots.drain(..) {
            for target in [slot.source, slot.composite].into_iter().flatten() {
                Renderer::delete_render_target(gl, &target)
            }
        }
    }
    fn get(
        &mut self,
        renderer: &mut Renderer,
        depth: usize,
        composite: bool,
        bounds: Rect,
        scale: Vec2,
    ) -> Result<RenderTarget, String> {
        let max = match self.max_size {
            Some(value) => value,
            None => {
                let value = renderer
                    .graphics_context()
                    .get_parameter(WebGl2RenderingContext::MAX_TEXTURE_SIZE)
                    .map_err(|error| format!("GPU group texture limit: {error:?}"))?
                    .as_f64()
                    .ok_or("GPU group texture limit is unavailable")?
                    as u32;
                self.max_size = Some(value);
                value
            }
        };
        let required = (bounds.size() * scale).ceil();
        if required.x > max as f32 || required.y > max as f32 {
            return Err(format!(
                "GPU group {}x{} exceeds texture limit {max}",
                required.x, required.y
            ));
        }
        let depth = self.depth(depth);
        let slot = if composite { &mut depth.composite } else { &mut depth.source };
        if let Some(target) = slot
            && target.texture.width >= required.x
            && target.texture.height >= required.y
        {
            return Ok(target.clone());
        }
        let old = slot
            .as_ref()
            .map_or(Vec2::ZERO, |target| vec2(target.texture.width, target.texture.height));
        let size = required.max(old);
        let size = [size.x.max(1.0) as u32, size.y.max(1.0) as u32]
            .map(|value| value.next_power_of_two().min(max));
        // Allocation binds a texture. Submit preceding users before replacing a target.
        renderer.flush();
        let next = renderer.create_render_target_size(size)?;
        if let Some(previous) = slot.replace(next.clone()) {
            renderer.destroy_render_target(&previous)
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parent_groups_preserve_the_nested_target_pool_between_siblings() {
        let mut targets = Targets::default();
        targets.depth(2);
        targets.depth(0);
        targets.depth(1);
        assert_eq!(targets.slots.len(), 3);
    }
    #[test]
    fn group_capture_alignment_uses_the_actual_framebuffer_scale() {
        // The actual target formula is also exercised by the GPU composition probe.
        let view = View {
            physical: [128, 64],
            logical: [64.0, 32.0],
            origin: vec2(-10.0, 12.0),
            clip: Rect::from_min_size(pos2(-10.0, 12.0), vec2(40.0, 20.0)),
        };
        assert_eq!(view.scale(), vec2(2.0, 2.0));
        assert_eq!(
            aligned(Rect::from_min_max(pos2(-1.2, 2.2), pos2(5.1, 8.7)), view.scale()),
            Rect::from_min_max(pos2(-1.5, 2.0), pos2(5.5, 9.0))
        );
    }
}

fn aligned(bounds: Rect, scale: Vec2) -> Rect {
    Rect::from_min_max(
        ((bounds.min.to_vec2() * scale).floor() / scale).to_pos2(),
        ((bounds.max.to_vec2() * scale).ceil() / scale).to_pos2(),
    )
}

fn target_uv(texture: &Texture, bounds: Rect, scale: Vec2) -> Rect {
    let size = bounds.size() * scale;
    // A top-left capture occupies the top rows of a vertically inverted GL target.
    Rect::from_min_max(pos2(0.0, 1.0), pos2(size.x / texture.width, 1.0 - size.y / texture.height))
}

impl GpuPainter {
    pub(super) fn draw_nodes(
        &mut self,
        renderer: &mut Renderer,
        images: &Images,
        nodes: &[Node],
        view: View,
        depth: usize,
    ) -> Result<(usize, usize), String> {
        let mut counts = (0, 0);
        for node in nodes {
            let rendered = match node {
                Node::Mesh { mesh, clip } => self.draw_mesh(renderer, images, mesh, *clip, view)?,
                Node::Group { style, clip, children } => {
                    self.draw_group(renderer, images, children, style, *clip, view, depth)?
                }
            };
            counts.0 += rendered.0;
            counts.1 += rendered.1;
        }
        Ok(counts)
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_group(
        &mut self,
        renderer: &mut Renderer,
        images: &Images,
        children: &[Node],
        style: &Style,
        clip: Rect,
        view: View,
        depth: usize,
    ) -> Result<(usize, usize), String> {
        let clip = clip.intersect(view.clip);
        if !clip.is_positive() || style.effect.opacity == 0.0 {
            return Ok((0, 0));
        }
        let source =
            effects::content_bounds(children).intersect(effects::capture_clip(clip, &style.effect));
        if !source.is_positive() {
            return Ok((0, 0));
        }
        let scale = view.scale();
        let source = aligned(source, scale);
        let target = self.groups.get(renderer, depth, false, source, scale)?;
        let capture = View::target(&target, source, scale);
        let mut rendered = Ok((0, 0));
        renderer.render_ui_group(&target, capture.logical, |renderer| {
            rendered = self.draw_nodes(renderer, images, children, capture, depth + 1);
        });
        let mut counts = rendered?;
        let source_uv = target_uv(&target.texture, source, scale);
        if style.effect.shadows.len() <= 1 {
            let added = draw_texture(
                renderer,
                &target.texture,
                source,
                source_uv,
                style.effect.shadows.first().copied(),
                false,
                style.filter,
                style.blend,
                style.effect.opacity,
                clip,
                view,
            );
            counts.0 += added.0;
            counts.1 += added.1;
        } else {
            let output =
                aligned(effects::effect_bounds(source, &style.effect).intersect(clip), scale);
            let composite = self.groups.get(renderer, depth, true, output, scale)?;
            let capture = View::target(&composite, output, scale);
            renderer.render_ui_group(&composite, capture.logical, |renderer| {
                for shadow in style.effect.shadows.iter().rev() {
                    let added = draw_texture(
                        renderer,
                        &target.texture,
                        source,
                        source_uv,
                        Some(*shadow),
                        true,
                        ColorFilter::default(),
                        BlendMode::Normal,
                        1.0,
                        output,
                        capture,
                    );
                    counts.0 += added.0;
                    counts.1 += added.1;
                }
                let added = draw_texture(
                    renderer,
                    &target.texture,
                    source,
                    source_uv,
                    None,
                    false,
                    style.filter,
                    BlendMode::Normal,
                    1.0,
                    output,
                    capture,
                );
                counts.0 += added.0;
                counts.1 += added.1;
            });
            let added = draw_texture(
                renderer,
                &composite.texture,
                output,
                target_uv(&composite.texture, output, scale),
                None,
                false,
                ColorFilter::default(),
                style.blend,
                style.effect.opacity,
                clip,
                view,
            );
            counts.0 += added.0;
            counts.1 += added.1;
        }
        Ok(counts)
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_texture(
    renderer: &mut Renderer,
    texture: &Texture,
    destination: Rect,
    uv: Rect,
    shadow: Option<crate::gpu_ui::ImageShadow>,
    only: bool,
    filter: ColorFilter,
    blend: BlendMode,
    opacity: f32,
    clip: Rect,
    view: View,
) -> (usize, usize) {
    let Some(clip) = clip_scissor(
        clip.intersect(view.clip).translate(-view.origin),
        view.physical,
        view.logical,
    ) else {
        return (0, 0);
    };
    let (destination, uv, shadow) = if let Some(shadow) = shadow {
        let (destination, uv, mut shadow) = shadow_geometry(uv, destination, shadow);
        shadow.only = only;
        (destination, uv, Some(shadow))
    } else {
        (destination, uv, None)
    };
    let positions = [
        destination.left_top(),
        destination.right_top(),
        destination.right_bottom(),
        destination.left_bottom(),
    ];
    let uvs = [uv.left_top(), uv.right_top(), uv.right_bottom(), uv.left_bottom()];
    let vertices: [Vertex; 4] = std::array::from_fn(|index| Vertex {
        position: [positions[index].x - view.origin.x, positions[index].y - view.origin.y],
        texel: [uvs[index].x * texture.width, uvs[index].y * texture.height],
        tint: [1.0, 1.0, 1.0, opacity],
    });
    let triangles =
        [[vertices[0], vertices[1], vertices[2]], [vertices[0], vertices[2], vertices[3]]];
    renderer.draw_clipped(clip, 1.0, |renderer| {
        let draw = |renderer: &mut Renderer| {
            renderer.draw_triangles_with_color_mode(
                texture,
                &triangles,
                blend,
                TextureColorMode::Premultiplied,
            )
        };
        let filtered = |renderer: &mut Renderer| {
            if filter == ColorFilter::default() {
                draw(renderer)
            } else {
                renderer.draw_color_filtered(filter, draw)
            }
        };
        if let Some(shadow) = shadow {
            renderer.draw_image_shadow(shadow, filtered)
        } else {
            filtered(renderer)
        };
    });
    (1, 2)
}
