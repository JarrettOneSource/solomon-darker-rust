//! Solomon's body, mask and private dirt paint state.
//!
//! References: `renderer/boneyard-solomon-render.ts`,
//! `boneyard-solomon-dirt-presentation.ts`, and `boneyard-solomon-view.ts`.
//! The encounter owns animation and dialogue; this module only projects its retained state.

use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::painter::PainterRegistration;

use super::SolomonDig;
use super::encounter::{SolomonDigCue, SolomonDigEvent, SolomonEncounter, SolomonPhase};

pub const SOLOMON_DIRT_DRAW_PASSES: usize = 2;
pub const SOLOMON_DIRT_VISIBLE_TICKS: u64 = 29;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolomonBodyBank {
    Dig,
    Dialogue,
    Walk,
}

impl SolomonBodyBank {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Dig => "dig",
            Self::Dialogue => "dialogue",
            Self::Walk => "walk",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolomonClipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolomonVisualState {
    pub body_bank: SolomonBodyBank,
    pub body_pose: usize,
    pub clip_rect_world: Option<SolomonClipRect>,
    pub direction: usize,
    pub mouth_pose: Option<usize>,
    pub native_body_record: usize,
    pub native_mouth_record: Option<usize>,
    pub offset_y: f64,
    pub grave_mark_visible: bool,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolomonPainterKind {
    Lantern,
    Actor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolomonPainter {
    pub kind: SolomonPainterKind,
    pub registration: PainterRegistration,
    pub world_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolomonSetPieceLighting {
    pub body_tint: u32,
    pub dirt_tint: u32,
    pub lantern_tint: u32,
}

/// The body is self-lit. Only dirt and the physical lantern query the accepted light field.
#[must_use]
pub fn native_solomon_set_piece_lighting(
    dig_position: Vec2,
    lantern_position: Vec2,
    tint_at: &dyn Fn(Vec2) -> u32,
) -> SolomonSetPieceLighting {
    SolomonSetPieceLighting {
        body_tint: 0xffffff,
        dirt_tint: tint_at(native_solomon_dirt_origin(dig_position)),
        lantern_tint: tint_at(lantern_position),
    }
}

#[must_use]
pub fn native_solomon_direction(heading_degrees: f64) -> usize {
    let normalized = ((heading_degrees % 360.0) + 360.0) % 360.0;
    ((normalized + 12.0) / 24.0).trunc() as usize % 15
}

#[must_use]
pub fn boneyard_solomon_painters(
    dig: &SolomonDig,
    encounter: Option<&SolomonEncounter>,
    lantern_registration: PainterRegistration,
    solomon_registration: PainterRegistration,
    lantern_position: Option<Vec2>,
) -> Vec<SolomonPainter> {
    let mut painters = Vec::new();
    if let Some(position) = lantern_position {
        painters.push(SolomonPainter {
            kind: SolomonPainterKind::Lantern,
            registration: lantern_registration,
            world_y: position.y,
        });
    }
    if encounter.is_none_or(|encounter| encounter.phase != SolomonPhase::Gone) {
        painters.push(SolomonPainter {
            kind: SolomonPainterKind::Actor,
            registration: solomon_registration,
            world_y: encounter.map_or(dig.position.y, |encounter| encounter.position.y),
        });
    }
    painters
}

#[must_use]
pub fn boneyard_solomon_visual_state(
    encounter: Option<&SolomonEncounter>,
    dig: &SolomonDig,
    tick: u64,
) -> SolomonVisualState {
    let Some(encounter) = encounter else {
        let program_index = tick / dig.ticks_per_frame % dig.frame_program.len() as u64;
        let pose = dig.frame_program[program_index as usize];
        return SolomonVisualState {
            body_bank: SolomonBodyBank::Dig,
            body_pose: pose,
            clip_rect_world: Some(clip_rect(dig.position, 100.0, 100.0, 200.0, 100.0)),
            direction: 0,
            mouth_pose: None,
            native_body_record: 2 + pose,
            native_mouth_record: None,
            offset_y: 5.0,
            grave_mark_visible: true,
            visible: true,
        };
    };
    let direction = native_solomon_direction(encounter.heading_degrees);
    match encounter.phase {
        SolomonPhase::Digging => SolomonVisualState {
            body_bank: SolomonBodyBank::Dig,
            body_pose: encounter.dig_frame,
            clip_rect_world: Some(clip_rect(encounter.position, 100.0, 100.0, 200.0, 100.0)),
            direction,
            mouth_pose: None,
            native_body_record: 2 + encounter.dig_frame,
            native_mouth_record: None,
            offset_y: 5.0 + encounter.dig_body_offset_y,
            grave_mark_visible: true,
            visible: true,
        },
        SolomonPhase::Turning | SolomonPhase::Speaking | SolomonPhase::RetreatHold => {
            SolomonVisualState {
                body_bank: SolomonBodyBank::Dialogue,
                body_pose: 0,
                clip_rect_world: Some(clip_rect(
                    encounter.position,
                    1000.0,
                    1000.0,
                    2000.0,
                    1000.0,
                )),
                direction,
                mouth_pose: Some(encounter.mouth_pose),
                native_body_record: 213 + direction,
                native_mouth_record: Some(228 + encounter.mouth_pose * 15 + direction),
                offset_y: 5.0 + encounter.transition_offset_y + encounter.motion,
                grave_mark_visible: true,
                visible: true,
            }
        }
        SolomonPhase::RetreatAccelerating => {
            let clip = if encounter.acceleration < 0.0 {
                Some(clip_rect(dig.position, 1000.0, 1000.0, 2000.0, 1000.0))
            } else {
                None
            };
            walk_visual(direction, 0, encounter.motion, clip)
        }
        SolomonPhase::Escaping => walk_visual(
            direction,
            encounter.walk_cycle.trunc() as usize % 6,
            encounter.motion,
            None,
        ),
        SolomonPhase::Gone => {
            SolomonVisualState { visible: false, ..walk_visual(direction, 0, 0.0, None) }
        }
    }
}

fn walk_visual(
    direction: usize,
    pose: usize,
    offset_y: f64,
    clip_rect_world: Option<SolomonClipRect>,
) -> SolomonVisualState {
    SolomonVisualState {
        body_bank: SolomonBodyBank::Walk,
        body_pose: pose,
        clip_rect_world,
        direction,
        mouth_pose: None,
        native_body_record: 95 + pose * 15 + direction,
        native_mouth_record: None,
        offset_y,
        grave_mark_visible: false,
        visible: true,
    }
}

fn clip_rect(position: Vec2, x: f64, y: f64, width: f64, height: f64) -> SolomonClipRect {
    SolomonClipRect { x: position.x - x, y: position.y - y, width, height }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSolomonDirtState {
    pub age_ticks: u64,
    pub alpha: f64,
    pub heading_degrees: f64,
    pub position: Vec2,
    pub speed: f64,
}

#[must_use]
pub fn native_solomon_dirt_origin(position: Vec2) -> Vec2 {
    Vec2::new(fround(position.x - 22.0), fround(position.y - 62.0))
}

#[must_use]
pub fn native_solomon_dirt_state_at(
    solomon_position: Vec2,
    age_ticks: u64,
) -> Option<NativeSolomonDirtState> {
    if age_ticks >= SOLOMON_DIRT_VISIBLE_TICKS {
        return None;
    }
    let mut state = NativeSolomonDirtState {
        age_ticks: 0,
        alpha: 1.0,
        heading_degrees: 35.0,
        position: native_solomon_dirt_origin(solomon_position),
        speed: 2.0,
    };
    for _ in 0..age_ticks {
        let radians = state.heading_degrees * std::f64::consts::PI / 180.0;
        let unit_x = fround(float::sin(radians));
        let unit_y = fround(-float::cos(radians));
        let delta_x = fround(state.speed * unit_x);
        let delta_y = fround(state.speed * unit_y);
        state.age_ticks += 1;
        state.alpha = fround(state.alpha - f64::from(0.035_f32)).max(0.0);
        state.heading_degrees = fround(state.heading_degrees + 2.0);
        state.position =
            Vec2::new(fround(state.position.x + delta_x), fround(state.position.y + delta_y));
        state.speed = fround(state.speed * f64::from(0.975_f32));
    }
    Some(state)
}

/// The first accepted snapshot establishes a cursor without replaying past dirt throws.
#[must_use]
pub fn native_solomon_dirt_event_delta(
    last_seen_event_id: Option<u64>,
    current: &[SolomonDigEvent],
) -> (u64, Vec<SolomonDigEvent>) {
    let latest_id = current.last().map_or(0, |event| event.id);
    let Some(last_id) = last_seen_event_id else {
        return (latest_id, Vec::new());
    };
    let mut events = Vec::new();
    for event in current {
        if event.id > last_id
            && matches!(event.cue, SolomonDigCue::ThrowDirt1 | SolomonDigCue::ThrowDirt2)
        {
            events.push(*event);
        }
    }
    (last_id.max(latest_id), events)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolomonDirtParticle {
    pub event_id: u64,
    birth_tick: u64,
    birth_position: Vec2,
    pub state: NativeSolomonDirtState,
}

/// Cosmetic state belongs to the retained view. Particle roots remain at their birth world
/// position when Solomon moves; changing the actor root must not drag the thrown dirt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolomonDirtPaintState {
    last_event_id: Option<u64>,
    particles: Vec<SolomonDirtParticle>,
}

impl SolomonDirtPaintState {
    pub fn reset(&mut self) {
        self.last_event_id = None;
        self.particles.clear();
    }

    #[must_use]
    pub fn particles(&self) -> &[SolomonDirtParticle] {
        &self.particles
    }

    pub fn prepare(&mut self, encounter: Option<&SolomonEncounter>, tick: u64) {
        let Some(encounter) = encounter else {
            self.reset();
            return;
        };
        let (event_id, events) =
            native_solomon_dirt_event_delta(self.last_event_id, &encounter.dig_events);
        self.last_event_id = Some(event_id);
        for event in events {
            if tick >= event.tick && tick - event.tick < SOLOMON_DIRT_VISIBLE_TICKS {
                self.particles.push(SolomonDirtParticle {
                    event_id: event.id,
                    birth_tick: event.tick,
                    birth_position: encounter.position,
                    state: native_solomon_dirt_state_at(encounter.position, 0).unwrap(),
                });
            }
        }
        self.particles.retain_mut(|particle| {
            if tick < particle.birth_tick {
                return true;
            }
            let age = tick - particle.birth_tick;
            let Some(state) = native_solomon_dirt_state_at(particle.birth_position, age) else {
                return false;
            };
            particle.state = state;
            true
        });
    }
}
