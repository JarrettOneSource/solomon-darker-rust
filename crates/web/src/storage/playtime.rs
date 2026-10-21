//! `playtime-store.ts`: page lifetime includes title, loading and settings time.
//! Each flush rereads storage so separate tabs add to the latest committed total.

use std::cell::Cell;
use std::rc::Rc;

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Storage, Window};

use super::{describe, local_storage, scoped_key};

const STORAGE_KEY: &str = "sdr.game.playtime.v1";
const FLUSH_INTERVAL_MS: i32 = 30_000;

pub fn read_total_playtime_ms() -> Result<u64, String> {
    read(&local_storage()?, &scoped_key(STORAGE_KEY)?)
}

fn read(storage: &Storage, key: &str) -> Result<u64, String> {
    let Some(raw) = storage.get_item(key).map_err(describe)? else { return Ok(0) };
    let number = raw.parse::<js_sys::Number>().expect("JavaScript Number conversion is total");
    let value = number.value_of();
    Ok(if js_sys::Number::is_safe_integer(&JsValue::from_f64(value)) && value >= 0.0 {
        value as u64
    } else {
        0
    })
}

pub struct PlaytimeRecorder {
    window: Window,
    storage: Storage,
    key: String,
    marked_at: Rc<Cell<f64>>,
    interval: i32,
    callback: Closure<dyn FnMut()>,
}

impl PlaytimeRecorder {
    pub fn start(window: &Window) -> Result<Self, String> {
        let storage = local_storage()?;
        let key = scoped_key(STORAGE_KEY)?;
        let marked_at = Rc::new(Cell::new(js_sys::Date::now()));
        let write_storage = storage.clone();
        let write_key = key.clone();
        let write_clock = marked_at.clone();
        let callback = Closure::<dyn FnMut()>::new(move || {
            if let Err(error) = flush(&write_storage, &write_key, &write_clock) {
                web_sys::console::error_1(&JsValue::from_str(&error));
            }
        });
        let interval = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                FLUSH_INTERVAL_MS,
            )
            .map_err(|_| "Could not start the playtime clock")?;
        let recorder = Self { window: window.clone(), storage, key, marked_at, interval, callback };
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
        if let Err(error) = flush(&self.storage, &self.key, &self.marked_at) {
            web_sys::console::error_1(&JsValue::from_str(&error));
        }
    }
}

fn flush(storage: &Storage, key: &str, marked_at: &Cell<f64>) -> Result<(), String> {
    let now = js_sys::Date::now();
    let elapsed = js_sys::Math::round(now - marked_at.replace(now)).max(0.0);
    if elapsed == 0.0 {
        return Ok(());
    }
    let total = read(storage, key)? as f64 + elapsed;
    storage.set_item(key, &total.to_string()).map_err(describe)
}
