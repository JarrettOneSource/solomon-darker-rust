//! Native enemy/loot requests and the ten shared enemy ambience loops.
//! References: `game-audio-native.ts`, `boneyard-enemy-ambient-audio.ts`, `BoneyardScene.tsx`.

use crate::Vec2;
use crate::boneyard::encounter::{SolomonDigCue, SolomonDigEvent};
use crate::boneyard::loot::BoneyardLootEvent;
use crate::enemies::EnemyToken;
use crate::enemies::boss_spell::BossSpellKind;
use crate::enemies::model::{EnemyBrain, EnemyEvent, EnemyEventKind, EnemyLifeState, EnemySound};
use crate::enemies::world::EnemyWorld;

use super::{
    CombatAudioCursor, CombatAudioFrame, CombatAudioLoop, CombatAudioLoopCommand,
    CombatAudioOutput, ambient_owner, cue, play,
};
use crate::audio::{AudioCue, AudioEvent, AudioOptions};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnemyEventSoundRequest {
    pub cue: AudioCue,
    pub playback_rate: f64,
    pub source_position: Option<Vec2>,
    pub volume: f64,
}

#[must_use]
pub fn native_enemy_event_sound_request(
    event: &EnemyEvent,
) -> Option<NativeEnemyEventSoundRequest> {
    if let Some(pitch) = event.deflect_pitch {
        return Some(NativeEnemyEventSoundRequest {
            cue: AudioCue::Swipe,
            playback_rate: pitch,
            source_position: None,
            volume: 1.0,
        });
    }
    if !matches!(
        event.kind,
        EnemyEventKind::EnemyActionSound
            | EnemyEventKind::EnemyDamageSound
            | EnemyEventKind::EnemyDeathSound
            | EnemyEventKind::PlayerDamageSound
            | EnemyEventKind::PlayerStatusSound
    ) {
        return None;
    }
    let sound = event.sound.expect("accepted enemy sound event carries its native cue");
    Some(NativeEnemyEventSoundRequest {
        cue: cue(sound.id()),
        playback_rate: event.pitch.expect("accepted enemy sound event carries its native pitch"),
        source_position: if sound == EnemySound::Poisoned
            || (event.kind == EnemyEventKind::PlayerStatusSound
                && sound == EnemySound::MagicMissileHit)
        {
            None
        } else {
            event.source_position
        },
        volume: event.gain_scale.expect("accepted enemy sound event carries its native gain"),
    })
}

#[must_use]
pub fn native_loot_event_sound_request(
    event: &BoneyardLootEvent,
) -> Option<NativeEnemyEventSoundRequest> {
    Some(NativeEnemyEventSoundRequest {
        cue: cue(event.sound?.as_str()),
        playback_rate: event.playback_rate?,
        source_position: Some(event.position),
        volume: 1.0,
    })
}

#[must_use]
pub fn native_solomon_dig_sound_request(event: SolomonDigEvent) -> NativeEnemyEventSoundRequest {
    NativeEnemyEventSoundRequest {
        cue: cue(event.cue.id()),
        playback_rate: 1.0,
        source_position: None,
        volume: if matches!(event.cue, SolomonDigCue::Shovel1 | SolomonDigCue::Shovel2) {
            0.5
        } else {
            1.0
        },
    }
}

pub const BONEYARD_ENEMY_AMBIENT_CUES: [AudioCue; 10] = [
    AudioCue::EarthquakeLoop,
    AudioCue::RollingStoneLoop,
    AudioCue::FlyblownLoop,
    AudioCue::MaggotsLoop,
    AudioCue::SoulLoop,
    AudioCue::SteadyWindLoop,
    AudioCue::IceBeamLoop,
    AudioCue::ElectricLoop,
    AudioCue::EerieLoop,
    AudioCue::LowFireLoop,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneyardEnemyAmbientRequest {
    pub cue: AudioCue,
    pub gain: f64,
}

#[must_use]
pub fn native_boneyard_enemy_ambient_requests(
    world: &EnemyWorld,
    tick: u64,
    point_gain: &dyn Fn(Vec2) -> f64,
    hit_gain: &dyn Fn(Vec2) -> f64,
) -> [BoneyardEnemyAmbientRequest; 10] {
    let mut gains = [0.0_f64; 10];
    for actor in &world.actors {
        if actor.life_state == EnemyLifeState::Dying {
            if actor.config.enemy_token == EnemyToken::DemonSkull {
                gains[0] = 1.0;
            }
            continue;
        }
        let spatial_gain = clamp_unit(point_gain(actor.position));
        match actor.config.enemy_token {
            EnemyToken::Zombie
                if actor.config.flags.contains(&crate::enemies::EnemyFlag::Rotten) =>
            {
                gains[2] = gains[2].max(spatial_gain);
            }
            EnemyToken::DireFaculty => {
                let hit_gain = clamp_unit(hit_gain(actor.position));
                gains[4] = gains[4].max(hit_gain.powi(2));
                gains[5] = gains[5].max(hit_gain.powi(2));
                let EnemyBrain::Faculty(brain) = &actor.brain else {
                    unreachable!("Faculty brain")
                };
                if brain.hand_mask != 0 {
                    gains[6] = gains[6].max(hit_gain);
                }
                if brain.lightning_active {
                    gains[7] = gains[7].max(spatial_gain);
                }
            }
            EnemyToken::Wraith => gains[4] = gains[4].max(spatial_gain),
            EnemyToken::Coffin => {
                let live_maggots = world
                    .maggots
                    .iter()
                    .filter(|maggot| {
                        // The source projector gives its six-tick bite precedence over death.
                        // A recently biting terminal row therefore still contributes here.
                        let biting = maggot
                            .last_attack_tick
                            .is_some_and(|attack| tick.saturating_sub(attack) < 6);
                        maggot.owner_coffin_actor_id == actor.id
                            && (biting || maggot.life_state != EnemyLifeState::Dying)
                    })
                    .count();
                let weighted = spatial_gain * (live_maggots as f64 / 200.0).min(1.0) * 0.5;
                gains[3] = gains[3].max(weighted);
            }
            _ => {}
        }
    }
    for spell in &world.boss_spells {
        let position = spell.owner.position;
        match &spell.kind {
            BossSpellKind::GreenFire { .. } => gains[9] = gains[9].max(point_gain(position) / 3.0),
            BossSpellKind::MouthBeamSegment { .. } => {
                if let Some(owner) =
                    world.actors.iter().find(|actor| actor.id == spell.owner.owner_actor_id)
                {
                    gains[6] = gains[6].max(point_gain(owner.position));
                }
            }
            BossSpellKind::UltraBanish { alpha, .. } => {
                for index in [0, 1, 4, 5, 9, 6] {
                    gains[index] = gains[index].max(*alpha);
                }
            }
            BossSpellKind::RainOfBones { alpha, .. } => {
                gains[8] = gains[8].max(clamp_unit(hit_gain(position)) * alpha * 0.5);
            }
            BossSpellKind::TragicCircle { .. } => {
                gains[8] = gains[8].max(clamp_unit(hit_gain(position)) * 0.5)
            }
            BossSpellKind::SkullMissile { .. } => {
                gains[8] = gains[8].max(clamp_unit(point_gain(position)) * 0.5)
            }
            BossSpellKind::DarkFireball { .. } => {
                let spatial_gain = clamp_unit(point_gain(position));
                gains[8] = gains[8].max(spatial_gain);
                gains[9] = gains[9].max(spatial_gain);
            }
            _ => {}
        }
    }
    std::array::from_fn(|index| BoneyardEnemyAmbientRequest {
        cue: BONEYARD_ENEMY_AMBIENT_CUES[index],
        gain: gains[index],
    })
}

impl CombatAudioCursor {
    pub(super) fn project_narration(
        &mut self,
        frame: &CombatAudioFrame<'_>,
        output: &mut CombatAudioOutput,
    ) {
        let Some(run_id) = frame.run_id else { return };
        if let Some(world) = frame.enemies {
            output.narration_mix = Some(world.boss_narration.mix);
            if self.boss_run_id.as_deref() != Some(run_id) {
                stop_faculty_voices(output);
                self.boss_run_id = Some(run_id.to_owned());
                self.boss_voice_event_id = 0;
            }
            if let Some(line) = world.boss_narration.current {
                if line.event_id != self.boss_voice_event_id {
                    stop_faculty_voices(output);
                    let offset = (line.cue.ticks() as f64
                        - world.boss_narration.ticks_remaining as f64)
                        / 100.0;
                    output.events.push(AudioEvent::Play(
                        cue(line.cue.id()),
                        AudioOptions { offset_seconds: offset, ..AudioOptions::default() },
                    ));
                    self.boss_voice_event_id = line.event_id;
                }
            } else if self.boss_voice_event_id != 0 {
                stop_faculty_voices(output);
                self.boss_voice_event_id = 0;
            }
        }
        if self.voice_run_id.as_deref() != Some(run_id) {
            stop_solomon_voices(output);
            self.voice_run_id = Some(run_id.to_owned());
            self.solomon_voice_event_id = 0;
        }
        if let Some(event) = frame.encounter.and_then(|encounter| encounter.voice_events.last())
            && event.id > self.solomon_voice_event_id
        {
            play(output, cue(event.cue.id()), 1.0, 1.0);
            self.solomon_voice_event_id = event.id;
        }
    }

    pub(super) fn project_world_events(
        &mut self,
        frame: &CombatAudioFrame<'_>,
        output: &mut CombatAudioOutput,
    ) {
        let changed_listener = self.listener_id != frame.listener_id;
        if self.run_id.is_some() && (self.run_id.as_deref() != frame.run_id || changed_listener) {
            // `subscribeRunCharmLossAudio` releases its owner-local stream on
            // run/player teardown, independently of the shared narration streams.
            output.events.push(AudioEvent::StopStream(AudioCue::LoseReverie));
        }
        if self.run_id.as_deref() != frame.run_id {
            self.run_id = frame.run_id.map(str::to_owned);
            self.enemy_event_id = 0;
            self.loot_event_id = 0;
            self.dig_event_id = None;
        }
        if changed_listener {
            self.listener_id = frame.listener_id.to_owned();
            self.enemy_event_id = frame.enemy_events.last().map_or(0, |event| event.event_id);
        }
        if frame.run_id.is_none() {
            return;
        }
        for event in frame.enemy_events {
            if event.event_id <= self.enemy_event_id {
                continue;
            }
            self.enemy_event_id = event.event_id;
            if event.kind == EnemyEventKind::PlayerCharmLost {
                if event.target_player_id.as_ref().and_then(|target| target.as_deref())
                    == Some(frame.listener_id)
                {
                    play(output, AudioCue::LoseReverie, 1.0, 1.0);
                }
                continue;
            }
            if event.kind == EnemyEventKind::PlayerCheatDeath {
                let position = event.source_position.expect("accepted rescue has its player root");
                let gain = (frame.point_gain)(position);
                // `nativePlayerCheatDeathSoundRequests` shares one point gain
                // across its four ordered cues; peer rescues are audible too.
                for (cue, rate) in [
                    (AudioCue::FlashSpell, 1.0),
                    (AudioCue::FlashSpell, 0.8_f32 as f64),
                    (AudioCue::FlashSpell, 0.5),
                    (AudioCue::Flash, 1.0),
                ] {
                    play(output, cue, rate, gain);
                }
                continue;
            }
            if event.kind == EnemyEventKind::EnemyDialogueStop {
                output.stop_encounter_dialogue = true;
            }
            let sound = native_enemy_event_sound_request(event);
            let position = sound.and_then(|sound| sound.source_position).or(event.source_position);
            let gain = position.map_or(1.0, frame.point_gain);
            if let Some(stream) = &event.stream {
                play(output, cue(stream), 1.0, gain);
            } else if let Some(sound) = sound {
                play(output, sound.cue, sound.playback_rate, sound.volume * gain);
            }
        }
        for event in frame.loot_events {
            if event.event_id <= self.loot_event_id {
                continue;
            }
            self.loot_event_id = event.event_id;
            if let Some(sound) = native_loot_event_sound_request(event) {
                let gain = sound.source_position.map_or(1.0, frame.point_gain);
                play(output, sound.cue, sound.playback_rate, sound.volume * gain);
            }
        }
        if let Some(encounter) = frame.encounter {
            if let Some(previous_id) = self.dig_event_id {
                for event in &encounter.dig_events {
                    if event.id <= previous_id {
                        continue;
                    }
                    let sound = native_solomon_dig_sound_request(*event);
                    play(
                        output,
                        sound.cue,
                        sound.playback_rate,
                        sound.volume * (frame.hit_gain)(encounter.position),
                    );
                }
            }
            self.dig_event_id = Some(
                encounter
                    .dig_events
                    .last()
                    .map_or(self.dig_event_id.unwrap_or(0), |event| event.id),
            );
        }
    }

    pub(super) fn project_enemy_ambient(
        &mut self,
        frame: &CombatAudioFrame<'_>,
        output: &mut CombatAudioOutput,
    ) {
        let requests = frame.enemies.map(|world| {
            native_boneyard_enemy_ambient_requests(
                world,
                frame.tick,
                frame.point_gain,
                frame.hit_gain,
            )
        });
        let mut active = Vec::new();
        for cue in BONEYARD_ENEMY_AMBIENT_CUES {
            let gain = requests
                .as_ref()
                .and_then(|requests| requests.iter().find(|request| request.cue == cue))
                .map_or(0.0, |request| request.gain);
            if gain > 0.0 {
                output.loops.push(CombatAudioLoopCommand::Start(CombatAudioLoop {
                    cue,
                    owner: ambient_owner(cue),
                    options: AudioOptions { volume: gain, ..AudioOptions::default() },
                }));
                active.push(cue);
            } else if self.ambient_active.contains(&cue) {
                output.loops.push(CombatAudioLoopCommand::Stop { cue, owner: ambient_owner(cue) });
            }
        }
        self.ambient_active = active;
    }
}

pub const BONEYARD_FACULTY_DIALOGUE_CUES: [AudioCue; 8] = [
    AudioCue::FacultyJoinUs1,
    AudioCue::FacultyJoinUs1Female,
    AudioCue::FacultyJoinUs2,
    AudioCue::FacultyJoinUs2Female,
    AudioCue::FacultyDeadIsBetter1,
    AudioCue::FacultyDeadIsBetter1Female,
    AudioCue::FacultyDeadIsBetter2,
    AudioCue::FacultyDeadIsBetter2Female,
];

pub const BONEYARD_SOLOMON_DIALOGUE_CUES: [AudioCue; 6] = [
    AudioCue::SolomonHello1,
    AudioCue::SolomonHello2,
    AudioCue::SolomonHello3,
    AudioCue::SolomonHello4,
    AudioCue::SolomonLaugh1,
    AudioCue::SolomonGetHimBoys,
];

fn stop_faculty_voices(output: &mut CombatAudioOutput) {
    for cue in BONEYARD_FACULTY_DIALOGUE_CUES {
        output.events.push(AudioEvent::StopStream(cue));
    }
}

fn stop_solomon_voices(output: &mut CombatAudioOutput) {
    for cue in BONEYARD_SOLOMON_DIALOGUE_CUES {
        output.events.push(AudioEvent::StopStream(cue));
    }
}

pub(super) fn stop_encounter_streams(output: &mut CombatAudioOutput) {
    stop_faculty_voices(output);
    stop_solomon_voices(output);
    for cue in [
        AudioCue::Trap,
        AudioCue::UnholyDie,
        AudioCue::UnholyScream,
        AudioCue::FacultyDie,
        AudioCue::FacultyNo,
        AudioCue::FacultyNoFemale,
        AudioCue::HeartBreak,
    ] {
        output.events.push(AudioEvent::StopStream(cue));
    }
}

fn clamp_unit(value: f64) -> f64 {
    if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 }
}
