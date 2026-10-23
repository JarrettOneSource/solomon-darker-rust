//! A small WebGL2 batcher: textured triangles with vertex tints, drawn in submission
//! order. Normal/add emit premultiplied RGB; multiply retains straight RGB, with
//! the Website's native per-channel blend equations. Destinations are
//! logical pixels of the 1600×900 view, which `begin_frame` letterboxes into the canvas.

pub mod announcements;
pub mod assets;

use solomon_sim::float;
use web_sys::{
    HtmlImageElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlFramebuffer, WebGlProgram,
    WebGlShader, WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

const VERTEX_SHADER: &str = r"#version 300 es
precision highp float;
in vec2 a_position;
in vec2 a_uv;
in vec4 a_color;
uniform vec2 u_logical_size;
uniform vec2 u_pixel_center_offset;
out vec2 v_uv;
out vec4 v_color;
void main() {
    vec2 position = a_position + u_pixel_center_offset;
    vec2 clip = vec2(
        position.x / u_logical_size.x * 2.0 - 1.0,
        1.0 - position.y / u_logical_size.y * 2.0);
    gl_Position = vec4(clip, 0.0, 1.0);
    v_uv = a_uv;
    v_color = a_color;
}
";

const FRAGMENT_SHADER: &str = r"#version 300 es
precision mediump float;
in vec2 v_uv;
in vec4 v_color;
uniform sampler2D u_texture;
uniform float u_saturation;
uniform float u_alpha_mask;
uniform float u_white_alpha_filter;
uniform float u_premultiply_rgb;
uniform vec2 u_color_filter;
uniform vec4 u_shadow_bounds;
uniform vec4 u_shadow_color;
uniform vec2 u_shadow_offset;
uniform vec2 u_shadow_sigma;
out vec4 o_color;
vec4 cropped_texture(vec2 uv) {
    if (any(lessThan(uv, u_shadow_bounds.xy)) || any(greaterThanEqual(uv, u_shadow_bounds.zw))) {
        return vec4(0.0);
    }
    vec2 halfTexel = 0.5 / vec2(textureSize(u_texture, 0));
    return textureLod(u_texture, clamp(uv, u_shadow_bounds.xy + halfTexel, u_shadow_bounds.zw - halfTexel), 0.0);
}
float shadow_alpha(vec2 uv) {
    if (u_shadow_sigma.x == 0.0 || u_shadow_sigma.y == 0.0) {
        return cropped_texture(uv).a;
    }
    // Normalized Gaussian sampled at one-third-sigma intervals, truncated at 3 sigma.
    const float weights[10] = float[10](
        0.1331759960, 0.1259790945, 0.1066390012, 0.0807753247, 0.0547502888,
        0.0332077288, 0.0180234111, 0.0087534623, 0.0038042390, 0.0014794517);
    vec2 stepSize = u_shadow_sigma / 3.0;
    float alpha = 0.0;
    for (int y = -9; y <= 9; ++y) {
        for (int x = -9; x <= 9; ++x) {
            alpha += cropped_texture(uv + vec2(float(x), float(y)) * stepSize).a
                * weights[abs(x)] * weights[abs(y)];
        }
    }
    return alpha;
}
void main() {
    bool shadow = u_shadow_sigma.x >= 0.0;
    vec4 texel = shadow ? cropped_texture(v_uv) : texture(u_texture, v_uv);
    texel.rgb = mix(texel.rgb, vec3(1.0), u_alpha_mask);
    float alpha = texel.a * v_color.a;
    alpha = mix(alpha, alpha * alpha, u_white_alpha_filter);
    float textureGrey = (texel.r + texel.g + texel.b) / 3.0;
    float vertexGrey = (v_color.r + v_color.g + v_color.b) / 3.0;
    vec3 colour = mix(vec3(textureGrey * vertexGrey), texel.rgb * v_color.rgb, u_saturation);
    colour = mix(colour, vec3(1.0), u_white_alpha_filter);
    if (u_color_filter.x >= 0.0) {
        float grey = dot(colour, vec3(0.213, 0.715, 0.072));
        colour = clamp(mix(vec3(grey), colour, u_color_filter.y), 0.0, 1.0);
        colour = clamp(colour * u_color_filter.x, 0.0, 1.0);
    }
    if (shadow) {
        float shadowAlpha = shadow_alpha(v_uv - u_shadow_offset) * u_shadow_color.a;
        float composedAlpha = texel.a + shadowAlpha * (1.0 - texel.a);
        vec3 composedRGB = colour * texel.a + u_shadow_color.rgb * shadowAlpha * (1.0 - texel.a);
        vec3 straightRGB = composedAlpha > 0.0 ? composedRGB / composedAlpha : vec3(0.0);
        alpha = composedAlpha * v_color.a;
        o_color = vec4(straightRGB * mix(1.0, alpha, u_premultiply_rgb), alpha);
    } else {
        o_color = vec4(colour * mix(1.0, alpha, u_premultiply_rgb), alpha);
    }
}
";

const FLOATS_PER_VERTEX: usize = 8;
const VERTICES_PER_TRIANGLE: usize = 3;

#[derive(Clone)]
pub struct Texture {
    id: u32,
    handle: WebGlTexture,
    pub width: f32,
    pub height: f32,
}

/// An offscreen colour target. Its texture is vertically inverted relative to loaded PNGs.
pub struct RenderTarget {
    pub texture: Texture,
    framebuffer: WebGlFramebuffer,
}

/// How a quad combines with what is already drawn, matching Pixi's blend modes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    /// Light added onto what is below: Pixi's `add` for the seals and the fountain spray.
    Add,
}

impl BlendMode {
    /// The stock renderer's independent RGB and alpha factors.
    fn factors(self) -> (u32, u32, u32, u32) {
        match self {
            Self::Normal => {
                (Gl::ONE, Gl::ONE_MINUS_SRC_ALPHA, Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA)
            }
            Self::Multiply => (Gl::ZERO, Gl::SRC_COLOR, Gl::ZERO, Gl::SRC_ALPHA),
            Self::Add => (Gl::ONE, Gl::ONE, Gl::SRC_ALPHA, Gl::ONE),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextureColorMode {
    #[default]
    Texture,
    /// Native diffuse RGB with the sampled texture alpha (white-alpha sprite filters).
    AlphaMask,
    /// Storm's white ColorMatrixFilter: native capture squares texture/vertex
    /// alpha, then the filter replaces RGB with white and premultiplies again.
    WhiteAlphaFilter,
}

/// Source CSS `saturate(...) brightness(...)` in sRGB, with alpha preserved.
/// Filter Effects 1 requires non-premultiplied colour and clamps each operation:
/// https://drafts.csswg.org/filter-effects/#ShorthandEquivalents
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorFilter {
    pub brightness: f32,
    pub saturation: f32,
}

impl Default for ColorFilter {
    fn default() -> Self {
        Self { brightness: 1.0, saturation: 1.0 }
    }
}

impl ColorFilter {
    /// The same Source filter for solid/text colours that do not sample an image.
    pub fn rgb(self, rgb: [f32; 3]) -> [f32; 3] {
        let grey = rgb[0] * 0.213 + rgb[1] * 0.715 + rgb[2] * 0.072;
        rgb.map(|channel| {
            let saturated =
                (grey * (1.0 - self.saturation) + channel * self.saturation).clamp(0.0, 1.0);
            (saturated * self.brightness).clamp(0.0, 1.0)
        })
    }
}

/// Crop-bounded SourceAlpha shadow. Coordinates are normalized source texture UVs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlphaShadow {
    pub bounds: [f32; 4],
    pub offset: [f32; 2],
    pub sigma: [f32; 2],
    pub color: [f32; 4],
}

/// Where a quad's texels come from and where they land.
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    /// Atlas x, y, width, height in texels.
    pub source: [f32; 4],
    /// Logical x, y, width, height.
    pub destination: [f32; 4],
    /// Multiply colour and alpha.
    pub tint: [f32; 4],
    pub blend: BlendMode,
}

/// One native mesh vertex. Texture coordinates remain in atlas texels until submission,
/// allowing the same primitive to draw sprites and repeating ground/road triangles.
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: [f32; 2],
    pub texel: [f32; 2],
    pub tint: [f32; 4],
}

pub struct Renderer {
    gl: Gl,
    u_logical_size: WebGlUniformLocation,
    u_pixel_center_offset: WebGlUniformLocation,
    u_saturation: WebGlUniformLocation,
    u_alpha_mask: WebGlUniformLocation,
    u_white_alpha_filter: WebGlUniformLocation,
    u_premultiply_rgb: WebGlUniformLocation,
    u_color_filter: WebGlUniformLocation,
    color_filter: Option<ColorFilter>,
    u_shadow_bounds: WebGlUniformLocation,
    u_shadow_color: WebGlUniformLocation,
    u_shadow_offset: WebGlUniformLocation,
    u_shadow_sigma: WebGlUniformLocation,
    shadow: Option<AlphaShadow>,
    texture_color_mode: TextureColorMode,
    forced_texture_color_mode: Option<TextureColorMode>,
    forced_opacity: Option<f32>,
    saturation: f32,
    opacity: f32,
    viewport: [i32; 4],
    logical_size: [f32; 2],
    native_world_pixel_centers: bool,
    vertices: Vec<f32>,
    bound_texture: Option<u32>,
    blend: BlendMode,
    next_texture_id: std::cell::Cell<u32>,
    white_texture: Texture,
    // Kept alive for the renderer's lifetime; the context holds them bound.
    _program: WebGlProgram,
    _vertex_array: WebGlVertexArrayObject,
    _vertex_buffer: WebGlBuffer,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.gl.delete_texture(Some(&self.white_texture.handle));
        self.gl.delete_buffer(Some(&self._vertex_buffer));
        self.gl.delete_vertex_array(Some(&self._vertex_array));
        self.gl.delete_program(Some(&self._program));
    }
}

impl Renderer {
    pub fn new(gl: Gl) -> Result<Self, String> {
        let program = link_program(&gl, VERTEX_SHADER, FRAGMENT_SHADER)?;
        gl.use_program(Some(&program));
        let vertex_array = gl.create_vertex_array().ok_or("cannot create a vertex array")?;
        gl.bind_vertex_array(Some(&vertex_array));
        let vertex_buffer = gl.create_buffer().ok_or("cannot create a vertex buffer")?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&vertex_buffer));
        let stride = (FLOATS_PER_VERTEX * 4) as i32;
        for (name, size, offset) in [("a_position", 2, 0), ("a_uv", 2, 8), ("a_color", 4, 16)] {
            let location = gl.get_attrib_location(&program, name);
            if location < 0 {
                return Err(format!("shader attribute {name} is missing"));
            }
            gl.enable_vertex_attrib_array(location as u32);
            gl.vertex_attrib_pointer_with_i32(
                location as u32,
                size,
                Gl::FLOAT,
                false,
                stride,
                offset,
            );
        }
        let u_logical_size = gl
            .get_uniform_location(&program, "u_logical_size")
            .ok_or("u_logical_size is missing")?;
        let u_pixel_center_offset = gl
            .get_uniform_location(&program, "u_pixel_center_offset")
            .ok_or("u_pixel_center_offset is missing")?;
        let u_texture =
            gl.get_uniform_location(&program, "u_texture").ok_or("u_texture is missing")?;
        let u_saturation =
            gl.get_uniform_location(&program, "u_saturation").ok_or("u_saturation is missing")?;
        let u_alpha_mask =
            gl.get_uniform_location(&program, "u_alpha_mask").ok_or("u_alpha_mask is missing")?;
        let u_white_alpha_filter = gl
            .get_uniform_location(&program, "u_white_alpha_filter")
            .ok_or("u_white_alpha_filter is missing")?;
        let u_premultiply_rgb = gl
            .get_uniform_location(&program, "u_premultiply_rgb")
            .ok_or("u_premultiply_rgb is missing")?;
        let u_color_filter = gl
            .get_uniform_location(&program, "u_color_filter")
            .ok_or("u_color_filter is missing")?;
        let u_shadow_bounds = gl
            .get_uniform_location(&program, "u_shadow_bounds")
            .ok_or("u_shadow_bounds is missing")?;
        let u_shadow_color = gl
            .get_uniform_location(&program, "u_shadow_color")
            .ok_or("u_shadow_color is missing")?;
        let u_shadow_offset = gl
            .get_uniform_location(&program, "u_shadow_offset")
            .ok_or("u_shadow_offset is missing")?;
        let u_shadow_sigma = gl
            .get_uniform_location(&program, "u_shadow_sigma")
            .ok_or("u_shadow_sigma is missing")?;
        gl.uniform1i(Some(&u_texture), 0);
        gl.uniform2f(Some(&u_pixel_center_offset), 0.0, 0.0);
        gl.uniform1f(Some(&u_saturation), 1.0);
        gl.uniform1f(Some(&u_alpha_mask), 0.0);
        gl.uniform1f(Some(&u_white_alpha_filter), 0.0);
        gl.uniform1f(Some(&u_premultiply_rgb), 1.0);
        gl.uniform2f(Some(&u_color_filter), -1.0, 1.0);
        gl.uniform2f(Some(&u_shadow_sigma), -1.0, -1.0);
        gl.active_texture(Gl::TEXTURE0);
        gl.enable(Gl::BLEND);
        let (source, destination, source_alpha, destination_alpha) = BlendMode::Normal.factors();
        gl.blend_func_separate(source, destination, source_alpha, destination_alpha);
        gl.pixel_storei(Gl::UNPACK_COLORSPACE_CONVERSION_WEBGL, Gl::NONE as i32);
        gl.pixel_storei(Gl::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0);
        gl.pixel_storei(Gl::UNPACK_FLIP_Y_WEBGL, 0);
        let white = gl.create_texture().ok_or("cannot create the solid-colour texture")?;
        gl.bind_texture(Gl::TEXTURE_2D, Some(&white));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            Gl::TEXTURE_2D,
            0,
            Gl::RGBA as i32,
            1,
            1,
            0,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            Some(&[255, 255, 255, 255]),
        )
        .map_err(|error| format!("solid-colour texture upload: {error:?}"))?;
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::NEAREST as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MAG_FILTER, Gl::NEAREST as i32);
        Ok(Self {
            gl,
            u_logical_size,
            u_pixel_center_offset,
            u_saturation,
            u_alpha_mask,
            u_white_alpha_filter,
            u_premultiply_rgb,
            u_color_filter,
            color_filter: None,
            u_shadow_bounds,
            u_shadow_color,
            u_shadow_offset,
            u_shadow_sigma,
            shadow: None,
            texture_color_mode: TextureColorMode::Texture,
            forced_texture_color_mode: None,
            forced_opacity: None,
            saturation: 1.0,
            opacity: 1.0,
            viewport: [0; 4],
            logical_size: [1.0; 2],
            native_world_pixel_centers: false,
            vertices: Vec::new(),
            bound_texture: None,
            blend: BlendMode::Normal,
            next_texture_id: std::cell::Cell::new(1),
            white_texture: Texture { id: 0, handle: white, width: 1.0, height: 1.0 },
            _program: program,
            _vertex_array: vertex_array,
            _vertex_buffer: vertex_buffer,
        })
    }

    /// Uploads a decoded image as a linearly filtered, edge-clamped texture.
    pub fn upload(&self, image: &HtmlImageElement) -> Result<Texture, String> {
        self.upload_with_repeat(image, false)
    }

    /// Native ground and road meshes intentionally address beyond one tile's texel bounds.
    pub fn upload_with_repeat(
        &self,
        image: &HtmlImageElement,
        repeat: bool,
    ) -> Result<Texture, String> {
        let wrap = if repeat { Gl::REPEAT } else { Gl::CLAMP_TO_EDGE };
        let handle = self.gl.create_texture().ok_or("cannot create a texture")?;
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(&handle));
        for (parameter, value) in [
            (Gl::TEXTURE_MIN_FILTER, Gl::LINEAR),
            (Gl::TEXTURE_MAG_FILTER, Gl::LINEAR),
            (Gl::TEXTURE_WRAP_S, wrap),
            (Gl::TEXTURE_WRAP_T, wrap),
        ] {
            self.gl.tex_parameteri(Gl::TEXTURE_2D, parameter, value as i32);
        }
        self.gl
            .tex_image_2d_with_u32_and_u32_and_html_image_element(
                Gl::TEXTURE_2D,
                0,
                Gl::RGBA as i32,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                image,
            )
            .map_err(|error| format!("texture upload of {}: {error:?}", image.src()))?;
        let id = self.next_texture_id.get();
        self.next_texture_id.set(id + 1);
        Ok(Texture {
            id,
            handle,
            width: image.natural_width() as f32,
            height: image.natural_height() as f32,
        })
    }

    /// Upload a source-authored RGBA ramp without passing through a browser canvas.
    pub fn upload_rgba(&self, width: u32, height: u32, pixels: &[u8]) -> Result<Texture, String> {
        assert!(width > 0 && height > 0, "RGBA textures have a positive size");
        assert_eq!(pixels.len(), width as usize * height as usize * 4);
        let handle = self.gl.create_texture().ok_or("cannot create an RGBA texture")?;
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(&handle));
        for (parameter, value) in [
            (Gl::TEXTURE_MIN_FILTER, Gl::LINEAR),
            (Gl::TEXTURE_MAG_FILTER, Gl::LINEAR),
            (Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE),
            (Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE),
        ] {
            self.gl.tex_parameteri(Gl::TEXTURE_2D, parameter, value as i32);
        }
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                Gl::TEXTURE_2D,
                0,
                Gl::RGBA as i32,
                width as i32,
                height as i32,
                0,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                Some(pixels),
            )
            .map_err(|error| format!("RGBA texture upload: {error:?}"))?;
        let id = self.next_texture_id.get();
        self.next_texture_id.set(id + 1);
        Ok(Texture { id, handle, width: width as f32, height: height as f32 })
    }

    pub fn create_render_target(&self, side: u32) -> Result<RenderTarget, String> {
        let handle = self.gl.create_texture().ok_or("cannot create a render texture")?;
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(&handle));
        for (parameter, value) in [
            (Gl::TEXTURE_MIN_FILTER, Gl::LINEAR),
            (Gl::TEXTURE_MAG_FILTER, Gl::LINEAR),
            (Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE),
            (Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE),
        ] {
            self.gl.tex_parameteri(Gl::TEXTURE_2D, parameter, value as i32);
        }
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                Gl::TEXTURE_2D,
                0,
                Gl::RGBA as i32,
                side as i32,
                side as i32,
                0,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                None,
            )
            .map_err(|error| format!("render texture allocation: {error:?}"))?;
        let framebuffer = self.gl.create_framebuffer().ok_or("cannot create a framebuffer")?;
        self.gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(&framebuffer));
        self.gl.framebuffer_texture_2d(
            Gl::FRAMEBUFFER,
            Gl::COLOR_ATTACHMENT0,
            Gl::TEXTURE_2D,
            Some(&handle),
            0,
        );
        let complete =
            self.gl.check_framebuffer_status(Gl::FRAMEBUFFER) == Gl::FRAMEBUFFER_COMPLETE;
        self.gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        if !complete {
            self.gl.delete_framebuffer(Some(&framebuffer));
            self.gl.delete_texture(Some(&handle));
            return Err("the light framebuffer is incomplete".into());
        }
        let id = self.next_texture_id.get();
        self.next_texture_id.set(id + 1);
        Ok(RenderTarget {
            texture: Texture { id, handle, width: side as f32, height: side as f32 },
            framebuffer,
        })
    }

    /// Draw one offscreen pass and restore the current canvas projection before returning.
    pub fn render_to(
        &mut self,
        target: &RenderTarget,
        logical: [f32; 2],
        draw: impl FnOnce(&mut Self),
    ) {
        self.render_to_color(target, logical, [0.0, 0.0, 0.0, 1.0], draw);
    }

    /// Transparent targets retain the native per-channel blend equations. Storm
    /// and Leviathan compose into these before entering the world painter queue.
    pub fn render_to_color(
        &mut self,
        target: &RenderTarget,
        logical: [f32; 2],
        clear: [f32; 4],
        draw: impl FnOnce(&mut Self),
    ) {
        self.render_target(target, logical, Some(clear), draw);
    }

    /// Harden multiplies its ice image onto the retained diffuse capture.
    pub fn render_onto(
        &mut self,
        target: &RenderTarget,
        logical: [f32; 2],
        draw: impl FnOnce(&mut Self),
    ) {
        self.render_target(target, logical, None, draw);
    }

    fn render_target(
        &mut self,
        target: &RenderTarget,
        logical: [f32; 2],
        clear: Option<[f32; 4]>,
        draw: impl FnOnce(&mut Self),
    ) {
        self.flush();
        self.gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(&target.framebuffer));
        self.gl.viewport(0, 0, target.texture.width as i32, target.texture.height as i32);
        self.bind_projection(logical, [target.texture.width, target.texture.height]);
        if let Some(clear) = clear {
            self.gl.clear_color(clear[0], clear[1], clear[2], clear[3]);
            self.gl.clear(Gl::COLOR_BUFFER_BIT);
        }
        self.bound_texture = None;
        draw(self);
        self.flush();
        self.gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        let [x, y, width, height] = self.viewport;
        self.gl.viewport(x, y, width, height);
        self.bind_projection(self.logical_size, [width as f32, height as f32]);
        self.bound_texture = None;
    }

    /// `NativePlayerDiffuseCapture`: preserve the articulated pose's diffuse
    /// colours and sampled alpha while suppressing every source texture's RGB.
    pub fn render_diffuse_to(
        &mut self,
        target: &RenderTarget,
        logical: [f32; 2],
        draw: impl FnOnce(&mut Self),
    ) {
        let previous = self.forced_texture_color_mode;
        self.forced_texture_color_mode = Some(TextureColorMode::AlphaMask);
        self.render_to_color(target, logical, [1.0, 1.0, 1.0, 0.0], draw);
        self.forced_texture_color_mode = previous;
    }

    /// Native Puppet hit sprites retain diffuse red and apply their parent alpha.
    pub fn draw_diffuse_with_opacity(&mut self, opacity: f32, draw: impl FnOnce(&mut Self)) {
        self.flush();
        let previous_mode = self.forced_texture_color_mode;
        let previous_opacity = self.forced_opacity;
        self.forced_texture_color_mode = Some(TextureColorMode::AlphaMask);
        self.forced_opacity = Some(opacity);
        draw(self);
        self.flush();
        self.forced_texture_color_mode = previous_mode;
        self.forced_opacity = previous_opacity;
    }

    /// The Arena shader desaturates the texture and vertex colours separately.
    pub fn set_saturation(&mut self, saturation: f32) {
        if saturation != self.saturation {
            self.flush();
            self.gl.uniform1f(Some(&self.u_saturation), saturation);
            self.saturation = saturation;
        }
    }

    /// Applies stock UI colour filters before the existing blend premultiplication.
    pub fn draw_color_filtered(&mut self, filter: ColorFilter, draw: impl FnOnce(&mut Self)) {
        assert!(filter.brightness.is_finite() && filter.brightness >= 0.0);
        assert!(filter.saturation.is_finite() && filter.saturation >= 0.0);
        let previous = self.color_filter;
        self.set_color_filter(Some(filter));
        draw(self);
        self.set_color_filter(previous);
    }

    fn set_color_filter(&mut self, filter: Option<ColorFilter>) {
        if filter != self.color_filter {
            self.flush();
            let values =
                filter.map_or([-1.0, 1.0], |filter| [filter.brightness, filter.saturation]);
            self.gl.uniform2f(Some(&self.u_color_filter), values[0], values[1]);
            self.color_filter = filter;
        }
    }

    /// Combines the filtered source with its blurred alpha before group opacity.
    pub fn draw_image_shadow(&mut self, shadow: AlphaShadow, draw: impl FnOnce(&mut Self)) {
        let previous = self.shadow;
        self.set_shadow(Some(shadow));
        draw(self);
        self.set_shadow(previous);
    }

    fn set_shadow(&mut self, shadow: Option<AlphaShadow>) {
        if shadow != self.shadow {
            self.flush();
            if let Some(shadow) = shadow {
                self.gl.uniform4fv_with_f32_array(Some(&self.u_shadow_bounds), &shadow.bounds);
                self.gl.uniform4fv_with_f32_array(Some(&self.u_shadow_color), &shadow.color);
                self.gl.uniform2fv_with_f32_array(Some(&self.u_shadow_offset), &shadow.offset);
                self.gl.uniform2fv_with_f32_array(Some(&self.u_shadow_sigma), &shadow.sigma);
            } else {
                self.gl.uniform2f(Some(&self.u_shadow_sigma), -1.0, -1.0);
            }
            self.shadow = shadow;
        }
    }

    /// A panel supplies its actual physical-buffer clip and fade. Flush before changing
    /// the native scissor so queued sprites cannot escape a scrolling Inventory viewport.
    pub fn draw_clipped(&mut self, clip: [i32; 4], opacity: f32, draw: impl FnOnce(&mut Self)) {
        self.flush();
        self.gl.enable(Gl::SCISSOR_TEST);
        self.gl.scissor(clip[0], clip[1], clip[2], clip[3]);
        self.opacity = opacity;
        draw(self);
        self.flush();
        self.opacity = 1.0;
        self.gl.disable(Gl::SCISSOR_TEST);
    }

    pub fn destroy_texture(&self, texture: &Texture) {
        self.gl.delete_texture(Some(&texture.handle));
    }

    pub fn graphics_context(&self) -> Gl {
        self.gl.clone()
    }

    /// Applies an egui glyph-atlas patch without replacing its live texture.
    pub fn update_rgba(
        &mut self,
        texture: &Texture,
        origin: [usize; 2],
        size: [usize; 2],
        pixels: &[u8],
    ) -> Result<(), String> {
        self.flush();
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(&texture.handle));
        self.bound_texture = Some(texture.id);
        self.gl
            .tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_opt_u8_array(
                Gl::TEXTURE_2D,
                0,
                origin[0] as i32,
                origin[1] as i32,
                size[0] as i32,
                size[1] as i32,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                Some(pixels),
            )
            .map_err(|error| format!("GPU UI texture patch: {error:?}"))
    }

    /// Draws the Rust UI over the complete canvas, then restores world projection/state.
    pub fn draw_screen_space(
        &mut self,
        physical: [i32; 2],
        logical: [f32; 2],
        draw: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        self.flush();
        let viewport = self.viewport;
        let logical_size = self.logical_size;
        let native_centers = self.native_world_pixel_centers;
        let saturation = self.saturation;
        self.viewport = [0, 0, physical[0], physical[1]];
        self.logical_size = logical;
        self.native_world_pixel_centers = false;
        self.gl.viewport(0, 0, physical[0], physical[1]);
        self.bind_projection(logical, [physical[0] as f32, physical[1] as f32]);
        self.set_saturation(1.0);
        // Uploading a new atlas changes the GL binding outside the draw batch.
        self.bound_texture = None;
        let result = draw(self);
        self.flush();
        self.gl.disable(Gl::SCISSOR_TEST);
        self.viewport = viewport;
        self.logical_size = logical_size;
        self.native_world_pixel_centers = native_centers;
        self.gl.viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
        self.bind_projection(logical_size, [viewport[2] as f32, viewport[3] as f32]);
        self.set_saturation(saturation);
        result
    }

    pub fn set_texture_sampling(&mut self, texture: &Texture, nearest: bool) {
        self.flush();
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(&texture.handle));
        self.bound_texture = Some(texture.id);
        let filter = if nearest { Gl::NEAREST } else { Gl::LINEAR };
        for parameter in [Gl::TEXTURE_MIN_FILTER, Gl::TEXTURE_MAG_FILTER] {
            self.gl.tex_parameteri(Gl::TEXTURE_2D, parameter, filter as i32);
        }
    }

    pub fn delete_ui_texture(gl: &Gl, texture: &Texture) {
        gl.delete_texture(Some(&texture.handle));
    }

    /// Authored mod wearables use nearest sampling for every frame sharing their sheet.
    pub fn set_texture_nearest(&mut self, texture: &Texture) {
        self.flush();
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(&texture.handle));
        self.bound_texture = Some(texture.id);
        for parameter in [Gl::TEXTURE_MIN_FILTER, Gl::TEXTURE_MAG_FILTER] {
            self.gl.tex_parameteri(Gl::TEXTURE_2D, parameter, Gl::NEAREST as i32);
        }
    }

    pub fn destroy_render_target(&self, target: &RenderTarget) {
        self.gl.delete_framebuffer(Some(&target.framebuffer));
        self.destroy_texture(&target.texture);
    }

    /// Source `installNativePixelCenterProjection` changes subsequent projection
    /// binds, including offscreen targets. Enable only on its Hub/Boneyard scopes.
    pub fn set_native_world_pixel_centers(&mut self, enabled: bool) {
        self.native_world_pixel_centers = enabled;
    }

    fn bind_projection(&self, logical: [f32; 2], physical: [f32; 2]) {
        self.gl.uniform2f(Some(&self.u_logical_size), logical[0], logical[1]);
        // Source adds projection.a/d * (0.5 / targetResolution). Express that
        // same translation in this target's logical units before projection.
        let offset = if self.native_world_pixel_centers {
            [
                (0.5 * f64::from(logical[0]) / f64::from(physical[0])) as f32,
                (0.5 * f64::from(logical[1]) / f64::from(physical[1])) as f32,
            ]
        } else {
            [0.0; 2]
        };
        self.gl.uniform2f(Some(&self.u_pixel_center_offset), offset[0], offset[1]);
    }

    /// Clears the whole canvas and letterboxes the logical view into it.
    pub fn begin_frame(&mut self, canvas_width: i32, canvas_height: i32, logical: [f32; 2]) {
        let gl = &self.gl;
        gl.viewport(0, 0, canvas_width, canvas_height);
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT);
        let scale = (canvas_width as f32 / logical[0]).min(canvas_height as f32 / logical[1]);
        let width = (logical[0] * scale).round() as i32;
        let height = (logical[1] * scale).round() as i32;
        gl.viewport((canvas_width - width) / 2, (canvas_height - height) / 2, width, height);
        self.viewport = [(canvas_width - width) / 2, (canvas_height - height) / 2, width, height];
        self.logical_size = logical;
        self.bind_projection(logical, [width as f32, height as f32]);
        self.vertices.clear();
        self.bound_texture = None;
    }

    /// Active logical-to-device scale, including the letterboxed viewport.
    pub fn physical_pixel_scale(&self) -> [f32; 2] {
        [
            self.viewport[2] as f32 / self.logical_size[0],
            self.viewport[3] as f32 / self.logical_size[1],
        ]
    }

    pub fn draw(&mut self, texture: &Texture, quad: Quad) {
        let [x, y, width, height] = quad.destination;
        let corners = [[x, y], [x + width, y], [x + width, y + height], [x, y + height]];
        self.draw_quad(texture, quad.source, corners, [quad.tint; 4], quad.blend);
    }

    /// Draws the native four-vertex attachment/effect quad, with colors in corner order:
    /// top-left, top-right, bottom-right, bottom-left. Positions may form a rotated quad.
    pub fn draw_quad(
        &mut self,
        texture: &Texture,
        source: [f32; 4],
        positions: [[f32; 2]; 4],
        colors: [[f32; 4]; 4],
        blend: BlendMode,
    ) {
        self.draw_quad_with_color_mode(
            texture,
            source,
            positions,
            colors,
            blend,
            TextureColorMode::Texture,
        );
    }

    /// Uses the stock texture alpha with diffuse RGB for native puppet-hit masks.
    pub fn draw_quad_with_color_mode(
        &mut self,
        texture: &Texture,
        source: [f32; 4],
        positions: [[f32; 2]; 4],
        colors: [[f32; 4]; 4],
        blend: BlendMode,
        color_mode: TextureColorMode,
    ) {
        let [x, y, width, height] = source;
        let texels = [[x, y], [x + width, y], [x + width, y + height], [x, y + height]];
        let vertices: [Vertex; 4] = std::array::from_fn(|index| Vertex {
            position: positions[index],
            texel: texels[index],
            tint: colors[index],
        });
        let triangles =
            [[vertices[0], vertices[1], vertices[2]], [vertices[0], vertices[2], vertices[3]]];
        self.draw_triangles_with_color_mode(texture, &triangles, blend, color_mode);
    }

    /// The shared WebGL2 submission primitive. Three-vertex arrays preserve triangle shape
    /// at the call site; atlas/repeating coordinates and per-vertex colors pass through.
    pub fn draw_triangles(
        &mut self,
        texture: &Texture,
        triangles: &[[Vertex; 3]],
        blend: BlendMode,
    ) {
        self.draw_triangles_with_color_mode(texture, triangles, blend, TextureColorMode::Texture);
    }

    pub fn draw_triangles_with_color_mode(
        &mut self,
        texture: &Texture,
        triangles: &[[Vertex; 3]],
        blend: BlendMode,
        color_mode: TextureColorMode,
    ) {
        let color_mode = self.forced_texture_color_mode.unwrap_or(color_mode);
        if self.bound_texture != Some(texture.id) {
            self.flush();
            self.gl.bind_texture(Gl::TEXTURE_2D, Some(&texture.handle));
            self.bound_texture = Some(texture.id);
        }
        if self.blend != blend {
            self.flush();
            // `native-fixed-function-render-pipeline.ts::installNativeBlendModes`.
            // Multiply preserves straight source RGB; normal/add use premultiplied RGB.
            let (source, destination, source_alpha, destination_alpha) = blend.factors();
            self.gl.blend_func_separate(source, destination, source_alpha, destination_alpha);
            self.gl.uniform1f(
                Some(&self.u_premultiply_rgb),
                if blend == BlendMode::Multiply { 0.0 } else { 1.0 },
            );
            self.blend = blend;
        }
        if self.texture_color_mode != color_mode {
            self.flush();
            self.gl.uniform1f(
                Some(&self.u_alpha_mask),
                if color_mode == TextureColorMode::AlphaMask { 1.0 } else { 0.0 },
            );
            self.gl.uniform1f(
                Some(&self.u_white_alpha_filter),
                if color_mode == TextureColorMode::WhiteAlphaFilter { 1.0 } else { 0.0 },
            );
            self.texture_color_mode = color_mode;
        }
        self.vertices.reserve(triangles.len() * VERTICES_PER_TRIANGLE * FLOATS_PER_VERTEX);
        for triangle in triangles {
            for vertex in triangle {
                self.vertices.extend_from_slice(&vertex.position);
                self.vertices.extend_from_slice(&[
                    vertex.texel[0] / texture.width,
                    vertex.texel[1] / texture.height,
                ]);
                let mut tint = vertex.tint;
                if let Some(opacity) = self.forced_opacity {
                    tint[3] = (tint[3] * opacity * 255.0).clamp(0.0, 255.0).trunc() / 255.0;
                }
                tint[3] *= self.opacity;
                self.vertices.extend_from_slice(&tint);
            }
        }
    }

    /// Rotates a quad around its top-left, preserving the private rooms' flame anchors.
    pub fn draw_rotated(&mut self, texture: &Texture, quad: Quad, rotation: f64) {
        if rotation == 0.0 {
            self.draw(texture, quad);
            return;
        }
        let [x, y, width, height] = quad.destination;
        let mut positions = [[x, y], [x + width, y], [x + width, y + height], [x, y + height]];
        let (sin, cos) = float::sin_cos(rotation);
        for position in &mut positions {
            let local_x = f64::from(position[0] - x);
            let local_y = f64::from(position[1] - y);
            position[0] = x + (local_x * cos - local_y * sin) as f32;
            position[1] = y + (local_x * sin + local_y * cos) as f32;
        }
        self.draw_quad(texture, quad.source, positions, [quad.tint; 4], quad.blend);
    }

    /// Solid logical-pixel rectangle for room exit masks and transition covers.
    pub fn draw_solid(&mut self, destination: [f32; 4], tint: [f32; 4]) {
        let white = self.white_texture.clone();
        self.draw(
            &white,
            Quad { source: [0.0, 0.0, 1.0, 1.0], destination, tint, blend: BlendMode::Normal },
        );
    }

    pub fn draw_solid_triangles(&mut self, triangles: &[[Vertex; 3]], blend: BlendMode) {
        let white = self.white_texture.clone();
        self.draw_triangles(&white, triangles, blend);
    }

    pub fn flush(&mut self) {
        if self.vertices.is_empty() {
            return;
        }
        // SAFETY: the view borrows `self.vertices`, which is not touched until the upload returns.
        unsafe {
            let view = js_sys::Float32Array::view(&self.vertices);
            self.gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &view, Gl::DYNAMIC_DRAW);
        }
        self.gl.draw_arrays(Gl::TRIANGLES, 0, (self.vertices.len() / FLOATS_PER_VERTEX) as i32);
        self.vertices.clear();
    }

    pub fn end_frame(&mut self) {
        self.flush();
    }
}

fn link_program(
    gl: &Gl,
    vertex_source: &str,
    fragment_source: &str,
) -> Result<WebGlProgram, String> {
    let vertex = compile_shader(gl, Gl::VERTEX_SHADER, vertex_source)?;
    let fragment = compile_shader(gl, Gl::FRAGMENT_SHADER, fragment_source)?;
    let program = gl.create_program().ok_or("cannot create a shader program")?;
    gl.attach_shader(&program, &vertex);
    gl.attach_shader(&program, &fragment);
    gl.link_program(&program);
    if gl.get_program_parameter(&program, Gl::LINK_STATUS).as_bool() != Some(true) {
        return Err(format!(
            "shader link failed: {}",
            gl.get_program_info_log(&program).unwrap_or_default()
        ));
    }
    Ok(program)
}

fn compile_shader(gl: &Gl, kind: u32, source: &str) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(kind).ok_or("cannot create a shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl.get_shader_parameter(&shader, Gl::COMPILE_STATUS).as_bool() != Some(true) {
        return Err(format!(
            "shader compile failed: {}",
            gl.get_shader_info_log(&shader).unwrap_or_default()
        ));
    }
    Ok(shader)
}
