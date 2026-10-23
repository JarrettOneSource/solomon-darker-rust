//! GPU Lua console from `CheatMenu.tsx`; raw code executes only in the actual host VM.
//! History and the last100 returned executions belong to this mounted view.

use super::{button, copy, format_values};
use crate::gpu_ui::UiFrame;
use egui::{Color32, FontFamily, FontId, Id, RichText, Stroke, vec2};
use solomon_sim::menu::cheats::{MAX_LUA_CONSOLE_CODE_LENGTH, append_cheat_console_history};
use solomon_sim::modding::value::LuaValue;
use wasm_bindgen::JsValue;

struct PendingConsole {
    id: u64,
    code: String,
}
struct ConsoleEntry {
    id: u64,
    code: String,
    output: Vec<String>,
    returned: String,
    error: Option<String>,
}
pub(super) struct ConsoleView {
    editor: String,
    entries: Vec<ConsoleEntry>,
    history: Vec<String>,
    history_cursor: Option<usize>,
    history_draft: String,
    pending: Option<PendingConsole>,
    next_code: Option<String>,
    focus: bool,
}
impl ConsoleView {
    pub fn new() -> Self {
        Self {
            editor: "return sd.runtime.get_frame_state()".into(),
            entries: Vec::new(),
            history: Vec::new(),
            history_cursor: None,
            history_draft: String::new(),
            pending: None,
            next_code: None,
            focus: false,
        }
    }
    pub fn focus(&mut self) {
        self.focus = true;
    }
    pub fn navigate_history(&mut self, direction: i32) -> Result<(), String> {
        if self.history.is_empty() {
            return Ok(());
        }
        if self.history_cursor.is_none() {
            self.history_draft = self.editor.clone();
        }
        let next = match self.history_cursor {
            None if direction < 0 => self.history.len() - 1,
            None => self.history.len(),
            Some(cursor) if direction < 0 => cursor.saturating_sub(1),
            Some(cursor) => (cursor + 1).min(self.history.len()),
        };
        if next == self.history.len() {
            self.history_cursor = None;
            self.editor = self.history_draft.clone();
        } else {
            self.history_cursor = Some(next);
            self.editor = self.history[next].clone();
        }
        Ok(())
    }
    pub fn prepare(&mut self) -> Result<Option<String>, String> {
        let raw = self.editor.clone();
        let code: String = js_sys::JsString::from(raw.as_str()).trim().into();
        if code.is_empty() || self.pending.is_some() || self.next_code.is_some() {
            return Ok(None);
        }
        append_cheat_console_history(&mut self.history, &code);
        self.history_cursor = None;
        self.history_draft.clear();
        self.next_code = Some(code);
        Ok(Some(raw))
    }
    pub fn set_request(&mut self, id: u64) {
        self.pending = Some(PendingConsole {
            id,
            code: self.next_code.take().expect("prepared console request has code"),
        });
    }
    pub fn complete(
        &mut self,
        id: u64,
        ok: bool,
        output: &[String],
        values: &[LuaValue],
        error: Option<&str>,
    ) -> Result<bool, String> {
        if !self.pending.as_ref().is_some_and(|pending| pending.id == id) {
            return Ok(false);
        }
        let pending = self.pending.take().expect("matched console request");
        self.entries.push(ConsoleEntry {
            id,
            code: pending.code,
            output: output.to_vec(),
            returned: if ok { format_values(values)? } else { String::new() },
            error: if ok {
                None
            } else {
                Some(error.unwrap_or("Lua execution failed.").to_owned())
            },
        });
        if self.entries.len() > 100 {
            self.entries.remove(0);
        }
        Ok(true)
    }
    pub fn clear(&mut self) -> Result<(), String> {
        self.entries.clear();
        Ok(())
    }
    pub fn paint(
        &mut self,
        gpu: &UiFrame<'_>,
        ui: &mut egui::Ui,
    ) -> Result<Option<String>, String> {
        let mut action = None;
        let focused = ui.memory(|memory| memory.has_focus(Id::unique("cheats/lua-editor")));
        if focused {
            if ui.input_mut(|input| {
                input.consume_key(egui::Modifiers::CTRL, egui::Key::Enter)
                    || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
            }) {
                action = Some("run".into());
            }
            let history = ui.input_mut(|input| {
                if input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp) {
                    -1
                } else if input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown) {
                    1
                } else {
                    0
                }
            });
            if history != 0 {
                self.navigate_history(history)?;
            }
        }
        ui.label(copy("LUA", 11.0));
        let response = ui.add_sized(
            vec2(ui.available_width(), 150.0),
            egui::TextEdit::multiline(&mut self.editor)
                .id(Id::unique("cheats/lua-editor"))
                .code_editor()
                .font(FontId::new(13.0, FontFamily::Name("JetBrains Mono".into())))
                .text_color(Color32::from_rgb(229, 225, 211))
                .background_color(Color32::from_rgb(5, 6, 7))
                .desired_width(f32::INFINITY),
        );
        gpu.record("cheats/lua-editor", "LUA", "textbox", &response);
        gpu.record_value("cheats/lua-editor", &self.editor, None);
        if self.focus {
            response.request_focus();
            self.focus = false;
        }
        if response.changed() {
            self.history_cursor = None;
            truncate_utf16(&mut self.editor, MAX_LUA_CONSOLE_CODE_LENGTH);
        }
        let busy = self.pending.is_some() || self.next_code.is_some();
        let count = localized_count(self.editor.encode_utf16().count())?;
        let max = localized_count(MAX_LUA_CONSOLE_CODE_LENGTH)?;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Ctrl/Cmd+Enter runs · Alt+↑/↓ history · {count} / {max}"))
                    .font(FontId::new(10.0, FontFamily::Name("JetBrains Mono".into())))
                    .color(Color32::from_rgb(169, 156, 118)),
            );
            if button(
                gpu,
                ui,
                "run",
                if busy { "RUNNING…" } else { "RUN LUA" },
                !busy && !self.editor.trim().is_empty(),
            )
            .clicked()
            {
                action = Some("run".into());
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(copy("OUTPUT", 13.0));
            if button(gpu, ui, "clear", "CLEAR", !self.entries.is_empty()).clicked() {
                action = Some("clear".into());
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("cheats/console-output")
            .max_height(ui.available_height().max(80.0))
            .show(ui, |ui| -> Result<(), String> {
                if self.entries.is_empty() {
                    ui.label(copy(
                        "Lua print output, return values, and errors appear here.",
                        12.0,
                    ));
                }
                for entry in self.entries.iter().rev() {
                    let result = egui::Frame::new()
                        .fill(Color32::from_rgb(6, 7, 8))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(75, 65, 42)))
                        .inner_margin(9.0)
                        .show(ui, |ui| -> Result<(), String> {
                            console_line(
                                gpu,
                                ui,
                                &format!("cheats/console-code:{}", entry.id),
                                &entry.code,
                                Color32::from_rgb(190, 182, 154),
                            )?;
                            for (index, line) in entry.output.iter().enumerate() {
                                console_line(
                                    gpu,
                                    ui,
                                    &format!("cheats/console-print:{}:{index}", entry.id),
                                    line,
                                    Color32::from_rgb(210, 211, 202),
                                )?;
                            }
                            if !entry.returned.is_empty() {
                                console_line(
                                    gpu,
                                    ui,
                                    &format!("cheats/console-return:{}", entry.id),
                                    &entry.returned,
                                    Color32::from_rgb(147, 202, 169),
                                )?;
                            }
                            if let Some(error) = &entry.error {
                                console_line(
                                    gpu,
                                    ui,
                                    &format!("cheats/console-error:{}", entry.id),
                                    error,
                                    Color32::from_rgb(255, 142, 120),
                                )?;
                            }
                            Ok(())
                        });
                    result.inner?;
                    gpu.record(
                        &format!("cheats/console-result:{}", entry.id),
                        &format!(
                            "{}\n{}\n{}",
                            entry.code,
                            entry.output.join("\n"),
                            entry.error.as_deref().unwrap_or(&entry.returned)
                        ),
                        if entry.error.is_some() { "alert" } else { "log" },
                        &result.response,
                    );
                }
                Ok(())
            })
            .inner?;
        Ok(action)
    }
}
fn console_line(
    gpu: &UiFrame<'_>,
    ui: &mut egui::Ui,
    id: &str,
    text: &str,
    color: Color32,
) -> Result<(), String> {
    let mut style = crate::gpu_ui::NativeLabelStyle::new(
        FontId::new(12.0, FontFamily::Name("JetBrains Mono".into())),
        color,
    );
    style.wrap_width = Some(ui.available_width());
    crate::gpu_ui::shaped_label(gpu, ui, id, text, style)?;
    Ok(())
}
fn localized_count(count: usize) -> Result<String, String> {
    let formatter = js_sys::Intl::NumberFormat::new(&js_sys::Array::new(), &js_sys::Object::new());
    formatter
        .format()
        .call1(&JsValue::UNDEFINED, &JsValue::from_f64(count as f64))
        .map_err(|_| "format Lua code length")?
        .as_string()
        .ok_or_else(|| "Lua code length is not text".into())
}
fn truncate_utf16(text: &mut String, maximum: usize) {
    let mut units = 0;
    for (index, character) in text.char_indices() {
        units += character.len_utf16();
        if units > maximum {
            text.truncate(index);
            return;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::truncate_utf16;
    #[test]
    fn lua_editor_limit_counts_utf16_without_cutting_a_unicode_character() {
        let mut value = "A😀βZ".to_owned();
        truncate_utf16(&mut value, 4);
        assert_eq!(value, "A😀β");
        let mut value = "A😀βZ".to_owned();
        truncate_utf16(&mut value, 2);
        assert_eq!(value, "A");
    }
}
