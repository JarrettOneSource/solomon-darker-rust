//! Boss stream timing and Faculty banter from Website `native-boss-audio.ts` and
//! `native-faculty-voices.ts`. These pure clocks advance steering RNG even when no audio
//! device is attached, because the same stream also drives enemy decisions.

use crate::NativeRng;
use crate::float::fround;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BossStreamCue {
    Trap,
    UnholyDie,
    UnholyScream,
    FacultyDie,
    FacultyNo,
    FacultyNoFemale,
    HeartBreak,
    FacultyJoinUs1,
    FacultyJoinUs1Female,
    FacultyJoinUs2,
    FacultyJoinUs2Female,
    FacultyDeadIsBetter1,
    FacultyDeadIsBetter1Female,
    FacultyDeadIsBetter2,
    FacultyDeadIsBetter2Female,
}

impl BossStreamCue {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Trap => "trap",
            Self::UnholyDie => "unholy-die",
            Self::UnholyScream => "unholy-scream",
            Self::FacultyDie => "faculty-die",
            Self::FacultyNo => "faculty-no",
            Self::FacultyNoFemale => "faculty-no-female",
            Self::HeartBreak => "heart-break",
            Self::FacultyJoinUs1 => "faculty-join-us-1",
            Self::FacultyJoinUs1Female => "faculty-join-us-1-female",
            Self::FacultyJoinUs2 => "faculty-join-us-2",
            Self::FacultyJoinUs2Female => "faculty-join-us-2-female",
            Self::FacultyDeadIsBetter1 => "faculty-dead-is-better-1",
            Self::FacultyDeadIsBetter1Female => "faculty-dead-is-better-1-female",
            Self::FacultyDeadIsBetter2 => "faculty-dead-is-better-2",
            Self::FacultyDeadIsBetter2Female => "faculty-dead-is-better-2-female",
        }
    }

    #[must_use]
    pub const fn ticks(self) -> u64 {
        match self {
            Self::Trap => 136,
            Self::UnholyDie => 600,
            Self::UnholyScream => 275,
            Self::FacultyDie => 533,
            Self::FacultyNo => 175,
            Self::FacultyNoFemale => 345,
            Self::HeartBreak => 894,
            Self::FacultyJoinUs1 => 362,
            Self::FacultyJoinUs1Female => 416,
            Self::FacultyJoinUs2 => 374,
            Self::FacultyJoinUs2Female => 347,
            Self::FacultyDeadIsBetter1 => 468,
            Self::FacultyDeadIsBetter1Female => 393,
            Self::FacultyDeadIsBetter2 => 390,
            Self::FacultyDeadIsBetter2Female => 505,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BossNarrationLine {
    pub cue: BossStreamCue,
    pub event_id: usize,
    pub started_tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BossNarration {
    pub current: Option<BossNarrationLine>,
    pub idle_ticks: u64,
    pub mix: f64,
    pub next_event_id: usize,
    pub pending: Vec<BossStreamCue>,
    pub ticks_remaining: u64,
}

impl Default for BossNarration {
    fn default() -> Self {
        Self {
            current: None,
            idle_ticks: 25,
            mix: 1.0,
            next_event_id: 1,
            pending: Vec::new(),
            ticks_remaining: 0,
        }
    }
}

impl BossNarration {
    pub fn step(&mut self, tick: u64, external_busy: bool) {
        if self.current.is_some() {
            self.ticks_remaining = self.ticks_remaining.saturating_sub(1);
            if self.ticks_remaining == 0 {
                self.current = None;
            }
        } else if self.idle_ticks < 25 {
            self.idle_ticks += 1;
        }
        self.enqueue(&[], tick, external_busy);
        let target = if self.current.is_some() || self.idle_ticks < 25 { 0.5 } else { 1.0 };
        self.mix = if self.mix > target {
            fround(self.mix - (0.025_f32 as f64)).max(target)
        } else {
            fround(self.mix + (0.025_f32 as f64)).min(target)
        };
    }

    pub fn enqueue(&mut self, cues: &[BossStreamCue], tick: u64, external_busy: bool) {
        self.pending.extend_from_slice(cues);
        if self.current.is_some() || external_busy || self.pending.is_empty() {
            return;
        }
        let cue = self.pending.remove(0);
        self.current =
            Some(BossNarrationLine { cue, event_id: self.next_event_id, started_tick: tick });
        self.idle_ticks = 0;
        self.next_event_id += 1;
        self.ticks_remaining = cue.ticks();
    }

    pub fn cancel(&mut self) {
        self.current = None;
        self.pending.clear();
        self.ticks_remaining = 0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct FacultyVoiceController {
    pub line: usize,
    pub ticks_remaining: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FacultyVoiceStep {
    pub cues: Vec<BossStreamCue>,
    pub state: Option<FacultyVoiceController>,
}

impl FacultyVoiceController {
    #[must_use]
    pub fn new(rng: &mut NativeRng) -> Self {
        Self { line: 0, ticks_remaining: 200 + rng.draw_integer(500) as u64 }
    }

    /// `stepNativeFacultyVoices`: members are the living Faculty in world order, with
    /// `true` selecting the female bank. Membership ends on the first dying tick.
    #[must_use]
    pub fn step(mut self, rng: &mut NativeRng, members: &[bool], busy: bool) -> FacultyVoiceStep {
        if members.is_empty() {
            return FacultyVoiceStep { cues: Vec::new(), state: None };
        }
        if busy {
            return FacultyVoiceStep { cues: Vec::new(), state: Some(self) };
        }
        if self.ticks_remaining > 0 {
            self.ticks_remaining -= 1;
            return FacultyVoiceStep { cues: Vec::new(), state: Some(self) };
        }
        let male = members.iter().any(|female| !*female);
        let female = members.iter().any(|female| *female);
        self.line += 1;
        let mut cooldown = 500 + rng.draw_integer(700) as u64;
        let mut cues = Vec::new();
        match self.line {
            1 => enqueue_voice(
                &mut cues,
                rng,
                male,
                female,
                BossStreamCue::FacultyJoinUs1,
                BossStreamCue::FacultyJoinUs1Female,
                true,
            ),
            2 => enqueue_voice(
                &mut cues,
                rng,
                male,
                female,
                BossStreamCue::FacultyDeadIsBetter1,
                BossStreamCue::FacultyDeadIsBetter1Female,
                false,
            ),
            3 => {
                enqueue_voice(
                    &mut cues,
                    rng,
                    male,
                    female,
                    BossStreamCue::FacultyJoinUs2,
                    BossStreamCue::FacultyJoinUs2Female,
                    false,
                );
                cooldown = 30;
            }
            4 => {
                enqueue_voice(
                    &mut cues,
                    rng,
                    male,
                    female,
                    BossStreamCue::FacultyDeadIsBetter2,
                    BossStreamCue::FacultyDeadIsBetter2Female,
                    true,
                );
                cooldown = 100 + rng.draw_integer(50) as u64;
            }
            5 => {
                if rng.draw_integer(4) == 3 && male {
                    cues.push(BossStreamCue::FacultyDeadIsBetter1);
                }
            }
            _ => {
                self.line = 0;
                cooldown = 1000 + rng.draw_integer(1000) as u64;
            }
        }
        self.ticks_remaining = cooldown * 2;
        FacultyVoiceStep { cues, state: Some(self) }
    }
}

fn enqueue_voice(
    cues: &mut Vec<BossStreamCue>,
    rng: &mut NativeRng,
    male: bool,
    female: bool,
    male_cue: BossStreamCue,
    female_cue: BossStreamCue,
    allow_reply: bool,
) {
    cues.push(if male { male_cue } else { female_cue });
    if male && female && allow_reply && rng.draw_integer(2) != 0 {
        cues.push(female_cue);
    }
}
