//! Real TCP/HTTP/WebSocket journeys through the same public transport used by the host.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::{CloseFrame, WebSocket};
use tungstenite::{Error, Message};

use super::*;
use solomon_protocol::WEBSOCKET_PATH;

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TestAssets(PathBuf);
impl TestAssets {
    fn new() -> Self {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        // macOS can return the same clock value to parallel tests. The process-local
        // ordinal gives every real HTTP fixture its own directory without retries.
        let ordinal = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("solomon-transport-{unique}-{ordinal}"));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("index.html"), "<title>Solomon Darker</title>").unwrap();
        fs::write(path.join("game.wasm"), b"\0asm0123456789").unwrap();
        Self(path)
    }
    fn start(&self, config: TransportConfig) -> NativeServer {
        NativeServer::start(
            TcpListener::bind(("127.0.0.1", 0)).unwrap(),
            AssetServer::new(&self.0).unwrap(),
            config,
        )
        .unwrap()
    }
}
impl Drop for TestAssets {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn connect(
    server: &NativeServer,
    path: &str,
    origin: Option<&str>,
) -> Result<WebSocket<TcpStream>, Error> {
    let mut request = format!("ws://{}{path}", server.local_addr()).into_client_request().unwrap();
    if let Some(origin) = origin {
        request.headers_mut().insert("origin", origin.parse().unwrap());
    }
    open_request(server, request)
}

fn open_request(
    server: &NativeServer,
    request: tungstenite::http::Request<()>,
) -> Result<WebSocket<TcpStream>, Error> {
    let stream = TcpStream::connect(server.local_addr()).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
    tungstenite::client(request, stream).map(|(socket, _)| socket).map_err(|error| match error {
        tungstenite::HandshakeError::Failure(error) => error,
        tungstenite::HandshakeError::Interrupted(_) => panic!("a blocking handshake completes"),
    })
}

fn next_event(server: &NativeServer) -> TransportEvent {
    server.recv_timeout(Duration::from_secs(3)).expect("the loopback peer publishes its event")
}
fn connected(server: &NativeServer) -> PeerSender {
    let TransportEvent::Connected { peer, remote_addr, origin, path } = next_event(server) else {
        panic!("a new WebSocket publishes Connected first");
    };
    assert!(remote_addr.ip().is_loopback());
    assert_eq!(origin, format!("http://{}", server.local_addr()));
    assert_eq!(path, WEBSOCKET_PATH);
    peer
}
fn next_message(client: &mut WebSocket<TcpStream>) -> Message {
    loop {
        let message = client.read().unwrap();
        if matches!(message, Message::Ping(_)) {
            client.flush().unwrap();
            continue;
        }
        return message;
    }
}
fn finish_close(client: &mut WebSocket<TcpStream>) {
    let _ = client.flush();
}

#[test]
fn game_sockets_bypass_asset_workers_and_keep_text_order_and_close_identity() {
    let assets = TestAssets::new();
    let server = assets.start(TransportConfig::default());
    let origin = format!("http://{}", server.local_addr());
    let mut clients = Vec::new();
    let mut previous_id = 0;
    for _ in 0..6 {
        let client = connect(&server, WEBSOCKET_PATH, Some(&origin)).unwrap();
        let peer = connected(&server);
        assert!(peer.id() > previous_id);
        previous_id = peer.id();
        clients.push((client, peer));
    }
    // Six open sockets would exhaust the old four-worker static pool if upgrades stayed
    // in those workers. MIME and byte ranges still use the original asset responder.
    let mut http = TcpStream::connect(server.local_addr()).unwrap();
    http.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    http.write_all(b"GET /game.wasm HTTP/1.1\r\nRange: bytes=0-3\r\n\r\n").unwrap();
    let mut response = Vec::new();
    http.read_to_end(&mut response).unwrap();
    assert!(response.starts_with(b"HTTP/1.1 206 Partial Content\r\n"));
    assert!(String::from_utf8_lossy(&response).contains("Content-Type: application/wasm\r\n"));
    assert!(response.ends_with(b"\0asm"));

    let (client, peer) = &mut clients[0];
    client.send(Message::Text("accepted input".into())).unwrap();
    let TransportEvent::Message { peer_id, payload } = next_event(&server) else {
        panic!("text is delivered to authority");
    };
    assert_eq!((peer_id, payload.as_str()), (peer.id(), "accepted input"));
    peer.send_reliable(Arc::from("control-one")).unwrap();
    peer.send_reliable(Arc::from("control-two")).unwrap();
    assert_eq!(next_message(client), Message::Text("control-one".into()));
    assert_eq!(next_message(client), Message::Text("control-two".into()));
    peer.replace_snapshot(Arc::from("shared world 🧙")).unwrap();
    assert_eq!(next_message(client), Message::Text("shared world 🧙".into()));
    client.send(Message::Ping(b"live".to_vec().into())).unwrap();
    assert_eq!(next_message(client), Message::Pong(b"live".to_vec().into()));
    peer.send_reliable(Arc::from("replacement notice")).unwrap();
    peer.close(4002, "session replaced").unwrap();
    assert_eq!(peer.send_reliable(Arc::from("too late")), Err(PeerSendError::Closed));
    assert_eq!(next_message(client), Message::Text("replacement notice".into()));
    let Message::Close(Some(close)) = next_message(client) else { panic!("close frame arrives") };
    assert_eq!(u16::from(close.code), 4002);
    finish_close(client);
    let TransportEvent::Disconnected { peer_id, close } = next_event(&server) else {
        panic!("the closed peer leaves authority");
    };
    assert_eq!(peer_id, peer.id());
    assert_eq!(close.code, Some(4002));
}

#[test]
fn upgrade_origin_binary_and_heartbeat_contracts_are_checked_on_real_sockets() {
    let assets = TestAssets::new();
    let server = assets.start(TransportConfig {
        max_message_bytes: 1024,
        heartbeat_interval: Duration::from_millis(30),
        missed_pong_limit: 2,
        close_timeout: Duration::from_millis(50),
        ..TransportConfig::default()
    });
    let origin = format!("http://{}", server.local_addr());
    for (path, requested_origin, expected_status) in [
        ("/wrong", Some(origin.as_str()), 404),
        ("/session?unexpected=1", Some(origin.as_str()), 404),
        (WEBSOCKET_PATH, None, 403),
        (WEBSOCKET_PATH, Some("http://unrelated.example"), 403),
    ] {
        let Error::Http(response) = connect(&server, path, requested_origin).unwrap_err() else {
            panic!("invalid upgrade receives an explicit HTTP rejection");
        };
        assert_eq!(response.status().as_u16(), expected_status);
    }
    let mut rebound =
        format!("ws://{}{WEBSOCKET_PATH}", server.local_addr()).into_client_request().unwrap();
    rebound.headers_mut().insert("host", "unrelated.example".parse().unwrap());
    rebound.headers_mut().insert("origin", "http://unrelated.example".parse().unwrap());
    let Error::Http(response) = open_request(&server, rebound).unwrap_err() else {
        panic!("a loopback listener rejects a rebound Host even when its Origin matches");
    };
    assert_eq!(response.status().as_u16(), 403);
    assert!(matches!(server.try_event(), Err(TryRecvError::Empty)));
    let mut binary = connect(&server, WEBSOCKET_PATH, Some(&origin)).unwrap();
    let peer = connected(&server);
    binary.send(Message::Binary(vec![1, 2, 3].into())).unwrap();
    let Message::Close(Some(frame)) = next_message(&mut binary) else {
        panic!("binary payload receives unsupported-data close");
    };
    assert_eq!(u16::from(frame.code), 1003);
    finish_close(&mut binary);
    let TransportEvent::Disconnected { peer_id, close } = next_event(&server) else {
        panic!("binary peer is removed");
    };
    assert_eq!(peer_id, peer.id());
    assert_eq!(close.code, Some(1003));

    let mut oversized = connect(&server, WEBSOCKET_PATH, Some(&origin)).unwrap();
    let peer = connected(&server);
    oversized.send(Message::Text("x".repeat(1025).into())).unwrap();
    let Message::Close(Some(frame)) = next_message(&mut oversized) else {
        panic!("oversized text receives the message-too-big close");
    };
    assert_eq!(u16::from(frame.code), 1009);
    let TransportEvent::Disconnected { peer_id, close } = next_event(&server) else {
        panic!("oversized peer is removed");
    };
    assert_eq!(peer_id, peer.id());
    assert_eq!(close.code, Some(1009));

    let _silent = connect(&server, WEBSOCKET_PATH, Some(&origin)).unwrap();
    let peer = connected(&server);
    // A client that never reads cannot answer heartbeat pings. It is retired without
    // blocking any authority tick or waiting forever for a close acknowledgement.
    let TransportEvent::Disconnected { peer_id, close } = next_event(&server) else {
        panic!("silent peer times out");
    };
    assert_eq!(peer_id, peer.id());
    assert_eq!(close.code, Some(4000));
}

#[test]
fn critical_events_survive_pressure_and_shutdown_interrupts_incomplete_requests() {
    let assets = TestAssets::new();
    let mut server =
        assets.start(TransportConfig { event_capacity: 1, ..TransportConfig::default() });
    let origin = format!("http://{}", server.local_addr());
    let mut clients = Vec::new();
    for _ in 0..2 {
        let mut client = connect(&server, WEBSOCKET_PATH, Some(&origin)).unwrap();
        client.close(Some(CloseFrame { code: 1000.into(), reason: "finished".into() })).unwrap();
        clients.push(client);
    }
    let mut arrivals = BTreeSet::new();
    let mut departures = BTreeSet::new();
    for _ in 0..4 {
        match next_event(&server) {
            TransportEvent::Connected { peer, .. } => {
                arrivals.insert(peer.id());
            }
            TransportEvent::Disconnected { peer_id, .. } => {
                departures.insert(peer_id);
            }
            TransportEvent::Message { .. } => panic!("there were no gameplay messages"),
            TransportEvent::Http { .. } => panic!("there were no hosted HTTP requests"),
            TransportEvent::Failed { reason } => panic!("the loopback listener failed: {reason}"),
        }
    }
    assert_eq!(arrivals.len(), 2);
    assert_eq!(arrivals, departures);

    // Fill critical delivery again and hold an unfinished HTTP request. Shutdown must
    // wake channel sends, socket reads and the acceptor, rather than waiting for deadlines.
    let _first = connect(&server, WEBSOCKET_PATH, Some(&origin)).unwrap();
    let _second = connect(&server, WEBSOCKET_PATH, Some(&origin)).unwrap();
    let mut partial_http = TcpStream::connect(server.local_addr()).unwrap();
    partial_http.write_all(b"GET /").unwrap();
    let started = Instant::now();
    server.shutdown();
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(matches!(server.try_event(), Err(TryRecvError::Disconnected)));
    server.shutdown();
    assert!(TcpStream::connect(server.local_addr()).is_err());
}

#[test]
fn reliable_backpressure_and_shared_snapshot_replacement_are_bounded() {
    // Exercise the producer while its consumer is stalled. These are public queue
    // contracts: accepted reliable messages remain ordered, obsolete snapshots release
    // their shared owner, and overload returns a concrete error without blocking.
    let peer = PeerSender(Arc::new(PeerState {
        transfers: Default::default(),
        id: 7,
        closed: false.into(),
        outbox: Default::default(),
        config: Arc::new(TransportConfig {
            max_message_bytes: 16,
            reliable_message_capacity: 2,
            reliable_byte_capacity: 8,
            ..TransportConfig::default()
        }),
    }));
    peer.send_reliable(Arc::from("one")).unwrap();
    peer.send_reliable(Arc::from("two")).unwrap();
    assert_eq!(peer.send_reliable(Arc::from("3")), Err(PeerSendError::ReliableQueueFull));
    assert_eq!(
        peer.replace_snapshot(Arc::from("seventeen bytes!!")),
        Err(PeerSendError::PayloadTooLarge)
    );
    let obsolete: Arc<str> = Arc::from("old frame");
    let current: Arc<str> = Arc::from("new frame");
    peer.replace_snapshot(obsolete.clone()).unwrap();
    peer.replace_snapshot(current.clone()).unwrap();
    assert_eq!(Arc::strong_count(&obsolete), 1);
    assert_eq!(Arc::strong_count(&current), 2);
    peer.close(4001, "host ended").unwrap();
    assert_eq!(peer.replace_snapshot(current), Err(PeerSendError::Closed));
    assert_eq!(peer.close(1006, "reserved code"), Err(PeerSendError::InvalidClose));
}

#[test]
fn hosted_http_frames_requests_and_routes_world_sockets_without_static_assets() {
    let mut server = NativeServer::start_hosted(
        TcpListener::bind(("127.0.0.1", 0)).unwrap(),
        TransportConfig {
            allowed_origins: vec!["https://staging.example".into()],
            ..TransportConfig::default()
        },
    )
    .unwrap();
    let hub = solomon_protocol::hosted::GAME_HUB_PATH;
    let college = "/game-sessions/abcdefghijklmnopqrstuvwxyzABCDEF";
    server.register_hosted_route(hub.into(), 1).unwrap();
    server.register_hosted_route(college.into(), 1).unwrap();
    assert!(server.register_hosted_route(hub.into(), 1).is_err());
    assert!(server.register_hosted_route("/game-sessions/not-an-id".into(), 1).is_err());

    // HttpClient.JsonContent can stream chunked requests. Both framings deliver the
    // same owned bytes to the authority; these workers have no access to its worlds.
    for framing in [
        "Content-Length: 7\r\n\r\n{\"x\":1}",
        "Transfer-Encoding: chunked\r\n\r\n3\r\n{\"x\r\n4\r\n\":1}\r\n0\r\n\r\n",
    ] {
        let address = server.local_addr();
        let request = format!(
            "POST /admin/hub/tickets?ignored=1 HTTP/1.1\r\nAuthorization: Bearer test-only\r\n{framing}"
        );
        let caller = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            stream.write_all(request.as_bytes()).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            response
        });
        let TransportEvent::Http { request, respond } = next_event(&server) else {
            panic!("a hosted HTTP request reaches the supervisor owner");
        };
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/admin/hub/tickets");
        assert_eq!(request.authorization.as_deref(), Some("Bearer test-only"));
        assert_eq!(request.body, br#"{"x":1}"#);
        respond
            .send(HostedHttpResponse {
                status: 201,
                headers: vec![("Content-Type".into(), "application/json".into())],
                body: br#"{"path":"/game-hub"}"#.to_vec(),
            })
            .unwrap();
        let response = caller.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 201 Created\r\n"));
        assert!(response.ends_with(r#"{"path":"/game-hub"}"#));
    }

    let first =
        connect(&server, "/game-hub?source-allows-query=1", Some("https://staging.example"))
            .unwrap();
    let TransportEvent::Connected { path, origin, .. } = next_event(&server) else {
        panic!("the admitted College socket carries its exact owner route");
    };
    assert_eq!(path, hub);
    assert_eq!(origin, "https://staging.example");
    assert_eq!(server.hosted_route_connections(hub), 1);
    let Error::Http(full) = connect(&server, hub, Some("https://staging.example")).unwrap_err()
    else {
        panic!("source per-world proxy capacity rejects the HTTP upgrade");
    };
    assert_eq!(full.status().as_u16(), 503);
    let second = connect(&server, college, None).unwrap();
    let TransportEvent::Connected { path, origin, .. } = next_event(&server) else {
        panic!("a different actual College has its independent transport slot");
    };
    assert_eq!(path, college);
    assert!(origin.is_empty());
    server.remove_hosted_route(college);
    let Error::Http(missing) = connect(&server, college, None).unwrap_err() else {
        panic!("a closed College cannot accept a new socket");
    };
    assert_eq!(missing.status().as_u16(), 404);
    drop((first, second));
    server.shutdown();
}

#[test]
fn pending_admissions_do_not_block_their_players_websocket_upgrades() {
    let mut server = NativeServer::start_hosted(
        TcpListener::bind(("127.0.0.1", 0)).unwrap(),
        TransportConfig::default(),
    )
    .unwrap();
    server.register_hosted_route("/game-hub".into(), 1).unwrap();
    let mut callers = Vec::new();
    let mut replies = Vec::new();
    for _ in 0..HTTP_WORKERS {
        let mut stream = TcpStream::connect(server.local_addr()).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        stream.write_all(b"POST /admin/rejoin HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}").unwrap();
        let TransportEvent::Http { respond, .. } = next_event(&server) else {
            panic!("the supervisor retains an unfinished recovery reply")
        };
        callers.push(stream);
        replies.push(respond);
    }
    // The authority intentionally has not replied to any HTTP request. The first
    // recovering player can still upgrade and publish its Hello to unblock them.
    let _client = connect(&server, "/game-hub", None).unwrap();
    assert!(matches!(next_event(&server), TransportEvent::Connected { .. }));
    for (reply, mut caller) in replies.into_iter().zip(callers) {
        reply.send(HostedHttpResponse::empty(201)).unwrap();
        let mut response = String::new();
        caller.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 201 Created\r\n"));
    }
    server.shutdown();
}
