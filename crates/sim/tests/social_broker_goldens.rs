//! Actual source broker delivery transactions, including connection replacement.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solomon_sim::profile::{WizardDiscipline, WizardElement};
use solomon_sim::social::broker::*;
use solomon_sim::social::{
    GameChatActivity, GameChatChannel, GameChatSender, GameChatWorldKind, GameOnlinePreferences,
    GamePlayerCardProfile, GameSessionKind,
};

struct Fact {
    host: String,
    local: String,
    who: String,
    name: String,
    available: bool,
    profile_null: bool,
    bad_reference: bool,
}

#[derive(Default)]
struct Worlds(BTreeMap<String, Fact>);

impl Worlds {
    fn fact(&self, participant: &GameSocialParticipant) -> &Fact {
        self.0
            .values()
            .find(|fact| {
                fact.host == participant.host_id && fact.local == participant.local_player_id
            })
            .unwrap()
    }
}

impl GameSocialWorlds for Worlds {
    fn profile(&self, participant: &GameSocialParticipant) -> Option<GamePlayerCardProfile> {
        let fact = self.fact(participant);
        if fact.profile_null {
            return None;
        }
        Some(GamePlayerCardProfile {
            player_id: fact.local.clone(),
            account_username: Some(format!("account-{}", fact.who)),
            activity: if fact.available {
                GameChatWorldKind::Hub
            } else {
                GameChatWorldKind::Boneyard
            },
            discipline: WizardDiscipline::Arcane,
            display_name: fact.name.clone(),
            element: WizardElement::Fire,
            gold: 37,
            highest_wave: Some(4),
            player_reference: reference(if fact.bad_reference { "z" } else { &fact.who }),
            session_kind: if fact.host == "college-a" {
                GameSessionKind::GlobalHub
            } else {
                GameSessionKind::PrivateCollege
            },
            total_playtime_ms: Some(9_000),
        })
    }

    fn can_receive_college_invitation(&self, participant: &GameSocialParticipant) -> bool {
        self.fact(participant).available
    }
}

fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    row[key].as_str().unwrap()
}
fn reference(who: &str) -> String {
    format!("player-ref-{}", who.repeat(32))
}
fn preferences(value: &Value) -> GameOnlinePreferences {
    GameOnlinePreferences {
        activity_messages: value["activityMessages"].as_bool().unwrap(),
        global_chat: value["globalChat"].as_bool().unwrap(),
        submit_runs: value["submitRuns"].as_bool().unwrap(),
    }
}
fn sender(value: &GameChatSender) -> Value {
    json!({"displayName":value.display_name,"playerId":value.player_id,"playerReference":value.player_reference})
}
fn profile(value: GamePlayerCardProfile) -> Value {
    json!({"accountUsername":value.account_username,
        "activity":if value.activity == GameChatWorldKind::Hub {"hub"} else {"boneyard"},
        "discipline":value.discipline.as_str(),"displayName":value.display_name,
        "element":value.element.as_str(),"gold":value.gold,"highestWave":value.highest_wave,
        "playerReference":value.player_reference,
        "sessionKind":if value.session_kind == GameSessionKind::GlobalHub {"global-hub"} else {"private-college"},
        "totalPlaytimeMs":value.total_playtime_ms})
}

fn deliveries(
    broker: &mut GameSocialBroker,
    connections: &BTreeMap<String, GameSocialConnection>,
) -> Value {
    let mut rows = Vec::new();
    for delivery in broker.take_deliveries() {
        let (connection, mut row) = match delivery {
            GameSocialDelivery::CollegeInvitations { connection, invitations } => {
                let invitations: Vec<_> = invitations
                    .iter()
                    .map(|invitation| {
                        json!({
                            "expiresAtUnixMs":invitation.expires_at_unix_ms,"id":invitation.id,
                            "inviter":sender(&invitation.inviter),"joinCode":invitation.join_code,
                        })
                    })
                    .collect();
                (connection, json!({"invitations":invitations}))
            }
            GameSocialDelivery::Chat { connection, message } => {
                let mut chat = json!({"channel":if message.channel == GameChatChannel::Global {"global"} else {"whisper"},
                    "deliveryId":message.delivery_id,"sender":sender(&message.sender),"text":message.text});
                if let Some(recipient) = message.recipient {
                    chat["recipient"] = sender(&recipient);
                }
                if let Some(activity) = message.activity {
                    chat["activity"] = json!(match activity {
                        GameChatActivity::EnteredCollege => "entered-college",
                        GameChatActivity::SearchingSolomon => "searching-solomon",
                        GameChatActivity::LeftGame => "left-game",
                    });
                }
                (connection, json!({"chat":chat}))
            }
        };
        let name = connections.iter().find(|(_, candidate)| **candidate == connection).unwrap().0;
        row["connection"] = json!(name);
        rows.push(row);
    }
    json!(rows)
}

#[test]
fn broker_delivers_pinned_cross_college_chat_and_invitation_lifecycles() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/social-broker-goldens.json")).unwrap();
    let mut broker = GameSocialBroker::default();
    let mut worlds = Worlds::default();
    let mut connections = BTreeMap::new();
    let mut next_invitation_id = 1;
    let mut now = 1_000;
    for (index, row) in fixture["rows"].as_array().unwrap().iter().enumerate() {
        let input = &row["input"];
        let current = input["connection"].as_str().and_then(|name| connections.get(name)).cloned();
        let mut result = Value::Null;
        let mut error = None;
        match text(row, "action") {
            "register" => {
                let who = text(input, "who");
                worlds.0.entry(who.into()).or_insert_with(|| Fact {
                    who: who.into(),
                    host: text(input, "host").into(),
                    local: text(input, "local").into(),
                    name: format!("Wizard {who}"),
                    available: true,
                    profile_null: false,
                    bad_reference: false,
                });
                let identity =
                    input["reference"].as_str().map_or_else(|| reference(who), str::to_owned);
                match broker.register(
                    GameSocialParticipant {
                        host_id: text(input, "host").into(),
                        local_player_id: text(input, "local").into(),
                    },
                    GameOnlinePreferences::default(),
                    identity,
                ) {
                    Ok(connection) => {
                        connections.insert(text(input, "connection").into(), connection);
                    }
                    Err(reason) => error = Some(reason),
                }
            }
            "activate" => broker.activate(current.as_ref().unwrap()),
            "close" => broker.close_connection(current.as_ref().unwrap()),
            "global" => {
                result = json!(broker.publish_global(
                    current.as_ref().unwrap(),
                    text(input, "text"),
                    &worlds
                ))
            }
            "whisper" => {
                result = json!(broker.publish_whisper(
                    current.as_ref().unwrap(),
                    &reference(text(input, "target")),
                    text(input, "text"),
                    &worlds
                ))
            }
            "activity" => broker.publish_activity(
                current.as_ref().unwrap(),
                match text(input, "activity") {
                    "entered-college" => GameChatActivity::EnteredCollege,
                    "searching-solomon" => GameChatActivity::SearchingSolomon,
                    "left-game" => GameChatActivity::LeftGame,
                    _ => unreachable!(),
                },
                &worlds,
            ),
            "preferences" => broker.set_online_preferences(
                current.as_ref().unwrap(),
                preferences(&input["preferences"]),
            ),
            "invite" => {
                let target = reference(text(input, "target"));
                let request = CollegeInvitationRequest {
                    target_reference: &target,
                    source_party_id: text(input, "party"),
                    join_code: text(input, "code"),
                    now_unix_ms: now,
                };
                let accepted = broker.invite_to_college(
                    current.as_ref().unwrap(),
                    request,
                    &mut || {
                        let id = format!("invitation-{next_invitation_id}");
                        next_invitation_id += 1;
                        id
                    },
                    &worlds,
                );
                result = json!(accepted.err().map(|reason| reason.id()));
            }
            "dismiss" => broker
                .dismiss_college_invitation(current.as_ref().unwrap(), text(input, "invitation")),
            "revoke" => {
                broker.revoke_college_invitations(current.as_ref().unwrap(), text(input, "party"))
            }
            "refresh" => {
                broker.refresh_college_invitation_availability(current.as_ref().unwrap(), &worlds)
            }
            "card" => {
                result = broker
                    .resolve_player_card(
                        current.as_ref().unwrap(),
                        &reference(text(input, "target")),
                        &worlds,
                    )
                    .map_or(Value::Null, profile)
            }
            "facts" => {
                let fact = worlds.0.get_mut(text(input, "who")).unwrap();
                let patch = &input["patch"];
                if let Some(name) = patch["name"].as_str() {
                    fact.name = name.into();
                }
                if let Some(value) = patch["available"].as_bool() {
                    fact.available = value;
                }
                if let Some(value) = patch["profileNull"].as_bool() {
                    fact.profile_null = value;
                }
                if let Some(value) = patch["badReference"].as_bool() {
                    fact.bad_reference = value;
                }
            }
            "prune" => {
                now = input["now"].as_u64().unwrap();
                broker.prune(now);
            }
            "shutdown" => broker.close(),
            action => panic!("unknown broker transaction {action}"),
        }
        assert_eq!(json!(error), row["error"], "broker error {index}:{}", row["action"]);
        assert_eq!(result, row["result"], "broker result {index}:{}", row["action"]);
        assert_eq!(
            deliveries(&mut broker, &connections),
            row["deliveries"],
            "broker deliveries {index}:{}",
            row["action"]
        );
    }
}
