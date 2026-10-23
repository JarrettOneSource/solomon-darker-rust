//! The Website supervisor contract over Rust College authorities. One thread owns the
//! Colleges, tickets and social broker; transport workers own bytes and heartbeat only.
//! Reference: `host/game-session-supervisor.ts` and `game-host.ts`.

pub(in crate::host) mod http;
mod preparation;
mod recovery;
mod shutdown;
mod social;
mod socket;
mod standalone;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, TryRecvError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use solomon_modding::package::MaterializedWebSessionContent;
use solomon_protocol::hosted::GAME_HUB_PATH;
use solomon_sim::modding::identity::{GameContentManifest, WebSessionContentSummary};
use solomon_sim::social::{GameSessionKind, broker::GameSocialBroker};

use super::authority::close_peer;
use super::hosted::{HostedAdmission, HostedGameHost};
use crate::transport::{HostedHttpResponse, NativeServer, PeerId, TransportEvent};

const MAXIMUM_EVENTS_PER_POLL: usize = 64;
const MAXIMUM_CATCH_UP_TICKS: u32 = 25;
const JOIN_INTENT_TIMEOUT_MS: f64 = 600_000.0;

pub struct HostedSupervisorConfig {
    pub asset_directory: PathBuf,
    pub admin_secret: String,
    pub revision: String,
    pub maximum_players: usize,
    pub maximum_sessions: usize,
    pub unclaimed_timeout: Duration,
    pub deployment_save_timeout: Duration,
    pub ml_bot_policy: Option<Arc<solomon_sim::ml_bot::MlBotPolicyRuntime>>,
    pub memorial_path: Option<PathBuf>,
    pub runtime_events: Option<super::runtime_events::RuntimeEventConfig>,
}

impl HostedSupervisorConfig {
    pub fn new(asset_directory: PathBuf, admin_secret: String, revision: String) -> Self {
        Self {
            asset_directory,
            admin_secret,
            revision,
            maximum_players: 16,
            maximum_sessions: 64,
            unclaimed_timeout: Duration::from_secs(120),
            deployment_save_timeout: Duration::from_secs(30),
            ml_bot_policy: None,
            memorial_path: None,
            runtime_events: None,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.admin_secret.len() < 32 {
            return Err(
                "The game session supervisor requires an admin secret of at least 32 bytes".into(),
            );
        }
        if self.revision.len() != 40
            || !self
                .revision
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("The game session supervisor revision must be a full Git commit ID".into());
        }
        if self.maximum_players == 0
            || self.maximum_sessions == 0
            || self.unclaimed_timeout.is_zero()
            || self.deployment_save_timeout.is_zero()
        {
            return Err(
                "The game session supervisor capacities and deadlines must be positive".into()
            );
        }
        Ok(())
    }
}

struct HostTicket {
    admission: HostedAdmission,
    expires_at_ms: f64,
}
struct CollegeRecord {
    host: HostedGameHost,
    path: String,
    tickets: BTreeMap<String, HostTicket>,
    created_at_ms: f64,
    claimed: bool,
}

#[derive(Clone)]
enum PartyLocator {
    Code(String),
    Public(String),
    Request(String),
}
struct JoinIntent {
    expires_at_ms: f64,
    locator: PartyLocator,
    party_id: String,
    request_token: Option<String>,
    session_id: String,
}
struct SupervisorJoinRequest {
    expires_at_ms: f64,
    intent_id: Option<String>,
    listing_id: String,
    session_id: String,
}
struct DeploymentRestart {
    target_revision: String,
    deployment_id: String,
    deadline_ms: f64,
    completed: bool,
    closing: bool,
    checkpoints: BTreeMap<PeerId, u64>,
    pending: BTreeSet<PeerId>,
    acknowledged: BTreeSet<PeerId>,
    respond: SyncSender<HostedHttpResponse>,
}

pub struct HostedSupervisor {
    config: HostedSupervisorConfig,
    standalone_credential: Option<String>,
    // Vec retains the source Map's creation order, including publication/broker order.
    colleges: Vec<CollegeRecord>,
    broker: GameSocialBroker,
    join_intents: BTreeMap<String, JoinIntent>,
    join_requests: BTreeMap<String, SupervisorJoinRequest>,
    deployment: Option<DeploymentRestart>,
    retired_recoveries: BTreeSet<String>,
    shared_host_tick: u64,
    preparations: Vec<preparation::AdmissionPreparation>,
    recovery_sessions: BTreeMap<String, recovery::PartyRecoverySession>,
    pending_rejoins: Vec<recovery::PendingPartyRejoin>,
    runtime_events: Option<super::runtime_events::RuntimeEventPublisher>,
    run_archives: Option<super::run_archive::RunArchiveStore>,
}

impl HostedSupervisor {
    pub fn new(config: HostedSupervisorConfig) -> Result<Self, String> {
        Self::create(config, None)
    }

    fn create(
        mut config: HostedSupervisorConfig,
        standalone_credential: Option<String>,
    ) -> Result<Self, String> {
        config.validate()?;
        let runtime_events = config
            .runtime_events
            .take()
            .map(super::runtime_events::RuntimeEventPublisher::new)
            .transpose()?;
        let run_archives = config
            .memorial_path
            .as_ref()
            .map(|path| {
                let directory = path
                    .parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .unwrap_or_else(|| std::path::Path::new("."));
                super::run_archive::RunArchiveStore::new(
                    directory.join("run-archives"),
                    runtime_events.as_ref().map(|publisher| publisher.sink()),
                )
            })
            .transpose()?;
        let content = Arc::new(stock_content("0".repeat(64)));
        let standalone = standalone_credential.is_some();
        let mut hub = HostedGameHost::new(
            if standalone { "standalone".into() } else { "shared-hub".into() },
            if standalone { GameSessionKind::Standalone } else { GameSessionKind::GlobalHub },
            config.maximum_players,
            content,
            &config.asset_directory,
            config.admin_secret.clone(),
            config.revision.clone(),
        )?;
        if let Some(policy) = &config.ml_bot_policy {
            hub.install_ml_bot_policy(Arc::clone(policy))?;
        }
        hub.runtime_events = runtime_events.as_ref().map(|publisher| publisher.sink());
        if let Some(archives) = &run_archives {
            hub.install_run_archives(archives.sink(), 0.0);
        }
        if let Some(path) = &config.memorial_path {
            let (file, initial) = super::memorial::MemorialPersistence::open(path.clone())?;
            let super::hosted::HostedWorlds::Global(worlds) = &mut hub.worlds else {
                unreachable!("the shared Hub owns Memorial persistence");
            };
            worlds.hub.hub_memorial = initial;
            hub.memorial_persistence = Some(file);
        }
        Ok(Self {
            config,
            standalone_credential,
            colleges: vec![CollegeRecord {
                host: hub,
                path: if standalone {
                    standalone::DESKTOP_GAME_PATH.into()
                } else {
                    GAME_HUB_PATH.into()
                },
                tickets: BTreeMap::new(),
                created_at_ms: 0.0,
                claimed: !standalone,
            }],
            broker: GameSocialBroker::default(),
            join_intents: BTreeMap::new(),
            join_requests: BTreeMap::new(),
            deployment: None,
            retired_recoveries: BTreeSet::new(),
            shared_host_tick: 0,
            preparations: Vec::new(),
            recovery_sessions: BTreeMap::new(),
            pending_rejoins: Vec::new(),
            runtime_events,
            run_archives,
        })
    }

    fn emit_runtime_event<T: serde::Serialize>(&self, event: &str, message: &str, details: T) {
        let Some(publisher) = &self.runtime_events else { return };
        publisher.sink().publish(super::runtime_events::RuntimeEventEntry {
            component: "session-supervisor".into(),
            event: event.into(),
            message: message.into(),
            details: serde_json::from_value(
                serde_json::to_value(details).expect("accepted supervisor event values serialize"),
            )
            .expect("supervisor event details are a finite object"),
            occurred_at_utc: Some(
                super::source_utc_timestamp(time::OffsetDateTime::now_utc())
                    .expect("current Source UTC time is representable"),
            ),
        });
    }

    pub fn run(self, server: NativeServer, stop: Arc<AtomicBool>) -> Result<(), String> {
        self.run_when_ready(server, stop, |_| {})
    }

    /// Readiness is published only after the actual socket route is registered.
    /// Desktop's parent can connect as soon as it receives its JSON-ready line.
    pub fn run_when_ready(
        mut self,
        mut server: NativeServer,
        stop: Arc<AtomicBool>,
        ready: impl FnOnce(std::net::SocketAddr),
    ) -> Result<(), String> {
        if !server.local_addr().ip().is_loopback() {
            return Err(
                "The game session supervisor must bind to loopback behind the TLS gateway".into()
            );
        }
        server
            .register_hosted_route(self.colleges[0].path.clone(), self.config.maximum_players)
            .map_err(|error| error.to_string())?;
        ready(server.local_addr());
        let start = Instant::now();
        let interval = Duration::from_millis(10);
        let mut next_tick = start + interval;
        let result = (|| {
            while !stop.load(Ordering::Acquire) {
                let now_ms = start.elapsed().as_secs_f64() * 1_000.0;
                for _ in 0..MAXIMUM_EVENTS_PER_POLL {
                    match server.try_event() {
                        Ok(event) => self.event(event, &server, now_ms)?,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return Ok(()),
                    }
                }
                self.complete_admissions(&server, now_ms)?;
                self.complete_party_rejoins(now_ms)?;
                let mut caught_up = 0;
                while Instant::now() >= next_tick && caught_up < MAXIMUM_CATCH_UP_TICKS {
                    for college in &mut self.colleges {
                        college.host.set_archive_deadline(next_tick);
                    }
                    self.tick(now_ms)?;
                    next_tick += interval;
                    caught_up += 1;
                }
                if caught_up == MAXIMUM_CATCH_UP_TICKS && Instant::now() >= next_tick {
                    // Drop excess wall time only. No world tick or RNG draw is invented.
                    next_tick = Instant::now() + interval;
                }
                self.prune(&server, now_ms)?;
                self.finish_deployment(&server, now_ms)?;
                let timeout = next_tick.saturating_duration_since(Instant::now());
                match server.recv_timeout(timeout) {
                    Ok(event) => {
                        self.event(event, &server, start.elapsed().as_secs_f64() * 1_000.0)?
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                }
            }
            Ok(())
        })();
        let closed = self.shutdown_authorities(&server, start, result.is_err());
        self.broker.close();
        server.shutdown();
        for preparation in self.preparations.drain(..) {
            // Each temporary definition VM belongs to its worker and closes there.
            // Shutdown joins these bounded executions rather than detaching Lua work.
            let _ = preparation.worker.join();
        }
        if let Some(archives) = &mut self.run_archives {
            archives.close();
        }
        if let Some(events) = &mut self.runtime_events {
            events.close();
        }
        result.and(closed)
    }

    fn event(
        &mut self,
        event: TransportEvent,
        server: &NativeServer,
        now_ms: f64,
    ) -> Result<(), String> {
        match event {
            TransportEvent::Http { request, respond } => {
                // The desktop socket has no provisioning/admin control plane.
                if self.standalone_credential.is_some() {
                    let _ = respond.send(HostedHttpResponse::empty(404));
                    return Ok(());
                }
                if let Some(response) = self.http(request, server, now_ms, respond.clone())? {
                    let _ = respond.send(response);
                }
            }
            TransportEvent::Connected { peer, path, .. } => {
                if self.deployment.is_some() {
                    return close_peer(&peer, 1012, "game updating");
                }
                if let Some(college) = self.colleges.iter_mut().find(|college| college.path == path)
                {
                    college
                        .host
                        .connections
                        .insert(peer.id(), super::connection::HostConnection::new(peer, now_ms));
                } else {
                    close_peer(&peer, 1008, "game session unavailable")?;
                }
            }
            TransportEvent::Message { peer_id, payload } => {
                self.message(peer_id, &payload, now_ms)?
            }
            TransportEvent::Disconnected { peer_id, close } => {
                self.disconnected(peer_id, close, now_ms)?;
            }
            TransportEvent::Failed { reason } => return Err(reason),
        }
        self.reset_standalone_if_empty(now_ms)?;
        self.deliver_broker()?;
        Ok(())
    }

    fn tick(&mut self, now_ms: f64) -> Result<(), String> {
        if self.deployment.is_some()
            || (self.standalone_credential.is_some() && self.colleges[0].host.players.is_empty())
        {
            return Ok(());
        }
        self.shared_host_tick += 1;
        for college in &mut self.colleges {
            let ticks = match college.host.step(now_ms) {
                Ok(ticks) => ticks,
                Err(reason) => {
                    college
                        .host
                        .close_run_archives(super::run_archive::RunArchiveEndReason::ServerError);
                    return Err(reason);
                }
            };
            college.host.prune_rejoins(now_ms);
            college.host.synchronize_rejoins()?;
            college.host.publish_runtime_activity();
            college.host.publish_leaderboard_receipts()?;
            college.host.publish_tick_audio(&ticks)?;
            college.host.publish_pending_saves(self.shared_host_tick.is_multiple_of(3_000))?;
            if self.shared_host_tick.is_multiple_of(5) || college.host.publish_dirty {
                college.host.publish_snapshot(now_ms)?;
            }
            if college.host.social_dirty {
                college.host.publish_social()?;
                college.host.social_dirty = false;
            }
            for (id, terminal) in college.host.retired_recoveries.drain(..) {
                self.recovery_sessions.remove(&id);
                if terminal {
                    self.retired_recoveries.insert(id);
                } else {
                    self.retired_recoveries.remove(&id);
                }
            }
        }
        self.broker.prune(unix_ms()?);
        self.deliver_broker()
    }

    fn prune(&mut self, server: &NativeServer, now_ms: f64) -> Result<(), String> {
        if let Some(archives) = &mut self.run_archives {
            archives.prune_if_due();
        }
        self.join_intents.retain(|_, intent| intent.expires_at_ms > now_ms);
        self.join_requests.retain(|_, request| request.expires_at_ms > now_ms);
        for college in &mut self.colleges {
            college.tickets.retain(|_, ticket| ticket.expires_at_ms > now_ms);
            college.host.prune_party_access(now_ms);
            college.host.retain_run_archives(now_ms);
            for connection in college.host.connections.values() {
                if connection.player_id.is_none() && connection.hello_deadline_ms <= now_ms {
                    close_peer(&connection.peer, 1008, "game admission timed out")?;
                }
            }
        }
        let expired: Vec<_> = self
            .colleges
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, college)| {
                let sockets = server.hosted_route_connections(&college.path);
                (college.claimed && college.host.players.is_empty() && sockets == 0)
                    || (!college.claimed
                        && sockets == 0
                        && now_ms - college.created_at_ms
                            >= self.config.unclaimed_timeout.as_secs_f64() * 1_000.0)
            })
            .map(|(index, _)| index)
            .collect();
        for index in expired.into_iter().rev() {
            let reason =
                if self.colleges[index].claimed { "empty-after-use" } else { "unclaimed-timeout" };
            self.close_college(index, server, reason)?;
        }
        Ok(())
    }

    fn close_college(
        &mut self,
        index: usize,
        server: &NativeServer,
        reason: &str,
    ) -> Result<(), String> {
        let mut college = self.colleges.remove(index);
        college.host.close_run_archives(super::run_archive::RunArchiveEndReason::HostClosed);
        server.remove_hosted_route(&college.path);
        self.recovery_sessions.retain(|id, recovery| {
            if recovery.session_id == college.host.id {
                self.retired_recoveries.insert(id.clone());
                false
            } else {
                true
            }
        });
        for lineage in &college.host.recovery_lineages {
            self.retired_recoveries.insert(lineage.recovery_id.clone());
        }
        for player in &college.host.players {
            if let Some(connection) = &player.social_connection {
                self.broker.close_connection(connection);
            }
        }
        for connection in college.host.connections.values() {
            close_peer(&connection.peer, 1012, "game session ended")?;
        }
        self.emit_runtime_event("session.closed", "A browser game session closed.",
            serde_json::json!({"reason":reason, "sessionCount":self.colleges.len()-1, "sessionId":college.host.id}));
        Ok(())
    }

    fn finish_deployment(&mut self, server: &NativeServer, now_ms: f64) -> Result<(), String> {
        let Some(restart) = &mut self.deployment else { return Ok(()) };
        if restart.completed {
            return Ok(());
        }
        if !restart.closing {
            if !restart.pending.is_empty() && now_ms < restart.deadline_ms {
                return Ok(());
            }
            for (index, college) in self.colleges.iter_mut().enumerate() {
                for (peer_id, connection) in &mut college.host.connections {
                    // Hub observers are outside restartForDeployment's client set.
                    // Private College closure also owns its observers and pending sockets.
                    if index != 0 || restart.checkpoints.contains_key(peer_id) {
                        connection.planned_disconnect =
                            Some(("deployment-restart", "game updating".into()));
                        close_peer(&connection.peer, 1012, "game updating")?;
                    }
                }
            }
            restart.closing = true;
        }
        for (index, college) in self.colleges.iter().enumerate() {
            if college.host.connections.keys().any(|peer| restart.checkpoints.contains_key(peer)) {
                return Ok(());
            }
            if index != 0
                && (!college.host.connections.is_empty()
                    || server.hosted_route_connections(&college.path) != 0)
            {
                return Ok(());
            }
        }
        // Transport close callbacks, including the bounded forced-close path, have now
        // completed. Source replies only after all private authorities/routes are gone.
        for index in (1..self.colleges.len()).rev() {
            self.close_college(index, server, "deployment-restart")?;
        }
        let restart = self.deployment.as_mut().expect("a draining restart remains retained");
        let saved = restart.acknowledged.len();
        let players = restart.checkpoints.len();
        let response = http::json(
            200,
            serde_json::json!({"status":"ready","deploymentId":restart.deployment_id,"players":players,
            "savedPlayers":saved,"targetRevision":restart.target_revision,"unacknowledgedPlayers":players-saved}),
        );
        let _ = restart.respond.send(response);
        // Stay draining after replying. The service manager replaces this process.
        restart.completed = true;
        Ok(())
    }
}

fn stock_content(manifest_sha256: String) -> MaterializedWebSessionContent {
    MaterializedWebSessionContent {
        assets: Vec::new(),
        boneyards: Vec::new(),
        compiled_mods: Vec::new(),
        manifest: GameContentManifest {
            manifest_sha256: manifest_sha256.clone(),
            mods: Vec::new(),
        },
        mod_sources: Vec::new(),
        summary: WebSessionContentSummary { manifest_sha256, mods: Vec::new() },
    }
}

fn unix_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis() as u64)
        .map_err(|error| format!("Cannot read the host UTC clock: {error}"))
}
