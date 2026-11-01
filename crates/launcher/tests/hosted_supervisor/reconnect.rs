//! Report 74 keeps the live authority through an unexpected proxy interruption.
use super::*;

#[test]
fn deliberate_exits_cleanup_immediately_while_transport_interruptions_retain_the_actor() {
    for (case, interrupted) in [
        ("empty-close", false),
        ("normal-close", false),
        ("disconnect-command", false),
        ("going-away", true),
        ("unclosed-tcp", true),
    ] {
        let mut host = Host::start();
        let (_, endpoint) = host.request("POST", "/admin/sessions", content(), true);
        let (mut client, welcome) = Client::join_with_welcome(&host, &endpoint, "Lifecycle");
        let ServerMessage::Welcome { credentials, .. } = welcome else { unreachable!() };
        client.send(ClientMessage::Ping { nonce: 74 });
        client.until(|message| matches!(message, ServerMessage::Pong { nonce: 74 }));
        match case {
            "empty-close" => client.close(),
            "normal-close" => client.close_with_reason(Some("finished".into())),
            "disconnect-command" => client.send(ClientMessage::Disconnect),
            _ => {
                let (completed, result) = mpsc::channel();
                let write = if interrupted && case == "going-away" {
                    ClientWrite::GoingAway { completed }
                } else {
                    ClientWrite::DropTransport { completed }
                };
                client.writes.send(write).unwrap();
                result.recv_timeout(Duration::from_secs(8)).unwrap().unwrap();
            }
        }
        client.finish_close();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let (_, health) = host.request("GET", "/health", Value::Null, true);
            if health["privatePlayers"] == 0 {
                if interrupted {
                    assert_eq!(health["recoveringPlayers"], 1, "{case}: {health}");
                    assert_eq!(health["privateSessions"], 1, "{case}: {health}");
                    let (_, presence) = host.request("GET", "/admin/presence", Value::Null, true);
                    assert!(
                        presence["items"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|row| { row["displayName"] == "Lifecycle" }),
                        "{case}: the same accepted actor remains present"
                    );
                    let (status, reserved) = reserve(&host, &endpoint, &credentials, 'g');
                    assert_eq!(status, 201, "{case}: {reserved}");
                    assert_eq!(reserved["path"], endpoint["path"]);
                    break;
                }
                assert_eq!(health["recoveringPlayers"], 0, "{case}: {health}");
                if health["privateSessions"] == 0 {
                    let (_, presence) = host.request("GET", "/admin/presence", Value::Null, true);
                    assert!(presence["items"].as_array().unwrap().is_empty(), "{case}: {presence}");
                    assert_eq!(reserve(&host, &endpoint, &credentials, 'g').0, 404, "{case}");
                    break;
                }
            }
            assert!(Instant::now() < deadline, "{case}: cleanup did not complete: {health}");
            thread::sleep(Duration::from_millis(5));
        }
        host.finish();
    }
}

#[test]
fn proxy_close_reattaches_same_private_actor_with_fresh_bound_one_use_ticket() {
    let mut host = Host::start();
    let mut admission = content();
    admission["leaderboardUserId"] = json!(42);
    let (status, endpoint) = host.request("POST", "/admin/sessions", admission, true);
    assert_eq!(status, 201);
    let (mut first, welcome) = Client::join_with_welcome(&host, &endpoint, "Retained");
    let ServerMessage::Welcome { credentials, .. } = welcome else { unreachable!() };
    first.send(ClientMessage::Hosted {
        command: HostedCommand::SetEnhancedEffects { enabled: false },
    });
    first.until(|message| {
        matches!(message, ServerMessage::Snapshot { frame }
        if frame.snapshot["enhanced_effects"] == false)
    });
    let (completed, result) = mpsc::channel();
    first.writes.send(ClientWrite::GoingAway { completed }).unwrap();
    result.recv_timeout(Duration::from_secs(8)).unwrap().unwrap();
    first.finish_close();
    let request = json!({"sessionId": endpoint["path"].as_str().unwrap().rsplit('/').next().unwrap(),
        "playerId": credentials.player_id, "resumeToken": credentials.token,
        "requestId": "r".repeat(32), "leaderboardUserId": 42});
    let mut wrong = request.clone();
    wrong["leaderboardUserId"] = json!(43);
    assert_eq!(host.request("POST", "/admin/reconnect", wrong, true).0, 403);
    let (status, reservation) = host.request("POST", "/admin/reconnect", request.clone(), true);
    assert_eq!(status, 201, "{reservation}");
    assert_eq!(reservation["path"], endpoint["path"]);
    assert_ne!(reservation["credential"], endpoint["credential"]);
    assert_eq!(host.request("POST", "/admin/reconnect", request, true).1, reservation);
    let hello = HostedHello {
        version: PROTOCOL_VERSION,
        credential: reservation["credential"].as_str().unwrap().into(),
        selection: NewGameSelection {
            name: "Ignored browser profile".into(),
            discipline: WizardDiscipline::Arcane,
            element: WizardElement::Ether,
        },
        profile: PlayerSocialProfile::default(),
        online_preferences: GameOnlinePreferences::default(),
        resume_token: Some(credentials.token.clone()),
        save_intent: None,
        save: None,
        begin_college_intro: false,
        decline_tutorial: false,
        cheats_enabled: false,
        enhanced_effects: true,
        allow_mod_mismatch: false,
    };
    let mut next = Client::connect(&host, &reservation);
    next.send(ClientMessage::Hosted { command: HostedCommand::Hello { hello: hello.clone() } });
    let reattached = next.until(|message| matches!(message, ServerMessage::Welcome { .. }));
    let ServerMessage::Welcome { credentials: rebound, frame, .. } = reattached else {
        unreachable!()
    };
    assert_eq!(rebound.player_id, credentials.player_id);
    assert_eq!(frame.snapshot["enhanced_effects"], false);
    let mut replay = Client::connect(&host, &reservation);
    replay.send(ClientMessage::Hosted { command: HostedCommand::Hello { hello } });
    assert!(matches!(
        replay.until(|message| matches!(message, ServerMessage::Error { .. })),
        ServerMessage::Error { code: solomon_protocol::HostErrorCode::InvalidCredentials, .. }
    ));
    replay.finish_close();
    next.send(ClientMessage::Ping { nonce: 74 });
    next.until(|message| matches!(message, ServerMessage::Pong { nonce: 74 }));
    next.close();
    next.finish_close();
    host.finish();
}

fn reserve(
    host: &Host,
    endpoint: &Value,
    credentials: &solomon_protocol::ResumeCredentials,
    request: char,
) -> (u16, Value) {
    host.request(
        "POST",
        "/admin/reconnect",
        json!({
            "sessionId": endpoint["path"].as_str().unwrap().rsplit('/').next().unwrap(),
            "playerId": credentials.player_id, "resumeToken": credentials.token,
            "requestId": request.to_string().repeat(32), "leaderboardUserId": null,
        }),
        true,
    )
}

fn reconnect_hello(
    endpoint: &Value,
    credentials: &solomon_protocol::ResumeCredentials,
) -> ClientMessage {
    ClientMessage::Hosted {
        command: HostedCommand::Hello {
            hello: HostedHello {
                version: PROTOCOL_VERSION,
                credential: endpoint["credential"].as_str().unwrap().into(),
                selection: NewGameSelection {
                    name: "Ignored browser profile".into(),
                    discipline: WizardDiscipline::Arcane,
                    element: WizardElement::Ether,
                },
                profile: PlayerSocialProfile::default(),
                online_preferences: GameOnlinePreferences::default(),
                resume_token: Some(credentials.token.clone()),
                save_intent: None,
                save: None,
                begin_college_intro: false,
                decline_tutorial: false,
                cheats_enabled: false,
                enhanced_effects: true,
                allow_mod_mismatch: false,
            },
        },
    }
}

#[test]
fn reservation_before_old_close_is_generation_safe_and_does_not_duplicate_the_actor() {
    let mut host = Host::start();
    let (_, endpoint) = host.request("POST", "/admin/sessions", content(), true);
    let (mut first, welcome) = Client::join_with_welcome(&host, &endpoint, "Retained");
    let ServerMessage::Welcome { credentials, .. } = welcome else { unreachable!() };
    let (status, reservation) = reserve(&host, &endpoint, &credentials, 'b');
    assert_eq!(status, 201, "{reservation}");
    let mut replacement = Client::connect(&host, &reservation);
    replacement.send(reconnect_hello(&reservation, &credentials));
    let welcome = replacement.until(|message| matches!(message, ServerMessage::Welcome { .. }));
    let ServerMessage::Welcome { credentials: rebound, frame, .. } = welcome else {
        unreachable!()
    };
    assert_eq!(rebound.player_id, credentials.player_id);
    assert_eq!(frame.snapshot["participant_order"].as_array().unwrap().len(), 1);
    first.finish_close();
    replacement.send(ClientMessage::Ping { nonce: 74 });
    replacement.until(|message| matches!(message, ServerMessage::Pong { nonce: 74 }));
    replacement.close();
    replacement.finish_close();
    host.finish();
}

#[test]
fn retained_run_welcome_preserves_current_world_and_renderer_readiness_within_lease() {
    let document = if let Some(path) = std::env::var_os("SOLOMON_REPORT074_LARGE_SAVE") {
        std::fs::read_to_string(path).unwrap()
    } else {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../sim/tests/fixtures/website/save-document-goldens.json"
        ))
        .unwrap();
        fixture["cases"].as_array().unwrap().iter().find(|case| case["name"] == "boneyard").unwrap()
            ["document"]
            .to_string()
    };
    let mut host = Host::start();
    let (_, endpoint) = host.request("POST", "/admin/sessions", content(), true);
    // Importing the large setup document precedes the measured transport episode.
    let (mut first, welcome) = Client::join_with_save_timeout(
        &host,
        &endpoint,
        "Retained",
        Some(document),
        Duration::from_secs(45),
    );
    let ServerMessage::Welcome { credentials, frame: original, .. } = welcome else {
        unreachable!()
    };
    let started = Instant::now();
    let (completed, result) = mpsc::channel();
    first.writes.send(ClientWrite::GoingAway { completed }).unwrap();
    result.recv_timeout(Duration::from_secs(8)).unwrap().unwrap();
    first.finish_close();
    let (status, reservation) = reserve(&host, &endpoint, &credentials, 'c');
    assert_eq!(status, 201, "{reservation}");
    let mut replacement = Client::connect(&host, &reservation);
    replacement.send(reconnect_hello(&reservation, &credentials));
    let welcome = replacement.until(|message| matches!(message, ServerMessage::Welcome { .. }));
    let welcome_bytes = serde_json::to_vec(&welcome).unwrap().len();
    let ServerMessage::Welcome { credentials: rebound, frame, .. } = welcome else {
        unreachable!()
    };
    assert_eq!(rebound.player_id, credentials.player_id);
    assert_eq!(frame.snapshot, original.snapshot, "the actual live world must be retained");
    let run_id = frame.snapshot["run"]["run_id"].as_str().unwrap().to_owned();
    replacement.send(ClientMessage::ReadyBoneyard { run_id });
    replacement
        .send(ClientMessage::ResumeReady { sequence: frame.host.resume_grace.unwrap().sequence });
    replacement.send(ClientMessage::Ping { nonce: 75 });
    replacement.until(|message| {
        matches!(message, ServerMessage::Snapshot { frame }
        if frame.snapshot["ticks"].as_u64() > original.snapshot["ticks"].as_u64())
    });
    eprintln!(
        "large valid retained Welcome: {welcome_bytes} bytes; readiness: {} ms",
        started.elapsed().as_millis()
    );
    assert!(started.elapsed() < Duration::from_secs(30));
    replacement.close();
    replacement.finish_close();
    host.finish();
}

#[test]
fn lease_expires_at_original_deadline_without_request_renewal_or_fresh_provision() {
    let mut host = Host::start();
    let (_, endpoint) = host.request("POST", "/admin/sessions", content(), true);
    let (mut first, welcome) = Client::join_with_welcome(&host, &endpoint, "Retained");
    let ServerMessage::Welcome { credentials, .. } = welcome else { unreachable!() };
    let started = Instant::now();
    let (status, reservation) = reserve(&host, &endpoint, &credentials, 'd');
    assert_eq!(status, 201);
    thread::sleep(Duration::from_secs(1));
    assert_eq!(reserve(&host, &endpoint, &credentials, 'd').1, reservation);
    let (_, health) = host.request("GET", "/health", Value::Null, true);
    assert_eq!(health["recoveringPlayers"], 1);
    assert_eq!(health["privatePlayers"], 0);
    first.finish_close();
    loop {
        let (_, health) = host.request("GET", "/health", Value::Null, true);
        if health["recoveringPlayers"] == 0 {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(35),
            "the retained actor outlived its lease"
        );
        thread::sleep(Duration::from_millis(100));
    }
    assert!(started.elapsed() >= Duration::from_millis(29_500));
    assert!(started.elapsed() < Duration::from_secs(35));
    assert_eq!(reserve(&host, &endpoint, &credentials, 'e').0, 404);
    host.finish();
}

#[test]
fn deployment_drains_the_live_replacement_before_renderer_readiness() {
    let mut host = Host::start();
    let (_, endpoint) = host.request("POST", "/admin/sessions", content(), true);
    let (mut first, welcome) = Client::join_with_welcome(&host, &endpoint, "Retained");
    let ServerMessage::Welcome { credentials, .. } = welcome else { unreachable!() };
    let (status, reservation) = reserve(&host, &endpoint, &credentials, 'f');
    assert_eq!(status, 201);
    first.finish_close();
    let mut replacement = Client::connect(&host, &reservation);
    replacement.send(reconnect_hello(&reservation, &credentials));
    replacement.until(|message| matches!(message, ServerMessage::Welcome { .. }));
    let (_, health) = host.request("GET", "/health", Value::Null, true);
    assert_eq!(health["recoveringPlayers"], 1);
    let address = host.address;
    let target = "b".repeat(40);
    let (finished, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        finished
            .send(request_at(
                address,
                "POST",
                "/admin/deployments/restart",
                json!({"targetRevision":target}),
                true,
            ))
            .unwrap();
    });
    let announcement = replacement.until(|message| {
        matches!(
            message,
            ServerMessage::Hosted { message: HostedServerMessage::DeploymentRestart { .. } }
        )
    });
    let ServerMessage::Hosted {
        message: HostedServerMessage::DeploymentRestart { checkpoint_sequence, target_revision, .. },
    } = announcement
    else {
        unreachable!()
    };
    replacement.until(|message| {
        matches!(message, ServerMessage::Hosted {
        message: HostedServerMessage::SaveCheckpoint { sequence, .. }
    } if *sequence == checkpoint_sequence)
    });
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    replacement.send(ClientMessage::Hosted {
        command: HostedCommand::DeploymentReady { checkpoint_sequence, target_revision },
    });
    replacement.finish_close();
    let (status, body) = result.recv_timeout(Duration::from_secs(8)).unwrap();
    worker.join().unwrap();
    assert_eq!(status, 200);
    assert_eq!(body["savedPlayers"], 1);
    assert_eq!(body["unacknowledgedPlayers"], 0);
    host.finish();
}
