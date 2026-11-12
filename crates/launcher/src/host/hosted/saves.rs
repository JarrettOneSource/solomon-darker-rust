//! Canonical hosted save capture and publication. The authority owns identities and captures;
//! autonomous document encoding and transfer preparation run in the Supervisor's single worker.

use std::borrow::Cow;
use std::sync::Arc;

use solomon_assets::website_save::{self, CreateSaveOptions, SaveIntegrity, SaveMetadata};
use solomon_protocol::hosted::SaveCheckpointReason;
use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::session::GameSession;
use solomon_sim::social::PartyVisibility;

use super::{HostedGameHost, HostedWorlds};
use crate::host::connection::HostConnection;
use crate::host::recovery::{PartyRecoveryClaim, create_owner_party_recovery_claim};
use crate::transport::{PeerId, PreparedSaveCheckpoint};

/// Forced publication options, including deployment's target revision.
pub(in crate::host) struct PlayerSaveRequest<'a> {
    pub target_revision: Option<&'a str>,
    pub force: bool,
    pub include_terminal_profile: bool,
}

/// Plain values from one authority observation. Forced saves can borrow the world;
/// the autonomous worker receives the same input with all borrows frozen into ownership.
struct OwnerSaveInput<'a> {
    state: Cow<'a, GameSession>,
    loaded: Option<Cow<'a, LoadedBoneyard>>,
    metadata: SaveMetadata,
    player_id: String,
    recovery: Option<(PartyRecoveryClaim, Cow<'a, str>)>,
    #[cfg(test)]
    gate: Option<Arc<SaveEncodeGate>>,
}

impl OwnerSaveInput<'_> {
    fn into_owned(self) -> OwnerSaveInput<'static> {
        OwnerSaveInput {
            state: Cow::Owned(self.state.into_owned()),
            loaded: self.loaded.map(|loaded| Cow::Owned(loaded.into_owned())),
            metadata: self.metadata,
            player_id: self.player_id,
            recovery: self.recovery.map(|(claim, secret)| (claim, Cow::Owned(secret.into_owned()))),
            #[cfg(test)]
            gate: self.gate,
        }
    }

    fn encode(self) -> Result<(String, Option<String>), String> {
        #[cfg(test)]
        if let Some(gate) = &self.gate {
            gate.wait()?;
        }
        if terminal(self.state.run.phase) {
            return website_save::create_profile(&self.state, &self.player_id, &self.metadata)
                .map(|document| (document, None));
        }
        let options = CreateSaveOptions {
            state: &self.state,
            player_id: &self.player_id,
            loaded_boneyard: self.loaded.as_deref(),
            metadata: &self.metadata,
            party_rejoin_token: None,
        };
        let Some((claim, secret)) = self.recovery else {
            return Ok((website_save::create(options)?, None));
        };
        let (document, token) = website_save::create_with_party_recovery(options, |created| {
            create_owner_party_recovery_claim(&secret, &claim, created)
        })?;
        Ok((document, Some(token)))
    }
}

#[derive(PartialEq, Eq)]
struct PlayerSaveIdentity {
    player_id: String,
    peer_id: PeerId,
    player_reference: String,
    resume_token: String,
    run_id: Option<String>,
    phase: GameRunPhase,
    party_id: Option<String>,
    recovery_id: Option<String>,
    party_members: Vec<String>,
    party_leader: Option<String>,
    party_visibility: Option<PartyVisibility>,
    staged: bool,
    content_manifest_sha256: String,
    integrity: SaveIntegrity,
    global_score_eligible: bool,
    leaderboard_user_id: Option<u32>,
    last_save_sequence: u64,
}

pub(in crate::host) struct SaveJob {
    pub college_id: String,
    instance: Arc<()>,
    identity: PlayerSaveIdentity,
    input: OwnerSaveInput<'static>,
    atomic: bool,
}

pub(in crate::host) struct SaveCompletion {
    pub college_id: String,
    instance: Arc<()>,
    identity: PlayerSaveIdentity,
    result: Result<EncodedSave, SaveEncodeFailure>,
}

struct EncodedSave {
    document: String,
    rejoin_token: Option<String>,
    prepared: PreparedSaveCheckpoint,
}

enum SaveEncodeFailure {
    Creation(String),
    Transport(String),
}

impl SaveJob {
    pub fn encode(self) -> SaveCompletion {
        let result = self.input.encode().map_err(SaveEncodeFailure::Creation).and_then(
            |(document, rejoin_token)| {
                let reason = if terminal(self.identity.phase) {
                    SaveCheckpointReason::GameOver
                } else {
                    SaveCheckpointReason::Progress
                };
                // Both the transport's full copy/UTF-16 conversion and the comparison cache's
                // owned document are prepared here. Admission only moves these values.
                let prepared = PreparedSaveCheckpoint::new(
                    document.clone(),
                    reason,
                    self.identity.last_save_sequence + 1,
                    self.atomic,
                )
                .map_err(SaveEncodeFailure::Transport)?;
                Ok(EncodedSave { document, rejoin_token, prepared })
            },
        );
        SaveCompletion {
            college_id: self.college_id,
            instance: self.instance,
            identity: self.identity,
            result,
        }
    }
}

fn terminal(phase: GameRunPhase) -> bool {
    matches!(phase, GameRunPhase::GameOver | GameRunPhase::Loadout)
}

impl HostedGameHost {
    fn owner_save_input(
        &self,
        id: &str,
        target_revision: Option<&str>,
    ) -> Result<OwnerSaveInput<'_>, String> {
        let player = self.player(id).ok_or("The saved player has no admitted identity")?;
        let mut metadata = player.save_metadata.clone();
        metadata.mod_state = self.prepared_mod_save_state(id)?;
        let staged = self.staged_rejoin_index(id).map(|index| &self.rejoin_slots[index]);
        let state = if let Some(slot) = staged {
            let active = self
                .active_run(&slot.party_id, &slot.run_id)
                .ok_or("The staged saved wizard lost its active run")?;
            let mut state = active.clone();
            state.rejoin_participant(slot.detached.as_ref().unwrap().clone(), None)?;
            Cow::Owned(state)
        } else {
            match &self.worlds {
                HostedWorlds::Global(worlds) => worlds
                    .party_save_state_for_player(id)?
                    .ok_or("The saved player has no actual party world")?,
                HostedWorlds::Private { state, .. } => Cow::Borrowed(state),
            }
        };
        let loaded = self
            .worlds
            .loaded_for_player(id)
            .or_else(|| self.worlds.loaded_for_run(&staged?.run_id));
        let lineage = self.recovery_lineages.iter().find(|lineage| {
            state.run.run_id.as_ref() == Some(&lineage.run_id)
                && self.party_id_for_player(id) == Some(&lineage.party_id)
        });
        let recovery = if !terminal(state.run.phase)
            && let Some(lineage) = lineage
        {
            let party = self
                .worlds
                .parties()
                .parties
                .iter()
                .find(|party| party.id == lineage.party_id)
                .ok_or("The recovery lineage lost its party membership")?;
            Some((
                PartyRecoveryClaim {
                    content_manifest_sha256: lineage.content.manifest.manifest_sha256.clone(),
                    global_score_eligible: player.global_score_eligible,
                    integrity: player.save_metadata.integrity,
                    leaderboard_user_id: player.leaderboard_user_id,
                    party_member_count: lineage.party_member_count,
                    party_leader_player_id: party.leader_player_id.clone(),
                    party_roster: self.recovery_roster(&party.id)?,
                    party_visibility: party.visibility,
                    player_id: id.into(),
                    recovery_id: lineage.recovery_id.clone(),
                    run_id: lineage.run_id.clone(),
                    session_kind: self.kind,
                    target_revision: Some(
                        target_revision.unwrap_or(&self.recovery_revision).into(),
                    ),
                },
                Cow::Borrowed(self.recovery_secret.as_str()),
            ))
        } else {
            None
        };
        Ok(OwnerSaveInput {
            state,
            loaded: loaded.map(Cow::Borrowed),
            metadata,
            player_id: id.into(),
            recovery,
            #[cfg(test)]
            gate: self.save_encode_gate.clone(),
        })
    }

    pub(in crate::host) fn owner_save(
        &self,
        id: &str,
        target_revision: Option<&str>,
    ) -> Result<(String, Option<String>), String> {
        self.owner_save_input(id, target_revision)?.encode()
    }

    pub(in crate::host) fn publish_player_save(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        request: PlayerSaveRequest<'_>,
    ) -> Result<u64, String> {
        if self.editor_test {
            return Ok(0);
        }
        let atomic =
            connection.initial_save_pending || request.force || request.include_terminal_profile;
        let terminal = self.worlds.state_for_player(id).is_some_and(|state| {
            matches!(state.run.phase, GameRunPhase::GameOver | GameRunPhase::Loadout)
        });
        if terminal && !request.include_terminal_profile {
            return Ok(0);
        }
        if self.staged_rejoin_index(id).is_some_and(|index| {
            let slot = &self.rejoin_slots[index];
            self.active_run(&slot.party_id, &slot.run_id).is_none()
        }) {
            return Ok(0);
        }
        let (save, rejoin_token) = match self.owner_save(id, request.target_revision) {
            Ok(save) => save,
            Err(reason) => {
                // A failed checkpoint is logged and reports sequence 0; it is not fatal to the
                // tick. Errors after the document is built still fail the host.
                eprintln!("save.checkpoint_failed player={id}: {reason}");
                return Ok(0);
            }
        };
        let player = self.player_mut(id).ok_or("The saved wizard is no longer connected")?;
        if !request.force && player.last_save_document.as_ref() == Some(&save) {
            connection.initial_save_pending = false;
            return Ok(player.last_save_sequence);
        }
        player.last_save_sequence += 1;
        let sequence = player.last_save_sequence;
        player.last_save_document = Some(save.clone());
        // Creation already returned the token written into this document. Re-reading
        // the entire continuation here would repeat save validation on the tick owner.
        if let Some(token) = rejoin_token
            && let Some(slot) = self.rejoin_slots.iter_mut().find(|slot| slot.player_id == id)
        {
            slot.token = Some(token);
        }
        let reason =
            if terminal { SaveCheckpointReason::GameOver } else { SaveCheckpointReason::Progress };
        connection.peer.publish_save_checkpoint(save, reason, sequence, atomic)?;
        connection.initial_save_pending = false;
        Ok(sequence)
    }

    fn save_identity(&self, id: &str) -> Option<PlayerSaveIdentity> {
        let player = self.player(id)?;
        let connection = self.connections.get(&player.peer_id)?;
        if connection.peer.is_closed() {
            return None;
        }
        let staged = self.staged_rejoin_index(id).map(|index| &self.rejoin_slots[index]);
        let state = if let Some(slot) = staged {
            self.active_run(&slot.party_id, &slot.run_id)?
        } else {
            self.worlds.state_for_player(id)?
        };
        let party = self.worlds.parties().party_for_player(id);
        let party_id = party.map(|party| party.id.clone());
        let recovery_id = self
            .recovery_lineages
            .iter()
            .find(|lineage| {
                Some(&lineage.party_id) == party_id.as_ref()
                    && Some(&lineage.run_id) == state.run.run_id.as_ref()
            })
            .map(|lineage| lineage.recovery_id.clone());
        Some(PlayerSaveIdentity {
            player_id: id.into(),
            peer_id: player.peer_id,
            player_reference: player.player_reference.clone(),
            resume_token: player.resume_token.clone(),
            run_id: state.run.run_id.clone(),
            phase: state.run.phase,
            party_id,
            recovery_id,
            party_members: party.map(|party| party.member_player_ids.clone()).unwrap_or_default(),
            party_leader: party.map(|party| party.leader_player_id.clone()),
            party_visibility: party.map(|party| party.visibility),
            staged: staged.is_some(),
            content_manifest_sha256: player.content.manifest.manifest_sha256.clone(),
            integrity: player.save_metadata.integrity,
            global_score_eligible: player.global_score_eligible,
            leaderboard_user_id: player.leaderboard_user_id,
            last_save_sequence: player.last_save_sequence,
        })
    }

    pub(in crate::host) fn request_autonomous_saves(&mut self, autosave: bool) {
        if self.editor_test {
            return;
        }
        for player in &self.players {
            let Some(connection) = self.connections.get(&player.peer_id) else { continue };
            if connection.peer.is_closed() {
                continue;
            }
            let Some(state) = self.worlds.state_for_player(&player.id) else { continue };
            let terminal_pending = terminal(state.run.phase)
                && (player.terminal_saved_run_id != state.run.run_id
                    || player.last_save_document.is_none());
            let in_flight = self.autonomous_save_player.as_ref() == Some(&player.id);
            if (autosave && !terminal(state.run.phase))
                || (!in_flight && (terminal_pending || connection.initial_save_pending))
            {
                self.pending_player_saves.insert(player.id.clone());
            }
        }
    }

    pub(in crate::host) fn capture_pending_save(&mut self) -> Option<SaveJob> {
        if self.editor_test || self.players.is_empty() || self.autonomous_save_player.is_some() {
            return None;
        }
        let count = self.players.len();
        for offset in 0..count {
            let index = (self.save_player_cursor + offset) % count;
            let id = self.players[index].id.clone();
            if !self.pending_player_saves.contains(&id) {
                continue;
            }
            let Some(identity) = self.save_identity(&id) else { continue };
            self.save_player_cursor = (index + 1) % count;
            let input = match self.owner_save_input(&id, None) {
                Ok(input) => input.into_owned(),
                Err(reason) => {
                    eprintln!("save.checkpoint_failed player={id}: {reason}");
                    return None;
                }
            };
            let atomic = self.connections[&identity.peer_id].initial_save_pending
                || terminal(identity.phase);
            // Later commands reinsert this ID while the owned capture is encoded.
            self.pending_player_saves.remove(&id);
            self.autonomous_save_player = Some(id.clone());
            return Some(SaveJob {
                college_id: self.id.clone(),
                instance: Arc::clone(&self.save_instance),
                identity,
                input,
                atomic,
            });
        }
        None
    }

    pub(in crate::host) fn accept_encoded_save(
        &mut self,
        completion: SaveCompletion,
    ) -> Result<(), String> {
        if !Arc::ptr_eq(&completion.instance, &self.save_instance) {
            return Ok(());
        }
        self.autonomous_save_player = None;
        let identity = completion.identity;
        let Some(current) = self.save_identity(&identity.player_id) else { return Ok(()) };
        if current != identity {
            // A newer forced checkpoint already covers this obligation. Other stale
            // observations need a fresh capture, without erasing any newer dirty ID.
            if current.last_save_sequence == identity.last_save_sequence {
                self.pending_player_saves.insert(identity.player_id);
            }
            return Ok(());
        }
        let encoded = match completion.result {
            Ok(encoded) => encoded,
            Err(SaveEncodeFailure::Creation(reason)) => {
                eprintln!("save.checkpoint_failed player={}: {reason}", identity.player_id);
                self.pending_player_saves.insert(identity.player_id);
                return Ok(());
            }
            Err(SaveEncodeFailure::Transport(reason)) => return Err(reason),
        };
        let player = self.player(&identity.player_id).expect("the save identity was checked");
        let duplicate = player.last_save_document.as_ref() == Some(&encoded.document);
        if !duplicate {
            self.connections[&identity.peer_id]
                .peer
                .publish_prepared_save_checkpoint(encoded.prepared)?;
            let player = self.player_mut(&identity.player_id).unwrap();
            player.last_save_sequence += 1;
            player.last_save_document = Some(encoded.document);
            if let Some(token) = encoded.rejoin_token
                && let Some(slot) = self.rejoin_slots.iter_mut().find(|slot| {
                    slot.player_id == identity.player_id
                        && Some(&slot.run_id) == identity.run_id.as_ref()
                        && Some(&slot.recovery_id) == identity.recovery_id.as_ref()
                        && Some(&slot.party_id) == identity.party_id.as_ref()
                })
            {
                slot.token = Some(token);
            }
        }
        self.connections.get_mut(&identity.peer_id).unwrap().initial_save_pending = false;
        if terminal(identity.phase) {
            self.player_mut(&identity.player_id).unwrap().terminal_saved_run_id = identity.run_id;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(in crate::host) struct SaveEncodeGate {
    pub entered: std::sync::mpsc::Sender<()>,
    pub release: std::sync::Mutex<Option<std::sync::mpsc::Receiver<Result<(), String>>>>,
}

#[cfg(test)]
impl SaveEncodeGate {
    pub fn wait(&self) -> Result<(), String> {
        let release = self.release.lock().unwrap().take();
        if let Some(release) = release {
            let _ = self.entered.send(());
            return release.recv().unwrap_or(Ok(()));
        }
        Ok(())
    }
}
