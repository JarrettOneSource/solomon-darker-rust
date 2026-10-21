//! Current editor admission through actual native HTTP and continuously read sockets.

use super::*;

fn hello(endpoint: &Value) -> HostedHello {
    HostedHello {
        version: PROTOCOL_VERSION,
        credential: endpoint["credential"].as_str().unwrap().into(),
        selection: NewGameSelection {
            name: "Editor".into(),
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
    }
}

fn preview(host: &Host, request: &Value) -> Value {
    let (status, endpoint) = host.request("POST", "/admin/editor-test", request.clone(), true);
    assert_eq!(status, 201, "{endpoint}");
    endpoint
}

fn reject(client: &mut Client, expected: solomon_protocol::HostErrorCode) -> String {
    let message = client.until(|message| matches!(message, ServerMessage::Error { .. }));
    let ServerMessage::Error { code, reason } = message else { unreachable!() };
    assert_eq!(code, expected);
    client.finish_close();
    reason
}

#[test]
fn editor_preview_plays_only_admitted_geometry_without_saves_or_hosted_membership() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../sim/tests/fixtures/website/editor-host-goldens.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../sim/tests/fixtures/website/editor-test-goldens.json"
    ))
    .unwrap();
    let request = json!({"name":"  Authored map  ","bytesBase64":native["binaries"]["spawn"]});
    let mut host = Host::start();
    let mut rows = Vec::new();
    let (status, body) = host.request("POST", "/admin/editor-test", json!({}), false);
    rows.push(json!({"case":"unauthorized","status":status,"body":body}));
    for (name, body) in [
        ("null", Value::Null),
        ("missing", json!({})),
        ("encoding", json!({"name":"Broken","bytesBase64":"!!!!"})),
    ] {
        let (status, body) = host.request("POST", "/admin/editor-test", body, true);
        let (_, health) = host.request("GET", "/health", Value::Null, true);
        rows.push(json!({"case":format!("reject-{name}"),"status":status,"body":body,
            "sessions":health["privateSessions"]}));
    }
    let endpoint = preview(&host, &request);
    rows.push(json!({"case":"endpoint","status":201,"kind":endpoint["sessionKind"],"protocolRecognized":endpoint["protocol"]=="solomon-game-v1"}));
    let (mut client, welcome) = Client::join_with_welcome(&host, &endpoint, "Editor");
    let ServerMessage::Welcome { hosted: Some(welcome), .. } = welcome else { unreachable!() };
    let choices: Vec<_> = welcome
        .boneyards
        .iter()
        .map(|choice| {
            json!({
                "id":choice.id,"name":choice.name,"source":choice.source,
                "modId":choice.mod_id,"modName":choice.mod_name
            })
        })
        .collect();
    rows.push(json!({"case":"welcome","kind":source_session_kind(welcome.session_kind),"choices":choices}));
    for name in ["save", "resume", "intro"] {
        let endpoint = preview(&host, &request);
        let mut rejected = Client::connect(&host, &endpoint);
        let mut hello = hello(&endpoint);
        match name {
            "save" => {
                hello.save = Some("not a save".into());
                hello.save_intent = Some(GameSaveIntent::Resume);
            }
            "resume" => hello.resume_token = Some("existing-session-resume-token".into()),
            _ => hello.begin_college_intro = true,
        }
        rejected.send(ClientMessage::Hosted { command: HostedCommand::Hello { hello } });
        let reason = reject(&mut rejected, solomon_protocol::HostErrorCode::InvalidCredentials);
        rows.push(
            json!({"case":format!("reject-{name}"),"code":"authentication-failed","reason":reason}),
        );
    }
    for id in ["default-random", "stock-tutorial", "unknown-editor-map"] {
        let endpoint = preview(&host, &request);
        let (mut rejected, _, _) = Client::join(&host, &endpoint, "Editor");
        rejected.send(ClientMessage::Hosted {
            command: HostedCommand::StartMatch { boneyard_id: id.into() },
        });
        let reason = reject(&mut rejected, solomon_protocol::HostErrorCode::InvalidMessage);
        rows.push(
            json!({"case":format!("reject-match-{id}"),"code":"invalid-message","reason":reason}),
        );
    }
    client.send(ClientMessage::StartRun { tutorial: true });
    client.send(ClientMessage::SaveBeforeLeave { request_id: 1 });
    let save = client.until(|message| {
        matches!(
            message,
            ServerMessage::Hosted {
                message: HostedServerMessage::SaveBeforeLeave { request_id: 1, .. }
            }
        )
    });
    let ServerMessage::Hosted {
        message: HostedServerMessage::SaveBeforeLeave { checkpoint_sequence, .. },
    } = save
    else {
        unreachable!()
    };
    rows.push(json!({"case":"hub-save-suppressed","sequence":checkpoint_sequence}));
    client.send(ClientMessage::Hosted {
        command: HostedCommand::StartMatch { boneyard_id: "editor-test".into() },
    });
    let loaded = client.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }));
    let ServerMessage::BoneyardLoaded { boneyard } = loaded else { unreachable!() };
    let initial = client.until(|message| {
        matches!(message,ServerMessage::Snapshot{frame}
        if frame.host.resume_grace.is_some() && !frame.snapshot["boneyard"].is_null())
    });
    let ServerMessage::Snapshot { frame } = initial else { unreachable!() };
    let initial_tick = frame.snapshot["ticks"].as_u64().unwrap();
    client.send(ClientMessage::ReadyBoneyard { run_id: boneyard.run_id.clone() });
    client.send(ClientMessage::ResumeReady { sequence: frame.host.resume_grace.unwrap().sequence });
    client.until(|message| {
        matches!(message,ServerMessage::Snapshot{frame}
        if frame.snapshot["ticks"].as_u64().unwrap()>=initial_tick+100)
    });
    rows.push(json!({"case":"authored-world","choice":boneyard.choice_id,"name":boneyard.name,
        "spawn":{"x":boneyard.scene.spawn.position.x,"y":boneyard.scene.spawn.position.y,
            "facingDeg":boneyard.scene.spawn.facing_degrees},
        "geometrySha256":boneyard.geometry_sha256,"advancedAtLeast100Ticks":true}));
    client.send(ClientMessage::SaveBeforeLeave { request_id: 2 });
    let save = client.until(|message| {
        matches!(
            message,
            ServerMessage::Hosted {
                message: HostedServerMessage::SaveBeforeLeave { request_id: 2, .. }
            }
        )
    });
    let ServerMessage::Hosted {
        message: HostedServerMessage::SaveBeforeLeave { checkpoint_sequence, .. },
    } = save
    else {
        unreachable!()
    };
    let published = client.pending.iter().any(|message| {
        matches!(
            message,
            ServerMessage::Hosted {
                message: HostedServerMessage::SaveCheckpoint { .. }
                    | HostedServerMessage::SaveCheckpointChunk { .. }
            }
        )
    });
    rows.push(json!({"case":"run-save-suppressed","sequence":checkpoint_sequence,"checkpointPublished":published}));
    for path in ["/admin/presence", "/admin/hub/parties", "/admin/matches"] {
        let (_, body) = host.request("GET", path, Value::Null, true);
        rows.push(json!({"case":path,"body":body}));
    }
    let (_, endpoint) = host.request("POST", "/admin/sessions", content(), true);
    let (mut normal, welcome) = Client::join_with_welcome(&host, &endpoint, "Editor");
    let ServerMessage::Welcome { hosted: Some(welcome), .. } = welcome else { unreachable!() };
    normal.until(|message| {
        matches!(
            message,
            ServerMessage::Hosted {
                message: HostedServerMessage::SaveCheckpoint { .. }
                    | HostedServerMessage::SaveCheckpointChunk { .. }
            }
        )
    });
    rows.push(json!({"case":"ordinary-college-unchanged","kind":source_session_kind(welcome.session_kind),
        "randomChoice":welcome.boneyards.iter().any(|choice|choice.id=="default-random"),"checkpointPublished":true}));
    normal.close();
    normal.finish_close();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let (_, health) = host.request("GET", "/health", Value::Null, true);
        if health["privateSessions"] == 4 {
            break;
        }
        assert!(Instant::now() < deadline, "disposable previews were not reaped after closing");
        thread::sleep(Duration::from_millis(5));
    }
    rows.push(json!({"case":"pre-wizard-rejections-retain-unclaimed-owners","sessions":4}));
    let (status, body) = host.request(
        "POST",
        "/admin/deployments/restart",
        json!({"targetRevision":"b".repeat(40)}),
        true,
    );
    rows.push(json!({"case":"deployment-omits-disposable-wizard","status":status,"body":body}));
    client.finish_close();
    let actual: solomon_sim::modding::LuaValue = serde_json::from_value(json!(rows)).unwrap();
    let expected: solomon_sim::modding::LuaValue =
        serde_json::from_value(fixture["rows"].clone()).unwrap();
    assert_eq!(actual, expected);
    host.finish();
}

fn source_session_kind(kind: solomon_sim::social::GameSessionKind) -> &'static str {
    use solomon_sim::social::GameSessionKind;
    match kind {
        GameSessionKind::GlobalHub => "global-hub",
        GameSessionKind::PrivateCollege => "private-college",
        GameSessionKind::Standalone => "standalone",
    }
}
