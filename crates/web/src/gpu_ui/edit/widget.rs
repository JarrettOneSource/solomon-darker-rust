//! One visible GPU editor. Native font geometry owns text, selection and caret.

use std::sync::{Arc, Mutex};

use egui::{Color32, FontId, Id, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2};

use super::model::Model;
use super::{
    EditorDescriptor, EditorFont, EditorRules, FontRunOptions, TextSelection, font_run,
    font_run_with_options, native_name_geometry,
};
use crate::gpu_ui::UiFrame;

#[derive(Clone)]
struct State {
    model: Model,
    offset: Vec2,
    caret_since: f64,
    pointer_caret: Option<Rect>,
    transport: Option<Arc<Model>>,
}

pub struct NativeEditOutput {
    pub response: Response,
    pub submitted: bool,
    pub rejected: Option<String>,
}

impl std::ops::Deref for NativeEditOutput {
    type Target = Response;
    fn deref(&self) -> &Response {
        &self.response
    }
}

/// Source field policy is supplied here once and reused synchronously by the
/// native input transport. There is no independent browser control renderer.
pub struct NativeTextEdit<'a> {
    text: &'a mut String,
    id: &'a str,
    label: &'a str,
    font: EditorFont,
    color: Color32,
    margin: Vec2,
    rules: EditorRules,
    multiline: bool,
    code_editor: bool,
    readonly: bool,
    enabled: bool,
    request_focus: bool,
    select_all: bool,
    hint: String,
    background: Option<Color32>,
    line_height: Option<f32>,
    tab_size: Option<f32>,
}

impl<'a> NativeTextEdit<'a> {
    pub fn singleline(text: &'a mut String, id: &'a str, label: &'a str) -> Self {
        Self {
            text,
            id,
            label,
            font: EditorFont::Browser(FontId::proportional(16.0)),
            color: Color32::WHITE,
            margin: vec2(4.0, 2.0),
            rules: EditorRules::default(),
            multiline: false,
            code_editor: false,
            readonly: false,
            enabled: true,
            request_focus: false,
            select_all: false,
            hint: String::new(),
            background: None,
            line_height: None,
            tab_size: None,
        }
    }
    pub fn multiline(text: &'a mut String, id: &'a str, label: &'a str) -> Self {
        Self { multiline: true, ..Self::singleline(text, id, label) }
    }
    pub fn font(mut self, font: FontId) -> Self {
        self.font = EditorFont::Browser(font);
        self
    }
    pub fn native_name(mut self, origin: Pos2, scale: f32) -> Self {
        self.font = EditorFont::NativeName { origin, scale, field: Rect::NOTHING };
        self
    }
    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }
    pub fn margin(mut self, margin: Vec2) -> Self {
        self.margin = margin;
        self
    }
    pub fn rules(mut self, rules: EditorRules) -> Self {
        self.rules = rules;
        self
    }
    pub fn code_editor(mut self, enabled: bool) -> Self {
        self.code_editor = enabled;
        self
    }
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn request_focus(mut self, focus: bool) -> Self {
        self.request_focus = focus;
        self
    }
    pub fn select_all(mut self, select: bool) -> Self {
        self.select_all = select;
        self
    }
    pub fn hint_text(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }
    pub fn background(mut self, color: Color32) -> Self {
        self.background = Some(color);
        self
    }

    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = Some(height);
        self
    }
    pub fn tab_size(mut self, spaces: f32) -> Self {
        self.tab_size = Some(spaces);
        self
    }

    pub fn show(self, frame: &UiFrame<'_>, ui: &mut Ui, rect: Rect) -> NativeEditOutput {
        let id = Id::unique(self.id);
        let enabled = self.enabled && ui.is_enabled();
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        if !enabled {
            child.disable();
        }
        let mut response = child.interact(rect, id, Sense::click_and_drag());
        if self.request_focus && enabled {
            response.request_focus();
        }
        let has_focus = response.has_focus() && enabled;
        if has_focus {
            frame.ctx.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        tab: false,
                        escape: false,
                    },
                )
            });
        }
        let key = id.with("native-editor-state");
        let state = frame.ctx.data_mut(|data| {
            data.get_temp::<Arc<Mutex<State>>>(key).unwrap_or_else(|| {
                let state = Arc::new(Mutex::new(State {
                    model: Model::new(self.text),
                    offset: Vec2::ZERO,
                    caret_since: 0.0,
                    pointer_caret: None,
                    transport: None,
                }));
                data.insert_temp(key, state.clone());
                state
            })
        });
        let mut state = state.lock().expect("native editor state");
        if state.model.text() != self.text.as_str() {
            state.pointer_caret = None;
        }
        state.model.reconcile(self.text);
        if self.select_all && enabled {
            let units = self.text.encode_utf16().count();
            state.model.select(TextSelection { anchor: 0, focus: units });
        }
        let before = self.text.clone();
        let mut submitted = false;
        let mut rejected = None;
        if enabled {
            for receipt in frame
                .editor_receipts
                .iter()
                .filter(|receipt| receipt.id == Some(id) || receipt.id.is_none() && has_focus)
            {
                let change = state.model.apply(receipt, self.rules, self.readonly);
                state.pointer_caret =
                    receipt.caret.filter(|_| receipt.text.as_ref() == state.model.text());
                submitted |= change.submitted;
                if let Some(reason) = change.rejected {
                    rejected = Some(reason);
                }
                if let Some(copied) = change.copied.filter(|_| !receipt.clipboard_handled) {
                    frame.ctx.copy_text(copied);
                }
                state.caret_since = frame.ctx.input(|input| input.time);
            }
        }
        *self.text = state.model.text().to_owned();
        if *self.text != before {
            response.mark_changed();
        }
        let content = rect.shrink2(self.margin);
        let mut font = self.font.clone();
        if let EditorFont::NativeName { field, .. } = &mut font {
            *field = content;
        }
        let result = self.paint_and_register(frame, &child, &response, &mut state, content, font);
        if let Err(error) = result {
            frame.fail(error);
        }
        frame.record(self.id, self.label, "textbox", &response);
        frame.record_value(self.id, self.text, None);
        NativeEditOutput { response, submitted, rejected }
    }

    fn paint_and_register(
        &self,
        frame: &UiFrame<'_>,
        ui: &Ui,
        response: &Response,
        state: &mut State,
        content: Rect,
        font: EditorFont,
    ) -> Result<(), String> {
        let (geometry, run) = match &font {
            EditorFont::Browser(font) => {
                let run = font_run_with_options(
                    frame.ctx,
                    font.clone(),
                    self.color,
                    self.text,
                    FontRunOptions {
                        wrap_width: self.multiline.then_some(content.width()),
                        line_height: self.line_height,
                        tab_size: self.tab_size,
                    },
                )?;
                (run.geometry.clone(), Some(run))
            }
            EditorFont::NativeName { origin, scale, .. } => {
                (native_name_geometry(self.text, *origin, *scale, content), None)
            }
        };
        let has_focus = response.has_focus() && response.enabled();
        let selection = state.model.selection();
        let caret = state.pointer_caret.unwrap_or_else(|| geometry.caret(selection.focus));
        let changed_or_focused = response.changed() || self.request_focus || self.select_all;
        if changed_or_focused || has_focus && !frame.editor_receipts.is_empty() {
            if matches!(font, EditorFont::Browser(_)) {
                for (offset, min, max, viewport) in [
                    (&mut state.offset.x, caret.min.x, caret.max.x + 2.0, content.width()),
                    (&mut state.offset.y, caret.min.y, caret.max.y, content.height()),
                ] {
                    if min < *offset {
                        *offset = min;
                    }
                    if max > *offset + viewport {
                        *offset = max - viewport;
                    }
                    *offset = offset.max(0.0);
                }
            }
        }
        if self.multiline && response.hovered() {
            let delta = ui.input(|input| input.smooth_scroll_delta);
            if delta != Vec2::ZERO {
                state.offset.y = (state.offset.y - delta.y)
                    .clamp(0.0, (geometry.size.y - content.height()).max(0.0));
                ui.input_mut(|input| input.smooth_scroll_delta = Vec2::ZERO);
            }
        }
        let origin = content.min - state.offset;
        let painter = ui.painter().with_clip_rect(content.intersect(ui.clip_rect()));
        if let Some(background) = self.background {
            ui.painter().rect_filled(response.rect, 0.0, background);
        }
        let transform = frame.ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default();
        if ui.is_visible() {
            if has_focus {
                for (index, selected) in geometry.selection_rects(selection).into_iter().enumerate()
                {
                    let selected = selected.translate(origin.to_vec2());
                    painter.rect_filled(
                        selected,
                        0.0,
                        Color32::from_rgba_unmultiplied(65, 105, 180, 150),
                    );
                    frame.record_rect(
                        &format!("{}/selection:{index}", self.id),
                        "Text selection",
                        "selection",
                        transform * selected.intersect(painter.clip_rect()),
                        false,
                    );
                }
            }
            if let Some(run) = run {
                run.paint(&painter, origin, if response.enabled() { 1.0 } else { 0.5 })?;
                if self.text.is_empty() && !self.hint.is_empty() {
                    let EditorFont::Browser(font) = &font else { unreachable!() };
                    font_run(
                        frame.ctx,
                        font.clone(),
                        self.color.gamma_multiply(0.5),
                        &self.hint,
                        self.multiline.then_some(content.width()),
                    )?
                    .paint(&painter, origin, 1.0)?;
                }
            } else if let EditorFont::NativeName { origin, scale, .. } = &font {
                paint_native_name(frame, &painter, self.text, *origin, *scale);
            }
            if has_focus {
                if let Some(composition) = state.model.composition {
                    for (index, selected) in
                        geometry.selection_rects(composition).into_iter().enumerate()
                    {
                        let selected = selected.translate(origin.to_vec2());
                        painter.line_segment(
                            [selected.left_bottom(), selected.right_bottom()],
                            Stroke::new(1.0, self.color),
                        );
                        frame.record_rect(
                            &format!("{}/composition:{index}", self.id),
                            "Composition",
                            "composition",
                            transform * selected.intersect(painter.clip_rect()),
                            false,
                        );
                    }
                }
                let caret = caret.translate(origin.to_vec2());
                let blink = (frame.ctx.input(|input| input.time) - state.caret_since)
                    .rem_euclid(1.0)
                    < 0.55;
                if blink {
                    painter.line_segment(
                        [caret.left_top(), caret.left_bottom()],
                        Stroke::new(1.0, self.color),
                    );
                    frame.record_rect(
                        &format!("{}/caret", self.id),
                        "Text caret",
                        "caret",
                        transform * caret.expand2(vec2(0.5, 0.0)).intersect(painter.clip_rect()),
                        false,
                    );
                }
                frame.ctx.request_repaint_after(std::time::Duration::from_millis(250));
                frame.ctx.output_mut(|output| {
                    output.ime = Some(egui::IMEOutput {
                        purpose: egui::IMEPurpose::Normal,
                        rect: transform * content,
                        cursor_rect: transform * caret,
                        should_interrupt_composition: false,
                    })
                });
            }
            if state.transport.as_ref().is_none_or(|model| {
                model.sequence != state.model.sequence
                    || model.text() != state.model.text()
                    || model.selection() != state.model.selection()
                    || model.composition != state.model.composition
            }) {
                state.transport = Some(Arc::new(state.model.clone()));
            }
            frame.editors.borrow_mut().push(EditorDescriptor {
                id: response.id,
                text: self.text.as_str().into(),
                model: state.transport.as_ref().expect("transport snapshot").clone(),
                rules: self.rules,
                readonly: self.readonly,
                multiline: self.multiline,
                code_editor: self.code_editor,
                enabled: response.enabled(),
                focused: has_focus,
                bounds: transform * response.interact_rect,
                content: transform * content,
                text_origin: transform * origin,
                scale: transform.scaling,
                font,
                geometry,
                font_options: FontRunOptions {
                    wrap_width: self.multiline.then_some(content.width()),
                    line_height: self.line_height,
                    tab_size: self.tab_size,
                },
            });
        }
        Ok(())
    }
}

fn paint_native_name(
    frame: &UiFrame<'_>,
    painter: &egui::Painter,
    text: &str,
    origin: Pos2,
    scale: f32,
) {
    use solomon_sim::menu::create_name::layout_create_wizard_name;
    use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextLayout};
    let mut name = layout_create_wizard_name(text);
    for glyph in &mut name.glyphs {
        glyph.center_x = f64::from(origin.x) + glyph.center_x * f64::from(scale);
        glyph.center_y = f64::from(origin.y) + glyph.center_y * f64::from(scale);
        glyph.scale *= f64::from(scale);
        glyph.tint = 0xd8ba70;
    }
    crate::gpu_ui::paint_text_layout(
        frame,
        painter,
        &NativeUiTextLayout {
            font: NativeUiFontName::Heading,
            glyphs: name.glyphs,
            lines: Vec::new(),
            width: name.width,
            height: name.height,
            unsupported_code_points: Vec::new(),
        },
    );
}
