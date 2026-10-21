//! Durable native roster slots composed with canonical participant import/retirement.
//! Source counterpart: `game-host.ts::detachPartyRejoinClient` and
//! `synchronizePartyRejoinMilestones`; the host never rebuilds a player's skill/economy state.

use std::collections::BTreeSet;

use solomon_protocol::ResumeGraceReason;
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::session::GameSession;

use super::authority::NativeAuthority;
use super::connection::HostConnection;
use super::persistence::SavedParticipant;

impl NativeAuthority {
    pub(super) fn validate_restored_membership(&self) -> Result<(), String> {
        if self.metadata.resume_slots.len() > self.maximum_players {
            return Err("The saved native roster exceeds this host's player capacity".into());
        }
        if self.metadata.next_player_id == 0 || self.metadata.next_chat_sequence == 0 {
            return Err("The saved native host has an invalid identity/sequence cursor".into());
        }
        let slots: BTreeSet<_> = self.metadata.resume_slots.keys().cloned().collect();
        let mut members = BTreeSet::new();
        if let Some(party) = &self.metadata.party {
            members = party.member_player_ids.iter().cloned().collect();
            if !members.is_subset(&slots)
                || members.len() != party.member_player_ids.len()
                || !members.contains(&party.leader_player_id)
                || party.id != self.metadata.session_id
                || party.join_code.is_some()
                || party.listing_id.is_some()
            {
                return Err("The saved native party and retained wizard slots disagree".into());
            }
        }
        let Some(session) = &self.session else {
            if !slots.is_empty() {
                return Err("The saved native wizard slots have no shared session".into());
            }
            return Ok(());
        };
        for id in &session.participant_order {
            if !members.contains(id) {
                return Err("A saved live participant is outside its native party".into());
            }
            let slot = self
                .metadata
                .resume_slots
                .get(id)
                .ok_or_else(|| format!("The saved participant {id} has no resume slot"))?;
            if slot.saved.is_some() {
                return Err(format!(
                    "The saved participant {id} is both materialized and detached"
                ));
            }
        }
        for (id, slot) in &self.metadata.resume_slots {
            if slot.token.len() != 64 || !slot.token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("A saved native resume credential is invalid".into());
            }
            let retained_id = match &slot.saved {
                Some(SavedParticipant::Hub { participant }) => Some(participant.id.as_str()),
                Some(SavedParticipant::Run { detached }) => Some(detached.participant.id.as_str()),
                Some(SavedParticipant::Retired { participant, .. }) => {
                    Some(participant.id.as_str())
                }
                None => session.participant(id).map(|participant| participant.id.as_str()),
            };
            if retained_id != Some(id.as_str()) {
                return Err(format!(
                    "The saved resume slot {id} has no matching actual participant"
                ));
            }
            solomon_assets::save_document::validate_participant(
                self.retained_participant(id).expect("the slot identity was validated above"),
            )?;
            if !members.contains(id)
                && !matches!(slot.saved, Some(SavedParticipant::Retired { .. }))
            {
                return Err("A departed native member must retain only its retired profile".into());
            }
        }
        Ok(())
    }

    pub(super) fn detach_restored_connections(&mut self) -> Result<(), String> {
        let Some(session) = &self.session else { return Ok(()) };
        let ids = session.participant_order.clone();
        // A restarted process has lost every old socket. Apply the same canonical
        // disconnect/removal transaction once, then let actual credentials re-admit it.
        for id in ids {
            self.capture_offline_participant(&id)?;
        }
        if !self.metadata.resume_slots.is_empty() {
            self.retire_empty_world()?;
            self.changed();
            self.require_checkpoint();
        }
        Ok(())
    }

    pub(super) fn disconnected(
        &mut self,
        connection: HostConnection,
        now_ms: f64,
    ) -> Result<(), String> {
        self.pending_save_acks.remove(&connection.peer.id());
        let Some(id) = connection.player_id else { return Ok(()) };
        let Some(slot) = self.metadata.resume_slots.get(&id) else { return Ok(()) };
        if slot.peer_id != Some(connection.peer.id()) {
            return Ok(());
        }
        let wait_for_living_player = self.disconnected_player_is_last_living(&id);
        self.metadata
            .resume_slots
            .get_mut(&id)
            .expect("the current connection owns its slot")
            .peer_id = None;
        if wait_for_living_player {
            let connected = self.connected_ids();
            self.control.begin_loading_grace(
                &id,
                ResumeGraceReason::PartyRejoinWait,
                &connected,
                std::slice::from_ref(&id),
            );
            self.neutralize_inputs();
        }
        if self.session.as_ref().is_some_and(|session| session.participant(&id).is_some()) {
            self.capture_offline_participant(&id)?;
        }
        self.retire_empty_world()?;
        self.changed();
        self.require_checkpoint();
        let facts = self.control_facts();
        self.control.disconnected(&id, &facts.context(), now_ms);
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(())
    }

    fn capture_offline_participant(&mut self, id: &str) -> Result<(), String> {
        let session = self.session.as_mut().ok_or("The retained participant has no session")?;
        let participant =
            session.participant(id).ok_or("The disconnecting participant is absent")?;
        if session.boneyard.as_ref().is_some_and(|world| world.tutorial.is_some()) {
            // Tutorial authority/checkpoints require its one real participant. Removing
            // the final socket's actor produces the pinned JS host's zero-player
            // assertion. Retain this private actor and freeze its world until resume.
            return Ok(());
        }
        let saved = if session.run.phase == GameRunPhase::Active && session.boneyard.is_some() {
            SavedParticipant::Run { detached: Box::new(session.detach_participant(id)?) }
        } else if session.run.phase == GameRunPhase::Hub && session.boneyard.is_none() {
            SavedParticipant::Hub { participant: Box::new(participant.clone()) }
        } else {
            let profile = session
                .retired_participant_profile(id)
                .ok_or("The retired participant has no durable profile")?;
            SavedParticipant::Retired {
                participant: Box::new(participant.clone()),
                economy: Box::new(profile.economy),
                hagatha_runtime: profile.hagatha_runtime,
            }
        };
        if !session.remove_participant(id) {
            return Err(
                "The captured participant could not be removed from the shared session".into()
            );
        }
        let slot = self
            .metadata
            .resume_slots
            .get_mut(id)
            .ok_or("The captured participant has no durable slot")?;
        slot.saved = Some(saved);
        slot.peer_id = None;
        Ok(())
    }

    fn disconnected_player_is_last_living(&self, id: &str) -> bool {
        let Some(session) = &self.session else { return false };
        if session.boneyard.is_none() || session.run.phase != GameRunPhase::Active {
            return false;
        }
        let living =
            |state| matches!(state, PlayerLifeState::Alive | PlayerLifeState::LethalPending);
        let Some(participant) = session.participant(id) else { return false };
        if !living(participant.player.combat.life_state) {
            return false;
        }
        let remaining_living = session.participants.iter().any(|(other, participant)| {
            other != id && living(participant.player.combat.life_state)
        });
        let remaining_connected = self.metadata.resume_slots.iter().any(|(other, slot)| {
            other != id && slot.peer_id.is_some() && session.participant(other).is_some()
        });
        !remaining_living && remaining_connected
    }

    pub(super) fn synchronize_offline_milestones(&mut self) -> Result<(), String> {
        let Some(session) = &mut self.session else { return Ok(()) };
        // BTreeMap preserves the source's player-ID-sorted transaction order. These
        // offline offer draws occur now, in the shared RNG stream, rather than on rejoin.
        for slot in self.metadata.resume_slots.values_mut() {
            let Some(SavedParticipant::Run { detached }) = &mut slot.saved else { continue };
            if let Some(milestone) = session.rejoin_milestone(detached) {
                session.synchronize_detached(detached, &milestone)?;
            }
        }
        Ok(())
    }

    pub(super) fn retire_offline_run_slots(&mut self) -> Result<(), String> {
        let Some(session) = &self.session else { return Ok(()) };
        for slot in self.metadata.resume_slots.values_mut() {
            let Some(SavedParticipant::Run { detached }) = &slot.saved else { continue };
            if session.run.phase == GameRunPhase::Active
                && session.run.run_id.as_deref() == Some(&detached.run_id)
            {
                continue;
            }
            let profile = GameSession::retired_detached_profile(detached);
            slot.saved = Some(SavedParticipant::Retired {
                participant: Box::new(detached.participant.clone()),
                economy: Box::new(profile.economy),
                hagatha_runtime: profile.hagatha_runtime,
            });
        }
        Ok(())
    }

    /// A disconnected living run keeps its exact world. A terminal or deliberately
    /// abandoned party has only retired profiles and returns through the canonical Hub
    /// reset, so those durable profiles remain admissible after a launcher restart.
    pub(super) fn retire_empty_world(&mut self) -> Result<(), String> {
        let Some(session) = &self.session else { return Ok(()) };
        if !session.participants.is_empty() {
            return Ok(());
        }
        let live_detached_run = session.run.phase == GameRunPhase::Active
            && self.metadata.resume_slots.values().any(|slot| {
                matches!(&slot.saved,
                Some(SavedParticipant::Run { detached })
                    if session.run.run_id.as_deref() == Some(detached.run_id.as_str()))
            });
        if live_detached_run
            || (session.boneyard.is_none() && session.run.phase == GameRunPhase::Hub)
        {
            return Ok(());
        }
        self.retire_offline_run_slots()?;
        self.session.as_mut().expect("the empty world is retained").return_to_hub();
        self.navigation = None;
        self.control = super::control::GameplayControl::default();
        Ok(())
    }
}
