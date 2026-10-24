//! Browser projection of stock UI art. Website `NativeUiSprite.tsx` and
//! `NativeUiStrip.tsx` use the same bundle registration and horizontal thirds.

pub(crate) mod belt_pull_off;
pub(crate) mod cloud;
pub(crate) mod item_hover;
pub(crate) mod mod_icon;
pub(crate) mod portraits;
pub(crate) mod sack_name;
pub(crate) mod settings;
pub(crate) mod skill_hover;
pub(crate) mod text;
pub(crate) mod trusted_gesture;

use solomon_assets::{SpriteRecord, parse_bundle};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, HtmlElement};

#[wasm_bindgen]
extern "C" {
    /// Source HTML surfaces use JavaScript's `String(value)`, including array and
    /// object coercion. Reuse the platform rather than duplicate its number rules.
    #[wasm_bindgen(js_name = String)]
    pub(crate) fn javascript_string(value: &JsValue) -> String;
}

use crate::assets;

pub struct StockAtlas {
    name: &'static str,
    records: Vec<SpriteRecord>,
    width: f64,
    height: f64,
}

impl StockAtlas {
    pub async fn load(name: &'static str) -> Result<Self, String> {
        let bytes = assets::fetch_bytes(&format!("assets/stock/{name}.bundle")).await?;
        let records = parse_bundle(&bytes).map_err(|error| format!("{name}.bundle: {error}"))?;
        let image = assets::load_image(&format!("assets/stock/{name}.png")).await?;
        Ok(Self {
            name,
            records,
            width: f64::from(image.natural_width()),
            height: f64::from(image.natural_height()),
        })
    }

    pub fn sprite(
        &self,
        document: &Document,
        index: usize,
        class: &str,
    ) -> Result<HtmlElement, String> {
        let record = self.record(index)?;
        let sprite = element(document, "span", class)?;
        sprite.set_attribute("aria-hidden", "true").map_err(|_| "sprite aria-hidden")?;
        sprite
            .set_attribute("data-stock-record", &format!("{}.{index}", self.name))
            .map_err(|_| "sprite record")?;
        style(
            &sprite,
            &format!(
                "display:block;position:relative;width:{}px;height:{}px;pointer-events:none",
                record.logical_width, record.logical_height
            ),
        )?;
        let crop = element(document, "span", "stock-crop")?;
        let [x, y] = record.registered_origin();
        style(
            &crop,
            &format!(
                "display:block;position:absolute;left:{x}px;top:{y}px;width:{}px;height:{}px;background-image:url('assets/stock/{}.png');background-position:-{}px -{}px;background-repeat:no-repeat",
                record.width, record.height, self.name, record.x, record.y
            ),
        )?;
        append(&sprite, &crop)?;
        Ok(sprite)
    }

    /// Source NativeUiSprite maskTint: white-alpha stock art keeps its alpha and
    /// takes one authored colour. Ordinary sprite colours remain untouched.
    pub fn mask_sprite(
        &self,
        document: &Document,
        index: usize,
        tint: u32,
        class: &str,
    ) -> Result<HtmlElement, String> {
        let sprite = self.sprite(document, index, class)?;
        let crop: HtmlElement = sprite
            .query_selector(".stock-crop")
            .map_err(|_| "stock mask crop")?
            .ok_or("stock mask has no crop")?
            .dyn_into()
            .map_err(|_| "stock mask crop is not HTML")?;
        let record = self.record(index)?;
        let source = format!("url('assets/stock/{}.png')", self.name);
        let position = format!("-{}px -{}px", record.x, record.y);
        let size = format!("{}px {}px", self.width, self.height);
        for (name, value) in [
            ("background-image", "none".to_owned()),
            ("background-color", format!("#{tint:06x}")),
            ("mask-image", source.clone()),
            ("mask-position", position.clone()),
            ("mask-size", size.clone()),
            ("mask-repeat", "no-repeat".to_owned()),
            ("-webkit-mask-image", source),
            ("-webkit-mask-position", position),
            ("-webkit-mask-size", size),
            ("-webkit-mask-repeat", "no-repeat".to_owned()),
        ] {
            crop.style().set_property(name, &value).map_err(|_| "stock sprite mask tint")?;
        }
        Ok(sprite)
    }

    /// Unregistered crop for repeated textures and untrimmed sink/frame pieces.
    pub fn raw_sprite(&self, document: &Document, index: usize) -> Result<HtmlElement, String> {
        let record = self.record(index)?;
        self.slice(
            document,
            index,
            [0.0, 0.0, f64::from(record.width), f64::from(record.height)],
            [0.0, 0.0, f64::from(record.width), f64::from(record.height)],
        )
    }

    /// Repeats an existing crop inside a clipped rectangle, preserving the stock texture scale.
    pub fn tiles(
        &self,
        document: &Document,
        index: usize,
        width: f64,
        height: f64,
    ) -> Result<HtmlElement, String> {
        let record = self.record(index)?;
        let tiles = element(document, "span", "stock-tiles")?;
        style(
            &tiles,
            &format!(
                "position:absolute;left:0;top:0;width:{width}px;height:{height}px;overflow:hidden;pointer-events:none"
            ),
        )?;
        let mut y = 0.0;
        while y < height {
            let mut x = 0.0;
            while x < width {
                let tile = self.raw_sprite(document, index)?;
                tile.style().set_property("left", &format!("{x}px")).map_err(|_| "tile x")?;
                tile.style().set_property("top", &format!("{y}px")).map_err(|_| "tile y")?;
                append(&tiles, &tile)?;
                x += f64::from(record.width);
            }
            y += f64::from(record.height);
        }
        Ok(tiles)
    }

    /// Conventional nine-slice art used by Inventory's information boxes and Skills pages.
    pub fn frame(
        &self,
        document: &Document,
        index: usize,
        width: f64,
        height: f64,
        border: f64,
    ) -> Result<HtmlElement, String> {
        let record = self.record(index)?;
        let frame = element(document, "span", "stock-frame")?;
        style(
            &frame,
            &format!(
                "position:absolute;left:0;top:0;width:{width}px;height:{height}px;pointer-events:none"
            ),
        )?;
        let source_x = [0.0, border, f64::from(record.width) - border];
        let source_y = [0.0, border, f64::from(record.height) - border];
        // Pixi's NineSliceGeometry shrinks all four destination borders together
        // when either axis is smaller than two borders; source UVs stay unchanged.
        let border_scale = 1.0_f64.min(width / (2.0 * border)).min(height / (2.0 * border));
        let destination_border = border * border_scale;
        let destination_x = [0.0, destination_border, width - destination_border];
        let destination_y = [0.0, destination_border, height - destination_border];
        let source_widths = [border, f64::from(record.width) - border * 2.0, border];
        let source_heights = [border, f64::from(record.height) - border * 2.0, border];
        let widths = [destination_border, width - destination_border * 2.0, destination_border];
        let heights = [destination_border, height - destination_border * 2.0, destination_border];
        for row in 0..3 {
            for column in 0..3 {
                if widths[column] <= 0.0 || heights[row] <= 0.0 {
                    continue;
                }
                append(
                    &frame,
                    &self.slice(
                        document,
                        index,
                        [
                            source_x[column],
                            source_y[row],
                            source_widths[column],
                            source_heights[row],
                        ],
                        [destination_x[column], destination_y[row], widths[column], heights[row]],
                    )?,
                )?;
            }
        }
        Ok(frame)
    }

    /// Native helper `0x00415230` repeats the middle third and preserves both end caps.
    pub fn strip(
        &self,
        document: &Document,
        index: usize,
        width: f64,
        class: &str,
    ) -> Result<HtmlElement, String> {
        let record = self.record(index)?;
        let strip = element(document, "span", class)?;
        strip.set_attribute("aria-hidden", "true").map_err(|_| "strip aria-hidden")?;
        style(
            &strip,
            &format!(
                "display:block;position:absolute;width:{width}px;height:{}px;overflow:hidden;pointer-events:none",
                record.height
            ),
        )?;
        if width == 0.0 {
            return Ok(strip);
        }
        let third = f64::from(record.width) / 3.0;
        let mut pieces = vec![(0.0, 0.0, third)];
        let mut remaining = width - 2.0 * third;
        let mut target_left = third;
        while remaining > 1e-9 {
            let width = third.min(remaining);
            pieces.push((third, target_left, width));
            target_left += width;
            remaining -= width;
        }
        pieces.push((2.0 * third, width - third, third));
        for (source_left, target_left, width) in pieces {
            let piece = element(document, "span", "stock-strip-piece")?;
            style(
                &piece,
                &format!(
                    "display:block;position:absolute;left:{target_left}px;top:0;width:{width}px;height:{}px;background-image:url('assets/stock/{}.png');background-position:-{}px -{}px;background-repeat:no-repeat",
                    record.height,
                    self.name,
                    f64::from(record.x) + source_left,
                    record.y
                ),
            )?;
            append(&strip, &piece)?;
        }
        Ok(strip)
    }

    pub fn record(&self, index: usize) -> Result<&SpriteRecord, String> {
        self.records.get(index).ok_or_else(|| format!("{}.bundle has no record {index}", self.name))
    }

    /// A source subrectangle stretched into a destination rectangle, both in pixels.
    pub fn slice(
        &self,
        document: &Document,
        index: usize,
        source: [f64; 4],
        destination: [f64; 4],
    ) -> Result<HtmlElement, String> {
        let record = self.record(index)?;
        let [source_x, source_y, source_width, source_height] = source;
        let [left, top, width, height] = destination;
        let scale_x = width / source_width;
        let scale_y = height / source_height;
        let slice = element(document, "span", "stock-slice")?;
        style(
            &slice,
            &format!(
                "position:absolute;display:block;pointer-events:none;left:{left}px;top:{top}px;width:{width}px;height:{height}px;background-image:url('assets/stock/{}.png');background-size:{}px {}px;background-position:{}px {}px;background-repeat:no-repeat",
                self.name,
                self.width * scale_x,
                self.height * scale_y,
                -(f64::from(record.x) + source_x) * scale_x,
                -(f64::from(record.y) + source_y) * scale_y
            ),
        )?;
        Ok(slice)
    }

    /// `NativeUiNineSlice.tsx`: one authored corner mirrored at the other three corners,
    /// with its final five percent stretched through the connecting edges and center.
    pub fn nine_slice(
        &self,
        document: &Document,
        index: usize,
        width: f64,
        height: f64,
    ) -> Result<HtmlElement, String> {
        self.native_frame(document, index, width, height, true)
    }

    /// Hall row and kill frames use the same native corners/edges with an empty interior.
    pub fn nine_slice_border(
        &self,
        document: &Document,
        index: usize,
        width: f64,
        height: f64,
    ) -> Result<HtmlElement, String> {
        self.native_frame(document, index, width, height, false)
    }

    fn native_frame(
        &self,
        document: &Document,
        index: usize,
        width: f64,
        height: f64,
        fill: bool,
    ) -> Result<HtmlElement, String> {
        let record = self.record(index)?;
        let corner_width = f64::from(record.width);
        let corner_height = f64::from(record.height);
        // CSS rectangles have half-open bounds. Reach the +1 mirrored corner origin
        // below, otherwise a transparent one-pixel seam exposes the world behind it.
        let inner_width = width - corner_width * 2.0 + 1.0;
        let inner_height = height - corner_height * 2.0 + 1.0;
        let frame = element(document, "span", "stock-nine-slice")?;
        // A centered button gives absolute children without a local origin a static
        // position at its center. Stock frames always begin at the parent top-left.
        style(
            &frame,
            &format!(
                "position:absolute;left:0;top:0;width:{width}px;height:{height}px;overflow:hidden;pointer-events:none"
            ),
        )?;
        let edge_width = corner_width * 0.05;
        let edge_height = corner_height * 0.05;
        let right = width - corner_width + 1.0;
        let bottom = height - corner_height + 1.0;
        let pieces = [
            (
                [0.0, 0.0, corner_width, corner_height],
                [0.0, 0.0, corner_width, corner_height],
                false,
                false,
            ),
            (
                [0.0, 0.0, corner_width, corner_height],
                [right, 0.0, corner_width, corner_height],
                true,
                false,
            ),
            (
                [0.0, 0.0, corner_width, corner_height],
                [0.0, bottom, corner_width, corner_height],
                false,
                true,
            ),
            (
                [0.0, 0.0, corner_width, corner_height],
                [right, bottom, corner_width, corner_height],
                true,
                true,
            ),
            (
                [corner_width - edge_width, 0.0, edge_width, corner_height],
                [corner_width, 0.0, inner_width, corner_height],
                false,
                false,
            ),
            (
                [corner_width - edge_width, 0.0, edge_width, corner_height],
                [corner_width, bottom, inner_width, corner_height],
                false,
                true,
            ),
            (
                [0.0, corner_height - edge_height, corner_width, edge_height],
                [0.0, corner_height, corner_width, inner_height],
                false,
                false,
            ),
            (
                [0.0, corner_height - edge_height, corner_width, edge_height],
                [right, corner_height, corner_width, inner_height],
                true,
                false,
            ),
            (
                [corner_width - edge_width, corner_height - edge_height, edge_width, edge_height],
                [corner_width, corner_height, inner_width, inner_height],
                false,
                false,
            ),
        ];
        let count = if fill { pieces.len() } else { pieces.len() - 1 };
        for &(source, destination, mirror_x, mirror_y) in &pieces[..count] {
            if destination[2] <= 0.0 || destination[3] <= 0.0 {
                continue;
            }
            let slice = self.slice(document, index, source, destination)?;
            slice
                .style()
                .set_property(
                    "transform",
                    &format!(
                        "scale({}, {})",
                        if mirror_x { -1 } else { 1 },
                        if mirror_y { -1 } else { 1 }
                    ),
                )
                .map_err(|_| "native frame mirroring")?;
            append(&frame, &slice)?;
        }
        Ok(frame)
    }
}

/// Existing extracted art, already composed from the stock records by the Website.
pub fn image(document: &Document, name: &str, class: &str) -> Result<HtmlElement, String> {
    let image = element(document, "img", class)?;
    image
        .set_attribute("src", &format!("assets/stock/menu/{name}.png"))
        .map_err(|_| "UI image source")?;
    image.set_attribute("alt", "").map_err(|_| "decorative image alt")?;
    image.set_attribute("draggable", "false").map_err(|_| "UI image dragging")?;
    Ok(image)
}

/// `planNativeUiMessageFrame`: the existing marble, chains, outer ornaments and
/// inner stock frame shared by confirmations, Party and native resume prompts.
pub fn message_frame(
    document: &Document,
    atlas: &StockAtlas,
    width: f64,
    height: f64,
) -> Result<HtmlElement, String> {
    let frame = element(document, "div", "stock-message-frame")?;
    style(&frame, "position:absolute;inset:0;pointer-events:none")?;
    for (record, x, y, tile_width, tile_height) in [
        (49, 0.0, 0.0, width, height),
        (10, 71.5, -6.5, width - 143.0, 19.0),
        (10, 71.5, height - 12.5, width - 143.0, 19.0),
        (79, -6.5, 76.5, 21.0, height - 153.0),
        (79, width - 14.5, 76.5, 21.0, height - 153.0),
    ] {
        let tile = atlas.tiles(document, record, tile_width, tile_height)?;
        tile.style().set_property("left", &format!("{x}px")).map_err(|_| "message tile x")?;
        tile.style().set_property("top", &format!("{y}px")).map_err(|_| "message tile y")?;
        append(&frame, &tile)?;
    }
    for (record, x, y, scale, rotation) in [
        (107, 29.0, 32.0, 1.0, 0),
        (108, width - 29.0, 32.0, 1.0, 0),
        (109, 29.0, height - 32.0, 1.0, 0),
        (110, width - 29.0, height - 32.0, 1.0, 0),
        (18, width / 2.0, -37.0, 1.0, 90),
        (8, width / 2.0, height + 50.0, 1.0, 0),
        (8, width / 2.0 - 75.0, height + 37.0, 0.75, 0),
        (8, width / 2.0 + 75.0, height + 37.0, 0.75, 0),
    ] {
        let sprite = atlas.sprite(document, record, "")?;
        add_style(
            &sprite,
            &format!(
                "position:absolute;left:{x}px;top:{y}px;transform:translate(-50%,-50%) rotate({rotation}deg) scale({scale})"
            ),
        )?;
        append(&frame, &sprite)?;
    }
    let inner = atlas.nine_slice(document, 17, width - 10.0, height - 10.0)?;
    inner.style().set_property("left", "5px").map_err(|_| "message inner x")?;
    inner.style().set_property("top", "5px").map_err(|_| "message inner y")?;
    append(&frame, &inner)?;
    // The inner frame precedes the header/arrow ornaments in the source plan. It is
    // inset, so the exterior art above remains visible without an extra z-index.
    Ok(frame)
}

/// Stock UI.101/102 body and UI.54 surround, reused by title, pause and modal actions.
pub fn native_button(
    document: &Document,
    label: &str,
    action: &str,
) -> Result<HtmlElement, String> {
    let button = element(document, "button", "native-button")?;
    button.set_attribute("type", "button").map_err(|_| "native button type")?;
    button.set_attribute("data-menu-action", action).map_err(|_| "native button action")?;
    button.set_attribute("aria-label", label).map_err(|_| "native button label")?;
    for (name, class) in [
        ("main-menu-button", "native-button-stone"),
        ("main-menu-button-hover", "native-button-hover"),
        ("main-menu-button-corner", "native-button-corner-left"),
        ("main-menu-button-corner", "native-button-corner-right"),
        ("main-menu-button-rail", "native-button-rail"),
    ] {
        append(&button, &image(document, name, class)?)?;
    }
    let text = element(document, "span", "native-button-label")?;
    let spec = solomon_sim::native_ui_text::NativeUiTextSpec {
        tint: 0xd9ba70,
        ..solomon_sim::native_ui_text::NativeUiTextSpec::new(
            solomon_sim::native_ui_text::NativeUiFontName::Menu,
            label,
            0.0,
            0.0,
        )
    };
    let glyphs = self::text::native_text(document, &spec, "native-button-glyphs")?;
    style(&glyphs, "position:absolute;left:50%;top:calc(50% + 9px);width:0;height:0")?;
    append(&text, &glyphs)?;
    append(&button, &text)?;
    Ok(button)
}

pub fn element(document: &Document, tag: &str, class: &str) -> Result<HtmlElement, String> {
    let element: HtmlElement = document
        .create_element(tag)
        .map_err(|_| format!("cannot create {tag}"))?
        .dyn_into()
        .map_err(|_| format!("{tag} is not an HTML element"))?;
    element.set_class_name(class);
    Ok(element)
}

pub fn append(parent: &HtmlElement, child: &HtmlElement) -> Result<(), String> {
    parent.append_child(child).map(|_| ()).map_err(|_| "cannot attach UI element".to_string())
}

pub fn style(element: &HtmlElement, css: &str) -> Result<(), String> {
    element.set_attribute("style", css).map_err(|_| "cannot set UI geometry".to_string())
}

/// Add placement without discarding a stock sprite's inline logical width/height.
pub(crate) fn add_style(node: &HtmlElement, css: &str) -> Result<(), String> {
    style(node, &format!("{};{css}", node.style().css_text()))
}

/// Multiplicative tint preserves stock shading. Filters are shared by color and retained
/// for the document lifetime, rather than recreated as equipment/selection nodes change.
pub(crate) fn tint(document: &Document, node: &HtmlElement, color: u32) -> Result<(), String> {
    if color == 0xffffff {
        return Ok(());
    }
    let id = format!("stock-tint-{color:06x}");
    if document.get_element_by_id(&id).is_none() {
        let namespace = Some("http://www.w3.org/2000/svg");
        let svg = document.create_element_ns(namespace, "svg").map_err(|_| "tint svg")?;
        svg.set_attribute("width", "0").map_err(|_| "tint width")?;
        svg.set_attribute("height", "0").map_err(|_| "tint height")?;
        svg.set_attribute("style", "position:absolute;pointer-events:none")
            .map_err(|_| "tint style")?;
        let filter = document.create_element_ns(namespace, "filter").map_err(|_| "tint filter")?;
        filter.set_attribute("id", &id).map_err(|_| "tint id")?;
        filter
            .set_attribute("color-interpolation-filters", "sRGB")
            .map_err(|_| "tint color space")?;
        let matrix =
            document.create_element_ns(namespace, "feColorMatrix").map_err(|_| "tint matrix")?;
        let red = f64::from((color >> 16) & 255) / 255.0;
        let green = f64::from((color >> 8) & 255) / 255.0;
        let blue = f64::from(color & 255) / 255.0;
        matrix.set_attribute("type", "matrix").map_err(|_| "tint matrix type")?;
        matrix
            .set_attribute(
                "values",
                &format!("{red} 0 0 0 0  0 {green} 0 0 0  0 0 {blue} 0 0  0 0 0 1 0"),
            )
            .map_err(|_| "tint matrix values")?;
        filter.append_child(&matrix).map_err(|_| "tint matrix append")?;
        svg.append_child(&filter).map_err(|_| "tint filter append")?;
        document
            .body()
            .ok_or("tint document body")?
            .append_child(&svg)
            .map_err(|_| "tint append")?;
    }
    node.style().set_property("filter", &format!("url('#{id}')")).map_err(|_| "art tint".into())
}
