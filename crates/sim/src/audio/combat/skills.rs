//! Primary channel, welded-primary and native secondary loop/cast cue ownership.
//! References: `primary-spell-audio.ts`, `weld-primary-audio-contract.ts` and `air-water-skill-audio.ts`.

use crate::audio::{AudioCue, AudioEvent, AudioOptions};
use crate::profile::WizardElement;
use crate::secondary::NativeSecondaryActorKind;
use crate::spells::model::{
    PrimaryProjectileRef, PrimarySpellProjectilePhase, PrimaryTransientRef,
};
use crate::spells::weld::native_weld_audio_plan;

use super::{
    CombatAudioCursor, CombatAudioFrame, CombatAudioLoop, CombatAudioPlayer, distance_gain,
    listener, player_attenuation,
};

pub(super) const PRIMARY_LOOP_CUES: [AudioCue; 15] = [
    AudioCue::CometLoop,
    AudioCue::ElectricLoop,
    AudioCue::EarthquakeLoop,
    AudioCue::FireLoop,
    AudioCue::GatherRocksLoop,
    AudioCue::IceBeamLoop,
    AudioCue::IceLoop,
    AudioCue::LightningLoop,
    AudioCue::LowFireLoop,
    AudioCue::MeteorLoop,
    AudioCue::PlaneCrossLoop,
    AudioCue::RainfallLoop,
    AudioCue::RollingStoneLoop,
    AudioCue::SteadyWindLoop,
    AudioCue::SteamLoop,
];

/// `None` means this is outside the accepted Air/Water cast table. Skill 76 is admitted
/// with an empty request list: its sound comes from the actual later actor, not the cast.
#[must_use]
pub fn native_air_water_accepted_cast_audio(skill_id: u32) -> Option<Vec<AudioEvent>> {
    let cues: &[(AudioCue, f64)] = match skill_id {
        27 | 72 => &[(AudioCue::MagicStorm, 1.0)],
        30 => &[(AudioCue::PrismaticShock, 1.0), (AudioCue::LightningStart, 0.8)],
        35 => &[(AudioCue::RingOfIce, 1.0)],
        76 => &[],
        _ => return None,
    };
    Some(
        cues.iter()
            .map(|&(cue, playback_rate)| {
                AudioEvent::Play(cue, AudioOptions { playback_rate, ..AudioOptions::default() })
            })
            .collect(),
    )
}

#[must_use]
pub fn native_weld_cast_sound_cues(build_id: u32, sound_variant: Option<u32>) -> Vec<AudioCue> {
    let plan = native_weld_audio_plan(build_id);
    let mut native_ids = plan.native_sound_ids.to_vec();
    if plan.native_sound_variant_ids.is_empty() {
        assert!(sound_variant.is_none(), "Weld build {build_id} has no native sound variant");
    } else {
        let variant = sound_variant.expect("Weld cast requires its native sound variant") as usize;
        native_ids.push(
            *plan
                .native_sound_variant_ids
                .get(variant)
                .expect("Weld sound variant lies in its authored bank"),
        );
    }
    native_ids
        .into_iter()
        .map(|id| match id {
            33 => AudioCue::FlameLashStart,
            38 => AudioCue::FrostMissile,
            44 => AudioCue::IceStart,
            57 => AudioCue::MagicMissile,
            87 => AudioCue::StartBoulder,
            97 => AudioCue::ThrowFire,
            203 => AudioCue::Shock1,
            204 => AudioCue::Shock2,
            205 => AudioCue::Shock3,
            224 => AudioCue::ThrowLightning1,
            225 => AudioCue::ThrowLightning2,
            _ => panic!("native Weld sound {id} has no stock audio asset"),
        })
        .collect()
}

#[must_use]
pub fn native_weld_loop_cues(build_id: u32) -> Vec<AudioCue> {
    native_weld_audio_plan(build_id)
        .native_loop_ids
        .iter()
        .map(|id| match id {
            157 => AudioCue::FireLoop,
            159 => AudioCue::GatherRocksLoop,
            160 => AudioCue::IceBeamLoop,
            165 => AudioCue::MeteorLoop,
            172 => AudioCue::SteamLoop,
            _ => panic!("native Weld loop {id} has no stock audio asset"),
        })
        .collect()
}

pub(super) fn selected_primary_element(player: &CombatAudioPlayer<'_>) -> WizardElement {
    match player.selected_primary_skill_id {
        8 => WizardElement::Ether,
        16 => WizardElement::Fire,
        24 => WizardElement::Air,
        32 => WizardElement::Water,
        40 => WizardElement::Earth,
        _ => player.element,
    }
}

pub(super) fn active_weld_build(player: &CombatAudioPlayer<'_>) -> Option<u32> {
    if player.selected_primary_skill_id != 52 {
        return None;
    }
    player.weld_build_id.filter(|id| (1000..=1009).contains(id))
}

pub(super) fn weld_one_shot(build_id: u32) -> bool {
    matches!(build_id, 1000 | 1001 | 1002 | 1009)
}

impl CombatAudioCursor {
    pub(super) fn primary_loops(&self, frame: &CombatAudioFrame<'_>) -> Vec<CombatAudioLoop> {
        let Some(listener) = listener(frame) else { return Vec::new() };
        let mut desired = Vec::new();
        for player in frame.players {
            if !player.cast.channel_active || player.world_key != listener.world_key {
                continue;
            }
            let Some(volume) = player_attenuation(frame, player.id) else { continue };
            let owner = format!("primary-player:{}", player.id);
            if let Some(build_id) = active_weld_build(player) {
                for cue in native_weld_loop_cues(build_id) {
                    desired.push(CombatAudioLoop {
                        cue,
                        owner: owner.clone(),
                        options: AudioOptions {
                            volume,
                            playback_rate: if build_id == 1007 && player.cast.underpowered {
                                0.75
                            } else {
                                1.0
                            },
                            offset_seconds: 0.0,
                        },
                    });
                }
                continue;
            }
            let request = match selected_primary_element(player) {
                WizardElement::Air => Some((
                    AudioCue::LightningLoop,
                    volume * if player.cast.underpowered { 0.75 } else { 1.0 },
                )),
                WizardElement::Water => Some((
                    AudioCue::IceLoop,
                    volume * if player.cast.underpowered { 0.5 } else { 1.0 },
                )),
                WizardElement::Earth => {
                    let gathering = frame.primary.projectiles.iter().any(|spell| {
                        matches!(spell, PrimaryProjectileRef::Earth(earth) if earth.common.owner_id == player.id
                            && earth.common.phase == PrimarySpellProjectilePhase::Held && earth.common.charge < 1.0
                            && earth.common.world_key == listener.world_key)
                    });
                    gathering.then_some((AudioCue::GatherRocksLoop, volume))
                }
                WizardElement::Ether | WizardElement::Fire => None,
            };
            if let Some((cue, volume)) = request {
                desired.push(CombatAudioLoop {
                    cue,
                    owner,
                    options: AudioOptions { volume, ..AudioOptions::default() },
                });
            }
        }
        for spell in frame.primary.projectiles.iter() {
            if let PrimaryProjectileRef::Earth(earth) = spell
                && earth.common.phase == PrimarySpellProjectilePhase::Flight
                && earth.common.world_key == listener.world_key
            {
                desired.push(CombatAudioLoop {
                    cue: AudioCue::RollingStoneLoop,
                    owner: format!("primary-spell:{}", earth.common.id),
                    options: AudioOptions {
                        volume: distance_gain(listener.position, earth.common.position),
                        ..AudioOptions::default()
                    },
                });
            }
        }
        for actor in frame.primary.transients.iter() {
            if let PrimaryTransientRef::AirHurricane(hurricane) = actor
                && hurricane.common.world_key == listener.world_key
                && hurricane.contact_charge > 0.0
            {
                request_secondary_loop(
                    &mut desired,
                    AudioCue::SteadyWindLoop,
                    hurricane.contact_charge * distance_gain(listener.position, hurricane.position),
                );
            }
        }
        for (owner_id, player) in frame.secondary.players.iter() {
            if player.planewalker_ticks_remaining == 0 {
                continue;
            }
            let Some(owner) = frame.players.iter().find(|player| player.id == owner_id) else {
                continue;
            };
            if owner.world_key == listener.world_key {
                request_secondary_loop(
                    &mut desired,
                    AudioCue::PlaneCrossLoop,
                    distance_gain(listener.position, owner.position),
                );
            }
        }
        for actor in frame.secondary.actors.iter() {
            if actor.world_key != listener.world_key {
                continue;
            }
            let gain = distance_gain(listener.position, actor.position);
            match actor.kind {
                NativeSecondaryActorKind::Leviathan | NativeSecondaryActorKind::EtherDrain => {
                    request_secondary_loop(&mut desired, AudioCue::PlaneCrossLoop, gain);
                }
                NativeSecondaryActorKind::MovingFire | NativeSecondaryActorKind::FirePatch => {
                    request_secondary_loop(&mut desired, AudioCue::LowFireLoop, gain);
                }
                NativeSecondaryActorKind::StormCloud => {
                    request_secondary_loop(&mut desired, AudioCue::RainfallLoop, gain);
                    request_secondary_loop(&mut desired, AudioCue::SteadyWindLoop, gain);
                }
                NativeSecondaryActorKind::AcidRain if actor.phase > 0.0 => {
                    request_secondary_loop(
                        &mut desired,
                        AudioCue::RainfallLoop,
                        gain * actor.phase,
                    );
                }
                NativeSecondaryActorKind::Earthquake => {
                    request_secondary_loop(&mut desired, AudioCue::EarthquakeLoop, gain)
                }
                NativeSecondaryActorKind::Comet => {
                    request_secondary_loop(&mut desired, AudioCue::CometLoop, gain)
                }
                NativeSecondaryActorKind::ElectricBurn if actor.age_ticks > 0 => {
                    request_secondary_loop(&mut desired, AudioCue::ElectricLoop, gain)
                }
                _ => {}
            }
            if actor.kind == NativeSecondaryActorKind::EtherDrain {
                request_secondary_loop(&mut desired, AudioCue::SteadyWindLoop, gain);
            }
        }
        desired
    }
}

pub(super) fn request_secondary_loop(
    desired: &mut Vec<CombatAudioLoop>,
    cue: AudioCue,
    volume: f64,
) {
    let owner = format!("native-ambient:{}", cue.as_str());
    if let Some(previous) =
        desired.iter_mut().find(|request| request.cue == cue && request.owner == owner)
    {
        previous.options.volume = previous.options.volume.max(volume);
    } else {
        desired.push(CombatAudioLoop {
            cue,
            owner,
            options: AudioOptions { volume, ..AudioOptions::default() },
        });
    }
}

pub(super) fn is_secondary_loop(cue: AudioCue) -> bool {
    matches!(
        cue,
        AudioCue::CometLoop
            | AudioCue::ElectricLoop
            | AudioCue::EarthquakeLoop
            | AudioCue::LowFireLoop
            | AudioCue::PlaneCrossLoop
            | AudioCue::RainfallLoop
            | AudioCue::SteadyWindLoop
    )
}
