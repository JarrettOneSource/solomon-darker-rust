use std::cell::Cell;
use std::rc::Rc;

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::Window;

use super::flush;

pub struct PlaytimeRecorder {
    window: Window,
    marked_at: Rc<Cell<f64>>,
    interval: i32,
    callback: Closure<dyn FnMut()>,
}

impl PlaytimeRecorder {
    pub fn start(window: &Window) -> Result<Self, String> {
        let marked_at = Rc::new(Cell::new(crate::platform::epoch_ms()));
        let write_clock = marked_at.clone();
        let callback = Closure::<dyn FnMut()>::new(move || {
            if let Err(error) = flush(&write_clock) {
                web_sys::console::error_1(&JsValue::from_str(&error));
            }
        });
        let interval = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                super::FLUSH_INTERVAL_MS as i32,
            )
            .map_err(|_| "Could not start the playtime clock")?;
        let recorder = Self { window: window.clone(), marked_at, interval, callback };
        window
            .add_event_listener_with_callback(
                "pagehide",
                recorder.callback.as_ref().unchecked_ref(),
            )
            .map_err(|_| "Could not attach the playtime close handler")?;
        Ok(recorder)
    }
}

impl Drop for PlaytimeRecorder {
    fn drop(&mut self) {
        self.window.clear_interval_with_handle(self.interval);
        if let Err(error) = self
            .window
            .remove_event_listener_with_callback("pagehide", self.callback.as_ref().unchecked_ref())
        {
            web_sys::console::error_1(&error);
        }
        if let Err(error) = flush(&self.marked_at) {
            web_sys::console::error_1(&JsValue::from_str(&error));
        }
    }
}
