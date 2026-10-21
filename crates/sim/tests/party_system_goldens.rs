//! Replay the current hosted party authority, including rejected transactions.

use serde_json::{Value, json};
use solomon_sim::social::{
    PartyActionRejection, PartyIdentity, PartyJoinRequester, PartySystem, PartyVisibility,
};

fn text<'a>(request: &'a Value, key: &str) -> &'a str {
    request[key].as_str().unwrap()
}

fn identity(suffix: &str) -> PartyIdentity {
    PartyIdentity {
        id: format!("party-{suffix}"),
        join_code: format!("CODE-{suffix}"),
        listing_id: format!("listing-{suffix}"),
    }
}

fn visibility(value: &str) -> PartyVisibility {
    match value {
        "public" => PartyVisibility::Public,
        "invite-only" => PartyVisibility::InviteOnly,
        "private" => PartyVisibility::Private,
        _ => panic!("the source visibility is known"),
    }
}

fn reason_id(reason: PartyActionRejection) -> &'static str {
    match reason {
        PartyActionRejection::AlreadyInParty => "already-in-party",
        PartyActionRejection::AlreadyInvited => "already-invited",
        PartyActionRejection::AlreadyRequested => "already-requested",
        PartyActionRejection::InvitationMissing => "invitation-missing",
        PartyActionRejection::NotLeader => "not-leader",
        PartyActionRejection::NotInHub => "not-in-hub",
        PartyActionRejection::NotRecipient => "not-recipient",
        PartyActionRejection::PartyFull => "party-full",
        PartyActionRejection::PartyMissing => "party-missing",
        PartyActionRejection::PartyPrivate => "party-private",
        PartyActionRejection::PlayerMissing => "player-missing",
        PartyActionRejection::RequestMissing => "request-missing",
        PartyActionRejection::SameParty => "same-party",
        PartyActionRejection::SelfInvite => "self-invite",
        PartyActionRejection::SelfKick => "self-kick",
    }
}

fn source_state(state: &PartySystem) -> Value {
    let mut parties = Vec::new();
    for party in &state.parties {
        let visibility = match party.visibility {
            PartyVisibility::Public => "public",
            PartyVisibility::InviteOnly => "invite-only",
            PartyVisibility::Private => "private",
        };
        parties.push(json!({
            "enhancedEffects":party.enhanced_effects,
            "id": party.id, "joinCode": party.join_code, "listingId": party.listing_id,
            "leaderPlayerId": party.leader_player_id,
            "memberPlayerIds": party.member_player_ids, "visibility": visibility,
        }));
        assert_eq!(state.party_by_join_code(party.join_code.as_ref().unwrap()), Some(party));
        assert_eq!(state.party_by_listing_id(party.listing_id.as_ref().unwrap()), Some(party));
    }
    let mut invitations = Vec::new();
    for invitation in &state.invitations {
        invitations.push(json!({
            "id": invitation.id, "invitedPlayerId": invitation.invited_player_id,
            "inviterPlayerId": invitation.inviter_player_id, "partyId": invitation.party_id,
        }));
    }
    let mut requests = Vec::new();
    for request in &state.join_requests {
        requests.push(json!({
            "id": request.id, "partyId": request.party_id,
            "requester": {
                "accountUsername": request.requester.account_username,
                "displayName": request.requester.display_name,
                "requesterId": request.requester.requester_id,
            },
        }));
    }
    json!({ "invitations": invitations, "joinRequests": requests,
        "nextInvitationOrdinal": state.next_invitation_ordinal,
        "parties": parties, "revision": state.revision })
}

#[test]
fn hosted_party_authority_replays_pinned_source_transactions() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/party-system-goldens.json")).unwrap();
    let mut state = PartySystem::default();
    for (index, row) in fixture["rows"].as_array().unwrap().iter().enumerate() {
        let action = text(row, "action");
        let request = &row["request"];
        let maximum = || request["maximum"].as_u64().unwrap() as usize;
        let mut error = None;
        let result = match action {
            "reset" => {
                state = PartySystem::default();
                Ok(())
            }
            "register" => {
                state.register_player(text(request, "id"), identity(text(request, "identity")));
                Ok(())
            }
            "remove" => {
                state.remove_player(text(request, "id"));
                Ok(())
            }
            "invite" => state.invite(text(request, "id"), text(request, "target"), maximum()),
            "accept" => {
                state.accept_invitation(text(request, "id"), text(request, "invitation"), maximum())
            }
            "deny" => state.deny_invitation(text(request, "id"), text(request, "invitation")),
            "join" => state.join(text(request, "id"), text(request, "party"), maximum()),
            "leave" => state.leave(text(request, "id"), identity(text(request, "identity"))),
            "kick" => state.kick(
                text(request, "id"),
                text(request, "target"),
                identity(text(request, "identity")),
            ),
            "visibility" => {
                state.set_visibility(text(request, "id"), visibility(text(request, "visibility")))
            }
            "effects" => state
                .set_enhanced_effects(text(request, "id"), request["enabled"].as_bool().unwrap()),
            "rotate" => state.rotate_join_code(text(request, "id"), text(request, "code").into()),
            "request" => state.request_join(
                text(request, "party"),
                text(request, "requestId").into(),
                PartyJoinRequester {
                    account_username: request["requester"]["accountUsername"]
                        .as_str()
                        .map(str::to_owned),
                    display_name: text(&request["requester"], "displayName").into(),
                    requester_id: text(&request["requester"], "requesterId").into(),
                },
                maximum(),
            ),
            "decide" => state.decide_join_request(text(request, "id"), text(request, "requestId")),
            "clear" => {
                state.clear_invitations(text(request, "party"));
                Ok(())
            }
            "restore" => {
                let members: Vec<_> = request["members"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap().to_owned())
                    .collect();
                error = state
                    .restore_membership(
                        text(request, "id"),
                        &members,
                        text(request, "leader"),
                        visibility(text(request, "visibility")),
                    )
                    .err();
                Ok(())
            }
            _ => panic!("unknown source action {action}"),
        };
        assert_eq!(
            result.is_ok() && error.is_none(),
            row["accepted"].as_bool().unwrap(),
            "row {index} {action}"
        );
        assert_eq!(json!(result.err().map(reason_id)), row["reason"], "row {index} {action}");
        assert_eq!(json!(error), row["error"], "row {index} {action}");
        assert_eq!(source_state(&state), row["expected"], "row {index} {action}");
    }
}
