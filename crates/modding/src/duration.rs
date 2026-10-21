//! The shared authored duration grammar used by catalog, rules and effects.

use solomon_sim::modding::LuaValue;

pub(crate) fn parse_duration_text(text: &str) -> Option<f64> {
    let (amount, scale) = if let Some(amount) = text.strip_suffix("ms") {
        (amount, 1.0)
    } else if let Some(amount) = text.strip_suffix('s') {
        (amount, 1_000.0)
    } else if let Some(amount) = text.strip_suffix('m') {
        (amount, 60_000.0)
    } else {
        (text.strip_suffix('h')?, 3_600_000.0)
    };
    let (whole, fraction) =
        amount.split_once('.').map_or((amount, None), |(whole, fraction)| (whole, Some(fraction)));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || whole.len() > 1 && whole.starts_with('0')
        || fraction.is_some_and(|value| {
            value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return None;
    }
    amount.parse::<f64>().ok().map(|value| value * scale)
}

pub(crate) fn duration_ticks_value(
    value: Option<&LuaValue>,
    rate: u32,
    field: &str,
) -> Result<u64, String> {
    let milliseconds = match value {
        Some(LuaValue::Number(value))
            if value.is_finite()
                && value.fract() == 0.0
                && (0.0..=9_007_199_254_740_991.0).contains(value) =>
        {
            *value
        }
        Some(LuaValue::String(text)) => {
            parse_duration_text(text).ok_or_else(|| format!("{field} is invalid"))?
        }
        _ => return Err(format!("{field} is invalid")),
    };
    if !milliseconds.is_finite() || milliseconds > 86_400_000.0 {
        return Err(format!("{field} exceeds 24 hours"));
    }
    Ok((milliseconds * f64::from(rate) / 1_000.0).ceil().max(1.0) as u64)
}
