//! Signed recovery Hello admission from `host/game-host.ts:1051..1708`. A portable
//! seed restores one real world; later members import only their retained participant.

use solomon_assets::website_save::{self, SaveIntegrity};
use solomon_protocol::hosted::{GameSaveIntent, HostedHello};
use solomon_protocol::{GameCommand, HostErrorCode, ResumeGraceReason, ServerMessage};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::social::GameSessionKind;

use super::admission::same_mods;
use super::{HostedAdmission, HostedGameHost, HostedPlayer, HostedWorlds};
use crate::host::authority::{encode, send_reliable};
use crate::host::connection::HostConnection;
use crate::host::recovery::{decode_party_recovery_claim, verify_party_recovery_claim};
use crate::host::signing::{credentials_equal, random_party_identity, random_token};

type AdmissionError = (HostErrorCode, String);

impl HostedGameHost {
    pub(in crate::host) fn admit_recovery(
        &mut self,
        connection: &mut HostConnection,
        hello: HostedHello,
        admission: HostedAdmission,
        now_ms: f64,
    ) -> Result<String, AdmissionError> {
        let token = admission
            .party_rejoin_token
            .as_deref()
            .ok_or_else(|| authentication("The party recovery seed is missing."))?;
        let claim = decode_party_recovery_claim(&self.recovery_secret, token)
            .ok_or_else(|| authentication("That party recovery claim is invalid."))?;
        let slot_index = if admission.party_recovery_seed {
            None
        } else {
            Some(
                self.valid_rejoin_reservation(token, admission.reservation_id.as_deref(), now_ms)
                    .ok_or_else(|| authentication("That active-party rejoin has expired."))?,
            )
        };
        let participant_count = if slot_index.is_some() {
            self.worlds.participant_count()
        } else {
            self.capacity_participant_count()
        };
        if participant_count >= self.maximum_players {
            return Err((HostErrorCode::HostFull, "The session is full.".into()));
        }
        let Some(document) = hello.save.as_deref() else {
            return Err(invalid("Active-party rejoin requires the saved wizard."));
        };
        if hello.save_intent != Some(GameSaveIntent::Resume) {
            return Err(invalid("Active-party rejoin requires the saved wizard."));
        }
        let profile = website_save::restore_profile(document).map_err(invalid_owned)?;
        let verified = verify_party_recovery_claim(&self.recovery_secret, token, document)
            .filter(|verified| {
                verified.player_id == claim.player_id
                    && verified.recovery_id == claim.recovery_id
                    && verified.run_id == claim.run_id
                    && verified.content_manifest_sha256
                        == admission.content.manifest.manifest_sha256
                    && verified.leaderboard_user_id == admission.leaderboard_user_id
                    && verified.session_kind == self.kind
                    && verified.integrity == profile.metadata.integrity
                    && (!admission.party_recovery_seed || verified.target_revision.is_some())
            })
            .ok_or_else(|| invalid("The saved party recovery claim is invalid."))?;
        if self.kind == GameSessionKind::GlobalHub
            && profile.metadata.integrity == SaveIntegrity::LocalOnly
        {
            return Err(invalid("Local-only saves require a private College."));
        }
        if !same_mods(&profile.metadata.mods, &admission.content.manifest.mods) {
            return Err(invalid(
                "The saved mod list does not match this session. Confirm the mismatch before loading it.",
            ));
        }
        let restored = website_save::restore(document).map_err(invalid_owned)?;
        if restored
            .session
            .participants
            .get(&restored.player_id)
            .expect("participant exists")
            .player
            .selection
            != hello.selection
        {
            return Err(invalid("The game save character does not match the resume request."));
        }
        if restored.player_id != verified.player_id
            || restored.loaded_boneyard.as_ref().map(|loaded| loaded.run_id.as_str())
                != Some(verified.run_id.as_str())
            || restored.session.run.phase != GameRunPhase::Active
            || restored.session.run.run_id.as_deref() != Some(verified.run_id.as_str())
            || profile
                .continuation
                .as_ref()
                .and_then(|continuation| continuation.summary.party_rejoin_token.as_deref())
                .is_none_or(|saved| !credentials_equal(saved, token))
        {
            return Err(invalid("The saved wizard does not match this active-party rejoin."));
        }
        let identity = random_party_identity().map_err(server_error)?;
        let resume_token = random_token::<32>().map_err(server_error)?;
        let new_reference = format!("player-ref-{}", random_token::<24>().map_err(server_error)?);
        let id = verified.player_id.clone();
        let party_id;
        let mut staged = false;
        if let Some(index) = slot_index {
            if self.rejoin_slots[index].detached.is_none() {
                self.rejoin_slots[index].detached =
                    Some(restored.session.detach_participant(&id).map_err(invalid_owned)?);
                self.rejoin_slots[index].profile = hello.profile.clone();
            }
            if self.rejoin_slots[index]
                .detached
                .as_ref()
                .is_none_or(|detached| detached.participant.player.selection != hello.selection)
            {
                return Err(invalid("The saved wizard does not match this active-party rejoin."));
            }
            self.rejoin_slots[index].party_identity = Some(identity);
            self.synchronize_rejoins().map_err(server_error)?;
            party_id = self.rejoin_slots[index].party_id.clone();
            if self.rejoin_slots[index]
                .detached
                .as_ref()
                .unwrap()
                .participant
                .player
                .progression
                .pending_offer
                .is_some()
            {
                self.rejoin_slots[index].connected = true;
                self.rejoin_slots[index].reservation = None;
                staged = true;
            } else {
                self.materialize_rejoin(index, admission.reservation_id.as_deref())
                    .map_err(invalid_owned)?;
            }
        } else {
            let navigation = self
                .content_loader
                .load_navigation(restored.loaded_boneyard.as_ref().unwrap())
                .map_err(server_error)?;
            self.worlds
                .restore_player(restored.session, restored.loaded_boneyard, &id, identity)
                .map_err(invalid_owned)?;
            party_id = self.party_id_for_player(&id).unwrap().to_owned();
            let members: Vec<_> =
                verified.party_roster.iter().map(|member| member.player_id.clone()).collect();
            if let Err(reason) = self.worlds.parties_mut().restore_membership(
                &id,
                &members,
                &verified.party_leader_player_id,
                verified.party_visibility,
            ) {
                self.worlds.remove_player(&id).map_err(server_error)?;
                return Err(invalid_owned(reason));
            }
            self.navigation.insert(party_id.clone(), navigation);
            self.register_recovery_lineage(&verified, party_id.clone(), admission.content.clone());
            if self.kind == GameSessionKind::PrivateCollege
                && let Some(number) = id
                    .strip_prefix("player-")
                    .and_then(|number| number.parse::<u64>().ok())
                    .filter(|number| *number <= 9_007_199_254_740_991)
            {
                self.next_player_id = self.next_player_id.max(number + 1);
            }
        }
        let slot = slot_index.map(|index| &self.rejoin_slots[index]);
        let player_reference =
            slot.and_then(|slot| slot.player_reference.clone()).unwrap_or(new_reference);
        let mut metadata = profile.metadata;
        metadata.integrity = if slot
            .map_or(verified.integrity == SaveIntegrity::LocalOnly, |slot| slot.local_only)
        {
            SaveIntegrity::LocalOnly
        } else {
            SaveIntegrity::GlobalClean
        };
        self.players.push(HostedPlayer {
            id: id.clone(),
            resume_token,
            content: admission.content,
            peer_id: connection.peer.id(),
            player_reference,
            social_connection: None,
            profile: slot.map_or(hello.profile, |slot| slot.profile.clone()),
            preferences: hello.online_preferences,
            developer_access: admission.developer_access,
            leaderboard_user_id: slot
                .map_or(verified.leaderboard_user_id, |slot| slot.leaderboard_user_id),
            save_metadata: metadata,
            global_score_eligible: slot
                .map_or(verified.global_score_eligible, |slot| slot.global_score_eligible),
            tutorial_eligible: false,
            terminal_saved_run_id: None,
            last_save_sequence: 0,
            last_save_document: None,
        });
        if slot_index.is_none() {
            self.initialize_admitted_mod_world(&id, true).map_err(invalid_owned)?;
        }
        if !self.cheat_policy_initialized && self.kind == GameSessionKind::PrivateCollege {
            self.cheats_enabled = hello.cheats_enabled && !admission.developer_access;
            self.cheat_policy_initialized = true;
        }
        let facts = self.control_facts(&party_id);
        let control = self.controls.entry(party_id.clone()).or_default();
        if !staged {
            control.require_renderer(&id, &verified.run_id);
        }
        control.begin_loading_grace(
            &id,
            if slot_index.is_some() {
                ResumeGraceReason::GameRejoined
            } else {
                ResumeGraceReason::GameRestarted
            },
            &facts.connected,
            &[],
        );
        if !staged {
            self.arm_party_rejoins(&party_id).map_err(server_error)?;
        }
        connection.player_id = Some(id.clone());
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(id)
    }

    pub(in crate::host) fn materialize_rejoin(
        &mut self,
        index: usize,
        reservation_id: Option<&str>,
    ) -> Result<(), String> {
        let slot = &self.rejoin_slots[index];
        let detached = slot
            .detached
            .as_ref()
            .ok_or("The active-party rejoin is missing its detached wizard.")?
            .clone();
        let identity = slot
            .party_identity
            .clone()
            .ok_or("The active-party rejoin is missing its detached wizard.")?;
        let maximum_members = self.available_party_members_excluding(
            Some(&slot.party_id),
            reservation_id,
            Some(&slot.player_id),
        );
        match &mut self.worlds {
            HostedWorlds::Global(worlds) => worlds
                .rejoin_party_run_player(detached, &slot.party_id, identity, maximum_members, None)
                .map_err(|reason| reason.id().to_owned())?,
            HostedWorlds::Private { state, parties } => {
                let destination = parties
                    .parties
                    .iter()
                    .find(|party| party.id == slot.party_id)
                    .ok_or("The active party no longer exists.")?;
                let existing = parties.party_for_player(&slot.player_id);
                if existing.is_some_and(|party| party.id != destination.id) {
                    return Err("The returning wizard belongs to another party.".into());
                }
                let registered = existing.is_none();
                let next_parties = if registered {
                    let mut next = parties.clone();
                    next.register_player(&slot.player_id, identity);
                    next.join(&slot.player_id, &slot.party_id, maximum_members)
                        .map_err(|reason| reason.text().to_owned())?;
                    Some(next)
                } else {
                    None
                };
                state.rejoin_participant(detached, None)?;
                if let Some(next) = next_parties {
                    *parties = next;
                }
            }
        }
        let slot = &mut self.rejoin_slots[index];
        slot.connected = true;
        slot.detached = None;
        slot.reservation = None;
        Ok(())
    }

    pub(in crate::host) fn apply_staged_skill_command(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        sequence: u64,
        command: GameCommand,
        now_ms: f64,
    ) -> Result<(), AdmissionError> {
        let index = self.staged_rejoin_index(id).expect("the command selected a staged owner");
        let slot = &mut self.rejoin_slots[index];
        let state = match &mut self.worlds {
            HostedWorlds::Global(worlds) => worlds
                .runs
                .iter_mut()
                .find(|run| {
                    run.party_id == slot.party_id && run.loaded_boneyard.run_id == slot.run_id
                })
                .map(|run| &mut run.state),
            HostedWorlds::Private { state, .. } => Some(state),
        }
        .ok_or_else(|| invalid("The active party no longer exists."))?;
        let detached = slot.detached.as_mut().expect("the staged owner retains one participant");
        let (accepted, rejected) = match command {
            GameCommand::ChooseSkill { choice } => (
                state.choose_detached_skill(detached, choice).map_err(server_error)?,
                "The skill choice is stale or not in this offer.",
            ),
            GameCommand::RerollSkillOffer { offer_sequence } => (
                state
                    .reroll_detached_skill_offer(detached, offer_sequence)
                    .map_err(server_error)?,
                "The level-up action is stale or unavailable.",
            ),
            GameCommand::DeferSkillChoice { offer_sequence } => (
                state
                    .defer_detached_skill_choice(detached, offer_sequence)
                    .map_err(server_error)?,
                "The level-up action is stale or unavailable.",
            ),
            _ => return Ok(()),
        };
        if !accepted {
            return Err(invalid(rejected));
        }
        let completed = detached.participant.player.progression.pending_offer.is_none();
        let party_id = slot.party_id.clone();
        let run_id = slot.run_id.clone();
        if completed {
            self.materialize_rejoin(index, None).map_err(invalid_owned)?;
            self.arm_party_rejoins(&party_id).map_err(server_error)?;
            let facts = self.control_facts(&party_id);
            let control = self.controls.entry(party_id.clone()).or_default();
            control.require_renderer(id, &run_id);
            control.advance(&facts.context(), now_ms);
            self.pending_player_saves.insert(id.into());
        }
        let receipt = encode(&ServerMessage::<()>::CommandResult {
            sequence,
            snapshot_sequence: self.next_snapshot_sequence,
            accepted: true,
            reason: None,
            inventory_result: None,
        })
        .map_err(server_error)?;
        connection.last_command_sequence = sequence;
        connection.last_command_receipt = Some(receipt.clone());
        send_reliable(&connection.peer, receipt).map_err(server_error)?;
        connection.input.neutralize();
        self.neutralize_party(&party_id);
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(())
    }
}

fn authentication(reason: &str) -> AdmissionError {
    (HostErrorCode::InvalidCredentials, reason.into())
}
fn invalid(reason: &str) -> AdmissionError {
    (HostErrorCode::InvalidMessage, reason.into())
}
fn invalid_owned(reason: String) -> AdmissionError {
    (HostErrorCode::InvalidMessage, reason)
}
fn server_error(reason: String) -> AdmissionError {
    (HostErrorCode::ServerError, reason)
}
