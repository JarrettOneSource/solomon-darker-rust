//! The opening Solomon encounter from `core-kernels/boneyard-encounter.ts`.
//!
//! Animation cursors and the private native RNG advance in the source order. Voice durations
//! are authoritative PCM durations rounded up to 100 Hz ticks; audio playback does not own
//! the encounter clock. Collision/pathfinding may resolve the requested escape position.

use crate::float;
use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::rng::seed_from_text;
use crate::{NativeRng, Vec2};

use super::{BoneyardBounds, SolomonDig};

pub const SOLOMON_COLLISION_RADIUS: f64 = 30.0;
pub const SOLOMON_NAVIGATION_CLEARANCE: f64 = 25.0;
pub const SOLOMON_ESCAPE_ROUTE_ARRIVAL_DISTANCE_SQUARED: f64 = 100.0;
pub const SOLOMON_ESCAPE_PATH_MARGIN: f64 = 50.0;
pub const SOLOMON_ESCAPE_TARGET_MARGIN: f64 = 100.0;
pub const SOLOMON_ESCAPE_TARGET_DISTANCE: f64 = 4096.0;

const RETREAT_HOLD_TICKS: u64 = 25;
const RETREAT_ACCELERATION: f64 = -7.0;
const RETREAT_ACCELERATION_STEP: f64 = 0.5;
const RETREAT_DISTANCE_PER_TICK: f64 = 3.0;
const ESCAPE_INITIAL_SPEED: f64 = 2.0;
const ESCAPE_SPEED_STEP: f64 = 0.05;
const ESCAPE_HEADING_DEFLECTION_DEGREES: f64 = 15.0;
const ESCAPE_INITIAL_ACCELERATION: f64 = -3.0;
const ESCAPE_ACCELERATION_STEP: f64 = 0.25;
const ESCAPE_LANDING_ACCELERATION: f64 = -2.0;
const ESCAPE_LIFETIME_TICKS: u64 = 515;
const MAX_TURN_RATE: f64 = 10.0;
const DIG_SHOVEL_CURSOR: f64 = 4.0;
const DIG_THROW_DIRT_CURSOR: f64 = 15.0;
const DIG_CURSOR_JITTER_END: f64 = 10.0;
const DIG_CURSOR_JITTER_MAXIMUM: f64 = 0.09_f32 as f64;
const DIG_TAIL_PROGRAM_SLOTS: usize = 5;
const DIG_TAIL_SLOWDOWN: f64 = 0.05_f32 as f64;
const DIG_BODY_BOB_AMPLITUDE_MINIMUM: f64 = 5.0;
const DIG_BODY_BOB_AMPLITUDE_RANGE: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SolomonPhase {
    Digging,
    Turning,
    Speaking,
    RetreatHold,
    RetreatAccelerating,
    Escaping,
    Gone,
}

impl SolomonPhase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Digging => "digging",
            Self::Turning => "turning",
            Self::Speaking => "speaking",
            Self::RetreatHold => "retreat-hold",
            Self::RetreatAccelerating => "retreat-accelerating",
            Self::Escaping => "escaping",
            Self::Gone => "gone",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SolomonDialogueMode {
    Ordinary,
    Tutorial,
}

impl SolomonDialogueMode {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ordinary => "ordinary",
            Self::Tutorial => "tutorial",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SolomonVoiceCue {
    Hello1,
    Hello2,
    Hello3,
    Hello4,
    Laugh1,
    GetHimBoys,
}

impl SolomonVoiceCue {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Hello1 => "solomon-hello-1",
            Self::Hello2 => "solomon-hello-2",
            Self::Hello3 => "solomon-hello-3",
            Self::Hello4 => "solomon-hello-4",
            Self::Laugh1 => "solomon-laugh-1",
            Self::GetHimBoys => "solomon-get-him-boys",
        }
    }

    #[must_use]
    pub const fn duration_ticks(self) -> u64 {
        match self {
            Self::Hello1 => 783,
            Self::Hello2 => 570,
            Self::Hello3 => 554,
            Self::Hello4 => 735,
            Self::Laugh1 => 247,
            Self::GetHimBoys => 245,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SolomonDigCue {
    Shovel1,
    Shovel2,
    ThrowDirt1,
    ThrowDirt2,
}

impl SolomonDigCue {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Shovel1 => "shovel-1",
            Self::Shovel2 => "shovel-2",
            Self::ThrowDirt1 => "throw-dirt-1",
            Self::ThrowDirt2 => "throw-dirt-2",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SolomonVoiceEvent {
    pub cue: SolomonVoiceCue,
    pub id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SolomonDigEvent {
    pub cue: SolomonDigCue,
    pub id: u64,
    pub tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolomonPlayer<'a> {
    pub id: &'a str,
    pub position: Vec2,
}

/// Native encounter state. The run event begins ordinary waves; the target player is locked
/// only while Solomon turns and speaks, and his escape continues to carry the voice queue.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SolomonEncounter {
    pub acceleration: f64,
    pub dig_body_bob_amplitude: f64,
    pub dig_body_offset_y: f64,
    pub dig_event_id: u64,
    pub dig_events: Vec<SolomonDigEvent>,
    pub dig_frame: usize,
    pub dig_frame_program: Vec<usize>,
    pub dig_phase: f64,
    pub dig_shovel_armed: bool,
    pub dig_ticks_per_frame: u64,
    pub dig_throw_dirt_armed: bool,
    pub dialogue_mode: SolomonDialogueMode,
    pub escape_collision_source_ids: Vec<String>,
    pub escape_speed: f64,
    pub escape_target: Option<Vec2>,
    pub heading_degrees: f64,
    pub lifetime_ticks_remaining: u64,
    pub mouth_pose: usize,
    pub mouth_pose_ticks_remaining: u64,
    pub motion: f64,
    pub phase: SolomonPhase,
    pub phase_ticks_remaining: u64,
    pub position: Vec2,
    pub queued_get_him_boys: bool,
    pub rng: NativeRng,
    pub run_event_id: u64,
    pub target_player_id: Option<String>,
    pub tutorial_dialogue_ticks: u64,
    pub transition_offset_y: f64,
    pub turn_rate: f64,
    pub voice_events: Vec<SolomonVoiceEvent>,
    pub voice_ticks_remaining: u64,
    pub walk_cycle: f64,
}

impl SolomonEncounter {
    #[must_use]
    pub fn new(dig: &SolomonDig, seed: &str) -> Self {
        Self::with_dialogue(dig, seed, SolomonDialogueMode::Ordinary, 0)
    }

    #[must_use]
    pub fn new_tutorial(dig: &SolomonDig, seed: &str, dialogue_ticks: u64) -> Self {
        assert!(dialogue_ticks > 0, "Tutorial Solomon dialogue duration must be positive");
        Self::with_dialogue(dig, seed, SolomonDialogueMode::Tutorial, dialogue_ticks)
    }

    fn with_dialogue(
        dig: &SolomonDig,
        seed: &str,
        dialogue_mode: SolomonDialogueMode,
        tutorial_dialogue_ticks: u64,
    ) -> Self {
        assert!(
            !dig.frame_program.is_empty() && dig.ticks_per_frame > 0,
            "Solomon Dig needs an animation program and positive frame timing"
        );
        let mut rng = NativeRng::new(i64::from(seed_from_text(&format!("{seed}:solomon-dig"))));
        let amplitude = rng.draw_scaled_float(DIG_BODY_BOB_AMPLITUDE_RANGE, false);
        Self {
            acceleration: 0.0,
            dig_body_bob_amplitude: fround(amplitude + DIG_BODY_BOB_AMPLITUDE_MINIMUM),
            dig_body_offset_y: 0.0,
            dig_event_id: 0,
            dig_events: Vec::new(),
            dig_frame: dig.frame_program[0],
            dig_frame_program: dig.frame_program.clone(),
            dig_phase: 0.0,
            dig_shovel_armed: true,
            dig_ticks_per_frame: dig.ticks_per_frame,
            dig_throw_dirt_armed: true,
            dialogue_mode,
            escape_collision_source_ids: Vec::new(),
            escape_speed: 0.0,
            escape_target: None,
            heading_degrees: 180.0,
            lifetime_ticks_remaining: 0,
            mouth_pose: 0,
            mouth_pose_ticks_remaining: 25,
            motion: 0.0,
            phase: SolomonPhase::Digging,
            phase_ticks_remaining: 0,
            position: dig.position,
            queued_get_him_boys: false,
            rng,
            run_event_id: 0,
            target_player_id: None,
            tutorial_dialogue_ticks,
            transition_offset_y: 0.0,
            turn_rate: 0.0,
            voice_events: Vec::new(),
            voice_ticks_remaining: 0,
            walk_cycle: 0.0,
        }
    }

    pub fn step(&mut self, players: &[SolomonPlayer<'_>], tick: u64) {
        match self.phase {
            SolomonPhase::Digging => self.step_digging(players, tick),
            SolomonPhase::Turning => self.face_target(players),
            SolomonPhase::Speaking => self.step_hello(players),
            SolomonPhase::RetreatHold => self.step_retreat_hold(),
            SolomonPhase::RetreatAccelerating => {
                self.step_voice_queue();
                self.apply_retreat_acceleration();
            }
            SolomonPhase::Escaping => self.step_escape(),
            SolomonPhase::Gone => self.step_voice_queue(),
        }
    }

    #[must_use]
    pub fn player_locked(&self, player_id: &str) -> bool {
        self.target_player_id.as_deref() == Some(player_id)
            && matches!(self.phase, SolomonPhase::Turning | SolomonPhase::Speaking)
    }

    #[must_use]
    pub fn combat_enabled(&self) -> bool {
        self.run_event_id > 0
    }

    fn step_digging(&mut self, players: &[SolomonPlayer<'_>], tick: u64) {
        let mut cursor = fround(self.dig_phase + fround(1.0 / self.dig_ticks_per_frame as f64));
        if cursor > DIG_SHOVEL_CURSOR && self.dig_shovel_armed {
            let cue = if self.rng.draw_integer(2) == 0 {
                SolomonDigCue::Shovel1
            } else {
                SolomonDigCue::Shovel2
            };
            self.dig_shovel_armed = false;
            self.append_dig_event(cue, tick);
        }
        if cursor > DIG_THROW_DIRT_CURSOR && self.dig_throw_dirt_armed {
            let cue = if self.rng.draw_integer(2) == 0 {
                SolomonDigCue::ThrowDirt1
            } else {
                SolomonDigCue::ThrowDirt2
            };
            self.dig_throw_dirt_armed = false;
            self.append_dig_event(cue, tick);
        }
        if cursor > DIG_THROW_DIRT_CURSOR
            || (cursor > DIG_SHOVEL_CURSOR && cursor < DIG_CURSOR_JITTER_END)
        {
            cursor = fround(cursor - self.rng.draw_scaled_float(DIG_CURSOR_JITTER_MAXIMUM, false));
        }
        if cursor > self.dig_frame_program.len() as f64 - DIG_TAIL_PROGRAM_SLOTS as f64 {
            cursor = fround(cursor - DIG_TAIL_SLOWDOWN);
        }
        self.dig_body_offset_y =
            native_solomon_dig_body_offset_y(cursor, self.dig_body_bob_amplitude);
        self.dig_frame = self.dig_frame_program.get(cursor.floor() as usize).copied().unwrap_or(0);
        self.dig_phase = cursor;
        self.acquire_target(players);
        if self.phase != SolomonPhase::Digging {
            return;
        }
        if cursor >= self.dig_frame_program.len() as f64 {
            cursor = fround(cursor - self.dig_frame_program.len() as f64);
            if self.rng.draw_integer(2) == 1 {
                cursor = DIG_SHOVEL_CURSOR;
            }
            self.dig_body_bob_amplitude = fround(
                self.rng.draw_scaled_float(DIG_BODY_BOB_AMPLITUDE_RANGE, false)
                    + DIG_BODY_BOB_AMPLITUDE_MINIMUM,
            );
            self.dig_shovel_armed = true;
            self.dig_throw_dirt_armed = true;
        }
        self.dig_frame = self.dig_frame_program.get(cursor.floor() as usize).copied().unwrap_or(0);
        self.dig_phase = cursor;
    }

    fn acquire_target(&mut self, players: &[SolomonPlayer<'_>]) {
        if self.dig_phase <= self.dig_frame_program.len() as f64 - 10.0 {
            return;
        }
        let mut target = None;
        let mut nearest_distance_squared = f64::INFINITY;
        for player in players {
            if !solomon_contact_contains(self.position, player.position) {
                continue;
            }
            let dx = player.position.x - self.position.x;
            let dy = player.position.y - self.position.y;
            let distance_squared = dx * dx + dy * dy;
            if distance_squared < nearest_distance_squared {
                target = Some(player.id);
                nearest_distance_squared = distance_squared;
            }
        }
        let Some(target) = target else { return };
        let (heading, offset) = if self.dig_frame < 6 {
            (180.0, 15.0)
        } else if self.dig_frame < 16 {
            (225.0, 6.0)
        } else {
            (270.0, 0.0)
        };
        self.dig_phase = 0.0;
        self.heading_degrees = heading;
        self.phase = SolomonPhase::Turning;
        self.target_player_id = Some(target.into());
        self.transition_offset_y = offset;
    }

    fn face_target(&mut self, players: &[SolomonPlayer<'_>]) {
        let current_id = self.target_player_id.as_deref();
        let target = players
            .iter()
            .find(|player| Some(player.id) == current_id)
            .or_else(|| nearest_player(self.position, players));
        let Some(target) = target else {
            self.target_player_id = None;
            self.transition_offset_y *= 0.9;
            return;
        };
        let desired_heading = heading_degrees_from_vector(
            target.position.x - self.position.x,
            target.position.y - self.position.y,
        );
        for _ in 0..=(self.turn_rate.trunc() as usize) {
            self.heading_degrees += native_turn_direction(self.heading_degrees, desired_heading);
        }
        self.heading_degrees = normalize_native_turn_heading(self.heading_degrees);
        self.target_player_id = Some(target.id.into());
        self.transition_offset_y *= 0.9;
        self.turn_rate = (self.turn_rate + 0.5).min(MAX_TURN_RATE);
        if self.phase != SolomonPhase::Turning
            || (self.heading_degrees - desired_heading).abs() > 1.0
        {
            return;
        }
        self.phase = SolomonPhase::Speaking;
        if self.dialogue_mode == SolomonDialogueMode::Tutorial {
            self.voice_ticks_remaining = self.tutorial_dialogue_ticks;
            return;
        }
        let cue = match self.rng.draw_integer(4) {
            0 => SolomonVoiceCue::Hello1,
            1 => SolomonVoiceCue::Hello2,
            2 => SolomonVoiceCue::Hello3,
            _ => SolomonVoiceCue::Hello4,
        };
        self.append_voice_event(cue);
    }

    fn step_hello(&mut self, players: &[SolomonPlayer<'_>]) {
        self.face_target(players);
        if self.voice_ticks_remaining > 1 {
            self.voice_ticks_remaining -= 1;
            self.step_mouth();
            return;
        }
        self.motion += 10.0;
        self.phase = SolomonPhase::RetreatHold;
        self.phase_ticks_remaining = RETREAT_HOLD_TICKS;
        self.voice_ticks_remaining = 0;
    }

    fn step_mouth(&mut self) {
        if self.mouth_pose_ticks_remaining > 1 {
            self.mouth_pose_ticks_remaining -= 1;
            return;
        }
        let prior = self.mouth_pose;
        while self.mouth_pose == prior {
            self.mouth_pose = self.rng.draw_integer(3) as usize;
        }
        self.mouth_pose_ticks_remaining = 40 + 2 * self.rng.draw_integer(25) as u64;
    }

    fn step_retreat_hold(&mut self) {
        self.step_voice_queue();
        if self.phase_ticks_remaining > 1 {
            self.phase_ticks_remaining -= 1;
            return;
        }
        self.heading_degrees = native_retreat_heading(self.heading_degrees);
        self.acceleration = RETREAT_ACCELERATION;
        self.motion = 0.0;
        self.phase = SolomonPhase::RetreatAccelerating;
        self.phase_ticks_remaining = 0;
        self.queued_get_him_boys = self.dialogue_mode == SolomonDialogueMode::Ordinary;
        if self.queued_get_him_boys {
            self.append_voice_event(SolomonVoiceCue::Laugh1);
        }
        self.apply_retreat_acceleration();
    }

    fn apply_retreat_acceleration(&mut self) {
        self.motion += self.acceleration;
        self.acceleration += RETREAT_ACCELERATION_STEP;
        self.position =
            move_along_heading(self.position, self.heading_degrees, RETREAT_DISTANCE_PER_TICK);
        if self.motion <= 0.0 {
            return;
        }
        let direction = if self.rng.draw_integer(2) == 0 { -1.0 } else { 1.0 };
        self.acceleration = ESCAPE_INITIAL_ACCELERATION;
        self.escape_collision_source_ids.clear();
        self.escape_speed = ESCAPE_INITIAL_SPEED;
        self.escape_target = None;
        self.heading_degrees =
            normalize_degrees(self.heading_degrees + direction * ESCAPE_HEADING_DEFLECTION_DEGREES);
        self.lifetime_ticks_remaining = ESCAPE_LIFETIME_TICKS;
        self.motion = 0.0;
        self.phase = SolomonPhase::Escaping;
        self.run_event_id += 1;
        self.walk_cycle = 0.0;
    }

    fn step_escape(&mut self) {
        self.step_voice_queue();
        if self.lifetime_ticks_remaining == 0 {
            self.escape_collision_source_ids.clear();
            self.escape_target = None;
            self.phase = SolomonPhase::Gone;
            return;
        }
        let prior_speed = self.escape_speed;
        self.escape_speed += ESCAPE_SPEED_STEP;
        self.lifetime_ticks_remaining -= 1;
        self.motion += self.acceleration;
        self.acceleration += ESCAPE_ACCELERATION_STEP;
        if self.motion > 0.0 {
            self.motion = 0.0;
            self.acceleration = ESCAPE_LANDING_ACCELERATION;
        }
        if self.lifetime_ticks_remaining == 0 {
            self.escape_collision_source_ids.clear();
            self.escape_target = None;
            self.phase = SolomonPhase::Gone;
        }
        self.position = move_along_heading(self.position, self.heading_degrees, prior_speed);
        self.walk_cycle += self.escape_speed / 30.0;
        if self.walk_cycle > 6.0 {
            self.walk_cycle -= 6.0;
        }
    }

    fn step_voice_queue(&mut self) {
        if self.voice_ticks_remaining > 1 {
            self.voice_ticks_remaining -= 1;
            return;
        }
        if self.voice_ticks_remaining == 1 && self.queued_get_him_boys {
            self.queued_get_him_boys = false;
            self.append_voice_event(SolomonVoiceCue::GetHimBoys);
            return;
        }
        self.voice_ticks_remaining = 0;
    }

    fn append_voice_event(&mut self, cue: SolomonVoiceCue) {
        self.voice_events.push(SolomonVoiceEvent { cue, id: self.voice_events.len() as u64 + 1 });
        self.voice_ticks_remaining = cue.duration_ticks();
    }

    fn append_dig_event(&mut self, cue: SolomonDigCue, tick: u64) {
        self.dig_event_id += 1;
        self.dig_events.push(SolomonDigEvent { cue, id: self.dig_event_id, tick });
        if self.dig_events.len() > 8 {
            self.dig_events.remove(0);
        }
    }
}

#[must_use]
pub fn solomon_contact_contains(solomon: Vec2, player: Vec2) -> bool {
    let dx = (solomon.x - player.x) / 1.5;
    let dy = (solomon.y - 10.0 - player.y) / 1.25;
    dx * dx + dy * dy < 10_000.0
}

#[must_use]
pub fn native_solomon_dig_body_offset_y(cursor: f64, amplitude: f64) -> f64 {
    if cursor <= DIG_SHOVEL_CURSOR - 1.0 || cursor > DIG_THROW_DIRT_CURSOR {
        return 0.0;
    }
    let degrees = fround(
        fround(cursor - (DIG_SHOVEL_CURSOR - 1.0))
            / (DIG_THROW_DIRT_CURSOR - DIG_SHOVEL_CURSOR + 1.0)
            * 180.0,
    );
    let radians = fround(degrees * fround(std::f64::consts::PI) / 180.0);
    fround(fround(float::sin(radians)) * amplitude)
}

#[must_use]
pub fn native_solomon_escape_target(
    position: Vec2,
    heading_degrees: f64,
    bounds: BoneyardBounds,
) -> Vec2 {
    let radians = heading_degrees * std::f64::consts::PI / 180.0;
    let end = Vec2::new(
        fround(position.x + float::sin(radians) * SOLOMON_ESCAPE_TARGET_DISTANCE),
        fround(position.y - float::cos(radians) * SOLOMON_ESCAPE_TARGET_DISTANCE),
    );
    clip_escape_segment(position, end, bounds, SOLOMON_ESCAPE_TARGET_MARGIN)
}

#[must_use]
pub fn native_solomon_escape_path_target(
    position: Vec2,
    escape_target: Vec2,
    bounds: BoneyardBounds,
) -> Vec2 {
    clip_escape_segment(position, escape_target, bounds, SOLOMON_ESCAPE_PATH_MARGIN)
}

fn nearest_player<'a, 'b>(
    position: Vec2,
    players: &'a [SolomonPlayer<'b>],
) -> Option<&'a SolomonPlayer<'b>> {
    let mut nearest: Option<&SolomonPlayer<'_>> = None;
    let mut nearest_distance_squared = f64::INFINITY;
    for player in players {
        let dx = player.position.x - position.x;
        let dy = player.position.y - position.y;
        let distance_squared = dx * dx + dy * dy;
        if distance_squared < nearest_distance_squared
            || (distance_squared == nearest_distance_squared
                && nearest.is_some_and(|prior| player.id < prior.id))
        {
            nearest = Some(player);
            nearest_distance_squared = distance_squared;
        }
    }
    nearest
}

fn clip_escape_segment(start: Vec2, end: Vec2, bounds: BoneyardBounds, margin: f64) -> Vec2 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let mut progress: f64 = 1.0;
    if dx > 0.0 {
        progress = progress.min((bounds.x + bounds.width + margin - start.x) / dx);
    } else if dx < 0.0 {
        progress = progress.min((bounds.x - margin - start.x) / dx);
    }
    if dy > 0.0 {
        progress = progress.min((bounds.y + bounds.height + margin - start.y) / dy);
    } else if dy < 0.0 {
        progress = progress.min((bounds.y - margin - start.y) / dy);
    }
    Vec2::new(fround(start.x + dx * progress), fround(start.y + dy * progress))
}

fn native_retreat_heading(heading: f64) -> f64 {
    let mut reversed = heading + 180.0;
    if reversed > 360.0 {
        reversed -= 360.0;
    }
    if reversed < 0.0 {
        reversed += 360.0;
    }
    reversed.clamp(45.0, 315.0)
}

fn move_along_heading(position: Vec2, heading: f64, distance: f64) -> Vec2 {
    let radians = heading * std::f64::consts::PI / 180.0;
    Vec2::new(
        position.x + float::sin(radians) * distance,
        position.y - float::cos(radians) * distance,
    )
}

fn normalize_degrees(value: f64) -> f64 {
    ((value % 360.0) + 360.0) % 360.0
}

fn normalize_native_turn_heading(value: f64) -> f64 {
    if value < 0.0 {
        return value + 360.0;
    }
    if value > 360.0 {
        return value - 360.0;
    }
    value
}

fn native_turn_direction(current: f64, target: f64) -> f64 {
    let current = normalize_degrees(current);
    let target = normalize_degrees(target);
    let difference = (current - target).abs();
    if difference < 1.0 || difference >= 359.0 {
        return 0.0;
    }
    if target <= current {
        return if current - target <= 180.0 { -1.0 } else { 1.0 };
    }
    if target - current > 180.0 { -1.0 } else { 1.0 }
}
