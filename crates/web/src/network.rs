//! The browser's native WebSocket boundary. The shared protocol carries the canonical
//! session; this owner only delivers ordered reliable events and the latest full snapshot.

mod transfer;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use solomon_protocol::client_input::ClientInputSequence;
use solomon_protocol::{
    ClientMessage, GameCommand, HostSnapshot, ResumeCredentials, ServerMessage, SnapshotFrame,
};
use solomon_sim::session::{GameCheckpoint, GameSessionInput};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket, Window};

const RESUME_KEY: &str = "solomon-darker-rust-native-resume-v1";
const MAX_RELIABLE_EVENTS: usize = 256;
const MAX_FRAME_BYTES: usize = solomon_protocol::GAME_WEBSOCKET_MAX_PAYLOAD_BYTES;
const MAX_PENDING_SEND_BYTES: u32 = 1024 * 1024;

pub type NativeServerMessage = ServerMessage<Box<GameCheckpoint>>;
pub type NativeSnapshot = SnapshotFrame<Box<GameCheckpoint>>;

pub struct ReceivedNativeSnapshot {
    pub frame: Box<NativeSnapshot>,
    pub received_at_ms: f64,
}

pub enum NetworkEvent {
    Opened,
    Message { message: Box<NativeServerMessage>, received_at_ms: f64 },
    Closed { code: u16, reason: String },
    Failed(String),
}

#[derive(Default)]
pub struct NetworkPoll {
    pub events: Vec<NetworkEvent>,
    pub snapshot: Option<ReceivedNativeSnapshot>,
    pub ping_changed: bool,
}

#[derive(Default)]
struct Inbox {
    poll: NetworkPoll,
    failed: bool,
    receiver: transfer::MessageReceiver,
}

pub struct NativeConnection {
    socket: WebSocket,
    inbox: Rc<RefCell<Inbox>>,
    ping: Rc<RefCell<ConnectionPing>>,
    welcome_deadline: Rc<WelcomeDeadline>,
    // DOM callbacks must outlive their WebSocket and are detached before dropping.
    _callbacks: Vec<Closure<dyn FnMut(Event)>>,
}

/// One browser connection's admission, command and input sequence. Applying authoritative
/// state belongs to Game; this owner acknowledges a snapshot only after Game accepts it.
pub struct NativeSession {
    connection: Rc<NativeConnection>,
    first_message: Option<ClientMessage>,
    input: ClientInputSequence,
    command_sequence: u64,
    pub credentials: Option<ResumeCredentials>,
    pub host: HostSnapshot,
    pub snapshot_sequence: u64,
    ping_timer: Option<PingTimer>,
}

/// `game-client-session.ts` and `game-observer-session.ts` share the same
/// browser-clock round trip and nonce sequence. The timer survives a suspended rAF.
struct ConnectionPing {
    next_nonce: u64,
    pending: BTreeMap<u64, f64>,
    milliseconds: Option<f64>,
}

struct PingTimer {
    window: Window,
    handle: i32,
    _callback: Closure<dyn FnMut()>,
}

struct WelcomeDeadline {
    window: Window,
    handle: Cell<Option<i32>>,
    callback: Closure<dyn FnMut()>,
}

impl WelcomeDeadline {
    fn refresh(&self) -> Result<(), String> {
        self.clear();
        let handle = self
            .window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                self.callback.as_ref().unchecked_ref(),
                solomon_protocol::transfer::GAME_WELCOME_PROGRESS_TIMEOUT_MS as i32,
            )
            .map_err(|_| "The game handshake deadline could not start")?;
        self.handle.set(Some(handle));
        Ok(())
    }

    fn clear(&self) {
        if let Some(handle) = self.handle.take() {
            self.window.clear_timeout_with_handle(handle);
        }
    }
}

impl Drop for WelcomeDeadline {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Drop for PingTimer {
    fn drop(&mut self) {
        self.window.clear_interval_with_handle(self.handle);
    }
}

impl NativeSession {
    pub fn connect(window: &Window, first_message: ClientMessage) -> Result<Self, String> {
        Self::connect_to(window, &local_endpoint(window)?, first_message)
    }

    pub fn connect_to(
        window: &Window,
        endpoint: &str,
        first_message: ClientMessage,
    ) -> Result<Self, String> {
        let connection = Rc::new(NativeConnection::connect_to(window, endpoint)?);
        if !matches!(first_message, ClientMessage::InspectSession { .. }) {
            connection.welcome_deadline.refresh()?;
        }
        Ok(Self {
            connection,
            first_message: Some(first_message),
            input: ClientInputSequence::default(),
            command_sequence: 0,
            credentials: None,
            host: HostSnapshot::default(),
            snapshot_sequence: 0,
            ping_timer: None,
        })
    }

    pub fn poll(&mut self) -> Result<NetworkPoll, String> {
        let poll = self.connection.poll();
        for event in &poll.events {
            if matches!(event, NetworkEvent::Opened)
                && let Some(message) = self.first_message.take()
            {
                self.connection.send(&message)?;
            }
        }
        Ok(poll)
    }

    pub fn send(&self, message: &ClientMessage) -> Result<(), String> {
        self.connection.send(message)
    }

    pub fn accept_welcome(&self) -> Result<(), String> {
        let next_offset =
            self.connection.inbox.borrow_mut().receiver.take_welcome_acknowledgement();
        if let Some(next_offset) = next_offset {
            self.send(&ClientMessage::WelcomeChunkAck { next_offset })?;
        }
        self.connection.welcome_deadline.clear();
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.connection.is_open()
    }

    pub fn start_ping(&mut self, observer: bool) -> Result<(), String> {
        let window = web_sys::window().ok_or("no window")?;
        let clock = window.performance().ok_or("The browser has no frame clock")?;
        send_ping(&self.connection, &mut self.connection.ping.borrow_mut(), clock.now(), observer)?;
        let connection = self.connection.clone();
        let ping = self.connection.ping.clone();
        let callback = Closure::<dyn FnMut()>::new(move || {
            if !connection.is_open() {
                return;
            }
            if let Err(error) =
                send_ping(&connection, &mut ping.borrow_mut(), clock.now(), observer)
            {
                fail(&connection.socket, &mut connection.inbox.borrow_mut(), error);
            }
        });
        let handle = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                2_000,
            )
            .map_err(|_| "The game connection ping timer could not start.")?;
        self.ping_timer = Some(PingTimer { window, handle, _callback: callback });
        Ok(())
    }

    #[must_use]
    pub fn ping_ms(&self) -> Option<f64> {
        self.connection.ping.borrow().milliseconds
    }

    pub fn send_command(&mut self, command: GameCommand) -> Result<u64, String> {
        self.command_sequence += 1;
        self.send(&ClientMessage::Command { sequence: self.command_sequence, command })?;
        Ok(self.command_sequence)
    }

    pub fn send_input(
        &mut self,
        snapshot_tick: u64,
        input: GameSessionInput,
        held: bool,
    ) -> Result<(), String> {
        if let Some(message) = self.input.queue_input(snapshot_tick, input, held)? {
            self.send(&message)?;
        }
        Ok(())
    }

    pub fn acknowledge_snapshot(
        &mut self,
        sequence: u64,
        input_acknowledgement: u64,
        host: HostSnapshot,
        level_up_held: bool,
    ) -> Result<(), String> {
        self.input.acknowledge(input_acknowledgement);
        if level_up_held
            || host.resume_grace.is_some()
            || self.host.pause != host.pause
            || self.host.resume_grace != host.resume_grace
        {
            self.input.reset();
        }
        self.snapshot_sequence = sequence;
        self.host = host;
        self.send(&ClientMessage::SnapshotAck { sequence })
    }

    pub fn stop_input(&mut self) {
        self.input.reset();
    }
}

fn send_ping(
    connection: &NativeConnection,
    ping: &mut ConnectionPing,
    now: f64,
    observer: bool,
) -> Result<(), String> {
    if !observer {
        ping.pending.retain(|_, sent| now - *sent < 10_000.0);
    }
    let nonce = ping.next_nonce;
    ping.next_nonce = if nonce == 0x7fff_ffff { 1 } else { nonce + 1 };
    ping.pending.insert(nonce, now);
    connection.send(&ClientMessage::Ping { nonce })
}

/// The native launcher marks its own URL. Static browser builds retain their local owner.
pub fn native_host_requested(window: &Window) -> Result<bool, String> {
    let query = window.location().search().map_err(|_| "Could not read the game URL")?;
    let params = web_sys::UrlSearchParams::new_with_str(&query)
        .map_err(|_| "Could not read the game URL parameters")?;
    Ok(params.get("host").as_deref() == Some("1"))
}

fn local_endpoint(window: &Window) -> Result<String, String> {
    let location = window.location();
    let scheme = match location.protocol().map_err(|_| "Could not read the game URL")?.as_str() {
        "https:" => "wss",
        "http:" => "ws",
        _ => return Err("The game must be opened through its launcher or web address".into()),
    };
    let host = location.host().map_err(|_| "Could not read the game host")?;
    let mount = crate::hosted::WebsiteMount::configured(window)?.unwrap_or_default();
    Ok(format!("{scheme}://{host}{}", mount.path(solomon_protocol::WEBSOCKET_PATH)))
}

impl NativeConnection {
    pub fn connect_to(window: &Window, endpoint: &str) -> Result<Self, String> {
        let socket = WebSocket::new(endpoint).map_err(|_| "Could not connect to the game host")?;
        let inbox = Rc::new(RefCell::new(Inbox::default()));
        let timed_out = inbox.clone();
        let timed_out_socket = socket.clone();
        let welcome_deadline = Rc::new(WelcomeDeadline {
            window: window.clone(),
            handle: Cell::new(None),
            callback: Closure::<dyn FnMut()>::new(move || {
                fail(
                    &timed_out_socket,
                    &mut timed_out.borrow_mut(),
                    "The game server handshake timed out.".into(),
                );
            }),
        });
        let ping = Rc::new(RefCell::new(ConnectionPing {
            next_nonce: 1,
            pending: BTreeMap::new(),
            milliseconds: None,
        }));
        let mut callbacks: Vec<Closure<dyn FnMut(Event)>> = Vec::with_capacity(4);

        let opened = inbox.clone();
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            opened.borrow_mut().poll.events.push(NetworkEvent::Opened);
        });
        socket.set_onopen(Some(callback.as_ref().unchecked_ref()));
        callbacks.push(callback);

        let received = inbox.clone();
        let receiver = socket.clone();
        let received_ping = ping.clone();
        let receiving_deadline = welcome_deadline.clone();
        let performance = window.performance().ok_or("The browser has no frame clock")?;
        let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            // Use the browser receipt clock, before decode/render work can delay the frame.
            let received_at_ms = performance.now();
            let mut inbox = received.borrow_mut();
            if inbox.failed {
                return;
            }
            let Some(text) =
                event.dyn_ref::<MessageEvent>().and_then(|event| event.data().as_string())
            else {
                fail(&receiver, &mut inbox, "The game host sent a non-text message".into());
                return;
            };
            if text.len() > MAX_FRAME_BYTES {
                fail(
                    &receiver,
                    &mut inbox,
                    "The game host sent a snapshot beyond the save limit".into(),
                );
                return;
            }
            let reception = serde_json::from_str::<Box<NativeServerMessage>>(&text)
                .map_err(|error| format!("The game host sent an invalid message: {error}"))
                .and_then(|message| inbox.receiver.receive(*message));
            match reception {
                Ok(reception) => {
                    if let Some(acknowledgement) = reception.acknowledgement {
                        let result =
                            if matches!(acknowledgement, ClientMessage::WelcomeChunkAck { .. }) {
                                receiving_deadline
                                    .refresh()
                                    .and_then(|()| send_socket(&receiver, &acknowledgement))
                            } else {
                                send_socket(&receiver, &acknowledgement)
                            };
                        if let Err(error) = result {
                            fail(&receiver, &mut inbox, error);
                            return;
                        }
                    }
                    let Some(message) = reception.message else { return };
                    match message {
                        ServerMessage::Pong { nonce } => {
                            let mut ping = received_ping.borrow_mut();
                            if let Some(sent) = ping.pending.remove(&nonce) {
                                ping.milliseconds = Some((received_at_ms - sent).round().max(0.0));
                                inbox.poll.ping_changed = true;
                            }
                        }
                        ServerMessage::Snapshot { frame } => {
                            inbox.poll.snapshot = Some(ReceivedNativeSnapshot {
                                frame: Box::new(frame),
                                received_at_ms,
                            })
                        }
                        message => {
                            if inbox.poll.events.len() >= MAX_RELIABLE_EVENTS {
                                fail(
                                    &receiver,
                                    &mut inbox,
                                    "The game connection fell too far behind".into(),
                                );
                            } else {
                                inbox.poll.events.push(NetworkEvent::Message {
                                    message: Box::new(message),
                                    received_at_ms,
                                });
                            }
                        }
                    }
                }
                Err(error) => fail(&receiver, &mut inbox, error),
            }
        });
        socket.set_onmessage(Some(callback.as_ref().unchecked_ref()));
        callbacks.push(callback);

        let closed = inbox.clone();
        let closing_deadline = welcome_deadline.clone();
        let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            closing_deadline.clear();
            closed.borrow_mut().receiver.close();
            if let Some(event) = event.dyn_ref::<CloseEvent>() {
                closed
                    .borrow_mut()
                    .poll
                    .events
                    .push(NetworkEvent::Closed { code: event.code(), reason: event.reason() });
            }
        });
        socket.set_onclose(Some(callback.as_ref().unchecked_ref()));
        callbacks.push(callback);

        let failed = inbox.clone();
        let failing_deadline = welcome_deadline.clone();
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            failing_deadline.clear();
            let mut inbox = failed.borrow_mut();
            inbox.receiver.close();
            if !inbox.failed {
                inbox.failed = true;
                inbox
                    .poll
                    .events
                    .push(NetworkEvent::Failed("The game connection was interrupted".into()));
            }
        });
        socket.set_onerror(Some(callback.as_ref().unchecked_ref()));
        callbacks.push(callback);
        Ok(Self { socket, inbox, ping, welcome_deadline, _callbacks: callbacks })
    }

    pub fn poll(&self) -> NetworkPoll {
        std::mem::take(&mut self.inbox.borrow_mut().poll)
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.socket.ready_state() == WebSocket::OPEN
    }

    pub fn send(&self, message: &ClientMessage) -> Result<(), String> {
        send_socket(&self.socket, message)
    }
}

fn send_socket(socket: &WebSocket, message: &ClientMessage) -> Result<(), String> {
    if socket.ready_state() != WebSocket::OPEN {
        return Err("The game host is not connected".into());
    }
    if socket.buffered_amount() > MAX_PENDING_SEND_BYTES {
        return Err("The game connection is not accepting input".into());
    }
    let text = serde_json::to_string(message)
        .map_err(|error| format!("Could not encode the game command: {error}"))?;
    socket.send_with_str(&text).map_err(|_| "Could not send the game command".into())
}

impl Drop for NativeConnection {
    fn drop(&mut self) {
        self.welcome_deadline.clear();
        self.socket.set_onopen(None);
        self.socket.set_onmessage(None);
        self.socket.set_onclose(None);
        self.socket.set_onerror(None);
        if self.socket.ready_state() < WebSocket::CLOSING
            && let Err(error) = self.socket.close()
        {
            web_sys::console::warn_2(&"Could not close the game connection".into(), &error);
        }
    }
}

fn fail(socket: &WebSocket, inbox: &mut Inbox, reason: String) {
    if inbox.failed {
        return;
    }
    inbox.failed = true;
    inbox.receiver.close();
    inbox.poll.snapshot = None;
    inbox.poll.events.push(NetworkEvent::Failed(reason));
    if let Err(error) = socket.close_with_code_and_reason(4003, "invalid host delivery") {
        web_sys::console::warn_2(&"Could not close the invalid game connection".into(), &error);
    }
}

/// Prefer this tab's admitted wizard, as in `save/game-resume-token.ts`. The native host
/// also persists its world across launcher exits, so remember the last profile across tabs
/// for the title's explicit Continue action. A fresh admission does not consume this token.
pub fn read_resume(window: &Window) -> Result<Option<ResumeCredentials>, String> {
    let key = crate::storage::scoped_key(RESUME_KEY)?;
    let storage = window
        .session_storage()
        .map_err(|_| "The browser blocked game reconnection storage")?
        .ok_or("The browser has no game reconnection storage")?;
    let text =
        storage.get_item(&key).map_err(|_| "Could not read game reconnection credentials")?;
    let text = match text {
        Some(text) => Some(text),
        None => window
            .local_storage()
            .map_err(|_| "The browser blocked saved game identity storage")?
            .ok_or("The browser has no saved game identity storage")?
            .get_item(&key)
            .map_err(|_| "Could not read the saved game identity")?,
    };
    let Some(text) = text else { return Ok(None) };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| format!("Could not read game reconnection credentials: {error}"))
}

pub fn remember_resume(window: &Window, credentials: &ResumeCredentials) -> Result<(), String> {
    let key = crate::storage::scoped_key(RESUME_KEY)?;
    let text = serde_json::to_string(credentials)
        .map_err(|error| format!("Could not encode game reconnection credentials: {error}"))?;
    let storage = window
        .session_storage()
        .map_err(|_| "The browser blocked game reconnection storage")?
        .ok_or("The browser has no game reconnection storage")?;
    storage
        .set_item(&key, &text)
        .map_err(|_| "Could not remember game reconnection credentials")?;
    window
        .local_storage()
        .map_err(|_| "The browser blocked saved game identity storage")?
        .ok_or("The browser has no saved game identity storage")?
        .set_item(&key, &text)
        .map_err(|_| "Could not remember the saved game identity".into())
}

/// A replaced tab may hold an obsolete credential. Remove only that pair, preserving
/// the newer credential another tab has already committed to shared local storage.
pub fn forget_rejected_resume(window: &Window, rejected: &ResumeCredentials) -> Result<(), String> {
    let key = crate::storage::scoped_key(RESUME_KEY)?;
    for storage in [window.session_storage(), window.local_storage()] {
        let storage = storage
            .map_err(|_| "The browser blocked saved game identity storage")?
            .ok_or("The browser has no saved game identity storage")?;
        if let Some(text) =
            storage.get_item(&key).map_err(|_| "Could not read the saved game identity")?
        {
            let stored: ResumeCredentials = serde_json::from_str(&text)
                .map_err(|error| format!("Could not read saved game identity: {error}"))?;
            if &stored == rejected {
                storage
                    .remove_item(&key)
                    .map_err(|_| "Could not clear the rejected game identity")?;
            }
        }
    }
    Ok(())
}
