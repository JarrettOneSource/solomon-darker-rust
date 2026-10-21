//! Stock CPanel widgets from Website `NativeSettingsPresentation.tsx` and
//! `native-settings-contract.ts`. Settings and Dark Cloud reuse the same records.

use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, NativeUiTextPlacement, NativeUiTextSpec,
};
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlElement};

use super::text::{native_text, paint_text_layout};
use super::{StockAtlas, append, element};

pub(crate) struct NativeSettingsArt {
    pub control_panel: StockAtlas,
    ui: StockAtlas,
}

impl NativeSettingsArt {
    pub async fn load() -> Result<Self, String> {
        Ok(Self {
            control_panel: StockAtlas::load("ControlPanel").await?,
            ui: StockAtlas::load("UI").await?,
        })
    }

    pub fn panel(&self, document: &Document) -> Result<HtmlElement, String> {
        let frame = element(document, "div", "game-settings-native-art native-settings-panel-art")?;
        for corner in ["top-left", "top-right", "bottom-left", "bottom-right"] {
            let sprite =
                self.ui.sprite(document, 17, &format!("game-settings-frame-corner {corner}"))?;
            sprite
                .style()
                .set_property("position", "absolute")
                .map_err(|_| "settings corner position")?;
            append(&frame, &sprite)?;
        }
        for side in ["left", "right"] {
            let sprite =
                self.ui.sprite(document, 18, &format!("game-settings-frame-flourish {side}"))?;
            sprite
                .style()
                .set_property("position", "absolute")
                .map_err(|_| "settings flourish position")?;
            append(&frame, &sprite)?;
        }
        Ok(frame)
    }

    pub fn row(&self, document: &Document, parent: &HtmlElement) -> Result<(), String> {
        let plate = self.control_panel.slice(
            document,
            3,
            [0.0, 0.0, 315.0, 44.0],
            [0.0, 0.0, 560.0, 44.0],
        )?;
        plate.set_class_name("game-settings-row-plate");
        append(parent, &plate)
    }

    pub fn arrow(&self, document: &Document, parent: &HtmlElement) -> Result<(), String> {
        append(parent, &self.control_panel.sprite(document, 0, "game-settings-action-arrow")?)
    }

    pub fn binding_plate(&self, document: &Document, parent: &HtmlElement) -> Result<(), String> {
        let plate = self.control_panel.slice(
            document,
            5,
            [0.0, 0.0, 159.0, 30.0],
            [0.0, 0.0, 200.0, 30.0],
        )?;
        plate.set_class_name("game-settings-binding-plate");
        append(parent, &plate)
    }

    pub fn toggle(&self, document: &Document, checked: bool) -> Result<HtmlElement, String> {
        self.control_panel.sprite(
            document,
            if checked { 9 } else { 8 },
            "game-settings-native-toggle",
        )
    }

    pub fn stone_button(
        &self,
        document: &Document,
        label: &str,
        action: &str,
    ) -> Result<HtmlElement, String> {
        let button = element(document, "button", "native-stone-button")?;
        button.set_attribute("type", "button").map_err(|_| "stone action type")?;
        button.set_attribute("aria-label", label).map_err(|_| "stone action label")?;
        button.set_attribute("data-menu-action", action).map_err(|_| "stone action identifier")?;
        for (record, class) in [(105, "native-stone-idle"), (106, "native-stone-pressed")] {
            let image = self.ui.slice(
                document,
                record,
                [0.0, 0.0, 141.0, 41.0],
                [0.0, 0.0, 300.0, 41.0],
            )?;
            image.set_class_name(class);
            append(&button, &image)?;
        }
        append(
            &button,
            &native_text(
                document,
                &NativeUiTextSpec {
                    scale: 1.15,
                    tint: 0xf2f0dc,
                    ..NativeUiTextSpec::new(NativeUiFontName::ControlPanel, label, 150.0, 24.5)
                },
                "native-stone-label",
            )?,
        )?;
        Ok(button)
    }

    pub fn set_stone_label(
        &self,
        document: &Document,
        button: &HtmlElement,
        label: &str,
    ) -> Result<(), String> {
        let node: HtmlElement = button
            .query_selector(".native-stone-label")
            .map_err(|_| "stone label lookup")?
            .ok_or("stone button has no label")?
            .dyn_into()
            .map_err(|_| "stone label is not HTML")?;
        let layout = solomon_sim::native_ui_text::layout_native_ui_text(&NativeUiTextSpec {
            scale: 1.15,
            tint: 0xf2f0dc,
            ..NativeUiTextSpec::new(NativeUiFontName::ControlPanel, label, 150.0, 24.5)
        });
        paint_text_layout(document, &node, &layout)?;
        button.set_attribute("aria-label", label).map_err(|_| "stone label semantics".to_owned())
    }
}

/// `NativeSettingsText` is a flow box whose ink uses the ControlPanel font.
pub(crate) fn settings_text(
    document: &Document,
    text: &str,
    scale: f64,
    tint: u32,
    alignment: Option<(NativeUiTextAlign, f64)>,
) -> Result<HtmlElement, String> {
    let (align, width) = alignment.unwrap_or((NativeUiTextAlign::Left, 0.0));
    let x = match align {
        NativeUiTextAlign::Left => 0.0,
        NativeUiTextAlign::Center => width / 2.0,
        NativeUiTextAlign::Right => width,
    };
    let node = native_text(
        document,
        &NativeUiTextSpec {
            align,
            placement: NativeUiTextPlacement::Box,
            scale,
            tint,
            ..NativeUiTextSpec::new(NativeUiFontName::ControlPanel, text, x, 0.0)
        },
        "native-settings-text",
    )?;
    if alignment.is_some() {
        node.style()
            .set_property("width", &format!("{width}px"))
            .map_err(|_| "settings text width")?;
    }
    Ok(node)
}
