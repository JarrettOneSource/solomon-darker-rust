//! Development-only comparison of the actual loader text owner with isolated point glyphs.
use super::*;
use solomon_graphics::TextureSampling;
use web_sys::WebGl2RenderingContext as Gl;

#[wasm_bindgen]
pub fn loader_font_fixture(gl: Gl, pixels: &[u8]) -> Result<String, JsValue> {
    probe(gl, pixels).map_err(|e| JsValue::from_str(&e))
}

fn probe(gl: Gl, pixels: &[u8]) -> Result<String, String> {
    if pixels.len() != 512 * 256 * 4 {
        return Err("Expected the exact Fonts RGBA page".into());
    }
    let window = web_sys::window().ok_or("No loader fixture window")?;
    let document = window.document().ok_or("No loader fixture document")?;
    let canvas = document
        .create_element("canvas")
        .map_err(|e| format!("Fixture canvas: {e:?}"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| "Fixture canvas type")?;
    let renderer = Renderer::new(gl.clone())?;
    let font = renderer.upload_rgba(512, 256, pixels)?;
    let mut loader = Loader {
        reader: solomon_graphics::announcements::ScreenReader::attach_loader(&document)?,
        window,
        canvas,
        renderer,
        art: Vec::new(),
        font: Some(font),
        progress: Progress::default(),
        frame: 0,
        meshes: 0,
        triangles: 0,
        viewport: [256.0; 2],
        logical: [256.0; 2],
        scale: 1.0,
        keyboard: Closure::<dyn FnMut(KeyboardEvent)>::new(|_| {}),
        widgets: Vec::new(),
        last_announced: String::new(),
    };
    let mut rows = Vec::new();
    for size in [16.0, 23.5, 36.0] {
        let mut observations = Vec::new();
        for reference in [true, false, true] {
            let mut bytes = vec![0; 256 * 256 * 4];
            loader.renderer.begin_frame(256, 256, [256.0; 2]);
            loader.renderer.draw_solid([0.0, 0.0, 256.0, 256.0], [0.1, 0.2, 0.3, 1.0]);
            if reference {
                // Uploading binds a texture; finish the background batch first.
                loader.renderer.flush();
                let mut spec = UiTextSpec::new(UiFontName::Body, "i.ii", 128.0, 128.3);
                spec.scale = f64::from(size) / ui_font(UiFontName::Body).metrics[0];
                let layout = layout_ui_text(&spec);
                let mut crops = Vec::new();
                for glyph in &layout.glyphs {
                    let [x, y, width, height] = glyph.glyph.frame.map(|v| v as usize);
                    let mut crop = Vec::with_capacity(width * height * 4);
                    for row in y..y + height {
                        let start = (row * 512 + x) * 4;
                        crop.extend_from_slice(&pixels[start..start + width * 4]);
                    }
                    crops.push(loader.renderer.upload_rgba(width as u32, height as u32, &crop)?);
                }
                loader.renderer.draw_sampled(TextureSampling::Nearest, |renderer| {
                    for (glyph, crop) in layout.glyphs.iter().zip(&crops) {
                        let ink = ui_glyph_ink_bounds(glyph);
                        let left = ink.left as f32;
                        let top = ink.top as f32;
                        let right = left + ink.width as f32;
                        let bottom = top + ink.height as f32;
                        renderer.draw_quad_with_color_mode(
                            crop,
                            [0.0, 0.0, crop.width, crop.height],
                            [[left, top], [right, top], [right, bottom], [left, bottom]],
                            [[1.0; 4]; 4],
                            BlendMode::Normal,
                            TextureColorMode::AlphaMask,
                        );
                    }
                });
                for crop in &crops {
                    loader.renderer.destroy_texture(crop);
                }
            } else {
                loader.text("fixture:font", "i.ii", 128.3, size, 0xffffff, None);
            }
            loader.renderer.flush();
            gl.read_pixels_with_opt_u8_array(
                0,
                0,
                256,
                256,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                Some(&mut bytes),
            )
            .map_err(|e| format!("Loader readback: {e:?}"))?;
            observations.push(bytes);
        }
        let maximum = observations[0]
            .iter()
            .zip(&observations[1])
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        let backgrounds: Vec<_> = observations.iter().map(|pixels| &pixels[..4]).collect();
        rows.push(serde_json::json!({"size":size,"maximumDifference":maximum,
                "restored":observations[0]==observations[2], "backgrounds":backgrounds}));
    }
    Ok(serde_json::json!(rows).to_string())
}
