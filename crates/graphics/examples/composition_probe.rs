//! Actual renderer regression for nested GPU captures and group alpha.
//! Run the bindgen web output with tools/browser-smoke/gpu-composition.cjs.

#[cfg(target_arch = "wasm32")]
mod browser {
    use solomon_graphics::{BlendMode, Renderer, Texture, TextureColorMode};
    use wasm_bindgen::prelude::*;
    use web_sys::WebGl2RenderingContext as Gl;

    #[wasm_bindgen]
    pub fn composition_probe(gl: Gl) -> Result<String, JsValue> {
        probe(gl).map_err(|error| JsValue::from_str(&error))
    }

    fn pixel(gl: &Gl, x: i32, y: i32) -> Result<[u8; 4], String> {
        let mut pixel = [0; 4];
        gl.read_pixels_with_opt_u8_array(x, y, 1, 1, Gl::RGBA, Gl::UNSIGNED_BYTE, Some(&mut pixel))
            .map_err(|error| format!("capture pixel: {error:?}"))?;
        Ok(pixel)
    }

    fn viewport(gl: &Gl) -> Result<Vec<i32>, String> {
        let value = gl
            .get_parameter(Gl::VIEWPORT)
            .map_err(|error| format!("capture viewport: {error:?}"))?;
        Ok(js_sys::Int32Array::new(&value).to_vec())
    }

    fn quad(
        renderer: &mut Renderer,
        texture: &Texture,
        source: [f32; 4],
        [x, y, width, height]: [f32; 4],
        mode: TextureColorMode,
        opacity: f32,
    ) {
        renderer.draw_quad_with_color_mode(
            texture,
            source,
            [[x, y], [x + width, y], [x + width, y + height], [x, y + height]],
            [[1.0, 1.0, 1.0, opacity]; 4],
            BlendMode::Normal,
            mode,
        );
    }

    fn probe(gl: Gl) -> Result<String, String> {
        let mut renderer = Renderer::new(gl.clone())?;
        let red = renderer.upload_rgba(1, 1, &[255, 0, 0, 128])?;
        let green = renderer.upload_rgba(1, 1, &[0, 255, 0, 128])?;
        let outer = renderer.create_render_target_size([3, 2])?;
        renderer.begin_frame(5, 4, [5.0, 4.0]);
        let mut capture = Err("outer capture did not execute".to_owned());
        renderer.render_ui_group(&outer, [3.0, 2.0], |renderer| {
            capture = (|| {
                quad(
                    renderer,
                    &red,
                    [0.0, 0.0, 1.0, 1.0],
                    [0.0, 0.0, 3.0, 2.0],
                    TextureColorMode::Texture,
                    1.0,
                );
                // Allocation binds a new texture; the caller submits the prior batch first.
                renderer.flush();
                let framebuffer = gl
                    .get_parameter(Gl::FRAMEBUFFER_BINDING)
                    .map_err(|error| format!("outer framebuffer: {error:?}"))?;
                let inner = renderer.create_render_target_size([1, 1])?;
                let allocation_restored = gl
                    .get_parameter(Gl::FRAMEBUFFER_BINDING)
                    .map_err(|error| format!("allocated framebuffer: {error:?}"))?
                    == framebuffer;
                renderer.render_ui_group(&inner, [1.0, 1.0], |renderer| {
                    quad(
                        renderer,
                        &green,
                        [0.0, 0.0, 1.0, 1.0],
                        [0.0, 0.0, 1.0, 1.0],
                        TextureColorMode::Texture,
                        1.0,
                    );
                });
                let nested_restored = gl
                    .get_parameter(Gl::FRAMEBUFFER_BINDING)
                    .map_err(|error| format!("nested framebuffer: {error:?}"))?
                    == framebuffer
                    && viewport(&gl)? == [0, 0, 3, 2];
                quad(
                    renderer,
                    &inner.texture,
                    [0.0, 1.0, 1.0, -1.0],
                    [1.0, 0.0, 1.0, 1.0],
                    TextureColorMode::Premultiplied,
                    1.0,
                );
                renderer.flush();
                let overlap = pixel(&gl, 1, 1)?;
                let border = pixel(&gl, 0, 1)?;
                renderer.destroy_render_target(&inner);
                Ok((allocation_restored, nested_restored, overlap, border))
            })();
        });
        let (allocation_restored, nested_restored, overlap, border) = capture?;
        let screen_restored = gl
            .get_parameter(Gl::FRAMEBUFFER_BINDING)
            .map_err(|error| format!("screen framebuffer: {error:?}"))?
            .is_null()
            && viewport(&gl)? == [0, 0, 5, 4];
        quad(
            &mut renderer,
            &outer.texture,
            [0.0, 2.0, 3.0, -2.0],
            [1.0, 1.0, 3.0, 2.0],
            TextureColorMode::Premultiplied,
            1.0,
        );
        renderer.end_frame();
        let screen = pixel(&gl, 2, 2)?;
        let outside = pixel(&gl, 0, 0)?;
        renderer.begin_frame(5, 4, [5.0, 4.0]);
        quad(
            &mut renderer,
            &outer.texture,
            [0.0, 2.0, 3.0, -2.0],
            [1.0, 1.0, 3.0, 2.0],
            TextureColorMode::Premultiplied,
            0.5,
        );
        renderer.end_frame();
        let faded = pixel(&gl, 2, 2)?;
        renderer.destroy_render_target(&outer);
        renderer.destroy_texture(&red);
        renderer.destroy_texture(&green);
        let error = gl.get_error();
        if error != Gl::NO_ERROR {
            return Err(format!("GPU composition error {error}"));
        }
        Ok(format!(
            "{{\"allocationRestored\":{allocation_restored},\"nestedRestored\":{nested_restored},\"screenRestored\":{screen_restored},\"overlap\":{overlap:?},\"border\":{border:?},\"screen\":{screen:?},\"outside\":{outside:?},\"faded\":{faded:?}}}"
        ))
    }
}
