//! Compare atlas sampling with an independently uploaded, isolated source image.
//! The record is next to opaque white pixels, as College's statue is at (1757, 197).

use glow::HasContext;
use solomon_graphics::{BlendMode, ColorFilter, Quad, Renderer};

pub fn probe(renderer: &mut Renderer) -> Result<String, String> {
    let mut pixels = vec![255; 2048 * 256 * 4];
    let source: Vec<u8> = (0..16)
        .flat_map(|i| [i * 11, 180 - i * 7, 40 + i * 9, if i < 4 { 0 } else { i * 16 }])
        .collect();
    for row in 0..4 {
        let start = ((197 + row) * 2048 + 1757) * 4;
        pixels[start..start + 16].copy_from_slice(&source[row * 16..(row + 1) * 16]);
    }
    let atlas = renderer.upload_rgba(2048, 256, &pixels)?;
    let isolated = renderer.upload_rgba(4, 4, &source)?;
    let target = renderer.create_render_target_size([32, 32])?;
    let gl = renderer.graphics_context();
    let result = (|| {
        let mut cases = Vec::new();
        for centers in [false, true] {
            renderer.set_native_world_pixel_centers(centers);
            for filtered in [false, true] {
                for (scale, offset) in [(1.2, 0.2), (3.5, 0.7), (0.7, 0.3)] {
                    let mut observations = Vec::new();
                    for from_atlas in [false, true, false] {
                        let mut image = vec![0; 32 * 32 * 4];
                        renderer.render_to_color(
                            &target,
                            [32.0; 2],
                            [0.1, 0.2, 0.3, 1.0],
                            |renderer| {
                                let draw = |renderer: &mut Renderer| {
                                    let quad = Quad {
                                        source: if from_atlas {
                                            [1757.0, 197.0, 4.0, 4.0]
                                        } else {
                                            [0.0, 0.0, 4.0, 4.0]
                                        },
                                        destination: [
                                            4.0 + offset,
                                            5.0 + offset,
                                            4.0 * scale,
                                            4.0 * scale,
                                        ],
                                        tint: [1.0; 4],
                                        blend: BlendMode::Normal,
                                    };
                                    if from_atlas {
                                        renderer.draw_clamped(&atlas, quad);
                                    } else {
                                        renderer.draw(&isolated, quad);
                                    }
                                };
                                if filtered {
                                    renderer.draw_color_filtered(ColorFilter::default(), draw);
                                } else {
                                    draw(renderer);
                                }
                                renderer.flush();
                                // SAFETY: this live context writes the initialized target-sized slice.
                                unsafe {
                                    gl.read_pixels(
                                        0,
                                        0,
                                        32,
                                        32,
                                        glow::RGBA,
                                        glow::UNSIGNED_BYTE,
                                        glow::PixelPackData::Slice(Some(&mut image)),
                                    );
                                }
                            },
                        );
                        observations.push(image);
                    }
                    let maximum = observations[0]
                        .iter()
                        .zip(&observations[1])
                        .map(|(left, right)| left.abs_diff(*right))
                        .max()
                        .unwrap_or(0);
                    let restored = observations[0] == observations[2];
                    cases.push(format!("{{\"pixelCenters\":{centers},\"filtered\":{filtered},\"scale\":{scale},\"offset\":{offset},\"maximumDifference\":{maximum},\"restored\":{restored}}}"));
                }
            }
        }
        // SAFETY: the probe still owns the live current graphics context.
        if unsafe { gl.get_error() } != glow::NO_ERROR {
            return Err("Atlas crop proof produced a graphics error".to_owned());
        }
        Ok(format!("[{}]", cases.join(",")))
    })();
    renderer.set_native_world_pixel_centers(false);
    renderer.destroy_render_target(&target);
    renderer.destroy_texture(&atlas);
    renderer.destroy_texture(&isolated);
    result
}
