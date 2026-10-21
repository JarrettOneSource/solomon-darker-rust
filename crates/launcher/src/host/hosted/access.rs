//! Party discovery, external consent requests and one-use reservation capacity.
//! Source: `game-host.ts` partyTarget/reserveExternalPartyJoin/prunePartyAccess.

use std::collections::BTreeSet;

use serde::Serialize;
use solomon_sim::modding::identity::WebSessionContentSummary;
use solomon_sim::social::{PartyActionRejection, PartyJoinRequester, PartyVisibility};

use super::{HostedGameHost, HostedWorlds, PartyReservation};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PartyStatus {
    Hub,
    Playing,
}

pub struct HostedPartyTarget {
    pub cheats_enabled: bool,
    pub content: WebSessionContentSummary,
    pub id: String,
    pub leader: String,
    pub member_count: usize,
    pub status: PartyStatus,
    pub visibility: PartyVisibility,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPartyDirectoryEntry {
    pub boneyard_name: Option<String>,
    pub cheats_enabled: bool,
    pub id: String,
    pub leader: String,
    pub max_members: usize,
    pub member_count: usize,
    pub members: Vec<String>,
    pub mod_count: usize,
    pub session_kind: &'static str,
    pub status: PartyStatus,
    pub visibility: &'static str,
}

pub struct ExternalPartyJoinRequest {
    pub expires_at_ms: f64,
    pub id: String,
    pub party_id: String,
    pub requester: PartyJoinRequester,
    pub status: ExternalPartyJoinRequestStatus,
    pub token: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalPartyJoinRequestStatus {
    Pending,
    Accepted,
    Denied,
}

impl HostedGameHost {
    pub fn party_target(&self, party_id: &str) -> Option<HostedPartyTarget> {
        let party = self.worlds.parties().parties.iter().find(|party| party.id == party_id)?;
        self.player(&party.leader_player_id)?;
        let state = self.worlds.state_for_player(&party.leader_player_id)?;
        let leader = state.participant(&party.leader_player_id)?.player.selection.name.clone();
        let playing = match &self.worlds {
            HostedWorlds::Global(worlds) => worlds.runs.iter().any(|run| run.party_id == party_id),
            HostedWorlds::Private { state, .. } => state.boneyard.is_some(),
        };
        Some(HostedPartyTarget {
            cheats_enabled: self.cheats_enabled,
            content: self.player(&party.leader_player_id)?.content.summary.clone(),
            id: party.id.clone(),
            leader,
            member_count: party.member_player_ids.len(),
            status: if playing { PartyStatus::Playing } else { PartyStatus::Hub },
            visibility: party.visibility,
        })
    }

    pub fn party_target_by_code(&self, code: &str) -> Option<HostedPartyTarget> {
        self.party_target(&self.worlds.parties().party_by_join_code(code)?.id)
    }

    pub fn party_target_by_listing(&self, listing: &str) -> Option<HostedPartyTarget> {
        let party = self.worlds.parties().party_by_listing_id(listing)?;
        if party.visibility == PartyVisibility::Private {
            return None;
        }
        self.party_target(&party.id)
    }

    /// Public projection omits an entire party when even one member is disconnected.
    /// Codes, credentials, opaque actor IDs and package manifests never enter this list.
    pub fn public_parties(&self) -> Vec<PublicPartyDirectoryEntry> {
        if self.editor_test {
            return Vec::new();
        }
        let mut output = Vec::new();
        for party in &self.worlds.parties().parties {
            if party.visibility == PartyVisibility::Private {
                continue;
            }
            let Some(target) = self.party_target(&party.id) else { continue };
            let mut members = Vec::new();
            for id in &party.member_player_ids {
                if self.player(id).is_none() {
                    break;
                }
                let Some(participant) =
                    self.worlds.state_for_player(id).and_then(|state| state.participant(id))
                else {
                    break;
                };
                members.push(participant.player.selection.name.clone());
            }
            if members.len() != party.member_player_ids.len() {
                continue;
            }
            let boneyard_name = match &self.worlds {
                HostedWorlds::Global(worlds) => worlds
                    .runs
                    .iter()
                    .find(|run| run.party_id == party.id)
                    .map(|run| run.loaded_boneyard.name.clone()),
                HostedWorlds::Private { state, .. } => {
                    state.boneyard.as_ref().map(|world| world.loaded.name.clone())
                }
            };
            let Some(listing_id) = &party.listing_id else { continue };
            output.push(PublicPartyDirectoryEntry {
                boneyard_name,
                cheats_enabled: target.cheats_enabled,
                id: listing_id.clone(),
                leader: target.leader,
                max_members: self.maximum_players,
                member_count: members.len(),
                members,
                mod_count: self.content.summary.mods.len(),
                session_kind: if matches!(self.worlds, HostedWorlds::Global(_)) {
                    "global-hub"
                } else {
                    "private-college"
                },
                status: target.status,
                visibility: if party.visibility == PartyVisibility::Public {
                    "public"
                } else {
                    "invite-only"
                },
            });
        }
        output
    }

    pub fn reserve_party_join(
        &mut self,
        party_id: &str,
        reservation_id: String,
        expires_at_ms: f64,
        now_ms: f64,
    ) -> Result<(), PartyActionRejection> {
        self.prune_party_access(now_ms);
        let target = self.party_target(party_id).ok_or(PartyActionRejection::PartyMissing)?;
        if target.status != PartyStatus::Hub {
            return Err(PartyActionRejection::NotInHub);
        }
        let reserved =
            self.reservations.iter().filter(|reservation| reservation.party_id == party_id).count();
        if target.member_count + reserved + self.detached_party_members(party_id, None)
            >= self.maximum_players
        {
            return Err(PartyActionRejection::PartyFull);
        }
        if let Some(existing) = self
            .reservations
            .iter_mut()
            .find(|reservation| reservation.reservation_id == reservation_id)
        {
            existing.party_id = party_id.into();
            existing.expires_at_ms = expires_at_ms;
        } else {
            self.reservations.push(PartyReservation {
                party_id: party_id.into(),
                reservation_id,
                expires_at_ms,
            });
        }
        Ok(())
    }

    pub fn valid_party_reservation(
        &mut self,
        party_id: &str,
        reservation_id: Option<&str>,
        now_ms: f64,
    ) -> bool {
        self.prune_party_access(now_ms);
        self.reservations.iter().any(|reservation| {
            reservation.party_id == party_id
                && Some(reservation.reservation_id.as_str()) == reservation_id
                && reservation.expires_at_ms > now_ms
        })
    }

    pub fn cancel_party_reservation(&mut self, reservation_id: &str) {
        self.reservations.retain(|reservation| reservation.reservation_id != reservation_id);
        for slot in &mut self.rejoin_slots {
            if slot
                .reservation
                .as_ref()
                .is_some_and(|reservation| reservation.reservation_id == reservation_id)
            {
                slot.reservation = None;
            }
        }
    }

    pub fn available_party_members(
        &self,
        party_id: Option<&str>,
        reservation_id: Option<&str>,
    ) -> usize {
        self.available_party_members_excluding(party_id, reservation_id, None)
    }

    pub(in crate::host) fn available_party_members_excluding(
        &self,
        party_id: Option<&str>,
        reservation_id: Option<&str>,
        excluded_player_id: Option<&str>,
    ) -> usize {
        let Some(party_id) = party_id else { return self.maximum_players };
        let reserved = self
            .reservations
            .iter()
            .filter(|reservation| {
                reservation.party_id == party_id
                    && Some(reservation.reservation_id.as_str()) != reservation_id
            })
            .count();
        self.maximum_players
            .saturating_sub(reserved + self.detached_party_members(party_id, excluded_player_id))
            .max(1)
    }

    pub fn detached_party_members(&self, party_id: &str, excluded_id: Option<&str>) -> usize {
        let mut detached = BTreeSet::new();
        for slot in &self.rejoin_slots {
            if slot.party_id == party_id
                && Some(slot.player_id.as_str()) != excluded_id
                && self.player(&slot.player_id).is_none()
                && !self.is_bot(&slot.player_id)
            {
                detached.insert(slot.player_id.as_str());
            }
        }
        for lineage in &self.recovery_lineages {
            if lineage.party_id == party_id {
                for member in &lineage.party_roster {
                    if Some(member.player_id.as_str()) != excluded_id
                        && self.player(&member.player_id).is_none()
                        && !self.is_bot(&member.player_id)
                    {
                        detached.insert(member.player_id.as_str());
                    }
                }
            }
        }
        detached.len()
    }

    pub fn capacity_participant_count(&self) -> usize {
        let mut detached = BTreeSet::new();
        for slot in &self.rejoin_slots {
            if self.player(&slot.player_id).is_none() && !self.is_bot(&slot.player_id) {
                detached.insert(slot.player_id.as_str());
            }
        }
        for lineage in &self.recovery_lineages {
            for member in &lineage.party_roster {
                if self.player(&member.player_id).is_none() && !self.is_bot(&member.player_id) {
                    detached.insert(member.player_id.as_str());
                }
            }
        }
        self.players.len() + self.bot_count() + detached.len()
    }

    pub fn create_party_join_request(
        &mut self,
        listing_id: &str,
        request_id: String,
        token: String,
        requester: PartyJoinRequester,
        expires_at_ms: f64,
        now_ms: f64,
    ) -> Result<(), PartyActionRejection> {
        self.prune_party_access(now_ms);
        let party = self
            .worlds
            .parties()
            .party_by_listing_id(listing_id)
            .ok_or(PartyActionRejection::PartyMissing)?;
        if party.visibility != PartyVisibility::InviteOnly {
            return Err(PartyActionRejection::PartyPrivate);
        }
        let party_id = party.id.clone();
        let maximum = self.available_party_members(Some(&party_id), None);
        self.worlds.parties_mut().request_join(
            &party_id,
            request_id.clone(),
            requester.clone(),
            maximum,
        )?;
        self.external_join_requests.push(ExternalPartyJoinRequest {
            expires_at_ms,
            id: request_id,
            party_id,
            requester,
            status: ExternalPartyJoinRequestStatus::Pending,
            token,
        });
        let request =
            self.external_join_requests.last().expect("accepted join request is retained");
        self.emit_party_activity(
            "party.join_requested",
            "Someone requested to join a party.",
            Some(&request.party_id),
            serde_json::json!({"requestId":request.id,"requester":super::activity::requester_activity(&request.requester)}),
        );
        self.social_dirty = true;
        Ok(())
    }

    pub fn decide_party_join_request(
        &mut self,
        id: &str,
        request_id: &str,
        accept: bool,
    ) -> Result<(), PartyActionRejection> {
        self.worlds.parties_mut().decide_join_request(id, request_id)?;
        if let Some(request) = self.external_join_requests.iter_mut().find(|request| {
            request.id == request_id && request.status == ExternalPartyJoinRequestStatus::Pending
        }) {
            request.status = if accept {
                ExternalPartyJoinRequestStatus::Accepted
            } else {
                ExternalPartyJoinRequestStatus::Denied
            };
        }
        self.social_dirty = true;
        Ok(())
    }

    pub fn prune_party_access(&mut self, now_ms: f64) {
        self.reservations.retain(|reservation| reservation.expires_at_ms > now_ms);
        let parties = self.worlds.parties();
        let mut expired_ids = Vec::new();
        self.external_join_requests.retain(|request| {
            let keep = request.expires_at_ms > now_ms
                && parties.parties.iter().any(|party| party.id == request.party_id);
            if !keep {
                expired_ids.push(request.id.clone());
            }
            keep
        });
        let parties = self.worlds.parties_mut();
        let previous_count = parties.join_requests.len();
        parties.join_requests.retain(|request| !expired_ids.contains(&request.id));
        if previous_count != parties.join_requests.len() {
            parties.revision += 1;
            self.social_dirty = true;
        }
    }
}
