//! Quickbar cooldown/icon presentation from Website `skill-quickbar.ts`.

use crate::native_ui_text::{
    NativeUiFontName, NativeUiTextLayout, NativeUiTextSpec, layout_native_ui_text,
    measure_native_ui_text,
};
use crate::secondary::NATIVE_SECONDARY_GLOBAL_COOLDOWN_TICKS;

pub struct NativeBeltBindingLayout {
    pub advance: f64,
    pub backing_left: f64,
    pub backing_width: f64,
    pub text: NativeUiTextLayout,
}

/// Source `layoutNativeQuickbarBinding`: the authored Belt font at baseline64,
/// with a three-piece key backing sized from glyph advance rather than HTML ink.
#[must_use]
pub fn layout_native_quickbar_binding(text: &str) -> NativeBeltBindingLayout {
    let advance = measure_native_ui_text(text, NativeUiFontName::Belt, 1.0);
    let backing_width = advance + 6.0;
    let mut spec = NativeUiTextSpec::new(NativeUiFontName::Belt, text, 26.5, 64.0);
    spec.tint = 0x000000;
    NativeBeltBindingLayout {
        advance,
        backing_left: (53.0 - backing_width) / 2.0,
        backing_width,
        text: layout_native_ui_text(&spec),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NativeSkillQuickbarCooldownPresentation {
    pub capacity: f64,
    pub remaining: f64,
}

#[must_use]
pub fn native_skill_quickbar_icon_alpha(cooldown: bool, unavailable: bool) -> f64 {
    if cooldown {
        0.25
    } else if unavailable {
        0.375
    } else {
        0.75
    }
}

#[must_use]
pub fn native_skill_quickbar_cooldown_presentation(
    row_remaining: f64,
    row_capacity: f64,
    global_remaining: f64,
) -> NativeSkillQuickbarCooldownPresentation {
    if row_capacity <= 0.0 || row_capacity.is_nan() {
        return NativeSkillQuickbarCooldownPresentation::default();
    }
    if row_remaining > 0.0 && global_remaining <= row_remaining {
        return NativeSkillQuickbarCooldownPresentation {
            capacity: row_capacity,
            remaining: row_remaining,
        };
    }
    NativeSkillQuickbarCooldownPresentation {
        capacity: NATIVE_SECONDARY_GLOBAL_COOLDOWN_TICKS,
        remaining: if global_remaining.is_nan() {
            global_remaining
        } else {
            global_remaining.max(0.0)
        },
    }
}

#[must_use]
pub fn native_cooldown_sector_path(remaining: f64, capacity: f64) -> String {
    if remaining <= 0.0 || capacity <= 0.0 || remaining.is_nan() || capacity.is_nan() {
        return String::new();
    }
    let quotient = remaining / capacity;
    let ratio = if quotient.is_nan() { quotient } else { quotient.min(1.0) };
    let start = 360.0 * (1.0 - ratio);
    let mut path = "M 26.5 26.5".to_owned();
    append_ray_point(&mut path, start);
    let mut boundary = ((start / 45.0).floor() + 1.0) * 45.0;
    while boundary <= 360.0 {
        append_ray_point(&mut path, boundary);
        boundary += 45.0;
    }
    path.push_str(" Z");
    path
}

fn append_ray_point(path: &mut String, degrees: f64) {
    let radians = degrees * std::f64::consts::PI / 180.0;
    let dx = radians.cos();
    let dy = -radians.sin();
    let scale = 26.5 / dx.abs().max(dy.abs());
    path.push_str(&format!(
        " L {} {}",
        coordinate(26.5 + dx * scale),
        coordinate(26.5 + dy * scale)
    ));
}

fn coordinate(value: f64) -> String {
    let rounded = (value * 1_000_000.0 + 0.5).floor() / 1_000_000.0;
    if rounded == 0.0 { "0".to_owned() } else { rounded.to_string() }
}
