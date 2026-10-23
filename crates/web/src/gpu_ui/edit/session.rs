//! Provisional OS transport state. Accepted events are replayed by the single
//! editor model in their native dispatch order, including earlier UI actions.

use egui::{Key, Modifiers, Pos2, Rect, pos2};
use web_sys::HtmlTextAreaElement;

use super::model::{EditGroup, Intent, Model, Receipt, byte_index, replacement};
use super::{EditorDescriptor, TextSelection};

pub(crate) struct Session {
    pub descriptor: EditorDescriptor,
    pub model: Model,
    pub pointer: Option<i32>,
    pub preferred_x: Option<f32>,
}

impl Session {
    pub fn new(descriptor: EditorDescriptor) -> Self {
        Self {
            model: descriptor.model.as_ref().clone(),
            descriptor,
            pointer: None,
            preferred_x: None,
        }
    }

    pub fn update(&mut self, descriptor: EditorDescriptor) {
        // Earlier layout passes must not replace an already queued later event's
        // transport draft. Its immutable receipt will be replayed in turn.
        let model = descriptor.model.as_ref();
        if model.sequence >= self.model.sequence
            && (model.sequence != self.model.sequence
                || model.text() != self.model.text()
                || model.selection() != self.model.selection()
                || model.composition != self.model.composition)
        {
            self.model = model.clone();
        }
        self.descriptor = descriptor;
    }

    pub fn receipt(
        &mut self,
        sequence: u64,
        intent: Intent,
        native_selection: Option<TextSelection>,
        caret: Option<Rect>,
    ) -> Receipt {
        let intent = match intent {
            Intent::Replace { range, text, group } => {
                let text = if self.descriptor.multiline {
                    text.replace("\r\n", "\n").replace('\r', "\n")
                } else {
                    text.replace(['\r', '\n'], "")
                };
                Intent::Replace { range, text, group }
            }
            intent => intent,
        };
        let mut proposed = self.model.text().to_owned();
        if let Intent::Replace { range, text, .. } = &intent {
            let range = range.clamp(&proposed).range();
            proposed.replace_range(
                byte_index(self.model.text(), range.start)
                    ..byte_index(self.model.text(), range.end),
                text,
            );
        }
        let mut receipt = Receipt {
            owner: Some(self.descriptor.owner),
            sequence,
            before: self.model.text().into(),
            text: proposed.into(),
            selection: native_selection.unwrap_or_else(|| match &intent {
                Intent::Replace { range, text, .. } => {
                    TextSelection::caret(range.range().start + text.encode_utf16().count())
                }
                _ => self.model.selection(),
            }),
            composing: self.model.composition.is_some(),
            intent,
            caret,
            clipboard_handled: false,
        };
        self.model.apply(&receipt, self.descriptor.rules, self.descriptor.readonly);
        receipt.text = self.model.text().into();
        receipt.selection = self.model.selection();
        receipt.composing = self.model.composition.is_some();
        receipt
    }

    pub fn input(
        &mut self,
        sequence: u64,
        input_type: &str,
        textarea: &HtmlTextAreaElement,
    ) -> Receipt {
        let next = textarea.value();
        let selection = selection(textarea);
        let (range, text) = replacement(self.model.text(), &next);
        self.preferred_x = None;
        let group = match input_type {
            "insertText" | "insertLineBreak" | "insertParagraph" => Some(EditGroup::Typing),
            "deleteContentBackward" => Some(EditGroup::Backspace),
            "deleteContentForward" => Some(EditGroup::Delete),
            "deleteWordBackward" => Some(EditGroup::BackwardWord),
            "deleteWordForward" => Some(EditGroup::ForwardWord),
            _ => None,
        };
        self.receipt(sequence, Intent::Replace { range, text, group }, Some(selection), None)
    }

    pub fn pointer(&mut self, point: Pos2, extend: bool) -> Result<(Intent, Rect), String> {
        let hit = self.descriptor.hit(point, self.model.text())?;
        self.preferred_x = Some(hit.rect.min.x);
        let anchor = if extend { self.model.selection().anchor } else { hit.index };
        Ok((Intent::Select(TextSelection { anchor, focus: hit.index }), hit.rect))
    }

    pub fn click_selection(&self, point: Pos2, clicks: i32) -> Result<Intent, String> {
        use unicode_segmentation::UnicodeSegmentation;
        let hit = self.descriptor.hit(point, self.model.text())?.index;
        let text = self.model.text();
        let byte = byte_index(text, hit);
        let (start, end) = if clicks >= 3 {
            let start = text[..byte].rfind('\n').map_or(0, |index| index + 1);
            let end = text[byte..].find('\n').map_or(text.len(), |index| byte + index + 1);
            (start, end)
        } else {
            text.split_word_bound_indices()
                .find(|(start, part)| byte >= *start && byte < *start + part.len())
                .map_or((byte, byte), |(start, part)| (start, start + part.len()))
        };
        Ok(Intent::Select(TextSelection {
            anchor: text[..start].encode_utf16().count(),
            focus: text[..end].encode_utf16().count(),
        }))
    }

    pub fn navigation(&mut self, key: Key, mods: Modifiers) -> Result<Option<Intent>, String> {
        if !matches!(
            key,
            Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown
        ) {
            self.preferred_x = None;
            return Ok(None);
        }
        // Console Alt+Up/Down belongs to its actual history action.
        if self.descriptor.code_editor && mods.alt {
            return Ok(None);
        }
        let geometry = self.descriptor.layout(self.model.text())?;
        let current = self.model.selection();
        let caret = geometry.caret(current.focus);
        let row = geometry
            .lines
            .iter()
            .position(|line| current.focus >= line.start && current.focus <= line.end)
            .unwrap_or(geometry.lines.len() - 1);
        let target = match key {
            Key::Home if mods.command || mods.ctrl => 0,
            Key::End if mods.command || mods.ctrl => self.model.text().encode_utf16().count(),
            Key::Home => geometry.lines[row].start,
            Key::End => geometry.lines[row].end,
            Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown => {
                let x = *self.preferred_x.get_or_insert(caret.min.x);
                let direction = if matches!(key, Key::ArrowUp | Key::PageUp) { -1.0 } else { 1.0 };
                let height = if matches!(key, Key::PageUp | Key::PageDown) {
                    self.descriptor.content.height() / self.descriptor.scale
                } else {
                    geometry.lines[row].height
                };
                geometry.hit(pos2(x, caret.center().y + direction * height))
            }
            _ => unreachable!(),
        };
        Ok(Some(Intent::Select(TextSelection {
            anchor: if mods.shift { current.anchor } else { target },
            focus: target,
        })))
    }

    pub fn sync(&self, textarea: &HtmlTextAreaElement) -> Result<(), String> {
        if textarea.value() != self.model.text() {
            textarea.set_value(self.model.text());
        }
        textarea.set_read_only(self.descriptor.readonly);
        let selection = self.model.selection();
        if selection != super::session::selection(textarea) {
            textarea
                .set_selection_range_with_direction(
                    selection.range().start as u32,
                    selection.range().end as u32,
                    if selection.anchor > selection.focus { "backward" } else { "forward" },
                )
                .map_err(|error| format!("GPU editor selection transport: {error:?}"))?;
        }
        Ok(())
    }
}

/// A prior layout can acknowledge an event without painting its old field.
/// Native transport is reconciled only after every receipt it already observed
/// has been delivered or deliberately discarded by actual UI ownership.
pub(crate) fn reconcile_session(
    session: &mut Option<Session>,
    focused: Option<EditorDescriptor>,
    acknowledged: u64,
) -> bool {
    if session.as_ref().is_some_and(|session| session.model.sequence > acknowledged) {
        return false;
    }
    match (session.as_mut(), focused) {
        (Some(current), Some(descriptor)) if current.descriptor.owner == descriptor.owner => {
            current.update(descriptor)
        }
        (_, Some(descriptor)) => *session = Some(Session::new(descriptor)),
        (_, None) => *session = None,
    }
    true
}

pub fn selection(textarea: &HtmlTextAreaElement) -> TextSelection {
    let start = textarea.selection_start().ok().flatten().unwrap_or(0) as usize;
    let end = textarea.selection_end().ok().flatten().unwrap_or(start as u32) as usize;
    let backward = textarea
        .selection_direction()
        .ok()
        .flatten()
        .is_some_and(|direction| direction == "backward");
    if backward {
        TextSelection { anchor: end, focus: start }
    } else {
        TextSelection { anchor: start, focus: end }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{EditorFont, EditorRules, native_name_geometry};
    use super::*;
    use egui::{Pos2, Rect, Vec2};

    fn descriptor(text: &str) -> EditorDescriptor {
        let field = Rect::from_min_size(Pos2::ZERO, Vec2::new(384.0, 49.0));
        let model = Model::new(text);
        EditorDescriptor {
            layer_id: egui::LayerId::background(),
            owner: super::super::EditorOwner { id: egui::Id::unique("create-name"), instance: 1 },
            text: text.into(),
            model: std::sync::Arc::new(model),
            rules: EditorRules::default(),
            readonly: false,
            multiline: false,
            code_editor: false,
            enabled: true,
            focused: true,
            bounds: field,
            content: field,
            text_origin: Pos2::ZERO,
            scale: 1.0,
            font: EditorFont::NativeName { origin: Pos2::ZERO, scale: 1.0, field },
            geometry: native_name_geometry(text, Pos2::ZERO, 1.0, field),
            font_options: super::super::FontRunOptions::default(),
        }
    }

    #[test]
    fn layout_cannot_overwrite_a_future_native_dispatch_preview() {
        let mut session = Session::new(descriptor("A"));
        let first = session.receipt(
            1,
            Intent::Replace {
                range: TextSelection::caret(1),
                text: "B".into(),
                group: Some(EditGroup::Typing),
            },
            None,
            None,
        );
        session.receipt(
            2,
            Intent::Replace {
                range: TextSelection::caret(2),
                text: "C".into(),
                group: Some(EditGroup::Typing),
            },
            None,
            None,
        );
        let mut earlier = descriptor("A");
        std::sync::Arc::make_mut(&mut earlier.model).apply(&first, EditorRules::default(), false);
        session.update(earlier);
        assert_eq!(session.model.text(), "ABC");
        let mut final_state = descriptor("ABC");
        std::sync::Arc::make_mut(&mut final_state.model).sequence = 2;
        session.update(final_state);
        assert_eq!(session.model.text(), "ABC");
    }

    #[test]
    fn native_field_rules_run_before_a_second_same_time_edit() {
        fn validate(value: &str) -> Option<String> {
            if value.is_empty() {
                None
            } else {
                solomon_sim::menu::create_name::validate_create_wizard_name(value)
                    .map(|reason| reason.message().into())
            }
        }
        let mut field = descriptor("AAAAAAAAAAA");
        field.rules.validate = Some(validate);
        let mut session = Session::new(field);
        let rejected = session.receipt(
            1,
            Intent::Replace {
                range: TextSelection::caret(11),
                text: "A".into(),
                group: Some(EditGroup::Typing),
            },
            None,
            None,
        );
        assert_eq!(session.model.text(), "AAAAAAAAAAA");
        assert_eq!(rejected.text.as_ref(), "AAAAAAAAAAA");
        let accepted = session.receipt(
            2,
            Intent::Replace {
                range: TextSelection { anchor: 0, focus: 11 },
                text: "Felix".into(),
                group: Some(EditGroup::Typing),
            },
            None,
            None,
        );
        assert_eq!(accepted.before.as_ref(), "AAAAAAAAAAA");
        assert_eq!(accepted.text.as_ref(), "Felix");
        assert_eq!(session.model.selection(), TextSelection::caret(5));
    }

    #[test]
    fn an_earlier_blur_or_other_field_cannot_retire_future_native_composition() {
        let mut later = descriptor("B");
        later.owner.id = egui::Id::unique("later-field");
        let mut session = Some(Session::new(later));
        let active = session.as_mut().unwrap();
        active.receipt(8, Intent::CompositionStart, None, None);
        active.receipt(
            9,
            Intent::Replace {
                range: TextSelection::caret(1),
                text: "語".into(),
                group: Some(EditGroup::Typing),
            },
            None,
            None,
        );
        assert!(!reconcile_session(&mut session, None, 7));
        assert!(!reconcile_session(&mut session, Some(descriptor("A")), 8));
        assert_eq!(session.as_ref().unwrap().descriptor.owner.id, egui::Id::unique("later-field"));
        assert_eq!(session.as_ref().unwrap().model.text(), "B語");
        assert!(session.as_ref().unwrap().model.composition.is_some());
        assert!(
            reconcile_session(&mut session, None, 9),
            "a removed field still acknowledges its discarded receipts"
        );
        assert!(session.is_none());
    }
}
