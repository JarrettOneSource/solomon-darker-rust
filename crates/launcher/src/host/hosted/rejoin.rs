//! Source per-run portable lineage and per-member retained rejoin ownership. A live
//! actor stays only in its canonical Session; a detached member retains its real snapshot.

use std::sync::Arc;

use solomon_modding::package::MaterializedWebSessionContent;
use solomon_sim::session::DetachedSessionParticipant;
use solomon_sim::social::{PartyIdentity, PartyVisibility, PlayerSocialProfile};

use super::{HostedGameHost, PartyReservation};
use crate::host::authority::close_peer;
use crate::host::recovery::{
    PartyRecoveryClaim, PartyRecoveryRosterMember, decode_party_recovery_claim,
};
use crate::host::signing::credentials_equal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostedRejoinStatus {
    Connected,
    Detached,
    Reserved,
    Staging,
}

pub struct HostedRejoinTarget {
    pub content: Arc<MaterializedWebSessionContent>,
    pub party_id: String,
    pub player_id: String,
    pub run_id: String,
    pub status: HostedRejoinStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostedRejoinRejection {
    AlreadyReserved,
    PlayerConnected,
    RunUnavailable,
}

pub struct HostedRejoinSlot {
    pub connected: bool,
    pub content: Arc<MaterializedWebSessionContent>,
    pub developer_access: bool,
    pub detached: Option<DetachedSessionParticipant>,
    pub global_score_eligible: bool,
    pub leaderboard_user_id: Option<u32>,
    pub local_only: bool,
    pub party_id: String,
    pub party_identity: Option<PartyIdentity>,
    pub player_id: String,
    pub player_reference: Option<String>,
    pub profile: PlayerSocialProfile,
    pub recovery_id: String,
    pub reservation: Option<PartyReservation>,
    pub run_id: String,
    pub token: Option<String>,
}

pub struct HostedRecoveryLineage {
    pub content: Arc<MaterializedWebSessionContent>,
    pub party_leader_player_id: String,
    pub party_id: String,
    pub party_member_count: usize,
    pub party_roster: Vec<PartyRecoveryRosterMember>,
    pub party_visibility: PartyVisibility,
    pub recovery_id: String,
    pub run_id: String,
}

impl HostedGameHost {
    pub(in crate::host) fn staged_rejoin_index(&self, id: &str) -> Option<usize> {
        self.rejoin_slots
            .iter()
            .position(|slot| slot.player_id == id && slot.connected && slot.detached.is_some())
    }

    /// `activePartyRecoveryLineage`: a signed roster identifies one retained run,
    /// rather than granting authority to recreate an arbitrary browser-held world.
    fn active_recovery_lineage(
        &self,
        claim: &PartyRecoveryClaim,
    ) -> Option<&HostedRecoveryLineage> {
        let lineage = self.recovery_lineages.iter().find(|lineage| {
            lineage.recovery_id == claim.recovery_id
                && lineage.run_id == claim.run_id
                && lineage.party_member_count == claim.party_member_count
                && lineage.party_leader_player_id == claim.party_leader_player_id
                && lineage.party_visibility == claim.party_visibility
                && lineage.content.manifest.manifest_sha256 == claim.content_manifest_sha256
                && self.kind == claim.session_kind
                && same_roster_membership(&lineage.party_roster, &claim.party_roster)
        })?;
        self.active_run(&lineage.party_id, &lineage.run_id)?;
        Some(lineage)
    }

    pub fn party_rejoin_target(&mut self, token: &str, now_ms: f64) -> Option<HostedRejoinTarget> {
        self.prune_rejoins(now_ms);
        let claim = decode_party_recovery_claim(&self.recovery_secret, token)?;
        let lineage = self.active_recovery_lineage(&claim)?;
        let slot = self.rejoin_slots.iter().find(|slot| {
            slot.recovery_id == claim.recovery_id && slot.player_id == claim.player_id
        });
        let materialized = self
            .worlds
            .state_for_player(&claim.player_id)
            .is_some_and(|state| state.run.run_id.as_deref() == Some(lineage.run_id.as_str()));
        let status = match slot {
            Some(slot) if slot.connected && slot.detached.is_some() => HostedRejoinStatus::Staging,
            _ if materialized || slot.is_some_and(|slot| slot.connected) => {
                HostedRejoinStatus::Connected
            }
            Some(slot) if slot.reservation.is_some() => HostedRejoinStatus::Reserved,
            _ => HostedRejoinStatus::Detached,
        };
        Some(HostedRejoinTarget {
            content: lineage.content.clone(),
            party_id: lineage.party_id.clone(),
            player_id: claim.player_id,
            run_id: lineage.run_id.clone(),
            status,
        })
    }

    pub fn reserve_party_rejoin(
        &mut self,
        token: &str,
        reservation_id: String,
        expires_at_ms: f64,
        now_ms: f64,
    ) -> Result<(), HostedRejoinRejection> {
        let target =
            self.party_rejoin_target(token, now_ms).ok_or(HostedRejoinRejection::RunUnavailable)?;
        match target.status {
            HostedRejoinStatus::Connected | HostedRejoinStatus::Staging => {
                return Err(HostedRejoinRejection::PlayerConnected);
            }
            HostedRejoinStatus::Reserved => return Err(HostedRejoinRejection::AlreadyReserved),
            HostedRejoinStatus::Detached => {}
        }
        let claim = decode_party_recovery_claim(&self.recovery_secret, token)
            .expect("the target admitted this signed claim");
        let index = match self.rejoin_slots.iter().position(|slot| {
            slot.recovery_id == claim.recovery_id && slot.player_id == claim.player_id
        }) {
            Some(index) => index,
            None => {
                self.rejoin_slots.push(HostedRejoinSlot {
                    connected: false,
                    content: target.content,
                    developer_access: false,
                    detached: None,
                    global_score_eligible: claim.global_score_eligible,
                    leaderboard_user_id: claim.leaderboard_user_id,
                    local_only: claim.integrity
                        == solomon_assets::website_save::SaveIntegrity::LocalOnly,
                    party_id: target.party_id.clone(),
                    party_identity: None,
                    player_id: claim.player_id,
                    player_reference: None,
                    profile: PlayerSocialProfile::default(),
                    recovery_id: claim.recovery_id,
                    reservation: None,
                    run_id: target.run_id,
                    token: None,
                });
                self.rejoin_slots.len() - 1
            }
        };
        let slot = &mut self.rejoin_slots[index];
        slot.token = Some(token.into());
        slot.reservation =
            Some(PartyReservation { party_id: target.party_id, reservation_id, expires_at_ms });
        Ok(())
    }

    pub(in crate::host) fn valid_rejoin_reservation(
        &mut self,
        token: &str,
        reservation_id: Option<&str>,
        now_ms: f64,
    ) -> Option<usize> {
        self.prune_rejoins(now_ms);
        let reservation_id = reservation_id?;
        let claim = decode_party_recovery_claim(&self.recovery_secret, token)?;
        self.active_recovery_lineage(&claim)?;
        self.rejoin_slots.iter().position(|slot| {
            slot.recovery_id == claim.recovery_id
                && slot.player_id == claim.player_id
                && !slot.connected
                && slot.reservation.as_ref().is_some_and(|reservation| {
                    credentials_equal(&reservation.reservation_id, reservation_id)
                        && reservation.expires_at_ms > now_ms
                })
        })
    }

    pub(in crate::host) fn prune_rejoins(&mut self, now_ms: f64) {
        for slot in &self.rejoin_slots {
            if slot.connected
                && slot.detached.is_some()
                && self.active_run(&slot.party_id, &slot.run_id).is_none()
                && let Some(connection) = self
                    .player(&slot.player_id)
                    .and_then(|player| self.connections.get(&player.peer_id))
            {
                close_peer(&connection.peer, 1000, "active party run ended")
                    .expect("the source's static close code and reason are valid");
            }
        }
        let terminal: Vec<_> = self
            .recovery_lineages
            .iter()
            .filter(|lineage| self.active_run(&lineage.party_id, &lineage.run_id).is_none())
            .map(|lineage| lineage.run_id.clone())
            .collect();
        for run_id in terminal {
            self.retire_recovery_run(&run_id, true);
        }
        for slot in &mut self.rejoin_slots {
            if slot
                .reservation
                .as_ref()
                .is_some_and(|reservation| reservation.expires_at_ms <= now_ms)
            {
                slot.reservation = None;
            }
        }
    }

    pub(in crate::host) fn register_recovery_lineage(
        &mut self,
        claim: &PartyRecoveryClaim,
        party_id: String,
        content: Arc<MaterializedWebSessionContent>,
    ) {
        self.recovery_lineages.retain(|lineage| lineage.recovery_id != claim.recovery_id);
        self.recovery_lineages.push(HostedRecoveryLineage {
            content,
            party_leader_player_id: claim.party_leader_player_id.clone(),
            party_id,
            party_member_count: claim.party_member_count,
            party_roster: claim.party_roster.clone(),
            party_visibility: claim.party_visibility,
            recovery_id: claim.recovery_id.clone(),
            run_id: claim.run_id.clone(),
        });
    }

    pub(in crate::host) fn recovery_roster(
        &self,
        party_id: &str,
    ) -> Result<Vec<PartyRecoveryRosterMember>, String> {
        let party = self
            .worlds
            .parties()
            .parties
            .iter()
            .find(|party| party.id == party_id)
            .ok_or("The recovery lineage has no retained party")?;
        let mut roster = Vec::with_capacity(party.member_player_ids.len());
        for id in &party.member_player_ids {
            if let Some(participant) = self.retained_participant(id) {
                let player = &participant.player;
                roster.push(PartyRecoveryRosterMember {
                    current_health: player.combat.current_health,
                    display_name: player.selection.name.clone(),
                    element: player.selection.element,
                    life_state: player.combat.life_state,
                    maximum_health: player.combat.maximum_health,
                    player_id: id.clone(),
                });
            } else {
                let row = self
                    .recovery_lineages
                    .iter()
                    .filter(|lineage| lineage.party_id == party_id)
                    .flat_map(|lineage| &lineage.party_roster)
                    .find(|row| row.player_id == *id)
                    .ok_or_else(|| {
                        format!("The recovery roster has no authoritative state for {id}")
                    })?;
                roster.push(row.clone());
            }
        }
        Ok(roster)
    }
}

fn same_roster_membership(
    first: &[PartyRecoveryRosterMember],
    second: &[PartyRecoveryRosterMember],
) -> bool {
    first.len() == second.len()
        && first.iter().zip(second).all(|(first, second)| {
            first.player_id == second.player_id
                && first.display_name == second.display_name
                && first.element == second.element
        })
}
