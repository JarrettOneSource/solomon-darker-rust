//! Native browser controls and authored editor styles from `MobileUiEditor.tsx`.
//! The editor intentionally uses the source HTML display font; gameplay stock text uses
//! the shared bitmap painter elsewhere.

use solomon_sim::mobile_ui::*;
use solomon_sim::player_panels::native_tooltip_fixed;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement, HtmlSelectElement};

use super::{MobileUiEditor, append, element};

impl MobileUiEditor {
    pub(super) fn build_controls(&self) -> Result<(), String> {
        if self.fullscreen {
            self.dock.set_text_content(None);
            let heading = element(&self.document, "div", "mobile-ui-editor-dock-heading")?;
            let handle = self.button(&heading, "⠿", "dock-handle", "Move editor actions")?;
            handle
                .remove_attribute("data-mobile-editor-action")
                .map_err(|_| "mobile actions handle")?;
            handle.set_class_name("mobile-ui-editor-dock-handle");
            self.text(&heading, "strong", "Mobile controls")?;
            let save = element(&self.document, "button", "mobile-ui-editor-dock-save")?;
            for (name, value) in [
                ("type", "button"),
                ("data-mobile-ui-save", ""),
                ("data-menu-action", "settings-back"),
            ] {
                save.set_attribute(name, value).map_err(|_| "mobile editor save action")?;
            }
            save.set_text_content(Some("SAVE"));
            append(&heading, &save)?;
            append(&self.dock, &heading)?;
            let actions = element(&self.document, "div", "mobile-ui-editor-dock-actions")?;
            self.history_buttons(&actions)?;
            self.button(&actions, "GRID ON", "grid", "GRID ON")?;
            let tools = self.button(&actions, "ADJUST +", "tools", "ADJUST +")?;
            tools
                .set_attribute("aria-controls", "mobile-ui-adjustments")
                .map_err(|_| "mobile inspector controls")?;
            append(&self.dock, &actions)?;
            let hint = element(&self.document, "p", "mobile-ui-editor-hint")?;
            hint.set_text_content(Some("Drag to move. Pinch to resize."));
            append(&self.dock, &hint)?;
            self.inspector
                .set_attribute("id", "mobile-ui-adjustments")
                .map_err(|_| "mobile inspector identity")?;
            self.picker(&self.inspector, "Control", "mobile-ui-editor-control-picker")?;
            self.adjustments()?;
            self.button(&self.inspector, "RESET LAYOUT", "reset-layout", "RESET LAYOUT")?;
            self.text(
                &self.inspector,
                "small",
                "Changes apply when you save. Undo also restores a reset.",
            )?;
            append(&self.dock, &self.inspector)?;
        } else {
            self.toolbar
                .set_attribute("aria-label", "Mobile UI editor tools")
                .map_err(|_| "mobile toolbar label")?;
            self.picker(&self.toolbar, "CONTROL", "")?;
            self.history_buttons(&self.toolbar)?;
            self.button(&self.toolbar, "GRID ON", "grid", "GRID ON")?;
            let zoom = element(&self.document, "div", "mobile-ui-editor-zoom-controls")?;
            zoom.set_attribute("aria-label", "Page zoom").map_err(|_| "mobile zoom label")?;
            self.button(&zoom, "−", "zoom-out", "Zoom out")?;
            let level = self.text(&zoom, "output", "100%")?;
            level
                .set_attribute("aria-label", "Zoom level")
                .map_err(|_| "mobile zoom value label")?;
            self.button(&zoom, "+", "zoom-in", "Zoom in")?;
            self.button(&zoom, "FIT", "fit", "FIT")?;
            append(&self.toolbar, &zoom)?;
            self.button(&self.toolbar, "RESET LAYOUT", "reset-layout", "RESET LAYOUT")?;
            self.adjustments()?;
        }
        let error = element(&self.document, "p", "mobile-ui-editor-error")?;
        error.set_attribute("role", "alert").map_err(|_| "mobile editor save error")?;
        error.set_hidden(true);
        append(&self.dock, &error)?;
        Ok(())
    }

    fn picker(&self, parent: &HtmlElement, label: &str, class: &str) -> Result<(), String> {
        let row = element(&self.document, "label", class)?;
        self.text(&row, "span", label)?;
        let select = element(&self.document, "select", "")?;
        select
            .set_attribute("aria-label", "Selected mobile UI element")
            .map_err(|_| "mobile control selector")?;
        select
            .set_attribute("data-mobile-editor-select", "")
            .map_err(|_| "mobile selector target")?;
        for id in MobileUiElementId::ALL {
            let option = element(&self.document, "option", "")?;
            option.set_attribute("value", id.as_str()).map_err(|_| "mobile control option")?;
            option.set_text_content(Some(id.label()));
            append(&select, &option)?;
        }
        append(&row, &select)?;
        append(parent, &row)
    }

    fn history_buttons(&self, parent: &HtmlElement) -> Result<(), String> {
        self.button(parent, "UNDO", "undo", "UNDO")?;
        self.button(parent, "REDO", "redo", "REDO")?;
        Ok(())
    }

    fn adjustments(&self) -> Result<(), String> {
        let root = element(&self.document, "div", "mobile-ui-editor-adjustments")?;
        let position = element(&self.document, "div", "mobile-ui-editor-position")?;
        self.text(&position, "span", "Move")?;
        for (label, action, description) in [
            ("←", "left", "Move control left"),
            ("↑", "up", "Move control up"),
            ("↓", "down", "Move control down"),
            ("→", "right", "Move control right"),
        ] {
            self.button(&position, label, action, description)?;
        }
        append(&root, &position)?;
        let size = element(&self.document, "div", "mobile-ui-editor-size")?;
        let label = self.text(&size, "label", "Size")?;
        label.set_attribute("for", "mobile-ui-control-size").map_err(|_| "mobile size label")?;
        self.text(&label, "output", "100%")?;
        self.button(&size, "−", "smaller", "Make control smaller")?;
        let range = element(&self.document, "input", "")?;
        for (name, value) in [
            ("id", "mobile-ui-control-size"),
            ("type", "range"),
            ("min", "40"),
            ("max", "300"),
            ("step", "5"),
            ("value", "100"),
            ("data-mobile-editor-scale", ""),
        ] {
            range.set_attribute(name, value).map_err(|_| "mobile size control")?;
        }
        append(&size, &range)?;
        self.button(&size, "+", "larger", "Make control larger")?;
        append(&root, &size)?;
        let rotation = element(&self.document, "div", "mobile-ui-editor-rotation")?;
        let label = self.text(&rotation, "span", "Rotation")?;
        self.text(&label, "output", "0°")?;
        for (label, action, description) in [
            ("−15°", "rotate-left", "Rotate control left"),
            ("0°", "straighten", "Straighten control"),
            ("+15°", "rotate-right", "Rotate control right"),
        ] {
            self.button(&rotation, label, action, description)?;
        }
        append(&root, &rotation)?;
        let reset =
            self.button(&root, "RESET THIS CONTROL", "reset-control", "RESET THIS CONTROL")?;
        reset.set_class_name("mobile-ui-editor-reset-control");
        append(&self.inspector, &root)
    }

    pub(super) fn build_elements(&mut self) -> Result<(), String> {
        for id in MobileUiElementId::ALL {
            let node = element(&self.document, "div", "mobile-ui-editor-element")?;
            node.set_attribute("role", "button").map_err(|_| "mobile editable element role")?;
            node.set_attribute("data-mobile-ui-editor-element", id.as_str())
                .map_err(|_| "mobile editable element identity")?;
            let preview = element(&self.document, "span", "mobile-ui-editor-preview")?;
            preview
                .set_attribute("aria-hidden", "true")
                .map_err(|_| "mobile preview decoration")?;
            preview
                .set_attribute("data-preview-element", id.as_str())
                .map_err(|_| "mobile preview identity")?;
            append(&node, &preview)?;
            for handle in MobileUiResizeHandle::ALL {
                let button = element(&self.document, "button", "mobile-ui-editor-resize-node")?;
                button.set_attribute("type", "button").map_err(|_| "mobile resize button")?;
                button
                    .set_attribute("data-resize-handle", handle.as_str())
                    .map_err(|_| "mobile resize identity")?;
                button
                    .set_attribute(
                        "aria-label",
                        &format!("Resize {} from {}", id.label(), handle.as_str()),
                    )
                    .map_err(|_| "mobile resize label")?;
                append(&node, &button)?;
            }
            let rotation = element(&self.document, "button", "mobile-ui-editor-rotate-handle")?;
            rotation.set_attribute("type", "button").map_err(|_| "mobile rotation button")?;
            rotation
                .set_attribute("aria-label", &format!("Rotate {}", id.label()))
                .map_err(|_| "mobile rotation label")?;
            append(&node, &rotation)?;
            append(&self.page_node, &node)?;
            self.elements.push(node);
            self.preview_nodes.push(preview);
        }
        Ok(())
    }

    pub(super) fn paint(&mut self) -> Result<(), String> {
        for (name, value) in [
            ("data-grid-snap", self.grid.to_string()),
            ("data-restoring-default", self.restoring_default.to_string()),
            ("data-selected-element", self.selected.as_str().to_owned()),
        ] {
            self.root.set_attribute(name, &value).map_err(|_| "mobile editor state")?;
        }
        self.page_node
            .set_attribute("data-grid-visible", if self.grid { "true" } else { "false" })
            .map_err(|_| "mobile grid visibility")?;
        self.page_node
            .set_attribute(
                "aria-label",
                &format!("Mobile HUD layout page, {} by {}", self.page.width, self.page.height),
            )
            .map_err(|_| "mobile page dimensions")?;
        self.page_node.style().set_css_text(&format!(
            "--mobile-ui-editor-grid:16px;width:{}px;height:{}px;transform:scale({})",
            self.page.width, self.page.height, self.zoom
        ));
        let inset = if self.fullscreen { 0.0 } else { 48.0 };
        self.shell.style().set_css_text(&format!(
            "width:{}px;height:{}px",
            self.page.width * self.zoom + inset,
            self.page.height * self.zoom + inset
        ));
        self.guide_layer.set_text_content(None);
        for guide in &self.guides {
            let node = element(&self.document, "span", "mobile-ui-editor-snap-guide")?;
            node.set_attribute("aria-hidden", "true")
                .map_err(|_| "mobile snap guide decoration")?;
            node.set_attribute("data-snap-guide-axis", guide.axis.as_str())
                .map_err(|_| "mobile guide axis")?;
            node.set_attribute("data-snap-guide-kind", guide.kind.as_str())
                .map_err(|_| "mobile guide priority")?;
            node.style()
                .set_property(
                    if guide.axis == MobileUiSnapAxis::X { "left" } else { "top" },
                    &format!("{}px", guide.position),
                )
                .map_err(|_| "mobile guide position")?;
            append(&self.guide_layer, &node)?;
        }
        for id in MobileUiElementId::ALL {
            let node = &self.elements[id as usize];
            let transform = self.layout.get(id);
            let size = self.geometry.sizes[id as usize];
            let active = self.selected == id;
            node.set_attribute(
                "aria-label",
                &format!(
                    "{}, x {} percent, y {} percent, scale {}, rotation {} degrees",
                    id.label(),
                    native_tooltip_fixed(transform.x, 1),
                    native_tooltip_fixed(transform.y, 1),
                    native_tooltip_fixed(transform.scale, 2),
                    native_tooltip_fixed(transform.rotation, 0)
                ),
            )
            .map_err(|_| "mobile control placement label")?;
            for (name, value) in [
                ("aria-pressed", active.to_string()),
                ("data-selected", active.to_string()),
                ("tabindex", if active { "0" } else { "-1" }.to_owned()),
            ] {
                node.set_attribute(name, &value).map_err(|_| "mobile control selection")?;
            }
            node.style().set_css_text(&format!("--mobile-ui-element-scale:{};width:{}px;height:{}px;left:{}%;top:{}%;transform:translate(-50%,-50%) rotate({}deg) scale({})",
                transform.scale,size.width,size.height,transform.x,transform.y,transform.rotation,transform.scale));
            let handles =
                node.query_selector_all("button").map_err(|_| "mobile adjustment handles")?;
            for index in 0..handles.length() {
                let Some(node) = handles.item(index) else { continue };
                node.dyn_into::<HtmlElement>()
                    .map_err(|_| "mobile adjustment button")?
                    .set_hidden(!active);
            }
        }
        self.paint_controls()?;
        if let Some(scroll) = self.pending_scroll.take() {
            // The shell and page must have their new dimensions before native scrolling
            // applies, matching the source's requestAnimationFrame zoom correction.
            self.set_scroll_position(scroll)?;
        }
        if self.dirty_preview {
            self.paint_previews()?;
            self.dirty_preview = false;
        }
        Ok(())
    }

    fn paint_controls(&self) -> Result<(), String> {
        for (action, disabled) in [
            ("undo", self.past.is_empty()),
            ("redo", self.future.is_empty()),
            ("smaller", self.layout.get(self.selected).scale <= MOBILE_UI_SCALE_MIN),
            ("larger", self.layout.get(self.selected).scale >= MOBILE_UI_SCALE_MAX),
            ("straighten", self.layout.get(self.selected).rotation == 0.0),
            ("reset-layout", self.restoring_default),
            ("zoom-out", self.zoom <= MOBILE_UI_PAGE_ZOOM_MIN),
            ("zoom-in", self.zoom >= MOBILE_UI_PAGE_ZOOM_MAX),
        ] {
            if let Some(button) = self.find(&format!("[data-mobile-editor-action='{action}']"))? {
                button
                    .toggle_attribute_with_force("disabled", disabled)
                    .map_err(|_| "mobile editor control availability")?;
            }
        }
        if let Some(grid) = self.find("[data-mobile-editor-action='grid']")? {
            let text = if self.grid { "GRID ON" } else { "GRID OFF" };
            grid.set_text_content(Some(text));
            grid.set_attribute("aria-label", text).map_err(|_| "mobile grid action label")?;
            grid.set_attribute("aria-pressed", if self.grid { "true" } else { "false" })
                .map_err(|_| "mobile grid state")?;
        }
        if let Some(tools) = self.find("[data-mobile-editor-action='tools']")? {
            tools.set_text_content(Some(if self.tools_open { "ADJUST −" } else { "ADJUST +" }));
            tools
                .set_attribute("aria-expanded", if self.tools_open { "true" } else { "false" })
                .map_err(|_| "mobile inspector state")?;
        }
        if self.fullscreen {
            self.inspector.set_hidden(!self.tools_open);
        }
        if let Some(hint) = self.find(".mobile-ui-editor-hint")? {
            hint.set_hidden(self.tools_open);
        }
        if let Some(level) = self.find(".mobile-ui-editor-zoom-controls output")? {
            level.set_text_content(Some(&format!("{}%", round(self.zoom * 100.0))));
        }
        if let Some(select) = self.find("[data-mobile-editor-select]")? {
            select
                .dyn_into::<HtmlSelectElement>()
                .map_err(|_| "mobile element picker")?
                .set_value(self.selected.as_str());
        }
        let transform = self.layout.get(self.selected);
        if let Some(range) = self.find("[data-mobile-editor-scale]")? {
            range
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "mobile size slider")?
                .set_value(&round(transform.scale * 100.0));
        }
        if let Some(output) = self.find(".mobile-ui-editor-size output")? {
            output.set_text_content(Some(&format!("{}%", round(transform.scale * 100.0))));
        }
        if let Some(output) = self.find(".mobile-ui-editor-rotation output")? {
            output.set_text_content(Some(&format!("{}°", round(transform.rotation))));
        }
        if let Some(adjustments) = self.find(".mobile-ui-editor-adjustments")? {
            adjustments
                .set_attribute("aria-label", &format!("Adjust {}", self.selected.label()))
                .map_err(|_| "mobile inspector selected label")?;
        }
        if !self.fullscreen {
            self.status.set_text_content(None);
            self.text(&self.status, "strong", self.selected.label())?;
            for value in [
                format!("X {}%", native_tooltip_fixed(transform.x, 1)),
                format!("Y {}%", native_tooltip_fixed(transform.y, 1)),
                format!("SIZE {}%", round(transform.scale * 100.0)),
                format!("ROTATE {}°", round(transform.rotation)),
            ] {
                self.text(&self.status, "span", &value)?;
            }
            self.text(
                &self.status,
                "small",
                "GRID snaps nearby edges and centres. Drag to move; pinch or use a node to resize.",
            )?;
        }
        if let Some(error) = self.find(".mobile-ui-editor-error")? {
            error.set_hidden(self.error.is_none());
            error.set_text_content(self.error.as_deref());
        }
        Ok(())
    }

    fn find(&self, selector: &str) -> Result<Option<HtmlElement>, String> {
        self.root
            .query_selector(selector)
            .map_err(|_| "mobile editor control selector".to_owned())?
            .map(|node| {
                node.dyn_into::<HtmlElement>()
                    .map_err(|_| "mobile editor control element".to_owned())
            })
            .transpose()
    }

    fn button(
        &self,
        parent: &HtmlElement,
        label: &str,
        action: &str,
        description: &str,
    ) -> Result<HtmlElement, String> {
        let button = element(&self.document, "button", "")?;
        for (name, value) in
            [("type", "button"), ("data-mobile-editor-action", action), ("aria-label", description)]
        {
            button.set_attribute(name, value).map_err(|_| "mobile editor action")?;
        }
        button.set_text_content(Some(label));
        append(parent, &button)?;
        Ok(button)
    }

    fn text(&self, parent: &HtmlElement, tag: &str, text: &str) -> Result<HtmlElement, String> {
        let node = element(&self.document, tag, "")?;
        node.set_text_content(Some(text));
        append(parent, &node)?;
        Ok(node)
    }
}

fn round(value: f64) -> String {
    (value + 0.5).floor().to_string()
}
