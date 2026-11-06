//! Licensed WOFF2 faces and system fallbacks, rasterized one glyph at a time.
//!
//! Website `main.tsx` imports Cinzel/Alegreya/JetBrains Mono. Its narrative face
//! deliberately falls back to Georgia; system font files are never redistributed.

use std::cell::RefCell;

use egui::epaint::text::{GlyphBitmap, GlyphRasterizerRequest, RasterizedGlyph};
use egui::{ColorImage, Context, vec2};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    CanvasRenderingContext2d, FontFace, FontFaceDescriptors, FontFaceSet, HtmlCanvasElement,
};

struct GlyphCanvas {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
}

thread_local! {
    static GLYPHS: RefCell<Option<GlyphCanvas>> = const { RefCell::new(None) };
}

pub(crate) struct PlatformFonts {
    set: FontFaceSet,
    faces: Vec<FontFace>,
}

impl Drop for PlatformFonts {
    fn drop(&mut self) {
        for face in &self.faces {
            self.set.delete(face);
        }
        GLYPHS.with(|glyphs| *glyphs.borrow_mut() = None);
    }
}

pub(crate) async fn load(
    surface: &crate::platform::Surface,
    ctx: &Context,
) -> Result<PlatformFonts, String> {
    let document = surface.canvas().owner_document().ok_or("Font document is unavailable")?;
    crate::runtime_ui::enter_phase("fonts")?;
    let set = document.fonts();
    let mut faces = Vec::new();
    let promises = js_sys::Array::new();
    for (family, file, weight, style) in super::FACES {
        let descriptors = FontFaceDescriptors::new();
        descriptors.set_weight(weight);
        descriptors.set_style(style);
        let face = FontFace::new_with_str_and_descriptors(
            family,
            &format!("url('assets/stock/fonts/{file}') format('woff2')"),
            &descriptors,
        )
        .map_err(|error| format!("GPU UI font {file}: {error:?}"))?;
        let loaded = face.load().map_err(|error| format!("GPU UI font {file} load: {error:?}"))?;
        promises.push(&wasm_bindgen_futures::future_to_promise(async move {
            JsFuture::from(loaded).await.map_err(|error| {
                wasm_bindgen::JsValue::from_str(&format!("GPU UI font {file}: {error:?}"))
            })?;
            crate::runtime_ui::completed(&format!("assets/stock/fonts/{file}"))
                .map_err(wasm_bindgen::JsValue::from)?;
            Ok(wasm_bindgen::JsValue::UNDEFINED)
        }));
        set.add(&face).map_err(|error| format!("GPU UI font {file} registration: {error:?}"))?;
        faces.push(face);
    }
    let fonts = PlatformFonts { set, faces };
    JsFuture::from(js_sys::Promise::all(&promises))
        .await
        .map_err(|error| format!("GPU UI font readiness: {error:?}"))?;
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|error| format!("glyph canvas: {error:?}"))?
        .dyn_into()
        .map_err(|_| "glyph canvas is not a canvas")?;
    let context: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .map_err(|error| format!("glyph rasterizer context: {error:?}"))?
        .ok_or("no browser glyph rasterizer")?
        .dyn_into()
        .map_err(|_| "glyph rasterizer is not 2d")?;
    GLYPHS.with(|glyphs| *glyphs.borrow_mut() = Some(GlyphCanvas { canvas, context }));
    super::configure(ctx);
    Ok(fonts)
}

pub(super) fn rasterize(request: &GlyphRasterizerRequest<'_>) -> Result<RasterizedGlyph, String> {
    GLYPHS.with(|glyphs| {
        let glyphs = glyphs.borrow();
        let glyphs = glyphs.as_ref().ok_or("browser glyph rasterizer is not initialized")?;
        let font = super::font_css(request.family, request.font_size_px);
        glyphs.context.set_font(&font);
        glyphs.context.set_text_baseline("alphabetic");
        let metrics = glyphs
            .context
            .measure_text(request.cluster)
            .map_err(|error| format!("glyph measurement: {error:?}"))?;
        let left = metrics.actual_bounding_box_left().ceil();
        let right = metrics.actual_bounding_box_right().ceil();
        let ascent = metrics.actual_bounding_box_ascent().ceil();
        let descent = metrics.actual_bounding_box_descent().ceil();
        let width = ((left + right).max(0.0) as u32 + 2).max(1);
        let height = ((ascent + descent).max(0.0) as u32 + 2).max(1);
        glyphs.canvas.set_width(width);
        glyphs.canvas.set_height(height);
        glyphs.context.set_font(&font);
        glyphs.context.set_text_baseline("alphabetic");
        glyphs.context.set_fill_style_str("#ffffff");
        glyphs
            .context
            .fill_text(request.cluster, left + 1.0, ascent + 1.0)
            .map_err(|error| format!("glyph paint: {error:?}"))?;
        let data = glyphs
            .context
            .get_image_data(0.0, 0.0, f64::from(width), f64::from(height))
            .map_err(|error| format!("glyph bitmap: {error:?}"))?
            .data();
        Ok(RasterizedGlyph {
            bitmap: GlyphBitmap {
                image: ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    &data.0,
                ),
                offset_px: vec2(-left as f32 - 1.0, -ascent as f32 - 1.0),
                is_color: egui::epaint::text::has_emoji_presentation(request.cluster),
            },
            advance_px: metrics.width() as f32,
        })
    })
}
