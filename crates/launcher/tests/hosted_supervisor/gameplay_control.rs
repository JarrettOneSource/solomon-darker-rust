//! Report 103's solo policy at both hosted admission surfaces, with unrelated
//! Global College and active-run clients retained as actual connected peers.

use super::*;
use solomon_protocol::{PauseSource, SnapshotFrame};

fn frame(
    client: &mut Client,
    accept: impl Fn(&SnapshotFrame<Value>) -> bool,
) -> SnapshotFrame<Value> {
    let message = client
        .until(|message| matches!(message, ServerMessage::Snapshot { frame } if accept(frame)));
    let ServerMessage::Snapshot { frame } = message else { unreachable!() };
    frame
}

fn enter_solo_run(client: &mut Client) -> SnapshotFrame<Value> {
    client.send(ClientMessage::Hosted {
        command: HostedCommand::StartMatch { boneyard_id: "default-random".into() },
    });
    let loaded = client.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }));
    let ServerMessage::BoneyardLoaded { boneyard } = loaded else { unreachable!() };
    let loading = frame(client, |frame| {
        frame.host.loading_run_id.as_deref() == Some(boneyard.run_id.as_str())
    });
    assert_eq!(loading.host.resume_grace.as_ref().unwrap().remaining_ms, None);
    client.send(ClientMessage::ReadyBoneyard { run_id: boneyard.run_id });
    client.send(ClientMessage::ResumeReady {
        sequence: loading.host.resume_grace.as_ref().unwrap().sequence,
    });
    let admitted = frame(client, |frame| {
        frame.sequence > loading.sequence
            && frame.host.loading_run_id.is_none()
            && frame.host.resume_grace.as_ref().is_none_or(|grace| grace.remaining_ms.is_some())
    });
    assert!(admitted.host.resume_grace.is_none(), "solo entry must not install a deadline");
    frame(client, |frame| {
        frame.snapshot["ticks"].as_u64().unwrap() > loading.snapshot["ticks"].as_u64().unwrap()
    })
}

fn reserve_join(host: &Host, leader: &mut Client) -> Value {
    let social = leader.until(|message| matches!(message, ServerMessage::Social { .. }));
    let ServerMessage::Social { snapshot } = social else { unreachable!() };
    let party = snapshot.party_state.unwrap().party;
    let (status, intent) =
        host.request("POST", "/admin/join/resolve", json!({"code":party.join_code}), true);
    assert_eq!(status, 201, "{intent}");
    let mut admission = content();
    admission["intentId"] = intent["intentId"].clone();
    admission["activeMods"] = json!(false);
    let (status, reserved) = host.request("POST", "/admin/join/admit", admission, true);
    assert_eq!(status, 201, "{reserved}");
    reserved
}

fn saved_owner(client: &mut Client, request: u64) -> String {
    client.send(ClientMessage::SaveBeforeLeave { request_id: request });
    let receipt = client.until(|message| {
        matches!(message,
        ServerMessage::Hosted { message: HostedServerMessage::SaveBeforeLeave { request_id, .. } }
            if *request_id == request)
    });
    let ServerMessage::Hosted {
        message: HostedServerMessage::SaveBeforeLeave { checkpoint_sequence, .. },
    } = receipt
    else {
        unreachable!()
    };
    let checkpoint = client.until(|message| {
        matches!(message,
        ServerMessage::Hosted { message: HostedServerMessage::SaveCheckpoint { sequence, .. } }
            if *sequence == checkpoint_sequence)
    });
    let ServerMessage::Hosted { message: HostedServerMessage::SaveCheckpoint { save, .. } } =
        checkpoint
    else {
        unreachable!()
    };
    save
}

#[test]
fn report103_party_ticket_rechecks_hub_before_actor_admission_and_preserves_saves() {
    let mut host = Host::start();
    let (_, endpoint) = host.request("POST", "/admin/hub/tickets", content(), true);
    let (mut leader, leader_id, _) = Client::join(&host, &endpoint, "Leader");
    let reserved = reserve_join(&host, &mut leader);
    let running = enter_solo_run(&mut leader);
    let mut late = Client::connect(&host, &reserved);
    late.send(ClientMessage::Hosted {
        command: HostedCommand::Hello {
            hello: HostedHello {
                version: PROTOCOL_VERSION,
                credential: reserved["credential"].as_str().unwrap().into(),
                selection: NewGameSelection {
                    name: "Late Lobby".into(),
                    discipline: WizardDiscipline::Arcane,
                    element: WizardElement::Ether,
                },
                profile: PlayerSocialProfile::default(),
                online_preferences: GameOnlinePreferences::default(),
                resume_token: None,
                save_intent: None,
                save: None,
                begin_college_intro: false,
                decline_tutorial: false,
                cheats_enabled: false,
                enhanced_effects: true,
                allow_mod_mismatch: false,
            },
        },
    });
    let admission = late.until(|message| {
        matches!(message, ServerMessage::Welcome { .. } | ServerMessage::Error { .. })
    });
    let ServerMessage::Error { code, reason } = admission else {
        panic!("an ordinary party ticket must not admit a wizard after combat starts")
    };
    assert_eq!(code, solomon_protocol::HostErrorCode::InvalidCredentials);
    assert_eq!(reason, "That party admission has expired.");
    late.finish_close();
    let (_, presence) = host.request("GET", "/admin/presence", Value::Null, true);
    assert_eq!(presence["items"].as_array().unwrap().len(), 1);
    assert_eq!(presence["items"][0]["displayName"], "Leader");
    let restored = solomon_assets::website_save::restore(&saved_owner(&mut leader, 7)).unwrap();
    assert_eq!(restored.player_id, leader_id);
    assert_eq!(restored.session.participant_order, [leader_id]);
    assert_eq!(restored.session.run.run_id.as_deref(), running.snapshot["run"]["run_id"].as_str());
    leader.close();
    leader.finish_close();
    host.finish();
}

#[test]
fn report103_party_ticket_in_hub_preserves_both_owner_saves() {
    let mut host = Host::start();
    let (_, endpoint) = host.request("POST", "/admin/hub/tickets", content(), true);
    let (mut leader, leader_id, _) = Client::join(&host, &endpoint, "Leader");
    let reserved = reserve_join(&host, &mut leader);
    let (mut guest, guest_id, _) = Client::join(&host, &reserved, "Guest");
    for (client, id) in [(&mut leader, leader_id), (&mut guest, guest_id)] {
        let restored = solomon_assets::website_save::restore(&saved_owner(client, 7)).unwrap();
        assert_eq!(restored.player_id, id);
        assert!(restored.session.participant(&id).is_some());
        assert!(restored.loaded_boneyard.is_none());
    }
    leader.close();
    leader.finish_close();
    guest.close();
    guest.finish_close();
    host.finish();
}

#[test]
fn report103_hosted_solo_entry_and_modals_ignore_connected_lobby_and_other_run_humans() {
    for route in ["/admin/sessions", "/admin/hub/tickets"] {
        let mut host = Host::start();
        let mut others = Vec::new();
        if route == "/admin/hub/tickets" {
            for (name, start_run) in [("Lobby", false), ("Other Run", true)] {
                let (_, endpoint) = host.request("POST", route, content(), true);
                let (mut other, _, _) = Client::join(&host, &endpoint, name);
                if start_run {
                    enter_solo_run(&mut other);
                }
                others.push(other);
            }
        }
        let (status, endpoint) = host.request("POST", route, content(), true);
        assert_eq!(status, 201);
        let (mut client, _, _) = Client::join(&host, &endpoint, "Solo");
        let mut running = enter_solo_run(&mut client);
        for source in [PauseSource::PauseMenu, PauseSource::Inventory, PauseSource::SkillBook] {
            client.send(ClientMessage::GameplayPause { paused: true, source });
            let paused = frame(&mut client, |frame| {
                frame.sequence > running.sequence && frame.host.pause.is_some()
            });
            client.send(ClientMessage::GameplayPause { paused: false, source });
            let released = frame(&mut client, |frame| {
                frame.sequence > paused.sequence && frame.host.pause.is_none()
            });
            assert!(released.host.resume_grace.is_none(), "{route} solo {source:?}");
            running = frame(&mut client, |frame| {
                frame.snapshot["ticks"].as_u64().unwrap()
                    > paused.snapshot["ticks"].as_u64().unwrap()
            });
            assert!(running.host.resume_grace.is_none());
        }
        client.close();
        client.finish_close();
        for other in &mut others {
            other.close();
            other.finish_close();
        }
        host.finish();
    }
}
