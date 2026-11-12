//! Allocation must preserve both queued draws and the renderer's cached texture binding.

use glow::HasContext;
use solomon_graphics::assets::Image;
use solomon_graphics::{
    BlendMode, GraphicsContext, Quad, RenderTarget, Renderer, Texture, TextureSampling,
};

#[cfg(not(target_arch = "wasm32"))]
pub const BLUE_IMAGE: &[u8] =
    include_bytes!("../../../../tools/browser-smoke/fixtures/allocation-blue.png");

enum Allocation {
    Texture(Texture),
    Target(RenderTarget),
}

impl Allocation {
    fn destroy(self, renderer: &Renderer) {
        match self {
            Self::Texture(texture) => renderer.destroy_texture(&texture),
            Self::Target(target) => renderer.destroy_render_target(&target),
        }
    }
}

#[derive(PartialEq)]
struct Bindings {
    texture: Option<glow::Texture>,
    framebuffer: Option<glow::Framebuffer>,
    sampler: Option<glow::Sampler>,
    viewport: [i32; 4],
}

fn bindings(gl: &GraphicsContext) -> Bindings {
    // SAFETY: the probe borrows its renderer's live current graphics context.
    unsafe {
        let mut viewport = [0; 4];
        gl.get_parameter_i32_slice(glow::VIEWPORT, &mut viewport);
        Bindings {
            texture: gl.get_parameter_texture(glow::TEXTURE_BINDING_2D),
            framebuffer: gl.get_parameter_framebuffer(glow::FRAMEBUFFER_BINDING),
            sampler: gl.get_parameter_sampler(glow::SAMPLER_BINDING),
            viewport,
        }
    }
}

fn draw_source(renderer: &mut Renderer, source: &Texture, x: f32) {
    renderer.draw(
        source,
        Quad {
            source: [0.0, 0.0, 1.0, 1.0],
            destination: [x, 0.0, 1.0, 1.0],
            tint: [1.0; 4],
            blend: BlendMode::Normal,
        },
    );
}

pub fn probe(
    renderer: &mut Renderer,
    image: &Image,
    incomplete_image: Option<&Image>,
) -> Result<String, String> {
    renderer.flush();
    let source = renderer.upload_rgba(1, 1, &[255, 0, 0, 255])?;
    let output = renderer.create_render_target_size([3, 1])?;
    let gl = renderer.graphics_context();
    let oversized =
        renderer.maximum_texture_size()?.checked_add(1).ok_or("Texture limit overflow")?;
    let oversized_pixels = [0, 0, 255, 255].repeat(oversized as usize);
    let mut kinds =
        vec!["rgba", "image", "repeat", "target", "target-size", "rgba-error", "target-error"];
    if incomplete_image.is_some() {
        kinds.push("image-error");
    }
    let mut rows = Vec::new();
    let result = renderer.draw_sampled(TextureSampling::Nearest, |renderer| {
        for kind in kinds {
            for preflushed in [false, true] {
                renderer.render_to_color(&output, [3.0, 1.0], [0.0, 0.0, 0.0, 1.0], |renderer| {
                        draw_source(renderer, &source, 0.0);
                        if preflushed {
                            renderer.flush();
                        }
                        let before = bindings(&gl);
                        let allocation = match kind {
                            "rgba" => renderer.upload_rgba(1, 1, &[0, 0, 255, 255]).map(Allocation::Texture),
                            "image" => renderer.upload(image).map(Allocation::Texture),
                            "repeat" => renderer.upload_with_repeat(image, true).map(Allocation::Texture),
                            "target" => renderer.create_render_target(1).map(Allocation::Target),
                            "target-size" => renderer.create_render_target_size([2, 1]).map(Allocation::Target),
                            "rgba-error" => renderer.upload_rgba(oversized, 1, &oversized_pixels).map(Allocation::Texture),
                            "target-error" => renderer.create_render_target_size([oversized, 1]).map(Allocation::Target),
                            "image-error" => renderer.upload(incomplete_image.expect("declared incomplete image case")).map(Allocation::Texture),
                            _ => unreachable!(),
                        };
                        let rejected = allocation.is_err();
                        let after = bindings(&gl);
                        // The source ID is unchanged, so the renderer may reuse its cached binding.
                        draw_source(renderer, &source, 1.0);
                        renderer.draw_solid([2.0, 0.0, 1.0, 1.0], [0.0, 1.0, 0.0, 1.0]);
                        renderer.flush();
                        let mut pixels = [0; 12];
                        let mut errors = Vec::new();
                        // SAFETY: the current target is 3x1 and pixels owns all 12 output bytes.
                        unsafe {
                            gl.read_pixels(0, 0, 3, 1, glow::RGBA, glow::UNSIGNED_BYTE,
                                glow::PixelPackData::Slice(Some(&mut pixels)));
                            loop {
                                let error = gl.get_error();
                                if error == glow::NO_ERROR { break; }
                                errors.push(error);
                            }
                        }
                        if let Ok(allocation) = allocation { allocation.destroy(renderer); }
                        rows.push(format!("{{\"kind\":\"{kind}\",\"preflushed\":{preflushed},\"rejected\":{rejected},\"textureRestored\":{},\"framebufferRestored\":{},\"samplerRestored\":{},\"viewportRestored\":{},\"pixels\":{pixels:?},\"errors\":{errors:?}}}",
                            before.texture == after.texture, before.framebuffer == after.framebuffer,
                            before.sampler == after.sampler, before.viewport == after.viewport));
                });
            }
        }
        format!("[{}]", rows.join(","))
    });
    renderer.destroy_render_target(&output);
    renderer.destroy_texture(&source);
    Ok(result)
}

#[cfg(target_arch = "wasm32")]
pub fn unbound_probe(
    gl: web_sys::WebGl2RenderingContext,
    image: &Image,
    incomplete_image: &Image,
) -> Result<String, String> {
    let mut renderer = Renderer::new(gl.clone())?;
    let oversized =
        renderer.maximum_texture_size()?.checked_add(1).ok_or("Texture limit overflow")?;
    let oversized_pixels = [0, 0, 255, 255].repeat(oversized as usize);
    let mut rows = Vec::new();
    for kind in [
        "rgba",
        "image",
        "repeat",
        "target",
        "target-size",
        "rgba-error",
        "target-error",
        "image-error",
    ] {
        let retired = renderer.upload_rgba(1, 1, &[255, 0, 0, 255])?;
        renderer.begin_frame(3, 1, [3.0, 1.0]);
        draw_source(&mut renderer, &retired, 0.0);
        renderer.flush();
        renderer.destroy_texture(&retired);
        // Use WebGL directly: glow 0.18's resource lookup panics on null bindings.
        let parameter =
            |name| gl.get_parameter(name).map_err(|error| format!("WebGL parameter: {error:?}"));
        let texture = parameter(glow::TEXTURE_BINDING_2D)?;
        let framebuffer = parameter(glow::FRAMEBUFFER_BINDING)?;
        let sampler = parameter(glow::SAMPLER_BINDING)?;
        let allocation = match kind {
            "rgba" => renderer.upload_rgba(1, 1, &[0, 0, 255, 255]).map(Allocation::Texture),
            "image" => renderer.upload(image).map(Allocation::Texture),
            "repeat" => renderer.upload_with_repeat(image, true).map(Allocation::Texture),
            "target" => renderer.create_render_target(1).map(Allocation::Target),
            "target-size" => renderer.create_render_target_size([2, 1]).map(Allocation::Target),
            "rgba-error" => {
                renderer.upload_rgba(oversized, 1, &oversized_pixels).map(Allocation::Texture)
            }
            "target-error" => {
                renderer.create_render_target_size([oversized, 1]).map(Allocation::Target)
            }
            "image-error" => renderer.upload(incomplete_image).map(Allocation::Texture),
            _ => unreachable!(),
        };
        let rejected = allocation.is_err();
        let texture_restored = parameter(glow::TEXTURE_BINDING_2D)? == texture;
        let framebuffer_restored = parameter(glow::FRAMEBUFFER_BINDING)? == framebuffer;
        let sampler_restored = parameter(glow::SAMPLER_BINDING)? == sampler;
        let mut errors = Vec::new();
        loop {
            let error = gl.get_error();
            if error == glow::NO_ERROR {
                break;
            }
            errors.push(error);
        }
        if let Ok(allocation) = allocation {
            allocation.destroy(&renderer);
        }
        rows.push(format!("{{\"kind\":\"{kind}\",\"unboundBefore\":{},\"textureRestored\":{texture_restored},\"framebufferRestored\":{framebuffer_restored},\"samplerRestored\":{sampler_restored},\"rejected\":{rejected},\"errors\":{errors:?}}}", texture.is_null()));
    }
    Ok(format!("[{}]", rows.join(",")))
}
