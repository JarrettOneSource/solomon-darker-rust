//! Source chat boundaries, accepted histories, durable party rows and ally projections.
use serde_json::Value;
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::profile::WizardElement;
use solomon_sim::social::*;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/social-goldens.json")).unwrap()
}

fn rows(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap()
}

fn channel(value: &Value) -> GameChatChannel {
    match text(value) {
        "party" => GameChatChannel::Party,
        "boneyard" => GameChatChannel::Boneyard,
        "global" => GameChatChannel::Global,
        "whisper" => GameChatChannel::Whisper,
        other => panic!("source channel {other}"),
    }
}

fn session_kind(value: &Value) -> GameSessionKind {
    match text(value) {
        "standalone" => GameSessionKind::Standalone,
        "private-college" => GameSessionKind::PrivateCollege,
        "global-hub" => GameSessionKind::GlobalHub,
        other => panic!("source session {other}"),
    }
}

fn world(value: &Value) -> GameChatWorldKind {
    match text(value) {
        "hub" => GameChatWorldKind::Hub,
        "boneyard" => GameChatWorldKind::Boneyard,
        other => panic!("source world {other}"),
    }
}

fn element(value: &Value) -> WizardElement {
    match text(value) {
        "air" => WizardElement::Air,
        "earth" => WizardElement::Earth,
        "ether" => WizardElement::Ether,
        "fire" => WizardElement::Fire,
        "water" => WizardElement::Water,
        other => panic!("source element {other}"),
    }
}

fn membership(value: &Value) -> PartyMembership {
    PartyMembership {
        enhanced_effects: true,
        id: text(&value["id"]).to_string(),
        join_code: value["joinCode"].as_str().map(String::from),
        leader_player_id: text(&value["leaderPlayerId"]).to_string(),
        listing_id: value["listingId"].as_str().map(String::from),
        member_player_ids: rows(&value["memberPlayerIds"])
            .iter()
            .map(|id| text(id).to_string())
            .collect(),
        visibility: match text(&value["visibility"]) {
            "private" => PartyVisibility::Private,
            "invite-only" => PartyVisibility::InviteOnly,
            "public" => PartyVisibility::Public,
            other => panic!("source visibility {other}"),
        },
    }
}

fn roster_player(value: &Value) -> PartyRosterPlayer {
    PartyRosterPlayer {
        connected: value["connected"].as_bool().unwrap(),
        current_health: value["currentHealth"].as_f64().unwrap(),
        display_name: text(&value["displayName"]).to_string(),
        element: element(&value["element"]),
        life_state: match text(&value["lifeState"]) {
            "alive" => PlayerLifeState::Alive,
            "lethal-pending" => PlayerLifeState::LethalPending,
            "dying" => PlayerLifeState::Dying,
            "spectating" => PlayerLifeState::Spectating,
            other => panic!("source life state {other}"),
        },
        maximum_health: value["maximumHealth"].as_f64().unwrap(),
        player_id: text(&value["playerId"]).to_string(),
    }
}

fn message(sequence: u64) -> GameChatMessage {
    GameChatMessage {
        activity: None,
        channel: GameChatChannel::Party,
        recipient: None,
        sender: GameChatSender {
            display_name: "Felix".to_string(),
            player_id: "player-b".to_string(),
            player_reference: "native:b".to_string(),
        },
        sequence,
        text: format!("message {sequence}"),
    }
}

#[test]
fn chat_text_channels_history_and_rate_boundaries_replay() {
    let fixture = fixture();
    for case in rows(&fixture["normalize"]) {
        let result = normalize_game_chat_text(text(&case["input"]));
        if let Some(expected) = case["text"].as_str() {
            assert_eq!(result.unwrap(), expected);
        } else {
            assert_eq!(result.unwrap_err().text(), text(&case["error"]));
        }
    }
    let membership = membership(&fixture["membership"]);
    for case in rows(&fixture["channels"]) {
        let count = case["memberCount"].as_u64().unwrap() as usize;
        let mut party = LocalPartyState {
            hub_players: Vec::new(),
            invitations: Vec::new(),
            join_requests: Vec::new(),
            party: membership.clone(),
            party_roster: Vec::new(),
            revision: 0,
        };
        party.party.member_player_ids = (0..count).map(|id| format!("player-{id}")).collect();
        let actual = available_game_chat_channels(
            world(&case["world"]),
            (count > 0).then_some(&party),
            session_kind(&case["sessionKind"]) != GameSessionKind::Standalone,
            case["whisper"].as_bool().unwrap(),
        );
        let expected: Vec<_> = rows(&case["available"]).iter().map(channel).collect();
        assert_eq!(actual, expected);
        for next in rows(&case["next"]) {
            assert_eq!(next_game_chat_channel(channel(&next[0]), &actual), channel(&next[1]));
        }
    }
    let mut history = Vec::new();
    for case in rows(&fixture["historySteps"]) {
        assert_eq!(
            append_game_chat_message(&mut history, message(case["sequence"].as_u64().unwrap())),
            case["changed"].as_bool().unwrap()
        );
        let expected: Vec<_> =
            rows(&case["sequences"]).iter().map(|sequence| sequence.as_u64().unwrap()).collect();
        let actual: Vec<_> = history.iter().map(|message| message.sequence).collect();
        assert_eq!(actual, expected);
    }
    for case in rows(&fixture["unread"]) {
        let mut message = message(1);
        message.sender.player_id = text(&case["sender"]).to_string();
        message.channel = channel(&case["channel"]);
        assert_eq!(
            should_increment_game_chat_unread(
                &message,
                "player-a",
                case["open"].as_bool().unwrap(),
                channel(&case["current"])
            ),
            case["value"].as_bool().unwrap()
        );
    }
    for case in rows(&fixture["fade"]) {
        assert_eq!(
            is_game_chat_faded(
                case["open"].as_bool().unwrap(),
                case["last"].as_f64().unwrap(),
                case["now"].as_f64().unwrap()
            ),
            case["value"].as_bool().unwrap()
        );
    }
    for case in rows(&fixture["rejections"]) {
        let rejection = GameChatRejection {
            channel: channel(&case["channel"]),
            reason: match text(&case["reason"]) {
                "channel-unavailable" => GameChatRejectionReason::ChannelUnavailable,
                "rate-limited" => GameChatRejectionReason::RateLimited,
                "target-unavailable" => GameChatRejectionReason::TargetUnavailable,
                other => panic!("source rejection {other}"),
            },
            retry_after_ms: case["retryAfterMs"].as_u64().unwrap(),
        };
        assert_eq!(rejection.text(), text(&case["text"]));
    }
    for journey in rows(&fixture["rateLimits"]) {
        let mut limiter = GameChatRateLimiter::default();
        for case in rows(journey) {
            assert_eq!(
                limiter.record(case["now"].as_u64().unwrap()),
                case["retry"].as_u64().unwrap()
            );
        }
    }
}

#[test]
fn party_projection_departure_and_kick_admission_replay() {
    let fixture = fixture();
    let party = membership(&fixture["membership"]);
    let profiles: Vec<_> = rows(&fixture["profiles"])
        .iter()
        .map(|profile| PartyPlayerProfile {
            account_username: profile["accountUsername"].as_str().map(String::from),
            highest_wave: profile["highestWave"].as_u64(),
            total_playtime_ms: profile["totalPlaytimeMs"].as_u64(),
            display_name: text(&profile["displayName"]).to_string(),
            player_id: text(&profile["playerId"]).to_string(),
        })
        .collect();
    let roster: Vec<_> = rows(&fixture["roster"]).iter().map(roster_player).collect();
    for case in rows(&fixture["projections"]) {
        let actual = project_direct_party_state(&party, &profiles, &roster, 13).unwrap();
        let expected = &case["value"];
        assert_eq!(actual.party, membership(&expected["party"]));
        assert_eq!(actual.revision, expected["revision"].as_u64().unwrap());
        let actual_profiles: Vec<_> =
            actual.hub_players.iter().map(|player| player.player_id.as_str()).collect();
        let expected_profiles: Vec<_> =
            rows(&expected["hubPlayers"]).iter().map(|player| text(&player["playerId"])).collect();
        assert_eq!(actual_profiles, expected_profiles);
        assert_eq!(
            actual.party_roster,
            rows(&expected["partyRoster"]).iter().map(roster_player).collect::<Vec<_>>()
        );
        for member in rows(&case["menu"]["members"]) {
            assert_eq!(
                party_member_display_name(&actual, text(&member["id"])),
                text(&member["name"])
            );
        }
    }
    assert_eq!(
        project_direct_party_state(&party, &profiles, &[], 13),
        Err(MissingPartyRosterPlayer("player-b".to_string()))
    );
    for case in rows(&fixture["kicks"]) {
        let expected = match case["reason"].as_str() {
            None => None,
            Some("party-missing") => Some(PartyActionRejection::PartyMissing),
            Some("not-leader") => Some(PartyActionRejection::NotLeader),
            Some("self-kick") => Some(PartyActionRejection::SelfKick),
            Some("player-missing") => Some(PartyActionRejection::PlayerMissing),
            other => panic!("source kick {other:?}"),
        };
        assert_eq!(
            party_kick_rejection(Some(&party), text(&case["requester"]), text(&case["target"])),
            expected
        );
    }
    for case in rows(&fixture["departures"]) {
        let mut party = party.clone();
        party.member_player_ids =
            rows(&case["ids"]).iter().map(|id| text(id).to_string()).collect();
        let expected = (!case["party"].is_null()).then(|| membership(&case["party"]));
        assert_eq!(party_after_member_departure(&party, text(&case["playerId"])), expected);
    }
    for case in rows(&fixture["leaveLabels"]) {
        assert_eq!(
            party_menu_leave_label(
                case["count"].as_u64().unwrap() as usize,
                session_kind(&case["kind"])
            ),
            case["label"].as_str()
        );
    }
}

#[test]
fn ally_rows_preserve_disconnected_identity_health_and_source_order() {
    let fixture = fixture();
    let mut players = Vec::new();
    for (id, player) in fixture["players"].as_object().unwrap() {
        let value = serde_json::json!({
            "playerId":id,"connected":true,"displayName":player["config"]["displayName"],
            "element":player["config"]["element"],"currentHealth":player["progression"]["currentHealth"],
            "maximumHealth":player["progression"]["maximumHealth"],"lifeState":player["progression"]["lifeState"],
        });
        players.push(roster_player(&value));
    }
    for case in rows(&fixture["playerAllies"]) {
        let retained = case["retained"]
            .as_array()
            .map(|rows| rows.iter().map(roster_player).collect::<Vec<_>>());
        let actual =
            derive_player_ally_hud_rows(&players, text(&case["local"]), retained.as_deref());
        let expected = rows(&case["rows"]);
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.id, text(&expected["id"]));
            assert_eq!(actual.connected, expected["connected"].as_bool().unwrap());
            assert_eq!(actual.dead, expected["dead"].as_bool().unwrap());
            assert_eq!(actual.health_ratio, expected["healthRatio"].as_f64().unwrap());
            assert_eq!(actual.accessible_status(), text(&expected["status"]));
            assert_eq!(
                actual.identity,
                AllyHudIdentity::Player {
                    display_name: text(&expected["identity"]["displayName"]).to_string(),
                    element: element(&expected["identity"]["element"]),
                }
            );
        }
    }
    let mut golems = Vec::new();
    for actor in rows(&fixture["golemActors"]) {
        if actor["kind"] == "golem" && !actor["golem"].is_null() {
            golems.push(AllyGolemProjection {
                id: actor["id"].as_u64().unwrap(),
                world_key: text(&actor["worldKey"]),
                current_health: actor["golem"]["currentHealth"].as_f64().unwrap(),
                maximum_health: actor["golem"]["maximumHealth"].as_f64().unwrap(),
            });
        }
    }
    for case in rows(&fixture["golems"]) {
        let actual = derive_golem_ally_hud_rows(&golems, text(&case["world"]));
        let expected = rows(&case["rows"]);
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.id, text(&expected["id"]));
            assert_eq!(actual.health_ratio, expected["healthRatio"].as_f64().unwrap());
            assert_eq!(actual.identity, AllyHudIdentity::Golem);
            assert!(actual.connected && !actual.dead);
        }
    }
}
