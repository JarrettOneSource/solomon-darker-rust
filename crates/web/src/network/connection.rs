//! Ordered transfer/decoder state stays with the socket. Only completed deliveries
//! cross to the renderer, so decoding never holds its inbox lock.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::{
    ClientMessage, GameCheckpoint, MAX_FRAME_BYTES, MAX_RELIABLE_EVENTS, NetworkEvent, NetworkPoll,
    ReceivedNativeSnapshot, ServerMessage, transfer,
};

#[derive(Default)]
pub(super) struct Delivery {
    pub poll: NetworkPoll,
    pub open: bool,
    pub ping_ms: Option<f64>,
}

pub(super) type SharedDelivery = Arc<Mutex<Delivery>>;

pub(super) struct ConnectionCore {
    pub delivery: SharedDelivery,
    failed: bool,
    receiver: transfer::MessageReceiver,
    snapshots: solomon_protocol::snapshot::SnapshotReader,
    welcome_deadline: Option<f64>,
    next_ping: Option<(f64, bool)>,
    next_nonce: u64,
    pending_pings: BTreeMap<u64, f64>,
}

impl ConnectionCore {
    pub fn new(delivery: SharedDelivery, welcome: bool, now: f64) -> Self {
        Self {
            delivery,
            failed: false,
            receiver: transfer::MessageReceiver::default(),
            snapshots: solomon_protocol::snapshot::SnapshotReader::default(),
            welcome_deadline: welcome.then(|| welcome_expires(now)),
            next_ping: None,
            next_nonce: 1,
            pending_pings: BTreeMap::new(),
        }
    }

    pub fn opened(&mut self) {
        let mut delivery = self.delivery.lock().unwrap();
        delivery.open = true;
        delivery.poll.events.push(NetworkEvent::Opened);
    }

    pub fn closed(&mut self, code: u16, reason: String) {
        self.clear();
        let mut delivery = self.delivery.lock().unwrap();
        delivery.open = false;
        delivery.poll.events.push(NetworkEvent::Closed { code, reason });
    }

    pub fn fail(&mut self, reason: String) {
        if self.failed {
            return;
        }
        self.failed = true;
        self.clear();
        let mut delivery = self.delivery.lock().unwrap();
        delivery.open = false;
        delivery.poll.snapshot = None;
        delivery.poll.events.push(NetworkEvent::Failed(reason));
    }

    fn clear(&mut self) {
        self.receiver.close();
        self.snapshots.clear();
        self.welcome_deadline = None;
        self.next_ping = None;
        self.pending_pings.clear();
    }

    pub fn receive(&mut self, text: &str, now: f64) -> Result<Option<ClientMessage>, String> {
        if self.failed {
            return Ok(None);
        }
        if text.len() > MAX_FRAME_BYTES {
            return Err("The game host sent a snapshot beyond the save limit".into());
        }
        let message = self
            .snapshots
            .decode::<Box<GameCheckpoint>>(text)
            .map_err(|error| format!("The game host sent an invalid message: {error}"))?;
        let reception = self.receiver.receive(message)?;
        if matches!(reception.acknowledgement, Some(ClientMessage::WelcomeChunkAck { .. })) {
            self.welcome_deadline = Some(welcome_expires(now));
        }
        if let Some(message) = reception.message {
            let mut delivery = self.delivery.lock().unwrap();
            match message {
                ServerMessage::Pong { nonce } => {
                    if let Some(sent) = self.pending_pings.remove(&nonce) {
                        delivery.ping_ms = Some((now - sent).round().max(0.0));
                        delivery.poll.ping_changed = true;
                    }
                }
                ServerMessage::Snapshot { frame } => {
                    delivery.poll.snapshot = Some(ReceivedNativeSnapshot {
                        frame: Box::new(frame),
                        received_at_ms: now,
                    });
                }
                message => {
                    if delivery.poll.events.len() >= MAX_RELIABLE_EVENTS {
                        return Err("The game connection fell too far behind".into());
                    }
                    delivery.poll.events.push(NetworkEvent::Message {
                        message: Box::new(message),
                        received_at_ms: now,
                    });
                }
            }
        }
        Ok(reception.acknowledgement)
    }

    pub fn accept_welcome(&mut self) -> Option<ClientMessage> {
        self.welcome_deadline = None;
        self.receiver
            .take_welcome_acknowledgement()
            .map(|next_offset| ClientMessage::WelcomeChunkAck { next_offset })
    }

    pub fn start_ping(&mut self, now: f64, observer: bool) -> ClientMessage {
        self.next_ping = Some((now + 2_000.0, observer));
        self.ping(now, observer)
    }

    fn ping(&mut self, now: f64, observer: bool) -> ClientMessage {
        if !observer {
            self.pending_pings.retain(|_, sent| now - *sent < 10_000.0);
        }
        let nonce = self.next_nonce;
        self.next_nonce = if nonce == 0x7fff_ffff { 1 } else { nonce + 1 };
        self.pending_pings.insert(nonce, now);
        ClientMessage::Ping { nonce }
    }

    pub fn next_deadline(&self) -> Option<f64> {
        self.welcome_deadline.into_iter().chain(self.next_ping.map(|(at, _)| at)).reduce(f64::min)
    }

    pub fn tick(&mut self, now: f64) -> Result<Option<ClientMessage>, String> {
        if self.welcome_deadline.is_some_and(|at| now >= at) {
            return Err("The game server handshake timed out.".into());
        }
        if let Some((at, observer)) = self.next_ping
            && now >= at
        {
            return Ok(Some(self.start_ping(now, observer)));
        }
        Ok(None)
    }
}

fn welcome_expires(now: f64) -> f64 {
    now + solomon_protocol::transfer::GAME_WELCOME_PROGRESS_TIMEOUT_MS as f64
}

pub(super) fn encode(message: &ClientMessage) -> Result<String, String> {
    serde_json::to_string(message)
        .map_err(|error| format!("Could not encode the game command: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_progress_extends_the_deadline_until_game_acceptance() {
        let delivery = Arc::new(Mutex::new(Delivery::default()));
        let mut core = ConnectionCore::new(delivery, true, 100.0);
        assert_eq!(core.next_deadline(), Some(5_100.0));
        let chunk: ServerMessage<Box<GameCheckpoint>> =
            ServerMessage::WelcomeChunk { data: vec![123], offset: 0, total_length: 20 };
        assert!(matches!(
            core.receive(&serde_json::to_string(&chunk).unwrap(), 4_000.0).unwrap(),
            Some(ClientMessage::WelcomeChunkAck { next_offset: 1 })
        ));
        assert_eq!(core.next_deadline(), Some(9_000.0));
        assert!(core.tick(8_999.0).is_ok());
        assert!(core.tick(9_000.0).unwrap_err().contains("timed out"));
        core.accept_welcome();
        assert!(core.next_deadline().is_none());
    }

    #[test]
    fn a_stalled_renderer_has_a_bounded_reliable_queue_and_one_failure() {
        let delivery = Arc::new(Mutex::new(Delivery::default()));
        let mut core = ConnectionCore::new(delivery.clone(), false, 0.0);
        let message: ServerMessage<Box<GameCheckpoint>> = ServerMessage::SessionStatus {
            version: solomon_protocol::PROTOCOL_VERSION,
            can_resume: false,
            selection: None,
            fresh_join_available: true,
        };
        let text = serde_json::to_string(&message).unwrap();
        for _ in 0..MAX_RELIABLE_EVENTS {
            core.receive(&text, 1.0).unwrap();
        }
        let error = core.receive(&text, 1.0).unwrap_err();
        assert!(error.contains("fell too far behind"));
        core.fail(error);
        core.fail("second failure".into());
        core.receive(&text, 1.0).unwrap();
        let delivery = delivery.lock().unwrap();
        assert_eq!(delivery.poll.events.len(), MAX_RELIABLE_EVENTS + 1);
        assert!(matches!(delivery.poll.events.last(), Some(NetworkEvent::Failed(_))));
        assert!(!delivery.open);
    }
}
