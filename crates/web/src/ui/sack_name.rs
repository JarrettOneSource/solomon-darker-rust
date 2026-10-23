//! Source `renderer/hub-inventory/sack-name.ts` typography and Pixi wrapping.
//! The native browser shapes each complete font run; egui owns its GPU texture.
use crate::gpu_ui::UiFrame;
use js_sys::{Array, Intl, Object, Reflect};
use solomon_sim::Vec2;
use solomon_sim::native_ui_text::{NativeUiFontName, native_ui_font};
use std::cell::RefCell;
use std::collections::VecDeque;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

#[derive(Clone)]
pub(crate) struct BrowserSackName {
    pub size: Vec2,
    pub baseline_offset: f64,
    texture: egui::TextureHandle,
    pixels: egui::Vec2,
    scale: f32,
}
impl BrowserSackName {
    pub fn paint(&self, painter: &egui::Painter, origin: egui::Pos2, scale: f32) {
        painter.image(
            self.texture.id(),
            egui::Rect::from_min_size(origin, self.pixels * self.scale * scale),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
}
#[derive(Clone, PartialEq, Eq)]
struct NameKey {
    text: String,
    tint: u32,
    max_width: u64,
    wrap: bool,
}
#[derive(Clone, Default)]
struct NameCache(VecDeque<(NameKey, BrowserSackName)>);
struct FontCanvas {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
}
thread_local! { static FONT_CANVAS: RefCell<Option<FontCanvas>> = const { RefCell::new(None) }; }

pub(crate) fn gpu_sack_name(
    frame: &UiFrame<'_>,
    text: &str,
    tint: u32,
    max_width: f64,
    wrap: bool,
) -> Result<BrowserSackName, String> {
    let key = NameKey { text: text.to_owned(), tint, max_width: max_width.to_bits(), wrap };
    let cache_id = egui::Id::unique("source-sack-font-runs");
    let cached = frame.ctx.data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<NameCache>(cache_id);
        let index = cache.0.iter().position(|(prior, _)| prior == &key)?;
        let entry = cache.0.remove(index).expect("located font run");
        let name = entry.1.clone();
        cache.0.push_back(entry);
        Some(name)
    });
    if let Some(name) = cached {
        return Ok(name);
    }
    let (image, size, baseline_offset, scale) = FONT_CANVAS.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let document = web_sys::window()
                .and_then(|window| window.document())
                .ok_or("Sack font document")?;
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
            *slot = Some(FontCanvas { canvas, context });
        }
        let FontCanvas { canvas, context } = slot.as_ref().expect("initialized font canvas");
        let font_size = native_ui_font(NativeUiFontName::Menu).metrics[0];
        let font = format!("normal normal normal {font_size}px \"Arial\",sans-serif");
        context.set_font(&font);
        let probe = context.measure_text("|ÉqÅM").map_err(|_| "Sack font probe")?;
        let mut ascent = probe.actual_bounding_box_ascent();
        let mut font_height = ascent + probe.actual_bounding_box_descent();
        if font_height == 0.0 {
            ascent = font_size;
            font_height = font_size;
        }
        let lines = if wrap {
            wrap_name(
                text,
                max_width,
                |token| {
                    context
                        .measure_text(token)
                        .map(|metrics| metrics.width())
                        .map_err(|_| "Sack font advance".into())
                },
                graphemes,
            )?
        } else {
            text.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(str::to_owned).collect()
        };
        let mut width = 0.0_f64;
        for line in &lines {
            let metrics = context.measure_text(line).map_err(|_| "Sack font line")?;
            width = width.max(
                metrics
                    .width()
                    .max(metrics.actual_bounding_box_left() + metrics.actual_bounding_box_right()),
            );
        }
        let height = font_size.max(font_height) + lines.len().saturating_sub(1) as f64 * font_size;
        let scale = if !wrap && width > max_width { max_width / width } else { 1.0 };
        let pixel_width = width.max(1.0).ceil() as u32;
        let pixel_height = height.max(1.0).ceil() as u32;
        canvas.set_width(pixel_width);
        canvas.set_height(pixel_height);
        context.set_font(&font);
        context.set_text_baseline("alphabetic");
        context.set_fill_style_str(&format!("#{tint:06x}"));
        let line_shift = ((font_size - font_height) / 2.0).max(0.0);
        for (index, line) in lines.iter().enumerate() {
            context
                .fill_text(line, 0.0, ascent + line_shift + index as f64 * font_size)
                .map_err(|_| "Sack font run")?;
        }
        let data = context
            .get_image_data(0.0, 0.0, pixel_width as f64, pixel_height as f64)
            .map_err(|_| "Sack font pixels")?
            .data();
        Ok::<_, String>((
            egui::ColorImage::from_rgba_unmultiplied(
                [pixel_width as usize, pixel_height as usize],
                &data.0,
            ),
            Vec2::new(width * scale, height * scale),
            ascent * scale,
            scale as f32,
        ))
    })?;
    let pixels = egui::vec2(image.width() as f32, image.height() as f32);
    let texture =
        frame.ctx.load_texture("Source Sack font run", image, egui::TextureOptions::LINEAR);
    let name = BrowserSackName { texture, size, baseline_offset, scale, pixels };
    frame.ctx.data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<NameCache>(cache_id);
        cache.0.push_back((key, name.clone()));
        if cache.0.len() > 128 {
            cache.0.pop_front();
        }
    });
    Ok(name)
}
/// Pixi wordWrap with the Sack's fixed whiteSpace=pre, breakWords=true and
/// letterSpacing=0. Fitting sums advance widths; final ink is measured afterward.
fn wrap_name<A, G>(
    text: &str,
    max_width: f64,
    mut advance: A,
    mut graphemes: G,
) -> Result<Vec<String>, String>
where
    A: FnMut(&str) -> Result<f64, String>,
    G: FnMut(&str) -> Result<Vec<String>, String>,
{
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut width = 0.0;
    let mut can_prepend_spaces = true;
    for token in tokens(text) {
        if token.starts_with(['\r', '\n']) {
            lines.push(trim_line(&line));
            line.clear();
            width = 0.0;
            can_prepend_spaces = true;
            continue;
        }
        let token_width = advance(&token)?;
        if token_width > max_width {
            if !line.is_empty() {
                lines.push(trim_line(&line));
                line.clear();
                width = 0.0;
            }
            for character in graphemes(&token)? {
                let character_width = advance(&character)?;
                if character_width + width > max_width {
                    lines.push(trim_line(&line));
                    line.clear();
                    width = 0.0;
                    can_prepend_spaces = false;
                }
                line.push_str(&character);
                width += character_width;
            }
        } else {
            if token_width + width > max_width {
                lines.push(trim_line(&line));
                line.clear();
                width = 0.0;
                can_prepend_spaces = false;
            }
            if !line.is_empty()
                || !token.chars().next().is_some_and(breaking_space)
                || can_prepend_spaces
            {
                line.push_str(&token);
                width += token_width;
            }
        }
    }
    let trimmed = trim_line(&line);
    if !trimmed.is_empty() {
        lines.push(trimmed);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    Ok(lines)
}

fn trim_line(line: &str) -> String {
    line.trim_end_matches(breaking_space).to_owned()
}

fn breaking_space(character: char) -> bool {
    matches!(character as u32, 9 | 32 | 8192..=8198 | 8200..=8202 | 8287 | 12288)
}

fn tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if breaking_space(character) || matches!(character, '\r' | '\n') {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
            if character == '\r' && characters.peek() == Some(&'\n') {
                characters.next();
                tokens.push("\r\n".to_owned());
            } else {
                tokens.push(character.to_string());
            }
            continue;
        }
        token.push(character);
        let break_after = matches!(character as u32, 45 | 8208 | 8211 | 8212 | 173);
        if break_after
            && characters
                .peek()
                .is_some_and(|next| !breaking_space(*next) && !matches!(next, '\r' | '\n'))
        {
            tokens.push(std::mem::take(&mut token));
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    tokens
}

fn graphemes(text: &str) -> Result<Vec<String>, String> {
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

#[cfg(test)]
mod tests {
    use super::wrap_name;

    #[test]
    fn sack_names_wrap_like_source_with_actual_browser_measurements() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../sim/tests/fixtures/website/sack-name-goldens.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            if !case["input"]["wrap"].as_bool().unwrap() {
                continue;
            }
            let text = case["input"]["value"].as_str().unwrap();
            let width = case["input"]["maxWidth"].as_f64().unwrap();
            let lines = wrap_name(
                text,
                width,
                |token| Ok(fixture["advances"][token].as_f64().expect(token)),
                |token| {
                    let mut characters = Vec::new();
                    for row in fixture["graphemes"][token].as_array().expect(token) {
                        characters.push(row.as_str().unwrap().to_owned());
                    }
                    Ok(characters)
                },
            )
            .unwrap();
            let expected: Vec<_> = case["source"]["lines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row.as_str().unwrap().to_owned())
                .collect();
            assert_eq!(lines, expected, "{text:?} width{width}");
        }
    }
}
