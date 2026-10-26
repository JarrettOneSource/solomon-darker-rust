//! Pure operator activity from Source `host/game-activity-events.ts`.
//! These compact before/after facts never retain or advance another Session.

use serde::{Deserialize, Serialize};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::modding::LuaObject;
use solomon_sim::session::GameSession;
use solomon_sim::social::{PartyJoinRequester, PartyVisibility, SocialAction};

use super::{HostedGameHost, HostedWorlds};

pub(super) struct RuntimeActivityEpoch {
    party_id: Option<String>,
    before: GameActivitySnapshot,
    boneyard: Option<serde_json::Value>,
}

pub(in crate::host) struct PartyActivityBefore {
    party_id: Option<String>,
    visibility: Option<PartyVisibility>,
    invitation: Option<InvitationActivity>,
    request: Option<(String, PartyJoinRequester)>,
}

struct InvitationActivity {
    id: String,
    party_id: String,
    inviter_id: String,
}

impl HostedGameHost {
    pub(in crate::host) fn capture_party_activity(
        &self,
        id: &str,
        action: &SocialAction,
    ) -> PartyActivityBefore {
        let party = self.worlds.parties().party_for_player(id);
        let invitation_id = match action {
            SocialAction::AcceptInvitation { invitation_id }
            | SocialAction::DenyInvitation { invitation_id } => Some(invitation_id),
            _ => None,
        };
        let invitation = invitation_id
            .and_then(|id| self.worlds.parties().invitations.iter().find(|row| &row.id == id))
            .map(|row| InvitationActivity {
                id: row.id.clone(),
                party_id: row.party_id.clone(),
                inviter_id: row.inviter_player_id.clone(),
            });
        let request = match action {
            SocialAction::DecideJoinRequest { request_id, .. } => self
                .external_join_requests
                .iter()
                .find(|row| &row.id == request_id)
                .map(|row| (row.party_id.clone(), row.requester.clone())),
            _ => None,
        };
        PartyActivityBefore {
            party_id: party.map(|party| party.id.clone()),
            visibility: party.map(|party| party.visibility),
            invitation,
            request,
        }
    }

    fn activity_player(&self, id: &str) -> serde_json::Value {
        let metadata = self
            .player(id)
            .map(|player| &player.profile)
            .or_else(|| self.bots.rows.iter().find(|bot| bot.id == id).map(|bot| &bot.profile));
        serde_json::json!({"accountUsername":metadata.and_then(|profile| profile.account_username.as_ref()),
            "displayName":metadata.and_then(|_|self.retained_participant(id))
                .map(|participant|participant.player.selection.name.as_str()).unwrap_or(id),"playerId":id})
    }

    pub(in crate::host) fn emit_party_activity(
        &self,
        event: &str,
        message: &str,
        party_id: Option<&str>,
        details: serde_json::Value,
    ) {
        if self.runtime_events.is_none() {
            return;
        }
        let party = party_id
            .and_then(|id| self.worlds.parties().parties.iter().find(|party| party.id == id));
        let mut context = serde_json::json!({"partyId":party_id,
            "partyLeader":party.map(|party|self.activity_player(&party.leader_player_id)),
            "partyMemberCount":party.map(|party|party.member_player_ids.len()),
            "partyMembers":party.map(|party|party.member_player_ids.iter().map(|id|self.activity_player(id)).collect::<Vec<_>>()).unwrap_or_default(),
            "partyVisibility":party.map(|party|visibility(party.visibility))});
        context
            .as_object_mut()
            .unwrap()
            .extend(details.as_object().expect("party event details are an object").clone());
        self.emit_runtime_event(event, message, context);
    }

    pub(in crate::host) fn emit_party_action(
        &self,
        id: &str,
        action: &SocialAction,
        before: PartyActivityBefore,
    ) {
        match action {
            SocialAction::Invite {player_id} => {
                if let Some(invitation)=self.worlds.parties().invitations.iter().find(|row|row.inviter_player_id==id&& &row.invited_player_id==player_id) {
                    self.emit_party_activity("party.invitation_sent","A player invited someone to a party.",Some(&invitation.party_id),
                        serde_json::json!({"invitationId":invitation.id,"invited":self.activity_player(player_id),"inviter":self.activity_player(id)}));
                }
            }
            SocialAction::AcceptInvitation {..} | SocialAction::DenyInvitation {..} => {
                if let Some(invitation)=before.invitation {
                    let accept=matches!(action,SocialAction::AcceptInvitation {..});
                    self.emit_party_activity(if accept {"party.invitation_accepted"} else {"party.invitation_denied"},
                        if accept {"A player accepted a party invitation."} else {"A player denied a party invitation."},Some(&invitation.party_id),
                        serde_json::json!({"invitationId":invitation.id,"invited":self.activity_player(id),"inviter":self.activity_player(&invitation.inviter_id)}));
                }
            }
            SocialAction::SetPartyVisibility {visibility:enabled} => self.emit_party_activity("party.visibility_changed",
                "A party leader changed party visibility.",before.party_id.as_deref(),serde_json::json!({"actor":self.activity_player(id),
                    "previousVisibility":before.visibility.map(visibility),"visibility":visibility(*enabled)})),
            SocialAction::RotatePartyCode => self.emit_party_activity("party.join_code_rotated",
                "A party leader rotated the private Party ID.",before.party_id.as_deref(),serde_json::json!({"actor":self.activity_player(id)})),
            SocialAction::DecideJoinRequest {request_id,accept} => {
                let party_id=before.request.as_ref().map(|(party,_)|party.as_str()).or(before.party_id.as_deref());
                self.emit_party_activity(if *accept {"party.join_request_accepted"} else {"party.join_request_denied"},
                    if *accept {"A party leader accepted a join request."} else {"A party leader denied a join request."},party_id,
                    serde_json::json!({"actor":self.activity_player(id),"requestId":request_id,
                        "requester":before.request.as_ref().map(|(_,requester)|requester_activity(requester))}));
            }
            SocialAction::LeaveParty if matches!(self.worlds,HostedWorlds::Global(_)) => self.emit_party_activity("party.member_left",
                "A player left a party.",before.party_id.as_deref(),serde_json::json!({"player":self.activity_player(id)})),
            SocialAction::Kick {player_id} => self.emit_party_activity("party.member_kicked",
                "A party leader removed a player.",before.party_id.as_deref(),serde_json::json!({"actor":self.activity_player(id),"removedPlayer":self.activity_player(player_id)})),
            _ => {},
        }
    }
    pub(in crate::host) fn emit_player_connected(
        &self,
        id: &str,
        selection: &solomon_sim::profile::NewGameSelection,
        rejoined_party: bool,
        replaced_connection: bool,
    ) {
        let Some(player) = self.player(id) else { return };
        let party = self.worlds.parties().party_for_player(id);
        self.emit_runtime_event(
            "player.connected",
            "A player authenticated with the game host.",
            serde_json::json!({"accountUsername":player.profile.account_username,
                "discipline":selection.discipline.as_str(),"displayName":selection.name,
                "element":selection.element.as_str(),"partyId":party.map(|party| &party.id),
                "partyMemberCount":party.map(|party| party.member_player_ids.len()),
                "playerId":id,"playerCount":self.players.len(),"rejoinedParty":rejoined_party,
                "replacedConnection":replaced_connection,"role":"shared"}),
        );
    }
    pub(in crate::host) fn emit_runtime_event<T: Serialize>(
        &self,
        event: &str,
        message: &str,
        details: T,
    ) {
        let Some(sink) = &self.runtime_events else { return };
        let details: LuaObject = serde_json::from_value(
            serde_json::to_value(details).expect("accepted runtime event values serialize"),
        )
        .expect("runtime event details are a finite object");
        let mut context: LuaObject = [
            ("sessionId".into(), solomon_sim::modding::LuaValue::String(self.id.clone())),
            (
                "sessionKind".into(),
                solomon_sim::modding::LuaValue::String(
                    if matches!(self.worlds, HostedWorlds::Global(_)) { "hub" } else { "private" }
                        .into(),
                ),
            ),
        ]
        .into_iter()
        .collect();
        for (key, value) in details.into_fields() {
            context.insert(key, value);
        }
        sink.publish(super::super::runtime_events::RuntimeEventEntry {
            component: "game-host".into(),
            details: context,
            event: event.into(),
            message: message.into(),
            occurred_at_utc: Some(
                crate::host::source_utc_timestamp(time::OffsetDateTime::now_utc())
                    .expect("current Source UTC time is representable"),
            ),
        });
    }

    pub(super) fn capture_runtime_activity(&mut self) {
        self.runtime_activity.clear();
        if self.runtime_events.is_none() {
            return;
        }
        match &self.worlds {
            HostedWorlds::Global(worlds) => {
                for run in &worlds.runs {
                    self.runtime_activity.push(RuntimeActivityEpoch {
                        party_id: Some(run.party_id.clone()),
                        before: project_game_activity(&run.state),
                        boneyard: Some(boneyard_activity(&run.loaded_boneyard)),
                    });
                }
            }
            HostedWorlds::Private { state, .. } => {
                self.runtime_activity.push(RuntimeActivityEpoch {
                    party_id: None,
                    before: project_game_activity(state),
                    boneyard: state.boneyard.as_ref().map(|world| boneyard_activity(&world.loaded)),
                })
            }
        }
    }

    pub(in crate::host) fn publish_runtime_activity(&mut self) {
        for epoch in std::mem::take(&mut self.runtime_activity) {
            let current = match (&self.worlds, &epoch.party_id) {
                (HostedWorlds::Global(worlds), Some(id)) => {
                    worlds.runs.iter().find(|run| &run.party_id == id).map(|run| &run.state)
                }
                (HostedWorlds::Private { state, .. }, None) => Some(state),
                _ => None,
            };
            if let Some(current) = current {
                self.emit_game_activity(
                    &epoch.before,
                    current,
                    epoch.boneyard,
                    epoch.party_id.as_deref(),
                );
            }
        }
    }

    pub(super) fn emit_game_activity(
        &self,
        previous: &GameActivitySnapshot,
        current: &GameSession,
        boneyard: Option<serde_json::Value>,
        party_id: Option<&str>,
    ) {
        if self.runtime_events.is_none() {
            return;
        }
        for activity in derive_game_activity_events(previous, &project_game_activity(current)) {
            let mut details = serde_json::json!({
                "boneyardId":boneyard.as_ref().and_then(|value| value.get("id")),
                "boneyardName":boneyard.as_ref().and_then(|value| value.get("name")),
                "boneyardSource":boneyard.as_ref().and_then(|value| value.get("source")),
                "partyId":party_id,
            });
            let fields = details.as_object_mut().expect("game activity context is an object");
            for (key, value) in activity.details.into_fields() {
                fields.insert(key, serde_json::to_value(value).expect("finite accepted activity"));
            }
            self.emit_runtime_event(activity.event, activity.message, details);
        }
    }
}

pub(super) fn requester_activity(requester: &PartyJoinRequester) -> serde_json::Value {
    serde_json::json!({"accountUsername":requester.account_username,
        "displayName":requester.display_name,"requesterId":requester.requester_id})
}

fn visibility(value: PartyVisibility) -> &'static str {
    match value {
        PartyVisibility::Public => "public",
        PartyVisibility::InviteOnly => "invite-only",
        PartyVisibility::Private => "private",
    }
}

pub(super) fn boneyard_activity(
    loaded: &solomon_sim::boneyard::LoadedBoneyard,
) -> serde_json::Value {
    serde_json::json!({ "id":loaded.choice_id, "name":loaded.name,
        "source":match loaded.choice_source {
            solomon_sim::boneyard::BoneyardSource::Default => "default",
            solomon_sim::boneyard::BoneyardSource::Mod => "mod",
        },
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GameActivityPlayer {
    current_health: f64,
    death_epoch: u64,
    discipline: String,
    display_name: String,
    element: String,
    level: u32,
    life_state: String,
    maximum_health: f64,
    player_id: String,
    x: f64,
    y: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct GameActivityWave {
    ordinal: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GameActivitySnapshot {
    phase: String,
    players: Vec<GameActivityPlayer>,
    run_id: Option<String>,
    tick: u64,
    wave: Option<GameActivityWave>,
}

#[derive(Serialize)]
pub(super) struct GameActivityEvent {
    pub details: LuaObject,
    pub event: &'static str,
    pub message: &'static str,
}

pub(super) fn project_game_activity(state: &GameSession) -> GameActivitySnapshot {
    GameActivitySnapshot {
        phase: state.run.phase.id().into(),
        run_id: state.run.run_id.clone(),
        tick: state.ticks,
        wave: state
            .boneyard
            .as_ref()
            .and_then(|world| world.waves.as_ref())
            .map(|waves| GameActivityWave { ordinal: waves.state.wave_ordinal }),
        players: state
            .participant_order
            .iter()
            .map(|id| {
                let participant = &state.participants.get(id).expect("participant exists");
                let player = &participant.player;
                GameActivityPlayer {
                    current_health: player.combat.current_health,
                    death_epoch: player.combat.death_epoch,
                    discipline: player.selection.discipline.as_str().into(),
                    display_name: player.selection.name.clone(),
                    element: player.selection.element.as_str().into(),
                    level: player.progression.level,
                    life_state: match player.combat.life_state {
                        PlayerLifeState::Alive => "alive",
                        PlayerLifeState::LethalPending => "lethal-pending",
                        PlayerLifeState::Dying => "dying",
                        PlayerLifeState::Spectating => "spectating",
                    }
                    .into(),
                    maximum_health: player.combat.maximum_health,
                    player_id: id.clone(),
                    x: participant.wizard.position.x,
                    y: participant.wizard.position.y,
                }
            })
            .collect(),
    }
}

pub(super) fn derive_game_activity_events(
    previous: &GameActivitySnapshot,
    current: &GameActivitySnapshot,
) -> Vec<GameActivityEvent> {
    let mut events = Vec::new();
    if previous.run_id != current.run_id && current.run_id.is_some() {
        events.push(event(
            "run.started",
            "A party started a Boneyard run.",
            serde_json::json!({
                "playerCount":current.players.len(), "players":current.players,
                "runId":current.run_id, "serverTick":current.tick,
            }),
        ));
    }
    for player in &current.players {
        let Some(before) =
            previous.players.iter().find(|before| before.player_id == player.player_id)
        else {
            continue;
        };
        if player.death_epoch > before.death_epoch {
            let mut details =
                serde_json::to_value(player).expect("finite accepted player activity");
            let source = details.as_object_mut().expect("player activity is an object");
            source.insert("runId".into(), serde_json::json!(current.run_id));
            source.insert("serverTick".into(), serde_json::json!(current.tick));
            source.insert(
                "wave".into(),
                serde_json::json!(current.wave.as_ref().map(|wave| wave.ordinal)),
            );
            events.push(event("player.died", "A player died in the Boneyard.", details));
        }
    }
    if previous.phase == "active" && current.phase == "game-over" {
        events.push(event("run.game_over", "A Boneyard run reached Game Over.", serde_json::json!({
            "playerCount":current.players.len(), "players":current.players,
            "runId":current.run_id, "serverTick":current.tick, "wave":current.wave.as_ref().map(|wave| wave.ordinal),
        })));
    }
    if previous.run_id.is_some() && current.run_id.is_none() {
        events.push(event("run.ended", "A Boneyard run ended.", serde_json::json!({
            "playerCount":current.players.len(), "reason":if previous.phase == "game-over" {"game-over"} else {"ended"},
            "runId":previous.run_id, "serverTick":current.tick, "wave":previous.wave.as_ref().map(|wave| wave.ordinal),
        })));
    }
    events
}

fn event(
    event: &'static str,
    message: &'static str,
    details: serde_json::Value,
) -> GameActivityEvent {
    GameActivityEvent {
        event,
        message,
        details: serde_json::from_value(details).expect("finite accepted game activity"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::modding::LuaValue;
    use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
    use solomon_sim::session::GameSessionOptions;

    #[test]
    fn operator_game_activity_uses_actual_source_identity_and_epoch_edges() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../sim/tests/fixtures/website/game-activity-goldens.json"
        ))
        .unwrap();
        let state = GameSession::new_participants(
            vec![
                (
                    "zeta".into(),
                    NewGameSelection {
                        name: "Aurelia".into(),
                        discipline: WizardDiscipline::Arcane,
                        element: WizardElement::Fire,
                    },
                ),
                (
                    "alpha".into(),
                    NewGameSelection {
                        name: "Octavia".into(),
                        discipline: WizardDiscipline::Body,
                        element: WizardElement::Water,
                    },
                ),
            ],
            GameSessionOptions {
                gameplay_rng_seed: 17,
                combat_rng_seed: 91,
                ..GameSessionOptions::default()
            },
        )
        .unwrap();
        let actual: LuaValue =
            serde_json::from_value(serde_json::to_value(project_game_activity(&state)).unwrap())
                .unwrap();
        let expected: LuaValue = serde_json::from_value(fixture["projected"].clone()).unwrap();
        assert_eq!(actual, expected);
        for case in fixture["cases"].as_array().unwrap() {
            let previous = serde_json::from_value(case["previous"].clone()).unwrap();
            let current = serde_json::from_value(case["current"].clone()).unwrap();
            let actual: LuaValue = serde_json::from_value(
                serde_json::to_value(derive_game_activity_events(&previous, &current)).unwrap(),
            )
            .unwrap();
            let expected: LuaValue = serde_json::from_value(case["events"].clone()).unwrap();
            assert_eq!(actual, expected, "{}", case["name"]);
        }
    }
}
