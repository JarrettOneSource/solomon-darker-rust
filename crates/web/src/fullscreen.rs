//! GPU fullscreen control and essential browser fullscreen/installation bridge.
//! Platform requests run inside the original trusted canvas click.

use crate::gpu_ui::UiFrame;
use egui::{Color32, Id, Order, PointerButton, Stroke, pos2, vec2};
use js_sys::{Function, Object, Promise, Reflect};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Event, MouseEvent, Window};

const CHANGE_EVENTS: [&str; 2] = ["fullscreenchange", "webkitfullscreenchange"];
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
    document: Document,
    state: Rc<RefCell<FullscreenState>>,
    click: Closure<dyn FnMut(Event)>,
    change: Closure<dyn FnMut(Event)>,
    placement: FullscreenPlacement,
}
impl FullscreenControl {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let state = Rc::new(RefCell::new(FullscreenState {
            mode: game_fullscreen_control_mode(document)?,
            active: fullscreen_active(document),
            show_help: false,
            error: None,
            target: None,
        }));
        let clicked = state.clone();
        let clicked_document = document.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(mouse) = event.dyn_ref::<MouseEvent>() else { return };
            let point = pos2(mouse.client_x() as f32, mouse.client_y() as f32);
            if !clicked
                .borrow()
                .target
                .as_ref()
                .is_some_and(|target| crate::ui::trusted_gesture::owns_point(target, point))
            {
                return;
            }
            event.prevent_default();
            event.stop_propagation();
            activate(&clicked_document, &clicked);
        });
        document
            .add_event_listener_with_callback_and_bool(
                "click",
                click.as_ref().unchecked_ref(),
                true,
            )
            .map_err(|_| "fullscreen trusted click")?;
        let changed = state.clone();
        let changed_document = document.clone();
        let change = Closure::<dyn FnMut(Event)>::new(move |_| {
            let mut state = changed.borrow_mut();
            state.active = fullscreen_active(&changed_document);
            state.error = None;
            state.show_help = false;
        });
        for name in CHANGE_EVENTS {
            document
                .add_event_listener_with_callback(name, change.as_ref().unchecked_ref())
                .map_err(|_| "fullscreen change listener")?;
        }
        Ok(Self {
            document: document.clone(),
            state,
            click,
            change,
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
        self.document
            .default_view()
            .ok_or("fullscreen window")?
            .match_media("(orientation:portrait) and (hover:none) and (pointer:coarse)")
            .map_err(|_| "fullscreen orientation query".to_owned())
            .map(|query| query.is_some_and(|query| query.matches()))
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
                keyboard_activation =
                    response.clicked() && !response.clicked_by(PointerButton::Primary);
            });
        if keyboard_activation {
            activate(&self.document, &self.state);
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
fn activate(document: &Document, state: &Rc<RefCell<FullscreenState>>) {
    {
        let mut state = state.borrow_mut();
        state.error = None;
        if state.mode == ControlMode::Install {
            state.show_help = !state.show_help;
            return;
        }
    }
    let request = toggle_fullscreen(document);
    let completed = state.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let result = match request {
            Ok(value) => JsFuture::from(Promise::resolve(&value)).await,
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            completed.borrow_mut().error = Some(error_message(error));
        }
    });
}

fn property(target: &JsValue, name: &str) -> JsValue {
    Reflect::get(target, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
}

fn method(target: &JsValue, name: &str) -> Option<Function> {
    property(target, name).dyn_into().ok()
}

fn standard_supported(document: &Document, target: &Element) -> bool {
    property(document, "fullscreenEnabled").as_bool() != Some(false)
        && method(target, "requestFullscreen").is_some()
        && method(document, "exitFullscreen").is_some()
}

fn webkit_supported(document: &Document, target: &Element) -> bool {
    property(document, "webkitFullscreenEnabled").as_bool() != Some(false)
        && method(target, "webkitRequestFullscreen").is_some()
        && method(document, "webkitExitFullscreen").is_some()
}

pub(crate) fn game_fullscreen_control_mode(document: &Document) -> Result<ControlMode, String> {
    let window = document.default_view().ok_or("fullscreen window")?;
    let target = document.document_element().ok_or("fullscreen document root")?;
    if standard_supported(document, &target) || webkit_supported(document, &target) {
        return Ok(ControlMode::Fullscreen);
    }
    Ok(if installed_display_mode(&window)? { ControlMode::Hidden } else { ControlMode::Install })
}

fn current_fullscreen_element(document: &Document) -> JsValue {
    let standard = property(document, "fullscreenElement");
    if standard.is_null_or_undefined() {
        property(document, "webkitFullscreenElement")
    } else {
        standard
    }
}

pub(crate) fn fullscreen_active(document: &Document) -> bool {
    document
        .document_element()
        .is_some_and(|target| current_fullscreen_element(document) == JsValue::from(target))
}

fn installed_display_mode(window: &Window) -> Result<bool, String> {
    if property(&window.navigator(), "standalone").as_bool() == Some(true) {
        return Ok(true);
    }
    for query in ["(display-mode: standalone)", "(display-mode: fullscreen)"] {
        if window
            .match_media(query)
            .map_err(|_| "fullscreen display mode query")?
            .is_some_and(|query| query.matches())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn toggle_fullscreen(document: &Document) -> Result<JsValue, JsValue> {
    let target = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("Fullscreen document root is unavailable."))?;
    if !current_fullscreen_element(document).is_null_or_undefined() {
        let standard = property(document, "fullscreenElement");
        let exit = if !standard.is_null_or_undefined() {
            method(document, "exitFullscreen")
        } else {
            method(document, "webkitExitFullscreen")
        };
        let exit = exit
            .or_else(|| method(document, "exitFullscreen"))
            .or_else(|| method(document, "webkitExitFullscreen"));
        return exit
            .ok_or_else(|| JsValue::from_str("Fullscreen is not available in this browser."))?
            .call0(document);
    }
    if standard_supported(document, &target) {
        let options = Object::new();
        Reflect::set(&options, &JsValue::from_str("navigationUI"), &JsValue::from_str("hide"))?;
        return method(&target, "requestFullscreen")
            .expect("supported fullscreen request")
            .call1(&target, &options);
    }
    if webkit_supported(document, &target) {
        return method(&target, "webkitRequestFullscreen")
            .expect("supported WebKit fullscreen request")
            .call0(&target);
    }
    Err(JsValue::from_str("Fullscreen is not available in this browser."))
}

pub(crate) fn error_message(error: JsValue) -> String {
    property(&error, "message")
        .as_string()
        .unwrap_or_else(|| "Fullscreen could not be changed.".to_owned())
}

impl Drop for FullscreenControl {
    fn drop(&mut self) {
        if let Err(error) = self.document.remove_event_listener_with_callback_and_bool(
            "click",
            self.click.as_ref().unchecked_ref(),
            true,
        ) {
            web_sys::console::error_1(&error);
        }
        for name in CHANGE_EVENTS {
            if let Err(error) = self
                .document
                .remove_event_listener_with_callback(name, self.change.as_ref().unchecked_ref())
            {
                web_sys::console::error_1(&error);
            }
        }
    }
}
