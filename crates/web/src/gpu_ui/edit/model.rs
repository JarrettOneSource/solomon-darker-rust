//! One accepted editing model. Native events carry immutable snapshots and intents;
//! replay can therefore follow an earlier Clear/history action in the same frame.

use std::sync::Arc;

use egui::util::undoer::Undoer;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub focus: usize,
}

impl Selection {
    pub fn caret(index: usize) -> Self {
        Self { anchor: index, focus: index }
    }

    pub fn range(self) -> std::ops::Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }

    pub fn clamp(self, text: &str) -> Self {
        Self { anchor: boundary(text, self.anchor), focus: boundary(text, self.focus) }
    }
}

/// Field-owned Source rules run in native dispatch as well as chronological UI
/// replay. Neither path duplicates or guesses the field's actual validation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rules {
    pub utf16_limit: Option<usize>,
    pub normalize: Option<fn(&str) -> String>,
    pub validate: Option<fn(&str) -> Option<String>>,
}

impl Rules {
    pub fn accept(self, value: &str) -> Result<String, String> {
        let mut value = self.normalize.map_or_else(|| value.to_owned(), |f| f(value));
        if let Some(limit) = self.utf16_limit {
            let mut units = 0;
            value = value
                .chars()
                .take_while(|character| {
                    units += character.len_utf16();
                    units <= limit
                })
                .collect();
        }
        if let Some(reason) = self.validate.and_then(|f| f(&value)) {
            Err(reason)
        } else {
            Ok(value)
        }
    }
}

#[derive(Clone, Debug)]
pub enum Intent {
    Replace { range: Selection, text: String },
    Select(Selection),
    SelectAll,
    Copy,
    Cut,
    Undo,
    Redo,
    Submit,
    CompositionStart,
    CompositionEnd,
}

#[derive(Clone, Debug)]
pub struct Receipt {
    pub owner: Option<super::EditorOwner>,
    pub sequence: u64,
    pub time: f64,
    pub before: Arc<str>,
    pub text: Arc<str>,
    pub selection: Selection,
    pub intent: Intent,
    pub composing: bool,
    pub caret: Option<egui::Rect>,
    pub clipboard_handled: bool,
}

impl Receipt {
    pub fn unbound(sequence: u64, time: f64, intent: Intent) -> Self {
        Self {
            owner: None,
            sequence,
            time,
            before: "".into(),
            text: "".into(),
            selection: Selection::caret(0),
            intent,
            composing: false,
            caret: None,
            clipboard_handled: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Value {
    text: String,
    selection: Selection,
}

#[derive(Clone)]
pub struct Model {
    value: Value,
    undo: Undoer<Value>,
    pub sequence: u64,
    pub composition: Option<Selection>,
}

#[derive(Default)]
pub struct Change {
    pub changed: bool,
    pub submitted: bool,
    pub rejected: Option<String>,
    pub copied: Option<String>,
}

impl Model {
    pub fn new(text: &str) -> Self {
        Self {
            value: Value {
                text: text.to_owned(),
                selection: Selection::caret(text.encode_utf16().count()),
            },
            undo: Undoer::default(),
            sequence: 0,
            composition: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.value.text
    }

    pub fn selection(&self) -> Selection {
        self.value.selection
    }

    pub fn select(&mut self, selection: Selection) {
        self.value.selection = selection.clamp(&self.value.text);
    }

    pub fn reconcile(&mut self, text: &str) {
        if self.value.text != text {
            self.value.text = text.to_owned();
            self.value.selection = Selection::caret(text.encode_utf16().count());
            self.undo = Undoer::default();
            self.composition = None;
        }
    }

    pub fn apply(&mut self, receipt: &Receipt, rules: Rules, readonly: bool) -> Change {
        if receipt.sequence <= self.sequence {
            return Change::default();
        }
        self.sequence = receipt.sequence;
        let mut change = Change::default();
        let before = self.value.clone();
        if self.composition.is_none() {
            self.undo.feed_state(receipt.time, &self.value);
        }
        match &receipt.intent {
            Intent::SelectAll => {
                self.select(Selection { anchor: 0, focus: self.value.text.encode_utf16().count() });
            }
            Intent::Select(selection) => {
                // A native selection belongs to its measured text. A preceding UI
                // action may already have installed another accepted draft.
                if receipt.before.as_ref() == self.value.text {
                    self.select(*selection);
                }
            }
            Intent::Copy | Intent::Cut => {
                let range = self.value.selection.range();
                if !range.is_empty() {
                    change.copied = Some(slice_utf16(&self.value.text, range.clone()).to_owned());
                    if matches!(receipt.intent, Intent::Cut) && !readonly {
                        self.replace(self.value.selection, "", rules, &mut change);
                    }
                }
            }
            Intent::Undo if !readonly => {
                if let Some(value) = self.undo.undo(&self.value).cloned() {
                    self.value = value;
                    self.composition = None;
                }
            }
            Intent::Redo if !readonly => {
                if let Some(value) = self.undo.redo(&self.value).cloned() {
                    self.value = value;
                    self.composition = None;
                }
            }
            Intent::Submit => change.submitted = true,
            Intent::CompositionStart if !readonly => {
                self.composition = Some(self.value.selection);
            }
            Intent::CompositionEnd => {
                self.composition = None;
            }
            Intent::Replace { range, text } if !readonly => {
                let same_text = receipt.before.as_ref() == self.value.text;
                let range = if same_text { *range } else { self.value.selection };
                self.replace(range, text, rules, &mut change);
                if same_text
                    && change.rejected.is_none()
                    && receipt.text.as_ref() == self.value.text
                {
                    self.select(receipt.selection);
                }
                if receipt.composing {
                    let start = self.composition.map_or(range.range().start, |r| r.range().start);
                    self.composition =
                        Some(Selection { anchor: start, focus: self.value.selection.focus });
                }
            }
            _ => {}
        }
        change.changed = self.value.text != before.text;
        if self.composition.is_none() && !matches!(receipt.intent, Intent::Undo | Intent::Redo) {
            self.undo.feed_state(receipt.time, &self.value);
        }
        change
    }

    fn replace(&mut self, selection: Selection, inserted: &str, rules: Rules, change: &mut Change) {
        let range = selection.clamp(&self.value.text).range();
        let mut proposed = self.value.text.clone();
        let start = byte_index(&proposed, range.start);
        let end = byte_index(&proposed, range.end);
        let remaining = proposed.encode_utf16().count() - (range.end - range.start);
        let mut units = 0;
        let inserted: String = inserted
            .chars()
            .take_while(|character| {
                units += character.len_utf16();
                rules.utf16_limit.is_none_or(|limit| units <= limit.saturating_sub(remaining))
            })
            .collect();
        proposed.replace_range(start..end, &inserted);
        match rules.accept(&proposed) {
            Ok(accepted) => {
                let position = if accepted == proposed {
                    range.start + inserted.encode_utf16().count()
                } else {
                    accepted.encode_utf16().count()
                };
                self.value.text = accepted;
                self.value.selection = Selection::caret(boundary(&self.value.text, position));
            }
            Err(reason) => change.rejected = Some(reason),
        }
    }
}

pub fn boundary(text: &str, index: usize) -> usize {
    let mut units = 0;
    for character in text.chars() {
        let end = units + character.len_utf16();
        if index < end {
            return units;
        }
        units = end;
    }
    units
}

pub fn byte_index(text: &str, index: usize) -> usize {
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        if units + character.len_utf16() > index {
            return byte;
        }
        units += character.len_utf16();
    }
    text.len()
}

pub fn slice_utf16(text: &str, range: std::ops::Range<usize>) -> &str {
    &text[byte_index(text, range.start)..byte_index(text, range.end)]
}

/// Minimal scalar-safe replacement represented by an actual native input change.
pub fn replacement(before: &str, after: &str) -> (Selection, String) {
    let mut prefix = 0;
    let mut prefix_bytes = 0;
    for (a, b) in before.chars().zip(after.chars()) {
        if a != b {
            break;
        }
        prefix += a.len_utf16();
        prefix_bytes += a.len_utf8();
    }
    let mut suffix_units = 0;
    let mut suffix_bytes = 0;
    for (a, b) in before[prefix_bytes..].chars().rev().zip(after[prefix_bytes..].chars().rev()) {
        if a != b {
            break;
        }
        suffix_units += a.len_utf16();
        suffix_bytes += a.len_utf8();
    }
    (
        Selection { anchor: prefix, focus: before.encode_utf16().count() - suffix_units },
        after[prefix_bytes..after.len() - suffix_bytes].to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_rule(value: &str) -> Option<String> {
        (!value.is_empty())
            .then(|| solomon_sim::menu::create_name::validate_create_wizard_name(value))
            .flatten()
            .map(|reason| reason.message().to_owned())
    }

    fn insert(sequence: u64, before: &str, range: Selection, text: &str) -> Receipt {
        let mut next = before.to_owned();
        next.replace_range(
            byte_index(before, range.range().start)..byte_index(before, range.range().end),
            text,
        );
        Receipt {
            owner: Some(super::super::EditorOwner { id: egui::Id::unique("name"), instance: 1 }),
            sequence,
            time: 1.0,
            before: before.into(),
            text: next.into(),
            selection: Selection::caret(range.range().start + text.encode_utf16().count()),
            intent: Intent::Replace { range, text: text.to_owned() },
            composing: false,
            caret: None,
            clipboard_handled: false,
        }
    }

    #[test]
    fn rejected_name_then_valid_replacement_share_one_dispatch_time() {
        let before = "AAAAAAAAAAA";
        let rules = Rules { validate: Some(create_rule), ..Default::default() };
        let mut model = Model::new(before);
        let rejected = model.apply(&insert(1, before, Selection::caret(11), "A"), rules, false);
        assert_eq!(model.text(), before);
        assert_eq!(rejected.rejected.as_deref(), Some("Wizard name is too wide."));
        let accepted = model.apply(
            &insert(2, before, Selection { anchor: 0, focus: 11 }, "Felix"),
            rules,
            false,
        );
        assert!(accepted.changed);
        assert_eq!(model.text(), "Felix");
        assert_eq!(model.selection(), Selection::caret(5));
    }

    #[test]
    fn queued_input_follows_an_earlier_real_clear_instead_of_its_old_native_preview() {
        let mut model = Model::new("AAAAAAAAAAA");
        model.reconcile("");
        let rules = Rules { validate: Some(create_rule), ..Default::default() };
        model.apply(&insert(1, "AAAAAAAAAAA", Selection::caret(11), "A"), rules, false);
        model.apply(&insert(2, "AAAAAAAAAAA", Selection::caret(11), "B"), rules, false);
        assert_eq!(model.text(), "AB");
        let mut undo = insert(3, "AB", Selection::caret(2), "");
        undo.intent = Intent::Undo;
        model.apply(&undo, rules, false);
        assert_eq!(model.text(), "");
    }

    #[test]
    fn utf16_limits_and_readonly_keep_the_actual_accepted_value() {
        let rules = Rules { utf16_limit: Some(32), ..Default::default() };
        let value = format!("{}🧙", "a".repeat(31));
        assert_eq!(rules.accept(&value).unwrap(), "a".repeat(31));
        let mut model = Model::new("A🧑‍💻é🧙🏽B");
        let original = model.text().to_owned();
        model.apply(
            &insert(
                1,
                &original,
                Selection { anchor: 0, focus: original.encode_utf16().count() },
                "changed",
            ),
            rules,
            true,
        );
        assert_eq!(model.text(), original);
        assert_eq!(replacement("A🧙B", "A🧙🏽B"), (Selection::caret(3), "🏽".to_owned()));
        assert_eq!(boundary("A🧙B", 2), 1);
    }

    #[test]
    fn maxlength_preserves_the_existing_suffix_and_replaces_selection_by_utf16_capacity() {
        let rules = Rules { utf16_limit: Some(32), ..Default::default() };
        let original = "a".repeat(31);
        let mut model = Model::new(&original);
        model.apply(&insert(1, &original, Selection::caret(0), "😀"), rules, false);
        assert_eq!(
            model.text(),
            original,
            "a supplementary scalar cannot fit in one remaining unit"
        );
        model.apply(&insert(2, &original, Selection::caret(0), "z😀"), rules, false);
        assert_eq!(model.text(), format!("z{original}"));
        assert_eq!(model.selection(), Selection::caret(1));
        let before = model.text().to_owned();
        model.apply(&insert(3, &before, Selection { anchor: 0, focus: 2 }, "🧙"), rules, false);
        assert_eq!(model.text(), format!("🧙{}", "a".repeat(30)));
        assert_eq!(model.selection(), Selection::caret(2));
    }

    #[test]
    fn submissions_belong_to_one_dispatch_and_do_not_change_selection_or_value() {
        let mut model = Model::new("a real query");
        let selected = Selection { anchor: 2, focus: 8 };
        model.select(selected);
        let mut receipt = insert(1, model.text(), selected, "");
        receipt.intent = Intent::Submit;
        assert!(model.apply(&receipt, Rules::default(), false).submitted);
        assert!(!model.apply(&receipt, Rules::default(), false).submitted);
        receipt.sequence = 2;
        assert!(model.apply(&receipt, Rules::default(), false).submitted);
        assert_eq!(model.text(), "a real query");
        assert_eq!(model.selection(), selected);
    }

    #[test]
    fn multiline_newline_composition_and_undo_preserve_full_graphemes() {
        let mut model = Model::new("");
        let text = "مرحبا\nשלום 🧑‍💻é";
        let mut receipt = insert(1, "", Selection::caret(0), "");
        receipt.intent = Intent::CompositionStart;
        model.apply(&receipt, Rules::default(), false);
        let mut input = insert(2, "", Selection::caret(0), text);
        input.composing = true;
        model.apply(&input, Rules::default(), false);
        assert_eq!(model.text(), text);
        assert!(model.composition.is_some());
        receipt.sequence = 3;
        receipt.intent = Intent::CompositionEnd;
        model.apply(&receipt, Rules::default(), false);
        receipt.sequence = 4;
        receipt.intent = Intent::Undo;
        model.apply(&receipt, Rules::default(), false);
        assert_eq!(model.text(), "");
        receipt.sequence = 5;
        receipt.intent = Intent::Redo;
        model.apply(&receipt, Rules::default(), false);
        assert_eq!(model.text(), text);
        let expected = "مرحبا\nשלום 🧑‍💻";
        let (range, inserted) = replacement(text, expected);
        model.apply(&insert(6, text, range, &inserted), Rules::default(), false);
        assert_eq!(model.text(), "مرحبا\nשלום 🧑‍💻", "backspace removes the combined e cluster once");
    }

    #[test]
    fn native_cut_paste_and_undo_preserve_distinct_source_transactions() {
        let mut model = Model::new("abc");
        model.select(Selection { anchor: 0, focus: 3 });
        let mut cut = insert(1, "abc", model.selection(), "");
        cut.intent = Intent::Cut;
        model.apply(&cut, Rules::default(), false);
        assert_eq!(model.text(), "");
        model.apply(&insert(2, "", Selection::caret(0), "abc"), Rules::default(), false);
        assert_eq!(model.text(), "abc");
        let mut undo = insert(3, "abc", Selection::caret(3), "");
        undo.intent = Intent::Undo;
        model.apply(&undo, Rules::default(), false);
        assert_eq!(model.text(), "", "native Undo first removes the paste, not the earlier cut");
        undo.sequence = 4;
        model.apply(&undo, Rules::default(), false);
        assert_eq!(model.text(), "abc");
        assert_eq!(
            model.selection(),
            Selection { anchor: 0, focus: 3 },
            "Undo cut restores its selected text"
        );
    }
}
