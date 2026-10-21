//! Website `lib/format.ts`. Locale-sensitive metadata stays with the browser's
//! native formatter, just as in the source; it is not gameplay arithmetic.

use std::cmp::Ordering;

use js_sys::{Date, Function, JsString, Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};

pub(super) fn compare(first: &str, second: &str) -> Ordering {
    JsString::from(first).locale_compare(second, &js_sys::Array::new(), &Object::new()).cmp(&0)
}

pub(super) fn lowercase(value: &str) -> String {
    JsString::from(value).to_locale_lower_case(None).into()
}

pub(super) fn count(value: u64) -> Result<String, String> {
    if value >= 10_000 {
        return Ok(format!("{}k", fixed(value as f64 / 1000.0, 1)?));
    }
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

pub(super) fn bytes(value: u64) -> Result<String, String> {
    if value < 1024 {
        Ok(format!("{value} B"))
    } else if value < 1024 * 1024 {
        Ok(format!("{} KB", fixed(value as f64 / 1024.0, 1)?))
    } else if value < 1024 * 1024 * 1024 {
        Ok(format!("{} MB", fixed(value as f64 / (1024.0 * 1024.0), 1)?))
    } else {
        Ok(format!("{} GB", fixed(value as f64 / (1024.0 * 1024.0 * 1024.0), 2)?))
    }
}

fn fixed(value: f64, digits: u8) -> Result<String, String> {
    js_sys::Number::from(value)
        .to_fixed(digits)
        .map(String::from)
        .map_err(|_| "format content file size".to_owned())
}

pub(super) fn date(value: &str) -> Result<String, String> {
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

pub(super) fn time_ago(value: &str) -> String {
    let date = Date::parse(value);
    if !date.is_finite() {
        return String::new();
    }
    let seconds = ((Date::now() - date) / 1000.0).max(0.0);
    if seconds < 60.0 {
        return "just now".to_owned();
    }
    let minutes = seconds / 60.0;
    if minutes < 60.0 {
        return format!("{}m ago", minutes.floor());
    }
    let hours = minutes / 60.0;
    if hours < 24.0 {
        return format!("{}h ago", hours.floor());
    }
    let days = hours / 24.0;
    if days < 30.0 {
        return format!("{}d ago", days.floor());
    }
    let months = days / 30.0;
    if months < 12.0 {
        return format!("{}mo ago", months.floor());
    }
    format!("{}y ago", (months / 12.0).floor())
}
