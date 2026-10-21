//! Dark Cloud's authored stock frames, tabs and controls from
//! `native-dark-cloud-frame.ts`, `native-ui-tabs.ts` and `native-ui-plan.ts`.
//! These helpers paint art only; views retain real data, selection and actions.

use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, NativeUiTextPlacement, NativeUiTextSpec,
    layout_native_ui_text, native_ui_font,
};
use web_sys::{Document, HtmlElement};

use super::text::native_text;
use super::{StockAtlas, add_style, append, element, style};

pub(crate) struct DarkCloudArt {
    ui: StockAtlas,
    controls: StockAtlas,
}

impl DarkCloudArt {
    pub async fn load() -> Result<Self, String> {
        Ok(Self {
            ui: StockAtlas::load("UI").await?,
            controls: StockAtlas::load("ControlPanel").await?,
        })
    }

    pub fn menu_skull(&self, document: &Document) -> Result<HtmlElement, String> {
        self.ui.sprite(document, 42, "")
    }

    pub fn scene(
        &self,
        document: &Document,
        width: f64,
        height: f64,
    ) -> Result<HtmlElement, String> {
        let root = element(document, "div", "dark-cloud-native-scene-art")?;
        for (record, rect) in [
            (30, [0.0, 65.0, width, 108.0]),
            (30, [0.0, height - 100.0, width, 108.0]),
            (33, [-74.0, 0.0, 124.0, height]),
            (33, [width - 60.0, 0.0, 124.0, height]),
        ] {
            self.tile(document, &root, record, rect)?;
        }
        for (record, x, y, anchor, mirror) in [
            (29, 135.0, 83.0, [0.5, 0.5], [true, true]),
            (29, width - 135.0, 83.0, [0.5, 0.5], [false, true]),
            (31, width - 112.0, 18.0, [0.0, 0.0], [false, false]),
            (32, 104.0, 26.5, [0.0, 0.0], [true, false]),
            (32, width + 44.0, height - 126.5, [0.0, 0.0], [true, false]),
            (31, -57.0, height - 135.0, [0.0, 0.0], [false, false]),
        ] {
            append(&root, &self.sprite(document, record, [x, y], anchor, 1.0, mirror)?)?;
        }
        for (left, positions) in [
            (0.0, [[-42.5, 162.5], [-82.5, (height - 8.0) / 2.0]]),
            (width - 55.0, [[width - 152.5, 162.5], [width - 112.5, (height - 8.0) / 2.0]]),
        ] {
            let clip = box_node(document, "", [left, 0.0, 55.0, height])?;
            add_style(&clip, "overflow:hidden")?;
            for [x, y] in positions {
                append(
                    &clip,
                    &self.sprite(document, 20, [x - left, y], [0.0, 0.0], 1.0, [false, false])?,
                )?;
            }
            append(&root, &clip)?;
        }
        append(&root, &element(document, "i", "native-dark-cloud-curtain")?)?;
        let shades = element(document, "div", "native-dark-cloud-frame-shade-box")?;
        for side in ["top", "bottom", "left", "right"] {
            append(
                &shades,
                &element(document, "i", &format!("native-dark-cloud-frame-shade {side}"))?,
            )?;
        }
        append(&root, &shades)?;
        Ok(root)
    }

    pub fn list_frame(&self, document: &Document, parent: &HtmlElement) -> Result<(), String> {
        let root = element(document, "div", "dark-cloud-native-list-art")?;
        append(parent, &root)?;
        // Source useNativeUiElementSize measures this element's layout box.
        // Its mobile CSS zoom shrinks the artwork, so the parent's physical
        // dimensions would apply that scale twice. This borderless, unpadded
        // root's computed dimensions retain the fractional layout coordinates.
        let css = document
            .default_view()
            .ok_or("Cloud list frame window")?
            .get_computed_style(&root)
            .map_err(|_| "Cloud list frame computed style")?
            .ok_or("Cloud list frame computed style")?;
        let width: f64 = css
            .get_property_value("width")
            .map_err(|_| "Cloud list frame width")?
            .trim_end_matches("px")
            .parse()
            .map_err(|_| "Cloud list frame width")?;
        let height: f64 = css
            .get_property_value("height")
            .map_err(|_| "Cloud list frame height")?
            .trim_end_matches("px")
            .parse()
            .map_err(|_| "Cloud list frame height")?;
        for (record, rect) in [
            (10, [-5.0, -14.0, width + 10.0, 19.0]),
            (10, [-5.0, height - 3.0, width + 10.0, 19.0]),
            (79, [-15.0, -2.0, 21.0, height + 4.0]),
            (79, [width - 2.0, -2.0, 21.0, height + 4.0]),
        ] {
            self.tile(document, &root, record, rect)?;
        }
        for (record, x, y) in [
            (107, 22.0, 25.0),
            (108, width - 22.0, 25.0),
            (109, 22.0, height - 25.0),
            (110, width - 22.0, height - 25.0),
        ] {
            append(
                &root,
                &self.sprite(document, record, [x, y], [0.5, 0.5], 1.0, [false, false])?,
            )?;
        }
        let back = box_node(document, "", [0.0, 0.0, width, height])?;
        add_style(&back, "background:#000")?;
        append(&root, &back)?;
        let clip = box_node(document, "", [20.0, 20.0, width - 40.0, height - 40.0])?;
        add_style(&clip, "overflow:hidden")?;
        self.tile(document, &clip, 49, [-20.0, -20.0, width, height])?;
        let shade = box_node(document, "", [-20.0, -20.0, width, 85.0])?;
        add_style(&shade, "background:#000;opacity:.5")?;
        append(&clip, &shade)?;
        append(&root, &clip)?;
        append(&root, &self.ui.nine_slice(document, 17, width, height)?)?;
        Ok(())
    }

    pub fn panel(
        &self,
        document: &Document,
        width: f64,
        height: f64,
    ) -> Result<HtmlElement, String> {
        let root = element(document, "div", "dark-cloud-native-panel-art")?;
        let shadow = box_node(document, "", [0.0, 0.0, width + 40.0, height + 40.0])?;
        add_style(&shadow, "background:#000")?;
        append(&root, &shadow)?;
        let shadow_frame = self.ui.nine_slice(document, 17, width + 40.0, height + 40.0)?;
        add_style(&shadow_frame, "filter:brightness(0)")?;
        append(&root, &shadow_frame)?;
        self.tile(document, &root, 49, [0.0, 0.0, width, height])?;
        let frame = self.ui.nine_slice(document, 17, width + 40.0, height + 40.0)?;
        add_style(&frame, "left:-20px;top:-20px")?;
        append(&root, &frame)?;
        for (x, mirrored) in [(-65.0, false), (width + 65.0, true)] {
            append(
                &root,
                &self.sprite(
                    document,
                    18,
                    [x, height / 2.0],
                    [0.5, 0.5],
                    1.0,
                    [mirrored, false],
                )?,
            )?;
        }
        Ok(root)
    }

    pub fn tab(
        &self,
        document: &Document,
        label: &str,
        width: f64,
        height: f64,
        scale: f64,
        selected: bool,
    ) -> Result<HtmlElement, String> {
        let root = box_node(document, "", [0.0, 0.0, width, height])?;
        let chrome_top = (height - 69.0 * scale).max(0.0);
        let top = chrome_top + if selected { 0.0 } else { 8.0 * scale };
        let bracket_height = if selected { 65.0 } else { 51.0 } * scale;
        let bracket_width = 34.0 * scale;
        let source_height = if selected { 65.0 } else { 51.0 };
        if width > 2.0 * bracket_width {
            append(
                &root,
                &self.ui.slice(
                    document,
                    13,
                    [34.0 * 0.95, 0.0, 34.0 * 0.05, source_height],
                    [bracket_width, top, width - bracket_width * 2.0, bracket_height],
                )?,
            )?;
        }
        append(
            &root,
            &self.ui.slice(
                document,
                13,
                [0.0, 0.0, 34.0, source_height],
                [0.0, top, bracket_width, bracket_height],
            )?,
        )?;
        let right = self.ui.slice(
            document,
            13,
            [0.0, 0.0, 34.0, source_height],
            [width - bracket_width, top, bracket_width, bracket_height],
        )?;
        add_style(&right, "transform:scaleX(-1)")?;
        append(&root, &right)?;
        append(
            &root,
            &native_text(
                document,
                &NativeUiTextSpec {
                    scale,
                    tint: 0xd9ba70,
                    ..NativeUiTextSpec::new(
                        NativeUiFontName::Menu,
                        label,
                        width / 2.0,
                        chrome_top + (45.0 + if selected { 0.0 } else { 8.0 }) * scale,
                    )
                },
                "",
            )?,
        )?;
        Ok(root)
    }

    pub fn control_panel(
        &self,
        document: &Document,
        width: f64,
        height: f64,
        field: bool,
    ) -> Result<HtmlElement, String> {
        self.controls.frame(
            document,
            if field { 5 } else { 3 },
            width,
            height,
            if field { 3.0 } else { 6.0 },
        )
    }

    /// Semantic buttons keep their listeners while only measured artwork is replaced.
    pub fn button(
        &self,
        document: &Document,
        button: &HtmlElement,
        label: &str,
        width: f64,
        height: f64,
        scale: f64,
    ) -> Result<(), String> {
        button.set_text_content(None);
        let chrome = box_node(document, "cloud-button-chrome", [0.0, 0.0, width, height])?;
        for (record, class) in [(101, "cloud-button-idle"), (102, "cloud-button-pressed")] {
            let source = self.ui.record(record)?;
            let image = self.ui.slice(
                document,
                record,
                [0.0, 0.0, f64::from(source.width), f64::from(source.height)],
                [0.0, 0.0, width, height],
            )?;
            image.set_class_name(class);
            append(&chrome, &image)?;
        }
        let end_width = 70.0 * scale;
        let end_height = 85.0 * scale;
        let surround = 6.0 * scale;
        append(
            &chrome,
            &self.sprite(
                document,
                54,
                [-surround, -surround],
                [0.0, 0.0],
                scale,
                [false, false],
            )?,
        )?;
        append(
            &chrome,
            &self.sprite(
                document,
                54,
                [width + surround, -surround],
                [0.0, 0.0],
                scale,
                [true, false],
            )?,
        )?;
        let connector = width + surround * 2.0 - end_width * 2.0;
        if connector > 0.0 {
            let source = self.ui.record(54)?;
            append(
                &chrome,
                &self.ui.slice(
                    document,
                    54,
                    [
                        f64::from(source.width) * 0.95,
                        0.0,
                        f64::from(source.width) * 0.05,
                        f64::from(source.height),
                    ],
                    [end_width - surround, -surround, connector, end_height],
                )?,
            )?;
        }
        append(
            &chrome,
            &native_text(
                document,
                &NativeUiTextSpec {
                    scale,
                    tint: 0xd9ba70,
                    ..NativeUiTextSpec::new(
                        NativeUiFontName::Menu,
                        label,
                        width / 2.0,
                        height / 2.0 + 9.0 * scale,
                    )
                },
                "cloud-button-label",
            )?,
        )?;
        append(button, &chrome)?;
        let overlay =
            box_node(document, "cloud-button-disabled-overlay", [0.0, 0.0, width, height])?;
        add_style(&overlay, "background:#808080;opacity:.25")?;
        append(button, &overlay)?;
        button
            .style()
            .set_property("--cloud-button-press", &format!("{}px", 6.0 * scale))
            .map_err(|_| "stock button press offset")?;
        Ok(())
    }

    pub fn tool_button(
        &self,
        document: &Document,
        button: &HtmlElement,
        icon: Option<usize>,
        label: &str,
        width: f64,
        height: f64,
    ) -> Result<(), String> {
        let scale = height / 52.0;
        button.set_text_content(None);
        let face = box_node(document, "cloud-tool-face", [0.0, 0.0, width, height])?;
        add_style(&face, "overflow:hidden")?;
        for (record, class) in [(103, "cloud-button-idle"), (104, "cloud-button-pressed")] {
            let body = self.sprite(
                document,
                record,
                [width / 2.0, height / 2.0],
                [0.5, 0.5],
                scale,
                [false, false],
            )?;
            body.set_class_name(class);
            append(&face, &body)?;
        }
        let content = if let Some(icon) = icon {
            self.sprite(
                document,
                icon,
                [width / 2.0, height / 2.0],
                [0.5, 0.5],
                scale,
                [false, false],
            )?
        } else {
            native_text(
                document,
                &NativeUiTextSpec {
                    scale,
                    tint: 0xd9ba70,
                    ..NativeUiTextSpec::new(
                        NativeUiFontName::Menu,
                        &label.to_lowercase(),
                        width / 2.0,
                        height / 2.0 + 8.0 * scale,
                    )
                },
                "",
            )?
        };
        content.set_class_name("cloud-tool-content");
        append(&face, &content)?;
        let overlay =
            box_node(document, "cloud-button-disabled-overlay", [0.0, 0.0, width, height])?;
        add_style(&overlay, "background:#808080;opacity:.25")?;
        append(&face, &overlay)?;
        append(button, &face)?;
        append(
            button,
            &self.sprite(
                document,
                53,
                [-6.0 * scale, -6.0 * scale],
                [0.0, 0.0],
                scale,
                [false, false],
            )?,
        )?;
        append(
            button,
            &self.sprite(
                document,
                53,
                [width + 6.0 * scale, -6.0 * scale],
                [0.0, 0.0],
                scale,
                [true, false],
            )?,
        )?;
        let source = self.ui.record(53)?;
        append(
            button,
            &self.ui.slice(
                document,
                53,
                [
                    f64::from(source.width) * 0.95,
                    0.0,
                    f64::from(source.width) * 0.05,
                    f64::from(source.height),
                ],
                [21.0 * scale, -6.0 * scale, width - 42.0 * scale, 62.0 * scale],
            )?,
        )?;
        button
            .style()
            .set_property("--cloud-tool-press", &format!("{}px", 4.0 * scale))
            .map_err(|_| "tool button press offset")?;
        Ok(())
    }

    pub fn stone_button(
        &self,
        document: &Document,
        button: &HtmlElement,
        label: &str,
        width: f64,
        height: f64,
    ) -> Result<(), String> {
        button.set_text_content(None);
        for (record, class) in [(105, "cloud-button-idle"), (106, "cloud-button-pressed")] {
            let image = self.ui.slice(
                document,
                record,
                [0.0, 0.0, 141.0, 41.0],
                [0.0, 0.0, width, height],
            )?;
            image.set_class_name(class);
            append(button, &image)?;
        }
        append(
            button,
            &native_text(
                document,
                &NativeUiTextSpec {
                    scale: 1.15,
                    tint: 0xf2f0dc,
                    ..NativeUiTextSpec::new(
                        NativeUiFontName::ControlPanel,
                        label,
                        width / 2.0,
                        height / 2.0 + 4.0,
                    )
                },
                "cloud-stone-label",
            )?,
        )?;
        let overlay =
            box_node(document, "cloud-stone-disabled-overlay", [0.0, 0.0, width, height])?;
        add_style(&overlay, "background:#202020;opacity:.25")?;
        append(button, &overlay)
    }

    fn tile(
        &self,
        document: &Document,
        parent: &HtmlElement,
        record: usize,
        rect: [f64; 4],
    ) -> Result<(), String> {
        let [left, top, width, height] = rect;
        let node = self.ui.tiles(document, record, width, height)?;
        add_style(&node, &format!("left:{left}px;top:{top}px"))?;
        append(parent, &node)
    }

    fn sprite(
        &self,
        document: &Document,
        record: usize,
        position: [f64; 2],
        anchor: [f64; 2],
        scale: f64,
        mirror: [bool; 2],
    ) -> Result<HtmlElement, String> {
        let definition = self.ui.record(record)?;
        let width = f64::from(definition.logical_width) * scale;
        let height = f64::from(definition.logical_height) * scale;
        let root = box_node(
            document,
            "",
            [position[0] - anchor[0] * width, position[1] - anchor[1] * height, width, height],
        )?;
        add_style(
            &root,
            &format!(
                "transform:scale({},{});transform-origin:{}% {}%",
                if mirror[0] { -1 } else { 1 },
                if mirror[1] { -1 } else { 1 },
                anchor[0] * 100.0,
                anchor[1] * 100.0
            ),
        )?;
        let image = self.ui.sprite(document, record, "")?;
        add_style(&image, &format!("transform:scale({scale});transform-origin:top left"))?;
        append(&root, &image)?;
        Ok(root)
    }
}

pub(crate) fn cloud_text(
    document: &Document,
    text: &str,
    font: NativeUiFontName,
    scale: f64,
    tint: u32,
    content: bool,
) -> Result<HtmlElement, String> {
    let spec = NativeUiTextSpec {
        align: NativeUiTextAlign::Left,
        placement: NativeUiTextPlacement::Box,
        scale,
        tint,
        ..NativeUiTextSpec::new(font, text, 0.0, 0.0)
    };
    if content && !layout_native_ui_text(&spec).unsupported_code_points.is_empty() {
        let node = element(document, "span", "native-ui-content-text")?;
        node.set_attribute("data-native-ui-content-text", "")
            .map_err(|_| "external content text marker")?;
        node.set_text_content(Some(text));
        style(
            &node,
            &format!("color:#{tint:06x};font-size:{}px", native_ui_font(font).metrics[0] * scale),
        )?;
        return Ok(node);
    }
    let root = element(document, "span", "")?;
    let accessible = element(document, "span", "native-ui-sr-only")?;
    accessible.set_text_content(Some(text));
    append(&root, &accessible)?;
    append(&root, &native_text(document, &spec, "")?)?;
    Ok(root)
}

fn box_node(document: &Document, class: &str, rect: [f64; 4]) -> Result<HtmlElement, String> {
    let node = element(document, "span", class)?;
    style(
        &node,
        &format!(
            "display:block;position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;pointer-events:none",
            rect[0], rect[1], rect[2], rect[3]
        ),
    )?;
    Ok(node)
}
