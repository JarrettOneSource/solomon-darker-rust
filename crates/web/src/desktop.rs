//! Native desktop controls and the shared in-game update notice.
//! The OS application owns sessions and external actions; the GPU UI submits requests.

pub mod launcher;

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::gpu_ui::UiFrame;
use egui::{Color32, FontFamily, FontId, RichText, Stroke, vec2};

/// Credentials are intentionally absent from Debug/log output.
#[derive(Clone)]
pub struct DesktopEndpoint {
    pub url: String,
    pub credential: String,
}

struct ControlState {
    state: launcher::DesktopState,
    pending: Vec<launcher::Action>,
    invite: Option<String>,
    close_error: Option<String>,
    message: Option<String>,
}

#[derive(Clone)]
pub struct DesktopControls(Rc<RefCell<ControlState>>);

thread_local! {
    static CONTROLS: RefCell<Weak<RefCell<ControlState>>> = const { RefCell::new(Weak::new()) };
}

impl DesktopControls {
    /// Install the single native application owner on its window thread.
    pub fn install(state: launcher::DesktopState) -> Self {
        let controls = Rc::new(RefCell::new(ControlState {
            state,
            pending: Vec::new(),
            invite: None,
            close_error: None,
            message: None,
        }));
        CONTROLS.with_borrow_mut(|current| {
            assert!(current.upgrade().is_none(), "one desktop application per window thread");
            *current = Rc::downgrade(&controls);
        });
        Self(controls)
    }

    pub fn state(&self) -> launcher::DesktopState {
        self.0.borrow().state.clone()
    }
    pub fn receive(&self, state: launcher::DesktopState) {
        self.0.borrow_mut().state = state;
    }
    pub fn request(&self, action: launcher::Action) {
        self.0.borrow_mut().pending.push(action);
    }
    pub fn take_actions(&self) -> Vec<launcher::Action> {
        std::mem::take(&mut self.0.borrow_mut().pending)
    }
    pub fn set_invite(&self, invite: Option<String>) {
        self.0.borrow_mut().invite = invite;
    }
    pub fn invite(&self) -> Option<String> {
        self.0.borrow().invite.clone()
    }
    pub fn set_message(&self, message: Option<String>) {
        self.0.borrow_mut().message = message;
    }
    pub fn set_close_error(&self, error: Option<String>) {
        self.0.borrow_mut().close_error = error;
    }
}

pub fn request(action: launcher::Action) -> Result<(), String> {
    CONTROLS.with_borrow(|controls| {
        let controls = controls.upgrade().ok_or("The desktop application is unavailable")?;
        controls.borrow_mut().pending.push(action);
        Ok(())
    })
}

struct Notice {
    current_revision: Option<String>,
    dismissed_revision: Option<String>,
}

impl Notice {
    fn visible(&self) -> bool {
        self.current_revision.is_some() && self.current_revision != self.dismissed_revision
    }
}

pub struct DesktopBridge {
    controls: DesktopControls,
    notice: Notice,
    close_prompt_visible: bool,
}

impl DesktopBridge {
    pub fn new(controls: DesktopControls) -> Self {
        Self {
            controls,
            notice: Notice { current_revision: None, dismissed_revision: None },
            close_prompt_visible: false,
        }
    }
}

impl DesktopBridge {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("desktop/update"))
    }

    pub fn paint(&mut self, frame: &mut UiFrame<'_>, show_controls: bool) -> Result<(), String> {
        let close_error = self.controls.0.borrow().close_error.clone();
        if let Some(error) = close_error {
            self.paint_close_error(frame, &error);
            self.close_prompt_visible = true;
            return Ok(());
        }
        self.close_prompt_visible = false;
        if show_controls {
            self.paint_controls(frame);
        }
        self.notice.current_revision = self.controls.state().update.map(|update| update.revision);
        if !self.notice.visible() {
            return Ok(());
        }
        let ctx = frame.ctx.clone();
        let mut action = None;
        let area = egui::Area::new(self.layer_id().id)
            .fade_in(false)
            .order(self.layer_id().order)
            .anchor(egui::Align2::RIGHT_TOP, vec2(-12.0, 12.0))
            .show(&ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(30, 24, 36))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(181, 152, 86)))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_max_width((ctx.content_rect().width() - 58.0).max(100.0));
                        ui.spacing_mut().item_spacing.x = 12.0;
                        ui.horizontal_wrapped(|ui| {
                            let copy = |text: &str| {
                                RichText::new(text)
                                    .font(FontId::new(14.0, FontFamily::Name("Georgia".into())))
                                    .color(Color32::from_rgb(242, 230, 199))
                            };
                            ui.label(copy("A Solomon Darker update is available."));
                            for (id, label) in [("download", "Download update"), ("later", "Later")]
                            {
                                let response = ui.add(
                                    egui::Button::new(copy(label))
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(139, 114, 68)))
                                        .corner_radius(3.0)
                                        .min_size(vec2(0.0, 30.0)),
                                );
                                frame.record(&format!("desktop/{id}"), label, "button", &response);
                                if response.clicked() {
                                    action = Some(id);
                                }
                            }
                        });
                    })
                    .response
                    .rect
            });
        frame.record_rect(
            "desktop/update",
            "A Solomon Darker update is available.",
            "status",
            area.inner,
            true,
        );
        match action {
            Some("later") => {
                let notice = &mut self.notice;
                notice.dismissed_revision = notice.current_revision.clone();
            }
            Some("download") => {
                self.controls.request(launcher::Action::Download);
            }
            _ => {}
        }
        Ok(())
    }

    fn paint_controls(&self, frame: &UiFrame<'_>) {
        let ctx = frame.ctx.clone();
        egui::Area::new(egui::Id::unique("desktop/controls"))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .anchor(egui::Align2::LEFT_BOTTOM, vec2(12.0, -12.0))
            .show(&ctx, |ui| {
                egui::Frame::new().fill(Color32::from_rgb(24, 20, 28)).inner_margin(8).show(
                    ui,
                    |ui| {
                        ui.horizontal(|ui| {
                            for (id, label, action, enabled) in [
                                (
                                    "launcher",
                                    "Save and return to launcher",
                                    launcher::Action::Launcher,
                                    true,
                                ),
                                (
                                    "copy-invite",
                                    "Copy multiplayer invite",
                                    launcher::Action::CopyInvite,
                                    self.controls.invite().is_some(),
                                ),
                                ("check", "Check for updates", launcher::Action::Check, true),
                            ] {
                                let response = ui.add_enabled(
                                    enabled,
                                    egui::Button::new(RichText::new(label).font(FontId::new(
                                        13.0,
                                        FontFamily::Name("Georgia".into()),
                                    ))),
                                );
                                frame.record(&format!("desktop/{id}"), label, "button", &response);
                                if response.clicked() {
                                    self.controls.request(action);
                                }
                            }
                        });
                        if let Some(message) = &self.controls.0.borrow().message {
                            let response = ui.label(message);
                            frame.record_rect(
                                "desktop/message",
                                message,
                                "status",
                                response.rect,
                                true,
                            );
                            frame.announce("desktop/message", message);
                        }
                    },
                );
            });
    }

    fn paint_close_error(&self, frame: &UiFrame<'_>, error: &str) {
        let ctx = frame.ctx.clone();
        let layer =
            egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("desktop/close-failed"));
        frame.input_scope("desktop/close-failed", layer);
        egui::Area::new(layer.id)
            .order(layer.order)
            .fade_in(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(&ctx, |ui| {
                ui.memory_mut(|memory| memory.set_modal_layer(layer));
                egui::Frame::new()
                    .fill(Color32::from_rgb(24, 20, 28))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(181, 152, 86)))
                    .inner_margin(24)
                    .show(ui, |ui| {
                        ui.set_max_width((ctx.content_rect().width() - 96.0).clamp(120.0, 520.0));
                        ui.heading("The latest progress could not be saved");
                        ui.add_space(12.0);
                        ui.label(error);
                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            for (id, label, action) in [
                                ("keep-playing", "Keep playing", launcher::Action::KeepPlaying),
                                (
                                    "close-unsaved",
                                    "Close without saving",
                                    launcher::Action::CloseWithoutSaving,
                                ),
                            ] {
                                let response = ui.button(label);
                                frame.record(&format!("desktop/{id}"), label, "button", &response);
                                if !self.close_prompt_visible && id == "keep-playing" {
                                    response.request_focus();
                                }
                                if response.clicked() {
                                    self.controls.request(action);
                                }
                            }
                        });
                    });
            });
        if frame.key_codes().iter().any(|key| key.pressed && key.code == "Escape") {
            self.controls.request(launcher::Action::KeepPlaying);
        }
    }
}
