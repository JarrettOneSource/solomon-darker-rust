//! Hosted party membership from Website `host/party-system.ts`.
//! Identity generation, deadlines and socket admission remain with the host. Ordered
//! parties and members retain source insertion order through invites, joins and departure.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    LocalPartyState, MissingPartyRosterPlayer, PartyActionRejection, PartyInvitationView,
    PartyJoinRequestView, PartyJoinRequester, PartyMembership, PartyPlayerProfile,
    PartyRosterPlayer, PartyVisibility, party_after_member_departure,
};

const MAX_PENDING_JOIN_REQUESTS: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyIdentity {
    pub id: String,
    pub join_code: String,
    pub listing_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyInvitation {
    pub id: String,
    pub invited_player_id: String,
    pub inviter_player_id: String,
    pub party_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyJoinRequest {
    pub id: String,
    pub party_id: String,
    pub requester: PartyJoinRequester,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartySystem {
    pub invitations: Vec<PartyInvitation>,
    pub join_requests: Vec<PartyJoinRequest>,
    pub next_invitation_ordinal: u64,
    pub parties: Vec<PartyMembership>,
    pub revision: u64,
}

impl Default for PartySystem {
    fn default() -> Self {
        Self {
            invitations: Vec::new(),
            join_requests: Vec::new(),
            next_invitation_ordinal: 1,
            parties: Vec::new(),
            revision: 0,
        }
    }
}

impl PartySystem {
    #[must_use]
    pub fn party_for_player(&self, player_id: &str) -> Option<&PartyMembership> {
        self.parties
            .iter()
            .find(|party| party.member_player_ids.iter().any(|member| member == player_id))
    }

    #[must_use]
    pub fn party_by_join_code(&self, code: &str) -> Option<&PartyMembership> {
        self.parties.iter().find(|party| party.join_code.as_deref() == Some(code))
    }

    #[must_use]
    pub fn party_by_listing_id(&self, listing_id: &str) -> Option<&PartyMembership> {
        self.parties.iter().find(|party| party.listing_id.as_deref() == Some(listing_id))
    }

    pub fn register_player(&mut self, player_id: &str, identity: PartyIdentity) {
        if self.party_for_player(player_id).is_some() {
            return;
        }
        self.parties.push(PartyMembership {
            enhanced_effects: true,
            id: identity.id,
            join_code: Some(identity.join_code),
            leader_player_id: player_id.into(),
            listing_id: Some(identity.listing_id),
            member_player_ids: vec![player_id.into()],
            visibility: PartyVisibility::Public,
        });
        self.revision += 1;
    }

    pub fn remove_player(&mut self, player_id: &str) {
        let Some(active) = self.party_for_player(player_id) else { return };
        let party_id = active.id.clone();
        let remaining = party_after_member_departure(active, player_id);
        self.parties.retain_mut(|party| {
            if party.id != party_id {
                return true;
            }
            if let Some(remaining) = &remaining {
                *party = remaining.clone();
                return true;
            }
            false
        });
        self.invitations.retain(|invitation| {
            invitation.invited_player_id != player_id
                && invitation.inviter_player_id != player_id
                && self.parties.iter().any(|party| party.id == invitation.party_id)
        });
        self.join_requests
            .retain(|request| self.parties.iter().any(|party| party.id == request.party_id));
        self.revision += 1;
    }

    pub fn leave(
        &mut self,
        player_id: &str,
        identity: PartyIdentity,
    ) -> Result<(), PartyActionRejection> {
        let source = self.party_for_player(player_id).ok_or(PartyActionRejection::PlayerMissing)?;
        if source.member_player_ids.len() == 1 {
            return Ok(());
        }
        // Source leave performs two membership changes, so both revisions are retained.
        self.remove_player(player_id);
        self.register_player(player_id, identity);
        Ok(())
    }

    pub fn kick(
        &mut self,
        leader_id: &str,
        target_id: &str,
        identity: PartyIdentity,
    ) -> Result<(), PartyActionRejection> {
        if let Some(reason) =
            super::party_kick_rejection(self.party_for_player(leader_id), leader_id, target_id)
        {
            return Err(reason);
        }
        self.remove_player(target_id);
        self.register_player(target_id, identity);
        Ok(())
    }

    pub fn invite(
        &mut self,
        inviter_id: &str,
        invited_id: &str,
        maximum_members: usize,
    ) -> Result<(), PartyActionRejection> {
        let inviter =
            self.party_for_player(inviter_id).ok_or(PartyActionRejection::PlayerMissing)?;
        let invited =
            self.party_for_player(invited_id).ok_or(PartyActionRejection::PlayerMissing)?;
        if inviter.leader_player_id != inviter_id {
            return Err(PartyActionRejection::NotLeader);
        }
        if inviter_id == invited_id {
            return Err(PartyActionRejection::SelfInvite);
        }
        if inviter.id == invited.id {
            return Err(PartyActionRejection::SameParty);
        }
        if invited.member_player_ids.len() != 1 {
            return Err(PartyActionRejection::AlreadyInParty);
        }
        if inviter.member_player_ids.len() >= maximum_members {
            return Err(PartyActionRejection::PartyFull);
        }
        if self.invitations.iter().any(|invitation| {
            invitation.party_id == inviter.id && invitation.invited_player_id == invited_id
        }) {
            return Err(PartyActionRejection::AlreadyInvited);
        }
        let party_id = inviter.id.clone();
        self.invitations.push(PartyInvitation {
            id: format!("invite-{}", self.next_invitation_ordinal),
            invited_player_id: invited_id.into(),
            inviter_player_id: inviter_id.into(),
            party_id,
        });
        self.next_invitation_ordinal += 1;
        self.revision += 1;
        Ok(())
    }

    pub fn accept_invitation(
        &mut self,
        player_id: &str,
        invitation_id: &str,
        maximum_members: usize,
    ) -> Result<(), PartyActionRejection> {
        let invitation = self
            .invitations
            .iter()
            .find(|invitation| invitation.id == invitation_id)
            .ok_or(PartyActionRejection::InvitationMissing)?;
        if invitation.invited_player_id != player_id {
            return Err(PartyActionRejection::NotRecipient);
        }
        let party_id = invitation.party_id.clone();
        self.join(player_id, &party_id, maximum_members)
    }

    pub fn join(
        &mut self,
        player_id: &str,
        destination_id: &str,
        maximum_members: usize,
    ) -> Result<(), PartyActionRejection> {
        let source = self.party_for_player(player_id).ok_or(PartyActionRejection::PartyMissing)?;
        let destination = self
            .parties
            .iter()
            .find(|party| party.id == destination_id)
            .ok_or(PartyActionRejection::PartyMissing)?;
        if source.id == destination.id || source.member_player_ids.len() != 1 {
            return Err(PartyActionRejection::AlreadyInParty);
        }
        if destination.member_player_ids.len() >= maximum_members {
            return Err(PartyActionRejection::PartyFull);
        }
        let fills_destination = destination.member_player_ids.len() + 1 >= maximum_members;
        let source_id = source.id.clone();
        self.invitations.retain(|invitation| {
            invitation.invited_player_id != player_id
                && invitation.party_id != source_id
                && (!fills_destination || invitation.party_id != destination_id)
        });
        self.join_requests.retain(|request| {
            request.requester.requester_id != player_id
                && request.party_id != source_id
                && (!fills_destination || request.party_id != destination_id)
        });
        self.parties.retain(|party| party.id != source_id);
        let destination = self.parties.iter_mut().find(|party| party.id == destination_id).unwrap();
        destination.member_player_ids.push(player_id.into());
        self.revision += 1;
        Ok(())
    }

    pub fn deny_invitation(
        &mut self,
        player_id: &str,
        invitation_id: &str,
    ) -> Result<(), PartyActionRejection> {
        let invitation = self
            .invitations
            .iter()
            .find(|invitation| invitation.id == invitation_id)
            .ok_or(PartyActionRejection::InvitationMissing)?;
        if invitation.invited_player_id != player_id {
            return Err(PartyActionRejection::NotRecipient);
        }
        self.invitations.retain(|invitation| invitation.id != invitation_id);
        self.revision += 1;
        Ok(())
    }

    pub fn set_visibility(
        &mut self,
        leader_id: &str,
        visibility: PartyVisibility,
    ) -> Result<(), PartyActionRejection> {
        let party_id = self.leader_party_id(leader_id)?;
        let party = self.parties.iter_mut().find(|party| party.id == party_id).unwrap();
        if party.visibility == visibility {
            return Ok(());
        }
        party.visibility = visibility;
        if visibility == PartyVisibility::Private {
            self.join_requests.retain(|request| request.party_id != party_id);
        }
        self.revision += 1;
        Ok(())
    }

    /// Current Source `setPartyEnhancedEffects`: a leader owns only their party's
    /// next-run choice; updating it does not change the persistent College's mode.
    pub fn set_enhanced_effects(
        &mut self,
        leader_id: &str,
        enabled: bool,
    ) -> Result<(), PartyActionRejection> {
        let party_id = self.leader_party_id(leader_id)?;
        let party = self.parties.iter_mut().find(|party| party.id == party_id).unwrap();
        if party.enhanced_effects != enabled {
            party.enhanced_effects = enabled;
            self.revision += 1;
        }
        Ok(())
    }

    pub fn rotate_join_code(
        &mut self,
        leader_id: &str,
        code: String,
    ) -> Result<(), PartyActionRejection> {
        let party_id = self.leader_party_id(leader_id)?;
        let party = self.parties.iter_mut().find(|party| party.id == party_id).unwrap();
        if party.join_code.as_deref() == Some(&code) {
            return Ok(());
        }
        party.join_code = Some(code);
        self.revision += 1;
        Ok(())
    }

    pub fn request_join(
        &mut self,
        party_id: &str,
        id: String,
        requester: PartyJoinRequester,
        maximum_members: usize,
    ) -> Result<(), PartyActionRejection> {
        let party = self
            .parties
            .iter()
            .find(|party| party.id == party_id)
            .ok_or(PartyActionRejection::PartyMissing)?;
        if party.visibility == PartyVisibility::Private {
            return Err(PartyActionRejection::PartyPrivate);
        }
        if party.member_player_ids.len() >= maximum_members {
            return Err(PartyActionRejection::PartyFull);
        }
        if self.join_requests.iter().any(|request| {
            request.party_id == party_id && request.requester.requester_id == requester.requester_id
        }) {
            return Err(PartyActionRejection::AlreadyRequested);
        }
        if self.join_requests.iter().filter(|request| request.party_id == party_id).count()
            >= MAX_PENDING_JOIN_REQUESTS
        {
            return Err(PartyActionRejection::PartyFull);
        }
        self.join_requests.push(PartyJoinRequest { id, party_id: party_id.into(), requester });
        self.revision += 1;
        Ok(())
    }

    /// Accept and deny both consume the pending request. The supervisor retains the
    /// decision and yields an admission intent; this kernel never materializes a stranger.
    pub fn decide_join_request(
        &mut self,
        leader_id: &str,
        request_id: &str,
    ) -> Result<(), PartyActionRejection> {
        let request = self
            .join_requests
            .iter()
            .find(|request| request.id == request_id)
            .ok_or(PartyActionRejection::RequestMissing)?;
        let party = self
            .parties
            .iter()
            .find(|party| party.id == request.party_id)
            .ok_or(PartyActionRejection::PartyMissing)?;
        if party.leader_player_id != leader_id {
            return Err(PartyActionRejection::NotLeader);
        }
        self.join_requests.retain(|request| request.id != request_id);
        self.revision += 1;
        Ok(())
    }

    pub fn clear_invitations(&mut self, party_id: &str) {
        let previous_invitations = self.invitations.len();
        let previous_requests = self.join_requests.len();
        let members = self
            .parties
            .iter()
            .find(|party| party.id == party_id)
            .map(|party| party.member_player_ids.as_slice())
            .unwrap_or(&[]);
        self.invitations.retain(|invitation| {
            invitation.party_id != party_id && !members.contains(&invitation.invited_player_id)
        });
        self.join_requests.retain(|request| request.party_id != party_id);
        if self.invitations.len() != previous_invitations
            || self.join_requests.len() != previous_requests
        {
            self.revision += 1;
        }
    }

    pub fn restore_membership(
        &mut self,
        claimant_id: &str,
        members: &[String],
        leader_id: &str,
        visibility: PartyVisibility,
    ) -> Result<(), String> {
        let party = self
            .party_for_player(claimant_id)
            .ok_or("party recovery claimant is not registered")?;
        let member_set: BTreeSet<_> = members.iter().collect();
        if members.is_empty()
            || member_set.len() != members.len()
            || !members.iter().any(|id| id == claimant_id)
            || !members.iter().any(|id| id == leader_id)
        {
            return Err("party recovery membership is invalid".into());
        }
        for id in members {
            if self.party_for_player(id).is_some_and(|candidate| candidate.id != party.id) {
                return Err(format!("party recovery player {id} already belongs to another party"));
            }
        }
        let party_id = party.id.clone();
        self.invitations.retain(|invitation| {
            invitation.party_id != party_id
                && !members.contains(&invitation.invited_player_id)
                && !members.contains(&invitation.inviter_player_id)
        });
        self.join_requests.retain(|request| request.party_id != party_id);
        let party = self.parties.iter_mut().find(|party| party.id == party_id).unwrap();
        party.leader_player_id = leader_id.into();
        party.member_player_ids = members.to_vec();
        party.visibility = visibility;
        self.revision += 1;
        Ok(())
    }

    pub fn project(
        &self,
        player_id: &str,
        profiles: &BTreeMap<String, PartyPlayerProfile>,
        hub_players: &BTreeSet<String>,
        roster: &BTreeMap<String, PartyRosterPlayer>,
    ) -> Result<LocalPartyState, MissingPartyRosterPlayer> {
        let party =
            self.party_for_player(player_id).expect("an admitted hosted player has a party");
        let profile = |id: &str| {
            profiles.get(id).cloned().unwrap_or_else(|| PartyPlayerProfile {
                account_username: None,
                highest_wave: None,
                total_playtime_ms: None,
                display_name: id.into(),
                player_id: id.into(),
            })
        };
        let mut hub_rows = Vec::new();
        for id in hub_players {
            hub_rows.push(profile(id));
        }
        let mut invitations = Vec::new();
        for invitation in &self.invitations {
            if invitation.invited_player_id == player_id {
                invitations.push(PartyInvitationView {
                    id: invitation.id.clone(),
                    inviter: profile(&invitation.inviter_player_id),
                    party_id: invitation.party_id.clone(),
                });
            }
        }
        let mut requests = Vec::new();
        if party.leader_player_id == player_id {
            for request in &self.join_requests {
                if request.party_id == party.id {
                    requests.push(PartyJoinRequestView {
                        id: request.id.clone(),
                        requester: request.requester.clone(),
                    });
                }
            }
        }
        let mut party_roster = Vec::new();
        for id in &party.member_player_ids {
            party_roster
                .push(roster.get(id).cloned().ok_or_else(|| MissingPartyRosterPlayer(id.clone()))?);
        }
        Ok(LocalPartyState {
            hub_players: hub_rows,
            invitations,
            join_requests: requests,
            party: party.clone(),
            party_roster,
            revision: self.revision,
        })
    }

    fn leader_party_id(&self, leader_id: &str) -> Result<String, PartyActionRejection> {
        let party = self.party_for_player(leader_id).ok_or(PartyActionRejection::PartyMissing)?;
        if party.leader_player_id != leader_id {
            return Err(PartyActionRejection::NotLeader);
        }
        Ok(party.id.clone())
    }
}
