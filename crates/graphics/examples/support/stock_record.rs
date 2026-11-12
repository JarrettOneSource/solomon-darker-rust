//! Real atlas pixels compared with an independently uploaded crop.

use glow::HasContext;
use solomon_graphics::{BlendMode, Quad, Renderer};

pub struct Sample {
    pub source: [u32; 4],
    pub native_uv: bool,
    pub isolated: bool,
    pub scale: f32,
}

pub fn render(
    renderer: &mut Renderer,
    size: [u32; 2],
    pixels: &[u8],
    sample: &Sample,
) -> Result<Vec<u8>, String> {
    let [x, y, width, height] = sample.source;
    let texture = if sample.isolated {
        // Preserve the original normalization denominator. Independent crop
        // allocations can round linear interpolation differently at sharp edges.
        // Extrude only this crop across the page, with no product sample bounds.
        let mut crop = Vec::with_capacity(pixels.len());
        for target_y in 0..size[1] {
            let source_y = target_y.clamp(y, y + height - 1);
            for target_x in 0..size[0] {
                let source_x = target_x.clamp(x, x + width - 1);
                let start = ((source_y * size[0] + source_x) * 4) as usize;
                crop.extend_from_slice(&pixels[start..start + 4]);
            }
        }
        renderer.upload_rgba(size[0], size[1], &crop)?
    } else {
        renderer
            .upload_rgba(size[0], size[1], pixels)?
            .clamped_to(sample.source.map(|value| value as f32))
    };
    let target = renderer.create_render_target_size([256, 256])?;
    let gl = renderer.graphics_context();
    let mut image = vec![0; 256 * 256 * 4];
    let left = x as f32;
    let top = y as f32;
    let source = if sample.native_uv {
        [left + 0.5, top + 0.5, width as f32 - 0.25, height as f32 - 0.25]
    } else {
        [left, top, width as f32, height as f32]
    };
    renderer.set_world_pixel_centers(true);
    renderer.render_to_color(&target, [256.0; 2], [0.0; 4], |renderer| {
        renderer.draw(
            &texture,
            Quad {
                source,
                destination: [8.2, 9.7, width as f32 * sample.scale, height as f32 * sample.scale],
                tint: [1.0; 4],
                blend: BlendMode::Normal,
            },
        );
        renderer.flush();
        // SAFETY: this live context writes the initialized target-sized slice.
        unsafe {
            gl.read_pixels(
                0,
                0,
                256,
                256,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut image)),
            );
        }
    });
    renderer.set_world_pixel_centers(false);
    renderer.destroy_render_target(&target);
    renderer.destroy_texture(&texture);
    Ok(image)
}
