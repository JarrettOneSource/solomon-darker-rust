//! Custom Sack names from `renderer/hub-inventory/sack-name.ts`.
//! Stock text stays on its bitmap atlas. This one fallback uses the same browser
//! Canvas2D font metrics and Pixi8.19 wrapping rules as Source's `Text`.

use js_sys::{Array, Intl, Object, Reflect};
use solomon_sim::Vec2;
use solomon_sim::native_ui_text::{NativeUiFontName, native_ui_font};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlElement};

use super::{element, style};

pub(crate) struct BrowserSackName {
    pub node: HtmlElement,
    pub size: Vec2,
    pub baseline_offset: f64,
}

pub(crate) fn browser_sack_name(
    document: &Document,
    text: &str,
    tint: u32,
    max_width: f64,
    wrap: bool,
) -> Result<BrowserSackName, String> {
    let font_size = native_ui_font(NativeUiFontName::Menu).metrics[0];
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|_| "Sack name canvas")?
        .dyn_into()
        .map_err(|_| "Sack name canvas")?;
    let context: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .map_err(|_| "Sack name Canvas2D context")?
        .ok_or("Sack name Canvas2D context")?
        .dyn_into()
        .map_err(|_| "Sack name Canvas2D context")?;
    let font = format!("normal normal normal {font_size}px \"Arial\",sans-serif");
    context.set_font(&font);

    // CanvasTextMetrics.measureFont measures this authored probe, rather than
    // the current name or the browser's CSS line box.
    let probe = context.measure_text("|ÉqÅM").map_err(|_| "Sack name font metrics")?;
    let mut ascent = probe.actual_bounding_box_ascent();
    let mut font_height = ascent + probe.actual_bounding_box_descent();
    if font_height == 0.0 {
        ascent = font_size;
        font_height = font_size;
    }
    let lines = if wrap {
        wrap_name(text, max_width, |token| advance(&context, token), graphemes)?
    } else {
        text.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(str::to_owned).collect()
    };
    let mut width: f64 = 0.0;
    for line in &lines {
        let metrics = context.measure_text(line).map_err(|_| "Sack name line measurement")?;
        // Source includes actual ink overhang when it exceeds advance width.
        width = width.max(
            metrics
                .width()
                .max(metrics.actual_bounding_box_left() + metrics.actual_bounding_box_right()),
        );
    }
    let height = font_size.max(font_height) + lines.len().saturating_sub(1) as f64 * font_size;
    let scale = if !wrap && width > max_width { max_width / width } else { 1.0 };
    // game-webgl.ts fixes the Inventory renderer's resolution at1.
    let pixel_width = width.max(1.0).ceil();
    let pixel_height = height.max(1.0).ceil();
    canvas.set_width(pixel_width as u32);
    canvas.set_height(pixel_height as u32);
    // Resizing resets Canvas2D state. Source draws a left-aligned, alphabetic
    // baseline with no stroke, shadow, padding or letter spacing.
    context.set_font(&font);
    context.set_text_baseline("alphabetic");
    context.set_fill_style_str(&format!("#{tint:06x}"));
    let line_shift = ((font_size - font_height) / 2.0).max(0.0);
    for (index, line) in lines.iter().enumerate() {
        context
            .fill_text(line, 0.0, ascent + line_shift + index as f64 * font_size)
            .map_err(|_| "Sack name canvas text")?;
    }
    canvas.set_attribute("aria-hidden", "true").map_err(|_| "Sack name canvas semantics")?;
    canvas
        .set_attribute(
            "style",
            &format!("display:block;width:{}px;height:{}px", pixel_width, pixel_height),
        )
        .map_err(|_| "Sack name canvas display size")?;
    let node = element(document, "span", "sack-name-browser-fallback")?;
    node.set_attribute("aria-label", text).map_err(|_| "Sack name semantics")?;
    node.set_attribute("data-native-sack-name-width", &width.to_string())
        .map_err(|_| "Sack name measured width")?;
    node.set_attribute("data-native-sack-name-height", &height.to_string())
        .map_err(|_| "Sack name measured height")?;
    node.set_attribute("data-native-sack-name-ascent", &ascent.to_string())
        .map_err(|_| "Sack name measured ascent")?;
    node.set_attribute(
        "data-native-sack-name-lines",
        &serde_json::to_string(&lines).map_err(|_| "Sack name measured lines")?,
    )
    .map_err(|_| "Sack name measured lines")?;
    style(
        &node,
        &format!(
            "position:absolute;display:block;width:{width}px;height:{height}px;font:{font_size}px/{font_size}px Arial,sans-serif;transform:scale({scale});transform-origin:top left;pointer-events:none",
        ),
    )?;
    node.append_child(&canvas).map_err(|_| "Sack name canvas attachment")?;
    Ok(BrowserSackName {
        node,
        size: Vec2::new(width * scale, height * scale),
        baseline_offset: ascent * scale,
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

fn advance(context: &CanvasRenderingContext2d, text: &str) -> Result<f64, String> {
    context
        .measure_text(text)
        .map(|metrics| metrics.width())
        .map_err(|_| "Sack name advance measurement".to_owned())
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
