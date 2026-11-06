//! Website `lib/format.ts`: shared units and relative time, platform locale for metadata.
pub(super) use crate::platform::format::{compare, date, lowercase};

pub(super) fn count(value: u64) -> Result<String, String> {
    if value >= 10_000 {
        Ok(format!("{}k", fixed(value as f64 / 1000.0, 1)?))
    } else {
        crate::platform::format::integer(value)
    }
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
    Ok(ryu_js::Buffer::new().format_to_fixed(value, digits).to_owned())
}

pub(super) fn time_ago(value: &str) -> String {
    let Some(date) = crate::platform::format::timestamp(value) else {
        return String::new();
    };
    let seconds = ((crate::platform::epoch_ms() - date) / 1000.0).max(0.0);
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
