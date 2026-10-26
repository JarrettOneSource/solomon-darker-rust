//! Exactly-once primary/secondary actor sounds from `primary-spell-audio.ts`.
//! Only the source's small previous-snapshot cursors are retained, not actor copies.

use std::collections::{BTreeMap, BTreeSet};

use crate::audio::AudioCue;
use crate::float::fround;
use crate::profile::WizardElement;
use crate::secondary::NativeSecondaryActorKind;
use crate::spells::earth::{
    native_earth_boulder_rock_hit_pitch, native_earth_boulder_stone_break_pitch,
};
use crate::spells::fire::native_fire_impact_pitch;
use crate::spells::model::{
    FireExplosionPresentation, PrimaryProjectileRef, PrimarySpellProjectilePhase,
    PrimarySpellTransient, PrimarySpellTransientBase,
};
use crate::spells::weld::{
    NativeWeldMeteorPhase, NativeWeldPersistentActorState, NativeWeldWorldActor,
};

use super::skills::{
    active_weld_build, is_secondary_loop, native_weld_cast_sound_cues, selected_primary_element,
    weld_one_shot,
};
use super::{
    CombatAudioCursor, CombatAudioFrame, CombatAudioOutput, cue, distance_gain, listener, play,
    player_attenuation,
};

type ActorKey = (String, u64);

#[derive(Clone, Copy)]
struct GoodImpSoundClock {
    bounce: u64,
    contact: u64,
}

#[derive(Default)]
pub(super) struct PrimarySoundMemory {
    ether_blasts: BTreeSet<ActorKey>,
    fire_impacts: BTreeSet<u64>,
    fire_explosions: BTreeSet<u64>,
    meteor_impacts: BTreeSet<ActorKey>,
    weld_impacts: BTreeSet<ActorKey>,
    earth_impacts: BTreeSet<ActorKey>,
    flying_hailstones: BTreeSet<ActorKey>,
    good_imps: BTreeMap<ActorKey, GoodImpSoundClock>,
    hail_sequences: BTreeMap<u64, u64>,
    ether_impacts: BTreeSet<u64>,
    staff_contacts: BTreeSet<ActorKey>,
    mindblasts: BTreeSet<ActorKey>,
}

impl PrimarySoundMemory {
    pub(super) fn capture(frame: &CombatAudioFrame<'_>) -> Self {
        let mut memory = Self::default();
        for actor in &frame.primary.transients {
            let common = actor.common();
            match actor {
                PrimarySpellTransient::EtherBlast(_) => {
                    memory.ether_blasts.insert(key(common));
                }
                PrimarySpellTransient::FireImpact(_) => {
                    memory.fire_impacts.insert(common.id);
                }
                PrimarySpellTransient::FireExplosion(_) => {
                    memory.fire_explosions.insert(common.id);
                }
                PrimarySpellTransient::EarthImpact(_) => {
                    memory.earth_impacts.insert(key(common));
                }
                PrimarySpellTransient::EtherImpact(_) => {
                    memory.ether_impacts.insert(common.id);
                }
                PrimarySpellTransient::FireGoodImp(imp) => {
                    memory.good_imps.insert(
                        key(common),
                        GoodImpSoundClock {
                            bounce: imp.bounce_sound_sequence,
                            contact: imp.contact_sound_sequence,
                        },
                    );
                }
                PrimarySpellTransient::WaterHail(hail) => {
                    memory.hail_sequences.insert(common.id, hail.bounce_sound_sequence);
                }
                PrimarySpellTransient::Weld(weld) => match weld {
                    NativeWeldWorldActor::Meteor(meteor)
                        if meteor.phase == NativeWeldMeteorPhase::Impact =>
                    {
                        memory.meteor_impacts.insert(key(common));
                    }
                    NativeWeldWorldActor::Impact(_) => {
                        memory.weld_impacts.insert(key(common));
                    }
                    NativeWeldWorldActor::Persistent(
                        NativeWeldPersistentActorState::Hailstones(hail),
                    ) if hail.phase == PrimarySpellProjectilePhase::Flight => {
                        memory.flying_hailstones.insert(key(common));
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        for actor in frame.staff_contacts {
            memory.staff_contacts.insert(key(&actor.common));
        }
        for actor in frame.secondary.actors.iter() {
            if actor.kind == NativeSecondaryActorKind::MindblastBurst {
                memory.mindblasts.insert((actor.world_key.clone(), actor.id));
            }
        }
        memory
    }
}

impl CombatAudioCursor {
    pub(super) fn project_primary(
        &self,
        frame: &CombatAudioFrame<'_>,
        output: &mut CombatAudioOutput,
    ) {
        let Some(listener) = listener(frame) else { return };
        let world_key = listener.world_key;
        for player in frame.players {
            let Some(previous) = self.players.get(player.id) else { continue };
            let Some(volume) = player_attenuation(frame, player.id) else { continue };
            let build = active_weld_build(player);
            let element = selected_primary_element(player);
            if player.cast.cast_sequence > previous.cast {
                if let Some(build) = build {
                    if !weld_one_shot(build) {
                        for _ in previous.cast..player.cast.cast_sequence {
                            for cue in native_weld_cast_sound_cues(build, None) {
                                play(
                                    output,
                                    cue,
                                    player.cast.last_weld_playback_rate.unwrap_or(1.0),
                                    volume,
                                );
                            }
                        }
                    }
                } else {
                    match element {
                        WizardElement::Air => play(output, AudioCue::LightningStart, 1.0, volume),
                        WizardElement::Earth => play(output, AudioCue::StartBoulder, 1.0, volume),
                        WizardElement::Water => play(output, AudioCue::IceStart, 1.0, volume),
                        WizardElement::Ether | WizardElement::Fire => {}
                    }
                }
            }
            if player.cast.fizzle_sequence > previous.fizzle {
                if element == WizardElement::Earth {
                    let boulder = frame.primary.projectiles.iter().find_map(|spell| match spell {
                        PrimaryProjectileRef::Earth(earth)
                            if earth.common.owner_id == player.id =>
                        {
                            Some(earth)
                        }
                        _ => None,
                    });
                    let gain = boulder.map_or(volume, |earth| {
                        distance_gain(listener.position, earth.common.position)
                    });
                    play(output, AudioCue::Fizzle, 0.5, gain * 0.5);
                } else {
                    play(output, AudioCue::Fizzle, 1.0, volume);
                }
            }
            if player.cast.emission_sequence > previous.emission {
                if let Some(build) = build {
                    if weld_one_shot(build) {
                        let pitch = player
                            .cast
                            .last_weld_playback_rate
                            .expect("Weld emission carries its accepted playback rate");
                        for _ in previous.emission..player.cast.emission_sequence {
                            for cue in native_weld_cast_sound_cues(
                                build,
                                player.cast.last_weld_sound_variant,
                            ) {
                                play(output, cue, pitch, volume);
                            }
                        }
                    }
                } else {
                    let pitch = if player.cast.underpowered { 0.75 } else { 1.0 };
                    match element {
                        WizardElement::Ether => play(output, AudioCue::MagicMissile, pitch, volume),
                        WizardElement::Fire => play(output, AudioCue::ThrowFire, pitch, volume),
                        _ => {}
                    }
                }
            }
            for _ in previous.ether_charge..player.cast.ether_blast_charge_cue_sequence {
                play(output, AudioCue::MagicShieldUp, 2.0, volume);
            }
        }

        // Preserve the source's separate actor passes: their sound order is observable
        // when several different effects are born on one accepted snapshot.
        for actor in &frame.primary.transients {
            if let PrimarySpellTransient::EtherBlast(blast) = actor
                && blast.common.world_key == world_key
                && !self.primary.ether_blasts.contains(&key(&blast.common))
                && let Some(volume) = player_attenuation(frame, &blast.common.owner_id)
            {
                play(output, AudioCue::LightningStart, 2.0, volume);
                play(output, AudioCue::GotoOrb, 0.75, volume);
                play(output, AudioCue::GotoOrb, 0.5, volume);
            }
        }
        for actor in &frame.primary.transients {
            if let PrimarySpellTransient::FireImpact(impact) = actor
                && impact.common.world_key == world_key
                && !self.primary.fire_impacts.contains(&impact.common.id)
            {
                play(
                    output,
                    AudioCue::FireballHit,
                    native_fire_impact_pitch(impact.common.id),
                    distance_gain(listener.position, impact.origin),
                );
            }
        }
        for actor in &frame.primary.transients {
            if let PrimarySpellTransient::FireExplosion(explosion) = actor
                && explosion.common.world_key == world_key
                && !self.primary.fire_explosions.contains(&explosion.common.id)
            {
                let volume = 2.0 * distance_gain(listener.position, explosion.origin);
                if explosion.presentation == FireExplosionPresentation::Steam {
                    play(output, AudioCue::ExplodeSteam, explosion.sound_pitch, volume);
                } else {
                    play(output, AudioCue::FireballHit, explosion.sound_pitch, volume);
                    play(output, AudioCue::ThrowFire, f64::from(0.8_f32), volume);
                }
            }
        }
        self.project_weld_impacts(frame, output);
        for actor in &frame.primary.transients {
            if let PrimarySpellTransient::EarthImpact(impact) = actor
                && impact.common.world_key == world_key
                && !self.primary.earth_impacts.contains(&key(&impact.common))
            {
                earth_impact(
                    output,
                    impact.charge,
                    distance_gain(listener.position, impact.origin),
                );
            }
        }
        self.project_releases_and_imps(frame, output);
        self.project_secondary_events(frame, output);
        for actor in &frame.primary.transients {
            if let PrimarySpellTransient::EtherImpact(impact) = actor
                && impact.common.world_key == world_key
                && !self.primary.ether_impacts.contains(&impact.common.id)
            {
                play(
                    output,
                    AudioCue::MagicMissileHit,
                    native_ether_impact_pitch(impact.common.id),
                    distance_gain(listener.position, impact.origin),
                );
            }
        }
        for contact in frame.staff_contacts {
            if contact.common.world_key != world_key
                || self.primary.staff_contacts.contains(&key(&contact.common))
            {
                continue;
            }
            let volume = distance_gain(listener.position, contact.origin);
            play(output, AudioCue::StaffSwoosh, contact.swoosh_pitch, volume);
            for (index, &pitch) in contact.impact_sound_pitches.iter().enumerate() {
                play(output, AudioCue::StaffHitWood, pitch, volume);
                if contact.pike_break_sound_indexes.contains(&index) {
                    play(output, AudioCue::PikeBreak, 1.0, volume);
                }
            }
            if let Some(sound) = contact.proc_sound {
                for &pitch in &contact.proc_sound_pitches {
                    play(output, cue(sound.as_str()), pitch, volume);
                }
            }
        }
    }

    fn project_weld_impacts(&self, frame: &CombatAudioFrame<'_>, output: &mut CombatAudioOutput) {
        let listener = listener(frame).unwrap();
        for actor in &frame.primary.transients {
            if let PrimarySpellTransient::Weld(NativeWeldWorldActor::Meteor(meteor)) = actor
                && meteor.common.owner.world_key == listener.world_key
                && meteor.phase == NativeWeldMeteorPhase::Impact
                && !self.primary.meteor_impacts.contains(&key(&meteor.common.owner))
            {
                let volume = if meteor.underpowered {
                    1.0
                } else {
                    2.0 * distance_gain(listener.position, meteor.position)
                };
                play(
                    output,
                    AudioCue::FireballHit,
                    meteor.impact_sound_pitch.expect("Meteor impact carries its native pitch"),
                    volume,
                );
                if let Some(pitch) = meteor.impact_throw_fire_pitch {
                    play(output, AudioCue::ThrowFire, pitch, volume);
                }
            }
        }
        for actor in &frame.primary.transients {
            let PrimarySpellTransient::Weld(NativeWeldWorldActor::Impact(impact)) = actor else {
                continue;
            };
            if impact.common.owner.world_key != listener.world_key
                || self.primary.weld_impacts.contains(&key(&impact.common.owner))
            {
                continue;
            }
            let volume = distance_gain(listener.position, impact.position);
            match impact.common.build_id {
                1006 => {
                    if let Some(charge) = impact.boulder_terminal_charge {
                        earth_impact(output, charge, volume);
                    }
                }
                1001 => {
                    if let Some(pitch) = impact.impact_sound_pitch {
                        play(output, AudioCue::IceStart, pitch, volume);
                    }
                }
                1002 => {
                    if let Some(pitch) = impact.impact_sound_pitch {
                        play(output, AudioCue::ThrowLightning1, pitch, volume);
                    }
                }
                1009 => {
                    if let (Some(pitch), Some(variant)) =
                        (impact.impact_sound_pitch, impact.impact_sound_variant)
                    {
                        let cue = [AudioCue::Shock1, AudioCue::Shock2, AudioCue::Shock3]
                            [variant as usize];
                        play(output, cue, pitch, volume);
                    }
                }
                _ => {}
            }
        }
    }

    fn project_releases_and_imps(
        &self,
        frame: &CombatAudioFrame<'_>,
        output: &mut CombatAudioOutput,
    ) {
        let listener = listener(frame).unwrap();
        for actor in &frame.primary.transients {
            if let PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(
                NativeWeldPersistentActorState::Hailstones(hail),
            )) = actor
                && hail.common.owner.world_key == listener.world_key
                && hail.phase == PrimarySpellProjectilePhase::Flight
                && !self.primary.flying_hailstones.contains(&key(&hail.common.owner))
            {
                play(output, AudioCue::IceStart, 1.5, 1.0);
                play(output, AudioCue::RockHit, 1.5, 1.0);
                play(output, AudioCue::HailShot, 1.0, 1.0);
            }
        }
        for actor in &frame.primary.transients {
            let PrimarySpellTransient::FireGoodImp(imp) = actor else { continue };
            if imp.common.world_key != listener.world_key {
                continue;
            }
            let previous = self.primary.good_imps.get(&key(&imp.common));
            for _ in previous.map_or(0, |previous| previous.bounce)..imp.bounce_sound_sequence {
                let cue = [
                    AudioCue::ImpVocal1,
                    AudioCue::ImpVocal2,
                    AudioCue::ImpVocal3,
                    AudioCue::ImpVocal4,
                    AudioCue::ImpVocal5,
                    AudioCue::ImpVocal6,
                    AudioCue::ImpVocal7,
                    AudioCue::ImpVocal8,
                ][imp.bounce_sound_index as usize];
                play(
                    output,
                    cue,
                    imp.bounce_sound_pitch,
                    distance_gain(listener.position, imp.position),
                );
            }
            for _ in previous.map_or(0, |previous| previous.contact)..imp.contact_sound_sequence {
                let cue = [AudioCue::Bite1, AudioCue::Bite2, AudioCue::Bite3]
                    [imp.contact_sound_index as usize];
                play(
                    output,
                    cue,
                    imp.contact_sound_pitch,
                    distance_gain(listener.position, imp.contact_origin.unwrap_or(imp.position)),
                );
            }
        }
        for actor in &frame.primary.transients {
            let PrimarySpellTransient::WaterHail(hail) = actor else { continue };
            if hail.common.world_key != listener.world_key {
                continue;
            }
            let Some(&previous) = self.primary.hail_sequences.get(&hail.common.id) else {
                continue;
            };
            if hail.bounce_sound_sequence <= previous {
                continue;
            }
            let variant =
                hail.bounce_sound_index.expect("Hail bounce advanced with its native sample");
            let pitch =
                hail.bounce_sound_pitch.expect("Hail bounce advanced with its native pitch");
            let cue = [
                AudioCue::HailBounce0,
                AudioCue::HailBounce1,
                AudioCue::HailBounce2,
                AudioCue::HailBounce3,
            ][variant as usize];
            let volume = distance_gain(listener.position, hail.position);
            for _ in previous..hail.bounce_sound_sequence {
                play(output, cue, pitch, volume);
            }
        }
    }

    fn project_secondary_events(
        &self,
        frame: &CombatAudioFrame<'_>,
        output: &mut CombatAudioOutput,
    ) {
        let listener = listener(frame).unwrap();
        for event in &frame.secondary.events {
            if event.event_id < self.next_secondary_event_id
                || event.world_key != listener.world_key
            {
                continue;
            }
            let Some(sound) = event.cue else { continue };
            let sound = cue(sound.as_str());
            if !is_secondary_loop(sound) {
                play(
                    output,
                    sound,
                    event.pitch,
                    event.gain * distance_gain(listener.position, event.position),
                );
            }
        }
        for actor in frame.secondary.actors.iter() {
            if actor.kind == NativeSecondaryActorKind::MindblastBurst
                && actor.world_key == listener.world_key
                && !self.primary.mindblasts.contains(&(actor.world_key.clone(), actor.id))
            {
                let volume = distance_gain(listener.position, actor.position);
                play(output, AudioCue::MagicShieldExplode, 1.0, volume);
                play(output, AudioCue::BigFire, 1.0, volume);
                play(output, AudioCue::BigFire, 0.8, volume);
            }
        }
    }
}

fn key(common: &PrimarySpellTransientBase) -> ActorKey {
    (common.world_key.clone(), common.id)
}

fn earth_impact(output: &mut CombatAudioOutput, charge: f64, volume: f64) {
    play(output, AudioCue::RockHit, native_earth_boulder_rock_hit_pitch(charge), volume * charge);
    play(output, AudioCue::StoneBreak, native_earth_boulder_stone_break_pitch(charge), volume);
}

/// `primary-spell-ether-native.ts::nativeEtherImpactPitch`: a presentation-only hash of
/// the accepted actor ID. It consumes no world entropy and narrows at the source fround.
#[must_use]
pub fn native_ether_impact_pitch(id: u64) -> f64 {
    let mut value = (id as u32).wrapping_mul(0x9e3779b1) ^ 0xc2b2ae35;
    value = (value ^ (value >> 16)).wrapping_mul(0x21f0aaad);
    value = (value ^ (value >> 15)).wrapping_mul(0x735a2d97);
    let unit = f64::from(value ^ (value >> 15)) / 4_294_967_296.0;
    fround(1.0 + unit * f64::from(0.1_f32))
}
