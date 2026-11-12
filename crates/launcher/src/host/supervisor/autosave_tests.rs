use super::*;
use crate::host::connection::HostConnection;
use crate::host::hosted::{HostedAdmission, SaveEncodeGate};
use crate::transport::{PeerSender, TransportConfig};
use solomon_protocol::hosted::HostedHello;
use solomon_protocol::{ClientMessage, PROTOCOL_VERSION};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::GameSessionInput;
use solomon_sim::social::{GameOnlinePreferences, GameSessionKind, PlayerSocialProfile};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;
use tungstenite::WebSocket;
use tungstenite::client::IntoClientRequest;

const SECRET: &str = "autosave-authority-regression-test-secret";
const ORIGIN: &str = "https://autosave.invalid";

fn supervisor() -> HostedSupervisor {
    HostedSupervisor::new(HostedSupervisorConfig::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web"),
        SECRET.into(),
        "a".repeat(40),
    ))
    .unwrap()
}

fn server() -> NativeServer {
    NativeServer::start_hosted(
        TcpListener::bind(("127.0.0.1", 0)).unwrap(),
        TransportConfig { allowed_origins: vec![ORIGIN.into()], ..TransportConfig::default() },
    )
    .unwrap()
}

fn peer(server: &NativeServer, path: &str) -> (PeerSender, WebSocket<TcpStream>) {
    if let Err(error) = server.register_hosted_route(path.into(), 16) {
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    }
    let mut request = format!("ws://{}{path}", server.local_addr()).into_client_request().unwrap();
    request.headers_mut().insert("Origin", ORIGIN.parse().unwrap());
    let stream = TcpStream::connect(server.local_addr()).unwrap();
    let (socket, _) = tungstenite::client(request, stream).unwrap();
    let TransportEvent::Connected { peer, .. } =
        server.recv_timeout(Duration::from_secs(3)).unwrap()
    else {
        panic!("expected connected peer")
    };
    (peer, socket)
}

fn admit(host: &mut HostedGameHost, peer: PeerSender, name: &str) -> String {
    let mut connection = HostConnection::new(peer, 0.0);
    let admission = HostedAdmission {
        reconnect: None,
        content: Arc::clone(&host.content),
        developer_access: false,
        leaderboard_user_id: None,
        party_id: None,
        reservation_id: None,
        party_rejoin_token: None,
        party_recovery_seed: false,
        observer: None,
    };
    let id = host
        .admit_ordinary(
            &mut connection,
            HostedHello {
                version: PROTOCOL_VERSION,
                credential: String::new(),
                selection: NewGameSelection {
                    name: name.into(),
                    element: WizardElement::Ether,
                    discipline: WizardDiscipline::Arcane,
                },
                profile: PlayerSocialProfile::default(),
                online_preferences: GameOnlinePreferences::default(),
                resume_token: None,
                save: None,
                save_intent: None,
                begin_college_intro: false,
                decline_tutorial: true,
                cheats_enabled: false,
                enhanced_effects: true,
                allow_mod_mismatch: false,
            },
            admission,
            0.0,
        )
        .unwrap();
    connection.welcomed(id.clone(), 0);
    host.connections.insert(connection.peer.id(), connection);
    host.pending_player_saves.insert(id.clone());
    id
}

#[test]
fn held_autosave_keeps_authority_input_and_independent_world_progressing() {
    let (entered, started) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (progress, progressed) = mpsc::channel();
    let worker = thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let mut supervisor = supervisor();
            let mut server = server();
            let (owner_peer, _owner_socket) = peer(&server, "/game-hub");
            let owner_peer_id = owner_peer.id();
            let owner = admit(&mut supervisor.colleges[0].host, owner_peer, "Owner");
            let mut witness = HostedGameHost::new(
                "witness-college".into(),
                GameSessionKind::PrivateCollege,
                16,
                Arc::clone(&supervisor.colleges[0].host.content),
                &supervisor.config.asset_directory,
                SECRET.into(),
                "a".repeat(40),
            )
            .unwrap();
            let (witness_peer, _witness_socket) = peer(&server, "/game");
            let witness_id = admit(&mut witness, witness_peer, "Witness");
            supervisor.colleges.push(CollegeRecord {
                host: witness,
                path: "/game".into(),
                reconnect_tickets: BTreeMap::new(),
                tickets: BTreeMap::new(),
                created_at_ms: 0.0,
                claimed: true,
            });
            supervisor.colleges[0].host.save_encode_gate =
                Some(Arc::new(SaveEncodeGate { entered, release: Mutex::new(Some(released)) }));
            supervisor.shared_host_tick = 2_999;
            supervisor.tick(0.0).unwrap();
            let input = ClientMessage::Input {
                sequence: 1,
                target_tick: 2,
                input: GameSessionInput::default(),
            };
            supervisor
                .message(owner_peer_id, &serde_json::to_string(&input).unwrap(), 1.0)
                .unwrap();
            for tick in 1..=5 {
                supervisor.tick(f64::from(tick) * 10.0).unwrap();
            }
            supervisor.colleges[0].host.request_autonomous_saves(true);
            let host = &supervisor.colleges[0].host;
            progress
                .send((
                    host.connections[&owner_peer_id].input.acknowledged_sequence(),
                    supervisor.colleges[1].host.state_for_player(&witness_id).unwrap().ticks,
                    host.player(&owner).unwrap().last_save_sequence,
                    host.connections[&owner_peer_id].initial_save_pending,
                ))
                .unwrap();
            // Keep the authority alive until its accepted completion can be observed.
            let deadline = Instant::now() + Duration::from_secs(3);
            while supervisor.colleges[0].host.player(&owner).unwrap().last_save_sequence == 0
                && Instant::now() < deadline
            {
                supervisor.tick(70.0).unwrap();
                thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(supervisor.colleges[0].host.player(&owner).unwrap().last_save_sequence, 1);
            assert!(!supervisor.colleges[0].host.connections[&owner_peer_id].initial_save_pending);
            finish_encoding(&mut supervisor);
            assert_eq!(
                supervisor.colleges[1].host.player(&witness_id).unwrap().last_save_sequence,
                1,
                "a second College is selected before a new request from the first"
            );
            assert!(supervisor.colleges[0].host.pending_player_saves.contains(&owner));
            server.shutdown();
        })
        .unwrap();
    started.recv_timeout(Duration::from_secs(5)).expect("real save reached the controlled encoder");
    let result = progressed.recv_timeout(Duration::from_millis(300));
    release.send(Ok(())).unwrap();
    worker.join().unwrap();
    let (input_ack, witness_tick, save_sequence, initial_pending) =
        result.expect("authority stalled behind autonomous save encoding");
    assert_eq!(input_ack, 1);
    assert!(witness_tick >= 6);
    assert_eq!(save_sequence, 0, "a held encode must not commit a checkpoint");
    assert!(initial_pending, "initial publication retires only on accepted completion");
}

fn standalone() -> HostedSupervisor {
    HostedSupervisor::new_standalone(
        HostedSupervisorConfig::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web"),
            SECRET.into(),
            "a".repeat(40),
        ),
        "test-standalone-credential".into(),
    )
    .unwrap()
}

fn finish_encoding(supervisor: &mut HostedSupervisor) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !supervisor.save_encoder.is_available() && Instant::now() < deadline {
        supervisor.complete_autonomous_save().unwrap();
        thread::sleep(Duration::from_millis(1));
    }
    assert!(supervisor.save_encoder.is_available(), "the real codec must complete");
}

#[test]
fn dirty_during_encoding_survives_and_other_players_are_selected_first() {
    let mut supervisor = standalone();
    let mut server = server();
    let (first_peer, _first_socket) = peer(&server, "/game");
    let first_peer_id = first_peer.id();
    let first = admit(&mut supervisor.colleges[0].host, first_peer, "First");
    let (second_peer, _second_socket) = peer(&server, "/game");
    let second = admit(&mut supervisor.colleges[0].host, second_peer, "Second");
    let host = &mut supervisor.colleges[0].host;
    let job = host.capture_pending_save().unwrap();
    for _ in 0..10 {
        host.request_autonomous_saves(false);
    }
    assert!(
        !host.pending_player_saves.contains(&first),
        "the initial obligation is already captured"
    );
    host.set_enhanced_effects(&first, false);
    assert!(host.pending_player_saves.contains(&first));
    host.accept_encoded_save(job.encode()).unwrap();
    assert_eq!(host.player(&first).unwrap().last_save_sequence, 1);
    assert!(host.pending_player_saves.contains(&first), "later dirtiness must survive completion");
    assert!(!host.connections[&first_peer_id].initial_save_pending);
    let frozen = host.player(&first).unwrap().last_save_document.as_ref().unwrap();
    assert!(solomon_assets::website_save::restore(frozen).unwrap().session.enhanced_effects);
    let second_job = host.capture_pending_save().unwrap();
    host.accept_encoded_save(second_job.encode()).unwrap();
    assert_eq!(
        host.player(&second).unwrap().last_save_sequence,
        1,
        "a busy first player cannot starve its neighbor"
    );
    assert_eq!(host.player(&first).unwrap().last_save_sequence, 1);
    let newest = host.capture_pending_save().unwrap();
    host.accept_encoded_save(newest.encode()).unwrap();
    let saved = host.player(&first).unwrap().last_save_document.as_ref().unwrap();
    assert!(!solomon_assets::website_save::restore(saved).unwrap().session.enhanced_effects);
    assert_eq!(host.player(&first).unwrap().last_save_sequence, 2);
    host.request_autonomous_saves(false);
    assert!(host.capture_pending_save().is_none());
    server.shutdown();
}

#[test]
fn frozen_recovery_save_matches_canonical_bytes_and_raw_signing() {
    use crate::host::recovery::{create_party_recovery_claim, decode_party_recovery_claim};
    use solomon_assets::website_save;
    let mut supervisor = standalone();
    let mut server = server();
    let (owner_peer, _socket) = peer(&server, "/game");
    let owner = admit(&mut supervisor.colleges[0].host, owner_peer, "Frozen");
    let host = &mut supervisor.colleges[0].host;
    assert!(host.start_match(&owner, "default-random", false).unwrap());
    let party_id = host.party_id_for_player(&owner).unwrap().to_owned();
    host.arm_party_rejoins(&party_id).unwrap();
    let (expected, token) = host.owner_save(&owner, None).unwrap();
    let token = token.unwrap();
    let claim = decode_party_recovery_claim(SECRET, &token).unwrap();
    let unsigned = website_save::create(website_save::CreateSaveOptions {
        state: host.state_for_player(&owner).unwrap(),
        player_id: &owner,
        loaded_boneyard: host.worlds.loaded_for_player(&owner),
        metadata: &host.player(&owner).unwrap().save_metadata,
        party_rejoin_token: None,
    })
    .unwrap();
    assert_eq!(
        create_party_recovery_claim(SECRET, &claim, &unsigned).unwrap(),
        token,
        "the frozen fast creator retains the independent raw-document signing contract"
    );
    let job = host.capture_pending_save().unwrap();
    host.worlds.state_for_player_mut(&owner).unwrap().ticks += 10;
    host.accept_encoded_save(job.encode()).unwrap();
    assert_eq!(host.player(&owner).unwrap().last_save_document.as_deref(), Some(expected.as_str()));
    assert_eq!(
        host.rejoin_slots.iter().find(|slot| slot.player_id == owner).unwrap().token.as_deref(),
        Some(token.as_str())
    );
    server.shutdown();
}

#[test]
fn forced_save_rejects_an_older_completion_while_deployment_is_suspended() {
    use crate::host::hosted::PlayerSaveRequest;
    let mut supervisor = standalone();
    let mut server = server();
    let (owner_peer, _socket) = peer(&server, "/game");
    let peer_id = owner_peer.id();
    let owner = admit(&mut supervisor.colleges[0].host, owner_peer, "Forced");
    let (entered, started) = mpsc::channel();
    let (release, released) = mpsc::channel();
    supervisor.colleges[0].host.save_encode_gate =
        Some(Arc::new(SaveEncodeGate { entered, release: Mutex::new(Some(released)) }));
    supervisor.start_autonomous_save().unwrap();
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    let host = &mut supervisor.colleges[0].host;
    host.set_enhanced_effects(&owner, false);
    let mut connection = host.connections.remove(&peer_id).unwrap();
    let forced = host
        .publish_player_save(
            &mut connection,
            &owner,
            PlayerSaveRequest {
                target_revision: Some(&"b".repeat(40)),
                force: true,
                include_terminal_profile: false,
            },
        )
        .unwrap();
    host.connections.insert(peer_id, connection);
    assert_eq!(forced, 1);
    let document = host.player(&owner).unwrap().last_save_document.clone();
    let (respond, _response) = mpsc::sync_channel(1);
    supervisor.deployment = Some(DeploymentRestart {
        target_revision: "b".repeat(40),
        deployment_id: "d1".repeat(16),
        deployment_sequence: 1,
        deadline_ms: 30_000.0,
        completed: false,
        closing: false,
        checkpoints: BTreeMap::from([(peer_id, forced)]),
        pending: BTreeSet::from([peer_id]),
        acknowledged: BTreeSet::new(),
        respond,
    });
    let tick = supervisor.shared_host_tick;
    release.send(Ok(())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !supervisor.save_encoder.is_available() && Instant::now() < deadline {
        supervisor.tick(10.0).unwrap();
        thread::sleep(Duration::from_millis(1));
    }
    assert!(supervisor.save_encoder.is_available(), "deployment still polls discarded completions");
    assert_eq!(supervisor.shared_host_tick, tick);
    let host = &supervisor.colleges[0].host;
    assert_eq!(host.player(&owner).unwrap().last_save_sequence, forced);
    assert_eq!(host.player(&owner).unwrap().last_save_document, document);
    assert!(
        host.pending_player_saves.contains(&owner),
        "new dirtiness remains coalesced during deployment"
    );
    server.shutdown();
}

#[test]
fn failed_terminal_encode_preserves_initial_and_terminal_obligations() {
    use solomon_sim::game_run::GameRunPhase;
    let mut supervisor = standalone();
    let mut server = server();
    let (owner_peer, _socket) = peer(&server, "/game");
    let peer_id = owner_peer.id();
    let owner = admit(&mut supervisor.colleges[0].host, owner_peer, "Terminal");
    let host = &mut supervisor.colleges[0].host;
    let state = host.worlds.state_for_player_mut(&owner).unwrap();
    state.run.phase = GameRunPhase::GameOver;
    state.run.run_id = Some("terminal-run".into());
    let (entered, _started) = mpsc::channel();
    let (release, released) = mpsc::channel();
    host.save_encode_gate =
        Some(Arc::new(SaveEncodeGate { entered, release: Mutex::new(Some(released)) }));
    release.send(Err("controlled encode failure".into())).unwrap();
    let job = host.capture_pending_save().unwrap();
    host.accept_encoded_save(job.encode()).unwrap();
    assert_eq!(host.player(&owner).unwrap().last_save_sequence, 0);
    assert!(host.player(&owner).unwrap().terminal_saved_run_id.is_none());
    assert!(host.connections[&peer_id].initial_save_pending);
    assert!(host.pending_player_saves.contains(&owner));
    let retry = host.capture_pending_save().unwrap();
    host.accept_encoded_save(retry.encode()).unwrap();
    assert_eq!(host.player(&owner).unwrap().last_save_sequence, 1);
    assert_eq!(host.player(&owner).unwrap().terminal_saved_run_id.as_deref(), Some("terminal-run"));
    assert!(!host.connections[&peer_id].initial_save_pending);
    host.request_autonomous_saves(false);
    assert!(host.capture_pending_save().is_none());
    server.shutdown();
}

#[test]
fn stale_run_phase_integrity_admission_and_peer_results_never_refill_the_cache() {
    use solomon_assets::website_save::SaveIntegrity;
    use solomon_sim::game_run::GameRunPhase;
    for change in ["run", "phase", "integrity", "eligibility", "admission", "peer", "host"] {
        let mut supervisor = standalone();
        let mut server = server();
        let (owner_peer, _socket) = peer(&server, "/game");
        let peer_id = owner_peer.id();
        let owner = admit(&mut supervisor.colleges[0].host, owner_peer, "Changing");
        let host = &mut supervisor.colleges[0].host;
        host.player_mut(&owner).unwrap().save_metadata.integrity = SaveIntegrity::GlobalClean;
        host.player_mut(&owner).unwrap().global_score_eligible = true;
        let job = host.capture_pending_save().unwrap();
        let mut replacement_socket = None;
        match change {
            "run" => {
                host.worlds.state_for_player_mut(&owner).unwrap().run.run_id =
                    Some("new-run".into())
            }
            "phase" => {
                host.worlds.state_for_player_mut(&owner).unwrap().run.phase = GameRunPhase::GameOver
            }
            "integrity" => {
                host.player_mut(&owner).unwrap().save_metadata.integrity = SaveIntegrity::LocalOnly
            }
            "eligibility" => host.player_mut(&owner).unwrap().global_score_eligible = false,
            "admission" => host.player_mut(&owner).unwrap().resume_token.push_str("-new-admission"),
            "peer" => {
                let (replacement, socket) = peer(&server, "/game");
                let mut connection = HostConnection::new(replacement, 1.0);
                connection.welcomed(owner.clone(), 0);
                host.player_mut(&owner).unwrap().peer_id = connection.peer.id();
                host.connections.remove(&peer_id);
                host.connections.insert(connection.peer.id(), connection);
                replacement_socket = Some(socket);
            }
            "host" => {
                let mut replacement = HostedGameHost::new(
                    host.id.clone(),
                    GameSessionKind::Standalone,
                    16,
                    Arc::clone(&host.content),
                    &supervisor.config.asset_directory,
                    SECRET.into(),
                    "a".repeat(40),
                )
                .unwrap();
                replacement.connections = std::mem::take(&mut host.connections);
                *host = replacement;
            }
            _ => unreachable!(),
        }
        host.accept_encoded_save(job.encode()).unwrap();
        if change != "host" {
            assert_eq!(host.player(&owner).unwrap().last_save_sequence, 0, "{change}");
            assert!(host.player(&owner).unwrap().last_save_document.is_none(), "{change}");
            assert!(
                host.pending_player_saves.contains(&owner),
                "{change} must request a current capture"
            );
        } else {
            assert!(host.players.is_empty());
            assert!(host.pending_player_saves.is_empty());
        }
        drop(replacement_socket);
        server.shutdown();
    }
}

#[test]
fn staged_rejoin_capture_keeps_detached_owner_geometry_and_recovery_together() {
    use crate::host::hosted::HostedWorlds;
    let mut supervisor = standalone();
    let mut server = server();
    let (owner_peer, _owner_socket) = peer(&server, "/game");
    let owner = admit(&mut supervisor.colleges[0].host, owner_peer, "Detached");
    let (other_peer, _other_socket) = peer(&server, "/game");
    let other = admit(&mut supervisor.colleges[0].host, other_peer, "Active");
    let host = &mut supervisor.colleges[0].host;
    assert!(host.start_match(&owner, "default-random", false).unwrap());
    let party_id = host.party_id_for_player(&owner).unwrap().to_owned();
    host.arm_party_rejoins(&party_id).unwrap();
    let detached = host.state_for_player(&owner).unwrap().detach_participant(&owner).unwrap();
    let HostedWorlds::Private { state, .. } = &mut host.worlds else { unreachable!() };
    state.remove_participant(&owner);
    let slot = host.rejoin_slots.iter_mut().find(|slot| slot.player_id == owner).unwrap();
    slot.detached = Some(detached);
    assert!(slot.connected);
    let expected = host.owner_save(&owner, None).unwrap();
    let job = host.capture_pending_save().unwrap();
    host.worlds.state_for_player_mut(&other).unwrap().ticks += 20;
    host.rejoin_slots
        .iter_mut()
        .find(|slot| slot.player_id == owner)
        .unwrap()
        .detached
        .as_mut()
        .unwrap()
        .participant
        .player
        .selection
        .name = "Later name".into();
    host.pending_player_saves.insert(owner.clone());
    host.accept_encoded_save(job.encode()).unwrap();
    let saved = host.player(&owner).unwrap().last_save_document.as_ref().unwrap();
    assert_eq!(saved, &expected.0);
    let restored = solomon_assets::website_save::restore(saved).unwrap();
    assert_eq!(restored.session.participants[&owner].player.selection.name, "Detached");
    assert_eq!(
        restored.loaded_boneyard.as_ref().unwrap().run_id,
        restored.session.run.run_id.unwrap()
    );
    assert!(host.state_for_player(&owner).is_none(), "the projection cannot insert a live actor");
    assert_eq!(
        host.rejoin_slots.iter().find(|slot| slot.player_id == owner).unwrap().token,
        expected.1
    );
    assert!(host.pending_player_saves.contains(&owner));
    server.shutdown();
}

#[test]
fn permanent_disconnect_retires_queued_work_and_ignores_its_captured_completion() {
    use crate::transport::TransportClose;
    let mut supervisor = standalone();
    let mut server = server();
    let (owner_peer, _socket) = peer(&server, "/game");
    let peer_id = owner_peer.id();
    let owner = admit(&mut supervisor.colleges[0].host, owner_peer, "Departing");
    let host = &mut supervisor.colleges[0].host;
    let job = host.capture_pending_save().unwrap();
    host.set_enhanced_effects(&owner, false);
    assert!(host.pending_player_saves.contains(&owner));
    let connection = host.connections.remove(&peer_id).unwrap();
    host.disconnected(connection, 10.0, TransportClose { code: Some(1000), reason: "done".into() })
        .unwrap();
    assert!(host.player(&owner).is_none());
    assert!(
        !host.pending_player_saves.contains(&owner),
        "permanent departure retires the queued obligation"
    );
    host.accept_encoded_save(job.encode()).unwrap();
    assert!(
        host.pending_player_saves.is_empty(),
        "an old completion cannot recreate departed work"
    );
    assert!(host.capture_pending_save().is_none());
    server.shutdown();
}

#[test]
fn retained_reconnect_actor_keeps_its_pending_save() {
    use crate::transport::TransportClose;
    let mut supervisor = supervisor();
    let mut server = server();
    let (owner_peer, _socket) = peer(&server, "/game-hub");
    let peer_id = owner_peer.id();
    let owner = admit(&mut supervisor.colleges[0].host, owner_peer, "Retained");
    let host = &mut supervisor.colleges[0].host;
    let connection = host.connections.remove(&peer_id).unwrap();
    host.disconnected(
        connection,
        10.0,
        TransportClose { code: Some(1006), reason: "transport lost".into() },
    )
    .unwrap();
    assert!(host.player(&owner).is_some());
    assert!(host.pending_player_saves.contains(&owner));
    server.shutdown();
}
