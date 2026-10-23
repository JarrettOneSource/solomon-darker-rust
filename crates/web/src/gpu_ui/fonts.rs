//! Licensed WOFF2 faces and system fallbacks, rasterized one glyph at a time.
//!
//! Website `main.tsx` imports Cinzel/Alegreya/JetBrains Mono. Its narrative face
//! deliberately falls back to Georgia; system font files are never redistributed.

use std::cell::RefCell;

use egui::epaint::text::{
    FontPriority, GlyphBitmap, GlyphRasterizer, GlyphRasterizerRequest, RasterizedGlyph,
};
use egui::{ColorImage, Context, FontDefinitions, FontFamily, vec2};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    CanvasRenderingContext2d, Document, FontFace, FontFaceDescriptors, FontFaceSet,
    HtmlCanvasElement,
};

struct GlyphCanvas {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
}

thread_local! {
    static GLYPHS: RefCell<Option<GlyphCanvas>> = const { RefCell::new(None) };
    static GLYPH_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(super) struct BrowserFonts {
    set: FontFaceSet,
    faces: Vec<FontFace>,
}

impl Drop for BrowserFonts {
    fn drop(&mut self) {
        for face in &self.faces {
            self.set.delete(face);
        }
        GLYPHS.with(|glyphs| *glyphs.borrow_mut() = None);
    }
}

const FACES: [(&str, &str, &str, &str); 9] = [
    ("SDGPU Cinzel", "cinzel-latin-500-normal.woff2", "500", "normal"),
    ("SDGPU Cinzel", "cinzel-latin-700-normal.woff2", "700", "normal"),
    ("SDGPU Cinzel", "cinzel-latin-800-normal.woff2", "800", "normal"),
    ("SDGPU Alegreya", "alegreya-latin-400-normal.woff2", "400", "normal"),
    ("SDGPU Alegreya", "alegreya-latin-500-normal.woff2", "500", "normal"),
    ("SDGPU Alegreya", "alegreya-latin-700-normal.woff2", "700", "normal"),
    ("SDGPU Alegreya", "alegreya-latin-400-italic.woff2", "400", "italic"),
    ("SDGPU JetBrains Mono", "jetbrains-mono-latin-400-normal.woff2", "400", "normal"),
    ("SDGPU JetBrains Mono", "jetbrains-mono-latin-600-normal.woff2", "600", "normal"),
];

pub(super) fn asset_count() -> usize {
    FACES.len()
}

pub(super) async fn load(document: &Document, ctx: &Context) -> Result<BrowserFonts, String> {
    let set = document.fonts();
    let mut faces = Vec::new();
    let promises = js_sys::Array::new();
    for (family, file, weight, style) in FACES {
        let descriptors = FontFaceDescriptors::new();
        descriptors.set_weight(weight);
        descriptors.set_style(style);
        let face = FontFace::new_with_str_and_descriptors(
            family,
            &format!("url('assets/stock/fonts/{file}') format('woff2')"),
            &descriptors,
        )
        .map_err(|error| format!("GPU UI font {file}: {error:?}"))?;
        promises.push(
            face.load().map_err(|error| format!("GPU UI font {file} load: {error:?}"))?.as_ref(),
        );
        set.add(&face).map_err(|error| format!("GPU UI font {file} registration: {error:?}"))?;
        faces.push(face);
    }
    let fonts = BrowserFonts { set, faces };
    JsFuture::from(js_sys::Promise::all(&promises))
        .await
        .map_err(|error| format!("GPU UI font readiness: {error:?}"))?;
    for (_, file, _, _) in FACES {
        crate::runtime_ui::completed("fonts", file, 1)?;
    }
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
    let mut definitions = FontDefinitions::default();
    let fallback = definitions.families[&FontFamily::Proportional].clone();
    for family in [
        "Georgia",
        "Georgia Bold",
        "Arial",
        "Palatino",
        "Cinzel",
        "Cinzel Bold",
        "Cinzel ExtraBold",
        "Alegreya",
        "Alegreya Medium",
        "Alegreya Bold",
        "Alegreya Italic",
        "JetBrains Mono",
        "JetBrains Mono Bold",
        "IM FELL English",
    ] {
        definitions.families.insert(FontFamily::Name(family.into()), fallback.clone());
    }
    ctx.set_fonts(definitions);
    ctx.set_glyph_rasterizers(vec![
        GlyphRasterizer::new("solomon_web::gpu_ui::browser_fonts", |request| {
            match rasterize(request) {
                Ok(glyph) => Some(glyph),
                Err(error) => {
                    GLYPH_ERROR.with(|slot| *slot.borrow_mut() = Some(error));
                    None
                }
            }
        })
        .with_priority(FontPriority::Highest),
    ]);
    Ok(fonts)
}

pub(super) fn take_error() -> Option<String> {
    GLYPH_ERROR.with(|slot| slot.borrow_mut().take())
}

fn font_css(family: &FontFamily, size: f32) -> String {
    let name = match family {
        FontFamily::Name(name) => name.as_ref(),
        FontFamily::Monospace => "JetBrains Mono",
        FontFamily::Proportional => "Georgia",
    };
    let (weight, style, family) = match name {
        "Cinzel" => ("500", "normal", "'SDGPU Cinzel',Georgia,serif"),
        "Cinzel Bold" => ("700", "normal", "'SDGPU Cinzel',Georgia,serif"),
        "Cinzel ExtraBold" => ("800", "normal", "'SDGPU Cinzel',Georgia,serif"),
        "Alegreya" => ("400", "normal", "'SDGPU Alegreya',Georgia,serif"),
        "Alegreya Medium" => ("500", "normal", "'SDGPU Alegreya',Georgia,serif"),
        "Alegreya Bold" => ("700", "normal", "'SDGPU Alegreya',Georgia,serif"),
        "Alegreya Italic" => ("400", "italic", "'SDGPU Alegreya',Georgia,serif"),
        "JetBrains Mono" => ("400", "normal", "'SDGPU JetBrains Mono',monospace"),
        "JetBrains Mono Bold" => ("600", "normal", "'SDGPU JetBrains Mono',monospace"),
        "Palatino" => ("400", "normal", "Palatino,'Palatino Linotype',Georgia,serif"),
        "Arial" => ("400", "normal", "Arial,Helvetica,sans-serif"),
        "Georgia Bold" => ("700", "normal", "Georgia,'Times New Roman',serif"),
        _ => ("400", "normal", "Georgia,'Times New Roman',serif"),
    };
    format!("{style} {weight} {size}px {family}")
}

fn rasterize(request: &GlyphRasterizerRequest<'_>) -> Result<RasterizedGlyph, String> {
    GLYPHS.with(|glyphs| {
        let glyphs = glyphs.borrow();
        let glyphs = glyphs.as_ref().ok_or("browser glyph rasterizer is not initialized")?;
        let font = font_css(request.family, request.font_size_px);
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
