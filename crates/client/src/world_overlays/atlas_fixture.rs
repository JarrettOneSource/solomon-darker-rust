//! Actual nameplate owner compared with independently uploaded point glyphs.

use super::*;
use solomon_graphics::TextureSampling;
use solomon_sim::ui_text::ui_glyph_ink_bounds;
use wasm_bindgen::prelude::*;
use web_sys::WebGl2RenderingContext as Gl;

#[wasm_bindgen]
pub fn world_font_fixture(gl: Gl, pixels: &[u8]) -> Result<String, JsValue> {
    probe(gl, pixels).map_err(|error| JsValue::from_str(&error))
}

fn view(font_atlas: Texture) -> WorldOverlaysView {
    WorldOverlaysView {
        font_atlas,
        world_key: None,
        speeches: Vec::new(),
        nameplates: Vec::new(),
        activities: Vec::new(),
        speech_views: Vec::new(),
    }
}

fn probe(gl: Gl, pixels: &[u8]) -> Result<String, String> {
    if pixels.len() != 512 * 256 * 4 {
        return Err("Expected the exact Fonts RGBA page".into());
    }
    let mut renderer = Renderer::new(gl.clone())?;
    let atlas = renderer.upload_rgba(512, 256, pixels)?;
    let blank = renderer.upload_rgba(512, 256, &vec![0; pixels.len()])?;
    let ramp = renderer.upload_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255])?;
    let mut actual = view(atlas);
    let mut reference = view(blank);
    let players = [WorldOverlayPlayer {
        id: "remote",
        name: "i.MW 0.ii",
        element: WizardElement::Air,
        position: Vec2::new(128.0, 150.0),
        current_health: 50.0,
        maximum_health: 100.0,
        activity: None,
    }];
    let camera = Camera { origin: Vec2::ZERO, scale: 1.0 };
    let frame = WorldOverlayFrame {
        world_key: "hub",
        players: &players,
        local_player_id: "local",
        camera: &camera,
        viewport: Vec2::new(256.0, 256.0),
        now_ms: 0.0,
        renderable: true,
    };
    let result = (|| {
        let mut rows = Vec::new();
        for size in [256, 384, 512] {
            let mut observations = Vec::new();
            for isolated in [true, false, true] {
                renderer.begin_frame(size, size, [256.0; 2]);
                renderer.draw_solid([0.0, 0.0, 256.0, 256.0], [0.1, 0.2, 0.3, 1.0]);
                if isolated {
                    // Keep the real plate geometry, then paint each original glyph
                    // from its own allocation. No atlas bounds participate here.
                    reference.draw(&mut renderer, &frame);
                    renderer.flush();
                    for nameplate in &reference.nameplates {
                        let position = nameplate.position.ok_or("Nameplate was culled")?;
                        let mut crops = Vec::new();
                        for glyph in &nameplate.layout.glyphs {
                            let [x, y, width, height] =
                                glyph.glyph.frame.map(|value| value as usize);
                            // Keep the original normalization denominator for nearest
                            // sampling: different-size textures can round exact texel
                            // boundary ties differently across GL implementations.
                            // All other artwork is replaced by the isolated crop's
                            // edge texels, so no shader clamp is used by this reference.
                            let mut crop = Vec::with_capacity(pixels.len());
                            for target_y in 0..256_usize {
                                let source_y = target_y.clamp(y, y + height - 1);
                                for target_x in 0..512_usize {
                                    let source_x = target_x.clamp(x, x + width - 1);
                                    let start = (source_y * 512 + source_x) * 4;
                                    crop.extend_from_slice(&pixels[start..start + 4]);
                                }
                            }
                            crops.push(renderer.upload_rgba(512, 256, &crop)?);
                        }
                        renderer.draw_sampled(TextureSampling::Nearest, |renderer| {
                            for shadow in [true, false] {
                                for (glyph, crop) in nameplate.layout.glyphs.iter().zip(&crops) {
                                    let ink = ui_glyph_ink_bounds(glyph);
                                    let offset = nameplate.layout.glyph_offset;
                                    renderer.draw(
                                        crop,
                                        Quad {
                                            source: [
                                                glyph.glyph.frame[0] as f32 + 0.5,
                                                glyph.glyph.frame[1] as f32 + 0.5,
                                                glyph.glyph.frame[2] as f32 - 0.25,
                                                glyph.glyph.frame[3] as f32 - 0.25,
                                            ],
                                            destination: [
                                                (position.x + offset.x + ink.left) as f32,
                                                (position.y
                                                    + offset.y
                                                    + ink.top
                                                    + if shadow { 1.0 } else { 0.0 })
                                                    as f32,
                                                ink.width as f32,
                                                ink.height as f32,
                                            ],
                                            tint: graphics::rgba(
                                                if shadow { 0 } else { 0xefe3c6 },
                                                if shadow { 0.5 } else { 1.0 },
                                            ),
                                            blend: BlendMode::Normal,
                                        },
                                    );
                                }
                            }
                        });
                        renderer.flush();
                        for crop in crops {
                            renderer.destroy_texture(&crop);
                        }
                    }
                } else {
                    actual.draw(&mut renderer, &frame);
                }
                // This ordinary linear ramp exposes a nearest sampler left behind.
                renderer.draw(
                    &ramp,
                    Quad {
                        source: [0.0, 0.0, 2.0, 1.0],
                        destination: [16.3, 180.2, 80.0, 20.0],
                        tint: [1.0; 4],
                        blend: BlendMode::Normal,
                    },
                );
                renderer.flush();
                let mut image = vec![0; (size * size * 4) as usize];
                gl.read_pixels_with_opt_u8_array(
                    0,
                    0,
                    size,
                    size,
                    Gl::RGBA,
                    Gl::UNSIGNED_BYTE,
                    Some(&mut image),
                )
                .map_err(|error| format!("World font pixels: {error:?}"))?;
                observations.push(image);
            }
            rows.push(serde_json::json!({
                "physicalSize":size,
                "maximumDifference":observations[0].iter().zip(&observations[1])
                    .map(|(left, right)| left.abs_diff(*right)).max().unwrap_or(0),
                "restored":observations[0] == observations[2],
                "changedPixels":observations[0].as_chunks::<4>().0.iter().zip(observations[1].as_chunks::<4>().0.iter())
                    .filter(|(left, right)| left.iter().zip(*right).any(|(a,b)| a.abs_diff(*b) > 1)).count(),
                "background": &observations[1][..4],
            }));
        }
        if gl.get_error() != Gl::NO_ERROR {
            return Err("World font graphics error".into());
        }
        serde_json::to_string(&rows).map_err(|error| error.to_string())
    })();
    renderer.destroy_texture(&actual.font_atlas);
    renderer.destroy_texture(&reference.font_atlas);
    renderer.destroy_texture(&ramp);
    result
}
