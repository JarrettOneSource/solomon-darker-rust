//! Nonvisual screen-reader announcements for GPU controls and Tutorial narration.

use std::collections::BTreeMap;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, HtmlElement};

use super::WidgetGeometry;

#[derive(Default)]
struct AnnouncementState {
    last_narration: String,
    focused: Option<String>,
    values: BTreeMap<String, (Option<String>, Option<bool>)>,
}

impl AnnouncementState {
    fn messages(
        &mut self,
        narration: &[String],
        widgets: &BTreeMap<String, WidgetGeometry>,
    ) -> Vec<String> {
        let mut messages = Vec::new();
        for text in narration.iter().filter(|text| !text.is_empty()) {
            if *text != self.last_narration {
                messages.push(text.clone());
                self.last_narration.clone_from(text);
            }
        }
        for widget in widgets.values() {
            let state = (widget.value.clone(), widget.selected);
            let changed = self.values.get(&widget.id).is_some_and(|previous| *previous != state);
            let control = matches!(
                widget.role.as_str(),
                "slider" | "checkbox" | "switch" | "spinbutton" | "combobox" | "radio"
            );
            if widget.activated
                || widget.focused && self.focused.as_deref() != Some(&widget.id)
                || changed && (widget.focused || control)
            {
                let mut description = format!("{}, {}", widget.label, widget.role);
                if let Some(selected) = widget.selected {
                    description.push_str(
                        if matches!(widget.role.as_str(), "checkbox" | "switch") {
                            if selected { ", checked" } else { ", unchecked" }
                        } else if selected {
                            ", selected"
                        } else {
                            ", not selected"
                        },
                    );
                } else if widget.role != "password"
                    && let Some(value) = &widget.value
                {
                    description.push_str(", ");
                    description.push_str(if value.is_empty() { "empty" } else { value });
                }
                messages.push(description);
            }
        }
        self.focused =
            widgets.values().find(|widget| widget.focused).map(|widget| widget.id.clone());
        self.values = widgets
            .values()
            .map(|widget| (widget.id.clone(), (widget.value.clone(), widget.selected)))
            .collect();
        messages
    }
}

pub(super) struct Accessibility {
    document: Document,
    native: Option<js_sys::Function>,
    status: Option<HtmlElement>,
    state: AnnouncementState,
}

impl Accessibility {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let native = js_sys::Reflect::get(document.as_ref(), &JsValue::from_str("ariaNotify"))
            .map_err(|error| format!("accessibility notification capability: {error:?}"))?
            .dyn_into::<js_sys::Function>()
            .ok();
        let status = if native.is_none() {
            let status: HtmlElement = document
                .create_element("div")
                .map_err(|error| format!("accessibility announcement adapter: {error:?}"))?
                .dyn_into()
                .map_err(|_| "accessibility announcement adapter is not HTML")?;
            for (name, value) in [
                ("data-gpu-announcer", ""),
                ("role", "status"),
                ("aria-live", "polite"),
                ("aria-atomic", "true"),
                (
                    "style",
                    "position:fixed;left:-10000px;top:0;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;pointer-events:none",
                ),
            ] {
                status
                    .set_attribute(name, value)
                    .map_err(|error| format!("accessibility announcement attribute: {error:?}"))?;
            }
            document
                .body()
                .ok_or("no body for accessibility announcements")?
                .append_child(&status)
                .map_err(|error| format!("accessibility announcement attachment: {error:?}"))?;
            Some(status)
        } else {
            None
        };
        Ok(Self { document: document.clone(), native, status, state: AnnouncementState::default() })
    }

    pub fn update(
        &mut self,
        output: &egui::PlatformOutput,
        narration: &[String],
        widgets: &BTreeMap<String, WidgetGeometry>,
    ) -> Result<(), String> {
        let messages = self.state.messages(narration, widgets);
        for message in &messages {
            self.speak(message)?;
        }
        let description = output.events_description();
        if messages.is_empty() && !description.is_empty() {
            self.speak(&description)?;
        }
        Ok(())
    }

    fn speak(&self, text: &str) -> Result<(), String> {
        if let Some(native) = &self.native {
            native
                .call1(self.document.as_ref(), &JsValue::from_str(text))
                .map_err(|error| format!("accessibility announcement: {error:?}"))?;
        } else if let Some(status) = &self.status {
            status.set_text_content(Some(text));
        }
        Ok(())
    }
}

impl Drop for Accessibility {
    fn drop(&mut self) {
        if let Some(status) = &self.status {
            status.remove();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_widget_focus_and_changed_value_are_announced_once_with_the_control_label() {
        let mut state = AnnouncementState::default();
        let mut widgets = BTreeMap::new();
        widgets.insert(
            "scale".to_owned(),
            WidgetGeometry {
                id: "scale".to_owned(),
                label: "UI SCALE".to_owned(),
                role: "slider".to_owned(),
                bounds: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 20.0)).into(),
                paint_bounds: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 20.0))
                    .into(),
                enabled: true,
                focused: true,
                modal: false,
                z: 0,
                value: Some("100".to_owned()),
                selected: None,
                egui_id: None,
                layer_id: None,
                duplicate: false,
                activated: false,
                interactive: true,
                keyboard_activated: false,
            },
        );
        assert_eq!(state.messages(&[], &widgets), ["UI SCALE, slider, 100"]);
        assert!(state.messages(&[], &widgets).is_empty());
        widgets.get_mut("scale").unwrap().value = Some("135".to_owned());
        assert_eq!(state.messages(&[], &widgets), ["UI SCALE, slider, 135"]);
        assert_eq!(
            state.messages(&["Tutorial narration".to_owned()], &widgets),
            ["Tutorial narration"]
        );
        assert!(state.messages(&["Tutorial narration".to_owned()], &widgets).is_empty());
    }

    #[test]
    fn password_values_do_not_enter_spoken_announcements() {
        let mut state = AnnouncementState::default();
        let widgets = BTreeMap::from([(
            "password".to_owned(),
            WidgetGeometry {
                id: "password".to_owned(),
                label: "Password".to_owned(),
                role: "password".to_owned(),
                bounds: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 20.0)).into(),
                paint_bounds: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 20.0))
                    .into(),
                enabled: true,
                focused: true,
                modal: false,
                z: 0,
                value: Some("private".to_owned()),
                selected: None,
                egui_id: None,
                layer_id: None,
                duplicate: false,
                activated: false,
                interactive: true,
                keyboard_activated: false,
            },
        )]);
        assert_eq!(state.messages(&[], &widgets), ["Password, password"]);
    }
}
