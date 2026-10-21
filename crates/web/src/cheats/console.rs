//! Source `CheatMenu.tsx` console: raw code goes to the actual VM; trimmed history
//! and the last100 returned executions belong only to this mounted browser view.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::menu::cheats::{MAX_LUA_CONSOLE_CODE_LENGTH, append_cheat_console_history};
use solomon_sim::modding::value::LuaValue;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Event, HtmlElement, HtmlTextAreaElement, KeyboardEvent};

use super::{UiEvent, button, format_values, text};
use crate::ui::{append, element};

struct PendingConsole {
    id: u64,
    code: String,
}

pub(super) struct ConsoleView {
    pub root: HtmlElement,
    editor: HtmlTextAreaElement,
    counter: HtmlElement,
    run: HtmlElement,
    clear: HtmlElement,
    output: HtmlElement,
    empty: HtmlElement,
    entries: Vec<HtmlElement>,
    history: Vec<String>,
    history_cursor: Option<usize>,
    history_draft: String,
    pending: Option<PendingConsole>,
    next_code: Option<String>,
    key: Closure<dyn FnMut(Event)>,
}

impl ConsoleView {
    pub fn new(document: &Document, events: &Rc<RefCell<Vec<UiEvent>>>) -> Result<Self, String> {
        let root = element(document, "div", "cheat-menu-console")?;
        root.set_id("cheat-menu-console");
        root.set_attribute("role", "tabpanel").map_err(|_| "Lua console role")?;
        root.set_attribute("aria-labelledby", "cheat-menu-console-tab")
            .map_err(|_| "Lua console description")?;
        let editor_box = element(document, "div", "cheat-menu-console-editor")?;
        let label = text(document, &editor_box, "label", "", "LUA")?;
        label.set_attribute("for", "cheat-menu-lua-editor").map_err(|_| "Lua editor label")?;
        let editor: HtmlTextAreaElement = element(document, "textarea", "")?
            .dyn_into()
            .map_err(|_| "Lua editor is not a textarea")?;
        editor.set_id("cheat-menu-lua-editor");
        editor.set_max_length(MAX_LUA_CONSOLE_CODE_LENGTH as i32);
        editor.set_spellcheck(false);
        editor.set_value("return sd.runtime.get_frame_state()");
        editor.set_attribute("data-cheat-field", "console").map_err(|_| "Lua editor field")?;
        append(&editor_box, editor.unchecked_ref())?;
        let footer = element(document, "footer", "")?;
        let counter = text(document, &footer, "span", "", "")?;
        let run = button(document, "RUN LUA", "run", "")?;
        append(&footer, &run)?;
        append(&editor_box, &footer)?;
        append(&root, &editor_box)?;
        let output = element(document, "section", "cheat-menu-console-output")?;
        output.set_attribute("aria-label", "Lua console output").map_err(|_| "Lua output label")?;
        let header = element(document, "header", "")?;
        text(document, &header, "strong", "", "OUTPUT")?;
        let clear = button(document, "CLEAR", "clear", "")?;
        clear.set_attribute("disabled", "").map_err(|_| "empty Lua output")?;
        append(&header, &clear)?;
        append(&output, &header)?;
        let empty = text(
            document,
            &output,
            "p",
            "",
            "Lua print output, return values, and errors appear here.",
        )?;
        append(&root, &output)?;

        let pending = events.clone();
        let key = Closure::new(move |event: Event| {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else { return };
            if key.key() == "Enter" && (key.ctrl_key() || key.meta_key()) {
                event.prevent_default();
                pending.borrow_mut().push(UiEvent::Action("run".to_owned()));
            } else if key.alt_key() && matches!(key.key().as_str(), "ArrowUp" | "ArrowDown") {
                event.prevent_default();
                pending.borrow_mut().push(UiEvent::History(if key.key() == "ArrowUp" {
                    -1
                } else {
                    1
                }));
            }
        });
        editor
            .add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref())
            .map_err(|_| "Lua console shortcuts")?;
        let view = Self {
            root,
            editor,
            counter,
            run,
            clear,
            output,
            empty,
            entries: Vec::new(),
            history: Vec::new(),
            history_cursor: None,
            history_draft: String::new(),
            pending: None,
            next_code: None,
            key,
        };
        view.update_editor()?;
        Ok(view)
    }

    pub fn focus(&self) -> Result<(), String> {
        self.editor.focus().map_err(|_| "focus Lua console".to_owned())
    }

    pub fn edited(&mut self) -> Result<(), String> {
        self.history_cursor = None;
        self.update_editor()
    }

    pub fn navigate_history(&mut self, direction: i32) -> Result<(), String> {
        if self.history.is_empty() {
            return Ok(());
        }
        if self.history_cursor.is_none() {
            self.history_draft = self.editor.value();
        }
        let next = match self.history_cursor {
            None if direction < 0 => self.history.len() - 1,
            None => self.history.len(),
            Some(cursor) if direction < 0 => cursor.saturating_sub(1),
            Some(cursor) => (cursor + 1).min(self.history.len()),
        };
        if next == self.history.len() {
            self.history_cursor = None;
            self.editor.set_value(&self.history_draft);
        } else {
            self.history_cursor = Some(next);
            self.editor.set_value(&self.history[next]);
        }
        self.update_editor()
    }

    pub fn prepare(&mut self) -> Result<Option<String>, String> {
        let raw = self.editor.value();
        let code: String = js_sys::JsString::from(raw.as_str()).trim().into();
        if code.is_empty() || self.pending.is_some() || self.next_code.is_some() {
            return Ok(None);
        }
        append_cheat_console_history(&mut self.history, &code);
        self.history_cursor = None;
        self.history_draft.clear();
        self.next_code = Some(code);
        self.update_editor()?;
        Ok(Some(raw))
    }

    pub fn set_request(&mut self, id: u64) {
        self.pending = Some(PendingConsole {
            id,
            code: self.next_code.take().expect("a prepared console request has source code"),
        });
    }

    pub fn complete(
        &mut self,
        document: &Document,
        id: u64,
        ok: bool,
        output: &[String],
        values: &[LuaValue],
        error: Option<&str>,
    ) -> Result<bool, String> {
        if !self.pending.as_ref().is_some_and(|pending| pending.id == id) {
            return Ok(false);
        }
        let pending = self.pending.take().expect("matched Lua console request");
        let article = element(document, "article", "")?;
        article
            .set_attribute("data-console-result", if ok { "ok" } else { "error" })
            .map_err(|_| "Lua result state")?;
        text(document, &article, "pre", "cheat-menu-console-source", &pending.code)?;
        for line in output {
            text(document, &article, "pre", "cheat-menu-console-print", line)?;
        }
        if ok {
            let returned = format_values(values)?;
            if !returned.is_empty() {
                text(document, &article, "pre", "cheat-menu-console-return", &returned)?;
            }
        } else {
            text(
                document,
                &article,
                "pre",
                "cheat-menu-console-error",
                error.unwrap_or("Lua execution failed."),
            )?;
        }
        // Source renders the newest result first; the sticky header stays first.
        let before = self.entries.last().unwrap_or(&self.empty);
        self.output.insert_before(&article, Some(before)).map_err(|_| "append Lua output")?;
        self.empty.set_hidden(true);
        self.entries.push(article);
        if self.entries.len() > 100 {
            self.entries.remove(0).remove();
        }
        self.clear.remove_attribute("disabled").map_err(|_| "enable clearing Lua output")?;
        self.update_editor()?;
        Ok(true)
    }

    pub fn clear(&mut self) -> Result<(), String> {
        for entry in self.entries.drain(..) {
            entry.remove();
        }
        self.empty.set_hidden(false);
        self.clear.set_attribute("disabled", "").map_err(|_| "empty Lua console output".to_owned())
    }

    fn update_editor(&self) -> Result<(), String> {
        let code = self.editor.value();
        let count = localized_count(code.encode_utf16().count())?;
        let maximum = localized_count(MAX_LUA_CONSOLE_CODE_LENGTH)?;
        self.counter.set_text_content(Some(&format!(
            "Ctrl/Cmd+Enter runs · Alt+↑/↓ history · {count} / {maximum}"
        )));
        let busy = self.pending.is_some() || self.next_code.is_some();
        let trimmed: String = js_sys::JsString::from(code.as_str()).trim().into();
        self.run
            .toggle_attribute_with_force("disabled", busy || trimmed.is_empty())
            .map_err(|_| "Lua execution admission")?;
        self.run.set_text_content(Some(if busy { "RUNNING…" } else { "RUN LUA" }));
        Ok(())
    }
}

impl Drop for ConsoleView {
    fn drop(&mut self) {
        self.editor
            .remove_event_listener_with_callback("keydown", self.key.as_ref().unchecked_ref())
            .expect("remove the installed Lua editor key listener");
    }
}

fn localized_count(count: usize) -> Result<String, String> {
    let formatter = js_sys::Intl::NumberFormat::new(&js_sys::Array::new(), &js_sys::Object::new());
    formatter
        .format()
        .call1(&JsValue::UNDEFINED, &JsValue::from_f64(count as f64))
        .map_err(|_| "format Lua code length")?
        .as_string()
        .ok_or_else(|| "Lua code length is not text".to_owned())
}
