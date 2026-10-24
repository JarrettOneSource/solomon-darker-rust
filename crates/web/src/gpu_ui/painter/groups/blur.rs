//! Dense separable alpha convolution for large native shadows.

use super::*;
use crate::gpu_ui::ImageShadow;

#[derive(Clone)]
struct AlphaImage {
    texture: Texture,
    bounds: Rect,
    uv: Rect,
}

fn uv_bounds(uv: Rect) -> [f32; 4] {
    [uv.min.x.min(uv.max.x), uv.min.y.min(uv.max.y), uv.min.x.max(uv.max.x), uv.min.y.max(uv.max.y)]
}

fn extended_uv(uv: Rect, source: Rect, destination: Rect) -> Rect {
    Rect::from_min_max(
        uv.min + (destination.min - source.min) / source.size() * uv.size(),
        uv.min + (destination.max - source.min) / source.size() * uv.size(),
    )
}

impl GpuPainter {
    fn blur_alpha(
        &mut self,
        renderer: &mut Renderer,
        source: &AlphaImage,
        sigma: f32,
        view: View,
        depth: usize,
    ) -> Result<(AlphaImage, (usize, usize)), String> {
        if sigma == 0.0 {
            return Ok((source.clone(), (0, 0)));
        }
        let scale = view.scale();
        let bounds = aligned(source.bounds.expand(sigma * 3.0), scale);
        let horizontal = self.groups.get(renderer, depth, TargetSlot::Horizontal, bounds, scale)?;
        let vertical = self.groups.get(renderer, depth, TargetSlot::Vertical, bounds, scale)?;
        let mut counts = (0, 0);
        renderer.render_ui_group(
            &horizontal,
            View::target(&horizontal, bounds, scale).logical,
            |renderer| {
                renderer.draw_alpha_blurred(
                    uv_bounds(source.uv),
                    [sigma * scale.x / source.texture.width, 0.0],
                    |renderer| {
                        counts = draw_texture(
                            renderer,
                            &source.texture,
                            bounds,
                            extended_uv(source.uv, source.bounds, bounds),
                            None,
                            ShadowOutput::Source,
                            ColorFilter::default(),
                            BlendMode::Normal,
                            1.0,
                            bounds,
                            View::target(&horizontal, bounds, scale),
                        );
                    },
                );
            },
        );
        let uv = target_uv(&horizontal.texture, bounds, scale);
        renderer.render_ui_group(
            &vertical,
            View::target(&vertical, bounds, scale).logical,
            |renderer| {
                renderer.draw_alpha_blurred(
                    uv_bounds(uv),
                    [0.0, sigma * scale.y / horizontal.texture.height],
                    |renderer| {
                        let added = draw_texture(
                            renderer,
                            &horizontal.texture,
                            bounds,
                            uv,
                            None,
                            ShadowOutput::Source,
                            ColorFilter::default(),
                            BlendMode::Normal,
                            1.0,
                            bounds,
                            View::target(&vertical, bounds, scale),
                        );
                        counts.0 += added.0;
                        counts.1 += added.1;
                    },
                );
            },
        );
        Ok((
            AlphaImage {
                texture: vertical.texture.clone(),
                bounds,
                uv: target_uv(&vertical.texture, bounds, scale),
            },
            counts,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_blurred_group(
        &mut self,
        renderer: &mut Renderer,
        images: &Images,
        target: &RenderTarget,
        source: Rect,
        style: &Style,
        knockout: Option<&[Node]>,
        view: View,
        depth: usize,
    ) -> Result<(usize, usize), String> {
        let scale = view.scale();
        let source = AlphaImage {
            texture: target.texture.clone(),
            bounds: source,
            uv: target_uv(&target.texture, source, scale),
        };
        let output = aligned(
            effects::effect_bounds(source.bounds, &style.effect).intersect(view.clip),
            scale,
        );
        let composite = self.groups.get(renderer, depth, TargetSlot::Composite, output, scale)?;
        let capture = View::target(&composite, output, scale);
        let mut result = Ok((0, 0));
        renderer.render_ui_group(&composite, capture.logical, |renderer| {
            result = (|| {
                let mut counts = (0, 0);
                for shadow in style.effect.shadows.iter().rev() {
                    let (blurred, added) =
                        self.blur_alpha(renderer, &source, shadow.blur, view, depth)?;
                    counts.0 += added.0;
                    counts.1 += added.1;
                    if style.output == ShadowOutput::Inner {
                        let added =
                            colored_alpha(renderer, &source, *shadow, Vec2::ZERO, output, capture);
                        counts.0 += added.0;
                        counts.1 += added.1;
                        let added = renderer.draw_erased(|renderer| {
                            plain_alpha(renderer, &blurred, shadow.offset, output, capture)
                        });
                        counts.0 += added.0;
                        counts.1 += added.1;
                    } else {
                        let added = colored_alpha(
                            renderer,
                            &blurred,
                            *shadow,
                            shadow.offset,
                            output,
                            capture,
                        );
                        counts.0 += added.0;
                        counts.1 += added.1;
                    }
                }
                if style.output == ShadowOutput::Outer {
                    let added = renderer.draw_erased(|renderer| {
                        if let Some(knockout) = knockout {
                            self.draw_nodes(renderer, images, knockout, capture, depth + 1)
                        } else {
                            Ok(plain_alpha(renderer, &source, Vec2::ZERO, output, capture))
                        }
                    })?;
                    counts.0 += added.0;
                    counts.1 += added.1;
                } else if style.output == ShadowOutput::Source {
                    let added = draw_texture(
                        renderer,
                        &source.texture,
                        source.bounds,
                        source.uv,
                        None,
                        ShadowOutput::Source,
                        style.filter,
                        BlendMode::Normal,
                        1.0,
                        output,
                        capture,
                    );
                    counts.0 += added.0;
                    counts.1 += added.1;
                }
                Ok(counts)
            })();
        });
        let mut counts = result?;
        let added = draw_texture(
            renderer,
            &composite.texture,
            output,
            target_uv(&composite.texture, output, scale),
            None,
            ShadowOutput::Source,
            ColorFilter::default(),
            style.blend,
            style.effect.opacity,
            view.clip,
            view,
        );
        counts.0 += added.0;
        counts.1 += added.1;
        Ok(counts)
    }
}

fn colored_alpha(
    renderer: &mut Renderer,
    image: &AlphaImage,
    shadow: ImageShadow,
    offset: Vec2,
    clip: Rect,
    view: View,
) -> (usize, usize) {
    draw_texture(
        renderer,
        &image.texture,
        image.bounds.translate(offset),
        image.uv,
        Some(ImageShadow { offset: Vec2::ZERO, blur: 0.0, color: shadow.color }),
        ShadowOutput::Shadow,
        ColorFilter::default(),
        BlendMode::Normal,
        1.0,
        clip,
        view,
    )
}

fn plain_alpha(
    renderer: &mut Renderer,
    image: &AlphaImage,
    offset: Vec2,
    clip: Rect,
    view: View,
) -> (usize, usize) {
    draw_texture(
        renderer,
        &image.texture,
        image.bounds.translate(offset),
        image.uv,
        None,
        ShadowOutput::Source,
        ColorFilter::default(),
        BlendMode::Normal,
        1.0,
        clip,
        view,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padding_preserves_inverted_target_uvs_and_excludes_reused_texture_rows() {
        let uv = Rect::from_min_max(pos2(0.0, 1.0), pos2(0.5, 0.75));
        assert_eq!(uv_bounds(uv), [0.0, 0.75, 0.5, 1.0]);
        let source = Rect::from_min_size(pos2(10.0, 20.0), vec2(20.0, 10.0));
        let destination = source.expand(10.0);
        let padded = extended_uv(uv, source, destination);
        assert_eq!(padded.min, pos2(-0.25, 1.25));
        assert_eq!(padded.max, pos2(0.75, 0.5));
    }
}
