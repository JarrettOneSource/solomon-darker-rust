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
    let connection =
        NativeConnection::connect_to(endpoint, crate::platform::NATIVE_ORIGIN, welcome).unwrap();
    wait_for(|| connection.is_open());
    connection
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
    let mut session =
        NativeSession::connect_to(&endpoint, ClientMessage::InspectSession { resume: None })
            .unwrap();
    let mut status = false;
    let mut closed = false;
    wait_for(|| {
        for event in session.poll().unwrap().events {
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
        drop(connection);
        server.join().unwrap();
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
    let connection =
        NativeConnection::connect_to(&endpoint, crate::platform::NATIVE_ORIGIN, true).unwrap();
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
