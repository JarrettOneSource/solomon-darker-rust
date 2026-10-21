//! Hagatha Seeker segments and textured quads from
//! `core-kernels/native-hagatha-effects.ts`. The host projects current loot
//! targets and draws these owner-local plans with the authored color ramp.

use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};
use crate::line::native_line_vertices;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHagathaSeekerProgram {
    pub distance_cap: f64,
    pub distance_cutoff: f64,
    pub end_distance_factor: f64,
    pub id_phase_degrees: f64,
    pub inner_radius: f64,
    pub join_radius: f64,
    pub pulse_amplitude: f64,
    pub pulse_base: f64,
    pub tick_phase_degrees: f64,
    pub transparent_color: u32,
    pub visible_color: u32,
    pub width: f64,
}

pub const NATIVE_HAGATHA_SEEKER_PROGRAM: NativeHagathaSeekerProgram = NativeHagathaSeekerProgram {
    distance_cap: 300.0,
    distance_cutoff: 100.0,
    end_distance_factor: 0.5,
    id_phase_degrees: 35.0,
    inner_radius: 35.0,
    join_radius: 50.0,
    pulse_amplitude: 0.1,
    pulse_base: 0.25,
    tick_phase_degrees: 2.0,
    transparent_color: 0xffffff,
    visible_color: 0xd8ba70,
    width: 3.0,
};

pub const NATIVE_HAGATHA_SEEKER_RAMP_RGBA: [u8; 8] = [255, 255, 255, 0, 216, 186, 112, 255];
pub const NATIVE_HAGATHA_SEEKER_TRANSPARENT_U: f64 = 0.25;
pub const NATIVE_HAGATHA_SEEKER_VISIBLE_U: f64 = 0.75;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHagathaSeekerTargetKind {
    Bonus,
    Gold,
    Sack,
}

impl NativeHagathaSeekerTargetKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bonus => "bonus",
            Self::Gold => "gold",
            Self::Sack => "sack",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHagathaSeekerTarget {
    pub id: usize,
    pub kind: NativeHagathaSeekerTargetKind,
    pub position: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHagathaSeekerSegment {
    pub alpha: f64,
    pub end: Vec2,
    pub end_visible: bool,
    pub start: Vec2,
    pub start_visible: bool,
    pub target_id: usize,
    pub target_kind: NativeHagathaSeekerTargetKind,
    pub width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHagathaSeekerMeshPlan {
    pub alpha: f64,
    pub alpha_byte: u8,
    pub uvs: [f64; 8],
    pub vertices: [f64; 8],
}

#[must_use]
pub fn native_hagatha_seeker_segments(
    player_position: Vec2,
    targets: &[NativeHagathaSeekerTarget],
    tick: f64,
) -> Vec<NativeHagathaSeekerSegment> {
    assert!(
        tick.is_finite() && tick >= 0.0,
        "Seeker presentation tick must be finite and non-negative"
    );
    let program = NATIVE_HAGATHA_SEEKER_PROGRAM;
    let mut segments = Vec::new();
    for target in targets {
        let dx = target.position.x - player_position.x;
        let dy = target.position.y - player_position.y;
        let distance = hypot(dx, dy);
        if distance.is_nan() || distance <= program.distance_cutoff {
            continue;
        }
        let direction_x = dx / distance;
        let direction_y = dy / distance;
        let capped_distance = distance.min(program.distance_cap);
        let phase_degrees =
            fround(program.tick_phase_degrees * tick + program.id_phase_degrees * target.id as f64);
        let phase_radians = fround(phase_degrees * fround(std::f64::consts::PI) / 180.0);
        let alpha = fround(
            program.pulse_base + program.pulse_amplitude * fround(float::sin(phase_radians)),
        );
        let point = |distance_along| {
            Vec2::new(
                fround(player_position.x + direction_x * distance_along),
                fround(player_position.y + direction_y * distance_along),
            )
        };
        segments.push(NativeHagathaSeekerSegment {
            alpha,
            end: point(program.join_radius),
            end_visible: true,
            start: point(program.inner_radius),
            start_visible: false,
            target_id: target.id,
            target_kind: target.kind,
            width: program.width,
        });
        segments.push(NativeHagathaSeekerSegment {
            alpha,
            end: point(capped_distance * program.end_distance_factor),
            end_visible: false,
            start: point(program.join_radius),
            start_visible: true,
            target_id: target.id,
            target_kind: target.kind,
            width: program.width,
        });
    }
    segments
}

#[must_use]
pub fn native_hagatha_seeker_mesh_plan(
    segment: NativeHagathaSeekerSegment,
) -> NativeHagathaSeekerMeshPlan {
    let start_u = if segment.start_visible {
        NATIVE_HAGATHA_SEEKER_VISIBLE_U
    } else {
        NATIVE_HAGATHA_SEEKER_TRANSPARENT_U
    };
    let end_u = if segment.end_visible {
        NATIVE_HAGATHA_SEEKER_VISIBLE_U
    } else {
        NATIVE_HAGATHA_SEEKER_TRANSPARENT_U
    };
    let alpha_byte = (segment.alpha.clamp(0.0, 1.0) * 255.0).trunc() as u8;
    NativeHagathaSeekerMeshPlan {
        alpha: f64::from(alpha_byte) / 255.0,
        alpha_byte,
        uvs: [start_u, 0.5, start_u, 0.5, end_u, 0.5, end_u, 0.5],
        vertices: native_line_vertices(segment.start, segment.end, segment.width),
    }
}
