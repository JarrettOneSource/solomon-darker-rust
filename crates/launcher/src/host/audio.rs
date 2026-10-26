//! Per-listener combat one-shots over the accepted session. Camera attenuation follows
//! `BoneyardScene.tsx`; clients retain snapshot-owned loops and narration mixes.

use solomon_protocol::ServerMessage;
use solomon_sim::Vec2;
use solomon_sim::audio::combat::{
    BONEYARD_SOLOMON_DIALOGUE_CUES, CombatAudioCursor, CombatAudioFrame, CombatAudioPlayer,
};
use solomon_sim::audio::{AudioCue, AudioEvent};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::session::GameSession;
use solomon_sim::spells::model::PrimaryTransientRef;
use solomon_sim::spells::staff::StaffTransientRef;

use super::authority::{NativeAuthority, send_control};
use super::connection::HostConnection;

#[derive(Clone, Copy)]
pub struct ListenerView {
    pub center: Vec2,
    pub world_width: f64,
}

impl ListenerView {
    pub fn new(center: Vec2, world_width: f64) -> Result<Self, &'static str> {
        if !center.x.is_finite()
            || !center.y.is_finite()
            || !world_width.is_finite()
            || !(0.0..=65_536.0).contains(&world_width)
            || world_width == 0.0
        {
            return Err("The listener camera must be finite with a positive supported width");
        }
        Ok(Self { center, world_width })
    }
}

impl NativeAuthority {
    pub(super) fn attach_combat_audio(&self, connection: &mut HostConnection) {
        let Some(session) = &self.session else { return };
        // Capturing the current frame suppresses retained historical one-shots. The
        // first real camera arrives after Welcome, before its renderer-ready ACK.
        project_listener_audio(session, connection, &[], true);
    }

    pub(super) fn publish_tick_audio(
        &mut self,
        accepted_events: &[AudioEvent],
    ) -> Result<(), String> {
        let Some(session) = &self.session else { return Ok(()) };
        for connection in self.connections.values_mut() {
            if connection.player_id.is_none() || connection.welcome_revision.is_some() {
                continue;
            }
            let events = project_listener_audio(session, connection, accepted_events, false);
            if !events.is_empty() {
                send_control(
                    &connection.peer,
                    ServerMessage::Audio { tick: session.ticks, events },
                )?;
            }
        }
        Ok(())
    }
}

pub(super) fn project_listener_audio(
    session: &GameSession,
    connection: &mut HostConnection,
    accepted_events: &[AudioEvent],
    attach_only: bool,
) -> Vec<AudioEvent> {
    let mut events = Vec::with_capacity(accepted_events.len());
    for event in accepted_events {
        if matches!(event, AudioEvent::Play(AudioCue::SolomonLaughBig, _)) {
            for cue in BONEYARD_SOLOMON_DIALOGUE_CUES {
                events.push(AudioEvent::StopStream(cue));
            }
        }
        events.push(*event);
    }
    let (Some(id), Some(view)) = (&connection.player_id, connection.listener_view) else {
        return events;
    };
    let Some(listener) = session.participant(id) else { return events };
    // These small borrowed rows contain every real participant. The world, spells,
    // enemy queues and retained histories are never cloned for a listener or frame.
    let mut world_keys = Vec::with_capacity(session.participant_order.len());
    for player_id in &session.participant_order {
        world_keys.push(session.world_key(player_id).expect("the ordered participant exists"));
    }
    let mut players = Vec::with_capacity(world_keys.len());
    for (player_id, world_key) in session.participant_order.iter().zip(&world_keys) {
        let participant = &session.participants[player_id];
        let wizard = &participant.wizard;
        players.push(CombatAudioPlayer {
            id: player_id,
            world_key,
            position: wizard.position,
            element: participant.player.selection.element,
            selected_primary_skill_id: participant.player.book.primary_skill_id as u32,
            weld_build_id: participant.player.book.weld_build_id,
            cast: &wizard.primary_cast,
            footstep_tick: wizard.footstep_tick,
        });
    }
    let mut staff_contacts = Vec::new();
    for transient in session.spells.transients.iter() {
        if let PrimaryTransientRef::Staff(StaffTransientRef::Contact(contact)) = transient {
            staff_contacts.push(contact.clone());
        }
    }
    let alternate = listener.player.combat.life_state != PlayerLifeState::Alive;
    let point_gain = |point| {
        solomon_sim::region_feedback::native_region_point_gain(
            point,
            view.center,
            view.world_width,
            alternate,
        )
    };
    let hit_gain = |point| {
        solomon_sim::region_feedback::native_region_hit_point_gain(
            point,
            view.center,
            view.world_width,
            alternate,
        )
    };
    let world = session.boneyard.as_ref();
    let frame = CombatAudioFrame {
        tick: session.ticks,
        run_id: world.map(|world| world.loaded.run_id.as_str()),
        listener_id: id,
        players: &players,
        primary: &session.spells,
        secondary: &session.secondary,
        staff_contacts: &staff_contacts,
        enemies: world.map(|world| &world.enemies),
        enemy_events: world.map_or(&[], |world| world.enemy_events.as_slice()),
        loot_events: world.map_or(&[], |world| world.loot_events.as_slice()),
        encounter: world.and_then(|world| world.encounter.as_ref()),
        point_gain: &point_gain,
        hit_gain: &hit_gain,
    };
    let cursor = connection.combat_audio.get_or_insert_with(|| CombatAudioCursor::new(&frame));
    if !attach_only {
        // The canonical cursor owns cue counters; the browser projects its loops and
        // narration mix from snapshots and must not replay these reliable one-shots.
        events.extend(cursor.update(&frame).events);
    }
    events
}
