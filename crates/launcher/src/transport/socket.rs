//! Bounded HTTP framing and owned connection lifetimes. Asset responses and long-lived
//! WebSockets leave the four framing workers after their complete request headers.

use std::io::{self, BufRead, BufReader, Cursor, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Instant;

use solomon_protocol::WEBSOCKET_PATH;
use tungstenite::error::ProtocolError;
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

/// A paused media reader may retain its response indefinitely. Nonblocking writes
/// keep shutdown cancellable on Windows, where shutdown of a cloned socket does
/// not guarantee that an already blocked send has returned.
struct AssetStream<'a> {
    stream: PrefixedStream,
    server: &'a ServerState,
}

impl Read for AssetStream<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.stream.read(output)
    }
}

impl Write for AssetStream<'_> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        loop {
            if self.server.stopping.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "asset server stopped",
                ));
            }
            match self.stream.write(input) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(self.server.config.io_poll_interval);
                }
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

pub(super) fn serve_connection(
    connection: AcceptedConnection,
    assets: Option<Arc<AssetServer>>,
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
        if let Some(assets) = assets {
            // The complete request headers are already buffered. Only response
            // writes can wait here; they retain their byte offset while paused.
            stream.stream.set_nonblocking(true)?;
            let worker_server = server.clone();
            let worker = thread::spawn(move || {
                let _lease = lease;
                let mut stream = AssetStream { stream, server: &worker_server };
                if let Err(error) = assets.serve_stream(&mut stream)
                    && !matches!(
                        error.kind(),
                        io::ErrorKind::BrokenPipe
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::UnexpectedEof
                    )
                {
                    eprintln!("Could not send a game asset: {error}");
                }
            });
            retain_socket_worker(server, worker);
            return Ok(());
        }
        stream.stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
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
        // Windows blocking send deadlines leave the connection indeterminate.
        // Established sockets use WouldBlock and Tungstenite's retained frame bytes.
        socket.get_ref().stream.set_nonblocking(true)
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

struct SocketClose {
    started: Instant,
    detail: TransportClose,
    queued: bool,
}

/// An established peer's WebSocket and the state its poll loop carries between iterations:
/// the snapshot encoder, the heartbeat, the close in progress, and whether a started frame
/// still has bytes to write.
struct PeerConnection<'a> {
    socket: WebSocket<PrefixedStream>,
    server: &'a ServerState,
    peer: &'a PeerSender,
    snapshots: solomon_protocol::snapshot::SnapshotWriter,
    last_ping: Instant,
    missed_pongs: usize,
    closing: Option<SocketClose>,
    write_pending: bool,
}

fn drive_peer(
    socket: WebSocket<PrefixedStream>,
    server: &ServerState,
    peer: &PeerSender,
) -> TransportClose {
    let mut connection = PeerConnection {
        socket,
        server,
        peer,
        snapshots: solomon_protocol::snapshot::SnapshotWriter::default(),
        last_ping: Instant::now(),
        missed_pongs: 0,
        closing: None,
        write_pending: false,
    };
    loop {
        if server.stopping.load(Ordering::Acquire) {
            return TransportClose { code: None, reason: "native transport stopped".to_owned() };
        }
        if connection.closing.is_none() {
            connection.start_requested_close();
        }
        if let Some(close) = &connection.closing
            && close.started.elapsed() >= server.config.close_timeout
        {
            return close.detail.clone();
        }
        if connection.closing.as_ref().is_none_or(|close| !close.queued) {
            if let Some(close) = connection.send_outbox() {
                return close;
            }
            if let Some(close) = connection.send_close_or_ping() {
                return close;
            }
        }
        // After receiving Close, only flush the started frame and queued reply.
        // A further read may report ConnectionClosed while that flush still has
        // WouldBlock bytes; it cannot establish that the output drained.
        let received = connection.socket.can_read().then(|| connection.socket.read());
        let idle = received.as_ref().is_none_or(|result| {
            matches!(result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock)
        });
        let ended = match received {
            None => None,
            Some(Ok(message)) => connection.receive_message(message),
            Some(Err(error)) => connection.receive_error(error),
        };
        if let Some(close) = ended {
            return close;
        }
        if let Some(close) = connection.flush_frames() {
            return close;
        }
        if idle {
            thread::sleep(server.config.io_poll_interval);
        }
    }
}

/// Each step returns `Some` with the connection's final close when the connection ends there.
impl PeerConnection<'_> {
    /// Starts the close the authority requested, or a heartbeat timeout after too many missed
    /// Pongs. An expired welcome transfer first requests its own close.
    fn start_requested_close(&mut self) {
        if self.peer.expire_welcome(Instant::now()) {
            let _ = self.peer.close(4000, "game welcome transfer timed out");
        }
        let requested = self
            .peer
            .0
            .outbox
            .lock()
            .expect("the peer outbox does not panic while locked")
            .close
            .take();
        if let Some(detail) = requested {
            self.closing = Some(SocketClose { started: Instant::now(), detail, queued: false });
        } else if self.last_ping.elapsed() >= self.server.config.heartbeat_interval
            && self.missed_pongs >= self.server.config.missed_pong_limit
        {
            self.peer.0.closed.store(true, Ordering::Release);
            self.closing = Some(SocketClose {
                started: Instant::now(),
                detail: TransportClose {
                    code: Some(4000),
                    reason: "connection timed out".to_owned(),
                },
                queued: false,
            });
        }
    }

    /// Sends queued reliable messages, then the latest snapshot, until a frame would block.
    fn send_outbox(&mut self) -> Option<TransportClose> {
        enum Publication {
            Reliable(Arc<str>),
            Snapshot(Arc<solomon_protocol::snapshot::PreparedSnapshot>),
        }

        for _ in 0..=self.server.config.reliable_message_capacity {
            if self.write_pending {
                break;
            }
            let payload = {
                let mut outbox =
                    self.peer.0.outbox.lock().expect("the peer outbox does not panic while locked");
                if let Some(payload) = outbox.reliable.pop_front() {
                    outbox.reliable_bytes -= payload.len();
                    Some(Publication::Reliable(payload))
                } else {
                    outbox.snapshot.take().map(Publication::Snapshot)
                }
            };
            let Some(payload) = payload else { break };
            let payload = match payload {
                Publication::Reliable(payload) => payload,
                Publication::Snapshot(document) => match self.snapshots.encode(document) {
                    Ok(payload) => payload,
                    Err(reason) => return Some(TransportClose { code: Some(1011), reason }),
                },
            };
            match self.socket.send(shared_text(payload)) {
                Ok(()) => {}
                Err(Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {
                    // The complete frame, including its remaining byte offset, now
                    // belongs to Tungstenite. Flush it before accepting another.
                    self.write_pending = true;
                }
                Err(error) => return Some(failed_connection(error, self.closing.take())),
            }
        }
        None
    }

    /// While closing, queues the close frame once every reliable message is written;
    /// otherwise sends a heartbeat Ping when one is due.
    fn send_close_or_ping(&mut self) -> Option<TransportClose> {
        if let Some(close) = &mut self.closing {
            let reliable_empty = self
                .peer
                .0
                .outbox
                .lock()
                .expect("the peer outbox does not panic while locked")
                .reliable
                .is_empty();
            if !self.write_pending && reliable_empty {
                match self.socket.close(Some(close_frame(&close.detail))) {
                    Ok(()) => {}
                    Err(Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) => return Some(failed_connection(error, self.closing.take())),
                }
                close.queued = true;
            }
        } else if self.last_ping.elapsed() >= self.server.config.heartbeat_interval {
            match self.socket.send(Message::Ping(Bytes::new())) {
                Ok(()) => {}
                Err(Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Some(failed_connection(error, self.closing.take())),
            }
            self.missed_pongs += 1;
            self.last_ping = Instant::now();
        }
        None
    }

    /// Text goes to the authority and binary data starts a 1003 close. A Pong answers the
    /// heartbeat, and a Close starts or confirms the close.
    fn receive_message(&mut self, message: Message) -> Option<TransportClose> {
        match message {
            Message::Text(payload) if self.closing.is_none() => {
                if self
                    .server
                    .events
                    .send(TransportEvent::Message {
                        peer_id: self.peer.id(),
                        payload: payload.to_string(),
                    })
                    .is_err()
                {
                    return Some(TransportClose {
                        code: None,
                        reason: "the authority stopped receiving transport events".to_owned(),
                    });
                }
            }
            Message::Binary(_) if self.closing.is_none() => {
                self.peer.0.closed.store(true, Ordering::Release);
                let close = TransportClose {
                    code: Some(1003),
                    reason: "game transport accepts text messages only".to_owned(),
                };
                let frame = close_frame(&close);
                self.closing =
                    Some(SocketClose { started: Instant::now(), detail: close, queued: true });
                match self.socket.close(Some(frame)) {
                    Ok(()) => {}
                    Err(Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) => return Some(failed_connection(error, self.closing.take())),
                }
            }
            Message::Pong(_) => self.missed_pongs = 0,
            Message::Close(frame) => {
                // Keep the queued reply through WouldBlock until it drains or the
                // existing close deadline expires, preserving the accepted identity.
                if let Some(close) = &mut self.closing {
                    close.queued = true;
                } else {
                    let detail = TransportClose {
                        // 1005 is local no-status identity, never a transmitted code.
                        // An accepted empty Close is distinct from EOF or I/O loss.
                        code: Some(frame.as_ref().map_or(1005, |frame| frame.code.into())),
                        reason: frame.map_or_else(String::new, |frame| frame.reason.to_string()),
                    };
                    self.closing =
                        Some(SocketClose { started: Instant::now(), detail, queued: true });
                }
            }
            _ => {}
        }
        None
    }

    /// A blocked read waits for the next poll. A finished close handshake or lost connection
    /// ends the connection; malformed input first sends its WebSocket close status.
    fn receive_error(&mut self, error: Error) -> Option<TransportClose> {
        match error {
            Error::Io(error) if error.kind() == io::ErrorKind::WouldBlock => None,
            Error::ConnectionClosed => Some(self.handle_connection_closed()),
            Error::Capacity(_) => {
                Some(reject_frame(&mut self.socket, 1009, "game message exceeds the payload bound"))
            }
            Error::Utf8(_) => {
                Some(reject_frame(&mut self.socket, 1007, "game text must be valid UTF-8"))
            }
            error @ Error::Protocol(ProtocolError::ResetWithoutClosingHandshake) => {
                // Tungstenite's EOF variant carries no malformed WebSocket frame.
                Some(failed_connection(error, self.closing.take()))
            }
            Error::Protocol(_) => {
                Some(reject_frame(&mut self.socket, 1002, "invalid WebSocket frame"))
            }
            error => Some(failed_connection(error, self.closing.take())),
        }
    }

    /// Flushes started frames. A frame that would block stays pending for the next poll.
    fn flush_frames(&mut self) -> Option<TransportClose> {
        // read queues automatic pong and close replies; do not replace them manually.
        self.write_pending = match self.socket.flush() {
            Ok(()) => false,
            Err(Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => true,
            Err(Error::ConnectionClosed) => return Some(self.handle_connection_closed()),
            Err(error) => return Some(failed_connection(error, self.closing.take())),
        };
        None
    }

    /// Tungstenite finished the close handshake: the connection ends with the close in
    /// progress, if any.
    fn handle_connection_closed(&mut self) -> TransportClose {
        self.closing.take().map_or_else(
            || TransportClose { code: None, reason: "connection closed".to_owned() },
            |close| close.detail,
        )
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
fn failed_connection(error: Error, closing: Option<SocketClose>) -> TransportClose {
    closing.map_or_else(
        || TransportClose { code: None, reason: error.to_string() },
        |close| close.detail,
    )
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
