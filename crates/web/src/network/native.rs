//! One cancellable socket worker owns transport, decoding and timers. Its inbox
//! contains reliable events and at most one complete presentation snapshot.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{
        self, Message,
        client::IntoClientRequest,
        protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
    },
};

use super::connection::{ConnectionCore, Delivery, SharedDelivery, encode};
use super::{
    ClientMessage, MAX_FRAME_BYTES, MAX_PENDING_SEND_BYTES, MAX_RELIABLE_EVENTS, NetworkPoll,
    RESUME_KEY, ResumeCredentials,
};

#[cfg(test)]
mod tests;

pub(super) struct NativeConnection {
    delivery: SharedDelivery,
    commands: mpsc::Sender<Command>,
    pending_bytes: Arc<AtomicUsize>,
    stop: watch::Sender<bool>,
    worker: Option<JoinHandle<()>>,
}

enum Command {
    Send(PendingText),
    AcceptWelcome,
    StartPing(bool),
}

struct PendingText {
    text: String,
    length: usize,
    pending_bytes: Arc<AtomicUsize>,
}

impl Drop for PendingText {
    fn drop(&mut self) {
        self.pending_bytes.fetch_sub(self.length, Ordering::Relaxed);
    }
}

impl NativeConnection {
    pub fn connect_to(
        endpoint: &str,
        origin: &str,
        first_message: ClientMessage,
    ) -> Result<Self, String> {
        let url = url::Url::parse(endpoint).map_err(|_| "Invalid game connection URL")?;
        if !matches!(url.scheme(), "ws" | "wss")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err("Invalid game connection URL".into());
        }
        let mut request = endpoint
            .into_client_request()
            .map_err(|error| format!("Invalid game connection: {error}"))?;
        request
            .headers_mut()
            .insert("Origin", origin.parse().map_err(|_| "Invalid game connection origin")?);
        let welcome = !matches!(first_message, ClientMessage::InspectSession { .. });
        let first_message = encode(&first_message)?;
        let delivery = Arc::new(Mutex::new(Delivery::default()));
        let mut core =
            ConnectionCore::new(delivery.clone(), welcome, crate::platform::monotonic_ms());
        let (commands, receiver) = mpsc::channel(MAX_RELIABLE_EVENTS);
        let (stop, stopping) = watch::channel(false);
        let worker = std::thread::Builder::new()
            .name("game-connection".into())
            .spawn(move || {
                let runtime =
                    match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            core.fail(format!("Could not start the game connection: {error}"));
                            return;
                        }
                    };
                runtime.block_on(run(request, first_message, &mut core, receiver, stopping));
                // A cancelled OS DNS lookup may finish independently; it holds no game or
                // window resources. Socket shutdown must not wait for that blocking pool.
                runtime.shutdown_background();
            })
            .map_err(|error| format!("Could not start the game connection: {error}"))?;
        Ok(Self {
            delivery,
            commands,
            pending_bytes: Arc::new(AtomicUsize::new(0)),
            stop,
            worker: Some(worker),
        })
    }

    pub fn poll(&self) -> NetworkPoll {
        std::mem::take(&mut self.delivery.lock().unwrap().poll)
    }
    pub fn is_open(&self) -> bool {
        self.delivery.lock().unwrap().open
    }
    pub fn ping_ms(&self) -> Option<f64> {
        self.delivery.lock().unwrap().ping_ms
    }

    fn disconnected_reason(&self) -> String {
        self.delivery.lock().unwrap().disconnected_reason()
    }

    fn command(&self, command: Command) -> Result<(), String> {
        if !self.is_open() {
            return Err(self.disconnected_reason());
        }
        self.commands.try_send(command).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => {
                "The game connection is not accepting input".into()
            }
            mpsc::error::TrySendError::Closed(_) => self.disconnected_reason(),
        })
    }

    pub fn send(&self, message: &ClientMessage) -> Result<(), String> {
        if !self.is_open() {
            return Err(self.disconnected_reason());
        }
        let text = encode(message)?;
        let length = text.len();
        // Match WebSocket.bufferedAmount: one large admission can enter an empty
        // queue, but subsequent input cannot pile up behind its pending bytes.
        self.pending_bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |pending| {
                (pending <= MAX_PENDING_SEND_BYTES).then(|| pending + length)
            })
            .map_err(|_| "The game connection is not accepting input")?;
        self.command(Command::Send(PendingText {
            text,
            length,
            pending_bytes: self.pending_bytes.clone(),
        }))
    }

    pub fn accept_welcome(&self) -> Result<(), String> {
        self.command(Command::AcceptWelcome)
    }
    pub fn start_ping(&self, observer: bool) -> Result<(), String> {
        self.command(Command::StartPing(observer))
    }
}

impl Drop for NativeConnection {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            crate::platform::error("The game connection worker panicked during shutdown");
        }
    }
}

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

async fn run(
    request: tungstenite::handshake::client::Request,
    first_message: String,
    core: &mut ConnectionCore,
    mut commands: mpsc::Receiver<Command>,
    mut stop: watch::Receiver<bool>,
) {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));
    let timeout = core.next_deadline().map_or(Duration::from_secs(15), until);
    let connected = tokio::select! {
        _ = stop.changed() => return,
        result = tokio::time::timeout(timeout, tokio_tungstenite::connect_async_with_config(request, Some(config), true)) => result,
    };
    let mut socket = match connected {
        Ok(Ok((socket, _))) => socket,
        Ok(Err(error)) => {
            core.fail(format!("Could not connect to the game host: {error}"));
            return;
        }
        Err(_) => {
            core.fail("The game server handshake timed out.".into());
            return;
        }
    };
    // Admission belongs to the socket worker, so a slow frame cannot delay it
    // until the independently running handshake deadline has expired.
    let result = async {
        tokio::select! {
            _ = stop.changed() => return Ok(()),
            result = socket.send(Message::Text(first_message.into())) =>
                result.map_err(|error| format!("Could not send game admission: {error}"))?,
        }
        core.opened();
        run_connected(&mut socket, core, &mut commands, &mut stop).await
    }
    .await;
    let close = result.as_ref().err().map(|_| CloseFrame {
        code: CloseCode::Library(4003),
        reason: "invalid host delivery".into(),
    });
    if let Err(error) = result {
        core.fail(error);
    }
    core.delivery.lock().unwrap().open = false;
    // Closing is best-effort and bounded, including an unresponsive remote peer.
    let _ = tokio::time::timeout(Duration::from_millis(100), socket.close(close)).await;
}

async fn run_connected(
    socket: &mut Socket,
    core: &mut ConnectionCore,
    commands: &mut mpsc::Receiver<Command>,
    stop: &mut watch::Receiver<bool>,
) -> Result<(), String> {
    loop {
        let deadline = core.next_deadline();
        let outbound = tokio::select! {
            _ = stop.changed() => return Ok(()),
            _ = wait_until(deadline) => core.tick(crate::platform::monotonic_ms())?.map(|message| encode(&message)).transpose()?,
            command = commands.recv() => match command {
                Some(Command::Send(mut pending)) => {
                    // Keep the byte credit until the actual send completes or fails.
                    let message = Message::Text(std::mem::take(&mut pending.text).into());
                    tokio::select! {
                        _ = stop.changed() => return Ok(()),
                        result = socket.send(message) => result.map_err(|error| format!("Could not send the game command: {error}"))?,
                    }
                    None
                }
                Some(Command::AcceptWelcome) => core.accept_welcome().map(|message| encode(&message)).transpose()?,
                Some(Command::StartPing(observer)) => Some(encode(&core.start_ping(crate::platform::monotonic_ms(), observer))?),
                None => return Ok(()),
            },
            message = socket.next() => {
                // Receipt time precedes JSON decode and main-thread presentation.
                let now = crate::platform::monotonic_ms();
                match message {
                    Some(Ok(Message::Text(text))) => core.receive(&text, now)?.map(|message| encode(&message)).transpose()?,
                    Some(Ok(Message::Close(frame))) => {
                        let (code, reason) = frame.map_or((1005, String::new()), |frame| (u16::from(frame.code), frame.reason.to_string()));
                        core.closed(code, reason);
                        return Ok(());
                    }
                    Some(Ok(Message::Ping(_))) => {
                        tokio::select! {
                            _ = stop.changed() => return Ok(()),
                            result = socket.flush() => result.map_err(|error| format!("Could not answer the game connection: {error}"))?,
                        }
                        None
                    }
                    Some(Ok(Message::Pong(_))) => None,
                    Some(Ok(_)) => return Err("The game host sent a non-text message".into()),
                    Some(Err(error)) => return Err(format!("The game connection was interrupted: {error}")),
                    None => { core.closed(1006, "The game connection ended".into()); return Ok(()); }
                }
            },
        };
        if let Some(text) = outbound {
            tokio::select! {
                _ = stop.changed() => return Ok(()),
                result = socket.send(Message::Text(text.into())) => result.map_err(|error| format!("Could not send the game command: {error}"))?,
            }
        }
    }
}

fn until(at: f64) -> Duration {
    Duration::from_secs_f64((at - crate::platform::monotonic_ms()).max(0.0) / 1000.0)
}

async fn wait_until(deadline: Option<f64>) {
    match deadline {
        Some(at) => tokio::time::sleep(until(at)).await,
        None => std::future::pending().await,
    }
}

pub fn read_resume() -> Result<Option<ResumeCredentials>, String> {
    crate::storage::with_preferences(|preferences| {
        preferences
            .get(RESUME_KEY)?
            .map(|text| {
                serde_json::from_str(&text).map_err(|error| {
                    format!("Could not read game reconnection credentials: {error}")
                })
            })
            .transpose()
    })
}

pub fn remember_resume(credentials: &ResumeCredentials) -> Result<(), String> {
    let text = serde_json::to_string(credentials)
        .map_err(|error| format!("Could not encode game reconnection credentials: {error}"))?;
    crate::storage::with_preferences(|preferences| preferences.set(RESUME_KEY, &text))
}

pub fn forget_rejected_resume(rejected: &ResumeCredentials) -> Result<(), String> {
    crate::storage::with_preferences(|preferences| {
        if let Some(text) = preferences.get(RESUME_KEY)? {
            let stored: ResumeCredentials = serde_json::from_str(&text)
                .map_err(|error| format!("Could not read saved game identity: {error}"))?;
            if &stored == rejected {
                preferences.remove(RESUME_KEY)?;
            }
        }
        Ok(())
    })
}
