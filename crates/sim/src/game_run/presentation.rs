//! Terminal-run presentation from `game-over-presentation.ts` and `game-over-audio.ts`.
//! The run owns the clock; rendering and audio never advance it.

use crate::Vec2;
use crate::audio::AudioCue;
use crate::float::fround;

use super::{
    GAME_OVER_ENTRY_FADE_TICKS, GAME_OVER_INPUT_ACCEPT_TICK, GameOverExitKind, GameRunLifecycle,
    GameRunPhase,
};

pub const SOLOMON_RIFF_GUITAR_TICK: u64 = 550;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameOverPresentation {
    pub accepts_input: bool,
    pub entry_fade_alpha: f64,
    pub exit_fade_alpha: f64,
    pub prompt_alpha: f64,
    pub title_alpha: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolomonRiffPresentation {
    pub frame_record: Option<usize>,
    pub offset: Vec2,
}

#[must_use]
pub fn game_over_presentation(
    ticks: u64,
    exit_ticks: Option<u64>,
    exit_kind: Option<GameOverExitKind>,
) -> GameOverPresentation {
    GameOverPresentation {
        accepts_input: exit_ticks.is_none() && ticks >= GAME_OVER_INPUT_ACCEPT_TICK,
        entry_fade_alpha: ((GAME_OVER_ENTRY_FADE_TICKS as f64 - ticks as f64)
            / GAME_OVER_ENTRY_FADE_TICKS as f64)
            .clamp(0.0, 1.0),
        exit_fade_alpha: exit_ticks.map_or(0.0, |ticks| {
            let duration =
                exit_kind.expect("Game Over exit ticks require an exit kind").duration_ticks();
            (ticks as f64 / duration as f64).clamp(0.0, 1.0)
        }),
        prompt_alpha: ((ticks as f64 - 400.0) * 0.005).clamp(0.0, 1.0),
        title_alpha: ((ticks as f64 - 300.0) * 0.005).clamp(0.0, 1.0),
    }
}

/// The stock twelve-frame Riff actor enters on tick 201 and finishes at tick 951.
#[must_use]
pub fn solomon_riff_presentation(ticks: u64) -> SolomonRiffPresentation {
    let mut visible = false;
    let mut offset = Vec2::new(-375.0, -5.0);
    let mut vertical_velocity = -4.0;
    let mut frame = 0.0;
    let mut phase = 0;
    for counter in 1..=ticks.min(951) {
        if !visible {
            visible = counter >= 201;
            continue;
        }
        if offset.y < 0.0 {
            offset.x = fround(offset.x + 4.4);
            offset.y = fround(offset.y + vertical_velocity).min(0.0);
            vertical_velocity = fround(vertical_velocity + 0.125);
            continue;
        }
        match phase {
            0 => {
                if frame < 3.0 {
                    frame = fround(frame - 0.03);
                }
                frame = fround(frame + 0.13);
                if frame >= 5.0 {
                    frame = 3.0;
                }
                if counter > 820 {
                    phase = 1;
                }
            }
            1 => {
                frame = ((counter / 8) & 1) as f64;
                if counter > 920 {
                    phase = 2;
                    frame = 6.0;
                }
            }
            _ => frame = fround(frame + 0.2).min(11.0),
        }
    }
    SolomonRiffPresentation { frame_record: visible.then_some(frame.trunc() as usize + 1), offset }
}

#[must_use]
pub fn game_over_audio_events(
    previous: &GameRunLifecycle,
    current: &GameRunLifecycle,
) -> Vec<AudioCue> {
    if current.phase != GameRunPhase::GameOver || current.run_id.is_none() {
        return Vec::new();
    }
    let entered = previous.phase != GameRunPhase::GameOver
        || previous.run_id != current.run_id
        || previous.game_over_event_id != current.game_over_event_id;
    if entered {
        vec![AudioCue::SolomonLaughBig]
    } else if previous.game_over_ticks < SOLOMON_RIFF_GUITAR_TICK
        && current.game_over_ticks >= SOLOMON_RIFF_GUITAR_TICK
    {
        vec![AudioCue::DeathGuitar]
    } else {
        Vec::new()
    }
}
