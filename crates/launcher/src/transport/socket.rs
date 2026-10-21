//! HTTP upgrade dispatch and one peer's WebSocket lifecycle. Long-lived sockets leave
//! the four asset workers immediately after their handshake.

use std::io::{self, BufRead, BufReader, Cursor, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Instant;

use solomon_protocol::WEBSOCKET_PATH;
use tungstenite::handshake::server::{Callback, ErrorResponse, Request, Response};
use tungstenite::http::StatusCode;
use tungstenite::protocol::{CloseFrame, WebSocket, WebSocketConfig};
use tungstenite::{Bytes, Error, Message, Utf8Bytes, accept_hdr_with_config};

use super::{
    AcceptedConnection, HostedRouteLease, PeerSender, PeerState, ServerState, TransportClose,
    TransportEvent,
};
use crate::AssetServer;

const MAX_HEADER_BYTES: usize = 16 * 1024;

struct UpgradePolicy<'a> {
    server: &'a ServerState,
    origin: &'a mut String,
    path: &'a mut String,
    route_lease: &'a mut Option<HostedRouteLease>,
}
impl Callback for UpgradePolicy<'_> {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        if self.server.hosted && self.server.hosted_draining.load(Ordering::Acquire) {
            return Err(reject(StatusCode::SERVICE_UNAVAILABLE, "Game Updating"));
        }
        if !self.server.hosted
            && request.uri().path_and_query().map(|uri| uri.as_str()) != Some(WEBSOCKET_PATH)
        {
            return Err(reject(StatusCode::NOT_FOUND, "Unknown game WebSocket path"));
        }
        let request_origin = request.headers().get("origin").and_then(|value| value.to_str().ok());
        let request_host = request.headers().get("host").and_then(|value| value.to_str().ok());
        if !self.server.hosted
            && self.server.loopback_only
            && !request_host.is_some_and(is_loopback_host)
        {
            return Err(reject(StatusCode::FORBIDDEN, "The local game requires a loopback host"));
        }
        let allowed = if self.server.hosted && request_origin.is_none() {
            // The pinned supervisor also permits native tools without an Origin.
            true
        } else if self.server.config.allowed_origins.is_empty() {
            request_origin
                .zip(request_host)
                .is_some_and(|(origin, host)| origin == format!("http://{host}"))
        } else {
            request_origin.is_some_and(|origin| {
                self.server.config.allowed_origins.iter().any(|allowed| origin == allowed)
            })
        };
        if !allowed {
            return Err(reject(StatusCode::FORBIDDEN, "The game browser origin is not allowed"));
        }
        if self.server.hosted {
            let routes = self.server.routes.lock().expect("hosted route bookkeeping cannot panic");
            let route = routes
                .get(request.uri().path())
                .ok_or_else(|| reject(StatusCode::NOT_FOUND, "That game session is unavailable"))?;
            if route
                .active
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                    (active < route.maximum).then_some(active + 1)
                })
                .is_err()
            {
                return Err(reject(StatusCode::SERVICE_UNAVAILABLE, "That game session is full"));
            }
            *self.route_lease = Some(HostedRouteLease(route.clone()));
        }
        *self.origin = request_origin.unwrap_or("").to_owned();
        *self.path = request.uri().path().to_owned();
        Ok(response)
    }
}

fn is_loopback_host(host: &str) -> bool {
    let Ok(authority) = host.parse::<tungstenite::http::uri::Authority>() else { return false };
    matches!(authority.host(), "localhost" | "127.0.0.1" | "[::1]" | "::1")
}

/// Replay the inspected headers and any read-ahead bytes through the protocol library.
/// It still validates the complete request, including rejecting premature frame data.
struct PrefixedStream {
    prefix: Cursor<Vec<u8>>,
    stream: TcpStream,
}
impl Read for PrefixedStream {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.prefix.position() < self.prefix.get_ref().len() as u64 {
            return self.prefix.read(output);
        }
        self.stream.read(output)
    }
}
impl Write for PrefixedStream {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.stream.write(input)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

pub(super) fn serve_connection(
    connection: AcceptedConnection,
    assets: Option<&AssetServer>,
    server: &Arc<ServerState>,
) -> io::Result<()> {
    let AcceptedConnection { id, remote_addr, stream, lease } = connection;
    stream.set_read_timeout(Some(server.config.handshake_timeout))?;
    stream.set_write_timeout(Some(server.config.handshake_timeout))?;
    let mut reader = BufReader::new(stream);
    let mut headers = Vec::new();
    loop {
        let previous_length = headers.len();
        let remaining = MAX_HEADER_BYTES + 1 - previous_length;
        let read = reader.by_ref().take(remaining as u64).read_until(b'\n', &mut headers)?;
        if headers.len() > MAX_HEADER_BYTES {
            return http_error(reader.get_mut(), "431 Request Header Fields Too Large");
        }
        if read == 0 {
            return http_error(reader.get_mut(), "400 Bad Request");
        }
        let line = &headers[previous_length..];
        if line == b"\r\n" || line == b"\n" {
            break;
        }
    }
    let upgrade = String::from_utf8_lossy(&headers).lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("upgrade") && value.trim().eq_ignore_ascii_case("websocket")
        })
    });
    headers.extend_from_slice(reader.buffer());
    let mut stream = PrefixedStream { prefix: Cursor::new(headers), stream: reader.into_inner() };
    if !upgrade {
        stream.stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
        if let Some(assets) = assets {
            return assets.serve_stream(&mut stream);
        }
        // An admission can wait for package execution or another recovering player.
        // Its bounded, leased connection must not occupy a framing worker needed by
        // the first player's WebSocket handshake.
        let worker_server = server.clone();
        let worker = thread::spawn(move || {
            let _lease = lease;
            if let Err(error) = super::http::serve(&mut stream, remote_addr, &worker_server)
                && !matches!(
                    error.kind(),
                    io::ErrorKind::BrokenPipe
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                )
            {
                eprintln!("Could not serve a supervisor request: {error}");
            }
        });
        retain_socket_worker(server, worker);
        return Ok(());
    }

    let peer = PeerSender(Arc::new(PeerState {
        id,
        closed: false.into(),
        outbox: Default::default(),
        transfers: Default::default(),
        config: server.config.clone(),
    }));
    {
        // Count handshakes too, so four concurrent upgrades cannot over-admit the limit.
        let mut peers = server.peers.lock().expect("peer bookkeeping does not panic while locked");
        if peers.len() >= server.config.max_peers {
            return http_error(&mut stream, "503 Service Unavailable");
        }
        peers.insert(id, peer.clone());
    }
    let mut origin = String::new();
    let mut path = String::new();
    let mut route_lease = None;
    let callback = UpgradePolicy {
        server,
        origin: &mut origin,
        path: &mut path,
        route_lease: &mut route_lease,
    };
    let config = WebSocketConfig::default()
        .write_buffer_size(0)
        .max_message_size(Some(server.config.max_message_bytes))
        .max_frame_size(Some(server.config.max_message_bytes))
        .max_write_buffer_size(server.config.max_message_bytes.saturating_add(1024));
    let handshake = accept_hdr_with_config(stream, callback, Some(config));
    let socket = match handshake {
        Ok(socket) => socket,
        Err(_) => {
            server.peers.lock().expect("peer bookkeeping does not panic while locked").remove(&id);
            peer.0.closed.store(true, Ordering::Release);
            return Ok(());
        }
    };
    let configure = || -> io::Result<()> {
        socket.get_ref().stream.set_nodelay(true)?;
        socket.get_ref().stream.set_read_timeout(Some(server.config.io_poll_interval))?;
        socket.get_ref().stream.set_write_timeout(Some(server.config.write_timeout))
    };
    if let Err(error) = configure() {
        finish_peer(server, &peer);
        return Err(error);
    }
    let server = server.clone();
    let worker_server = server.clone();
    let worker = thread::spawn(move || {
        let _lease = lease;
        let _route_lease = route_lease;
        if worker_server
            .events
            .send(TransportEvent::Connected { peer: peer.clone(), remote_addr, origin, path })
            .is_err()
        {
            finish_peer(&worker_server, &peer);
            return;
        }
        let close = drive_peer(socket, &worker_server, &peer);
        if std::env::var_os("SDR_NATIVE_DIAGNOSTICS").is_some() {
            let credential = std::env::var("SDR_GAME_BOOTSTRAP_CREDENTIAL").unwrap_or_default();
            let reason = if credential.is_empty() {
                close.reason.clone()
            } else {
                close.reason.replace(&credential, "[redacted]")
            };
            eprintln!("native socket terminal: peer={id} code={:?} reason={reason:?}", close.code);
        }
        finish_peer(&worker_server, &peer);
        // The bounded channel may block this socket worker, never the authority. Shutdown
        // drops its receiver first, guaranteeing this critical send can finish or fail.
        let _ = worker_server.events.send(TransportEvent::Disconnected { peer_id: id, close });
    });
    retain_socket_worker(&server, worker);
    Ok(())
}

fn retain_socket_worker(server: &ServerState, worker: thread::JoinHandle<()>) {
    let mut workers = server
        .socket_workers
        .lock()
        .expect("socket worker bookkeeping does not panic while locked");
    let mut completed = Vec::new();
    let mut index = 0;
    while index < workers.len() {
        if workers[index].is_finished() {
            completed.push(workers.swap_remove(index));
        } else {
            index += 1;
        }
    }
    workers.push(worker);
    drop(workers);
    for worker in completed {
        worker.join().expect("the completed connection worker does not panic");
    }
}

fn finish_peer(server: &ServerState, peer: &PeerSender) {
    peer.0.closed.store(true, Ordering::Release);
    *peer.0.outbox.lock().expect("the peer outbox does not panic while locked") =
        Default::default();
    server.peers.lock().expect("peer bookkeeping does not panic while locked").remove(&peer.id());
}

fn drive_peer(
    mut socket: WebSocket<PrefixedStream>,
    server: &ServerState,
    peer: &PeerSender,
) -> TransportClose {
    let mut last_ping = Instant::now();
    let mut missed_pongs = 0;
    let mut closing: Option<(Instant, TransportClose)> = None;
    loop {
        if server.stopping.load(Ordering::Acquire) {
            return TransportClose { code: None, reason: "native transport stopped".to_owned() };
        }
        if closing.is_none() {
            if peer.expire_welcome(Instant::now()) {
                let _ = peer.close(4000, "game welcome transfer timed out");
            }
            let outgoing = {
                let mut outbox =
                    peer.0.outbox.lock().expect("the peer outbox does not panic while locked");
                std::mem::take(&mut *outbox)
            };
            for payload in outgoing.reliable {
                if let Err(error) = socket.send(shared_text(payload)) {
                    return failed_connection(error);
                }
            }
            if let Some(payload) = outgoing.snapshot
                && let Err(error) = socket.send(shared_text(payload))
            {
                return failed_connection(error);
            }
            let requested_close = outgoing.close.or_else(|| {
                if last_ping.elapsed() >= server.config.heartbeat_interval
                    && missed_pongs >= server.config.missed_pong_limit
                {
                    Some(TransportClose {
                        code: Some(4000),
                        reason: "connection timed out".to_owned(),
                    })
                } else {
                    None
                }
            });
            if let Some(close) = requested_close {
                peer.0.closed.store(true, Ordering::Release);
                if let Err(error) = socket.close(Some(close_frame(&close))) {
                    return failed_connection(error);
                }
                closing = Some((Instant::now(), close));
            } else if last_ping.elapsed() >= server.config.heartbeat_interval {
                if let Err(error) = socket.send(Message::Ping(Bytes::new())) {
                    return failed_connection(error);
                }
                missed_pongs += 1;
                last_ping = Instant::now();
            }
        }
        if let Some((started, close)) = &closing
            && started.elapsed() >= server.config.close_timeout
        {
            return close.clone();
        }
        match socket.read() {
            Ok(Message::Text(payload)) if closing.is_none() => {
                if server
                    .events
                    .send(TransportEvent::Message {
                        peer_id: peer.id(),
                        payload: payload.to_string(),
                    })
                    .is_err()
                {
                    return TransportClose {
                        code: None,
                        reason: "the authority stopped receiving transport events".to_owned(),
                    };
                }
            }
            Ok(Message::Binary(_)) if closing.is_none() => {
                peer.0.closed.store(true, Ordering::Release);
                let close = TransportClose {
                    code: Some(1003),
                    reason: "game transport accepts text messages only".to_owned(),
                };
                if let Err(error) = socket.close(Some(close_frame(&close))) {
                    return failed_connection(error);
                }
                closing = Some((Instant::now(), close));
            }
            Ok(Message::Pong(_)) => missed_pongs = 0,
            Ok(Message::Close(frame)) => {
                // Tungstenite queued the close reply. Flush it before releasing TCP.
                let _ = socket.flush();
                return closing.map_or_else(
                    || TransportClose {
                        code: frame.as_ref().map(|frame| frame.code.into()),
                        reason: frame.map_or_else(String::new, |frame| frame.reason.to_string()),
                    },
                    |(_, close)| close,
                );
            }
            Ok(_) => {}
            Err(Error::Io(error))
                if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {}
            Err(Error::ConnectionClosed) => {
                return closing.map_or_else(
                    || TransportClose { code: None, reason: "connection closed".to_owned() },
                    |(_, close)| close,
                );
            }
            Err(Error::Capacity(_)) => {
                return reject_frame(&mut socket, 1009, "game message exceeds the payload bound");
            }
            Err(Error::Utf8(_)) => {
                return reject_frame(&mut socket, 1007, "game text must be valid UTF-8");
            }
            Err(Error::Protocol(_)) => {
                return reject_frame(&mut socket, 1002, "invalid WebSocket frame");
            }
            Err(error) => return failed_connection(error),
        }
        // read queues automatic pong and close replies; do not replace them manually.
        if let Err(error) = socket.flush() {
            return failed_connection(error);
        }
    }
}

/// Bytes retains this Arc owner, avoiding a world-body copy for every peer. Conversion
/// validates UTF-8 without allocation; the input already came from a Rust string.
struct SharedText(Arc<str>);
impl AsRef<[u8]> for SharedText {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
fn shared_text(payload: Arc<str>) -> Message {
    Message::Text(
        Utf8Bytes::try_from(Bytes::from_owner(SharedText(payload)))
            .expect("a shared Rust string is valid UTF-8"),
    )
}

fn close_frame(close: &TransportClose) -> CloseFrame {
    CloseFrame {
        code: close.code.expect("a requested close has a WebSocket code").into(),
        reason: close.reason.clone().into(),
    }
}
fn failed_connection(error: Error) -> TransportClose {
    TransportClose { code: None, reason: error.to_string() }
}
fn reject_frame(socket: &mut WebSocket<PrefixedStream>, code: u16, reason: &str) -> TransportClose {
    let close = TransportClose { code: Some(code), reason: reason.to_owned() };
    // Framing failures end the connection; send the RFC close before releasing TCP.
    let _ = socket.close(Some(close_frame(&close)));
    close
}
fn reject(status: StatusCode, reason: &str) -> ErrorResponse {
    let mut response = ErrorResponse::new(Some(reason.to_owned()));
    *response.status_mut() = status;
    response
}
fn http_error(stream: &mut impl Write, status: &str) -> io::Result<()> {
    write!(stream, "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}
