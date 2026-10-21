//! Browser-owned source saves. A checkpoint is the actual schema-37 owner document;
//! the server never acknowledges a disk/cloud write on behalf of the client.
//! Source: `game-host.ts::publishSaveCheckpointForClient`.

use std::borrow::Cow;

use solomon_assets::website_save::{self, CreateSaveOptions};
use solomon_protocol::hosted::SaveCheckpointReason;
use solomon_sim::game_run::GameRunPhase;

use super::{HostedGameHost, HostedWorlds};
use crate::host::connection::HostConnection;
use crate::host::recovery::{PartyRecoveryClaim, create_party_recovery_claim};

pub(in crate::host) struct PlayerSaveRequest<'a> {
    pub target_revision: Option<&'a str>,
    pub force: bool,
    pub include_terminal_profile: bool,
}

impl HostedGameHost {
    pub(in crate::host) fn owner_save(
        &self,
        id: &str,
        target_revision: Option<&str>,
    ) -> Result<String, String> {
        let player = self.player(id).ok_or("The saved player has no admitted identity")?;
        let mut metadata = player.save_metadata.clone();
        metadata.mod_state = self.prepared_mod_save_state(id)?;
        let staged = self.staged_rejoin_index(id).map(|index| &self.rejoin_slots[index]);
        let state = if let Some(slot) = staged {
            let active = self
                .active_run(&slot.party_id, &slot.run_id)
                .ok_or("The staged saved wizard lost its active run")?;
            // Disk/cloud publication is infrequent. Source projects that one detached
            // participant into its retained world; this owned save copy never ticks or
            // replaces the authoritative world (ordinary snapshots remain borrowed).
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
        if matches!(state.run.phase, GameRunPhase::GameOver | GameRunPhase::Loadout) {
            return website_save::create_profile(&state, id, &metadata);
        }
        let loaded = self
            .worlds
            .loaded_for_player(id)
            .or_else(|| self.worlds.loaded_for_run(&staged?.run_id));
        let options = |token| CreateSaveOptions {
            state: &state,
            player_id: id,
            loaded_boneyard: loaded,
            metadata: &metadata,
            party_rejoin_token: token,
        };
        let unsigned = website_save::create(options(None))?;
        let Some(lineage) = self.recovery_lineages.iter().find(|lineage| {
            state.run.run_id.as_ref() == Some(&lineage.run_id)
                && self.party_id_for_player(id) == Some(&lineage.party_id)
        }) else {
            return Ok(unsigned);
        };
        let party = self
            .worlds
            .parties()
            .parties
            .iter()
            .find(|party| party.id == lineage.party_id)
            .ok_or("The recovery lineage lost its party membership")?;
        let claim = PartyRecoveryClaim {
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
            target_revision: Some(target_revision.unwrap_or(&self.recovery_revision).into()),
        };
        let token = create_party_recovery_claim(&self.recovery_secret, &claim, &unsigned)?;
        website_save::create(options(Some(&token)))
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
        let atomic = std::mem::take(&mut connection.initial_save_pending)
            || request.force
            || request.include_terminal_profile;
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
        let save = match self.owner_save(id, request.target_revision) {
            Ok(save) => save,
            Err(reason) => {
                // Source checkpoint_failed is a logged save failure/sequence0, not a
                // fatal game epoch. Actual execution/filter errors still fail the host.
                eprintln!("save.checkpoint_failed player={id}: {reason}");
                return Ok(0);
            }
        };
        let player = self.player_mut(id).ok_or("The saved wizard is no longer connected")?;
        if !request.force && player.last_save_document.as_ref() == Some(&save) {
            return Ok(player.last_save_sequence);
        }
        player.last_save_sequence += 1;
        let sequence = player.last_save_sequence;
        player.last_save_document = Some(save.clone());
        let saved = website_save::parse(&save)?;
        if let Some(token) = saved
            .continuation
            .as_ref()
            .and_then(|continuation| continuation.summary.party_rejoin_token.as_ref())
            && let Some(slot) = self.rejoin_slots.iter_mut().find(|slot| slot.player_id == id)
        {
            slot.token = Some(token.clone());
        }
        let reason =
            if terminal { SaveCheckpointReason::GameOver } else { SaveCheckpointReason::Progress };
        connection.peer.publish_save_checkpoint(save, reason, sequence, atomic)?;
        Ok(sequence)
    }

    pub(in crate::host) fn publish_pending_saves(&mut self, autosave: bool) -> Result<(), String> {
        let ids: Vec<_> = self
            .players
            .iter()
            .filter_map(|player| {
                if self.connections.get(&player.peer_id)?.peer.is_closed() {
                    return None;
                }
                let state = self.worlds.state_for_player(&player.id)?;
                let terminal =
                    matches!(state.run.phase, GameRunPhase::GameOver | GameRunPhase::Loadout)
                        && player.terminal_saved_run_id != state.run.run_id;
                (terminal || autosave || self.pending_player_saves.contains(&player.id)).then(
                    || (player.id.clone(), player.peer_id, terminal, state.run.run_id.clone()),
                )
            })
            .collect();
        for (id, peer_id, terminal, run_id) in ids {
            let Some(mut connection) = self.connections.remove(&peer_id) else { continue };
            let result = self.publish_player_save(
                &mut connection,
                &id,
                PlayerSaveRequest {
                    target_revision: None,
                    force: false,
                    include_terminal_profile: terminal,
                },
            );
            self.connections.insert(peer_id, connection);
            result?;
            self.pending_player_saves.remove(&id);
            if terminal {
                self.player_mut(&id)
                    .expect("the saved player remains connected")
                    .terminal_saved_run_id = run_id;
            }
        }
        Ok(())
    }
}
