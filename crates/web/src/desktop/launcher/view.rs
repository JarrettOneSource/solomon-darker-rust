//! GPU presentation of the original desktop launcher's three modes and settings.

use egui::{Color32, FontFamily, FontId, RichText, Stroke, Ui, vec2};

use super::model::{Action, Model, SessionOptions};
use crate::gpu_ui::{NativeEditOutput, NativeTextEdit, UiFrame};

const GOLD: Color32 = Color32::from_rgb(229, 203, 142);
const TEXT: Color32 = Color32::from_rgb(229, 223, 208);
const MUTED: Color32 = Color32::from_rgb(183, 171, 185);

fn copy(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).font(FontId::new(size, FontFamily::Name("Georgia".into()))).color(color)
}

pub(super) fn paint(model: &mut Model, frame: &mut UiFrame<'_>) -> Result<(), String> {
    let ctx = frame.ctx.clone();
    egui::Area::new(egui::Id::unique("desktop-launcher"))
        .order(egui::Order::Background)
        .fixed_pos(egui::Pos2::ZERO)
        .movable(false)
        .fade_in(false)
        .show(&ctx, |ui| {
            ui.set_width(ctx.content_rect().width());
            ui.set_height(ctx.content_rect().height());
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(Color32::from_rgb(12, 9, 16)))
                .show(ui, |ui| {
                    frame.input_scope("desktop-launcher", ui.layer_id());
                    let width = ui.available_width();
                    let compact = width <= 1000.0;
                    let padding = if compact { 24.0 } else { 38.0 };
                    let content_width = (width - padding * 2.0).min(1104.0);
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.add_space(if compact { 28.0 } else { 42.0 });
                        ui.horizontal(|ui| {
                            ui.add_space(((width - content_width) * 0.5).max(0.0));
                            ui.vertical(|ui| {
                                ui.set_width(content_width);
                                content(model, frame, ui, compact);
                            });
                        });
                    });
                });
        });
    Ok(())
}

fn content(model: &mut Model, frame: &UiFrame<'_>, ui: &mut Ui, compact: bool) {
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("THE DARK IS YOURS").size(11.0).color(Color32::from_rgb(188, 165, 115)),
        );
        ui.add_space(12.0);
        ui.label(copy("Solomon Darker", if compact { 40.0 } else { 48.0 }, GOLD));
        ui.add_space(10.0);
        ui.label(copy("Play offline. Bring your friends.", 19.0, MUTED));
    });
    ui.add_space(30.0);
    if let Some(update) = &model.state.update {
        egui::Frame::new()
            .fill(Color32::from_rgb(43, 37, 22))
            .stroke(Stroke::new(1.0, Color32::from_rgb(164, 136, 71)))
            .inner_margin(18)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.vertical(|ui| {
                        ui.label(copy("A new update is available", 18.0, GOLD));
                        ui.label(copy(
                            "Your saves stay on this device when you replace the app.",
                            14.0,
                            TEXT,
                        ));
                    });
                    if button(
                        frame,
                        ui,
                        "launcher/download",
                        "Download update",
                        !model.launching && model.pending.is_none(),
                        false,
                        vec2(160.0, 42.0),
                    ) {
                        model.pending = Some(Action::Download);
                    }
                    frame.record_value("launcher/download", &update.revision, None);
                });
            });
        ui.add_space(22.0);
    }
    ui.spacing_mut().item_spacing.x = 18.0;
    ui.columns(3, |columns| {
        for (index, ui) in columns.iter_mut().enumerate() {
            mode_card(model, frame, ui, index, compact);
        }
    });
    ui.add_space(14.0);
    let status = ui.add_sized(
        vec2(ui.available_width(), 28.0),
        egui::Label::new(copy(&model.status, 15.0, Color32::from_rgb(231, 189, 137))),
    );
    if !model.status.is_empty() {
        frame.record_rect("launcher/status", &model.status, "status", status.rect, true);
        frame.record_value("launcher/status", &model.status, None);
        frame.announce("launcher/status", &model.status);
    }
    help(model, frame, ui);
    footer(model, frame, ui);
}

fn mode_card(model: &mut Model, frame: &UiFrame<'_>, ui: &mut Ui, index: usize, compact: bool) {
    egui::Frame::new()
        .fill(Color32::from_rgb(20, 16, 24))
        .stroke(Stroke::new(1.0, Color32::from_rgb(74, 59, 56)))
        .inner_margin(if compact { 20 } else { 28 })
        .show(ui, |ui| {
            let width = ui.available_width();
            let top = ui.cursor().top();
            ui.set_min_height(376.0);
            ui.label(copy(["I", "II", "III"][index], 13.0, Color32::from_rgb(154, 126, 83)));
            ui.add_space(14.0);
            ui.label(copy(["Play solo", "Host a game", "Join a friend"][index], 25.0, TEXT));
            ui.add_space(14.0);
            ui.label(copy([
                "Enter the College and explore the Boneyards. No account or internet connection needed.",
                "Your computer runs the game. Friends connect directly using your invite.",
                "Paste an invite from the host. Both of you need the same version of the app.",
            ][index], 15.0, MUTED));
            ui.add_space((top + 162.0 - ui.cursor().top()).max(8.0));
            match index {
                1 => host_fields(model, frame, ui, width),
                2 => {
                    field_label(ui, "Multiplayer invite");
                    edit(frame, ui, "launcher/invite", "Multiplayer invite", &mut model.invite,
                        "solomondarker://…", true, !model.launching, width);
                }
                _ => {}
            }
            ui.add_space((top + 334.0 - ui.cursor().top()).max(16.0));
            let (id, label) = [
                ("launcher/solo", "Play offline"),
                ("launcher/start-host", "Host game"),
                ("launcher/join", "Join game"),
            ][index];
            if button(frame, ui, id, label, !model.launching && model.pending.is_none(),
                index == 0, vec2(width, 42.0)) {
                let options = match index {
                    1 => model.host_options(),
                    2 => model.join_options(),
                    _ => Ok(SessionOptions::Solo),
                };
                model.launch(options);
            }
        });
}

fn host_fields(model: &mut Model, frame: &UiFrame<'_>, ui: &mut Ui, width: f32) {
    field_label(ui, "Your reachable IP or hostname");
    let host = edit(
        frame,
        ui,
        "launcher/host",
        "Your reachable IP or hostname",
        &mut model.host,
        "192.168.1.20",
        false,
        !model.launching,
        width,
    );
    if host.submitted {
        model.launch(model.host_options());
    }
    ui.add_space(10.0);
    field_label(ui, "TCP port");
    let port = edit(
        frame,
        ui,
        "launcher/port",
        "TCP port",
        &mut model.port,
        "27843",
        false,
        !model.launching,
        width,
    );
    if port.has_focus() {
        let step = ui.input(|input| {
            i32::from(input.key_pressed(egui::Key::ArrowUp))
                - i32::from(input.key_pressed(egui::Key::ArrowDown))
        });
        if step != 0
            && let Ok(value) = model.port.parse::<u16>()
        {
            model.port = (i32::from(value) + step).clamp(1, 65535).to_string();
        }
    }
    if port.submitted {
        model.launch(model.host_options());
    }
}

fn help(model: &mut Model, frame: &UiFrame<'_>, ui: &mut Ui) {
    let label =
        if model.help_open { "▾ Playing with friends" } else { "▸ Playing with friends" };
    let response = ui.add(egui::Button::new(copy(label, 14.0, GOLD)).frame(false));
    frame.record("launcher/help", "Playing with friends", "button", &response);
    if response.clicked() {
        model.help_open = !model.help_open;
    }
    frame.record_value(
        "launcher/help",
        if model.help_open { "open" } else { "closed" },
        Some(model.help_open),
    );
    if model.help_open {
        ui.label(copy("On the same network, use your local IP. Over the internet, use a public IP or VPN address. Allow Solomon Darker through your firewall and forward the chosen TCP port to the host when needed.", 14.0, MUTED));
        ui.add_space(8.0);
        ui.label(copy("After starting a game, choose Game → Copy multiplayer invite and send it to your friends. Keep the host app open. To switch modes, choose Game → Save and return to launcher.", 14.0, MUTED));
    }
}

fn footer(model: &mut Model, frame: &UiFrame<'_>, ui: &mut Ui) {
    ui.add_space(28.0);
    ui.separator();
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        let build = format!(
            "Build {} · Saves stored on this device",
            model.state.build.revision.chars().take(8).collect::<String>()
        );
        let label = ui.label(copy(&build, 12.0, MUTED));
        frame.record_rect("launcher/build", &build, "text", label.rect, true);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(
                frame,
                ui,
                "launcher/website",
                "Website & Dark Cloud",
                !model.launching && model.pending.is_none(),
                false,
                vec2(155.0, 34.0),
            ) {
                model.pending = Some(Action::Website);
            }
            if button(
                frame,
                ui,
                "launcher/check",
                "Check for updates",
                !model.launching && !model.checking && model.pending.is_none(),
                false,
                vec2(145.0, 34.0),
            ) {
                model.checking = true;
                model.status = "Checking for updates…".into();
                model.pending = Some(Action::Check);
            }
        });
    });
    ui.add_space(24.0);
}

#[allow(clippy::too_many_arguments)] // Source buttons retain their labels, dimensions and enabled state.
fn button(
    frame: &UiFrame<'_>,
    ui: &mut Ui,
    id: &str,
    label: &str,
    enabled: bool,
    primary: bool,
    size: egui::Vec2,
) -> bool {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(copy(label, 15.0, Color32::from_rgb(236, 218, 179)))
            .fill(if primary {
                Color32::from_rgb(107, 71, 41)
            } else {
                Color32::from_rgb(44, 32, 32)
            })
            .stroke(Stroke::new(1.0, Color32::from_rgb(128, 102, 62)))
            .corner_radius(3.0)
            .min_size(size),
    );
    frame.record(id, label, "button", &response);
    if frame.focus_visible(&response) {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            3.0,
            Stroke::new(2.0, GOLD),
            egui::StrokeKind::Outside,
        );
    }
    response.clicked()
}

fn field_label(ui: &mut Ui, label: &str) {
    ui.label(
        RichText::new(label)
            .font(FontId::new(12.0, FontFamily::Name("Arial".into())))
            .color(Color32::from_rgb(199, 185, 158)),
    );
    ui.add_space(5.0);
}

#[allow(clippy::too_many_arguments)] // The three Source fields supply their own geometry and admission.
fn edit(
    frame: &UiFrame<'_>,
    ui: &mut Ui,
    id: &str,
    label: &str,
    value: &mut String,
    hint: &str,
    multiline: bool,
    enabled: bool,
    width: f32,
) -> NativeEditOutput {
    let (_, rect) = ui.allocate_space(vec2(width, if multiline { 116.0 } else { 40.0 }));
    let editor = if multiline {
        NativeTextEdit::multiline(value, id, label)
    } else {
        NativeTextEdit::singleline(value, id, label)
    };
    ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(9, 8, 13));
    let result = editor
        .font(FontId::monospace(14.0))
        .color(Color32::from_rgb(239, 231, 217))
        .margin(vec2(11.0, 10.0))
        .hint_text(hint)
        .enabled(enabled)
        .show(frame, ui, rect);
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(
            if result.has_focus() { 2.0 } else { 1.0 },
            if result.has_focus() { GOLD } else { Color32::from_rgb(75, 65, 74) },
        ),
        egui::StrokeKind::Inside,
    );
    result
}
