//! Nonvisual screen-reader announcements for GPU controls and Tutorial narration.

use solomon_graphics::announcements::ScreenReader;
use std::collections::{BTreeMap, BTreeSet};
use web_sys::Document;

use super::WidgetGeometry;

#[derive(Default)]
struct AnnouncementState {
    narration: BTreeSet<String>,
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
        let mut current_narration = BTreeSet::new();
        for text in narration.iter().filter(|text| !text.is_empty()) {
            if current_narration.insert(text.clone()) && !self.narration.contains(text) {
                messages.push(text.clone());
            }
        }
        self.narration = current_narration;
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
    reader: ScreenReader,
    state: AnnouncementState,
}

impl Accessibility {
    pub fn attach(document: &Document) -> Result<Self, String> {
        Ok(Self { reader: ScreenReader::attach(document)?, state: AnnouncementState::default() })
    }

    pub fn update(
        &mut self,
        output: &egui::PlatformOutput,
        narration: &[String],
        widgets: &BTreeMap<String, WidgetGeometry>,
    ) -> Result<(), String> {
        if !self.reader.is_foreground()? {
            return Ok(());
        }
        let mut messages = self.state.messages(narration, widgets);
        let description = output.events_description();
        if messages.is_empty() && !description.is_empty() {
            messages.push(description);
        }
        if !messages.is_empty() {
            self.reader.announce_batch(&messages)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_loading_batches_stay_quiet_and_later_owner_returns_are_announced() {
        let mut state = AnnouncementState::default();
        let widgets = BTreeMap::new();
        let loading = vec!["Loading Boneyard…".to_owned(), "Awakening the world...".to_owned()];
        assert_eq!(state.messages(&loading, &widgets), loading);
        assert!(state.messages(&loading, &widgets).is_empty());
        let materializing =
            vec!["Loading Boneyard…".to_owned(), "Preparing your wizard...".to_owned()];
        assert_eq!(state.messages(&materializing, &widgets), ["Preparing your wizard..."]);
        assert!(state.messages(&materializing, &widgets).is_empty());
        assert!(state.messages(&[], &widgets).is_empty());
        assert_eq!(state.messages(&loading, &widgets), loading);
    }

    #[test]
    fn a_changed_frame_preserves_message_order_and_coalesces_identical_current_text() {
        let mut state = AnnouncementState::default();
        let widgets = BTreeMap::new();
        let lesson = vec![
            "Midnight, six months ago".to_owned(),
            "Midnight, six months ago".to_owned(),
            "Sirmin: SOLOMON DARK!  Show yourself!".to_owned(),
        ];
        assert_eq!(
            state.messages(&lesson, &widgets),
            ["Midnight, six months ago", "Sirmin: SOLOMON DARK!  Show yourself!"]
        );
        assert!(state.messages(&lesson, &widgets).is_empty());
        assert_eq!(state.messages(&["SURVIVE. ".to_owned()], &widgets), ["SURVIVE. "]);
        assert_eq!(state.messages(&lesson, &widgets).len(), 2);
    }

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
                hit_shape: super::super::HitShape::Rect,
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
                hit_shape: super::super::HitShape::Rect,
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
