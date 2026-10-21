//! Title, Create and pause presentation from Website `renderer/title-menu-render-contract.ts`,
//! `create-menu-motion.ts` and `pause-menu-contract.ts`. Time is supplied by the browser.

pub mod cheats;
pub mod create_name;
pub mod dark_cloud;
pub mod loading;
pub mod party_directory;
pub mod save_mods;
pub mod shared;

use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};
use crate::profile::{WizardDiscipline, WizardElement};
use crate::rng::NativeRng;

pub const CREATE_ENTRY_ANIMATION_MS: f64 = 2330.0;
pub const CREATE_SELECTION_ANIMATION_MS: f64 = 2630.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateHandPose {
    Fist,
    Cupped,
    Raised,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CreateMotion {
    pub disciplines_visible: bool,
    pub elements_visible: bool,
    pub left_impulse: Vec2,
    pub left_offset: Vec2,
    pub left_pose: CreateHandPose,
    pub right_impulse: Vec2,
    pub right_offset: Vec2,
    pub right_pose: CreateHandPose,
    pub settled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CreateElementReveal {
    pub opacity: f64,
    pub position: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CreateSelectedElementMotion {
    pub position: Vec2,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TitleGrave {
    pub image_index: usize,
    pub rotation: f64,
    pub x: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TitleGraveRow {
    pub baseline: f64,
    pub gray: f64,
    pub scale: f64,
    pub speed_per_tick: f64,
    pub graves: Vec<TitleGrave>,
    pub next_image_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TitleState {
    pub rows: [TitleGraveRow; 3],
    pub cloak_phase: f64,
    pub ticks: u64,
    widths: [f64; 9],
    random: NativeRng,
}

impl TitleState {
    #[must_use]
    pub fn new(widths: [f64; 9]) -> Self {
        let mut random = NativeRng::new(0x5d1f_2a93);
        let rows = [(552.0, 0.0, 0.3, 0.05), (712.0, 0.5, 0.7, 0.1), (900.0, 1.0, 1.1, 0.2)].map(
            |(baseline, gray, scale, speed_per_tick)| {
                let mut row = TitleGraveRow {
                    baseline,
                    gray,
                    scale,
                    speed_per_tick,
                    graves: Vec::new(),
                    next_image_index: random.draw_integer(3) as usize,
                };
                while !step_title_grave_row(&mut row, &widths, &mut random, true) {}
                row
            },
        );
        Self { rows, cloak_phase: 0.0, ticks: 0, widths, random }
    }

    pub fn step(&mut self) {
        for row in &mut self.rows {
            step_title_grave_row(row, &self.widths, &mut self.random, false);
        }
        let theta = self.ticks as f64 * std::f64::consts::PI / 180.0;
        self.cloak_phase = (self.cloak_phase + 0.025 + 0.005 * float::sin(theta)) % 5.0;
        self.ticks += 1;
    }
}

fn step_title_grave_row(
    row: &mut TitleGraveRow,
    widths: &[f64; 9],
    random: &mut NativeRng,
    seed: bool,
) -> bool {
    let advance =
        row.speed_per_tick + if seed { (row.speed_per_tick * 250.0).min(25.0) } else { 0.0 };
    for grave in &mut row.graves {
        grave.x -= advance;
    }
    let removed = row.graves.iter().any(|grave| grave.x < -200.0);
    row.graves.retain(|grave| grave.x >= -200.0);
    let spawn = row.graves.last().is_none_or(|last| {
        last.x + (widths[row.next_image_index] + widths[last.image_index]) * 0.5 * row.scale
            < 1700.0
    });
    if spawn {
        // The title helper scales a double directly; NativeRng's float API would add
        // single-precision stores that this presentation reference does not perform.
        let magnitude = f64::from(random.draw_integer(100_001)) / 100_000.0 * 10.0;
        let rotation = if random.draw_integer(2) == 1 { -magnitude } else { magnitude };
        let image_index = row.next_image_index;
        row.graves.push(TitleGrave {
            image_index,
            rotation,
            x: 1700.0 + 50.0 * row.scale * row.scale,
        });
        row.next_image_index = if random.draw_integer(10) == 1 {
            random.draw_integer(9) as usize
        } else {
            (image_index + 1) % 9
        };
    }
    removed
}

#[must_use]
pub fn title_backdrop_offsets_at(seconds: f64) -> [f64; 4] {
    let elapsed = seconds.max(0.0);
    let detail = (0.384 + elapsed / (200.0 / 3.0)) % 1.0;
    let shadow = (0.075 + elapsed / (400.0 / 9.0)) % 1.0;
    let cloud_width = 512.0 * 553.0 / 218.0;
    [
        (1.0 - detail) * cloud_width,
        (1.0 - shadow) * cloud_width,
        385.0 + elapsed * 21.0,
        356.0 + elapsed * 3.0,
    ]
}

#[must_use]
pub fn native_pause_menu_reveal(opening: bool, elapsed_ms: f64) -> f64 {
    let count = (elapsed_ms.max(0.0) / 10.0).floor() as usize;
    let mut reveal = if opening { 0.0 } else { 1.0 };
    for _ in 0..count.min(if opening { 29 } else { 20 }) {
        reveal = fround(
            reveal + if opening { 0.035_000_000_149_011_61 } else { -0.050_000_000_745_058_06 },
        );
        if opening && reveal > 1.0 {
            return 1.0;
        }
        if !opening && reveal < 0.0 {
            return 0.0;
        }
    }
    reveal
}

#[must_use]
pub fn create_entry_motion_at(elapsed_ms: f64) -> CreateMotion {
    let elapsed = elapsed_ms.max(0.0);
    let (left_impulse, left_offset, left_pose) =
        native_open_state_at(elapsed, Vec2::new(-50.0, 200.0), 120, 0x12_0e17);
    CreateMotion {
        disciplines_visible: false,
        elements_visible: elapsed >= 1340.0,
        left_impulse,
        left_offset,
        left_pose,
        right_impulse: Vec2::ZERO,
        right_offset: Vec2::new(50.0, 300.0),
        right_pose: CreateHandPose::Fist,
        settled: elapsed >= 1400.0,
    }
}

#[must_use]
pub fn create_selection_motion_at(elapsed_ms: f64) -> CreateMotion {
    let elapsed = elapsed_ms.max(0.0);
    let (right_impulse, right_offset, right_pose) =
        native_open_state_at(elapsed - 980.0, Vec2::new(50.0, 300.0), 51, 0x05_1ec7);
    CreateMotion {
        disciplines_visible: elapsed >= 1640.0,
        elements_visible: false,
        left_impulse: Vec2::ZERO,
        left_offset: if elapsed >= 980.0 {
            Vec2::new(-125.90988, 200.0)
        } else {
            left_selection_offset_at(elapsed)
        },
        left_pose: if elapsed < 910.0 { CreateHandPose::Raised } else { CreateHandPose::Cupped },
        right_impulse,
        right_offset,
        right_pose,
        settled: elapsed >= 1680.0,
    }
}

#[must_use]
pub fn create_hand_idle_offset_at(elapsed_ms: f64) -> Vec2 {
    let ticks = (elapsed_ms.max(0.0) / 10.0).floor();
    let phase = ticks * 0.5 * std::f64::consts::PI / 180.0;
    Vec2::new(float::sin(phase) * 5.0, float::sin(phase * 0.5) * 2.5)
}

#[must_use]
pub fn create_element_reveal_motion_at(
    element: WizardElement,
    elapsed_ms: f64,
) -> CreateElementReveal {
    let ticks = reveal_ticks(elapsed_ms.max(0.0), 1340.0);
    let remainder = reveal_remainder(ticks);
    let settled = element_start(element);
    CreateElementReveal {
        opacity: ticks as f64 / 100.0,
        position: Vec2::new(
            settled.x + (775.0 - settled.x) * remainder,
            settled.y + (510.0 - settled.y) * remainder,
        ),
    }
}

#[must_use]
pub fn create_discipline_reveal_motion_at(discipline: WizardDiscipline, elapsed_ms: f64) -> Vec2 {
    let x = match discipline {
        WizardDiscipline::Arcane => 1025.0,
        WizardDiscipline::Body => 875.0,
        WizardDiscipline::Mind => 725.0,
        WizardDiscipline::Air
        | WizardDiscipline::Earth
        | WizardDiscipline::Ether
        | WizardDiscipline::Fire
        | WizardDiscipline::Water => {
            unreachable!("Create reveals only its three ordinary manual discipline choices")
        }
    };
    Vec2::new(x + 50.0 * reveal_remainder(reveal_ticks(elapsed_ms.max(0.0), 1640.0)), 460.0)
}

#[must_use]
pub fn create_selected_element_motion_at(
    element: WizardElement,
    elapsed_ms: f64,
) -> CreateSelectedElementMotion {
    let elapsed = elapsed_ms.max(0.0);
    if elapsed >= 980.0 {
        return CreateSelectedElementMotion { position: Vec2::new(450.0, 660.0), scale: 3.0 };
    }
    let ticks = ((elapsed - 600.0).max(0.0) / 10.0).floor() as usize;
    let start = element_start(element);
    let mut cursor = 0.0;
    let mut position = start;
    let mut scale = 1.0;
    let mut offset = Vec2::ZERO;
    for _ in 0..ticks {
        for _ in 0..2 {
            offset = selection_step(offset);
            position =
                natural_path_at([start, Vec2::new(650.0, 685.0), Vec2::new(450.0, 660.0)], cursor);
            cursor = offset.y / 200.0 * 2.0;
            scale = offset.y / 200.0 * 2.0 + 1.0;
        }
    }
    CreateSelectedElementMotion { position, scale }
}

fn native_impulse(tick: i32, salt: i32) -> Vec2 {
    let mut value = (tick ^ salt).wrapping_mul(0x045d_9f3b);
    value = (value ^ ((value as u32 >> 16) as i32)).wrapping_mul(0x045d_9f3b);
    value ^= (value as u32 >> 16) as i32;
    let angle = f64::from(value as u32) / 4_294_967_296.0 * std::f64::consts::PI * 2.0;
    let next = (value ^ 0x9e37_79b9u32 as i32).wrapping_mul(0x27d4_eb2d);
    let magnitude = f64::from((next ^ ((next as u32 >> 15) as i32)) as u32) / 4_294_967_296.0 * 5.0;
    Vec2::new(fround(float::cos(angle) * magnitude), fround(-float::sin(angle) * magnitude))
}

fn shorten_and_damp(offset: Vec2) -> Vec2 {
    let length = hypot(offset.x, offset.y);
    if length <= 3.5 {
        return Vec2::ZERO;
    }
    let shortened = fround((length - 3.5) / length);
    Vec2::new(
        fround(fround(offset.x * shortened) * fround(0.7)),
        fround(fround(offset.y * shortened) * fround(0.8)),
    )
}

fn native_open_state_at(
    elapsed_ms: f64,
    initial_offset: Vec2,
    initial_timer: i32,
    salt: i32,
) -> (Vec2, Vec2, CreateHandPose) {
    let ticks = (elapsed_ms.max(0.0) / 10.0).floor() as i32;
    let mut offset = initial_offset;
    let mut impulse = Vec2::ZERO;
    let mut pose = CreateHandPose::Fist;
    let mut recoil_phase = 0.0;
    let mut timer = initial_timer;
    for tick in 1..=ticks {
        timer -= 1;
        let mut recoil_y = 0.0;
        if timer < 1 {
            if timer == 0 {
                recoil_phase = 0.5;
            }
            recoil_y = fround(float::sin(recoil_phase * std::f64::consts::PI / 180.0) * 150.0);
            recoil_phase = fround(recoil_phase - 0.025).max(0.0);
            offset = shorten_and_damp(offset);
            let speed = hypot(offset.x, offset.y);
            if speed < 10.0 {
                pose = CreateHandPose::Cupped;
            }
            if speed < 1.0 {
                pose = CreateHandPose::Raised;
            }
        }
        if timer < 100 && pose == CreateHandPose::Fist {
            impulse = native_impulse(tick, salt);
            offset =
                Vec2::new(fround(fround(offset.x - 0.01) * fround(1.01)), fround(offset.y + 1.0));
        }
        impulse.y = fround(impulse.y + recoil_y);
    }
    (impulse, offset, pose)
}

fn selection_step(offset: Vec2) -> Vec2 {
    Vec2::new(
        fround(offset.x - fround(offset.y / 30.0)),
        fround(fround(offset.y + 0.25) * fround(1.05)).min(200.0),
    )
}

fn left_selection_offset_at(elapsed_ms: f64) -> Vec2 {
    let ticks = (((elapsed_ms - 600.0).max(0.0) / 10.0).floor() as usize).min(38);
    let mut offset = Vec2::ZERO;
    for _ in 0..ticks {
        for _ in 0..2 {
            offset = selection_step(offset);
        }
    }
    offset
}

fn reveal_ticks(elapsed_ms: f64, start: f64) -> usize {
    if elapsed_ms < start {
        return 0;
    }
    (((elapsed_ms - start) / 10.0).floor() as usize + 1).min(100)
}

fn reveal_remainder(ticks: usize) -> f64 {
    let mut remainder = 1.0;
    for _ in 0..ticks {
        remainder = fround(remainder * fround(0.920_000_016_689_300_5));
    }
    remainder
}

fn element_start(element: WizardElement) -> Vec2 {
    match element {
        WizardElement::Air => Vec2::new(816.346, 654.189),
        WizardElement::Earth => Vec2::new(656.798, 417.651),
        WizardElement::Ether => Vec2::new(826.303, 369.046),
        WizardElement::Fire => Vec2::new(924.909, 515.235),
        WizardElement::Water => Vec2::new(650.644, 593.879),
    }
}

/// `native-natural-spline.ts` specialized to Create's three authored control points.
/// Its two segments have natural (zero-curvature) endpoints; operation order is preserved.
fn natural_path_at(points: [Vec2; 3], cursor: f64) -> Vec2 {
    if cursor <= 0.0 {
        return points[0];
    }
    if cursor >= 2.0 {
        return points[2];
    }
    let axis = |values: [f64; 3]| {
        let alpha = 3.0 * (values[2] - values[1]) - 3.0 * (values[1] - values[0]);
        let middle = alpha / 4.0;
        let coefficients = [
            [values[1] - values[0] - middle / 3.0, 0.0, middle / 3.0],
            [values[2] - values[1] - 2.0 * middle / 3.0, middle, -middle / 3.0],
        ];
        let index = cursor.floor() as usize;
        let progress = cursor - index as f64;
        let [linear, quadratic, cubic] = coefficients[index];
        values[index] + progress * (linear + progress * (quadratic + progress * cubic))
    };
    Vec2::new(axis(points.map(|point| point.x)), axis(points.map(|point| point.y)))
}
