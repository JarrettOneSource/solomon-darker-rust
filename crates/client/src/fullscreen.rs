//! GPU fullscreen control and essential browser fullscreen/installation bridge.
//! Platform requests run inside the original trusted canvas click.

use crate::gpu_ui::UiFrame;
use crate::platform::Surface;
#[cfg(target_arch = "wasm32")]
use egui::PointerButton;
use egui::{Color32, Id, Order, Stroke, pos2, vec2};
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
use browser as platform;
#[cfg(not(target_arch = "wasm32"))]
use native as platform;

pub(crate) type FullscreenRequest =
    futures_util::future::LocalBoxFuture<'static, Result<(), String>>;
pub(crate) fn game_fullscreen_control_mode(surface: &Surface) -> Result<ControlMode, String> {
    platform::mode(surface)
}
pub(crate) fn fullscreen_active(surface: &Surface) -> bool {
    platform::active(surface)
}
pub(crate) fn toggle_fullscreen(surface: &Surface) -> Result<FullscreenRequest, String> {
    platform::toggle(surface)
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FullscreenPlacement {
    Hidden,
    Gameplay,
    Title,
    DarkCloud,
    Create { frame_scale: f64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlMode {
    Fullscreen,
    Install,
    Hidden,
}
struct FullscreenState {
    mode: ControlMode,
    active: bool,
    show_help: bool,
    error: Option<String>,
    target: Option<egui::Response>,
}
pub struct FullscreenControl {
    surface: Surface,
    state: Rc<RefCell<FullscreenState>>,
    #[cfg(target_arch = "wasm32")]
    _binding: browser::Binding,
    placement: FullscreenPlacement,
}
impl FullscreenControl {
    pub fn attach(surface: &Surface) -> Result<Self, String> {
        let state = Rc::new(RefCell::new(FullscreenState {
            mode: game_fullscreen_control_mode(surface)?,
            active: fullscreen_active(surface),
            show_help: false,
            error: None,
            target: None,
        }));
        #[cfg(target_arch = "wasm32")]
        let binding = browser::Binding::attach(surface, &state)?;
        Ok(Self {
            surface: surface.clone(),
            state,
            #[cfg(target_arch = "wasm32")]
            _binding: binding,
            placement: FullscreenPlacement::Hidden,
        })
    }
    pub fn update(&mut self, placement: FullscreenPlacement) -> Result<(), String> {
        self.placement = placement;
        if placement == FullscreenPlacement::Hidden {
            self.state.borrow_mut().target = None;
        }
        Ok(())
    }
    /// Source menu.css raises this body root only for the native coarse portrait query.
    pub fn above_orientation_guidance(&self) -> Result<bool, String> {
        self.surface.portrait_touch()
    }
    pub fn fixed_layers() -> [egui::LayerId; 2] {
        [Self::control_layer(), Self::help_layer()]
    }
    pub fn control_layer() -> egui::LayerId {
        egui::LayerId::new(Order::Foreground, Id::unique("fullscreen/control"))
    }
    pub fn help_layer() -> egui::LayerId {
        egui::LayerId::new(Order::Tooltip, Id::unique("fullscreen/help"))
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        self.state.borrow_mut().active = fullscreen_active(&self.surface);
        let (mode, active, show_help, error) = {
            let state = self.state.borrow();
            (state.mode, state.active, state.show_help, state.error.clone())
        };
        if self.placement == FullscreenPlacement::Hidden || mode == ControlMode::Hidden {
            self.state.borrow_mut().target = None;
            return Ok(());
        }
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        let size = vec2(33.0, 33.0);
        let point = match self.placement {
            FullscreenPlacement::Gameplay => pos2(screen.right() - 82.0, 8.0),
            FullscreenPlacement::Title => pos2(screen.right() - 41.0, screen.bottom() - 41.0),
            FullscreenPlacement::DarkCloud => pos2(screen.right() - 41.0, 8.0),
            FullscreenPlacement::Create { frame_scale } => {
                pos2(screen.right() - 41.0 - 80.0 * frame_scale as f32, 10.0 * frame_scale as f32)
            }
            FullscreenPlacement::Hidden => return Ok(()),
        };
        let label = if mode == ControlMode::Install {
            "Fullscreen options"
        } else if active {
            "Exit fullscreen"
        } else {
            "Enter fullscreen"
        };
        let mut keyboard_activation = false;
        egui::Area::new(Self::control_layer().id)
            .fade_in(false)
            .order(Self::control_layer().order)
            .fixed_pos(point)
            .show(&ctx, |ui| {
                let (bounds, response) = ui.allocate_exact_size(size, egui::Sense::click());
                self.state.borrow_mut().target = Some(response.clone());
                let painter = ui.painter();
                painter.rect_filled(bounds, 3.0, Color32::from_black_alpha(178));
                painter.rect_stroke(
                    bounds,
                    3.0,
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(215, 190, 133, 128)),
                    egui::StrokeKind::Inside,
                );
                let origin = bounds.min + vec2(5.0, 5.0);
                let color = Color32::from_rgb(224, 211, 171);
                for (x, y, sx, sy) in [
                    (3.0, 3.0, 1.0, 1.0),
                    (21.0, 3.0, -1.0, 1.0),
                    (3.0, 21.0, 1.0, -1.0),
                    (21.0, 21.0, -1.0, -1.0),
                ] {
                    let corner = if active {
                        origin + vec2(x + 6.0 * sx, y + 6.0 * sy)
                    } else {
                        origin + vec2(x, y)
                    };
                    let sign = if active { -1.0 } else { 1.0 };
                    painter.line_segment(
                        [corner, corner + vec2(6.0 * sx * sign, 0.0)],
                        Stroke::new(1.7, color),
                    );
                    painter.line_segment(
                        [corner, corner + vec2(0.0, 6.0 * sy * sign)],
                        Stroke::new(1.7, color),
                    );
                }
                frame.record("fullscreen/toggle", label, "button", &response);
                frame.record_value(
                    "fullscreen/toggle",
                    if active { "true" } else { "false" },
                    Some(active),
                );
                // Pointer/touch clicks already invoked the browser capability in
                // their trusted DOM event. Intermediate paints cannot consume
                // that ownership; only non-pointer activation belongs here.
                #[cfg(target_arch = "wasm32")]
                {
                    keyboard_activation =
                        response.clicked() && !response.clicked_by(PointerButton::Primary);
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    keyboard_activation = response.clicked();
                }
            });
        if keyboard_activation {
            activate(&self.surface, &self.state);
        }
        if show_help || error.is_some() {
            let message=error.unwrap_or_else(||"Fullscreen on iPhone or iPad\nTap Share, Add to Home Screen, keep Open as Web App enabled if shown, then launch Solomon Darker from its icon.\nOtherwise, open the game in a browser that supports fullscreen.".into());
            egui::Area::new(Self::help_layer().id)
                .fade_in(false)
                .order(Self::help_layer().order)
                .fixed_pos(pos2(
                    (point.x - 290.0).max(8.0),
                    (point.y + 40.0).min(screen.bottom() - 180.0).max(8.0),
                ))
                .show(&ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(10, 12, 17))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(216, 188, 114)))
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            ui.set_max_width(320.0);
                            let response = ui.label(
                                egui::RichText::new(&message)
                                    .color(Color32::from_rgb(238, 227, 196))
                                    .size(14.0),
                            );
                            frame.record("fullscreen/message", &message, "status", &response);
                        });
                });
        }
        Ok(())
    }
}
fn activate(surface: &Surface, state: &Rc<RefCell<FullscreenState>>) {
    {
        let mut state = state.borrow_mut();
        state.error = None;
        if state.mode == ControlMode::Install {
            state.show_help = !state.show_help;
            return;
        }
    }
    let request = toggle_fullscreen(surface);
    let completed = state.clone();
    crate::platform::spawn_local(async move {
        let result = match request {
            Ok(request) => request.await,
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            completed.borrow_mut().error = Some(error);
        }
    });
}
