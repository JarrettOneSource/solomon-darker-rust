//! Complete appendage construction and articulated Leviathan geometry from Website
//! `core-kernels/native-secondary-leviathan.ts`.

use crate::float;
use crate::float::{atan2, fround};
use crate::{NativeRng, Vec2};

pub const NATIVE_LEVIATHAN_ACTIVE_FIRST_AGE: u64 = 41;
pub const NATIVE_LEVIATHAN_ACTIVE_LAST_AGE: u64 = 1640;
pub const NATIVE_LEVIATHAN_LIFETIME_TICKS: u64 = 1664;
pub const NATIVE_LEVIATHAN_TARGET_RANGE: f64 = 300.0;
pub const NATIVE_ETHER_BOLT_LIFETIME_TICKS: u64 = 200;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeLeviathanAppendageBirth {
    pub bank: u32,
    pub base_offset: Vec2,
    pub countdown: i32,
    pub heading_degrees: f64,
    pub spin_degrees: f64,
    pub spin_step_degrees: f64,
    pub sprite_scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLeviathanBirth {
    pub appendages: Vec<NativeLeviathanAppendageBirth>,
    pub maximum_scale: f64,
    pub quantity: usize,
    pub rotation_radians: f64,
}

#[derive(Clone, Copy)]
struct Layout {
    base_x: f64,
    base_y_minimum: f64,
    base_y_maximum: f64,
    heading_degrees: f64,
    sprite_scale: f64,
}

const LAYOUTS: [&[Layout]; 5] = [
    &[Layout {
        base_x: 0.0,
        base_y_minimum: 20.0,
        base_y_maximum: 40.0,
        heading_degrees: 10.0,
        sprite_scale: 2.1,
    }],
    &[
        Layout {
            base_x: -10.0,
            base_y_minimum: 20.0,
            base_y_maximum: 40.0,
            heading_degrees: 10.0,
            sprite_scale: 2.1,
        },
        Layout {
            base_x: 10.0,
            base_y_minimum: 10.0,
            base_y_maximum: 20.0,
            heading_degrees: 135.0,
            sprite_scale: 2.0,
        },
    ],
    &[
        Layout {
            base_x: 0.0,
            base_y_minimum: 20.0,
            base_y_maximum: 40.0,
            heading_degrees: 10.0,
            sprite_scale: 2.1,
        },
        Layout {
            base_x: 15.0,
            base_y_minimum: 10.0,
            base_y_maximum: 20.0,
            heading_degrees: 135.0,
            sprite_scale: 2.0,
        },
        Layout {
            base_x: -15.0,
            base_y_minimum: 10.0,
            base_y_maximum: 20.0,
            heading_degrees: 225.0,
            sprite_scale: 2.0,
        },
    ],
    &[
        Layout {
            base_x: -10.0,
            base_y_minimum: 20.0,
            base_y_maximum: 40.0,
            heading_degrees: 10.0,
            sprite_scale: 2.1,
        },
        Layout {
            base_x: 10.0,
            base_y_minimum: 20.0,
            base_y_maximum: 50.0,
            heading_degrees: 10.0,
            sprite_scale: 2.0,
        },
        Layout {
            base_x: 18.0,
            base_y_minimum: 10.0,
            base_y_maximum: 20.0,
            heading_degrees: 135.0,
            sprite_scale: 2.0,
        },
        Layout {
            base_x: -18.0,
            base_y_minimum: 10.0,
            base_y_maximum: 20.0,
            heading_degrees: 225.0,
            sprite_scale: 2.0,
        },
    ],
    &[
        Layout {
            base_x: 0.0,
            base_y_minimum: 40.0,
            base_y_maximum: 50.0,
            heading_degrees: 10.0,
            sprite_scale: 2.25,
        },
        Layout {
            base_x: -18.0,
            base_y_minimum: 20.0,
            base_y_maximum: 40.0,
            heading_degrees: 10.0,
            sprite_scale: 2.1,
        },
        Layout {
            base_x: 18.0,
            base_y_minimum: 20.0,
            base_y_maximum: 50.0,
            heading_degrees: 10.0,
            sprite_scale: 2.0,
        },
        Layout {
            base_x: 18.0,
            base_y_minimum: 10.0,
            base_y_maximum: 20.0,
            heading_degrees: 135.0,
            sprite_scale: 2.0,
        },
        Layout {
            base_x: -18.0,
            base_y_minimum: 10.0,
            base_y_maximum: 20.0,
            heading_degrees: 225.0,
            sprite_scale: 2.0,
        },
    ],
];

const MUZZLE_SOCKETS: [Vec2; 15] = [
    Vec2::new(0.0, -44.5),
    Vec2::new(7.5, -43.5),
    Vec2::new(13.0, -40.0),
    Vec2::new(16.5, -34.5),
    Vec2::new(17.5, -28.5),
    Vec2::new(15.5, -22.5),
    Vec2::new(10.5, -17.5),
    Vec2::new(3.5, -15.5),
    Vec2::new(-3.5, -15.5),
    Vec2::new(-10.5, -17.5),
    Vec2::new(-15.5, -22.5),
    Vec2::new(-17.5, -28.5),
    Vec2::new(-16.5, -34.5),
    Vec2::new(-13.0, -40.0),
    Vec2::new(-7.5, -43.5),
];

#[must_use]
pub fn create_native_leviathan_birth(
    rng: &mut NativeRng,
    configured_maximum: f64,
    force_maximum: bool,
) -> NativeLeviathanBirth {
    let rotation = rng.draw_scaled_float(360.0, false);
    let maximum = (configured_maximum + 0.5).floor().clamp(1.0, 5.0) as i32;
    let quantity = if force_maximum { maximum } else { rng.draw_integer(maximum) + 1 } as usize;
    let mut appendages = Vec::with_capacity(quantity);
    for layout in LAYOUTS[quantity - 1] {
        let base_y = rng.draw_scaled_float(layout.base_y_maximum - layout.base_y_minimum, false);
        let spin = rng.draw_scaled_float(360.0, false);
        let spin_step = rng.draw_scaled_float(1.0, false);
        let bank = rng.draw_integer(2) as u32;
        let countdown = rng.draw_integer(100);
        appendages.push(NativeLeviathanAppendageBirth {
            bank,
            base_offset: Vec2::new(layout.base_x, fround(-(layout.base_y_minimum + base_y))),
            countdown,
            heading_degrees: layout.heading_degrees,
            spin_degrees: spin,
            spin_step_degrees: fround(2.0 + spin_step),
            sprite_scale: layout.sprite_scale,
        });
    }
    let maximum_scale = match quantity {
        1 => 0.75,
        2 => 0.85_f32 as f64,
        3 => 0.95_f32 as f64,
        _ => 1.0,
    };
    NativeLeviathanBirth {
        appendages,
        maximum_scale,
        quantity,
        rotation_radians: rotation * std::f64::consts::PI / 180.0,
    }
}

#[must_use]
pub fn native_leviathan_current_scale(age_ticks: u64) -> f64 {
    if age_ticks <= 40 {
        let mut scale = 0.0;
        for _ in 0..age_ticks {
            scale = fround(scale + 0.025_f32 as f64);
        }
        return scale;
    }
    if age_ticks < NATIVE_LEVIATHAN_ACTIVE_LAST_AGE {
        return 1.0;
    }
    let mut scale = 1.0;
    for _ in 0..(age_ticks - NATIVE_LEVIATHAN_ACTIVE_LAST_AGE + 1).min(25) {
        scale = fround(scale - 0.04_f32 as f64);
    }
    scale.max(0.0)
}

#[must_use]
pub fn native_leviathan_phase(age_ticks: u64) -> u32 {
    if age_ticks <= 40 {
        0
    } else if age_ticks < NATIVE_LEVIATHAN_ACTIVE_LAST_AGE {
        1
    } else {
        2
    }
}

#[must_use]
pub fn native_leviathan_active(age_ticks: u64) -> bool {
    (NATIVE_LEVIATHAN_ACTIVE_FIRST_AGE..=NATIVE_LEVIATHAN_ACTIVE_LAST_AGE).contains(&age_ticks)
}

#[must_use]
pub fn native_leviathan_heading_vector(heading_degrees: f64) -> Vec2 {
    let radians = heading_degrees * std::f64::consts::PI / 180.0;
    Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)))
}

#[must_use]
pub fn native_leviathan_heading_degrees(from: Vec2, to: Vec2) -> f64 {
    let degrees = atan2(to.x - from.x, -(to.y - from.y)) * 180.0 / std::f64::consts::PI;
    if degrees < 0.0 { degrees + 360.0 } else { degrees }
}

#[must_use]
pub fn native_leviathan_direction_frame(heading_degrees: f64) -> usize {
    let rounded_heading = (heading_degrees + 0.5).floor();
    let frame = ((rounded_heading + 12.0) / 24.0).floor() as usize;
    if frame > 14 { frame - 15 } else { frame }
}

#[must_use]
pub fn native_leviathan_appendage_record(bank: u32, heading_degrees: f64) -> usize {
    343 + bank as usize * 15 + native_leviathan_direction_frame(heading_degrees)
}

#[must_use]
pub fn native_leviathan_appendage_local_root(
    base_offset: Vec2,
    recoil: Vec2,
    spin_degrees: f64,
    deployment: f64,
) -> Vec2 {
    let spin = native_leviathan_heading_vector(spin_degrees);
    Vec2::new(
        fround(base_offset.x + recoil.x + 2.0 * spin.x),
        fround(base_offset.y + recoil.y + 4.0 * spin.y + 35.0 + deployment * 100.0),
    )
}

#[must_use]
pub fn native_leviathan_appendage_presentation_root(
    parent_position: Vec2,
    parent_composite_scale: f64,
    local_root: Vec2,
) -> Vec2 {
    Vec2::new(
        fround(parent_position.x + parent_composite_scale * local_root.x),
        fround(parent_position.y + parent_composite_scale * local_root.y),
    )
}

#[must_use]
pub fn native_leviathan_muzzle_position(
    parent_position: Vec2,
    local_root: Vec2,
    bank: u32,
    heading_degrees: f64,
    sprite_scale: f64,
) -> Vec2 {
    assert!(bank <= 1, "Leviathan appendage has one of its two authored banks");
    // Both authored banks use the same socket positions; their texture records differ.
    let socket = MUZZLE_SOCKETS[native_leviathan_direction_frame(heading_degrees)];
    let wobble =
        float::sin(heading_degrees * std::f64::consts::PI / 180.0) * 5.0 * std::f64::consts::PI
            / 180.0;
    let cos = float::cos(wobble);
    let sin = float::sin(wobble);
    Vec2::new(
        fround(parent_position.x + local_root.x + sprite_scale * (socket.x * cos - socket.y * sin)),
        fround(parent_position.y + local_root.y + sprite_scale * (socket.x * sin + socket.y * cos)),
    )
}

#[must_use]
pub fn native_leviathan_inside_target_lane(
    origin: Vec2,
    heading_degrees: f64,
    target: Vec2,
) -> bool {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let distance_squared = dx * dx + dy * dy;
    if distance_squared >= NATIVE_LEVIATHAN_TARGET_RANGE * NATIVE_LEVIATHAN_TARGET_RANGE {
        return false;
    }
    if distance_squared == 0.0 {
        return true;
    }
    let target_heading = native_leviathan_heading_degrees(origin, target);
    let delta = (((target_heading - heading_degrees + 540.0) % 360.0) - 180.0).abs();
    delta <= 25.0
}
