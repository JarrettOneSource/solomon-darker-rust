//! Ordinary textured draws do not need the filter/shadow shader's instruction cost.
//! Apple Software Renderer retains that cost even when effect uniforms are disabled.

use glow::HasContext;

use super::{BlendMode, GraphicsContext, TextureColorMode, VERTEX_SHADER, link_program};

const FRAGMENT_SHADER: &str = r"#version 300 es
precision mediump float;
in highp vec2 v_uv;
in vec4 v_color;
uniform sampler2D u_texture;
uniform highp vec4 u_texture_bounds;
uniform float u_premultiply_rgb;
uniform float u_texture_premultiplied;
out vec4 o_color;
void main() {
    highp vec2 uv = u_texture_bounds.z >= u_texture_bounds.x
        ? clamp(v_uv, u_texture_bounds.xy, u_texture_bounds.zw) : v_uv;
    vec4 texel = texture(u_texture, uv);
    if (u_texture_premultiplied > 0.5 && texel.a > 0.0) {
        texel.rgb /= texel.a;
    }
    float alpha = texel.a * v_color.a;
    vec3 colour = texel.rgb * v_color.rgb;
    o_color = vec4(colour * mix(1.0, alpha, u_premultiply_rgb), alpha);
}
";

pub(super) struct SpriteProgram {
    gl: GraphicsContext,
    program: glow::Program,
    logical_size: glow::UniformLocation,
    pixel_center_offset: glow::UniformLocation,
    premultiply_rgb: glow::UniformLocation,
    texture_premultiplied: glow::UniformLocation,
    texture_bounds: glow::UniformLocation,
}

impl SpriteProgram {
    pub(super) fn new(gl: &GraphicsContext) -> Result<Self, String> {
        // SAFETY: Renderer creates and retains this program in its current context.
        unsafe {
            let program = link_program(gl, VERTEX_SHADER, FRAGMENT_SHADER)?;
            let uniform = |name| {
                gl.get_uniform_location(program, name)
                    .ok_or_else(|| format!("sprite shader uniform {name} is missing"))
            };
            let result = (|| {
                let texture = uniform("u_texture")?;
                let result = Self {
                    gl: gl.clone(),
                    program,
                    logical_size: uniform("u_logical_size")?,
                    pixel_center_offset: uniform("u_pixel_center_offset")?,
                    premultiply_rgb: uniform("u_premultiply_rgb")?,
                    texture_premultiplied: uniform("u_texture_premultiplied")?,
                    texture_bounds: uniform("u_texture_bounds")?,
                };
                gl.use_program(Some(program));
                gl.uniform_1_i32(Some(&texture), 0);
                Ok(result)
            })();
            if result.is_err() {
                gl.delete_program(program);
            }
            result
        }
    }

    pub(super) fn bind(
        &self,
        logical: [f32; 2],
        offset: [f32; 2],
        blend: BlendMode,
        color_mode: TextureColorMode,
        bounds: Option<[f32; 4]>,
    ) {
        // SAFETY: the batcher owns this program in the same live current context.
        unsafe {
            self.gl.use_program(Some(self.program));
            self.gl.uniform_2_f32_slice(Some(&self.logical_size), &logical);
            self.gl.uniform_2_f32_slice(Some(&self.pixel_center_offset), &offset);
            self.gl.uniform_4_f32_slice(
                Some(&self.texture_bounds),
                &bounds.unwrap_or([1.0, 1.0, 0.0, 0.0]),
            );
            self.gl.uniform_1_f32(
                Some(&self.premultiply_rgb),
                if blend == BlendMode::Multiply { 0.0 } else { 1.0 },
            );
            self.gl.uniform_1_f32(
                Some(&self.texture_premultiplied),
                if color_mode == TextureColorMode::Premultiplied { 1.0 } else { 0.0 },
            );
        }
    }
}

impl Drop for SpriteProgram {
    fn drop(&mut self) {
        // SAFETY: the Renderer drops all programs before retiring its context.
        unsafe { self.gl.delete_program(self.program) }
    }
}
