//! OS text ranges and positions projected from the visible editor's shaped layout.

use std::collections::BTreeSet;

use egui::accesskit::{Action, Node, Role, TextDirection, TextPosition, TextSelection, TreeUpdate};
use egui::{Id, Rect, pos2, vec2};
use unicode_segmentation::UnicodeSegmentation;

use super::{EditorDescriptor, TextSelection as ModelSelection, model::slice_utf16};

// AccessKit word-start indices are u8. Split long lines without losing text.
const RUN_CHARACTERS: usize = u8::MAX as usize;

struct TextRun {
    id: Id,
    row: usize,
    value: String,
    lengths: Vec<u8>,
    offsets: Vec<usize>,
    rectangles: Vec<Rect>,
    bounds: Rect,
    rtl: bool,
}

impl TextRun {
    fn end(&self) -> usize {
        *self.offsets.last().expect("a text run retains its initial position")
    }
}

pub(in crate::gpu_ui) fn update(update: &mut TreeUpdate, editors: &[EditorDescriptor]) {
    for editor in editors {
        update_editor(update, editor);
    }
}

fn update_editor(update: &mut TreeUpdate, editor: &EditorDescriptor) {
    let Some((_, parent)) =
        update.nodes.iter_mut().find(|(id, _)| *id == editor.owner.id.accesskit_id())
    else {
        return;
    };
    parent.set_role(if editor.multiline { Role::MultilineTextInput } else { Role::TextInput });
    parent.set_value(editor.text.as_ref());
    if editor.enabled {
        parent.add_action(Action::SetTextSelection);
    }
    if editor.enabled && !editor.readonly {
        parent.add_action(Action::SetValue);
        parent.add_action(Action::ReplaceSelectedText);
    } else {
        parent.set_read_only();
    }
    let runs = text_runs(editor);
    let selection = editor.model.selection();
    parent.set_children(runs.iter().map(|run| run.id.accesskit_id()).collect::<Vec<_>>());
    parent.set_text_selection(TextSelection {
        anchor: position(editor, &runs, selection.anchor),
        focus: position(editor, &runs, selection.focus),
    });
    let mut words = BTreeSet::new();
    let mut offset = 0;
    for part in editor.text.split_word_bounds() {
        if !part.trim().is_empty() {
            words.insert(offset);
        }
        offset += part.encode_utf16().count();
    }
    for (index, run) in runs.iter().enumerate() {
        let mut node = Node::new(Role::TextRun);
        node.set_text_direction(if run.rtl {
            TextDirection::RightToLeft
        } else {
            TextDirection::LeftToRight
        });
        node.set_value(&run.value);
        let bounds = Rect::from_min_max(
            editor.text_origin + run.bounds.min.to_vec2() * editor.scale,
            editor.text_origin + run.bounds.max.to_vec2() * editor.scale,
        );
        node.set_bounds(egui::accesskit::Rect {
            x0: bounds.min.x.into(),
            y0: bounds.min.y.into(),
            x1: bounds.max.x.into(),
            y1: bounds.max.y.into(),
        });
        node.set_character_lengths(run.lengths.clone());
        node.set_character_positions(
            run.rectangles
                .iter()
                .map(|rect| {
                    (if run.rtl {
                        run.bounds.right() - rect.right()
                    } else {
                        rect.left() - run.bounds.left()
                    }) * editor.scale
                })
                .collect::<Vec<_>>(),
        );
        node.set_character_widths(
            run.rectangles.iter().map(|rect| rect.width() * editor.scale).collect::<Vec<_>>(),
        );
        node.set_word_starts(
            run.offsets[..run.offsets.len() - 1]
                .iter()
                .enumerate()
                .filter_map(|(index, offset)| words.contains(offset).then_some(index as u8))
                .collect::<Vec<_>>(),
        );
        if let Some(previous) = index.checked_sub(1).map(|i| &runs[i]).filter(|r| r.row == run.row)
        {
            node.set_previous_on_line(previous.id.accesskit_id());
        }
        if let Some(next) = runs.get(index + 1).filter(|r| r.row == run.row) {
            node.set_next_on_line(next.id.accesskit_id());
        }
        update.nodes.push((run.id.accesskit_id(), node));
    }
}

/// Resolve the OS's run-relative character indices through the same projection.
/// A stale or foreign run cannot select another field's text.
pub(in crate::gpu_ui) fn selection(
    editor: &EditorDescriptor,
    selection: &TextSelection,
) -> Option<ModelSelection> {
    let runs = text_runs(editor);
    let offset = |position: TextPosition| {
        runs.iter()
            .find(|run| run.id.accesskit_id() == position.node)?
            .offsets
            .get(position.character_index)
            .copied()
    };
    Some(ModelSelection { anchor: offset(selection.anchor)?, focus: offset(selection.focus)? })
}

fn position(editor: &EditorDescriptor, runs: &[TextRun], offset: usize) -> TextPosition {
    // Keep the editor's preceding-row affinity at a soft wrap, but use the next
    // character's leading edge where text direction changes within that row.
    let row = editor
        .geometry
        .lines
        .iter()
        .position(|line| offset >= line.start && offset <= line.end)
        .unwrap_or(editor.geometry.lines.len() - 1);
    let run =
        runs.iter().filter(|run| run.row == row).find(|run| offset < run.end()).unwrap_or_else(
            || runs.iter().rfind(|run| run.row == row).expect("text has an empty row"),
        );
    TextPosition {
        node: run.id.accesskit_id(),
        character_index: run
            .offsets
            .partition_point(|start| *start < offset)
            .min(run.offsets.len() - 1),
    }
}

fn text_runs(editor: &EditorDescriptor) -> Vec<TextRun> {
    let mut runs = Vec::new();
    for (row, line) in editor.geometry.lines.iter().enumerate() {
        let newline = editor.geometry.lines.get(row + 1).is_some_and(|next| next.start > line.end);
        let mut offset = line.start;
        let mut chunk = 0;
        let mut run = TextRun {
            id: editor.owner.id.with("accesskit-text").with(row).with(chunk),
            row,
            value: String::new(),
            lengths: Vec::new(),
            offsets: vec![offset],
            rectangles: Vec::new(),
            bounds: Rect::NOTHING,
            rtl: false,
        };
        let text = slice_utf16(&editor.text, line.start..line.end + usize::from(newline));
        // The layout and editor navigate graphemes, including CRLF as one break.
        // AccessKit's byte lengths are u8; only an oversized grapheme needs to be
        // represented in smaller UTF-8 chunks to preserve all of its text.
        let characters = text.graphemes(true).flat_map(|mut remaining| {
            std::iter::from_fn(move || {
                if remaining.is_empty() {
                    return None;
                }
                let boundary = remaining.floor_char_boundary(usize::from(u8::MAX));
                let (character, rest) = remaining.split_at(boundary);
                remaining = rest;
                Some(character)
            })
        });
        for character in characters {
            let cluster = line
                .clusters
                .get(line.clusters.partition_point(|c| c.end <= offset))
                .filter(|c| c.start <= offset);
            let (rect, rtl) = if let Some(cluster) = cluster {
                (
                    cluster.rects.iter().copied().reduce(Rect::union).unwrap_or_else(|| {
                        Rect::from_min_size(
                            pos2(cluster.leading.x, line.top),
                            vec2(0.0, line.height),
                        )
                    }),
                    cluster.leading.x > cluster.trailing.x,
                )
            } else {
                let x = line
                    .carets
                    .iter()
                    .find(|caret| caret.index == offset)
                    .map_or(line.width, |caret| caret.rect.left());
                (Rect::from_min_size(pos2(x, line.top), vec2(0.0, line.height)), run.rtl)
            };
            if !run.value.is_empty() && (run.rtl != rtl || run.rectangles.len() == RUN_CHARACTERS) {
                runs.push(run);
                chunk += 1;
                run = TextRun {
                    id: editor.owner.id.with("accesskit-text").with(row).with(chunk),
                    row,
                    value: String::new(),
                    lengths: Vec::new(),
                    offsets: vec![offset],
                    rectangles: Vec::new(),
                    bounds: Rect::NOTHING,
                    rtl,
                };
            }
            run.rtl = rtl;
            run.value.push_str(character);
            offset += character.encode_utf16().count();
            run.lengths.push(character.len() as u8);
            run.offsets.push(offset);
            run.rectangles.push(rect);
            run.bounds = run.bounds.union(rect);
        }
        if run.value.is_empty() {
            run.bounds = Rect::from_min_size(pos2(0.0, line.top), vec2(0.0, line.height));
        }
        runs.push(run);
    }
    runs
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use egui::accesskit::{TreeId, TreeInfo};

    use super::*;
    use crate::gpu_ui::edit::session::tests::text_descriptor;

    #[test]
    fn accessible_caret_matches_the_visible_caret_at_bidi_boundaries() {
        let (_fonts, mut editor) = text_descriptor("abאב");
        let id = editor.owner.id.accesskit_id();
        for offset in 0..=4 {
            Arc::make_mut(&mut editor.model).select(ModelSelection::caret(offset));
            let mut tree_update = TreeUpdate {
                nodes: vec![(id, Node::new(Role::TextInput))],
                tree: Some(TreeInfo::new(id)),
                tree_id: TreeId::ROOT,
                focus: id,
            };
            update(&mut tree_update, std::slice::from_ref(&editor));
            let tree = accesskit_consumer::Tree::new(tree_update, true);
            let node = tree.state().node_by_tree_local_id(id, TreeId::ROOT).unwrap();
            let selection = node.text_selection().unwrap();
            assert_eq!(selection.start().to_global_utf16_index(), offset);
            let actual = selection.bounding_boxes();
            let expected =
                editor.text_origin.x + editor.geometry.caret(offset).left() * editor.scale;
            assert_eq!(actual.len(), 1);
            assert!(
                (actual[0].x0 - f64::from(expected)).abs() < 0.01,
                "caret {offset}: visible x={expected}, accessible={:?}",
                actual[0]
            );
        }
    }

    #[test]
    fn oversized_graphemes_retain_complete_utf8_text() {
        let text = format!("e{}", "\u{301}".repeat(200));
        let (_fonts, editor) = text_descriptor(&text);
        let id = editor.owner.id.accesskit_id();
        let mut tree_update = TreeUpdate {
            nodes: vec![(id, Node::new(Role::TextInput))],
            tree: Some(TreeInfo::new(id)),
            tree_id: TreeId::ROOT,
            focus: id,
        };
        update(&mut tree_update, &[editor]);
        let tree = accesskit_consumer::Tree::new(tree_update, true);
        let node = tree.state().node_by_tree_local_id(id, TreeId::ROOT).unwrap();
        let range = node.document_range();
        assert_eq!(range.text(), text);
        assert_eq!(range.end().to_global_utf16_index(), 201);
    }
}
