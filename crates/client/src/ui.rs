//! Shared GPU presentation helpers and the platform's native Lua value formatting.

pub(crate) mod belt_pull_off;
pub(crate) mod item_hover;
pub(crate) mod mod_icon;
pub(crate) mod portraits;
pub(crate) mod sack_name;
pub(crate) mod skill_hover;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) mod trusted_gesture;

use solomon_sim::modding::LuaValue;

/// Source String(value) coercion over the JSON-compatible Lua domain.
/// Arrays join recursively; null entries are empty, and objects retain their
/// native display spelling instead of silently becoming serialized JSON.
pub(crate) fn javascript_string(value: &LuaValue) -> String {
    match value {
        LuaValue::Null => "null".into(),
        LuaValue::Boolean(value) => value.to_string(),
        LuaValue::Number(value) => javascript_number(*value),
        LuaValue::String(value) => value.clone(),
        LuaValue::Array(values) => values
            .iter()
            .map(|value| {
                if matches!(value, LuaValue::Null) {
                    String::new()
                } else {
                    javascript_string(value)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        LuaValue::Object(_) => "[object Object]".into(),
    }
}

pub(crate) fn javascript_number(value: f64) -> String {
    ryu_js::Buffer::new().format(value).to_owned()
}

pub(crate) fn trim_javascript(value: &str) -> &str {
    value.trim_matches(|c: char| {
        matches!(c,
        '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
        '\u{205f}' | '\u{3000}' | '\u{feff}')
    })
}

/// Source Number(string), used by numeric keyboard bindings and stored counters.
pub(crate) fn parse_javascript_number(value: &str) -> f64 {
    let value = trim_javascript(value);
    if value.is_empty() {
        return 0.0;
    }
    match value {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    for (prefixes, radix) in [(["0x", "0X"], 16), (["0o", "0O"], 8), (["0b", "0B"], 2)] {
        if let Some(digits) = prefixes.iter().find_map(|prefix| value.strip_prefix(prefix)) {
            return if digits.is_empty() {
                f64::NAN
            } else {
                digits
                    .chars()
                    .try_fold(0.0, |number, digit| {
                        digit
                            .to_digit(radix)
                            .map(|digit| number * f64::from(radix) + f64::from(digit))
                    })
                    .unwrap_or(f64::NAN)
            };
        }
    }
    if value.bytes().any(|c| !matches!(c, b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')) {
        return f64::NAN;
    }
    value.parse().unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_values_keep_source_string_coercion() {
        assert_eq!(javascript_number(-0.0), "0");
        assert_eq!(javascript_number(1e21), "1e+21");
        assert_eq!(javascript_number(1e-7), "1e-7");
        let value = LuaValue::Array(vec![
            LuaValue::Null,
            LuaValue::Number(3.0),
            LuaValue::Array(vec![LuaValue::String("x".into()), LuaValue::Boolean(false)]),
            LuaValue::Object(Default::default()),
        ]);
        assert_eq!(javascript_string(&value), ",3,x,false,[object Object]");
    }
}
