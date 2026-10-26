//! One native OpenGL and WebGL2 batcher: textured triangles with vertex tints, drawn in submission
//! order. Normal/add emit premultiplied RGB; multiply retains straight RGB, with
//! the Website's native per-channel blend equations. Destinations are
//! logical pixels of the 1600×900 view, which `begin_frame` letterboxes into the canvas.

#[cfg(target_arch = "wasm32")]
pub mod announcements;
pub mod assets;
pub mod raster;
#[cfg(not(target_arch = "wasm32"))]
pub mod text;

use std::rc::Rc;

use assets::Image;
use glow::HasContext;
use solomon_sim::float;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};
#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext};

/// The renderer and its image caches share one context on the window's render thread.
pub type GraphicsContext = Rc<glow::Context>;

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
uniform float u_shadow_only;
uniform float u_texture_premultiplied;
uniform vec4 u_blur_bounds;
uniform vec2 u_blur_sigma;
out vec4 o_color;
vec4 bounded_texture(vec2 uv, vec4 bounds) {
    if (any(lessThan(uv, bounds.xy)) || any(greaterThanEqual(uv, bounds.zw))) {
        return vec4(0.0);
    }
    vec2 halfTexel = 0.5 / vec2(textureSize(u_texture, 0));
    return textureLod(u_texture, clamp(uv, bounds.xy + halfTexel, bounds.zw - halfTexel), 0.0);
}
vec4 cropped_texture(vec2 uv) {
    return bounded_texture(uv, u_shadow_bounds);
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
    if (any(greaterThan(u_blur_sigma, vec2(0.0)))) {
        vec2 size = vec2(textureSize(u_texture, 0));
        vec2 stepSize = sign(u_blur_sigma) / size;
        float sigma = dot(u_blur_sigma, size);
        int radius = int(ceil(3.0 * sigma));
        float alpha = 0.0;
        float total = 0.0;
        for (int i = -radius; i <= radius; ++i) {
            float distance = float(i) / sigma;
            float weight = exp(-0.5 * distance * distance);
            alpha += bounded_texture(v_uv + stepSize * float(i), u_blur_bounds).a * weight;
            total += weight;
        }
        alpha /= total;
        o_color = vec4(vec3(alpha), alpha);
        return;
    }
    bool shadow = u_shadow_sigma.x >= 0.0;
    vec4 texel = shadow ? cropped_texture(v_uv) : texture(u_texture, v_uv);
    if (u_texture_premultiplied > 0.5 && texel.a > 0.0) {
        texel.rgb /= texel.a;
    }
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
        float blurredAlpha = shadow_alpha(v_uv - u_shadow_offset);
        float shadowAlpha = blurredAlpha * u_shadow_color.a;
        if (u_shadow_only > 0.5) {
            alpha = shadowAlpha * v_color.a;
            if (u_shadow_only > 2.5) {
                alpha = texel.a * (1.0 - blurredAlpha) * u_shadow_color.a * v_color.a;
            } else if (u_shadow_only > 1.5) {
                alpha *= 1.0 - texel.a;
            }
            o_color = vec4(u_shadow_color.rgb * mix(1.0, alpha, u_premultiply_rgb), alpha);
            return;
        }
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

#[derive(Clone)]
pub struct Texture {
    id: u32,
    handle: glow::Texture,
    pub width: f32,
    pub height: f32,
}

/// An offscreen colour target. Its texture is vertically inverted relative to loaded PNGs.
#[derive(Clone)]
pub struct RenderTarget {
    pub texture: Texture,
    framebuffer: glow::Framebuffer,
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
                (glow::ONE, glow::ONE_MINUS_SRC_ALPHA, glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA)
            }
            Self::Multiply => (glow::ZERO, glow::SRC_COLOR, glow::ZERO, glow::SRC_ALPHA),
            Self::Add => (glow::ONE, glow::ONE, glow::SRC_ALPHA, glow::ONE),
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
    Premultiplied,
}

/// Per-draw sampling without changing the shared texture's own parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextureSampling {
    #[default]
    Texture,
    Nearest,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShadowOutput {
    #[default]
    Source,
    Shadow,
    /// CSS outer box-shadow clips the caster's original interior from the shadow.
    Outer,
    /// CSS inset box-shadow casts the outside silhouette into the padding box.
    Inner,
}

/// Crop-bounded SourceAlpha shadow. Coordinates are normalized source texture UVs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlphaShadow {
    pub bounds: [f32; 4],
    pub offset: [f32; 2],
    pub sigma: [f32; 2],
    pub color: [f32; 4],
    pub output: ShadowOutput,
}

#[derive(Clone, Copy, PartialEq)]
struct AlphaBlur {
    bounds: [f32; 4],
    sigma: [f32; 2],
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
    gl: GraphicsContext,
    #[cfg(target_arch = "wasm32")]
    browser_gl: Option<WebGl2RenderingContext>,
    u_logical_size: glow::UniformLocation,
    u_pixel_center_offset: glow::UniformLocation,
    u_saturation: glow::UniformLocation,
    u_alpha_mask: glow::UniformLocation,
    u_white_alpha_filter: glow::UniformLocation,
    u_premultiply_rgb: glow::UniformLocation,
    u_color_filter: glow::UniformLocation,
    color_filter: Option<ColorFilter>,
    u_shadow_bounds: glow::UniformLocation,
    u_shadow_color: glow::UniformLocation,
    u_shadow_offset: glow::UniformLocation,
    u_shadow_sigma: glow::UniformLocation,
    shadow: Option<AlphaShadow>,
    u_shadow_only: glow::UniformLocation,
    u_texture_premultiplied: glow::UniformLocation,
    u_blur_bounds: glow::UniformLocation,
    u_blur_sigma: glow::UniformLocation,
    blur: Option<AlphaBlur>,
    framebuffer: Option<glow::Framebuffer>,
    standard_alpha: bool,
    erasing: bool,
    texture_color_mode: TextureColorMode,
    forced_texture_color_mode: Option<TextureColorMode>,
    forced_opacity: Option<f32>,
    saturation: f32,
    opacity: f32,
    clip: Option<[i32; 4]>,
    viewport: [i32; 4],
    logical_size: [f32; 2],
    native_world_pixel_centers: bool,
    vertices: Vec<f32>,
    bound_texture: Option<u32>,
    sampling: TextureSampling,
    nearest_sampler: glow::Sampler,
    blend: BlendMode,
    next_texture_id: std::cell::Cell<u32>,
    white_texture: Texture,
    // Kept alive for the renderer's lifetime; the context holds them bound.
    _program: glow::Program,
    _vertex_array: glow::VertexArray,
    _vertex_buffer: glow::Buffer,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            self.gl.delete_texture(self.white_texture.handle);
            self.gl.delete_sampler(self.nearest_sampler);
            self.gl.delete_buffer(self._vertex_buffer);
            self.gl.delete_vertex_array(self._vertex_array);
            self.gl.delete_program(self._program);
        }
    }
}

impl Renderer {
    /// Shared opaque, non-antialiased game and interface canvas.
    #[cfg(target_arch = "wasm32")]
    pub fn for_canvas(canvas: &HtmlCanvasElement) -> Result<Self, String> {
        let options = js_sys::Object::new();
        for (key, value) in [("alpha", false), ("antialias", false), ("premultipliedAlpha", false)]
        {
            js_sys::Reflect::set(&options, &JsValue::from_str(key), &JsValue::from_bool(value))
                .map_err(|_| "context options")?;
        }
        let context = canvas
            .get_context_with_context_options("webgl2", &options)
            .map_err(|error| format!("webgl2 context: {error:?}"))?
            .ok_or("this browser has no WebGL2")?
            .dyn_into::<WebGl2RenderingContext>()
            .map_err(|_| "webgl2 context has the wrong type".to_string())?;
        Self::new(context)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(context: WebGl2RenderingContext) -> Result<Self, String> {
        let gl = Rc::new(glow::Context::from_webgl2_context(context.clone()));
        // SAFETY: the browser owns this live WebGL2 context and validates its calls.
        let mut renderer = unsafe { Self::new_current(gl)? };
        renderer.browser_gl = Some(context);
        Ok(renderer)
    }

    /// Builds the shared painter in the native window's current OpenGL context.
    ///
    /// # Safety
    /// The caller must keep this context current on this thread whenever the renderer
    /// or a texture cache uses it, and keep the native context alive until they drop.
    #[cfg(not(target_arch = "wasm32"))]
    pub unsafe fn for_native(gl: GraphicsContext) -> Result<Self, String> {
        // SAFETY: the window owner supplies and retains the current context.
        unsafe { Self::new_current(gl) }
    }

    unsafe fn new_current(gl: GraphicsContext) -> Result<Self, String> {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            let program = link_program(&gl, VERTEX_SHADER, FRAGMENT_SHADER)?;
            gl.use_program(Some(program));
            let vertex_array = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vertex_array));
            let vertex_buffer = gl.create_buffer()?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
            let stride = (FLOATS_PER_VERTEX * 4) as i32;
            for (name, size, offset) in [("a_position", 2, 0), ("a_uv", 2, 8), ("a_color", 4, 16)] {
                let location = gl
                    .get_attrib_location(program, name)
                    .ok_or_else(|| format!("shader attribute {name} is missing"))?;
                gl.enable_vertex_attrib_array(location);
                gl.vertex_attrib_pointer_f32(location, size, glow::FLOAT, false, stride, offset);
            }
            let u_logical_size = gl
                .get_uniform_location(program, "u_logical_size")
                .ok_or("u_logical_size is missing")?;
            let u_pixel_center_offset = gl
                .get_uniform_location(program, "u_pixel_center_offset")
                .ok_or("u_pixel_center_offset is missing")?;
            let u_texture =
                gl.get_uniform_location(program, "u_texture").ok_or("u_texture is missing")?;
            let u_saturation = gl
                .get_uniform_location(program, "u_saturation")
                .ok_or("u_saturation is missing")?;
            let u_alpha_mask = gl
                .get_uniform_location(program, "u_alpha_mask")
                .ok_or("u_alpha_mask is missing")?;
            let u_white_alpha_filter = gl
                .get_uniform_location(program, "u_white_alpha_filter")
                .ok_or("u_white_alpha_filter is missing")?;
            let u_premultiply_rgb = gl
                .get_uniform_location(program, "u_premultiply_rgb")
                .ok_or("u_premultiply_rgb is missing")?;
            let u_color_filter = gl
                .get_uniform_location(program, "u_color_filter")
                .ok_or("u_color_filter is missing")?;
            let u_shadow_bounds = gl
                .get_uniform_location(program, "u_shadow_bounds")
                .ok_or("u_shadow_bounds is missing")?;
            let u_shadow_color = gl
                .get_uniform_location(program, "u_shadow_color")
                .ok_or("u_shadow_color is missing")?;
            let u_shadow_offset = gl
                .get_uniform_location(program, "u_shadow_offset")
                .ok_or("u_shadow_offset is missing")?;
            let u_shadow_sigma = gl
                .get_uniform_location(program, "u_shadow_sigma")
                .ok_or("u_shadow_sigma is missing")?;
            let u_shadow_only = gl
                .get_uniform_location(program, "u_shadow_only")
                .ok_or("u_shadow_only is missing")?;
            let u_texture_premultiplied = gl
                .get_uniform_location(program, "u_texture_premultiplied")
                .ok_or("u_texture_premultiplied is missing")?;
            let u_blur_bounds = gl
                .get_uniform_location(program, "u_blur_bounds")
                .ok_or("u_blur_bounds is missing")?;
            let u_blur_sigma = gl
                .get_uniform_location(program, "u_blur_sigma")
                .ok_or("u_blur_sigma is missing")?;
            gl.uniform_1_i32(Some(&u_texture), 0);
            gl.uniform_2_f32(Some(&u_pixel_center_offset), 0.0, 0.0);
            gl.uniform_1_f32(Some(&u_saturation), 1.0);
            gl.uniform_1_f32(Some(&u_alpha_mask), 0.0);
            gl.uniform_1_f32(Some(&u_white_alpha_filter), 0.0);
            gl.uniform_1_f32(Some(&u_premultiply_rgb), 1.0);
            gl.uniform_2_f32(Some(&u_color_filter), -1.0, 1.0);
            gl.uniform_2_f32(Some(&u_shadow_sigma), -1.0, -1.0);
            gl.uniform_1_f32(Some(&u_shadow_only), 0.0);
            gl.uniform_1_f32(Some(&u_texture_premultiplied), 0.0);
            gl.uniform_2_f32(Some(&u_blur_sigma), 0.0, 0.0);
            gl.active_texture(glow::TEXTURE0);
            let nearest_sampler = gl.create_sampler()?;
            for (parameter, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
                (glow::TEXTURE_MAG_FILTER, glow::NEAREST),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.sampler_parameter_i32(nearest_sampler, parameter, value as i32);
            }
            gl.bind_sampler(0, None);
            gl.enable(glow::BLEND);
            let (source, destination, source_alpha, destination_alpha) =
                BlendMode::Normal.factors();
            gl.blend_func_separate(source, destination, source_alpha, destination_alpha);
            #[cfg(target_arch = "wasm32")]
            gl.pixel_store_i32(
                WebGl2RenderingContext::UNPACK_COLORSPACE_CONVERSION_WEBGL,
                glow::NONE as i32,
            );
            #[cfg(target_arch = "wasm32")]
            gl.pixel_store_i32(WebGl2RenderingContext::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0);
            #[cfg(target_arch = "wasm32")]
            gl.pixel_store_i32(WebGl2RenderingContext::UNPACK_FLIP_Y_WEBGL, 0);
            let white = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(white));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                1,
                1,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&[255, 255, 255, 255])),
            );
            check_error(&gl, "texture upload")?;
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
            Ok(Self {
                gl,
                #[cfg(target_arch = "wasm32")]
                browser_gl: None,
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
                u_shadow_only,
                u_texture_premultiplied,
                u_blur_bounds,
                u_blur_sigma,
                blur: None,
                framebuffer: None,
                standard_alpha: false,
                erasing: false,
                texture_color_mode: TextureColorMode::Texture,
                forced_texture_color_mode: None,
                forced_opacity: None,
                saturation: 1.0,
                opacity: 1.0,
                clip: None,
                viewport: [0; 4],
                logical_size: [1.0; 2],
                native_world_pixel_centers: false,
                vertices: Vec::new(),
                bound_texture: None,
                sampling: TextureSampling::Texture,
                nearest_sampler,
                blend: BlendMode::Normal,
                next_texture_id: std::cell::Cell::new(1),
                white_texture: Texture { id: 0, handle: white, width: 1.0, height: 1.0 },
                _program: program,
                _vertex_array: vertex_array,
                _vertex_buffer: vertex_buffer,
            })
        }
    }

    /// Uploads a decoded image as a linearly filtered, edge-clamped texture.
    pub fn upload(&self, image: &Image) -> Result<Texture, String> {
        self.upload_with_repeat(image, false)
    }

    /// Native ground and road meshes intentionally address beyond one tile's texel bounds.
    pub fn upload_with_repeat(&self, image: &Image, repeat: bool) -> Result<Texture, String> {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            let wrap = if repeat { glow::REPEAT } else { glow::CLAMP_TO_EDGE };
            let handle = self.gl.create_texture()?;
            self.gl.bind_texture(glow::TEXTURE_2D, Some(handle));
            for (parameter, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, wrap),
                (glow::TEXTURE_WRAP_T, wrap),
            ] {
                self.gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, value as i32);
            }
            #[cfg(target_arch = "wasm32")]
            self.browser_gl
                .as_ref()
                .expect("browser renderer has its image-upload context")
                .tex_image_2d_with_u32_and_u32_and_html_image_element(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    image,
                )
                .map_err(|error| format!("texture upload of {}: {error:?}", image.src()))?;
            #[cfg(not(target_arch = "wasm32"))]
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                image.width() as i32,
                image.height() as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(image.rgba())),
            );
            check_error(&self.gl, "image texture upload")?;
            let id = self.next_texture_id.get();
            self.next_texture_id.set(id + 1);
            Ok(Texture {
                id,
                handle,
                width: image.natural_width() as f32,
                height: image.natural_height() as f32,
            })
        }
    }

    /// Upload a source-authored RGBA ramp without passing through a browser canvas.
    pub fn upload_rgba(&self, width: u32, height: u32, pixels: &[u8]) -> Result<Texture, String> {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            assert!(width > 0 && height > 0, "RGBA textures have a positive size");
            assert_eq!(pixels.len(), width as usize * height as usize * 4);
            let handle = self.gl.create_texture()?;
            self.gl.bind_texture(glow::TEXTURE_2D, Some(handle));
            for (parameter, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                self.gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, value as i32);
            }
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(pixels)),
            );
            check_error(&self.gl, "texture upload")?;
            let id = self.next_texture_id.get();
            self.next_texture_id.set(id + 1);
            Ok(Texture { id, handle, width: width as f32, height: height as f32 })
        }
    }

    pub fn create_render_target(&self, side: u32) -> Result<RenderTarget, String> {
        self.create_render_target_size([side, side])
    }

    pub fn create_render_target_size(
        &self,
        [width, height]: [u32; 2],
    ) -> Result<RenderTarget, String> {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            let handle = self.gl.create_texture()?;
            self.gl.bind_texture(glow::TEXTURE_2D, Some(handle));
            for (parameter, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                self.gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, value as i32);
            }
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            check_error(&self.gl, "texture upload")?;
            let framebuffer = self.gl.create_framebuffer()?;
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            self.gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(handle),
                0,
            );
            let complete =
                self.gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE;
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, self.framebuffer);
            if !complete {
                self.gl.delete_framebuffer(framebuffer);
                self.gl.delete_texture(handle);
                return Err("the light framebuffer is incomplete".into());
            }
            let id = self.next_texture_id.get();
            self.next_texture_id.set(id + 1);
            Ok(RenderTarget {
                texture: Texture { id, handle, width: width as f32, height: height as f32 },
                framebuffer,
            })
        }
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
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            self.flush();
            let previous_clip = self.clip;
            self.set_clip(None);
            let previous_framebuffer = self.framebuffer.replace(target.framebuffer);
            let previous_viewport = self.viewport;
            let previous_logical_size = self.logical_size;
            self.viewport = [0, 0, target.texture.width as i32, target.texture.height as i32];
            self.logical_size = logical;
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(target.framebuffer));
            self.gl.viewport(0, 0, target.texture.width as i32, target.texture.height as i32);
            self.bind_projection(logical, [target.texture.width, target.texture.height]);
            if let Some(clear) = clear {
                self.gl.clear_color(clear[0], clear[1], clear[2], clear[3]);
                self.gl.clear(glow::COLOR_BUFFER_BIT);
            }
            self.bound_texture = None;
            draw(self);
            self.flush();
            self.set_clip(None);
            self.framebuffer = previous_framebuffer;
            self.viewport = previous_viewport;
            self.logical_size = previous_logical_size;
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, self.framebuffer);
            let [x, y, width, height] = self.viewport;
            self.gl.viewport(x, y, width, height);
            self.bind_projection(self.logical_size, [width as f32, height as f32]);
            self.bound_texture = None;
            self.set_clip(previous_clip);
        }
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
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            if saturation != self.saturation {
                self.flush();
                self.gl.uniform_1_f32(Some(&self.u_saturation), saturation);
                self.saturation = saturation;
            }
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
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            if filter != self.color_filter {
                self.flush();
                let values =
                    filter.map_or([-1.0, 1.0], |filter| [filter.brightness, filter.saturation]);
                self.gl.uniform_2_f32(Some(&self.u_color_filter), values[0], values[1]);
                self.color_filter = filter;
            }
        }
    }

    /// Combines the filtered source with its blurred alpha before group opacity.
    pub fn draw_image_shadow(&mut self, shadow: AlphaShadow, draw: impl FnOnce(&mut Self)) {
        let previous = self.shadow;
        self.set_shadow(Some(shadow));
        draw(self);
        self.set_shadow(previous);
    }

    /// One dense Gaussian alpha pass; exactly one normalized sigma axis is positive.
    /// Two orthogonal passes avoid undersampling wide opaque edges at large radii.
    pub fn draw_alpha_blurred(
        &mut self,
        bounds: [f32; 4],
        sigma: [f32; 2],
        draw: impl FnOnce(&mut Self),
    ) {
        assert!(sigma.iter().all(|value| value.is_finite() && *value >= 0.0));
        assert!((sigma[0] > 0.0) != (sigma[1] > 0.0));
        let previous = self.blur;
        self.set_alpha_blur(Some(AlphaBlur { bounds, sigma }));
        draw(self);
        self.set_alpha_blur(previous);
    }

    fn set_alpha_blur(&mut self, blur: Option<AlphaBlur>) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            if self.blur != blur {
                self.flush();
                if let Some(blur) = blur {
                    self.gl.uniform_4_f32_slice(Some(&self.u_blur_bounds), &blur.bounds);
                    self.gl.uniform_2_f32_slice(Some(&self.u_blur_sigma), &blur.sigma);
                } else {
                    self.gl.uniform_2_f32(Some(&self.u_blur_sigma), 0.0, 0.0);
                }
                self.blur = blur;
            }
        }
    }

    fn set_shadow(&mut self, shadow: Option<AlphaShadow>) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            if shadow != self.shadow {
                self.flush();
                if let Some(shadow) = shadow {
                    self.gl.uniform_4_f32_slice(Some(&self.u_shadow_bounds), &shadow.bounds);
                    self.gl.uniform_4_f32_slice(Some(&self.u_shadow_color), &shadow.color);
                    self.gl.uniform_2_f32_slice(Some(&self.u_shadow_offset), &shadow.offset);
                    self.gl.uniform_2_f32_slice(Some(&self.u_shadow_sigma), &shadow.sigma);
                    self.gl.uniform_1_f32(
                        Some(&self.u_shadow_only),
                        match shadow.output {
                            ShadowOutput::Source => 0.0,
                            ShadowOutput::Shadow => 1.0,
                            ShadowOutput::Outer => 2.0,
                            ShadowOutput::Inner => 3.0,
                        },
                    );
                } else {
                    self.gl.uniform_2_f32(Some(&self.u_shadow_sigma), -1.0, -1.0);
                }
                self.shadow = shadow;
            }
        }
    }

    /// GPU UI groups use ordinary SourceOver alpha, then compose into the native canvas.
    pub fn render_ui_group(
        &mut self,
        target: &RenderTarget,
        logical: [f32; 2],
        draw: impl FnOnce(&mut Self),
    ) {
        self.flush();
        let previous = self.standard_alpha;
        let previous_erasing = self.erasing;
        self.standard_alpha = true;
        self.erasing = false;
        self.bind_blend();
        self.render_to_color(target, logical, [0.0; 4], draw);
        self.standard_alpha = previous;
        self.erasing = previous_erasing;
        self.bind_blend();
    }

    /// Remove the actual source alpha from a premultiplied GPU composition.
    pub fn draw_erased<R>(&mut self, draw: impl FnOnce(&mut Self) -> R) -> R {
        self.flush();
        let previous = self.erasing;
        self.erasing = true;
        self.bind_blend();
        let result = draw(self);
        self.flush();
        self.erasing = previous;
        self.bind_blend();
        result
    }

    fn bind_blend(&self) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            if self.erasing {
                self.gl.blend_func_separate(
                    glow::ZERO,
                    glow::ONE_MINUS_SRC_ALPHA,
                    glow::ZERO,
                    glow::ONE_MINUS_SRC_ALPHA,
                );
                return;
            }
            let (source, destination, source_alpha, destination_alpha) = self.blend.factors();
            let source_alpha = if self.standard_alpha && self.blend == BlendMode::Normal {
                glow::ONE
            } else {
                source_alpha
            };
            self.gl.blend_func_separate(source, destination, source_alpha, destination_alpha);
        }
    }

    /// A panel supplies its actual physical-buffer clip and fade. Flush before changing
    /// the native scissor so queued sprites cannot escape a scrolling Inventory viewport.
    pub fn draw_clipped(&mut self, clip: [i32; 4], opacity: f32, draw: impl FnOnce(&mut Self)) {
        let previous_clip = self.clip;
        let previous_opacity = self.opacity;
        self.set_clip(Some(clip));
        self.opacity = opacity;
        draw(self);
        self.opacity = previous_opacity;
        self.set_clip(previous_clip);
    }

    /// Adjacent UI meshes can retain their scissor without breaking the draw batch.
    /// Target and screen-space scopes restore the incoming clip independently.
    pub fn set_clip(&mut self, clip: Option<[i32; 4]>) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            if self.clip == clip {
                return;
            }
            self.flush();
            if let Some([x, y, width, height]) = clip {
                self.gl.enable(glow::SCISSOR_TEST);
                self.gl.scissor(x, y, width, height);
            } else {
                self.gl.disable(glow::SCISSOR_TEST);
            }
            self.clip = clip;
        }
    }

    pub fn destroy_texture(&self, texture: &Texture) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            self.gl.delete_texture(texture.handle);
        }
    }

    pub fn graphics_context(&self) -> GraphicsContext {
        self.gl.clone()
    }

    pub fn maximum_texture_size(&self) -> Result<u32, String> {
        // SAFETY: the owning window retains this context on its render thread.
        let size = unsafe { self.gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE) };
        check_error(&self.gl, "GPU texture limit")?;
        u32::try_from(size)
            .ok()
            .filter(|size| *size > 0)
            .ok_or_else(|| "GPU texture limit is unavailable".into())
    }

    /// Applies an egui glyph-atlas patch without replacing its live texture.
    pub fn update_rgba(
        &mut self,
        texture: &Texture,
        origin: [usize; 2],
        size: [usize; 2],
        pixels: &[u8],
    ) -> Result<(), String> {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            self.flush();
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture.handle));
            self.bound_texture = Some(texture.id);
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                origin[0] as i32,
                origin[1] as i32,
                size[0] as i32,
                size[1] as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(pixels)),
            );
            check_error(&self.gl, "GPU UI texture patch")
        }
    }

    /// Draws the Rust UI over the complete canvas, then restores world projection/state.
    pub fn draw_screen_space(
        &mut self,
        physical: [i32; 2],
        logical: [f32; 2],
        draw: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            self.flush();
            let clip = self.clip;
            let sampling = self.sampling;
            self.set_clip(None);
            let viewport = self.viewport;
            let logical_size = self.logical_size;
            let native_centers = self.native_world_pixel_centers;
            let saturation = self.saturation;
            let opacity = self.opacity;
            self.viewport = [0, 0, physical[0], physical[1]];
            self.logical_size = logical;
            self.native_world_pixel_centers = false;
            self.opacity = 1.0;
            self.gl.viewport(0, 0, physical[0], physical[1]);
            self.bind_projection(logical, [physical[0] as f32, physical[1] as f32]);
            self.set_saturation(1.0);
            // Uploading a new atlas changes the GL binding outside the draw batch.
            self.bound_texture = None;
            let result = draw(self);
            self.flush();
            self.set_clip(clip);
            self.set_sampling(sampling);
            self.viewport = viewport;
            self.logical_size = logical_size;
            self.native_world_pixel_centers = native_centers;
            self.opacity = opacity;
            self.gl.viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
            self.bind_projection(logical_size, [viewport[2] as f32, viewport[3] as f32]);
            self.set_saturation(saturation);
            result
        }
    }

    pub fn set_texture_sampling(&mut self, texture: &Texture, nearest: bool) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            self.flush();
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture.handle));
            self.bound_texture = Some(texture.id);
            let filter = if nearest { glow::NEAREST } else { glow::LINEAR };
            for parameter in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
                self.gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, filter as i32);
            }
        }
    }

    /// A pixelated UI draw may borrow a linearly filtered world texture unchanged.
    pub fn draw_sampled<R>(
        &mut self,
        sampling: TextureSampling,
        draw: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = self.sampling;
        self.set_sampling(sampling);
        let result = draw(self);
        self.set_sampling(previous);
        result
    }

    /// Retain sampling across adjacent batches; scoped draws restore it themselves.
    pub fn set_sampling(&mut self, sampling: TextureSampling) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            if self.sampling != sampling {
                self.flush();
                self.gl.bind_sampler(
                    0,
                    match sampling {
                        TextureSampling::Texture => None,
                        TextureSampling::Nearest => Some(self.nearest_sampler),
                    },
                );
                self.sampling = sampling;
            }
        }
    }

    pub fn delete_ui_texture(gl: &GraphicsContext, texture: &Texture) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            gl.delete_texture(texture.handle);
        }
    }

    /// Authored mod wearables use nearest sampling for every frame sharing their sheet.
    pub fn set_texture_nearest(&mut self, texture: &Texture) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            self.flush();
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture.handle));
            self.bound_texture = Some(texture.id);
            for parameter in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
                self.gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, glow::NEAREST as i32);
            }
        }
    }

    pub fn destroy_render_target(&self, target: &RenderTarget) {
        Self::delete_render_target(&self.gl, target);
    }

    pub fn delete_render_target(gl: &GraphicsContext, target: &RenderTarget) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            gl.delete_framebuffer(target.framebuffer);
            gl.delete_texture(target.texture.handle);
        }
    }

    /// Source `installNativePixelCenterProjection` changes subsequent projection
    /// binds, including offscreen targets. Enable only on its Hub/Boneyard scopes.
    pub fn set_native_world_pixel_centers(&mut self, enabled: bool) {
        self.native_world_pixel_centers = enabled;
    }

    fn bind_projection(&self, logical: [f32; 2], physical: [f32; 2]) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            self.gl.uniform_2_f32(Some(&self.u_logical_size), logical[0], logical[1]);
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
            self.gl.uniform_2_f32(Some(&self.u_pixel_center_offset), offset[0], offset[1]);
        }
    }

    /// Clears the whole canvas and letterboxes the logical view into it.
    pub fn begin_frame(&mut self, canvas_width: i32, canvas_height: i32, logical: [f32; 2]) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            self.set_clip(None);
            let gl = &self.gl;
            gl.viewport(0, 0, canvas_width, canvas_height);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            let scale = (canvas_width as f32 / logical[0]).min(canvas_height as f32 / logical[1]);
            let width = (logical[0] * scale).round() as i32;
            let height = (logical[1] * scale).round() as i32;
            gl.viewport((canvas_width - width) / 2, (canvas_height - height) / 2, width, height);
            self.viewport =
                [(canvas_width - width) / 2, (canvas_height - height) / 2, width, height];
            self.logical_size = logical;
            self.bind_projection(logical, [width as f32, height as f32]);
            self.vertices.clear();
            self.bound_texture = None;
        }
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
        self.draw_vertices(texture, triangles.iter().flatten().copied(), blend, color_mode);
    }

    /// Submit an indexed UI mesh without expanding it into a second CPU triangle buffer.
    pub fn draw_indexed_with_color_mode(
        &mut self,
        texture: &Texture,
        vertices: &[Vertex],
        indices: &[u32],
        blend: BlendMode,
        color_mode: TextureColorMode,
    ) {
        self.draw_vertices(
            texture,
            indices.iter().map(|index| vertices[*index as usize]),
            blend,
            color_mode,
        );
    }

    fn draw_vertices(
        &mut self,
        texture: &Texture,
        vertices: impl Iterator<Item = Vertex>,
        blend: BlendMode,
        color_mode: TextureColorMode,
    ) {
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            let color_mode = self.forced_texture_color_mode.unwrap_or(color_mode);
            if self.bound_texture != Some(texture.id) {
                self.flush();
                self.gl.bind_texture(glow::TEXTURE_2D, Some(texture.handle));
                self.bound_texture = Some(texture.id);
            }
            if self.blend != blend {
                self.flush();
                // `native-fixed-function-render-pipeline.ts::installNativeBlendModes`.
                // Multiply preserves straight source RGB; normal/add use premultiplied RGB.
                self.blend = blend;
                self.bind_blend();
                self.gl.uniform_1_f32(
                    Some(&self.u_premultiply_rgb),
                    if blend == BlendMode::Multiply { 0.0 } else { 1.0 },
                );
            }
            if self.texture_color_mode != color_mode {
                self.flush();
                self.gl.uniform_1_f32(
                    Some(&self.u_alpha_mask),
                    if color_mode == TextureColorMode::AlphaMask { 1.0 } else { 0.0 },
                );
                self.gl.uniform_1_f32(
                    Some(&self.u_white_alpha_filter),
                    if color_mode == TextureColorMode::WhiteAlphaFilter { 1.0 } else { 0.0 },
                );
                self.gl.uniform_1_f32(
                    Some(&self.u_texture_premultiplied),
                    if color_mode == TextureColorMode::Premultiplied { 1.0 } else { 0.0 },
                );
                self.texture_color_mode = color_mode;
            }
            self.vertices.reserve(vertices.size_hint().1.unwrap_or(0) * FLOATS_PER_VERTEX);
            for vertex in vertices {
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
        // SAFETY: the renderer owns these handles in the live current context;
        // uploads borrow initialized slices only for the duration of each driver call.
        unsafe {
            if self.vertices.is_empty() {
                return;
            }
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&self.vertices),
                glow::DYNAMIC_DRAW,
            );
            self.gl.draw_arrays(
                glow::TRIANGLES,
                0,
                (self.vertices.len() / FLOATS_PER_VERTEX) as i32,
            );
            self.vertices.clear();
        }
    }

    pub fn end_frame(&mut self) {
        self.flush();
    }
}

fn link_program(
    gl: &GraphicsContext,
    vertex_source: &str,
    fragment_source: &str,
) -> Result<glow::Program, String> {
    // SAFETY: the renderer owns these handles in the live current context;
    // uploads borrow initialized slices only for the duration of each driver call.
    unsafe {
        let vertex = compile_shader(gl, glow::VERTEX_SHADER, vertex_source)?;
        let fragment = compile_shader(gl, glow::FRAGMENT_SHADER, fragment_source)?;
        let program = gl.create_program()?;
        gl.attach_shader(program, vertex);
        gl.attach_shader(program, fragment);
        gl.link_program(program);
        gl.detach_shader(program, vertex);
        gl.detach_shader(program, fragment);
        gl.delete_shader(vertex);
        gl.delete_shader(fragment);
        if !gl.get_program_link_status(program) {
            let message = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(format!("shader link failed: {message}"));
        }
        Ok(program)
    }
}

fn compile_shader(gl: &GraphicsContext, kind: u32, source: &str) -> Result<glow::Shader, String> {
    // SAFETY: the renderer owns these handles in the live current context;
    // uploads borrow initialized slices only for the duration of each driver call.
    unsafe {
        let shader = gl.create_shader(kind)?;
        gl.shader_source(shader, &shader_source(source));
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            let message = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            return Err(format!("shader compile failed: {message}"));
        }
        Ok(shader)
    }
}

fn shader_source(source: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        source.to_owned()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        source
            .replace("#version 300 es", "#version 330 core")
            .lines()
            .filter(|line| !line.starts_with("precision "))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn check_error(gl: &glow::Context, operation: &str) -> Result<(), String> {
    // SAFETY: callers use the renderer's current context.
    let error = unsafe { gl.get_error() };
    if error == glow::NO_ERROR {
        Ok(())
    } else {
        Err(format!("{operation}: graphics error 0x{error:04x}"))
    }
}
