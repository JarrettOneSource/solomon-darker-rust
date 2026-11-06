use super::*;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc as sync;
use std::time::Instant;

use solomon_protocol::{
    ServerMessage, SnapshotFrame,
    hosted::{HostedServerMessage, SaveCheckpointReason},
};
use solomon_sim::session::{GameCheckpoint, GameSession, GameSessionOptions};
use tungstenite::WebSocket;

use crate::network::{NativeSession, NetworkEvent};

type Server = WebSocket<TcpStream>;

#[expect(
    clippy::result_large_err,
    reason = "Tungstenite requires its HTTP response error in the handshake callback"
)]
fn server(handler: impl FnOnce(Server) + Send + 'static) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
        let socket = tungstenite::accept_hdr(
            stream,
            |request: &tungstenite::handshake::server::Request, response| {
                assert_eq!(request.headers()["Origin"], crate::platform::NATIVE_ORIGIN);
                assert_eq!(request.uri().path(), "/game");
                Ok(response)
            },
        )
        .unwrap();
        handler(socket);
    });
    (format!("ws://{address}/game"), thread)
}

fn command(socket: &mut Server) -> ClientMessage {
    serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap()
}

fn send(socket: &mut Server, message: &ServerMessage<Box<GameCheckpoint>>) {
    socket.send(Message::Text(serde_json::to_string(message).unwrap().into())).unwrap();
}

fn frame(sequence: u64) -> SnapshotFrame<Box<GameCheckpoint>> {
    SnapshotFrame {
        sequence,
        flame_presentation: None,
        input_acknowledgements: Default::default(),
        host: Default::default(),
        snapshot: Box::new(
            GameSession::new_participants(Vec::new(), GameSessionOptions::default())
                .unwrap()
                .checkpoint(),
        ),
    }
}

fn wait_for(mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(start.elapsed() < Duration::from_secs(4), "connection did not make progress");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn connect(endpoint: &str, welcome: bool) -> NativeConnection {
    let connection = NativeConnection::connect_to(
        endpoint,
        crate::platform::NATIVE_ORIGIN,
        first_message(welcome),
    )
    .unwrap();
    wait_for(|| connection.is_open());
    connection
}

fn first_message(welcome: bool) -> ClientMessage {
    use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
    if !welcome {
        return ClientMessage::InspectSession { resume: None };
    }
    ClientMessage::Hello {
        version: solomon_protocol::PROTOCOL_VERSION,
        enhanced_effects: true,
        selection: NewGameSelection {
            name: "Aurelia".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        resume: None,
        initial_save: None,
        begin_college_intro: false,
    }
}

#[test]
fn native_admission_starts_before_the_renderer_polls() {
    let (admitted, received) = sync::channel();
    let (endpoint, server) = server(move |mut socket| {
        let message = socket.read().unwrap();
        if let Message::Text(text) = message {
            assert!(matches!(
                serde_json::from_str::<ClientMessage>(&text).unwrap(),
                ClientMessage::InspectSession { resume: None }
            ));
            admitted.send(()).unwrap();
            assert!(matches!(socket.read().unwrap(), Message::Close(_)));
        }
    });
    crate::platform::configure_native_location("http://native.test/game/", None).unwrap();
    let session =
        NativeSession::connect_to(&endpoint, ClientMessage::InspectSession { resume: None })
            .unwrap();
    // The native transport keeps running while an expensive frame occupies the UI.
    let admission = received.recv_timeout(Duration::from_secs(3));
    drop(session);
    server.join().unwrap();
    assert!(admission.is_ok(), "native admission waited for the renderer to poll");
}

#[test]
fn rejected_input_release_remains_unsent_until_transport_admits_it() {
    use solomon_sim::{Vec2, session::GameSessionInput};

    let (sent, received) = sync::channel();
    let (endpoint, server) = server(move |mut socket| {
        assert!(matches!(command(&mut socket), ClientMessage::InspectSession { resume: None }));
        loop {
            match socket.read().unwrap() {
                Message::Text(text) => {
                    sent.send(serde_json::from_str::<ClientMessage>(&text).unwrap()).unwrap();
                }
                Message::Close(_) => break,
                message => panic!("unexpected game message: {message:?}"),
            }
        }
    });
    crate::platform::configure_native_location("http://native.test/game/", None).unwrap();
    let mut session =
        NativeSession::connect_to(&endpoint, ClientMessage::InspectSession { resume: None })
            .unwrap();
    wait_for(|| session.is_open());
    let active = GameSessionInput {
        movement: Vec2::new(1.0, 0.0),
        primary: true,
        quickbar: Some(3),
        ..GameSessionInput::default()
    };
    session.send_input(10, active, false).unwrap();
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(3)).unwrap(),
        ClientMessage::Input { sequence: 1, target_tick: 11, input } if input == active
    ));
    wait_for(|| session.connection.pending_bytes.load(Ordering::Relaxed) == 0);

    // Force the real transport's admission boundary without a timing-dependent flood.
    session.connection.pending_bytes.store(MAX_PENDING_SEND_BYTES + 1, Ordering::Relaxed);
    let release = session.send_input(10, GameSessionInput::default(), false);
    let fatal_release = session.send_input(10, GameSessionInput::default(), true);
    session.connection.pending_bytes.store(0, Ordering::Relaxed);
    session.send_input(10, GameSessionInput::default(), true).unwrap();
    let delivered = received.recv_timeout(Duration::from_secs(3));
    drop(session);
    server.join().unwrap();

    assert_eq!(release.unwrap_err(), "The game connection is not accepting input");
    assert_eq!(
        fatal_release,
        Err("The game connection is not accepting input".into()),
        "a rejected release must not suppress a later release"
    );
    assert!(matches!(
        delivered.unwrap(),
        ClientMessage::Input { sequence: 2, target_tick: 12, input }
            if input == GameSessionInput::default()
    ));
}

#[test]
fn actual_session_sends_admission_and_retains_the_server_close_reason() {
    let (endpoint, server) = server(|mut socket| {
        assert!(matches!(command(&mut socket), ClientMessage::InspectSession { resume: None }));
        send(
            &mut socket,
            &ServerMessage::SessionStatus {
                version: solomon_protocol::PROTOCOL_VERSION,
                can_resume: true,
                selection: None,
                fresh_join_available: false,
            },
        );
        socket
            .close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "inspection complete".into(),
            }))
            .unwrap();
        let _ = socket.read();
    });
    crate::platform::configure_native_location("http://native.test/game/", None).unwrap();
    let session =
        NativeSession::connect_to(&endpoint, ClientMessage::InspectSession { resume: None })
            .unwrap();
    let mut status = false;
    let mut closed = false;
    wait_for(|| {
        for event in session.poll().events {
            match event {
                NetworkEvent::Message { message, .. } => {
                    assert!(matches!(
                        *message,
                        ServerMessage::SessionStatus { can_resume: true, .. }
                    ));
                    status = true;
                }
                NetworkEvent::Closed { code, reason } => {
                    assert_eq!((code, reason.as_str()), (1000, "inspection complete"));
                    closed = true;
                }
                NetworkEvent::Failed(error) => panic!("{error}"),
                NetworkEvent::Opened => {}
            }
        }
        status && closed
    });
    assert!(!session.is_open());
    drop(session);
    server.join().unwrap();
}

#[test]
fn welcome_waits_for_game_acceptance_and_background_delivery_compacts_snapshots() {
    let (unacknowledged, pending) = sync::channel();
    let (save_acknowledged, delivered) = sync::channel();
    let (endpoint, server) = server(move |mut socket| {
        assert!(matches!(command(&mut socket), ClientMessage::Hello { .. }));
        let welcome = ServerMessage::Welcome {
            version: solomon_protocol::PROTOCOL_VERSION,
            credentials: ResumeCredentials { player_id: "wizard".into(), token: "resume".into() },
            tick_rate: solomon_protocol::GAME_TICK_RATE,
            snapshot_rate: solomon_protocol::SNAPSHOT_RATE,
            hosted: None,
            frame: frame(1),
        };
        let text: Vec<u16> = serde_json::to_string(&welcome).unwrap().encode_utf16().collect();
        let mut sender = solomon_protocol::transfer::GameStringSender::new(text.clone()).unwrap();
        let mut chunks = std::collections::VecDeque::from(sender.start());
        while let Some(chunk) = chunks.pop_front() {
            let next_offset = chunk.offset + chunk.data.len();
            send(
                &mut socket,
                &ServerMessage::WelcomeChunk {
                    data: chunk.data,
                    offset: chunk.offset,
                    total_length: chunk.total_length,
                },
            );
            if next_offset == text.len() {
                break;
            }
            assert!(
                matches!(command(&mut socket), ClientMessage::WelcomeChunkAck { next_offset: received } if received == next_offset)
            );
            chunks.extend(sender.acknowledge(next_offset).unwrap().chunks);
        }
        socket.get_mut().set_read_timeout(Some(Duration::from_millis(100))).unwrap();
        assert!(
            matches!(socket.read(), Err(tungstenite::Error::Io(error)) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
        );
        unacknowledged.send(()).unwrap();
        socket.get_mut().set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        assert!(
            matches!(command(&mut socket), ClientMessage::WelcomeChunkAck { next_offset } if next_offset == text.len())
        );
        let mut writer = solomon_protocol::snapshot::SnapshotWriter::default();
        for sequence in [2, 3] {
            let snapshot =
                solomon_protocol::snapshot::PreparedSnapshot::new(&frame(sequence)).unwrap();
            let text = writer.encode(snapshot).unwrap();
            socket.send(Message::Text(text.to_string().into())).unwrap();
        }
        let save: Vec<u16> = "complete 🧙 save".encode_utf16().collect();
        send(
            &mut socket,
            &ServerMessage::Hosted {
                message: HostedServerMessage::SaveCheckpointChunk {
                    data: save.clone(),
                    offset: 0,
                    total_length: save.len(),
                    reason: SaveCheckpointReason::Progress,
                    sequence: 7,
                },
            },
        );
        assert!(
            matches!(command(&mut socket), ClientMessage::SaveCheckpointChunkAck { sequence: 7, next_offset } if next_offset == save.len())
        );
        save_acknowledged.send(()).unwrap();
        assert!(matches!(socket.read().unwrap(), Message::Close(_)));
    });
    let connection = connect(&endpoint, true);
    pending.recv_timeout(Duration::from_secs(3)).unwrap();
    let messages: Vec<_> = connection
        .poll()
        .events
        .into_iter()
        .filter_map(|event| match event {
            NetworkEvent::Message { message, .. } => Some(message),
            NetworkEvent::Opened => None,
            _ => panic!("unexpected delivery"),
        })
        .collect();
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0].as_ref(), ServerMessage::Welcome { .. }));
    connection.accept_welcome().unwrap();
    // No renderer polls while the worker handles the snapshots and checkpoint ACK.
    delivered.recv_timeout(Duration::from_secs(3)).unwrap();
    let poll = connection.poll();
    let snapshot = poll.snapshot.unwrap();
    assert_eq!(snapshot.frame.sequence, 3);
    assert!(snapshot.received_at_ms.is_finite());
    assert_eq!(poll.events.len(), 1);
    let NetworkEvent::Message { message, .. } = &poll.events[0] else {
        panic!("expected the complete save")
    };
    assert!(
        matches!(message.as_ref(), ServerMessage::Hosted { message: HostedServerMessage::SaveCheckpoint { save, sequence: 7, .. } } if save == "complete 🧙 save")
    );
    drop(connection);
    server.join().unwrap();
}

#[test]
fn pings_continue_while_rendering_is_suspended() {
    let (finished, received) = sync::channel();
    let (endpoint, server) = server(move |mut socket| {
        assert!(matches!(command(&mut socket), ClientMessage::InspectSession { .. }));
        for expected in [1, 2] {
            assert!(
                matches!(command(&mut socket), ClientMessage::Ping { nonce } if nonce == expected)
            );
            send(&mut socket, &ServerMessage::Pong { nonce: expected });
        }
        finished.send(()).unwrap();
        assert!(matches!(socket.read().unwrap(), Message::Close(_)));
    });
    let connection = connect(&endpoint, false);
    connection.start_ping(false).unwrap();
    received.recv_timeout(Duration::from_secs(4)).unwrap();
    wait_for(|| connection.ping_ms().is_some());
    assert!(connection.poll().ping_changed);
    drop(connection);
    server.join().unwrap();
}

#[test]
fn binary_and_invalid_json_fail_once_and_close_the_socket() {
    for (payload, expected) in [
        (Message::Binary(vec![0, 1].into()), "non-text"),
        (Message::Text("{invalid".into()), "invalid message"),
    ] {
        let (send_payload, ready) = sync::channel();
        let (endpoint, server) = server(move |mut socket| {
            assert!(matches!(command(&mut socket), ClientMessage::InspectSession { .. }));
            ready.recv().unwrap();
            socket.send(payload).unwrap();
            let Message::Close(Some(frame)) = socket.read().unwrap() else {
                panic!("missing close")
            };
            assert_eq!(u16::from(frame.code), 4003);
        });
        let connection = connect(&endpoint, false);
        send_payload.send(()).unwrap();
        wait_for(|| !connection.is_open());
        let rejected_admission = connection.accept_welcome();
        let failures: Vec<_> = connection
            .poll()
            .events
            .into_iter()
            .filter_map(|event| match event {
                NetworkEvent::Failed(error) => Some(error),
                _ => None,
            })
            .collect();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains(expected), "{}", failures[0]);
        let rejected_ping = connection.start_ping(false);
        let rejected_send = connection.send(&ClientMessage::Ping { nonce: 1 });
        drop(connection);
        server.join().unwrap();
        assert_eq!(rejected_admission.unwrap_err(), failures[0]);
        assert_eq!(rejected_ping.unwrap_err(), failures[0]);
        assert_eq!(rejected_send.unwrap_err(), failures[0]);
    }
}

#[test]
fn dropping_a_stalled_handshake_cancels_the_socket_worker() {
    use std::io::Read;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("ws://{}/game", listener.local_addr().unwrap());
    let (accepted, received) = sync::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut buffer = [0; 4096];
        assert!(stream.read(&mut buffer).unwrap() > 0);
        accepted.send(()).unwrap();
        assert_eq!(stream.read(&mut buffer).unwrap(), 0);
    });
    let connection = NativeConnection::connect_to(
        &endpoint,
        crate::platform::NATIVE_ORIGIN,
        first_message(true),
    )
    .unwrap();
    received.recv_timeout(Duration::from_secs(2)).unwrap();
    let start = Instant::now();
    drop(connection);
    assert!(start.elapsed() < Duration::from_millis(500));
    server.join().unwrap();
}

#[test]
fn a_rejected_old_resume_does_not_delete_a_new_identity() {
    let path = std::env::temp_dir().join(format!(
        "solomon-resume-{}-{}",
        std::process::id(),
        crate::platform::random_uuid().unwrap()
    ));
    crate::storage::configure_native_storage(path.clone()).unwrap();
    let old = ResumeCredentials { player_id: "old".into(), token: "first".into() };
    let new = ResumeCredentials { player_id: "new".into(), token: "second".into() };
    remember_resume(&old).unwrap();
    remember_resume(&new).unwrap();
    forget_rejected_resume(&old).unwrap();
    assert_eq!(read_resume().unwrap(), Some(new.clone()));
    forget_rejected_resume(&new).unwrap();
    assert_eq!(read_resume().unwrap(), None);
    std::fs::remove_dir_all(path).unwrap();
}
