//! Actual renderer regression for nested GPU captures and group alpha.
//! Run the bindgen web output with tools/browser-smoke/gpu-composition.cjs.

#[cfg(target_arch = "wasm32")]
mod browser {
    use solomon_graphics::{
        AlphaShadow, BlendMode, Renderer, ShadowOutput, Texture, TextureColorMode, TextureSampling,
    };
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::WebGl2RenderingContext as Gl;

    #[wasm_bindgen]
    pub fn composition_probe(gl: Gl) -> Result<String, JsValue> {
        probe(gl).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn sampling_probe(gl: Gl) -> Result<String, JsValue> {
        sampling(gl).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn spread_shadow_probe(gl: Gl) -> Result<String, JsValue> {
        spread_shadow(gl).map_err(|error| JsValue::from_str(&error))
    }

    fn spread_shadow(gl: Gl) -> Result<String, String> {
        let mut renderer = Renderer::new(gl.clone())?;
        let caster = renderer.upload_rgba(3, 1, &[255; 12])?;
        let target = renderer.create_render_target_size([5, 3])?;
        let nested = renderer.create_render_target_size([1, 1])?;
        renderer.begin_frame(5, 4, [5.0, 4.0]);
        let mut nested_ink = Err("nested capture did not run".to_owned());
        renderer.render_ui_group(&target, [5.0, 3.0], |renderer| {
            renderer.draw_image_shadow(
                AlphaShadow {
                    bounds: [0.0, 0.0, 1.0, 1.0],
                    offset: [0.0, 0.0],
                    sigma: [0.0, 0.0],
                    color: [0.0, 0.0, 1.0, 1.0],
                    output: ShadowOutput::Shadow,
                },
                |renderer| {
                    quad(
                        renderer,
                        &caster,
                        [0.0, 0.0, 3.0, 1.0],
                        [1.0, 1.0, 3.0, 1.0],
                        TextureColorMode::Texture,
                        1.0,
                    )
                },
            );
            renderer.draw_erased(|renderer| {
                renderer.render_ui_group(&nested, [1.0, 1.0], |renderer| {
                    renderer.draw_solid([0.0, 0.0, 1.0, 1.0], [0.0, 1.0, 0.0, 1.0]);
                    renderer.flush();
                    nested_ink = pixel(&gl, 0, 0);
                });
                renderer.draw_solid([2.0, 1.0, 1.0, 1.0], [1.0; 4]);
                renderer.draw_solid([3.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.5]);
            });
            renderer.draw_solid([0.0, 0.0, 1.0, 1.0], [1.0, 0.0, 0.0, 1.0]);
        });
        let mut capture = Err("spread capture did not run".to_owned());
        renderer.render_onto(&target, [5.0, 3.0], |_| {
            capture = (|| {
                Ok((pixel(&gl, 1, 1)?, pixel(&gl, 2, 1)?, pixel(&gl, 3, 1)?, pixel(&gl, 0, 2)?))
            })();
        });
        let (exterior, interior, antialias, restored) = capture?;
        let nested_ink = nested_ink?;
        renderer.draw_solid([0.0, 0.0, 5.0, 4.0], [0.0, 1.0, 0.0, 1.0]);
        quad(
            &mut renderer,
            &target.texture,
            [0.0, 3.0, 5.0, -3.0],
            [0.0, 0.0, 5.0, 3.0],
            TextureColorMode::Premultiplied,
            0.5,
        );
        renderer.end_frame();
        let through_hole = pixel(&gl, 2, 2)?;
        renderer.destroy_texture(&caster);
        renderer.destroy_render_target(&nested);
        renderer.destroy_render_target(&target);
        if gl.get_error() != Gl::NO_ERROR {
            return Err("spread shadow proof produced a WebGL error".to_owned());
        }
        Ok(format!(
            "{{\"exterior\":{exterior:?},\"interior\":{interior:?},\"antialias\":{antialias:?},\"restored\":{restored:?},\"nestedInk\":{nested_ink:?},\"throughHole\":{through_hole:?}}}"
        ))
    }

    fn sampling(gl: Gl) -> Result<String, String> {
        let mut renderer = Renderer::new(gl.clone())?;
        let texture = renderer.upload_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255])?;
        renderer.begin_frame(5, 4, [5.0, 4.0]);
        let sample = |renderer: &mut Renderer| -> Result<_, String> {
            quad(
                renderer,
                &texture,
                [0.0, 0.0, 2.0, 1.0],
                [0.0, 0.0, 5.0, 1.0],
                TextureColorMode::Texture,
                1.0,
            );
            renderer.flush();
            pixel(&gl, 1, 3)
        };
        let linear = sample(&mut renderer)?;
        let (nearest, nested, nested_restored, sampler) =
            renderer.draw_sampled(TextureSampling::Nearest, |renderer| -> Result<_, String> {
                let nearest = sample(renderer)?;
                let sampler = gl
                    .get_parameter(Gl::SAMPLER_BINDING)
                    .map_err(|error| format!("nearest sampler: {error:?}"))?;
                let nested = renderer.draw_sampled(TextureSampling::Texture, sample)?;
                let nested_restored = gl
                    .get_parameter(Gl::SAMPLER_BINDING)
                    .map_err(|error| format!("restored nearest sampler: {error:?}"))?
                    == sampler;
                Ok((nearest, nested, nested_restored, sampler))
            })?;
        let restored = sample(&mut renderer)?;
        let unbound = gl
            .get_parameter(Gl::SAMPLER_BINDING)
            .map_err(|error| format!("restored texture sampler: {error:?}"))?
            .is_null();
        let mut parameters = Vec::new();
        for parameter in
            [Gl::TEXTURE_MIN_FILTER, Gl::TEXTURE_MAG_FILTER, Gl::TEXTURE_WRAP_S, Gl::TEXTURE_WRAP_T]
        {
            let value = gl
                .get_tex_parameter(Gl::TEXTURE_2D, parameter)
                .as_f64()
                .ok_or("borrowed texture parameter is not numeric")?;
            parameters.push(value as u32);
        }
        renderer.destroy_texture(&texture);
        drop(renderer);
        let deleted = !gl.is_sampler(Some(sampler.unchecked_ref()));
        if gl.get_error() != Gl::NO_ERROR {
            return Err("sampler proof produced a WebGL error".to_owned());
        }
        Ok(format!(
            "{{\"linear\":{linear:?},\"nearest\":{nearest:?},\"nested\":{nested:?},\"restored\":{restored:?},\"nestedRestored\":{nested_restored},\"unbound\":{unbound},\"parameters\":{parameters:?},\"deleted\":{deleted}}}"
        ))
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
        let caster = renderer.upload_rgba(2, 1, &[255, 0, 0, 255, 255, 0, 0, 255])?;
        renderer.begin_frame(5, 4, [5.0, 4.0]);
        renderer.draw_image_shadow(
            AlphaShadow {
                bounds: [0.0, 0.0, 1.0, 1.0],
                offset: [0.5, 0.0],
                sigma: [0.0, 0.0],
                color: [0.0, 0.0, 1.0, 1.0],
                output: ShadowOutput::Outer,
            },
            |renderer| {
                quad(
                    renderer,
                    &caster,
                    [0.0, 0.0, 3.0, 1.0],
                    [1.0, 1.0, 3.0, 1.0],
                    TextureColorMode::Texture,
                    1.0,
                );
            },
        );
        renderer.end_frame();
        let outer_interior = pixel(&gl, 2, 2)?;
        let outer_exterior = pixel(&gl, 3, 2)?;
        renderer.begin_frame(5, 4, [5.0, 4.0]);
        renderer.draw_image_shadow(
            AlphaShadow {
                bounds: [0.0, 0.0, 1.0, 1.0],
                offset: [0.5, 0.0],
                sigma: [0.0, 0.0],
                color: [0.0, 0.0, 1.0, 1.0],
                output: ShadowOutput::Inner,
            },
            |renderer| {
                quad(
                    renderer,
                    &caster,
                    [0.0, 0.0, 3.0, 1.0],
                    [1.0, 1.0, 3.0, 1.0],
                    TextureColorMode::Texture,
                    1.0,
                );
            },
        );
        quad(
            &mut renderer,
            &red,
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            TextureColorMode::Texture,
            1.0,
        );
        renderer.end_frame();
        let inner_left = pixel(&gl, 1, 2)?;
        let inner_clear = pixel(&gl, 2, 2)?;
        let inner_exterior = pixel(&gl, 3, 2)?;
        let restored = pixel(&gl, 0, 3)?;
        renderer.destroy_texture(&caster);
        renderer.destroy_render_target(&outer);
        renderer.destroy_texture(&red);
        renderer.destroy_texture(&green);
        let error = gl.get_error();
        if error != Gl::NO_ERROR {
            return Err(format!("GPU composition error {error}"));
        }
        Ok(format!(
            "{{\"allocationRestored\":{allocation_restored},\"nestedRestored\":{nested_restored},\"screenRestored\":{screen_restored},\"overlap\":{overlap:?},\"border\":{border:?},\"screen\":{screen:?},\"outside\":{outside:?},\"faded\":{faded:?},\"outerInterior\":{outer_interior:?},\"outerExterior\":{outer_exterior:?},\"innerLeft\":{inner_left:?},\"innerClear\":{inner_clear:?},\"innerExterior\":{inner_exterior:?},\"restored\":{restored:?}}}"
        ))
    }
}
