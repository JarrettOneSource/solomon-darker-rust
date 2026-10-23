//! GPU stock widgets shared by the menu screens. Source geometry comes from
//! `NativeUiButton`, `NativeUiControlPanel`, and `NativeSettingsPresentation`.

use crate::gpu_ui::{StockAtlas, UiFrame, native_text};
use egui::{
    Align2, Color32, FontId, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use unicode_segmentation::UnicodeSegmentation;

pub(super) const GOLD: Color32 = Color32::from_rgb(217, 186, 112);
pub(super) const PALE: Color32 = Color32::from_rgb(191, 255, 191);
pub(super) const ERROR: Color32 = Color32::from_rgb(228, 163, 154);

#[derive(Clone, Copy)]
pub(super) struct Stage {
    pub origin: Pos2,
    pub scale: f32,
}
impl Stage {
    pub fn fit(view: Rect) -> Self {
        let scale = (view.width() / 1600.0).min(view.height() / 900.0);
        Self { origin: view.center() - Vec2::new(800.0, 450.0) * scale, scale }
    }
    pub fn at(self, x: f64, y: f64) -> Pos2 {
        self.origin + Vec2::new(x as f32, y as f32) * self.scale
    }
    pub fn rect(self, x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::from_min_size(self.at(x, y), Vec2::new(w as f32, h as f32) * self.scale)
    }
}

pub(super) fn color(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}
pub(super) fn alpha(rgb: u32, a: f64) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (rgb >> 16) as u8,
        (rgb >> 8) as u8,
        rgb as u8,
        (a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}
pub(super) fn image(frame: &UiFrame<'_>, ui: &Ui, name: &str, rect: Rect, tint: Color32) {
    frame.image(ui.painter(), &format!("assets/stock/menu/{name}.png"), rect, None, tint);
}
pub(super) fn mirrored_image(
    frame: &UiFrame<'_>,
    ui: &Ui,
    name: &str,
    rect: Rect,
    flip_x: bool,
    flip_y: bool,
    tint: Color32,
) {
    let uv = Rect::from_min_max(
        Pos2::new(if flip_x { 1.0 } else { 0.0 }, if flip_y { 1.0 } else { 0.0 }),
        Pos2::new(if flip_x { 0.0 } else { 1.0 }, if flip_y { 0.0 } else { 1.0 }),
    );
    frame.image(ui.painter(), &format!("assets/stock/menu/{name}.png"), rect, Some(uv), tint);
}
pub(crate) fn interact(
    frame: &UiFrame<'_>,
    ui: &Ui,
    id: &str,
    label: &str,
    role: &str,
    rect: Rect,
    enabled: bool,
) -> Response {
    let response = interact_unstyled(frame, ui, id, label, role, rect, enabled);
    if enabled && ui.is_enabled() && frame.focus_visible(&response) {
        ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, GOLD), StrokeKind::Inside);
    }
    response
}

pub(crate) fn focus_outline(
    frame: &UiFrame<'_>,
    ui: &Ui,
    response: &Response,
    width: f32,
    offset: f32,
    color: Color32,
) {
    if response.enabled() && frame.focus_visible(response) {
        ui.painter().rect_stroke(
            response.rect.expand(offset),
            0.0,
            Stroke::new(width, color),
            StrokeKind::Outside,
        );
    }
}

pub(crate) fn interact_unstyled(
    frame: &UiFrame<'_>,
    ui: &Ui,
    id: &str,
    label: &str,
    role: &str,
    rect: Rect,
    enabled: bool,
) -> Response {
    let enabled = enabled && ui.is_enabled();
    let response =
        ui.interact(rect, Id::unique(id), if enabled { Sense::click() } else { Sense::hover() });
    focus_on_pointer(&response);
    if enabled {
        frame.record(id, label, role, &response);
    } else {
        frame.record_rect(id, label, role, rect, false);
    }
    response
}

/// Match native form controls: the actual pointer target retains keyboard focus.
pub(crate) fn focus_on_pointer(response: &Response) {
    if response.enabled()
        && response.visible()
        && (response.is_pointer_button_down_on()
            || response.clicked_by(egui::PointerButton::Primary))
    {
        response.request_focus();
    }
}

pub(super) fn submitted_on_enter(response: &Response) -> bool {
    // Single-line TextEdit surrenders focus while processing its Enter event.
    response.lost_focus() && response.ctx.input(|input| input.key_pressed(egui::Key::Enter))
}
// Bitmap font, placement and tint are the authored drawing contract.
#[allow(clippy::too_many_arguments)]
pub(super) fn native(
    frame: &UiFrame<'_>,
    ui: &Ui,
    text: &str,
    font: NativeUiFontName,
    position: Pos2,
    scale: f32,
    tint: u32,
    align: NativeUiTextAlign,
) {
    native_text(
        frame,
        ui.painter(),
        &NativeUiTextSpec {
            align,
            scale: f64::from(scale),
            tint,
            ..NativeUiTextSpec::new(font, text, f64::from(position.x), f64::from(position.y))
        },
    );
}
pub(super) fn label(
    ui: &Ui,
    text: &str,
    position: Pos2,
    align: Align2,
    font: &str,
    size: f32,
    tint: Color32,
) -> Rect {
    ui.painter().text(
        position,
        align,
        text,
        FontId::new(size, egui::FontFamily::Name(font.into())),
        tint,
    )
}
pub(super) fn paragraph(
    ui: &Ui,
    text: &str,
    rect: Rect,
    font: &str,
    size: f32,
    tint: Color32,
) -> f32 {
    let galley = ui.painter().layout(
        text.to_owned(),
        FontId::new(size, egui::FontFamily::Name(font.into())),
        tint,
        rect.width(),
    );
    let height = galley.size().y;
    ui.painter().galley(rect.min, galley, tint);
    height
}
pub(crate) fn stock_button(
    frame: &UiFrame<'_>,
    ui: &Ui,
    id: &str,
    text: &str,
    rect: Rect,
    scale: f32,
    enabled: bool,
) -> Response {
    let response = interact(frame, ui, id, text, "button", rect, enabled);
    let paint =
        frame.with_brightness(if enabled && frame.focus_visible(&response) { 1.2 } else { 1.0 });
    paint_stock_button(&paint, ui, &response, rect, scale, enabled);
    let pressed = if enabled && stock_button_pressed(ui, &response) {
        Vec2::splat(6.0 * scale)
    } else {
        Vec2::ZERO
    };
    if !text.is_empty() {
        crate::gpu_ui::native_text(
            &paint,
            ui.painter(),
            &NativeUiTextSpec {
                scale: f64::from(scale),
                alpha: if enabled { 1.0 } else { 0.36 },
                tint: 0xd9ba70,
                ..NativeUiTextSpec::new(
                    NativeUiFontName::Menu,
                    text,
                    f64::from(rect.center().x + pressed.x),
                    f64::from(rect.center().y + 9.0 * scale + pressed.y),
                )
            },
        );
    }
    response
}
pub(crate) fn stock_button_pressed(ui: &Ui, response: &Response) -> bool {
    response.is_pointer_button_down_on()
        || (response.has_focus()
            && ui.input(|input| {
                input.key_down(egui::Key::Enter) || input.key_down(egui::Key::Space)
            }))
}

/// Paint stock chrome around an existing real interaction without recording another widget.
pub(crate) fn paint_stock_button(
    frame: &UiFrame<'_>,
    ui: &Ui,
    response: &Response,
    rect: Rect,
    scale: f32,
    enabled: bool,
) {
    paint_stock_button_highlight(
        frame,
        ui,
        rect,
        scale,
        enabled,
        response.hovered() || frame.focus_visible(response),
    );
}
/// Shared nativeUI54 chrome; each source family chooses its actual highlighted state.
pub(crate) fn paint_stock_button_highlight(
    frame: &UiFrame<'_>,
    ui: &Ui,
    rect: Rect,
    scale: f32,
    enabled: bool,
    highlight: bool,
) {
    paint_stock_button_tinted(frame, ui, rect, scale, enabled, highlight, Color32::WHITE);
}
pub(crate) fn paint_stock_button_tinted(
    frame: &UiFrame<'_>,
    ui: &Ui,
    rect: Rect,
    scale: f32,
    enabled: bool,
    highlight: bool,
    tint: Color32,
) {
    image(
        frame,
        ui,
        if enabled && highlight { "main-menu-button-hover" } else { "main-menu-button" },
        rect,
        tint,
    );
    let surround = 6.0 * scale;
    let end = Vec2::new(70.0, 85.0) * scale;
    let left = rect.min - Vec2::splat(surround);
    image(frame, ui, "main-menu-button-corner", Rect::from_min_size(left, end), tint);
    mirrored_image(
        frame,
        ui,
        "main-menu-button-corner",
        Rect::from_min_size(Pos2::new(rect.right() + surround - end.x, left.y), end),
        true,
        false,
        tint,
    );
    let rail = Rect::from_min_max(
        Pos2::new(left.x + 70.0 * scale, left.y),
        Pos2::new(rect.right() - 64.0 * scale, left.y + end.y),
    );
    if rail.width() > 0.0 {
        image(frame, ui, "main-menu-button-rail", rail, tint);
    }
}

pub(super) fn stone_button(
    frame: &UiFrame<'_>,
    ui: &Ui,
    atlas: &StockAtlas,
    id: &str,
    text: &str,
    rect: Rect,
    enabled: bool,
) -> Result<Response, String> {
    let response = interact_unstyled(frame, ui, id, text, "button", rect, enabled);
    atlas.paint_raw(
        frame,
        ui.painter(),
        if enabled && stock_button_pressed(ui, &response) { 106 } else { 105 },
        rect,
        alpha(0xffffff, if enabled { 1.0 } else { 0.36 }),
    )?;
    native(
        frame,
        ui,
        text,
        NativeUiFontName::ControlPanel,
        rect.center() + Vec2::new(0.0, 4.0),
        1.15,
        0xf2f0dc,
        NativeUiTextAlign::Center,
    );
    focus_outline(frame, ui, &response, 2.0, 3.0, alpha(0xefd17d, 0.9));
    Ok(response)
}
pub(super) fn flat_button(
    frame: &UiFrame<'_>,
    ui: &Ui,
    id: &str,
    text: &str,
    rect: Rect,
    enabled: bool,
) -> Response {
    let response = interact(frame, ui, id, text, "button", rect, enabled);
    gradient(ui, rect, color(0x3c301a), color(0x171108));
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, color(0x947b42)), StrokeKind::Inside);
    label(
        ui,
        text,
        rect.center(),
        Align2::CENTER_CENTER,
        "Cinzel Bold",
        11.0,
        if enabled { color(0xf0d690) } else { color(0x786b48) },
    );
    response
}
pub(crate) fn scrollbar_style(ui: &mut Ui, thumb: u32, track: u32, thin: bool) {
    let style = ui.style_mut();
    if thin {
        style.spacing.scroll = egui::style::ScrollStyle::solid();
        style.spacing.scroll.bar_inner_margin = 0.0;
    }
    style.spacing.scroll.foreground_color = false;
    style.visuals.extreme_bg_color = color(track);
    for visual in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        visual.bg_fill = color(thumb);
    }
}

pub(super) fn gradient(ui: &Ui, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
}
// This field adapter keeps native identity, typography and editing policy explicit.
#[allow(clippy::too_many_arguments)]
pub(super) fn edit(
    frame: &UiFrame<'_>,
    ui: &mut Ui,
    id: &str,
    label: &str,
    value: &mut String,
    rect: Rect,
    font: &str,
    size: f32,
    enabled: bool,
    multiline: bool,
) -> Response {
    let mut edit = if multiline {
        egui::TextEdit::multiline(value)
    } else {
        egui::TextEdit::singleline(value)
    };
    edit = edit
        .id(Id::unique(id))
        .font(FontId::new(size, egui::FontFamily::Name(font.into())))
        .text_color(GOLD)
        .frame(egui::Frame::NONE)
        .desired_width(rect.width())
        .interactive(enabled)
        .margin(Vec2::new(10.0, 7.0));
    let response = ui.put(rect, edit);
    frame.record(id, label, "textbox", &response);
    frame.record_value(id, value, None);
    response
}
pub(super) fn control_panel(
    frame: &UiFrame<'_>,
    ui: &Ui,
    atlas: &StockAtlas,
    rect: Rect,
    field: bool,
) -> Result<(), String> {
    atlas.paint_frame(
        frame,
        ui.painter(),
        if field { 5 } else { 3 },
        rect,
        if field { 3.0 } else { 6.0 },
        Color32::WHITE,
    )
}

/// Source Dark Cloud/Shelf button policy uses pressed chrome and half-opacity disabled art.
pub(crate) fn cloud_button(
    frame: &UiFrame<'_>,
    ui: &Ui,
    id: &str,
    label: &str,
    rect: Rect,
    scale: f32,
    enabled: bool,
) -> Response {
    cloud_button_with_label(frame, ui, id, label, label, rect, scale, enabled)
}

// The stock button separates its accessible name from the authored bitmap label.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cloud_button_with_label(
    frame: &UiFrame<'_>,
    ui: &Ui,
    id: &str,
    label: &str,
    accessible_label: &str,
    rect: Rect,
    scale: f32,
    enabled: bool,
) -> Response {
    let response = interact_unstyled(frame, ui, id, accessible_label, "button", rect, enabled);
    let opacity = if enabled { 1.0 } else { 0.5 };
    let pressed = enabled && stock_button_pressed(ui, &response);
    paint_stock_button_tinted(frame, ui, rect, scale, enabled, pressed, alpha(0xffffff, opacity));
    let press = if pressed { Vec2::splat(6.0 * scale) } else { Vec2::ZERO };
    crate::gpu_ui::native_text(
        frame,
        ui.painter(),
        &solomon_sim::native_ui_text::NativeUiTextSpec {
            alpha: opacity,
            scale: f64::from(scale),
            tint: 0xd9ba70,
            ..solomon_sim::native_ui_text::NativeUiTextSpec::new(
                NativeUiFontName::Menu,
                label,
                f64::from(rect.center().x + press.x),
                f64::from(rect.center().y + 9.0 * scale + press.y),
            )
        },
    );
    if !enabled {
        ui.painter().rect_filled(rect, 0.0, alpha(0x808080, 0.25));
    }
    if enabled {
        focus_outline(frame, ui, &response, 2.0, 3.0, alpha(0xefd17d, 0.9));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form_pass(ctx: &egui::Context, text: &mut String, events: Vec<egui::Event>) -> (bool, bool) {
        let mut state = (false, false);
        ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                ctx.root_ui(|ui| {
                    let response = ui.put(
                        Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(240.0, 44.0)),
                        egui::TextEdit::singleline(text).id(Id::unique("native-form-input")),
                    );
                    state = (response.has_focus(), submitted_on_enter(&response));
                });
            },
        )
        .drop_without_applying_deltas();
        state
    }

    #[test]
    fn actual_single_line_enter_submits_once_after_the_field_surrenders_focus() {
        let ctx = egui::Context::default();
        let mut text = "a real query".to_owned();
        form_pass(&ctx, &mut text, Vec::new());
        let pos = Pos2::new(50.0, 40.0);
        let click = [true, false].map(|pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        let (focused, submitted) = form_pass(
            &ctx,
            &mut text,
            std::iter::once(egui::Event::PointerMoved(pos)).chain(click).collect(),
        );
        assert!(focused);
        assert!(!submitted);
        let enter = || egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let (focused, submitted) = form_pass(&ctx, &mut text, vec![enter()]);
        assert!(!focused);
        assert!(submitted);
        form_pass(
            &ctx,
            &mut text,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: Some(egui::Key::Enter),
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!form_pass(&ctx, &mut text, vec![enter()]).1);
    }
}

pub(crate) fn narrative_line(
    frame: &UiFrame<'_>,
    ui: &Ui,
    text: &str,
    rect: Rect,
    size: f32,
    color: Color32,
) -> Result<(), String> {
    let font = egui::FontId::new(size, egui::FontFamily::Name("IM FELL English".into()));
    let source = text
        .split([' ', '\t', '\n', '\r', '\u{000c}'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let mut run = crate::gpu_ui::font_run(ui.ctx(), font.clone(), color, &source, None)?;
    if run.size().x > rect.width() {
        let boundaries: Vec<_> = source.grapheme_indices(true).map(|(offset, _)| offset).collect();
        let mut low = 0;
        let mut high = boundaries.len();
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            let end = boundaries.get(middle).copied().unwrap_or(source.len());
            let candidate = format!("{}…", &source[..end]);
            let measured =
                crate::gpu_ui::font_run(ui.ctx(), font.clone(), color, &candidate, None)?;
            if measured.size().x <= rect.width() {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        let end = boundaries.get(low).copied().unwrap_or(source.len());
        run =
            crate::gpu_ui::font_run(ui.ctx(), font, color, &format!("{}…", &source[..end]), None)?;
    }
    run.paint(
        frame,
        &ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())),
        rect.left_center() - Vec2::new(0.0, run.size().y / 2.0),
        1.0,
    )
}

/// A Source circle gradient, including its premultiplied alpha, painted once per pixel.
pub(crate) fn radial_gradient(
    ui: &Ui,
    rect: Rect,
    center: Pos2,
    radius: f32,
    inner: Color32,
    outer: Color32,
) {
    const STEPS: u32 = 32;
    let mut mesh = egui::Mesh::default();
    let from = inner.to_array();
    let to = outer.to_array();
    for y in 0..=STEPS {
        for x in 0..=STEPS {
            let position = rect.min
                + rect.size() * Vec2::new(x as f32 / STEPS as f32, y as f32 / STEPS as f32);
            let t = (position.distance(center) / radius).clamp(0.0, 1.0);
            let channels = std::array::from_fn::<u8, 4, _>(|index| {
                (f32::from(from[index]) + (f32::from(to[index]) - f32::from(from[index])) * t)
                    .round() as u8
            });
            mesh.colored_vertex(
                position,
                Color32::from_rgba_premultiplied(
                    channels[0],
                    channels[1],
                    channels[2],
                    channels[3],
                ),
            );
            if x > 0 && y > 0 {
                let i = y * (STEPS + 1) + x;
                mesh.add_triangle(i - STEPS - 2, i - 1, i);
                mesh.add_triangle(i - STEPS - 2, i, i - STEPS - 1);
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}
