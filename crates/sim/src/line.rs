//! Shared line quads and packed colors from `core-kernels/native-line.ts`.
//! Retail: 0x00455840 / 0x005E4510 -> 0x0041FB90.

use crate::Vec2;
use crate::float::{fround, hypot};

#[must_use]
pub fn native_line_vertices(start: Vec2, end: Vec2, width: f64) -> [f64; 8] {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = hypot(dx, dy);
    assert!(
        length > 0.0 && width.is_finite() && width > 0.0,
        "Native line requires distinct endpoints and positive width"
    );
    let px = -dy / length * width * 0.5;
    let py = dx / length * width * 0.5;
    [
        start.x - px,
        start.y - py,
        start.x + px,
        start.y + py,
        end.x - px,
        end.y - py,
        end.x + px,
        end.y + py,
    ]
    .map(fround)
}

#[must_use]
pub fn native_line_color(red: f64, green: f64, blue: f64, alpha: f64) -> u32 {
    // The source truncates and wraps each channel with &0xff, without clamping.
    let channel = |value| (fround(value) * 255.0).trunc().rem_euclid(256.0) as u32;
    channel(red) | channel(green) << 8 | channel(blue) << 16 | channel(alpha) << 24
}
