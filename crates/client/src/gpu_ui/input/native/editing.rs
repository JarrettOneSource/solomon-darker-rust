//! Native keyboard/IME intents; accepted content, undo and rules stay in the shared model.

use crate::gpu_ui::edit::{EditorFont, EditorOwner, native_shape};
use solomon_graphics::text::parley::{editing::Cursor, layout::Affinity};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub(super) struct Editing {
    caret: Option<VisualCaret>,
}

struct VisualCaret {
    owner: EditorOwner,
    sequence: u64,
    row: usize,
    line_start: usize,
    line_text: String,
    cursor: Cursor,
}

impl Editing {
    pub fn reset(&mut self) {
        self.caret = None;
    }

    pub fn pointer(&mut self, queue: &InputQueue, caret: Rect) -> Result<(), String> {
        self.reset();
        let Some(session) = &queue.session else { return Ok(()) };
        let EditorFont::Browser(font) = &session.descriptor.font else { return Ok(()) };
        let geometry = session.descriptor.layout(session.model.text())?;
        let index = session.model.selection().focus;
        let Some((row, line)) = geometry
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| index >= line.start && index <= line.end)
            .min_by(|(_, a), (_, b)| {
                (a.top - caret.top()).abs().total_cmp(&(b.top - caret.top()).abs())
            })
        else {
            return Ok(());
        };
        let shaped = native_shape(
            font,
            &line.raster_text,
            session.descriptor.font_options.letter_spacing.unwrap_or(0.0),
        )?;
        let leading = line
            .clusters
            .iter()
            .find(|cluster| cluster.start == index)
            .map(|cluster| cluster.leading.x);
        let trailing = line
            .clusters
            .iter()
            .find(|cluster| cluster.end == index)
            .map(|cluster| cluster.trailing.x);
        let distance = |x: Option<f32>| x.map_or(f32::INFINITY, |x| (x - caret.left()).abs());
        let affinity = if distance(trailing) < distance(leading) {
            Affinity::Upstream
        } else {
            Affinity::Downstream
        };
        self.caret = Some(VisualCaret {
            owner: session.descriptor.owner,
            sequence: session.model.sequence,
            row,
            line_start: line.start,
            line_text: line.text.clone(),
            cursor: Cursor::from_byte_index(
                &shaped.layout,
                byte_index(&line.text, index - line.start),
                affinity,
            ),
        });
        Ok(())
    }

    pub fn key(
        &mut self,
        queue: &mut InputQueue,
        key: Key,
        mods: Modifiers,
    ) -> Result<bool, String> {
        let Some(session) = queue.session.as_mut() else { return Ok(false) };
        let horizontal = matches!(key, Key::ArrowLeft | Key::ArrowRight)
            && !(cfg!(target_os = "macos") && mods.mac_cmd);
        if horizontal && let EditorFont::Browser(font) = &session.descriptor.font {
            let forward = key == Key::ArrowRight;
            let word = if cfg!(target_os = "macos") { mods.alt } else { mods.ctrl };
            session.preferred_x = None;
            let geometry = session.descriptor.layout(session.model.text())?;
            let selection = session.model.selection();
            let previous = self.caret.as_ref().filter(|caret| {
                caret.owner == session.descriptor.owner
                    && caret.sequence == session.model.sequence
                    && geometry.lines.get(caret.row).is_some_and(|line| {
                        line.start == caret.line_start && line.text == caret.line_text
                    })
            });
            let mut row = previous.map_or_else(
                || {
                    geometry
                        .lines
                        .iter()
                        .position(|line| {
                            selection.focus >= line.start && selection.focus <= line.end
                        })
                        .unwrap_or(geometry.lines.len() - 1)
                },
                |caret| caret.row,
            );
            let spacing = session.descriptor.font_options.letter_spacing.unwrap_or(0.0);
            let mut shaped = native_shape(font, &geometry.lines[row].raster_text, spacing)?;
            let mut cursor = previous.map_or_else(
                || {
                    Cursor::from_byte_index(
                        &shaped.layout,
                        byte_index(
                            &geometry.lines[row].text,
                            selection.focus - geometry.lines[row].start,
                        ),
                        Affinity::Downstream,
                    )
                },
                |caret| caret.cursor.refresh(&shaped.layout),
            );
            if !mods.shift && !word && !selection.range().is_empty() {
                let anchor = geometry.caret(selection.anchor);
                let focus = cursor.geometry(&shaped.layout, 0.0);
                let focus_point = (geometry.lines[row].top, focus.x0 as f32);
                if ((anchor.top(), anchor.left()) > focus_point) == forward {
                    row = geometry
                        .lines
                        .iter()
                        .position(|line| {
                            selection.anchor >= line.start && selection.anchor <= line.end
                        })
                        .unwrap_or(geometry.lines.len() - 1);
                    shaped = native_shape(font, &geometry.lines[row].raster_text, spacing)?;
                    cursor = Cursor::from_byte_index(
                        &shaped.layout,
                        byte_index(
                            &geometry.lines[row].text,
                            selection.anchor - geometry.lines[row].start,
                        ),
                        Affinity::Downstream,
                    );
                }
            } else {
                let next = match (forward, word) {
                    (true, false) => cursor.next_visual(&shaped.layout),
                    (false, false) => cursor.previous_visual(&shaped.layout),
                    (true, true) => cursor.next_visual_word(&shaped.layout),
                    (false, true) => cursor.previous_visual_word(&shaped.layout),
                };
                if next == cursor
                    && ((forward && row + 1 < geometry.lines.len()) || (!forward && row > 0))
                {
                    row = if forward { row + 1 } else { row - 1 };
                    shaped = native_shape(font, &geometry.lines[row].raster_text, spacing)?;
                    cursor = Cursor::from_point(
                        &shaped.layout,
                        if forward { -1.0 } else { shaped.layout.full_width() + 1.0 },
                        0.0,
                    );
                } else {
                    cursor = next;
                }
            }
            let line = &geometry.lines[row];
            let index = line.start + line.raster_text[..cursor.index()].encode_utf16().count();
            // The common geometry has already applied tab stops and line height.
            // Affinity chooses the correct side when one index has two visual positions.
            let x = if cursor.affinity() == Affinity::Upstream {
                line.clusters
                    .iter()
                    .find(|cluster| cluster.end == index)
                    .map(|cluster| cluster.trailing.x)
            } else {
                line.clusters
                    .iter()
                    .find(|cluster| cluster.start == index)
                    .map(|cluster| cluster.leading.x)
            }
            .unwrap_or_else(|| cursor.geometry(&shaped.layout, 0.0).x0 as f32);
            let rect = Rect::from_min_size(pos2(x, line.top), vec2(0.0, line.height));
            let selected = TextSelection {
                anchor: if mods.shift { selection.anchor } else { index },
                focus: index,
            };
            self.caret = Some(VisualCaret {
                owner: session.descriptor.owner,
                sequence: queue.edit_sequence + 1,
                row,
                line_start: line.start,
                line_text: line.text.clone(),
                cursor,
            });
            queue.edit(Intent::Select(selected), Some(rect));
            return Ok(true);
        }
        self.reset();
        if let Some(intent) = key_intent(session, key, mods)? {
            queue.edit(intent, None);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

use super::*;
use crate::gpu_ui::edit::model::byte_index;

fn key_intent(session: &mut Session, key: Key, mods: Modifiers) -> Result<Option<Intent>, String> {
    let command = if mods.command {
        match key {
            Key::A => Some(Intent::SelectAll),
            Key::Z if mods.shift => Some(Intent::Redo),
            Key::Z => Some(Intent::Undo),
            Key::Y => Some(Intent::Redo),
            _ => None,
        }
    } else {
        None
    };
    if command.is_some() {
        return Ok(command);
    }
    if key == Key::Enter {
        return Ok(if !session.descriptor.multiline {
            Some(Intent::Submit)
        } else if mods.command || mods.ctrl {
            None
        } else {
            Some(Intent::Replace {
                range: session.model.selection(),
                text: "\n".into(),
                group: Some(EditGroup::Typing),
            })
        });
    }
    if matches!(key, Key::ArrowLeft | Key::ArrowRight) {
        session.preferred_x = None;
        if cfg!(target_os = "macos") && mods.mac_cmd {
            return session.navigation(
                if key == Key::ArrowLeft { Key::Home } else { Key::End },
                Modifiers { shift: mods.shift, ..Modifiers::NONE },
            );
        }
        let word = if cfg!(target_os = "macos") { mods.alt } else { mods.ctrl };
        return Ok(Some(Intent::Select(horizontal(
            session,
            key == Key::ArrowRight,
            word,
            mods.shift,
        )?)));
    }
    if matches!(key, Key::Backspace | Key::Delete) {
        let backward = key == Key::Backspace;
        let word = if cfg!(target_os = "macos") { mods.alt } else { mods.ctrl };
        let mut range = session.model.selection();
        if range.range().is_empty() {
            range.focus = deletion_boundary(session.model.text(), range.focus, backward, word);
        }
        return Ok(Some(Intent::Replace {
            range,
            text: String::new(),
            group: Some(match (backward, word) {
                (true, false) => EditGroup::Backspace,
                (false, false) => EditGroup::Delete,
                (true, true) => EditGroup::BackwardWord,
                (false, true) => EditGroup::ForwardWord,
            }),
        }));
    }
    session.navigation(key, mods)
}

fn deletion_boundary(text: &str, focus: usize, backward: bool, word: bool) -> usize {
    let byte = byte_index(text, focus);
    let boundary = if word {
        if backward {
            text[..byte]
                .split_word_bound_indices()
                .rev()
                .find(|(_, part)| !part.trim().is_empty())
                .map_or(0, |(index, _)| index)
        } else {
            let suffix = &text[byte..];
            let mut end = byte;
            let mut found = false;
            for (index, part) in suffix.split_word_bound_indices() {
                if found && !part.trim().is_empty() {
                    break;
                }
                found |= !part.trim().is_empty();
                end = byte + index + part.len();
            }
            end
        }
    } else if backward {
        text[..byte].grapheme_indices(true).next_back().map_or(0, |(index, _)| index)
    } else {
        byte + text[byte..].graphemes(true).next().map_or(0, str::len)
    };
    text[..boundary].encode_utf16().count()
}

fn horizontal(
    session: &Session,
    forward: bool,
    word: bool,
    extend: bool,
) -> Result<TextSelection, String> {
    let geometry = session.descriptor.layout(session.model.text())?;
    let selection = session.model.selection();
    let position = |index| {
        let caret = geometry.caret(index);
        (caret.top(), caret.left())
    };
    let anchor = position(selection.anchor);
    let focus = position(selection.focus);
    if !extend && !word && !selection.range().is_empty() {
        return Ok(TextSelection::caret(if (anchor > focus) == forward {
            selection.anchor
        } else {
            selection.focus
        }));
    }
    let mut edges: Vec<_> = geometry
        .lines
        .iter()
        .flat_map(|line| {
            line.clusters.iter().flat_map(move |cluster| {
                [
                    (cluster.start, (line.top, cluster.leading.x)),
                    (cluster.end, (line.top, cluster.trailing.x)),
                ]
            })
        })
        .collect();
    edges.sort_by(|a, b| a.1.0.total_cmp(&b.1.0).then(a.1.1.total_cmp(&b.1.1)));
    if !forward {
        edges.reverse();
    }
    let text = session.model.text();
    let word_edges: std::collections::BTreeSet<_> = text
        .split_word_bound_indices()
        .filter(|(_, part)| !part.trim().is_empty())
        .flat_map(|(index, part)| {
            [
                text[..index].encode_utf16().count(),
                text[..index + part.len()].encode_utf16().count(),
            ]
        })
        .collect();
    let target = edges
        .iter()
        .filter(|(index, point)| {
            *index != selection.focus && if forward { *point > focus } else { *point < focus }
        })
        .find(|(index, _)| !word || word_edges.contains(index))
        .or_else(|| edges.last())
        .map_or(selection.focus, |(index, _)| *index);
    Ok(TextSelection { anchor: if extend { selection.anchor } else { target }, focus: target })
}

pub(super) fn ime(queue: &mut InputQueue, event: egui::ImeEvent) {
    match event {
        egui::ImeEvent::Preedit { text, active_range_chars } => {
            if text.is_empty() {
                if queue.composing {
                    queue.edit(Intent::CompositionEnd { cancelled: true }, None);
                }
                queue.composing = false;
                return;
            }
            if !queue.composing {
                queue.edit(Intent::CompositionStart, None);
            }
            queue.composing = true;
            let range = queue.session.as_ref().map_or(TextSelection::caret(0), |session| {
                session.model.composition.unwrap_or_else(|| session.model.selection())
            });
            let start = range.range().start;
            let selected = active_range_chars.map(|active| TextSelection {
                anchor: start + text.chars().take(active.start).map(char::len_utf16).sum::<usize>(),
                focus: start + text.chars().take(active.end).map(char::len_utf16).sum::<usize>(),
            });
            queue.edit_with_selection(Intent::Replace { range, text, group: None }, None, selected);
        }
        egui::ImeEvent::Commit(text) => {
            let range = queue.session.as_ref().map_or(TextSelection::caret(0), |session| {
                session.model.composition.unwrap_or_else(|| session.model.selection())
            });
            if !queue.composing {
                queue.edit(Intent::CompositionStart, None);
            }
            queue.edit(Intent::Replace { range, text, group: None }, None);
            queue.edit(Intent::CompositionEnd { cancelled: false }, None);
            queue.composing = false;
        }
        egui::ImeEvent::DeleteSurrounding { before_chars, after_chars } => {
            if let Some(session) = &queue.session {
                let text = session.model.text();
                let range = session.model.selection().range();
                let before = text[..byte_index(text, range.start)]
                    .chars()
                    .rev()
                    .take(before_chars)
                    .map(char::len_utf16)
                    .sum::<usize>();
                let after = text[byte_index(text, range.end)..]
                    .chars()
                    .take(after_chars)
                    .map(char::len_utf16)
                    .sum::<usize>();
                queue.edit(
                    Intent::Replace {
                        range: TextSelection {
                            anchor: range.start - before,
                            focus: range.end + after,
                        },
                        text: String::new(),
                        group: None,
                    },
                    None,
                );
            }
        }
        #[allow(deprecated)]
        egui::ImeEvent::Enabled | egui::ImeEvent::Disabled => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_ui::edit::session::tests::text_descriptor;

    fn queue(
        text: &str,
        selection: TextSelection,
    ) -> (crate::gpu_ui::fonts::PlatformFonts, InputQueue) {
        let (fonts, descriptor) = text_descriptor(text);
        let mut session = Session::new(descriptor);
        session.model.select(selection);
        (fonts, InputQueue { session: Some(session), ..Default::default() })
    }
    fn text(queue: &InputQueue) -> &str {
        queue.session.as_ref().unwrap().model.text()
    }
    fn preedit(queue: &mut InputQueue, text: &str) {
        ime(queue, egui::ImeEvent::Preedit { text: text.into(), active_range_chars: None });
    }

    #[test]
    fn cancelled_native_ime_restores_the_replaced_selection_without_an_undo_step() {
        let selection = TextSelection { anchor: 2, focus: 5 };
        let (_fonts, mut queue) = queue("A old B", selection);
        preedit(&mut queue, "に");
        assert_eq!(text(&queue), "A に B");
        preedit(&mut queue, "日本");
        assert_eq!(text(&queue), "A 日本 B");
        preedit(&mut queue, "");
        assert_eq!(text(&queue), "A old B");
        assert_eq!(queue.session.as_ref().unwrap().model.selection(), selection);
        queue.edit(Intent::Undo, None);
        assert_eq!(text(&queue), "A old B");
        assert!(!queue.composing);
    }

    #[test]
    fn native_ime_commit_is_one_undoable_replacement_even_after_preedit_clear() {
        let (_fonts, mut queue) = queue("A old B", TextSelection { anchor: 2, focus: 5 });
        preedit(&mut queue, "に");
        preedit(&mut queue, "日本");
        preedit(&mut queue, "");
        ime(&mut queue, egui::ImeEvent::Commit("日本語".into()));
        assert_eq!(text(&queue), "A 日本語 B");
        queue.edit(Intent::Undo, None);
        assert_eq!(text(&queue), "A old B");
        queue.edit(Intent::Redo, None);
        assert_eq!(text(&queue), "A 日本語 B");
        assert!(queue.session.as_ref().unwrap().model.composition.is_none());
    }

    #[test]
    fn ime_candidate_selection_counts_utf16_and_replaces_only_its_composition() {
        let (_fonts, mut queue) = queue("A!", TextSelection::caret(1));
        ime(
            &mut queue,
            egui::ImeEvent::Preedit { text: "😀語".into(), active_range_chars: Some(1..2) },
        );
        assert_eq!(text(&queue), "A😀語!");
        assert_eq!(
            queue.session.as_ref().unwrap().model.selection(),
            TextSelection { anchor: 3, focus: 4 }
        );
        preedit(&mut queue, "文");
        assert_eq!(text(&queue), "A文!");
        ime(&mut queue, egui::ImeEvent::Commit("文字".into()));
        assert_eq!(text(&queue), "A文字!");
    }

    #[test]
    fn horizontal_arrows_visit_the_interior_of_a_right_to_left_run() {
        let (_fonts, mut queue) = queue("ab אבג cd", TextSelection::caret(0));
        let mut positions = Vec::new();
        let mut editing = Editing::default();
        for _ in 0..12 {
            assert!(editing.key(&mut queue, Key::ArrowRight, Modifiers::NONE).unwrap());
            positions.push(queue.session.as_ref().unwrap().model.selection().focus);
        }
        assert!(positions.windows(2).any(|pair| pair == [5, 4]), "{positions:?}");
        assert_eq!(positions.last(), Some(&9));
        positions.clear();
        for _ in 0..12 {
            assert!(editing.key(&mut queue, Key::ArrowLeft, Modifiers::NONE).unwrap());
            positions.push(queue.session.as_ref().unwrap().model.selection().focus);
        }
        assert!(positions.windows(2).any(|pair| pair == [4, 5]), "{positions:?}");
        assert_eq!(positions.last(), Some(&0));
    }

    #[test]
    fn backspace_removes_complete_graphemes_and_retains_grouped_undo() {
        let original = "A👩‍👩‍👧‍👦e\u{301}";
        let (_fonts, mut queue) =
            queue(original, TextSelection::caret(original.encode_utf16().count()));
        for expected in ["A👩‍👩‍👧‍👦", "A"] {
            assert!(Editing::default().key(&mut queue, Key::Backspace, Modifiers::NONE).unwrap());
            assert_eq!(text(&queue), expected);
        }
        queue.edit(Intent::Undo, None);
        assert_eq!(text(&queue), original);
    }
}
