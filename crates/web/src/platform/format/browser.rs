//! Native browser Intl and Date behavior, retaining the Website display contract.
use std::cmp::Ordering;

use js_sys::{Date, Function, JsString, Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};

pub fn compare(first: &str, second: &str) -> Ordering {
    JsString::from(first).locale_compare(second, &js_sys::Array::new(), &Object::new()).cmp(&0)
}

pub fn lowercase(value: &str) -> String {
    JsString::from(value).to_locale_lower_case(None).into()
}

pub fn integer(value: u64) -> Result<String, String> {
    // Number.toLocaleString uses this native Intl operation. An empty locale
    // list selects the browser default, matching the source's omitted argument.
    let formatter = js_sys::Intl::NumberFormat::new(&js_sys::Array::new(), &Object::new());
    formatter
        .format()
        .call1(&JsValue::UNDEFINED, &JsValue::from_f64(value as f64))
        .map_err(|_| "format browser count")?
        .as_string()
        .ok_or_else(|| "browser count is not text".to_owned())
}
pub fn date(value: &str) -> Result<String, String> {
    let date = Date::new(&JsValue::from_str(value));
    if date.get_time().is_nan() {
        return Ok(String::new());
    }
    let options = Object::new();
    for (name, value) in [("year", "numeric"), ("month", "short"), ("day", "numeric")] {
        Reflect::set(&options, &name.into(), &value.into()).map_err(|_| "date format option")?;
    }
    let method: Function = Reflect::get(&date, &"toLocaleDateString".into())
        .map_err(|_| "read browser date formatter")?
        .dyn_into()
        .map_err(|_| "browser date formatter is not callable")?;
    method
        .call2(&date, &JsValue::UNDEFINED, &options)
        .map_err(|_| "format browser date")?
        .as_string()
        .ok_or_else(|| "browser date is not text".to_owned())
}

pub fn timestamp(value: &str) -> Option<f64> {
    let milliseconds = Date::parse(value);
    milliseconds.is_finite().then_some(milliseconds)
}
