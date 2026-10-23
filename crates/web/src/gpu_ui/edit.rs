//! Complete font-run typography and the native GPU editing owner.
mod font_run;
mod geometry;
pub(super) mod model;
pub(super) mod session;
mod widget;

use std::sync::Arc;

use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Vec2};

use super::UiFrame;

pub use font_run::{
    FontRunOptions, NativeFontRun, NativeFontStroke, font_run, font_run_with_options,
};
pub use model::{Rules as EditorRules, Selection as TextSelection};
pub use widget::{NativeEditOutput, NativeTextEdit};

#[derive(Clone, Debug)]
pub struct NativeLabelStyle {
    pub font: FontId,
    pub color: Color32,
    pub opacity: f32,
    pub wrap_width: Option<f32>,
    pub line_height: Option<f32>,
    pub tab_size: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub stroke: Option<NativeFontStroke>,
    pub sense: Sense,
}

impl NativeLabelStyle {
    pub fn new(font: FontId, color: Color32) -> Self {
        Self {
            font,
            color,
            opacity: 1.0,
            wrap_width: None,
            line_height: None,
            tab_size: None,
            letter_spacing: None,
            stroke: None,
            sense: Sense::hover(),
        }
    }
}

/// This is the painted label itself. Its actual native metrics determine both
/// layout and hit geometry; diagnostics never invent an invisible proxy label.
pub fn shaped_label(
    frame: &UiFrame<'_>,
    ui: &mut egui::Ui,
    id: &str,
    text: &str,
    style: NativeLabelStyle,
) -> Result<Response, String> {
    let run = font_run_with_options(
        frame.ctx,
        style.font,
        style.color,
        text,
        FontRunOptions {
            wrap_width: style.wrap_width,
            line_height: style.line_height,
            tab_size: style.tab_size,
            letter_spacing: style.letter_spacing,
            stroke: style.stroke,
        },
    )?;
    let (_, rect) = ui.allocate_space(run.size());
    let response = ui.interact(rect, egui::Id::unique(id), style.sense);
    if ui.is_visible() && style.opacity > 0.0 {
        run.paint(frame, ui.painter(), rect.min, style.opacity)?;
        if style.sense.interactive() {
            frame.record(id, text, "button", &response);
        } else {
            let transform = frame.ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default();
            let bounds = run.ink_bounds().translate(rect.min.to_vec2()).intersect(ui.clip_rect());
            frame.record_rect(id, text, "text", transform * bounds, false);
        }
        frame.record_value(id, text, None);
    }
    Ok(response)
}

#[derive(Clone, Debug)]
pub(crate) enum EditorFont {
    Browser(FontId),
    NativeName { origin: Pos2, scale: f32, field: Rect },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EditorOwner {
    pub id: egui::Id,
    pub instance: u64,
}

#[derive(Clone, Default)]
struct EditorMounts {
    active: std::collections::HashMap<egui::Id, u64>,
    next: u64,
}

pub(super) fn owner(ctx: &egui::Context, id: egui::Id) -> EditorOwner {
    ctx.data_mut(|data| {
        let mounts =
            data.get_temp_mut_or_default::<EditorMounts>(egui::Id::unique("native-editor-mounts"));
        let instance = if let Some(instance) = mounts.active.get(&id) {
            *instance
        } else {
            mounts.next += 1;
            let instance = mounts.next;
            mounts.active.insert(id, instance);
            instance
        };
        EditorOwner { id, instance }
    })
}

#[derive(Clone)]
pub(crate) struct EditorDescriptor {
    pub owner: EditorOwner,
    pub text: Arc<str>,
    pub model: Arc<model::Model>,
    pub rules: EditorRules,
    pub readonly: bool,
    pub multiline: bool,
    pub code_editor: bool,
    pub enabled: bool,
    pub focused: bool,
    pub bounds: Rect,
    pub content: Rect,
    pub text_origin: Pos2,
    pub scale: f32,
    pub font: EditorFont,
    pub geometry: Arc<geometry::Geometry>,
    pub font_options: FontRunOptions,
}

impl EditorDescriptor {
    pub fn layout(&self, text: &str) -> Result<Arc<geometry::Geometry>, String> {
        Ok(if text == self.text.as_ref() {
            self.geometry.clone()
        } else {
            match &self.font {
                EditorFont::Browser(font) => font_run::geometry(font, text, self.font_options)?,
                EditorFont::NativeName { origin, scale, field } => {
                    native_name_geometry(text, *origin, *scale, *field)
                }
            }
        })
    }
    pub fn hit(&self, point: Pos2, text: &str) -> Result<geometry::Caret, String> {
        Ok(self.layout(text)?.hit_caret(((point - self.text_origin) / self.scale).to_pos2()))
    }
}

fn native_name_geometry(
    text: &str,
    origin: Pos2,
    scale: f32,
    field: Rect,
) -> Arc<geometry::Geometry> {
    use solomon_sim::menu::create_name::{layout_create_wizard_name, measure_create_wizard_name};
    use solomon_sim::native_ui_text::{
        NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec, layout_native_ui_text,
    };
    let height = field.height();
    let mut carets = Vec::new();
    let mut clusters = Vec::new();
    let shift = if text.is_empty() {
        field.width() / 2.0
    } else {
        let native = layout_create_wizard_name(text);
        let uppercase = text.to_uppercase();
        let plain = layout_native_ui_text(&NativeUiTextSpec {
            align: NativeUiTextAlign::Left,
            ..NativeUiTextSpec::new(NativeUiFontName::Heading, &uppercase, 0.0, 0.0)
        });
        if let (Some(native), Some(plain)) = (native.glyphs.first(), plain.glyphs.first()) {
            origin.x - field.min.x + (native.center_x - plain.center_x) as f32 * scale
        } else {
            (field.width() - measure_create_wizard_name(text) as f32 * scale) / 2.0
        }
    };
    let mut previous = shift;
    carets.push(geometry::Caret {
        index: 0,
        rect: Rect::from_min_size(egui::pos2(shift, 0.0), Vec2::new(0.0, height)),
    });
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        let end = byte + character.len_utf8();
        let x = shift + measure_create_wizard_name(&text[..end]) as f32 * scale;
        let start = units;
        units += character.len_utf16();
        clusters.push(geometry::Cluster {
            start,
            end: units,
            leading: egui::pos2(previous, 0.0),
            trailing: egui::pos2(x, 0.0),
            rects: vec![Rect::from_min_max(egui::pos2(previous, 0.0), egui::pos2(x, height))],
        });
        carets.push(geometry::Caret {
            index: units,
            rect: Rect::from_min_size(egui::pos2(x, 0.0), Vec2::new(0.0, height)),
        });
        previous = x;
    }
    Arc::new(geometry::Geometry {
        size: Vec2::new(field.width(), height),
        lines: vec![geometry::Line {
            start: 0,
            end: units,
            text: text.into(),
            raster_text: text.into(),
            paint_spans: vec![geometry::PaintSpan {
                source: f32::NEG_INFINITY..f32::INFINITY,
                offset: 0.0,
            }],
            top: 0.0,
            baseline: height,
            height,
            width: field.width(),
            carets,
            clusters,
        }],
    })
}

#[derive(Clone, Default)]
struct NativeFocus(Option<egui::Id>);

pub fn text_edit_focused(ctx: &egui::Context) -> bool {
    let focus = ctx
        .data(|data| data.get_temp::<NativeFocus>(egui::Id::unique("native-editor-focus")))
        .and_then(|focus| focus.0);
    focus.is_some_and(|id| ctx.memory(|memory| memory.has_focus(id)))
}

pub(super) fn set_focus(ctx: &egui::Context, editors: &[EditorDescriptor]) {
    let id = editors
        .iter()
        .find(|editor| editor.enabled && editor.focused)
        .map(|editor| editor.owner.id);
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::unique("native-editor-focus"), NativeFocus(id));
        let mounts =
            data.get_temp_mut_or_default::<EditorMounts>(egui::Id::unique("native-editor-mounts"));
        mounts.active =
            editors.iter().map(|editor| (editor.owner.id, editor.owner.instance)).collect();
    });
}
