//! Source audio names, spatial gain and presentation cues. Authoritative gameplay emits
//! sound/pitch events; no audio adapter is allowed to advance a gameplay random stream.

mod catalog;
pub mod combat;
pub use catalog::{AUDIO_CATALOG, AudioCue, AudioDefinition, AudioKind};

use crate::hub::teacher::{HUB_TEACHER_CAST_TICKS, HUB_TEACHER_CYCLE_TICKS};
use crate::profile::WizardElement;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicScene {
    Title,
    Create,
    Hub,
    Boneyard,
    Combat,
    GameOver,
}
impl MusicScene {
    #[must_use]
    pub const fn cue(self) -> AudioCue {
        match self {
            Self::Title => AudioCue::Solomondarktheme,
            Self::Create => AudioCue::Selection,
            Self::Hub => AudioCue::Academy,
            Self::Boneyard => AudioCue::Prelude,
            Self::Combat => AudioCue::Combat,
            Self::GameOver => AudioCue::Death,
        }
    }
    #[must_use]
    pub const fn transition_ticks(self) -> u32 {
        match self {
            Self::Hub => 2,
            Self::GameOver => 0,
            _ => 100,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct AudioOptions {
    pub volume: f64,
    pub playback_rate: f64,
    pub offset_seconds: f64,
}
impl Default for AudioOptions {
    fn default() -> Self {
        Self { volume: 1.0, playback_rate: 1.0, offset_seconds: 0.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateAudioAction {
    PlaySound(AudioCue),
    PlayStream(AudioCue),
    PauseStream(AudioCue),
}

#[must_use]
pub fn create_entry_audio_events(previous: f64, current: f64) -> Vec<CreateAudioAction> {
    let mut events = Vec::new();
    if crossed(previous, current, 200.0) {
        events.push(CreateAudioAction::PlayStream(AudioCue::StartCast));
    }
    if crossed(previous, current, 1340.0) {
        events.push(CreateAudioAction::PauseStream(AudioCue::StartCast));
        events.push(CreateAudioAction::PlayStream(AudioCue::ChooseElement));
    }
    events
}
#[must_use]
pub fn create_selection_audio_events(
    element: WizardElement,
    previous: f64,
    current: f64,
) -> Vec<CreateAudioAction> {
    let mut events = Vec::new();
    if crossed(previous, current, 980.0) {
        events.push(CreateAudioAction::PlaySound(match element {
            WizardElement::Air => AudioCue::LightningStart,
            WizardElement::Earth => AudioCue::RockHit,
            WizardElement::Ether => AudioCue::MagicMissile,
            WizardElement::Fire => AudioCue::ThrowFire,
            WizardElement::Water => AudioCue::IceStart,
        }));
    }
    if crossed(previous, current, 990.0) {
        events.push(CreateAudioAction::PlayStream(AudioCue::StartCast));
    }
    if crossed(previous, current, 1640.0) {
        events.push(CreateAudioAction::PauseStream(AudioCue::StartCast));
        events.push(CreateAudioAction::PlayStream(AudioCue::ChooseElement));
    }
    events
}
fn crossed(previous: f64, current: f64, threshold: f64) -> bool {
    previous < threshold && current >= threshold
}

#[must_use]
pub fn hub_audio_attenuation(distance: f64) -> f64 {
    if distance < 150.0 {
        return 1.0;
    }
    if distance > 800.0 {
        return 0.25;
    }
    (1.0 - (distance - 150.0) / 650.0).max(0.25)
}
#[must_use]
pub fn native_footstep_cue(tick: u32, player_id: &str) -> AudioCue {
    if stable_hash(player_id, tick) & 1 == 0 { AudioCue::Step1 } else { AudioCue::Step2 }
}
#[must_use]
pub fn new_native_footstep_tick(previous: Option<u32>, current: u32) -> Option<u32> {
    previous.filter(|previous| *previous != current && current != 0).map(|_| current)
}

/// Teacher audio follows the existing ambient clock. Catching up a presentation frame
/// emits each actual release without sampling or advancing a gameplay random stream.
#[must_use]
pub fn hub_teacher_releases_between(previous_seconds: f64, current_seconds: f64) -> Vec<u32> {
    if current_seconds <= previous_seconds {
        return Vec::new();
    }
    fn count(seconds: f64) -> u32 {
        let cast = HUB_TEACHER_CAST_TICKS as f64 / 100.0;
        let cycle = HUB_TEACHER_CYCLE_TICKS as f64 / 100.0;
        if seconds < cast { 0 } else { ((seconds - cast) / cycle).floor() as u32 + 1 }
    }
    (count(previous_seconds.max(0.0))..count(current_seconds.max(0.0))).collect()
}
#[must_use]
pub fn hub_teacher_summon_pitch(release: u32) -> f64 {
    1.0 + f64::from(stable_hash("teacher-summon", release)) / 4_294_967_296.0 * 0.1
}
fn stable_hash(value: &str, salt: u32) -> u32 {
    let mut hash = 0x811c9dc5 ^ salt;
    for code in value.encode_utf16() {
        hash ^= u32::from(code);
        hash = hash.wrapping_mul(0x01000193);
    }
    hash ^= hash >> 16;
    hash.wrapping_mul(0x045d9f3b)
}

/// Presentation requests carry no mutable gameplay state. Delayed cues use native audio
/// scheduling (dowsing echoes), while streams retain a named replacement/stop owner.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum AudioEvent {
    Play(AudioCue, AudioOptions),
    PlayAfter { cue: AudioCue, options: AudioOptions, delay_ms: f64 },
    StopStream(AudioCue),
}
impl From<CreateAudioAction> for AudioEvent {
    fn from(action: CreateAudioAction) -> Self {
        match action {
            CreateAudioAction::PlaySound(cue) | CreateAudioAction::PlayStream(cue) => {
                Self::Play(cue, AudioOptions::default())
            }
            CreateAudioAction::PauseStream(cue) => Self::StopStream(cue),
        }
    }
}
