//! One transport generation's host cursors. Input and snapshot ACKs are monotonic within
//! this generation; stable participant identities and resume tokens live above it.

use solomon_protocol::flame_presentation::FlamePresentationBaseline;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use solomon_protocol::HubActivity;
use solomon_sim::audio::combat::CombatAudioCursor;
use solomon_sim::social::GameChatRateLimiter;

use super::audio::ListenerView;

use super::input::InputQueue;
use crate::transport::PeerSender;

// Source `host/game-host.ts:73,7674`: suspend at eight pending frames and recover
// at two. These render ACKs are independent of the socket's ordered compaction baseline.
const SNAPSHOT_HIGH_WATER_MARK: usize = 8;
const SNAPSHOT_LOW_WATER_MARK: usize = 2;

pub struct HostConnection {
    pub peer: PeerSender,
    pub player_id: Option<String>,
    pub cheats_enabled: bool,
    pub hello_deadline_ms: f64,
    pub connected_at_utc_ms: Option<f64>,
    pub planned_disconnect: Option<(&'static str, String)>,
    pub initial_save_pending: bool,
    pub input: InputQueue,
    pub last_command_sequence: u64,
    pub last_command_receipt: Option<Arc<str>>,
    /// Source shares this monotonic lane between authored actions and casts.
    /// Console request IDs remain independent.
    pub last_mod_request_id: u64,
    pub pending_lua_request_ids: std::collections::BTreeSet<u64>,
    pub listener_view: Option<ListenerView>,
    pub combat_audio: Option<CombatAudioCursor>,
    pub chat_limit: GameChatRateLimiter,
    pub hub_activity: Option<HubActivity>,
    pub welcome_revision: Option<u64>,
    boneyard_run_id: Option<String>,
    last_sent_snapshot: u64,
    acknowledged_snapshot: u64,
    pending_snapshots: VecDeque<u64>,
    sent_flame_baselines: BTreeMap<u64, FlamePresentationBaseline>,
    snapshot_backpressure: bool,
}

impl HostConnection {
    /// Source handles transfer ACKs before ordinary/player/observer dispatch. No
    /// command can interleave the ordered Welcome prefix except explicit disconnect.
    pub fn handle_transfer_message(
        &self,
        message: &solomon_protocol::ClientMessage,
    ) -> Result<bool, String> {
        use solomon_protocol::ClientMessage;
        match message {
            ClientMessage::WelcomeChunkAck { next_offset } => {
                self.peer.acknowledge_welcome(*next_offset)?;
                return Ok(true);
            }
            ClientMessage::Disconnect => return Ok(false),
            _ => {}
        }
        if self.peer.welcome_pending() {
            return Err("The welcome must be acknowledged before game commands.".into());
        }
        Ok(false)
    }

    #[must_use]
    pub fn new(peer: PeerSender, now_ms: f64) -> Self {
        Self {
            peer,
            player_id: None,
            cheats_enabled: false,
            hello_deadline_ms: now_ms + 5_000.0,
            connected_at_utc_ms: None,
            planned_disconnect: None,
            initial_save_pending: false,
            input: InputQueue::default(),
            last_command_sequence: 0,
            last_command_receipt: None,
            last_mod_request_id: 0,
            pending_lua_request_ids: std::collections::BTreeSet::new(),
            listener_view: None,
            combat_audio: None,
            chat_limit: GameChatRateLimiter::default(),
            hub_activity: None,
            welcome_revision: None,
            boneyard_run_id: None,
            last_sent_snapshot: 0,
            acknowledged_snapshot: 0,
            pending_snapshots: VecDeque::new(),
            sent_flame_baselines: BTreeMap::new(),
            snapshot_backpressure: false,
        }
    }

    pub fn welcomed(&mut self, player_id: String, sequence: u64) {
        self.initial_save_pending = true;
        self.player_id = Some(player_id);
        self.welcome_revision = None;
        // The source welcome initializes its acknowledged baseline to this full frame.
        self.last_sent_snapshot = sequence;
        self.acknowledged_snapshot = sequence;
        self.pending_snapshots.clear();
        self.sent_flame_baselines.clear();
        self.snapshot_backpressure = false;
    }

    /// `ensureBoneyardLoaded`: one reliable geometry publication per actual run.
    /// Snapshot frequency never causes another scene clone/serialization.
    pub fn ensure_boneyard_loaded(
        &mut self,
        loaded: Option<&solomon_sim::boneyard::LoadedBoneyard>,
    ) -> Result<(), String> {
        if loaded.map(|loaded| loaded.run_id.as_str()) == self.boneyard_run_id.as_deref() {
            return Ok(());
        }
        if let Some(loaded) = loaded {
            super::authority::send_control(
                &self.peer,
                solomon_protocol::ServerMessage::BoneyardLoaded {
                    boneyard: Box::new(loaded.clone()),
                },
            )?;
        }
        self.boneyard_run_id = loaded.map(|loaded| loaded.run_id.clone());
        Ok(())
    }

    #[must_use]
    pub fn can_publish_snapshot(&self) -> bool {
        self.player_id.is_some()
            && !self.peer.is_closed()
            && !self.peer.welcome_pending()
            && self.welcome_revision.is_none()
            && !self.snapshot_backpressure
    }

    pub fn published_snapshot(&mut self, sequence: u64) {
        self.last_sent_snapshot = sequence;
        self.pending_snapshots.push_back(sequence);
        if self.pending_snapshots.len() >= SNAPSHOT_HIGH_WATER_MARK {
            self.snapshot_backpressure = true;
        }
    }

    pub fn acknowledged_flame_baseline(&self) -> Option<FlamePresentationBaseline> {
        self.sent_flame_baselines.get(&self.acknowledged_snapshot).copied()
    }

    pub fn record_flame_baseline(&mut self, sequence: u64, baseline: FlamePresentationBaseline) {
        self.sent_flame_baselines.insert(sequence, baseline);
        self.sent_flame_baselines.retain(|sent, _| *sent >= self.acknowledged_snapshot);
        // Same retained sent-baseline window as Source. Native high water limits
        // ordinarily keep this much smaller, but ACK never creates a second cursor.
        while self.sent_flame_baselines.len() > 64 {
            let oldest = self
                .sent_flame_baselines
                .keys()
                .copied()
                .find(|sent| *sent != self.acknowledged_snapshot)
                .unwrap();
            self.sent_flame_baselines.remove(&oldest);
        }
    }

    pub fn retained_flame_baselines(&self) -> impl Iterator<Item = FlamePresentationBaseline> + '_ {
        self.sent_flame_baselines.values().copied()
    }

    pub fn acknowledge_snapshot(&mut self, sequence: u64) -> Result<(), &'static str> {
        if sequence <= self.acknowledged_snapshot {
            return Ok(());
        }
        if sequence > self.last_sent_snapshot {
            return Err("Snapshot acknowledgement is ahead of the server.");
        }
        if !self.pending_snapshots.contains(&sequence) {
            return Err("Snapshot acknowledgement was not sent to this connection.");
        }
        self.acknowledged_snapshot = sequence;
        self.sent_flame_baselines.retain(|sent, _| *sent >= sequence);
        while self.pending_snapshots.front().is_some_and(|pending| *pending <= sequence) {
            self.pending_snapshots.pop_front();
        }
        if self.pending_snapshots.len() <= SNAPSHOT_LOW_WATER_MARK {
            self.snapshot_backpressure = false;
        }
        Ok(())
    }
}
