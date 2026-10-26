//! Nonvisual screen-reader announcements for GPU controls and Tutorial narration.

#[cfg(target_arch = "wasm32")]
use solomon_graphics::announcements::ScreenReader;
use std::collections::{BTreeMap, btree_map::Entry};

use super::WidgetGeometry;

pub(super) struct Announcement {
    pub owner: String,
    pub text: String,
}

#[derive(Default)]
struct AnnouncementState {
    narration: BTreeMap<String, String>,
    focused: Option<String>,
    values: BTreeMap<String, (Option<String>, Option<bool>)>,
}

impl AnnouncementState {
    fn messages(
        &mut self,
        narration: &[Announcement],
        widgets: &BTreeMap<String, WidgetGeometry>,
    ) -> Vec<String> {
        let mut messages = Vec::new();
        let mut current_narration = BTreeMap::new();
        for announcement in narration.iter().filter(|announcement| !announcement.text.is_empty()) {
            if let Entry::Vacant(entry) = current_narration.entry(announcement.owner.clone()) {
                if self.narration.get(&announcement.owner) != Some(&announcement.text) {
                    messages.push(announcement.text.clone());
                }
                entry.insert(announcement.text.clone());
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
    #[cfg(target_arch = "wasm32")]
    reader: ScreenReader,
    #[cfg(not(target_arch = "wasm32"))]
    surface: crate::platform::Surface,
    #[cfg(not(target_arch = "wasm32"))]
    spoken: (u64, String),
    state: AnnouncementState,
}

impl Accessibility {
    pub fn attach(surface: &crate::platform::Surface) -> Result<Self, String> {
        Ok(Self {
            #[cfg(target_arch = "wasm32")]
            reader: ScreenReader::attach(
                &surface.canvas().owner_document().ok_or("No accessibility document")?,
            )?,
            #[cfg(not(target_arch = "wasm32"))]
            surface: surface.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            spoken: (0, String::new()),
            state: AnnouncementState::default(),
        })
    }

    pub fn update(
        &mut self,
        output: &mut egui::PlatformOutput,
        narration: &[Announcement],
        widgets: &BTreeMap<String, WidgetGeometry>,
    ) -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        let foreground = self.reader.is_foreground()?;
        #[cfg(not(target_arch = "wasm32"))]
        let foreground = self.surface.window().has_focus();
        if !foreground {
            return Ok(());
        }
        let mut messages = self.state.messages(narration, widgets);
        let description = output.events_description();
        if messages.is_empty() && !description.is_empty() {
            messages.push(description);
        }
        if !messages.is_empty() {
            #[cfg(target_arch = "wasm32")]
            self.reader.announce_batch(&messages)?;
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.spoken.0 = self.spoken.0.wrapping_add(1);
                self.spoken.1 = messages.join(". ");
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(update) = &mut output.accesskit_update {
            update_native_tree(update, widgets, &self.spoken);
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn role(name: &str) -> egui::accesskit::Role {
    use egui::accesskit::Role;
    match name {
        "button" => Role::Button,
        "textbox" | "input" | "text-entry" => Role::TextInput,
        "password" => Role::PasswordInput,
        "slider" => Role::Slider,
        "checkbox" => Role::CheckBox,
        "switch" => Role::Switch,
        "combobox" => Role::ComboBox,
        "radio" => Role::RadioButton,
        "tab" => Role::Tab,
        "menuitem" => Role::MenuItem,
        "status" => Role::Status,
        _ => Role::Label,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn update_native_tree(
    update: &mut egui::accesskit::TreeUpdate,
    widgets: &BTreeMap<String, WidgetGeometry>,
    spoken: &(u64, String),
) {
    use egui::accesskit::{Live, Node, Role, Toggled};
    for widget in widgets.values() {
        let Some(id) = widget.egui_id else { continue };
        let Some((_, node)) =
            update.nodes.iter_mut().find(|(node_id, _)| *node_id == id.accesskit_id())
        else {
            continue;
        };
        let role = role(&widget.role);
        if !(role == Role::TextInput && node.role() == Role::MultilineTextInput) {
            node.set_role(role);
        }
        node.set_label(&widget.label);
        node.set_author_id(&widget.id);
        if !widget.enabled {
            node.set_disabled();
        }
        if widget.role != "password"
            && let Some(value) = &widget.value
        {
            node.set_value(value);
        }
        if let Some(selected) = widget.selected {
            if matches!(role, Role::CheckBox | Role::Switch) {
                node.set_toggled(if selected { Toggled::True } else { Toggled::False });
            } else {
                node.set_selected(selected);
            }
        }
    }
    if spoken.1.is_empty() {
        return;
    }
    let Some(tree) = &update.tree else { return };
    let id = egui::Id::unique(("solomon-narration", spoken.0)).accesskit_id();
    if let Some((_, root)) = update.nodes.iter_mut().find(|(id, _)| *id == tree.root) {
        root.push_child(id);
        let mut status = Node::new(Role::Status);
        status.set_live(Live::Polite);
        status.set_live_atomic();
        status.set_label(&spoken.1);
        update.nodes.push((id, status));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn announcement(owner: &str, text: &str) -> Announcement {
        Announcement { owner: owner.to_owned(), text: text.to_owned() }
    }

    #[test]
    fn unchanged_loading_batches_stay_quiet_and_later_owner_returns_are_announced() {
        let mut state = AnnouncementState::default();
        let widgets = BTreeMap::new();
        let loading = vec![
            announcement("match-loading:label", "Loading Boneyard…"),
            announcement("runtime:status", "Awakening the world..."),
        ];
        assert_eq!(
            state.messages(&loading, &widgets),
            ["Loading Boneyard…", "Awakening the world..."]
        );
        assert!(state.messages(&loading, &widgets).is_empty());
        let materializing = vec![
            announcement("match-loading:label", "Loading Boneyard…"),
            announcement("runtime:status", "Preparing your wizard..."),
        ];
        assert_eq!(state.messages(&materializing, &widgets), ["Preparing your wizard..."]);
        assert!(state.messages(&materializing, &widgets).is_empty());
        assert!(state.messages(&[], &widgets).is_empty());
        assert_eq!(
            state.messages(&loading, &widgets),
            ["Loading Boneyard…", "Awakening the world..."]
        );
    }

    #[test]
    fn distinct_receipts_preserve_identical_text_order_and_only_same_owner_is_coalesced() {
        let mut state = AnnouncementState::default();
        let widgets = BTreeMap::new();
        let receipts = vec![
            announcement("social/chat:session-1:8", "Sirmin: Hello"),
            announcement("world-notification:book:-2", "+1 Ring of Fire"),
            announcement("social/chat:session-1:8", "Sirmin: Hello"),
            announcement("social/chat:session-1:9", "Sirmin: Hello"),
        ];
        assert_eq!(
            state.messages(&receipts, &widgets),
            ["Sirmin: Hello", "+1 Ring of Fire", "Sirmin: Hello"]
        );
        assert!(state.messages(&receipts, &widgets).is_empty());
        let next = vec![
            announcement("social/chat:session-1:9", "Sirmin: Hello"),
            announcement("social/chat:session-1:10", "Sirmin: Hello"),
            announcement("world-notification:book:-3", "+1 Ring of Fire"),
        ];
        assert_eq!(state.messages(&next, &widgets), ["Sirmin: Hello", "+1 Ring of Fire"]);
        assert!(state.messages(&next, &widgets).is_empty());
        assert!(state.messages(&[], &widgets).is_empty());
        assert_eq!(state.messages(&receipts, &widgets).len(), 3);
    }

    #[test]
    fn status_content_changes_and_real_owner_returns_are_announced() {
        let mut state = AnnouncementState::default();
        let widgets = BTreeMap::new();
        let initial = vec![announcement("save-status", "Save imported")];
        assert_eq!(state.messages(&initial, &widgets), ["Save imported"]);
        assert!(state.messages(&initial, &widgets).is_empty());
        let changed = vec![announcement("save-status", "Save downloaded")];
        assert_eq!(state.messages(&changed, &widgets), ["Save downloaded"]);
        assert!(state.messages(&changed, &widgets).is_empty());
        let emptied = vec![announcement("save-status", "")];
        assert!(state.messages(&emptied, &widgets).is_empty());
        assert_eq!(state.messages(&changed, &widgets), ["Save downloaded"]);
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
            state.messages(&[announcement("tutorial:narration:1", "Tutorial narration")], &widgets),
            ["Tutorial narration"]
        );
        assert!(
            state
                .messages(&[announcement("tutorial:narration:1", "Tutorial narration")], &widgets)
                .is_empty()
        );
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
