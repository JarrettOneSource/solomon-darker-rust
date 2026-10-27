//! Game session commands and delivery shared by native and browser clients.
//! Presentation accepts a welcome or snapshot before acknowledging its state.

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
mod browser_resume;
mod connection;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod transfer;

#[cfg(target_arch = "wasm32")]
use browser::NativeConnection;
#[cfg(target_arch = "wasm32")]
pub use browser_resume::{forget_rejected_resume, read_resume, remember_resume};
#[cfg(not(target_arch = "wasm32"))]
use native::NativeConnection;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{forget_rejected_resume, read_resume, remember_resume};

use solomon_protocol::client_input::ClientInputSequence;
use solomon_protocol::{
    ClientMessage, GameCommand, HostSnapshot, ResumeCredentials, ServerMessage, SnapshotFrame,
};
use solomon_sim::session::{GameCheckpoint, GameSessionInput};

const RESUME_KEY: &str = "solomon-darker-rust-native-resume-v1";
const MAX_RELIABLE_EVENTS: usize = 256;
const MAX_FRAME_BYTES: usize = solomon_protocol::GAME_WEBSOCKET_MAX_PAYLOAD_BYTES;
const MAX_PENDING_SEND_BYTES: usize = 1024 * 1024;

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

pub struct NativeSession {
    connection: NativeConnection,
    input: ClientInputSequence,
    command_sequence: u64,
    pub credentials: Option<ResumeCredentials>,
    pub host: HostSnapshot,
    pub snapshot_sequence: u64,
}

impl NativeSession {
    pub fn connect(first_message: ClientMessage) -> Result<Self, String> {
        Self::connect_to(&local_endpoint()?, first_message)
    }

    pub fn connect_to(endpoint: &str, first_message: ClientMessage) -> Result<Self, String> {
        let connection = NativeConnection::connect_to(
            endpoint,
            &crate::platform::network_origin()?,
            first_message,
        )?;
        Ok(Self {
            connection,
            input: ClientInputSequence::default(),
            command_sequence: 0,
            credentials: None,
            host: HostSnapshot::default(),
            snapshot_sequence: 0,
        })
    }

    pub fn poll(&self) -> NetworkPoll {
        self.connection.poll()
    }

    pub fn send(&self, message: &ClientMessage) -> Result<(), String> {
        self.connection.send(message)
    }

    pub fn accept_welcome(&self) -> Result<(), String> {
        self.connection.accept_welcome()
    }

    pub fn is_open(&self) -> bool {
        self.connection.is_open()
    }

    pub fn start_ping(&mut self, observer: bool) -> Result<(), String> {
        self.connection.start_ping(observer)
    }

    #[must_use]
    pub fn ping_ms(&self) -> Option<f64> {
        self.connection.ping_ms()
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

/// The launcher marks a local-authority URL; static previews retain their local owner.
pub fn native_host_requested() -> Result<bool, String> {
    Ok(crate::platform::location()?
        .query_pairs()
        .find(|(key, _)| key == "host")
        .is_some_and(|(_, value)| value == "1"))
}

fn local_endpoint() -> Result<String, String> {
    let mut url = crate::platform::location()?;
    let scheme = match url.scheme() {
        "https" => "wss",
        "http" => "ws",
        _ => return Err("The game must be opened through its launcher or web address".into()),
    };
    url.set_scheme(scheme).map_err(|()| "Could not select the game connection scheme")?;
    let mount = crate::platform::website_mount()?.unwrap_or_default();
    url.set_path(&mount.path(solomon_protocol::WEBSOCKET_PATH));
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.into())
}
