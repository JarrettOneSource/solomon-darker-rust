use super::{Metrics, TextLayout};
use js_sys::{Array, Intl, Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub(super) struct FontCanvas {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    font: String,
}
impl FontCanvas {
    pub(super) fn new(size: f64) -> Result<Self, String> {
        let document =
            web_sys::window().and_then(|window| window.document()).ok_or("Sack font document")?;
        let canvas: HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(|_| "Sack font canvas")?
            .dyn_into()
            .map_err(|_| "Sack font canvas type")?;
        let context: CanvasRenderingContext2d = canvas
            .get_context("2d")
            .map_err(|_| "Sack font rasterizer")?
            .ok_or("Sack font rasterizer")?
            .dyn_into()
            .map_err(|_| "Sack font rasterizer type")?;
        let font = format!("normal normal normal {size}px \"Arial\",sans-serif");
        context.set_font(&font);
        Ok(Self { canvas, context, font })
    }
    pub(super) fn metrics(&self, text: &str) -> Result<Metrics, String> {
        let metrics = self.context.measure_text(text).map_err(|_| "Sack font metrics")?;
        Ok(Metrics {
            advance: metrics.width(),
            left: metrics.actual_bounding_box_left(),
            right: metrics.actual_bounding_box_right(),
            ascent: metrics.actual_bounding_box_ascent(),
            descent: metrics.actual_bounding_box_descent(),
        })
    }
    pub(super) fn paint(
        &self,
        lines: &[String],
        tint: u32,
        layout: TextLayout,
    ) -> Result<egui::ColorImage, String> {
        self.canvas.set_width(layout.width);
        self.canvas.set_height(layout.height);
        self.context.set_font(&self.font);
        self.context.set_text_baseline("alphabetic");
        self.context.set_fill_style_str(&format!("#{tint:06x}"));
        for (index, line) in lines.iter().enumerate() {
            self.context
                .fill_text(line, 0.0, layout.baseline + index as f64 * layout.line_height)
                .map_err(|_| "Sack font run")?;
        }
        let data = self
            .context
            .get_image_data(0.0, 0.0, layout.width as f64, layout.height as f64)
            .map_err(|_| "Sack font pixels")?
            .data();
        Ok(egui::ColorImage::from_rgba_unmultiplied(
            [layout.width as usize, layout.height as usize],
            &data.0,
        ))
    }
}

pub(super) fn graphemes(text: &str) -> Result<Vec<String>, String> {
    let intl = Reflect::get(&js_sys::global(), &JsValue::from_str("Intl"))
        .map_err(|_| "Sack name Intl")?;
    let constructor =
        Reflect::get(&intl, &JsValue::from_str("Segmenter")).map_err(|_| "Sack name Segmenter")?;
    if !constructor.is_function() {
        // Source uses code points on browsers without Intl.Segmenter.
        return Ok(text.chars().map(|character| character.to_string()).collect());
    }
    let segmenter = Intl::Segmenter::new(&Array::new(), &Object::new());
    let segments = segmenter.segment(text);
    let iterator = js_sys::try_iter(&segments)
        .map_err(|_| "Sack name grapheme iterator")?
        .ok_or("Sack name grapheme iterator")?;
    let mut graphemes = Vec::new();
    for segment in iterator {
        let segment = segment.map_err(|_| "Sack name grapheme")?;
        let value = Reflect::get(&segment, &JsValue::from_str("segment"))
            .map_err(|_| "Sack name grapheme value")?
            .as_string()
            .ok_or("Sack name grapheme value")?;
        graphemes.push(value);
    }
    Ok(graphemes)
}
