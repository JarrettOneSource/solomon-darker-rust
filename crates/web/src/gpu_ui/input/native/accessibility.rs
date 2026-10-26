use egui::accesskit::{Action, ActionData, ActionRequest, TreeId};

use super::super::{InputQueue, Session};
use crate::gpu_ui::edit::{TextSelection, model::Intent};

/// Assistive technology edits the same model, rules, undo history and ordered
/// receipts as the keyboard. SetValue can target an editor without stealing focus.
pub(super) fn edit_action(
    queue: &mut InputQueue,
    ctx: &egui::Context,
    request: &ActionRequest,
) -> bool {
    if request.target_tree != TreeId::ROOT {
        return false;
    }
    let Some(editor) = queue
        .editors
        .iter()
        .find(|editor| editor.enabled && editor.owner.id.accesskit_id() == request.target_node)
        .cloned()
    else {
        return false;
    };
    if matches!(request.action, Action::Focus | Action::Click) {
        ctx.memory_mut(|memory| memory.request_focus(editor.owner.id));
        if queue.session.as_ref().is_none_or(|session| session.descriptor.owner != editor.owner) {
            queue.session = Some(Session::new(editor));
        }
        return true;
    }
    if !matches!(request.action, Action::SetValue | Action::ReplaceSelectedText) {
        return false;
    }
    let Some(ActionData::Value(text)) = &request.data else {
        return true;
    };
    if editor.readonly {
        return true;
    }
    let same_owner =
        queue.session.as_ref().is_some_and(|session| session.descriptor.owner == editor.owner);
    let previous = if same_owner { None } else { queue.session.replace(Session::new(editor)) };
    let model = &queue.session.as_ref().expect("targeted editor session").model;
    let range = if request.action == Action::SetValue {
        TextSelection { anchor: 0, focus: model.text().encode_utf16().count() }
    } else {
        model.selection()
    };
    queue.edit(Intent::Replace { range, text: text.to_string(), group: None }, None);
    if !same_owner {
        queue.session = previous;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_ui::edit::{model::Model, session::tests::text_descriptor};

    #[test]
    fn accessibility_can_replace_an_unfocused_field_without_bypassing_its_edit_rules() {
        let ctx = egui::Context::default();
        let (_fonts, mut editor) = text_descriptor("before");
        editor.focused = false;
        let mut queue = InputQueue { editors: vec![editor.clone()], ..Default::default() };
        let request = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: editor.owner.id.accesskit_id(),
            data: Some(ActionData::Value("New 😀 text".into())),
        };
        assert!(edit_action(&mut queue, &ctx, &request));
        assert!(queue.session.is_none());
        assert!(!ctx.memory(|memory| memory.has_focus(editor.owner.id)));
        assert_eq!(queue.edits[0].text.as_ref(), "New 😀 text");
        assert_eq!(queue.edits[0].owner, Some(editor.owner));
        assert_eq!(queue.edits[0].selection, TextSelection::caret(11));
        queue.editors[0].readonly = true;
        assert!(edit_action(&mut queue, &ctx, &request));
        assert_eq!(queue.edits.len(), 1);
    }

    #[test]
    fn accessibility_click_focuses_the_real_editor_and_keeps_undo_for_replacement() {
        let ctx = egui::Context::default();
        let mut model = Model::new("before");
        model.select(TextSelection { anchor: 0, focus: 6 });
        let (_fonts, mut editor) = text_descriptor("before");
        editor.model = std::sync::Arc::new(model);
        let mut queue = InputQueue { editors: vec![editor.clone()], ..Default::default() };
        let mut request = ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: editor.owner.id.accesskit_id(),
            data: None,
        };
        assert!(edit_action(&mut queue, &ctx, &request));
        assert!(ctx.memory(|memory| memory.has_focus(editor.owner.id)));
        request.action = Action::ReplaceSelectedText;
        request.data = Some(ActionData::Value("after".into()));
        assert!(edit_action(&mut queue, &ctx, &request));
        assert_eq!(queue.session.as_ref().unwrap().model.text(), "after");
        queue.edit(Intent::Undo, None);
        assert_eq!(queue.session.as_ref().unwrap().model.text(), "before");
    }
}
