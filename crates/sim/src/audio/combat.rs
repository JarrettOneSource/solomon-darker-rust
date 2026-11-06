//! Combat sound projection: spell sounds and loops, footsteps, enemy, loot and Solomon dig
//! events, encounter narration and the shared enemy ambience loops.
//!
//! The session supplies borrowed accepted state. This cursor retains only sound counters,
//! actor identities and loop mixes; it never advances gameplay or random streams.

mod primary;
mod skills;
mod world;

pub use primary::ether_impact_pitch;
pub use skills::{air_water_accepted_cast_audio, weld_cast_sound_cues, weld_loop_cues};
pub use world::{
    BONEYARD_ENEMY_AMBIENT_CUES, BONEYARD_FACULTY_DIALOGUE_CUES, BONEYARD_SOLOMON_DIALOGUE_CUES,
    EnemyEventSoundRequest, boneyard_enemy_ambient_requests, enemy_event_sound_request,
    loot_event_sound_request, solomon_dig_sound_request,
};

use std::collections::BTreeMap;

use crate::Vec2;
use crate::boneyard::encounter::SolomonEncounter;
use crate::boneyard::loot::BoneyardLootEvent;
use crate::enemies::model::EnemyEvent;
use crate::enemies::world::EnemyWorld;
use crate::profile::WizardElement;
use crate::secondary::SecondarySimulationState;
use crate::spells::casting::PlayerPrimaryCastState;
use crate::spells::model::SpellState;
use crate::spells::staff::PlayerStaffContactEvent;

use super::{AudioCue, AudioEvent, AudioOptions, footstep_cue, hub_audio_attenuation};

/// A looping cue held by one named owner, with its current playback options.
#[derive(Clone, Debug, PartialEq)]
pub struct CombatAudioLoop {
    pub cue: AudioCue,
    pub owner: String,
    pub options: AudioOptions,
}

/// A loop change for the audio player: start an owner's loop (or change its options), or
/// stop it.
#[derive(Clone, Debug, PartialEq)]
pub enum CombatAudioLoopCommand {
    Start(CombatAudioLoop),
    Stop { cue: AudioCue, owner: String },
}

/// The sound events, loop changes and narration mix produced by one combat audio update.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CombatAudioOutput {
    pub events: Vec<AudioEvent>,
    pub loops: Vec<CombatAudioLoopCommand>,
    /// Asks the audio player to stop the Faculty, Solomon and Tutorial dialogue streams.
    pub stop_encounter_dialogue: bool,
    pub narration_mix: Option<f64>,
}

/// One player's sound inputs for a frame: identity, world, position, element, selected
/// primary or Weld, cast counters and latest footstep tick.
pub struct CombatAudioPlayer<'a> {
    pub id: &'a str,
    pub world_key: &'a str,
    pub position: Vec2,
    pub element: WizardElement,
    pub selected_primary_skill_id: u32,
    pub weld_build_id: Option<u32>,
    pub cast: &'a PlayerPrimaryCastState,
    pub footstep_tick: u64,
}

/// All inputs are actual completed-tick facts. The camera gains are the same canonical
/// point/hit queries used by Boneyard rendering, including the dying-player adjustment.
pub struct CombatAudioFrame<'a> {
    pub tick: u64,
    pub run_id: Option<&'a str>,
    pub listener_id: &'a str,
    pub players: &'a [CombatAudioPlayer<'a>],
    pub primary: &'a SpellState,
    pub secondary: &'a SecondarySimulationState,
    pub staff_contacts: &'a [PlayerStaffContactEvent],
    pub enemies: Option<&'a EnemyWorld>,
    pub enemy_events: &'a [EnemyEvent],
    pub loot_events: &'a [BoneyardLootEvent],
    pub encounter: Option<&'a SolomonEncounter>,
    pub point_gain: &'a dyn Fn(Vec2) -> f64,
    pub hit_gain: &'a dyn Fn(Vec2) -> f64,
}

#[derive(Clone, Copy)]
struct PlayerSoundClock {
    cast: u64,
    emission: u64,
    fizzle: u64,
    ether_charge: u64,
    footstep: u64,
}

impl From<&CombatAudioPlayer<'_>> for PlayerSoundClock {
    fn from(player: &CombatAudioPlayer<'_>) -> Self {
        Self {
            cast: player.cast.cast_sequence,
            emission: player.cast.emission_sequence,
            fizzle: player.cast.fizzle_sequence,
            ether_charge: player.cast.ether_blast_charge_cue_sequence,
            footstep: player.footstep_tick,
        }
    }
}

/// What the previous combat audio frame already played: sound counters, actor identities
/// and active loops, so each one-shot plays once.
pub struct CombatAudioCursor {
    players: BTreeMap<String, PlayerSoundClock>,
    primary: primary::PrimarySoundMemory,
    next_secondary_event_id: u64,
    loops: Vec<CombatAudioLoop>,
    ambient_active: Vec<AudioCue>,
    run_id: Option<String>,
    listener_id: String,
    enemy_event_id: usize,
    loot_event_id: u64,
    dig_event_id: Option<u64>,
    boss_run_id: Option<String>,
    boss_voice_event_id: usize,
    voice_run_id: Option<String>,
    solomon_voice_event_id: u64,
}

impl CombatAudioCursor {
    /// Attach to a snapshot without replaying retained one-shots. Calling `update` with this
    /// same frame then starts its live loops.
    #[must_use]
    pub fn new(frame: &CombatAudioFrame<'_>) -> Self {
        let mut players = BTreeMap::new();
        for player in frame.players {
            players.insert(player.id.to_owned(), PlayerSoundClock::from(player));
        }
        Self {
            players,
            primary: primary::PrimarySoundMemory::capture(frame),
            next_secondary_event_id: frame.secondary.next_event_id,
            loops: Vec::new(),
            ambient_active: Vec::new(),
            run_id: frame.run_id.map(str::to_owned),
            listener_id: frame.listener_id.to_owned(),
            enemy_event_id: frame.enemy_events.last().map_or(0, |event| event.event_id),
            loot_event_id: frame.loot_events.last().map_or(0, |event| event.event_id),
            dig_event_id: frame.encounter.map(|encounter| encounter.dig_event_id),
            boss_run_id: None,
            boss_voice_event_id: 0,
            voice_run_id: frame.run_id.map(str::to_owned),
            solomon_voice_event_id: frame
                .encounter
                .and_then(|encounter| encounter.voice_events.last())
                .map_or(0, |event| event.id),
        }
    }

    pub fn update(&mut self, frame: &CombatAudioFrame<'_>) -> CombatAudioOutput {
        let mut output = CombatAudioOutput::default();
        self.project_primary(frame, &mut output);
        self.project_world_events(frame, &mut output);
        self.project_narration(frame, &mut output);
        self.project_footsteps(frame, &mut output);
        let desired = self.primary_loops(frame);
        self.sync_primary_loops(desired, &mut output);
        self.project_enemy_ambient(frame, &mut output);
        self.players.clear();
        for player in frame.players {
            self.players.insert(player.id.to_owned(), PlayerSoundClock::from(player));
        }
        self.primary = primary::PrimarySoundMemory::capture(frame);
        self.next_secondary_event_id = frame.secondary.next_event_id;
        output
    }

    /// Scene teardown releases every owned loop through the existing audio adapter.
    pub fn destroy(&mut self) -> CombatAudioOutput {
        let mut output = CombatAudioOutput::default();
        for request in self.loops.drain(..) {
            output
                .loops
                .push(CombatAudioLoopCommand::Stop { cue: request.cue, owner: request.owner });
        }
        for cue in self.ambient_active.drain(..) {
            output.loops.push(CombatAudioLoopCommand::Stop { cue, owner: ambient_owner(cue) });
        }
        output.events.push(AudioEvent::StopStream(AudioCue::LoseReverie));
        world::stop_encounter_streams(&mut output);
        output.narration_mix = Some(1.0);
        output
    }

    fn project_footsteps(&self, frame: &CombatAudioFrame<'_>, output: &mut CombatAudioOutput) {
        for player in frame.players {
            let Some(previous) = self.players.get(player.id) else { continue };
            if player.footstep_tick == 0 || player.footstep_tick == previous.footstep {
                continue;
            }
            let Some(volume) = player_attenuation(frame, player.id) else { continue };
            play(output, footstep_cue(player.footstep_tick as u32, player.id), 1.0, volume * 0.5);
        }
    }

    fn sync_primary_loops(
        &mut self,
        desired: Vec<CombatAudioLoop>,
        output: &mut CombatAudioOutput,
    ) {
        for cue in skills::PRIMARY_LOOP_CUES {
            for request in desired.iter().filter(|request| request.cue == cue) {
                let previous = self
                    .loops
                    .iter()
                    .find(|previous| previous.cue == cue && previous.owner == request.owner);
                if previous.is_none_or(|previous| previous.options != request.options) {
                    output.loops.push(CombatAudioLoopCommand::Start(request.clone()));
                }
            }
            for previous in self.loops.iter().filter(|request| request.cue == cue) {
                if !desired
                    .iter()
                    .any(|request| request.cue == cue && request.owner == previous.owner)
                {
                    output
                        .loops
                        .push(CombatAudioLoopCommand::Stop { cue, owner: previous.owner.clone() });
                }
            }
        }
        self.loops = desired;
    }
}

fn listener<'a>(frame: &'a CombatAudioFrame<'a>) -> Option<&'a CombatAudioPlayer<'a>> {
    frame.players.iter().find(|player| player.id == frame.listener_id)
}

fn player_attenuation(frame: &CombatAudioFrame<'_>, id: &str) -> Option<f64> {
    let listener = listener(frame)?;
    let source = frame.players.iter().find(|player| player.id == id)?;
    if source.world_key != listener.world_key {
        return None;
    }
    Some(distance_gain(listener.position, source.position))
}

fn distance_gain(listener: Vec2, position: Vec2) -> f64 {
    hub_audio_attenuation(crate::float::hypot(position.x - listener.x, position.y - listener.y))
}

fn play(output: &mut CombatAudioOutput, cue: AudioCue, pitch: f64, volume: f64) {
    output.events.push(AudioEvent::Play(
        cue,
        AudioOptions { playback_rate: pitch, volume, offset_seconds: 0.0 },
    ));
}

fn cue(id: &str) -> AudioCue {
    AudioCue::from_id(id)
        .unwrap_or_else(|| panic!("source combat cue {id} is absent from the stock catalog"))
}

fn ambient_owner(cue: AudioCue) -> String {
    format!("boneyard-enemy-ambient:{}", cue.as_str())
}
