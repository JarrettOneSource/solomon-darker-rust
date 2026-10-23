//! Custom Sack names use individually rasterized Arial glyphs on the GPU.
//! The source Pixi word-wrap algorithm remains shared with its retained goldens.
use crate::gpu_ui::UiFrame;
use js_sys::{Array, Intl, Object, Reflect};
use solomon_sim::Vec2;
use solomon_sim::native_ui_text::{NativeUiFontName, native_ui_font};
use wasm_bindgen::JsValue;

pub(crate) struct BrowserSackName {
    pub lines: Vec<String>,
    pub size: Vec2,
    pub baseline_offset: f64,
    pub font_size: f32,
    pub scale: f32,
    pub tint: u32,
}
impl BrowserSackName {
    pub fn paint(
        &self,
        frame: &UiFrame<'_>,
        painter: &egui::Painter,
        origin: egui::Pos2,
        scale: f32,
    ) {
        for (index, line) in self.lines.iter().enumerate() {
            let font = egui::FontId::new(
                self.font_size * self.scale * scale,
                egui::FontFamily::Name("Arial".into()),
            );
            painter.text(
                origin + egui::vec2(0.0, index as f32 * self.font_size * self.scale * scale),
                egui::Align2::LEFT_TOP,
                line,
                font,
                egui::Color32::from_rgb(
                    (self.tint >> 16) as u8,
                    (self.tint >> 8) as u8,
                    self.tint as u8,
                ),
            );
        }
        let _ = frame;
    }
}
pub(crate) fn gpu_sack_name(
    frame: &UiFrame<'_>,
    text: &str,
    tint: u32,
    max_width: f64,
    wrap: bool,
) -> Result<BrowserSackName, String> {
    let font_size = native_ui_font(NativeUiFontName::Menu).metrics[0] as f32;
    let font = egui::FontId::new(font_size, egui::FontFamily::Name("Arial".into()));
    let advance = |text: &str| {
        Ok(frame.ctx.fonts_mut(|fonts| {
            fonts.layout_no_wrap(text.into(), font.clone(), egui::Color32::WHITE).size().x
        }) as f64)
    };
    let lines = if wrap {
        wrap_name(text, max_width, advance, graphemes)?
    } else {
        text.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(str::to_owned).collect()
    };
    let mut width: f64 = 0.0;
    for line in &lines {
        width = width.max(advance(line)?);
    }
    let scale = if !wrap && width > max_width { (max_width / width) as f32 } else { 1.0 };
    Ok(BrowserSackName {
        size: Vec2::new(width * scale as f64, font_size as f64 * lines.len() as f64 * scale as f64),
        baseline_offset: font_size as f64 * 0.8 * scale as f64,
        lines,
        font_size,
        scale,
        tint,
    })
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
