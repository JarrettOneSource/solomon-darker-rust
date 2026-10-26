//! Browser mouse/touch capture. Held gameplay state lives in the parent module.
use super::*;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Event, EventTarget, HtmlCanvasElement, MouseEvent, PointerEvent};

pub(super) struct InputTransport {
    canvas: HtmlCanvasElement,
    _listeners: Vec<InputListener>,
}
impl InputTransport {
    pub fn attach(
        surface: &crate::platform::Surface,
        state: Rc<RefCell<GameplayInputState>>,
        hotbar: HotbarSelection,
    ) -> Result<Self, String> {
        let canvas = surface.canvas().clone();
        let document = canvas.owner_document().ok_or("No gameplay document")?;
        let window = document.default_view().ok_or("No gameplay window")?;
        canvas
            .style()
            .set_property("touch-action", "none")
            .map_err(|_| "Cannot configure touch gestures")?;
        let mut listeners = Vec::new();
        for name in ["mousedown", "mousemove", "mouseup"] {
            let state = state.clone();
            let hotbar = hotbar.clone();
            let canvas = canvas.clone();
            let target: &EventTarget =
                if name == "mousedown" { canvas.as_ref() } else { window.as_ref() };
            let captured_canvas = canvas.clone();
            listen(target, name, &mut listeners, move |event| {
                let Some(mouse) = event.dyn_ref::<MouseEvent>() else {
                    return;
                };
                let Some(point) =
                    canvas_point(&captured_canvas, mouse.client_x(), mouse.client_y())
                else {
                    return;
                };
                let bounds = captured_canvas.get_bounding_client_rect();
                let logical = egui::pos2(
                    mouse.client_x() as f32 - bounds.left() as f32,
                    mouse.client_y() as f32 - bounds.top() as f32,
                );
                let phase = match name {
                    "mousedown" => controls::MousePhase::Down,
                    "mouseup" => controls::MousePhase::Up,
                    _ => controls::MousePhase::Move,
                };
                if state.borrow_mut().mouse(phase, mouse.button(), logical, point, hotbar.start()) {
                    event.prevent_default();
                }
            })?;
        }
        listen(canvas.as_ref(), "contextmenu", &mut listeners, |event| event.prevent_default())?;
        let clicked = state.clone();
        let clicked_canvas = canvas.clone();
        listen(canvas.as_ref(), "click", &mut listeners, move |event| {
            let Some(mouse) = event.dyn_ref::<MouseEvent>() else { return };
            let bounds = clicked_canvas.get_bounding_client_rect();
            let logical = egui::pos2(
                mouse.client_x() as f32 - bounds.left() as f32,
                mouse.client_y() as f32 - bounds.top() as f32,
            );
            clicked.borrow_mut().click(
                mouse.button(),
                logical,
                canvas_point(&clicked_canvas, mouse.client_x(), mouse.client_y()),
            );
        })?;
        for name in
            ["pointerdown", "pointermove", "pointerup", "pointercancel", "lostpointercapture"]
        {
            let captured = state.clone();
            let surface = canvas.clone();
            let bank = hotbar.clone();
            listen(window.as_ref(), name, &mut listeners, move |event| {
                let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return };
                let client = match pointer_client_point(pointer) {
                    Ok(point) => point,
                    Err(message) => {
                        report_input_error(Err(message));
                        return;
                    }
                };
                let bounds = surface.get_bounding_client_rect();
                let point = Vec2::new(client.x - bounds.left(), client.y - bounds.top());
                let id = pointer.pointer_id();
                let outcome = {
                    let mut state = captured.borrow_mut();
                    match name {
                        "pointerdown" => state.begin_ui_pointer(id, pointer.button(), point),
                        "pointermove" => state.move_ui_pointer(id, point, bank.bank()),
                        _ => state.end_ui_pointer(id, name == "pointerup", bank.bank()),
                    }
                };
                if outcome.owned {
                    event.prevent_default();
                }
                if outcome.capture {
                    report_input_error(
                        surface
                            .set_pointer_capture(id)
                            .map_err(|_| "cannot capture gameplay UI pointer".to_owned()),
                    );
                }
                if outcome.release && surface.has_pointer_capture(id) {
                    report_input_error(
                        surface
                            .release_pointer_capture(id)
                            .map_err(|_| "cannot release gameplay UI pointer".to_owned()),
                    );
                }
            })?;
        }
        for (target, name) in [
            (window.as_ref(), "blur"),
            (window.as_ref(), "pagehide"),
            (document.as_ref(), "visibilitychange"),
        ] {
            let captured = state.clone();
            let visibility = document.clone();
            let surface = canvas.clone();
            listen(target, name, &mut listeners, move |_| {
                if name == "visibilitychange" && !visibility.hidden() {
                    return;
                }
                let pointers = {
                    let mut state = captured.borrow_mut();
                    let pointers = state.captured_ui_pointers();
                    clear_input(&mut state);
                    pointers
                };
                release_pointers(&surface, pointers);
            })?;
        }
        Ok(Self { canvas, _listeners: listeners })
    }
    pub fn release_pointers(&self, pointers: Vec<i32>) {
        release_pointers(&self.canvas, pointers);
    }
}
fn canvas_point(canvas: &HtmlCanvasElement, x: i32, y: i32) -> Option<Vec2> {
    let bounds = canvas.get_bounding_client_rect();
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return None;
    }
    Some(Vec2::new(
        (f64::from(x) - bounds.left()) / bounds.width(),
        (f64::from(y) - bounds.top()) / bounds.height(),
    ))
}

fn pointer_client_point(pointer: &PointerEvent) -> Result<Vec2, String> {
    // PointerEvent refines MouseEvent's integer coordinates to doubles. Chrome
    // preserves 50.5 here, while web-sys's inherited client_x() truncates it.
    let x = js_sys::Reflect::get(pointer.as_ref(), &JsValue::from_str("clientX"))
        .map_err(|_| "cannot read pointer clientX")?
        .as_f64()
        .ok_or("pointer clientX is not numeric")?;
    let y = js_sys::Reflect::get(pointer.as_ref(), &JsValue::from_str("clientY"))
        .map_err(|_| "cannot read pointer clientY")?
        .as_f64()
        .ok_or("pointer clientY is not numeric")?;
    Ok(Vec2::new(x, y))
}
fn report_input_error(result: Result<(), String>) {
    if let Err(message) = result {
        crate::platform::error(&message);
    }
}
struct InputListener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}
impl Drop for InputListener {
    fn drop(&mut self) {
        if let Err(error) = self.target.remove_event_listener_with_callback_and_bool(
            self.name,
            self.callback.as_ref().unchecked_ref(),
            true,
        ) {
            web_sys::console::error_1(&error);
        }
    }
}
fn listen(
    target: &EventTarget,
    name: &'static str,
    listeners: &mut Vec<InputListener>,
    callback: impl FnMut(Event) + 'static,
) -> Result<(), String> {
    let callback = Closure::<dyn FnMut(Event)>::new(callback);
    target
        .add_event_listener_with_callback_and_bool(name, callback.as_ref().unchecked_ref(), true)
        .map_err(|_| format!("cannot listen for {name}"))?;
    listeners.push(InputListener { target: target.clone(), name, callback });
    Ok(())
}
fn release_pointers(canvas: &HtmlCanvasElement, pointers: Vec<i32>) {
    for id in pointers {
        if canvas.has_pointer_capture(id) {
            report_input_error(
                canvas
                    .release_pointer_capture(id)
                    .map_err(|_| "cannot release gameplay pointer".to_owned()),
            );
        }
    }
}
