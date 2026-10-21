//! Website `NativeUiText.tsx` / `NativeUiTextGlyphs.tsx`: stock mask glyphs with
//! authored pens and ink bounds. No browser font metrics participate in this painter.

use solomon_sim::native_ui_text::{
    NativeUiTextLayout, NativeUiTextPlacement, NativeUiTextSpec, layout_native_ui_text,
    native_ui_font, native_ui_glyph_ink_bounds,
};
use web_sys::{Document, HtmlElement};

use super::{append, element, style};

/// Baseline text keeps a zero-size authored pen; box text occupies its measured flow box.
/// The caller positions this node in the same coordinate space as `spec.x` / `spec.y`.
pub fn native_text(
    document: &Document,
    spec: &NativeUiTextSpec<'_>,
    class: &str,
) -> Result<HtmlElement, String> {
    let layout = layout_native_ui_text(spec);
    let node = element(document, "span", class)?;
    let (position, width, height) = match spec.placement {
        NativeUiTextPlacement::Baseline => ("absolute", 0.0, 0.0),
        NativeUiTextPlacement::Box => ("relative", layout.width, layout.height),
    };
    style(
        &node,
        &format!(
            "display:inline-block;position:{position};left:0;top:0;width:{width}px;height:{height}px;vertical-align:middle;pointer-events:none",
        ),
    )?;
    paint_text_layout(document, &node, &layout)?;
    Ok(node)
}

/// Replace only glyph children; callers retain their actual DOM geometry and event handlers.
pub fn paint_text_layout(
    document: &Document,
    parent: &HtmlElement,
    layout: &NativeUiTextLayout,
) -> Result<(), String> {
    parent.set_text_content(None);
    parent.set_attribute("aria-hidden", "true").map_err(|_| "native text decorative state")?;
    parent
        .set_attribute("data-native-ui-font", layout.font.as_str())
        .map_err(|_| "native text font")?;
    if layout.unsupported_code_points.is_empty() {
        parent
            .remove_attribute("data-native-ui-unsupported")
            .map_err(|_| "native text supported glyphs")?;
    } else {
        let codes: Vec<_> = layout.unsupported_code_points.iter().map(u32::to_string).collect();
        parent
            .set_attribute("data-native-ui-unsupported", &codes.join(","))
            .map_err(|_| "native text unsupported glyphs")?;
    }
    let atlas = native_ui_font(layout.font).atlas;
    let [atlas_width, atlas_height] = atlas.dimensions();
    let source = format!("url('assets/stock/{}.png')", atlas.as_str());
    for glyph in &layout.glyphs {
        let node = element(document, "i", "native-ui-glyph")?;
        node.set_attribute("data-native-ui-glyph", &glyph.glyph.code_point.to_string())
            .map_err(|_| "native text glyph code")?;
        let ink = native_ui_glyph_ink_bounds(glyph);
        let [x, y, _, frame_height] = glyph.glyph.frame;
        let mask_position = format!("{}px {}px", -x * glyph.scale, -y * glyph.scale);
        let mask_size = format!("{}px {}px", atlas_width * glyph.scale, atlas_height * glyph.scale);
        // The native italic shear moves the top/bottom of the logical quad by +3/-3.
        // Its transform origin remains the sprite registration, not the tight mask crop.
        let transform = if glyph.italic {
            format!("matrix(1,0,{},1,0,0)", 6.0 / frame_height)
        } else {
            "none".to_owned()
        };
        let origin_x =
            (glyph.glyph.logical_size[0] / 2.0 - glyph.glyph.trim_origin[0]) * glyph.scale;
        let origin_y =
            (glyph.glyph.logical_size[1] / 2.0 - glyph.glyph.trim_origin[1]) * glyph.scale;
        style(
            &node,
            &format!(
                "display:block;position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;background-color:#{:06x};image-rendering:pixelated;opacity:{};mask-image:{source};mask-position:{mask_position};mask-repeat:no-repeat;mask-size:{mask_size};-webkit-mask-image:{source};-webkit-mask-position:{mask_position};-webkit-mask-repeat:no-repeat;-webkit-mask-size:{mask_size};transform:{transform};transform-origin:{origin_x}px {origin_y}px;pointer-events:none",
                ink.left, ink.top, ink.width, ink.height, glyph.tint, glyph.alpha,
            ),
        )?;
        append(parent, &node)?;
    }
    Ok(())
}
