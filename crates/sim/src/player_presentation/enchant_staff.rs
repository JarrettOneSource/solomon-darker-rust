//! `player-enchant-staff-presentation.ts`. The socket endpoints come from the
//! already-loaded Clothes hand record; the source extracted the same points into
//! `player-staff-attachment-program.json`. No second socket table is needed.

use crate::float;
use std::f64::consts::PI;

use crate::Vec2;
use crate::float::{fround, hypot};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStaffAttachmentFrame {
    pub start: Vec2,
    pub end: Vec2,
    pub front: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnchantStaffDrawInput {
    pub attachment: NativeStaffAttachmentFrame,
    pub effective_rank: u32,
    pub heading_index: usize,
    pub living: bool,
    pub native_staff: bool,
    pub pose: usize,
    pub selected_primary_skill_id: Option<u32>,
    pub selector: usize,
    pub tick: f64,
    pub weld_build_id: Option<u32>,
    pub width_sample: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnchantStaffDrawPlan {
    pub aura_record: Option<usize>,
    pub body_record: usize,
    pub end: Vec2,
    pub far_alpha: f64,
    pub front: bool,
    pub near_alpha: f64,
    pub start: Vec2,
    pub tint: u32,
    pub vertices: [Vec2; 4],
    pub width_factor: f64,
}

#[must_use]
pub fn native_enchant_staff_glow_alpha(tick: f64) -> f64 {
    fround(float::sin(tick * 5.0 * PI / 180.0) * 0.2 + 0.5)
}

#[must_use]
pub fn native_enchant_staff_glow_tint(
    selected_primary_skill_id: Option<u32>,
    weld_build_id: Option<u32>,
) -> Option<u32> {
    let selected = selected_primary_skill_id?;
    let base = match selected {
        8 | 80 => [1.0, 0.1, 1.0],
        16 => [1.0, 0.35, 0.1],
        24 => [0.1, 1.0, 1.0],
        32 => [0.1, 0.5, 1.0],
        40 => [0.1, 1.0, 0.1],
        52 => {
            let build = weld_build_id.expect("Enchant Staff Weld color requires its build");
            WELD_BASE_COLORS[(build - 1000) as usize]
        }
        _ => panic!("unsupported Enchant Staff selected primary {selected}"),
    };
    let luminance = base[0] * f64::from(0.3086_f32)
        + base[1] * f64::from(0.6094_f32)
        + base[2] * f64::from(0.082_f32);
    let channel = |value: f64| ((luminance * 0.85 + value * 0.15) * 255.0).round() as u32;
    Some(channel(base[0]) << 16 | channel(base[1]) << 8 | channel(base[2]))
}

#[must_use]
pub fn native_enchant_staff_draw_plan(
    input: NativeEnchantStaffDrawInput,
) -> Option<NativeEnchantStaffDrawPlan> {
    assert!(input.selector < 6, "Enchant Staff selector is within 0..6");
    assert!(input.pose < 10, "Enchant Staff attachment pose is within 0..10");
    if !input.living
        || !input.native_staff
        || input.selected_primary_skill_id.is_none()
        || input.effective_rank == 0
    {
        return None;
    }
    let frame = input.attachment;
    let tint =
        native_enchant_staff_glow_tint(input.selected_primary_skill_id, input.weld_build_id)?;
    let width_sample = input.width_sample.unwrap_or_else(|| {
        visual_width_sample(input.tick, input.heading_index % 24, input.pose, input.selector)
    });
    assert!(
        width_sample.is_finite() && (0.0..=1.5).contains(&width_sample),
        "Enchant Staff width sample is within 0..=1.5"
    );
    let delta = Vec2::new(frame.end.x - frame.start.x, frame.end.y - frame.start.y);
    let length = hypot(delta.x, delta.y);
    assert!(length > 0.0, "Enchant Staff attachment endpoints are distinct");
    let along = Vec2::new(delta.x / length, delta.y / length);
    let perpendicular = Vec2::new(-along.y, along.x);
    let width_factor = fround(2.0 + width_sample);
    let half_width = [6.0, 6.0, 6.0, 6.0, 8.0, 9.0][input.selector] * 0.5 * width_factor;
    let extended_end = Vec2::new(frame.end.x + along.x * 5.0, frame.end.y + along.y * 5.0);
    let near_alpha = native_enchant_staff_glow_alpha(input.tick);
    Some(NativeEnchantStaffDrawPlan {
        aura_record: if input.selector < 2 { Some(11 + input.selector) } else { None },
        body_record: 5 + input.selector,
        end: frame.end,
        far_alpha: fround(near_alpha * 0.35),
        front: frame.front,
        near_alpha,
        start: frame.start,
        tint,
        vertices: [
            Vec2::new(
                frame.start.x - perpendicular.x * half_width,
                frame.start.y - perpendicular.y * half_width,
            ),
            Vec2::new(
                frame.start.x + perpendicular.x * half_width,
                frame.start.y + perpendicular.y * half_width,
            ),
            Vec2::new(
                extended_end.x - perpendicular.x * half_width,
                extended_end.y - perpendicular.y * half_width,
            ),
            Vec2::new(
                extended_end.x + perpendicular.x * half_width,
                extended_end.y + perpendicular.y * half_width,
            ),
        ],
        width_factor,
    })
}

fn visual_width_sample(tick: f64, heading: usize, pose: usize, selector: usize) -> f64 {
    // Source Math.trunc(tick) | 0 wraps before the cosmetic bit mixer.
    let mut mixed = tick.trunc().rem_euclid(4_294_967_296.0) as u32;
    mixed ^= (heading as u32 + 1).wrapping_mul(0x045d_9f3b);
    mixed ^= (pose as u32 + 1).wrapping_mul(0x119d_e1f3);
    mixed ^= (selector as u32 + 1).wrapping_mul(0x0003_449f);
    mixed = (mixed << 21) ^ mixed;
    mixed ^= mixed >> 11;
    mixed = ((mixed << 4) ^ mixed).wrapping_mul(0x0a67_cfcf);
    let word = i64::from(mixed as i32).unsigned_abs() % 100_001;
    fround(word as f64 / 100_000.0 * 1.5)
}

const WELD_BASE_COLORS: [[f64; 3]; 15] = [
    [1.0, 0.1, 0.5],
    [1.0, 0.5, 1.0],
    [1.0, 0.75, 1.0],
    [1.0, 0.75, 0.5],
    [1.0, 0.75, 1.0],
    [0.75, 0.75, 0.75],
    [1.0, 0.75, 1.0],
    [1.0, 0.75, 0.5],
    [0.8, 1.0, 1.0],
    [0.9, 1.0, 1.0],
    [1.0, 0.1, 0.5],
    [1.0, 0.35, 0.1],
    [0.1, 0.5, 1.0],
    [0.1, 1.0, 1.0],
    [0.1, 1.0, 0.1],
];
