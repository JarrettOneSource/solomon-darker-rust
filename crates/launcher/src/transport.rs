//! Native text WebSocket delivery beside the packaged asset server.
//!
//! `host/game-host.ts`, `client/game-transport.ts` and `websocket-heartbeat.ts` own the
//! reference transport contracts. Gameplay admission and the one authoritative session
//! remain with the caller; socket workers never interpret JSON or step a game.

mod http;
mod socket;
#[cfg(test)]
mod tests;
mod transfer;

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::AssetServer;

pub use http::{HostedHttpRequest, HostedHttpResponse};

const HTTP_WORKERS: usize = 4;
const HTTP_QUEUE_CAPACITY: usize = 64;

/// Opaque connection identity. IDs increase for the lifetime of this transport and are
/// never reused. The authority assigns stable player identities separately.
pub type PeerId = u64;

#[derive(Clone, Debug)]
pub struct TransportConfig {
    /// Empty means the browser Origin must match the request's HTTP Host exactly.
    /// A trusted HTTPS proxy may supply an explicit list of complete allowed origins.
    pub allowed_origins: Vec<String>,
    pub max_peers: usize,
    pub event_capacity: usize,
    pub reliable_message_capacity: usize,
    pub reliable_byte_capacity: usize,
    pub max_message_bytes: usize,
    pub handshake_timeout: Duration,
    pub io_poll_interval: Duration,
    pub heartbeat_interval: Duration,
    pub missed_pong_limit: usize,
    pub close_timeout: Duration,
}

impl Default for TransportConfig {
    fn default() -> Self {
        // The source permits an escaped 96 MiB save document in client-hello. The caller
        // still validates each JSON message's own contract after transport decoding.
        let max_message_bytes = solomon_protocol::GAME_WEBSOCKET_MAX_PAYLOAD_BYTES;
        Self {
            allowed_origins: Vec::new(),
            max_peers: 16,
            event_capacity: 256,
            reliable_message_capacity: 32,
            reliable_byte_capacity: max_message_bytes,
            max_message_bytes,
            handshake_timeout: Duration::from_secs(5),
            io_poll_interval: Duration::from_millis(10),
            heartbeat_interval: Duration::from_secs(5),
            missed_pong_limit: 6,
            close_timeout: Duration::from_secs(1),
        }
    }
}

impl TransportConfig {
    fn validate(&self) -> io::Result<()> {
        if self.max_peers == 0
            || self.event_capacity == 0
            || self.reliable_message_capacity == 0
            || self.reliable_byte_capacity == 0
            || self.max_message_bytes == 0
            || self.missed_pong_limit == 0
            || self.handshake_timeout.is_zero()
            || self.io_poll_interval.is_zero()
            || self.heartbeat_interval.is_zero()
            || self.close_timeout.is_zero()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "transport queue bounds, payload bounds and deadlines must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportClose {
    /// No close code means TCP ended without an accepted WebSocket close frame.
    pub code: Option<u16>,
    pub reason: String,
}

pub enum TransportEvent {
    Connected {
        peer: PeerSender,
        remote_addr: SocketAddr,
        origin: String,
        path: String,
    },
    /// HTTP workers wait for this owned reply; only the supervisor thread may inspect
    /// admission, rooms or parties. Socket workers never borrow a game world.
    Http {
        request: HostedHttpRequest,
        respond: SyncSender<HostedHttpResponse>,
    },
    Message {
        peer_id: PeerId,
        payload: String,
    },
    Disconnected {
        peer_id: PeerId,
        close: TransportClose,
    },
    Failed {
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerSendError {
    Closed,
    PayloadTooLarge,
    ReliableQueueFull,
    InvalidClose,
    WelcomeSidebandLimit,
}

impl fmt::Display for PeerSendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Closed => "the game connection is closed",
            Self::PayloadTooLarge => "the game message exceeds the transport payload bound",
            Self::ReliableQueueFull => "the game connection's reliable queue is full",
            Self::InvalidClose => "the WebSocket close code or reason is invalid",
            Self::WelcomeSidebandLimit => "Welcome sideband queue exceeds its byte limit",
        })
    }
}
impl std::error::Error for PeerSendError {}

#[derive(Default)]
struct PeerOutbox {
    reliable: VecDeque<Arc<str>>,
    reliable_bytes: usize,
    snapshot: Option<Arc<str>>,
    close: Option<TransportClose>,
}

struct PeerState {
    id: PeerId,
    closed: AtomicBool,
    outbox: Mutex<PeerOutbox>,
    config: Arc<TransportConfig>,
    transfers: Mutex<transfer::PeerTransfers>,
}

/// A nonblocking producer for one peer. Large published snapshots can share the same
/// `Arc<str>` across peers; replacing one never queues an obsolete snapshot behind it.
#[derive(Clone)]
pub struct PeerSender(Arc<PeerState>);

impl fmt::Debug for PeerSender {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("PeerSender").field("id", &self.id()).finish()
    }
}

impl PeerSender {
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0.closed.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn id(&self) -> PeerId {
        self.0.id
    }

    pub fn send_reliable(&self, payload: Arc<str>) -> Result<(), PeerSendError> {
        self.validate_payload(&payload)?;
        let mut transfers =
            self.0.transfers.lock().expect("transfer producers never panic while locked");
        if let Some(welcome) = &mut transfers.welcome {
            return welcome.defer(payload, self.0.config.max_message_bytes);
        }
        self.queue_reliable(payload)
    }

    fn queue_reliable(&self, payload: Arc<str>) -> Result<(), PeerSendError> {
        let mut outbox = self.0.outbox.lock().expect("the peer outbox does not panic while locked");
        if outbox.close.is_some() || self.0.closed.load(Ordering::Acquire) {
            return Err(PeerSendError::Closed);
        }
        if outbox.reliable.len() >= self.0.config.reliable_message_capacity
            || payload.len()
                > self.0.config.reliable_byte_capacity.saturating_sub(outbox.reliable_bytes)
        {
            return Err(PeerSendError::ReliableQueueFull);
        }
        outbox.reliable_bytes += payload.len();
        outbox.reliable.push_back(payload);
        Ok(())
    }

    pub fn replace_snapshot(&self, payload: Arc<str>) -> Result<(), PeerSendError> {
        self.validate_payload(&payload)?;
        let transfers =
            self.0.transfers.lock().expect("transfer producers never panic while locked");
        if transfers.welcome.is_some() {
            return Ok(());
        }
        let mut outbox = self.0.outbox.lock().expect("the peer outbox does not panic while locked");
        if outbox.close.is_some() || self.0.closed.load(Ordering::Acquire) {
            return Err(PeerSendError::Closed);
        }
        outbox.snapshot = Some(payload);
        Ok(())
    }

    /// Already accepted reliable messages precede the close frame. The replaceable
    /// snapshot is discarded, and new sends are rejected.
    pub fn close(&self, code: u16, reason: &str) -> Result<(), PeerSendError> {
        if reason.len() > 123
            || !tungstenite::protocol::frame::coding::CloseCode::from(code).is_allowed()
        {
            return Err(PeerSendError::InvalidClose);
        }
        if self.0.closed.load(Ordering::Acquire) {
            return Err(PeerSendError::Closed);
        }
        let mut transfers =
            self.0.transfers.lock().expect("transfer producers never panic while locked");
        transfers.welcome = None;
        transfers.save.close();
        let mut outbox = self.0.outbox.lock().expect("the peer outbox does not panic while locked");
        if self.0.closed.swap(true, Ordering::AcqRel) {
            return Err(PeerSendError::Closed);
        }
        outbox.snapshot = None;
        outbox.close = Some(TransportClose { code: Some(code), reason: reason.to_owned() });
        Ok(())
    }

    fn validate_payload(&self, payload: &str) -> Result<(), PeerSendError> {
        if self.0.closed.load(Ordering::Acquire) {
            return Err(PeerSendError::Closed);
        }
        if payload.len() > self.0.config.max_message_bytes {
            return Err(PeerSendError::PayloadTooLarge);
        }
        Ok(())
    }
}

struct ServerState {
    config: Arc<TransportConfig>,
    loopback_only: bool,
    stopping: AtomicBool,
    hosted_draining: AtomicBool,
    next_connection_id: AtomicU64,
    events: SyncSender<TransportEvent>,
    peers: Mutex<BTreeMap<PeerId, PeerSender>>,
    connections: Mutex<BTreeMap<u64, TcpStream>>,
    socket_workers: Mutex<Vec<JoinHandle<()>>>,
    hosted: bool,
    routes: Mutex<BTreeMap<String, Arc<HostedRoute>>>,
}

struct HostedRoute {
    active: AtomicUsize,
    maximum: usize,
}

/// Transport-only capacity, acquired inside the HTTP handshake and released even when
/// it fails. The route owns no participant, Session, credential or resume state.
struct HostedRouteLease(Arc<HostedRoute>);

impl Drop for HostedRouteLease {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
    }
}

struct ConnectionLease {
    id: u64,
    server: Arc<ServerState>,
}
impl Drop for ConnectionLease {
    fn drop(&mut self) {
        self.server
            .connections
            .lock()
            .expect("connection bookkeeping does not panic while locked")
            .remove(&self.id);
    }
}

struct AcceptedConnection {
    id: u64,
    remote_addr: SocketAddr,
    stream: TcpStream,
    lease: ConnectionLease,
}

/// The authority owns this receiver and drains it before each game tick. A full event
/// channel applies TCP backpressure: Connected and Disconnected are never silently lost.
/// Shutdown closes the receiver first, waking any worker blocked on critical delivery.
pub struct NativeServer {
    address: SocketAddr,
    state: Arc<ServerState>,
    events: Option<Receiver<TransportEvent>>,
    listener_worker: Option<JoinHandle<()>>,
    http_workers: Vec<JoinHandle<()>>,
}

impl NativeServer {
    pub fn start(
        listener: TcpListener,
        assets: AssetServer,
        config: TransportConfig,
    ) -> io::Result<Self> {
        Self::start_front_door(listener, Some(assets), config)
    }

    /// The Website's existing TLS/backend edge owns static files and account APIs.
    /// This listener serves only supervisor HTTP and admitted game WebSockets.
    pub fn start_hosted(listener: TcpListener, config: TransportConfig) -> io::Result<Self> {
        Self::start_front_door(listener, None, config)
    }

    fn start_front_door(
        listener: TcpListener,
        assets: Option<AssetServer>,
        config: TransportConfig,
    ) -> io::Result<Self> {
        config.validate()?;
        let address = listener.local_addr()?;
        let (event_sender, events) = mpsc::sync_channel(config.event_capacity);
        let state = Arc::new(ServerState {
            config: Arc::new(config),
            loopback_only: address.ip().is_loopback(),
            stopping: AtomicBool::new(false),
            hosted_draining: AtomicBool::new(false),
            next_connection_id: AtomicU64::new(1),
            events: event_sender,
            peers: Mutex::new(BTreeMap::new()),
            connections: Mutex::new(BTreeMap::new()),
            socket_workers: Mutex::new(Vec::new()),
            hosted: assets.is_none(),
            routes: Mutex::new(BTreeMap::new()),
        });
        let assets = Arc::new(assets);
        let (http_sender, requests) = mpsc::sync_channel::<AcceptedConnection>(HTTP_QUEUE_CAPACITY);
        let requests = Arc::new(Mutex::new(requests));
        let mut http_workers = Vec::new();
        for _ in 0..HTTP_WORKERS {
            let state = state.clone();
            let assets = assets.clone();
            let requests = requests.clone();
            http_workers.push(thread::spawn(move || {
                loop {
                    let next = requests
                        .lock()
                        .expect("the HTTP receiver does not panic while locked")
                        .recv();
                    let Ok(connection) = next else { break };
                    if state.stopping.load(Ordering::Acquire) {
                        break;
                    }
                    if let Err(error) =
                        socket::serve_connection(connection, assets.as_ref().as_ref(), &state)
                        && !matches!(
                            error.kind(),
                            io::ErrorKind::BrokenPipe
                                | io::ErrorKind::ConnectionReset
                                | io::ErrorKind::UnexpectedEof
                                | io::ErrorKind::TimedOut
                                | io::ErrorKind::WouldBlock
                        )
                    {
                        eprintln!("Could not serve a game connection: {error}");
                    }
                }
            }));
        }
        let listener_state = state.clone();
        let listener_worker = thread::spawn(move || {
            for accepted in listener.incoming() {
                if listener_state.stopping.load(Ordering::Acquire) {
                    break;
                }
                let stream = match accepted {
                    Ok(stream) => stream,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        let _ = listener_state.events.send(TransportEvent::Failed {
                            reason: format!("the native game listener failed: {error}"),
                        });
                        break;
                    }
                };
                let Ok(remote_addr) = stream.peer_addr() else { continue };
                let Ok(retained_stream) = stream.try_clone() else { continue };
                let id = listener_state.next_connection_id.fetch_add(1, Ordering::Relaxed);
                {
                    let mut connections = listener_state
                        .connections
                        .lock()
                        .expect("connection bookkeeping does not panic while locked");
                    if connections.len()
                        >= listener_state.config.max_peers + HTTP_QUEUE_CAPACITY + HTTP_WORKERS
                    {
                        continue;
                    }
                    connections.insert(id, retained_stream);
                }
                let connection = AcceptedConnection {
                    id,
                    remote_addr,
                    stream,
                    lease: ConnectionLease { id, server: listener_state.clone() },
                };
                // A saturated HTTP queue must not block acceptance/shutdown. Dropping this
                // request closes its socket and its lease; no participant was admitted.
                let _ = http_sender.try_send(connection);
            }
        });
        Ok(Self {
            address,
            state,
            events: Some(events),
            listener_worker: Some(listener_worker),
            http_workers,
        })
    }

    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// The source supervisor rejects new HTTP upgrades before checking their route or
    /// origin once deployment begins. Existing sockets still complete their save handshake.
    pub fn begin_hosted_deployment(&self) {
        self.state.hosted_draining.store(true, Ordering::Release);
    }

    /// Install the exact source supervisor path. Replacing a live route is rejected;
    /// the owning supervisor must close it before provisioning another room identity.
    pub fn register_hosted_route(&self, path: String, maximum: usize) -> io::Result<()> {
        if !self.state.hosted || !http::hosted_websocket_path(&path) || maximum == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid hosted game route"));
        }
        let mut routes = self.state.routes.lock().expect("hosted route bookkeeping cannot panic");
        if routes.contains_key(&path) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "hosted route already exists",
            ));
        }
        routes.insert(path, Arc::new(HostedRoute { active: AtomicUsize::new(0), maximum }));
        Ok(())
    }

    pub fn remove_hosted_route(&self, path: &str) {
        self.state.routes.lock().expect("hosted route bookkeeping cannot panic").remove(path);
    }

    #[must_use]
    pub fn hosted_route_connections(&self, path: &str) -> usize {
        self.state
            .routes
            .lock()
            .expect("hosted route bookkeeping cannot panic")
            .get(path)
            .map_or(0, |route| route.active.load(Ordering::Acquire))
    }

    pub fn try_event(&self) -> Result<TransportEvent, TryRecvError> {
        self.events.as_ref().ok_or(TryRecvError::Disconnected)?.try_recv()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<TransportEvent, RecvTimeoutError> {
        self.events.as_ref().ok_or(RecvTimeoutError::Disconnected)?.recv_timeout(timeout)
    }

    /// Joins all owned workers, including incomplete handshakes. No external service or
    /// listener is stopped. Calling it twice is harmless.
    pub fn shutdown(&mut self) {
        if self.state.stopping.swap(true, Ordering::AcqRel) {
            return;
        }
        self.events.take();
        {
            let connections = self
                .state
                .connections
                .lock()
                .expect("connection bookkeeping does not panic while locked");
            for stream in connections.values() {
                let _ = stream.shutdown(Shutdown::Both);
            }
        }
        // Wake the blocking accept without periodic polling or an idle busy loop.
        let wake_address = match self.address.ip() {
            IpAddr::V4(address) if address.is_unspecified() => {
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.address.port())
            }
            IpAddr::V6(address) if address.is_unspecified() => {
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), self.address.port())
            }
            _ => self.address,
        };
        let _ = TcpStream::connect(wake_address);
        if let Some(worker) = self.listener_worker.take() {
            worker.join().expect("the listener worker does not panic");
        }
        for worker in self.http_workers.drain(..) {
            worker.join().expect("the HTTP worker does not panic");
        }
        let workers = std::mem::take(
            &mut *self
                .state
                .socket_workers
                .lock()
                .expect("socket worker bookkeeping does not panic while locked"),
        );
        for worker in workers {
            worker.join().expect("the WebSocket worker does not panic");
        }
    }
}

impl Drop for NativeServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
