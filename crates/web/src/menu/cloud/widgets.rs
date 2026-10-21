//! Small DOM counterparts of the source's stock Dark Cloud widgets. All values
//! and actions stay with their owning views; these helpers only assemble art.

use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextPlacement, NativeUiTextSpec};
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlElement};

use crate::ui::cloud::{DarkCloudArt, cloud_text};
use crate::ui::text::native_text;
use crate::ui::{append, element, style};

// Keep the authored text properties beside their semantic DOM wrapper; a second
// builder would obscure these small, private counterparts of the source JSX.
#[allow(clippy::too_many_arguments)]
pub(super) fn text(
    document: &Document,
    parent: &HtmlElement,
    tag: &str,
    class: &str,
    value: &str,
    font: NativeUiFontName,
    scale: f64,
    tint: u32,
    content: bool,
) -> Result<HtmlElement, String> {
    let node = element(document, tag, class)?;
    append(&node, &cloud_text(document, value, font, scale, tint, content)?)?;
    append(parent, &node)?;
    Ok(node)
}

pub(super) fn narrative(
    document: &Document,
    parent: &HtmlElement,
    tag: &str,
    class: &str,
    value: &str,
) -> Result<HtmlElement, String> {
    let node = element(document, tag, class)?;
    node.set_text_content(Some(value));
    append(parent, &node)?;
    Ok(node)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn button(
    document: &Document,
    label: &str,
    action: &str,
    class: &str,
    width: Option<f64>,
    height: f64,
    scale: f64,
    disabled: bool,
) -> Result<HtmlElement, String> {
    let node =
        element(document, "button", &format!("native-ui-button cloud-stock-button {class}"))?;
    node.set_attribute("type", "button").map_err(|_| "Cloud button type")?;
    node.set_attribute("aria-label", label).map_err(|_| "Cloud button label")?;
    node.set_attribute("data-menu-action", action).map_err(|_| "Cloud button action")?;
    node.set_attribute("data-cloud-label", label).map_err(|_| "Cloud button artwork label")?;
    node.set_attribute("data-cloud-scale", &scale.to_string()).map_err(|_| "Cloud button scale")?;
    node.toggle_attribute_with_force("disabled", disabled).map_err(|_| "Cloud button admission")?;
    let width = width.map_or_else(|| "100%".to_owned(), |width| format!("{width}px"));
    style(&node, &format!("position:relative;width:{width};height:{height}px"))?;
    Ok(node)
}

pub(super) fn control_panel(
    document: &Document,
    parent: &HtmlElement,
    class: &str,
    field: bool,
) -> Result<HtmlElement, String> {
    let node = element(document, "div", &format!("native-ui-control-panel {class}"))?;
    node.set_attribute("data-native-ui-control-panel", if field { "field" } else { "inset" })
        .map_err(|_| "Cloud control panel variant")?;
    let art = element(document, "span", "native-ui-control-panel-art")?;
    art.set_attribute("aria-hidden", "true").map_err(|_| "Cloud control panel art")?;
    append(&node, &art)?;
    append(parent, &node)?;
    Ok(node)
}

pub(super) fn control_action(
    document: &Document,
    parent: &HtmlElement,
    label: &str,
    action: &str,
    selected: bool,
    disabled: bool,
) -> Result<HtmlElement, String> {
    let node = element(document, "button", "native-ui-control-action")?;
    node.set_attribute("type", "button").map_err(|_| "Cloud control action type")?;
    node.set_attribute("aria-label", label).map_err(|_| "Cloud control action label")?;
    node.set_attribute("data-menu-action", action).map_err(|_| "Cloud control action")?;
    node.toggle_attribute_with_force("disabled", disabled)
        .map_err(|_| "Cloud control admission")?;
    append(
        &node,
        &cloud_text(
            document,
            label,
            NativeUiFontName::Medium,
            1.0,
            if disabled {
                0x8e7c4d
            } else if selected {
                0xbfffbf
            } else {
                0xd9ba70
            },
            false,
        )?,
    )?;
    append(parent, &node)?;
    Ok(node)
}

/// Source ResizeObserver-driven artwork, measured only when its physical box
/// changes. Inputs, caret/focus and semantic nodes survive these updates.
pub(super) fn paint_art(
    document: &Document,
    parent: &HtmlElement,
    art: &DarkCloudArt,
) -> Result<(), String> {
    let nodes = parent
        .query_selector_all(".cloud-stock-button, .native-ui-control-panel-art, .cloud-tool-button")
        .map_err(|_| "Cloud measured artwork selector")?;
    for index in 0..nodes.length() {
        let node: HtmlElement = nodes
            .item(index)
            .ok_or("Cloud artwork disappeared")?
            .dyn_into()
            .map_err(|_| "Cloud artwork target")?;
        let bounds = node.get_bounding_client_rect();
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            continue;
        }
        let key = format!("{},{}", bounds.width(), bounds.height());
        if node.get_attribute("data-cloud-art-size").as_deref() == Some(key.as_str()) {
            continue;
        }
        if node.class_list().contains("native-ui-control-panel-art") {
            let field = node.parent_element().is_some_and(|parent| {
                parent.get_attribute("data-native-ui-control-panel").as_deref() == Some("field")
            });
            node.set_text_content(None);
            append(&node, &art.control_panel(document, bounds.width(), bounds.height(), field)?)?;
        } else if node.class_list().contains("cloud-tool-button") {
            let icon = node
                .get_attribute("data-cloud-tool-icon")
                .map(|value| value.parse::<usize>().map_err(|_| "Cloud tool icon"))
                .transpose()?;
            let label = node.get_attribute("data-cloud-label").ok_or("Cloud tool has no label")?;
            art.tool_button(document, &node, icon, &label, bounds.width(), bounds.height())?;
        } else {
            let label =
                node.get_attribute("data-cloud-label").ok_or("Cloud button has no label")?;
            let scale: f64 = node
                .get_attribute("data-cloud-scale")
                .ok_or("Cloud button has no scale")?
                .parse()
                .map_err(|_| "Cloud button scale")?;
            if node.has_attribute("data-cloud-stone") {
                art.stone_button(document, &node, &label, bounds.width(), bounds.height())?;
            } else {
                art.button(document, &node, &label, bounds.width(), bounds.height(), scale)?;
            }
        }
        node.set_attribute("data-cloud-art-size", &key)
            .map_err(|_| "Cloud artwork measured size")?;
    }
    Ok(())
}

pub(super) fn baseline(
    document: &Document,
    parent: &HtmlElement,
    value: &str,
    font: NativeUiFontName,
    scale: f64,
    x: f64,
    y: f64,
) -> Result<(), String> {
    append(
        parent,
        &native_text(
            document,
            &NativeUiTextSpec {
                scale,
                tint: 0xd9ba70,
                placement: NativeUiTextPlacement::Baseline,
                ..NativeUiTextSpec::new(font, value, x, y)
            },
            "",
        )?,
    )
}

pub(super) fn focused_action(document: &Document, root: &HtmlElement) -> Option<String> {
    let active = document.active_element()?;
    if !root.contains(Some(active.as_ref())) {
        return None;
    }
    active.get_attribute("data-menu-action")
}

pub(super) fn restore_focus(
    document: &Document,
    root: &HtmlElement,
    action: Option<&str>,
) -> Result<(), String> {
    let Some(action) = action else {
        return Ok(());
    };
    let nodes = root
        .query_selector_all("[data-menu-action]:not([disabled])")
        .map_err(|_| "Cloud retained focus selector")?;
    for index in 0..nodes.length() {
        let node: HtmlElement = nodes
            .item(index)
            .ok_or("Cloud focus target disappeared")?
            .dyn_into()
            .map_err(|_| "Cloud focus target")?;
        if node.get_attribute("data-menu-action").as_deref() == Some(action) {
            if document.active_element().as_ref() != Some(node.as_ref()) {
                node.focus().map_err(|_| "restore Cloud control focus")?;
            }
            break;
        }
    }
    Ok(())
}
