use super::*;

#[test]
fn website_only_cutover_keeps_signed_game_revision_and_retires_the_old_transport() {
    let game_revision = "a".repeat(40);
    let publication = "ce".repeat(16);
    let mut host = Host::start_with_revision(&game_revision);
    let (_, health) = host.request("GET", "/health", Value::Null, false);
    assert_eq!(health["deploymentProtocol"], 2);
    for invalid in [None, Some("not-a-publication-id")] {
        let (status, _) = request_at_with_deployment(
            host.address,
            "POST",
            "/admin/deployments/restart",
            json!({"targetRevision":game_revision}),
            true,
            invalid,
        );
        assert_eq!(status, 400);
        let (_, health) = host.request("GET", "/health", Value::Null, false);
        assert_eq!(health["draining"], false);
    }
    let (_, endpoint) = host.request("POST", "/admin/sessions", content(), true);
    let (mut client, player_id, _) = Client::join(&host, &endpoint, "Publication survivor");
    client.send(ClientMessage::Hosted {
        command: HostedCommand::StartMatch { boneyard_id: "default-random".into() },
    });
    client.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }));
    let address = host.address;
    let target = game_revision.clone();
    let id = publication.clone();
    let (finished, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        finished
            .send(request_at_with_deployment(
                address,
                "POST",
                "/admin/deployments/restart",
                json!({"targetRevision":target}),
                true,
                Some(&id),
            ))
            .unwrap();
    });
    let checkpoint = client.until(|message| {
        let ServerMessage::Hosted { message: HostedServerMessage::SaveCheckpoint { save, .. } } =
            message
        else {
            return false;
        };
        let document: Value = serde_json::from_str(save).unwrap();
        document["continuation"]["summary"]["partyRejoinToken"]
            .as_str()
            .and_then(|token| {
                solomon_launcher::host::recovery::decode_party_recovery_claim(SECRET, token)
            })
            .is_some_and(|claim| claim.target_revision.as_deref() == Some(&game_revision))
    });
    let ServerMessage::Hosted {
        message: HostedServerMessage::SaveCheckpoint { save, sequence, .. },
    } = checkpoint
    else {
        unreachable!()
    };
    let announcement = client.until(|message| {
        matches!(
            message,
            ServerMessage::Hosted { message: HostedServerMessage::DeploymentRestart { .. } }
        )
    });
    let ServerMessage::Hosted {
        message:
            HostedServerMessage::DeploymentRestart {
                checkpoint_sequence,
                target_revision,
                deployment_id,
            },
    } = announcement
    else {
        unreachable!()
    };
    assert_eq!(checkpoint_sequence, sequence);
    assert_eq!(target_revision, game_revision);
    assert_eq!(deployment_id, publication);
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    client.send(ClientMessage::Hosted {
        command: HostedCommand::DeploymentReady {
            checkpoint_sequence,
            target_revision: game_revision.clone(),
        },
    });
    client.finish_close();
    let (status, body) = result.recv_timeout(Duration::from_secs(8)).unwrap();
    worker.join().unwrap();
    assert_eq!(status, 200);
    assert_eq!(body["deploymentId"], publication);
    assert_eq!(body["targetRevision"], game_revision);
    assert_eq!(body["savedPlayers"], 1);
    let (_, health) = host.request("GET", "/health", Value::Null, false);
    assert_eq!(health["privateSessions"], 0);
    host.finish();

    // The signed claim remains a game-recovery claim. A Website publication is
    // neither a game revision nor a reason to reject the saved wizard.
    let mut replacement = Host::start_with_revision(&game_revision);
    let document: Value = serde_json::from_str(&save).unwrap();
    let (status, endpoint) = replacement.request(
        "POST",
        "/admin/rejoin",
        json!({
            "content":content()["content"], "save":save,
            "token":document["continuation"]["summary"]["partyRejoinToken"],
        }),
        true,
    );
    assert_eq!(status, 201, "{endpoint}");
    let (mut restored, welcome) =
        Client::join_with_save(&replacement, &endpoint, "Publication survivor", Some(save));
    let ServerMessage::Welcome { credentials, .. } = welcome else { unreachable!() };
    assert_eq!(credentials.player_id, player_id);
    restored.close();
    restored.finish_close();
    replacement.finish();
}
