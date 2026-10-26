//! Observable source supervisor behavior over real HTTP and WebSocket connections.
//! The fixture was recorded from the unchanged Website public supervisor entry point.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use solomon_launcher::host::supervisor::{HostedSupervisor, HostedSupervisorConfig};
use solomon_launcher::transport::{NativeServer, TransportConfig};
use solomon_protocol::hosted::{GameSaveIntent, HostedCommand, HostedHello, HostedServerMessage};
use solomon_protocol::{ClientMessage, GameCommand, PROTOCOL_VERSION, ServerMessage};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::social::{
    GameChatChannel, GameOnlinePreferences, GameSessionKind, PlayerSocialProfile, SocialAction,
};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Error, Message, WebSocket};

const SECRET: &str = "runtime-parity-public-entry-test-secret";
const ORIGIN: &str = "https://rust-parity.invalid";

#[path = "hosted_supervisor/deployment.rs"]
mod deployment_tests;
#[path = "hosted_supervisor/desktop.rs"]
mod desktop_tests;
#[path = "hosted_supervisor/editor_test.rs"]
mod editor_test;
#[path = "hosted_supervisor/run_archives.rs"]
mod run_archive_tests;
#[path = "hosted_supervisor/runtime_events.rs"]
mod runtime_event_tests;

struct Host {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<(), String>>>,
}
impl Host {
    fn start() -> Self {
        Self::start_with_revision(&"a".repeat(40))
    }
    fn start_with_revision(revision: &str) -> Self {
        Self::start_with_policy(revision, false)
    }
    fn start_with_policy(revision: &str, learned_bots: bool) -> Self {
        Self::start_with_events(revision, learned_bots, None)
    }
    fn start_with_events(
        revision: &str,
        learned_bots: bool,
        runtime_events: Option<solomon_launcher::host::runtime_events::RuntimeEventConfig>,
    ) -> Self {
        Self::start_with_storage(revision, learned_bots, runtime_events, None)
    }
    fn start_with_storage(
        revision: &str,
        learned_bots: bool,
        runtime_events: Option<solomon_launcher::host::runtime_events::RuntimeEventConfig>,
        memorial_path: Option<PathBuf>,
    ) -> Self {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web");
        let mut config =
            HostedSupervisorConfig::new(assets.clone(), SECRET.into(), revision.into());
        config.runtime_events = runtime_events;
        config.memorial_path = memorial_path;
        if learned_bots {
            let bytes =
                std::fs::read(assets.join("assets/native/ml-bot-policy-v7-selected.sdml")).unwrap();
            let checkpoint =
                solomon_assets::ml_bot_checkpoint::decode_ml_bot_policy_checkpoint(&bytes).unwrap();
            config.ml_bot_policy = Some(Arc::new(checkpoint.into_runtime().unwrap()));
        }
        let server = NativeServer::start_hosted(
            TcpListener::bind(("127.0.0.1", 0)).unwrap(),
            TransportConfig {
                allowed_origins: vec![ORIGIN.into()],
                max_peers: 128,
                ..TransportConfig::default()
            },
        )
        .unwrap();
        let address = server.local_addr();
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker = thread::Builder::new()
            .name("hosted-supervisor-journey".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                // Native Lua VMs belong to the actual authority thread. Only plain
                // configuration and the transport cross this test's thread boundary.
                let authority = HostedSupervisor::new(config)?;
                let result = authority.run(server, signal);
                if let Err(reason) = &result {
                    eprintln!("The actual hosted authority stopped: {reason}");
                }
                result
            })
            .unwrap();
        Self { address, stop, worker: Some(worker) }
    }
    fn request(&self, method: &str, path: &str, body: Value, authorized: bool) -> (u16, Value) {
        request_at(self.address, method, path, body, authorized)
    }
    fn finish(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap().unwrap();
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn request_at(
    address: SocketAddr,
    method: &str,
    path: &str,
    body: Value,
    authorized: bool,
) -> (u16, Value) {
    let publication = (path == "/admin/deployments/restart").then_some("d1".repeat(16));
    request_at_with_deployment(
        address,
        method,
        path,
        body,
        authorized,
        publication.as_deref(),
        Some(1),
    )
}

fn request_at_with_deployment(
    address: SocketAddr,
    method: &str,
    path: &str,
    body: Value,
    authorized: bool,
    deployment_id: Option<&str>,
    deployment_sequence: Option<u64>,
) -> (u16, Value) {
    let body = if method == "GET" { String::new() } else { body.to_string() };
    let mut stream = TcpStream::connect(address).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
    let authorization =
        if authorized { format!("Authorization: Bearer {SECRET}\r\n") } else { String::new() };
    let publication =
        deployment_id.map(|id| format!("X-Solomon-Deployment-Id: {id}\r\n")).unwrap_or_default();
    let publication_sequence = deployment_sequence
        .map(|sequence| format!("X-Solomon-Deployment-Sequence: {sequence}\r\n"))
        .unwrap_or_default();
    write!(stream,"{method} {path} HTTP/1.1\r\nHost: {address}\r\n{authorization}{publication}{publication_sequence}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, serde_json::from_str(body).unwrap())
}

enum ClientWrite {
    Message { message: Box<ClientMessage>, completed: Sender<Result<(), String>> },
    Close { reason: Option<String>, completed: Sender<Result<(), String>> },
    Stop,
}

enum ClientObservation {
    Message(Box<ServerMessage<Value>>),
    SaveProgress { sequence: u64, received: usize, total: usize },
    Closed,
    Failed(String),
}

struct Client {
    writes: Sender<ClientWrite>,
    observations: Receiver<ClientObservation>,
    pending: VecDeque<ServerMessage<Value>>,
    worker: Option<JoinHandle<()>>,
    closed: bool,
    save_progress: Option<(u64, usize, usize)>,
}

// Source connect() keeps a live message listener on every socket. Waiting for one
// client's observation must not stop another client's ACKs, pongs or reliable input.
fn drive_client(
    mut socket: WebSocket<TcpStream>,
    writes: Receiver<ClientWrite>,
    observations: &Sender<ClientObservation>,
) -> Result<(), String> {
    use solomon_protocol::transfer::{GameStringChunk, GameStringReceiver};
    let mut snapshots = solomon_protocol::snapshot::SnapshotReader::default();
    let mut welcome = GameStringReceiver::new(
        solomon_protocol::GAME_WEBSOCKET_MAX_PAYLOAD_BYTES,
        "welcome".into(),
    );
    let mut save = GameStringReceiver::new(solomon_protocol::MAX_SAVE_BYTES, "save".into());
    let mut flame = solomon_protocol::flame_presentation::FlamePresentationReplay::new(
        solomon_protocol::flame_presentation::MAX_FLAME_PRESENTATION_BIRTHS,
        None,
    );
    let mut saving = None;
    let mut saved = 0;
    loop {
        for write in writes.try_iter() {
            match write {
                ClientWrite::Message { message, completed } => {
                    let result = serde_json::to_string(&message)
                        .map_err(|error| error.to_string())
                        .and_then(|text| {
                            socket
                                .send(Message::Text(text.into()))
                                .map_err(|error| error.to_string())
                        });
                    let _ = completed.send(result.clone());
                    result?;
                }
                ClientWrite::Close { reason, completed } => {
                    let frame = reason.map(|reason| tungstenite::protocol::CloseFrame {
                        code: tungstenite::protocol::frame::coding::CloseCode::Normal,
                        reason: reason.into(),
                    });
                    let result = socket.close(frame).map_err(|error| error.to_string());
                    let _ = completed.send(result.clone());
                    result?;
                }
                ClientWrite::Stop => {
                    let _ = socket.close(None);
                    return Ok(());
                }
            }
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                let mut message = snapshots.decode::<Value>(&text)?;
                // The Source browser detaches its session message listener before
                // closing. Drain in-flight observations without ACKing a closed writer.
                if let ServerMessage::WelcomeChunk { data, offset, total_length } = message {
                    let next_offset = offset + data.len();
                    let complete =
                        welcome.accept(&GameStringChunk { data, offset, total_length })?;
                    if let Some(complete) = complete {
                        let complete =
                            String::from_utf16(&complete).map_err(|error| error.to_string())?;
                        message =
                            serde_json::from_str(&complete).map_err(|error| error.to_string())?;
                        if !matches!(message, ServerMessage::Welcome { .. }) {
                            return Err("The welcome transfer contains another message".into());
                        }
                    } else {
                        if socket.can_write() {
                            socket
                                .send(Message::Text(
                                    serde_json::to_string(&ClientMessage::WelcomeChunkAck {
                                        next_offset,
                                    })
                                    .unwrap()
                                    .into(),
                                ))
                                .map_err(|error| error.to_string())?;
                        }
                        continue;
                    }
                    if socket.can_write() {
                        socket
                            .send(Message::Text(
                                serde_json::to_string(&ClientMessage::WelcomeChunkAck {
                                    next_offset,
                                })
                                .unwrap()
                                .into(),
                            ))
                            .map_err(|error| error.to_string())?;
                    }
                }
                if let ServerMessage::Hosted {
                    message:
                        HostedServerMessage::SaveCheckpointChunk {
                            data,
                            offset,
                            total_length,
                            reason,
                            sequence,
                        },
                } = message
                {
                    let next_offset = offset + data.len();
                    let _ = observations.send(ClientObservation::SaveProgress {
                        sequence,
                        received: next_offset,
                        total: total_length,
                    });
                    let mut complete = None;
                    if sequence > saved && saving.is_none_or(|current| sequence >= current) {
                        if saving != Some(sequence) {
                            save.close();
                            saving = Some(sequence);
                        }
                        complete = save.accept(&GameStringChunk { data, offset, total_length })?;
                    }
                    if socket.can_write() {
                        socket
                            .send(Message::Text(
                                serde_json::to_string(&ClientMessage::SaveCheckpointChunkAck {
                                    sequence,
                                    next_offset,
                                })
                                .unwrap()
                                .into(),
                            ))
                            .map_err(|error| error.to_string())?;
                    }
                    let Some(complete) = complete else {
                        continue;
                    };
                    saved = sequence;
                    saving = None;
                    let save = String::from_utf16(&complete).map_err(|error| error.to_string())?;
                    message = ServerMessage::Hosted {
                        message: HostedServerMessage::SaveCheckpoint { save, reason, sequence },
                    };
                }
                if let ServerMessage::Hosted {
                    message: HostedServerMessage::SaveCheckpoint { sequence, .. },
                } = &message
                    && *sequence > saved
                {
                    saved = *sequence;
                    if saving.is_some_and(|current| current <= saved) {
                        save.close();
                        saving = None;
                    }
                }
                if let ServerMessage::Welcome { frame, .. } | ServerMessage::Snapshot { frame } =
                    &message
                {
                    let presentation = frame
                        .flame_presentation
                        .as_ref()
                        .ok_or("Native frame omitted current Flame history")?;
                    if matches!(&message, ServerMessage::Welcome { .. })
                        && !matches!(
                            presentation,
                            solomon_protocol::flame_presentation::FlamePresentationFrame::Reset { .. }
                        )
                    {
                        return Err("A native Welcome must reset its Flame baseline".into());
                    }
                    flame.accept(presentation)?;
                }
                if let ServerMessage::Snapshot { frame } = &message
                    && socket.can_write()
                {
                    let ack = serde_json::to_string(&ClientMessage::SnapshotAck {
                        sequence: frame.sequence,
                    })
                    .map_err(|error| error.to_string())?;
                    socket.send(Message::Text(ack.into())).map_err(|error| error.to_string())?;
                }
                if observations.send(ClientObservation::Message(Box::new(message))).is_err() {
                    return Ok(());
                }
            }
            Ok(Message::Close(_)) | Err(Error::ConnectionClosed) => {
                let _ = socket.flush();
                return Ok(());
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                socket.flush().map_err(|error| error.to_string())?;
            }
            Ok(other) => return Err(format!("unexpected hosted socket output: {other:?}")),
            Err(Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
}

#[test]
fn console_and_learned_bots_follow_the_actual_source_supervisor_lifecycle() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/hosted-console-goldens.json"
    ))
    .unwrap();
    let mut host = Host::start_with_policy(&"a".repeat(40), true);
    let (_, endpoint) = host.request("POST", "/admin/hub/tickets", content(), true);
    let (mut guest, _, _) = Client::join(&host, &endpoint, "Guest");
    let mut rows = vec![json!({"case":"global-ordinary-lua-rejected",
        "result":execute_lua(&mut guest,1,"return 17")})];
    let (_, endpoint) = host.request("POST", "/admin/sessions", content(), true);
    let (mut private_owner, _, _) = Client::join(&host, &endpoint, "Private owner");
    for (case, request, code) in [
        ("private-authority-with-cheats-off", 1, "return 17, \"actual Lua\""),
        ("persistent-console-globals", 2, "counter = (counter or 0) + 1; return counter"),
        ("persistent-console-globals-next-request", 3, "counter = counter + 1; return counter"),
    ] {
        rows.push(json!({"case":case,"result":execute_lua(&mut private_owner,request,code)}));
    }
    let mut admission = content();
    admission["developerAccess"] = Value::Bool(true);
    let (_, endpoint) = host.request("POST", "/admin/hub/tickets", admission.clone(), true);
    let (mut developer, _, _) = Client::join(&host, &endpoint, "Developer");
    let summoned = execute_lua(
        &mut developer,
        1,
        "return sd.bots.summon({element=\"water\",discipline=\"mind\"})",
    );
    assert_eq!(summoned["ok"], true, "{summoned}");
    let bot = &summoned["values"][0];
    let bot_id = bot["player_id"].as_str().unwrap().to_owned();
    rows.push(json!({"case":"actual-bot-reservation","name":bot["display_name"],
        "opaqueId":bot_id.starts_with("bot-") && bot_id.len()==20}));
    let message = developer.until(|message| {
        matches!(message, ServerMessage::Social {snapshot}
        if snapshot.party_state.as_ref().is_some_and(|state|
            state.hub_players.iter().any(|player| player.player_id==bot_id)))
    });
    let ServerMessage::Social { snapshot } = message else { unreachable!() };
    let mut names: Vec<_> = snapshot
        .party_state
        .unwrap()
        .hub_players
        .into_iter()
        .map(|player| player.display_name)
        .collect();
    names.sort();
    rows.push(json!({"case":"admitted-bot-roster","names":names}));
    let (status, mut health) = host.request("GET", "/health", Value::Null, false);
    assert_eq!(status, 200);
    // Each runtime owns its wire protocol; compare the same supervisor health fields.
    assert_eq!(health["protocol"], "solomon-game-v1");
    health.as_object_mut().unwrap().remove("protocol");
    assert_eq!(health.as_object_mut().unwrap().remove("deploymentProtocol"), Some(json!(2)));
    rows.push(json!({"case":"live-bot-health","health":health}));
    developer.send(ClientMessage::Command {
        sequence: 1,
        command: GameCommand::Social { action: SocialAction::Invite { player_id: bot_id.clone() } },
    });
    let invited_at = Instant::now();
    developer.until(|message| matches!(message, ServerMessage::Social {snapshot}
        if snapshot.party_state.as_ref().is_some_and(|state| state.party.member_player_ids.len()==2)));
    let (_, presence) = host.request("GET", "/admin/presence", Value::Null, true);
    let actual =
        presence["items"].as_array().unwrap().iter().find(|row| row["bot"] == true).unwrap();
    rows.push(json!({"case":"delayed-bot-invitation",
        "waitedAtLeastThreeSeconds":invited_at.elapsed()>=Duration::from_millis(2900),
        "bot":actual["bot"],"developer":actual["developer"],"displayName":actual["displayName"],
        "partyLeader":actual["partyLeader"],"partySize":actual["partySize"],"activity":actual["activity"]}));
    let (_, endpoint) = host.request("POST", "/admin/hub/tickets", admission, true);
    let (mut other, _, _) = Client::join(&host, &endpoint, "Other developer");
    rows.push(json!({"case":"one-console-owner-per-authority",
        "result":execute_lua(&mut other,1,"return 23")}));
    developer.close();
    developer.finish_close();
    wait_for_presence(&host, |rows| !rows.iter().any(|row| row["displayName"] == "Developer"));
    rows.push(json!({"case":"new-console-owner-after-disconnect",
        "result":execute_lua(&mut other,2,"return counter == nil, 23")}));
    guest.close();
    guest.finish_close();
    other.close();
    other.finish_close();
    let presence = wait_for_presence(&host, |rows| {
        !rows.iter().any(|row| row["session"] == "global-hub" && row["bot"] == false)
    });
    rows.push(json!({"case":"last-shared-human-removes-bots",
        "bots":presence.iter().filter(|row| row["bot"]==true).count()}));
    let actual: solomon_sim::modding::LuaValue = serde_json::from_value(json!(rows)).unwrap();
    let expected: solomon_sim::modding::LuaValue =
        serde_json::from_value(fixture["rows"].clone()).unwrap();
    assert_eq!(actual, expected);
    host.finish();
}

fn execute_lua(client: &mut Client, request_id: u64, code: &str) -> Value {
    client.send(ClientMessage::Hosted {
        command: HostedCommand::LuaExecute { request_id, code: code.into() },
    });
    let message = client.until(|message| {
        matches!(message, ServerMessage::Hosted {
        message: HostedServerMessage::LuaResult {request_id:actual,..}
    } if *actual==request_id)
    });
    let ServerMessage::Hosted {
        message: HostedServerMessage::LuaResult { error, ok, output, values, .. },
    } = message
    else {
        unreachable!()
    };
    json!({"error":error,"ok":ok,"output":output,"values":values})
}

fn wait_for_presence(host: &Host, ready: impl Fn(&[Value]) -> bool) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let (_, presence) = host.request("GET", "/admin/presence", Value::Null, true);
        let rows = presence["items"].as_array().unwrap();
        if ready(rows) {
            return rows.clone();
        }
        assert!(
            Instant::now() < deadline,
            "actual presence did not reach the expected lifecycle boundary"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
impl Client {
    fn connect(host: &Host, endpoint: &Value) -> Self {
        Self::connect_with_origin(host, endpoint, ORIGIN)
    }
    fn connect_with_origin(host: &Host, endpoint: &Value, origin: &str) -> Self {
        let mut request = format!("ws://{}{}", host.address, endpoint["path"].as_str().unwrap())
            .into_client_request()
            .unwrap();
        request.headers_mut().insert("origin", origin.parse().unwrap());
        let stream = TcpStream::connect(host.address).unwrap();
        // Match the live native client's immediate commands and checkpoint ACKs.
        stream.set_nodelay(true).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(8))).unwrap();
        let socket = tungstenite::client(request, stream).unwrap().0;
        // Short reads let the established client poll commands; the handshake needs
        // the same bounded wait as other authority operations on a busy CI runner.
        socket.get_ref().set_read_timeout(Some(Duration::from_millis(10))).unwrap();
        let (writes, incoming_writes) = mpsc::channel();
        let (outgoing_observations, observations) = mpsc::channel();
        let worker = thread::spawn(move || {
            let observation = match drive_client(socket, incoming_writes, &outgoing_observations) {
                Ok(()) => ClientObservation::Closed,
                Err(reason) => ClientObservation::Failed(reason),
            };
            let _ = outgoing_observations.send(observation);
        });
        Self {
            writes,
            observations,
            pending: VecDeque::new(),
            worker: Some(worker),
            closed: false,
            save_progress: None,
        }
    }
    fn join(host: &Host, endpoint: &Value, name: &str) -> (Self, String, GameSessionKind) {
        let (client, welcome) = Self::join_with_welcome(host, endpoint, name);
        let ServerMessage::Welcome { credentials, hosted: Some(hosted), .. } = welcome else {
            panic!("hosted welcome missing")
        };
        (client, credentials.player_id, hosted.session_kind)
    }
    fn join_with_welcome(
        host: &Host,
        endpoint: &Value,
        name: &str,
    ) -> (Self, ServerMessage<Value>) {
        Self::join_with_save(host, endpoint, name, None)
    }
    fn join_with_save(
        host: &Host,
        endpoint: &Value,
        name: &str,
        save: Option<String>,
    ) -> (Self, ServerMessage<Value>) {
        let selection = if let Some(save) = &save {
            let saved = solomon_assets::website_save::restore(save).unwrap();
            saved.session.participants[&saved.player_id].player.selection.clone()
        } else {
            NewGameSelection {
                name: name.into(),
                discipline: WizardDiscipline::Arcane,
                element: WizardElement::Ether,
            }
        };
        let mut client = Self::connect(host, endpoint);
        client.send(ClientMessage::Hosted {
            command: HostedCommand::Hello {
                hello: HostedHello {
                    version: PROTOCOL_VERSION,
                    credential: endpoint["credential"].as_str().unwrap().into(),
                    selection,
                    profile: PlayerSocialProfile::default(),
                    online_preferences: GameOnlinePreferences::default(),
                    resume_token: None,
                    save_intent: save.as_ref().map(|_| GameSaveIntent::Resume),
                    save,
                    begin_college_intro: false,
                    decline_tutorial: false,
                    cheats_enabled: false,
                    enhanced_effects: true,
                    allow_mod_mismatch: false,
                },
            },
        });
        let welcome = client.until(|message| matches!(message, ServerMessage::Welcome { .. }));
        (client, welcome)
    }
    #[track_caller]
    fn send(&mut self, message: ClientMessage) {
        let (completed, result) = mpsc::channel();
        self.writes.send(ClientWrite::Message { message: Box::new(message), completed }).unwrap();
        result.recv_timeout(Duration::from_secs(8)).unwrap().unwrap();
    }

    fn close(&mut self) {
        self.close_with_reason(None);
    }

    fn close_with_reason(&mut self, reason: Option<String>) {
        let (completed, result) = mpsc::channel();
        self.writes.send(ClientWrite::Close { reason, completed }).unwrap();
        result.recv_timeout(Duration::from_secs(8)).unwrap().unwrap();
    }

    #[track_caller]
    fn until(&mut self, accept: impl Fn(&ServerMessage<Value>) -> bool) -> ServerMessage<Value> {
        if let Some(index) = self.pending.iter().position(&accept) {
            return self.pending.remove(index).unwrap();
        }
        let progress_before = self.save_progress;
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            match self.observations.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(ClientObservation::Message(message)) => {
                    if accept(&message) {
                        return *message;
                    }
                    // Source next() retains unmatched reliable messages. In particular,
                    // waiting for Loaded cannot consume the run's only checkpoint.
                    self.pending.push_back(*message);
                }
                Ok(ClientObservation::SaveProgress { sequence, received, total }) => {
                    self.save_progress = Some((sequence, received, total));
                }
                Ok(ClientObservation::Closed) => {
                    self.closed = true;
                    panic!("the hosted socket closed before its requested observation");
                }
                Ok(ClientObservation::Failed(reason)) => {
                    panic!("hosted connection failed: {reason}")
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => panic!("the socket listener stopped"),
            }
        }
        let errors: Vec<_> = self
            .pending
            .iter()
            .filter_map(|message| match message {
                ServerMessage::Error { code, reason } => Some((code, reason)),
                _ => None,
            })
            .collect();
        let latest = self.pending.iter().rev().find_map(|message| match message {
            ServerMessage::Snapshot { frame } => {
                Some((&frame.host, &frame.snapshot["run"], &frame.snapshot["ticks"]))
            }
            _ => None,
        });
        let completed_saves: Vec<_> =
            self.pending
                .iter()
                .filter_map(|message| match message {
                    ServerMessage::Hosted {
                        message: HostedServerMessage::SaveCheckpoint { sequence, save, .. },
                    } => {
                        Some((
                            *sequence,
                            serde_json::from_str::<Value>(save).unwrap()["continuation"]["summary"]
                                ["worldKind"]
                                .clone(),
                        ))
                    }
                    _ => None,
                })
                .collect();
        panic!(
            "the real hosted authority did not reach the requested state; \
             pending errors: {errors:?}; latest host/run/tick: {latest:?}; \
             save transfer before/after: {progress_before:?}/{:?}; completed saves: {completed_saves:?}",
            self.save_progress
        )
    }

    #[track_caller]
    fn finish_close(&mut self) {
        if self.closed {
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            match self.observations.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(ClientObservation::Closed) => {
                    self.closed = true;
                    return;
                }
                Ok(ClientObservation::Failed(reason)) => {
                    panic!("deployment close failed: {reason}")
                }
                Ok(ClientObservation::Message(_) | ClientObservation::SaveProgress { .. }) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => panic!("the socket listener stopped"),
            }
        }
        panic!("deployment must close its owned player socket")
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.writes.send(ClientWrite::Stop);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn kind(value: GameSessionKind) -> &'static str {
    match value {
        GameSessionKind::GlobalHub => "global-hub",
        GameSessionKind::PrivateCollege => "private-college",
        GameSessionKind::Standalone => "standalone",
    }
}
fn content() -> Value {
    json!({"content":{"manifestSha256":"0".repeat(64),"mods":[]}})
}

#[test]
fn restored_hosted_boneyards_advance_after_the_actual_renderer_and_resume_admissions() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/save-document-goldens.json"
    ))
    .unwrap();
    let document = if let Some(path) = std::env::var_os("SOLOMON_TEST_CONTINUATION_DOCUMENT") {
        std::fs::read_to_string(path).unwrap()
    } else {
        fixture["cases"].as_array().unwrap().iter().find(|case| case["name"] == "boneyard").unwrap()
            ["document"]
            .to_string()
    };
    let source = solomon_assets::website_save::restore(&document).unwrap();
    let run_id = source.loaded_boneyard.as_ref().unwrap().run_id.clone();
    let saved_tick = source.session.ticks;
    let id = source.player_id;
    let name = source.session.participants[&id].player.selection.name.clone();
    let routes =
        if source.metadata.integrity == solomon_assets::website_save::SaveIntegrity::GlobalClean {
            vec!["/admin/hub/tickets", "/admin/sessions"]
        } else {
            // The actual ZIP witness is local-only. Its source continuation may enter a
            // private College; admitting it into Global would weaken the trust boundary.
            vec!["/admin/sessions"]
        };
    for route in routes {
        let mut host = Host::start();
        let mut warm_global = if route == "/admin/sessions" {
            let (_, endpoint) = host.request("POST", "/admin/hub/tickets", content(), true);
            let (mut original, _, _) = Client::join(&host, &endpoint, "Original guest");
            original.send(ClientMessage::Hosted {
                command: HostedCommand::StartMatch { boneyard_id: "default-random".into() },
            });
            let loaded =
                original.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }));
            let ServerMessage::BoneyardLoaded { boneyard } = loaded else { unreachable!() };
            let loading = original.until(|message| {
                matches!(message, ServerMessage::Snapshot { frame }
                    if frame.host.resume_grace.is_some())
            });
            let ServerMessage::Snapshot { frame } = loading else { unreachable!() };
            original.send(ClientMessage::ReadyBoneyard { run_id: boneyard.run_id.clone() });
            original.send(ClientMessage::ResumeReady {
                sequence: frame.host.resume_grace.as_ref().unwrap().sequence,
            });
            original.until(|message| {
                matches!(message, ServerMessage::Snapshot { frame }
                    if frame.host.resume_grace.is_none()
                        && frame.snapshot["ticks"].as_u64().unwrap() > 100)
            });
            Some(original)
        } else {
            None
        };
        let (status, endpoint) = host.request("POST", route, content(), true);
        assert_eq!(status, 201, "{route}: {endpoint}");
        let (mut client, message) =
            Client::join_with_save(&host, &endpoint, &name, Some(document.clone()));
        let ServerMessage::Welcome { credentials, frame, .. } = message else { unreachable!() };
        assert_eq!(credentials.player_id, id);
        assert_eq!(frame.snapshot["run"]["run_id"], run_id);
        assert_eq!(frame.snapshot["ticks"], saved_tick);
        client.send(ClientMessage::ReadyBoneyard { run_id: run_id.clone() });
        client.send(ClientMessage::ResumeReady {
            sequence: frame.host.resume_grace.as_ref().unwrap().sequence,
        });
        let message = client.until(|message| {
            matches!(message, ServerMessage::Snapshot { frame }
                if frame.host.loading_run_id.is_none()
                    && frame.host.resume_grace.is_none()
                    && frame.snapshot["ticks"].as_u64().unwrap() > saved_tick + 100)
        });
        let ServerMessage::Snapshot { frame } = message else { unreachable!() };
        assert_eq!(frame.snapshot["boneyard"]["run_id"], run_id);
        assert_eq!(frame.snapshot["participant_order"], json!([id]));
        if let Some(original) = &mut warm_global {
            let (_, matches) = host.request("GET", "/admin/matches", Value::Null, true);
            let retained_match = matches["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["players"].as_array().unwrap().iter().any(|player| player == &name))
                .unwrap();
            let (status, endpoint) = host.request(
                "POST",
                "/admin/observers",
                json!({"matchId":retained_match["id"],
                    "observer":{"userId":71,"username":"Teardown auditor"}}),
                true,
            );
            assert_eq!(status, 201);
            let mut observer = Client::connect(&host, &endpoint);
            observer.send(ClientMessage::Hosted {
                command: HostedCommand::ObserverHello {
                    version: PROTOCOL_VERSION,
                    credential: endpoint["credential"].as_str().unwrap().into(),
                },
            });
            observer.until(|message| matches!(message, ServerMessage::Welcome { .. }));
            // The browser witness keeps its original Global run alive while importing
            // a private continuation, then closes the imported browser. Retaining an
            // actual observer keeps this owner from being pruned before its next epoch.
            client.close();
            client.finish_close();
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                let (_, health) = host.request("GET", "/health", Value::Null, true);
                if health["privatePlayers"] == 0 {
                    break;
                }
                assert!(Instant::now() < deadline, "the private participant must disconnect");
                thread::sleep(Duration::from_millis(10));
            }
            original.send(ClientMessage::Ping { nonce: 719 });
            original.until(|message| matches!(message, ServerMessage::Pong { nonce: 719 }));
            let preceding_tick = original
                .pending
                .iter()
                .filter_map(|message| match message {
                    ServerMessage::Snapshot { frame } => frame.snapshot["ticks"].as_u64(),
                    _ => None,
                })
                .max()
                .unwrap();
            original.until(|message| {
                matches!(message, ServerMessage::Snapshot { frame }
                    if frame.snapshot["ticks"].as_u64().unwrap() > preceding_tick + 100)
            });
            observer.finish_close();
        }
        host.finish();
    }
}

#[test]
fn prepared_runtime_rules_publish_and_save_the_actual_authoritative_world() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/hosted-mod-runtime-goldens.json"
    ))
    .unwrap();
    let mut host = Host::start();
    let (status, endpoint) =
        host.request("POST", "/admin/sessions", json!({"content":fixture["payload"]}), true);
    assert_eq!(status, 201, "{endpoint}");
    let (mut client, welcome) = Client::join_with_welcome(&host, &endpoint, "Authored Runtime");
    let ServerMessage::Welcome { hosted: Some(welcome), .. } = welcome else { unreachable!() };
    let initial = client.until(|message| {
        matches!(message, ServerMessage::Hosted { message: HostedServerMessage::ModContent { .. } })
    });
    let ServerMessage::Hosted { message: HostedServerMessage::ModContent { projection } } = initial
    else {
        unreachable!()
    };
    let initial = serde_json::to_value(projection).unwrap();
    let catalog_ids: Vec<_> =
        welcome.mod_catalog.iter().map(|item| &item.content.content_id).collect();
    let mut rows = vec![json!({"case":"prepared-private-welcome", "catalogIds":catalog_ids,
        "contentCount":initial["content"].as_array().unwrap().len(),"powerups":initial["powerups"]})];
    client.send(ClientMessage::Hosted {
        command: HostedCommand::StartMatch { boneyard_id: "default-random".into() },
    });
    let loaded = client.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }));
    let ServerMessage::BoneyardLoaded { boneyard } = loaded else { unreachable!() };
    let grace = client.until(|message| {
        matches!(message, ServerMessage::Snapshot { frame }
        if frame.host.resume_grace.is_some() && !frame.snapshot["boneyard"].is_null())
    });
    let ServerMessage::Snapshot { frame } = grace else { unreachable!() };
    client.send(ClientMessage::ReadyBoneyard { run_id: boneyard.run_id.clone() });
    client.send(ClientMessage::ResumeReady { sequence: frame.host.resume_grace.unwrap().sequence });
    let runtime = client.until(|message| matches!(message, ServerMessage::Hosted {
        message: HostedServerMessage::ModRuntime { projection, .. }
    } if projection.get("powerup_actors").and_then(solomon_sim::modding::LuaValue::as_array).is_some_and(|rows| rows.len() == 1)));
    let ServerMessage::Hosted { message: HostedServerMessage::ModRuntime { projection, .. } } =
        runtime
    else {
        unreachable!()
    };
    let runtime = serde_json::to_value(projection).unwrap();
    let mut powerups = runtime["powerup_actors"].clone();
    let mut birth_ticks = Vec::new();
    for powerup in powerups.as_array_mut().unwrap() {
        birth_ticks.push(powerup["spawned_tick"].as_f64().unwrap());
        powerup.as_object_mut().unwrap().remove("spawned_tick");
    }
    rows.push(json!({"case":"actual-run-start-rule","powerups":powerups}));
    client.send(ClientMessage::SaveBeforeLeave { request_id: 1 });
    let saved = client.until(|message| match message {
        ServerMessage::Hosted { message: HostedServerMessage::SaveCheckpoint { save, .. } } => {
            let document: Value = serde_json::from_str(save).unwrap();
            document["continuation"]["summary"]["worldKind"] == "boneyard"
                && document["modState"].as_object().is_some_and(|states| {
                    states.values().any(|state| {
                        state["powerups"].as_array().is_some_and(|rows| rows.len() == 1)
                    })
                })
        }
        _ => false,
    });
    let ServerMessage::Hosted { message: HostedServerMessage::SaveCheckpoint { save, .. } } = saved
    else {
        unreachable!()
    };
    let document: Value = serde_json::from_str(&save).unwrap();
    let saved_tick = document["continuation"]["summary"]["savedAtTick"].as_f64().unwrap();
    let mut mod_state = document["modState"].clone();
    // Real socket scheduling chooses the entry tick. Preserve its causal ordering;
    // controlled-tick host fixtures separately compare every exact clock value.
    for state in mod_state.as_object_mut().unwrap().values_mut() {
        for powerup in state["powerups"].as_array_mut().unwrap() {
            assert!(powerup["spawned_tick"].as_f64().unwrap() <= saved_tick);
            powerup.as_object_mut().unwrap().remove("spawned_tick");
        }
    }
    rows.push(json!({"case":"actual-prepared-save","modState":mod_state,
        "birthNotAfterSave":birth_ticks.into_iter().all(|tick| tick <= saved_tick)}));
    client.send(ClientMessage::Hosted {
        command: HostedCommand::ModAction {
            action: solomon_protocol::hosted::ModAction::ShopBuy,
            arguments: serde_json::from_value(json!({"row":0})).unwrap(),
            request_id: 1,
            target: runtime["shops"][0]["content_id"].as_str().unwrap().into(),
        },
    });
    client.until(|message| match message {
        ServerMessage::Hosted {
            message:
                HostedServerMessage::SaveCheckpoint {
                    save,
                    reason: solomon_protocol::hosted::SaveCheckpointReason::GameOver,
                    ..
                },
        } => {
            let document: Value = serde_json::from_str(save).unwrap();
            document["continuation"].is_null()
        }
        _ => false,
    });
    client.send(ClientMessage::SaveBeforeLeave { request_id: 2 });
    let receipt = client.until(|message| {
        matches!(
            message,
            ServerMessage::Hosted {
                message: HostedServerMessage::SaveBeforeLeave { request_id: 2, .. }
            }
        )
    });
    let ServerMessage::Hosted {
        message: HostedServerMessage::SaveBeforeLeave { checkpoint_sequence, .. },
    } = receipt
    else {
        unreachable!()
    };
    assert!(checkpoint_sequence > 0, "explicit terminal leave must publish its actual profile");
    let terminal = client.until(|message| {
        matches!(message, ServerMessage::Hosted {
        message: HostedServerMessage::SaveCheckpoint { sequence, .. }
    } if *sequence == checkpoint_sequence)
    });
    let ServerMessage::Hosted { message: HostedServerMessage::SaveCheckpoint { save, reason, .. } } =
        terminal
    else {
        unreachable!()
    };
    let document: Value = serde_json::from_str(&save).unwrap();
    rows.push(json!({"case":"explicit-terminal-leave","nonzero":checkpoint_sequence > 0,
        "reason":reason,"profileOnly":document["continuation"].is_null()}));
    let actual: solomon_sim::modding::LuaValue =
        serde_json::from_value(Value::Array(rows)).unwrap();
    let expected: solomon_sim::modding::LuaValue =
        serde_json::from_value(fixture["rows"].clone()).unwrap();
    assert_eq!(actual, expected);
    client.close();
    host.finish();
}

#[test]
fn supervisor_replays_source_http_colleges_broker_and_party_membership() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/hosted-live-goldens.json"
    ))
    .unwrap();
    let mut host = Host::start();
    let mut rows = Vec::new();
    let (status, body) = host.request("POST", "/admin/sessions", content(), false);
    rows.push(json!({"case":"unauthorized","status":status,"body":body}));
    let mut invalid = content();
    invalid["leaderboardUserId"] = json!(0);
    let (status, body) = host.request("POST", "/admin/sessions", invalid, true);
    rows.push(json!({"case":"invalid-account","status":status,"body":body}));
    let (one_status, one) = host.request("POST", "/admin/sessions", content(), true);
    let (two_status, two) = host.request("POST", "/admin/sessions", content(), true);
    rows.push(json!({"case":"private-provision","status":[one_status,two_status],"distinctPaths":one["path"]!=two["path"],
        "distinctCredentials":one["credential"]!=two["credential"],"kind":one["sessionKind"]}));
    let (mut a, alpha, alpha_kind) = Client::join(&host, &one, "Alpha");
    let (mut b, beta, beta_kind) = Client::join(&host, &two, "Beta");
    rows.push(json!({"case":"private-owners","playerIds":[alpha,beta],"kinds":[kind(alpha_kind),kind(beta_kind)]}));
    a.send(ClientMessage::Command {
        sequence: 1,
        command: GameCommand::Social {
            action: SocialAction::SendChat {
                channel: GameChatChannel::Global,
                text: "hello Colleges".into(),
                recipient_player_reference: None,
            },
        },
    });
    let message = b.until(
        |message| matches!(message,ServerMessage::Chat{message}if message.text=="hello Colleges"),
    );
    let ServerMessage::Chat { message } = message else { unreachable!() };
    rows.push(json!({"case":"cross-college-chat","channel":"global","sender":message.sender.display_name,
        "referenceIsOpaque":message.sender.player_id.starts_with("player-ref-"),"senderReferenceMatches":message.sender.player_id==message.sender.player_reference}));
    let (_, endpoint) = host.request("POST", "/admin/hub/tickets", content(), true);
    let (mut h1, gamma, _) = Client::join(&host, &endpoint, "Gamma");
    let (_, endpoint) = host.request("POST", "/admin/hub/tickets", content(), true);
    let (mut h2, delta, _) = Client::join(&host, &endpoint, "Delta");
    h1.send(ClientMessage::Command {
        sequence: 1,
        command: GameCommand::Social { action: SocialAction::Invite { player_id: delta } },
    });
    let message=h2.until(|message|matches!(message,ServerMessage::Social{snapshot}if snapshot.party_state.as_ref().is_some_and(|party|party.invitations.len()==1)));
    let ServerMessage::Social { snapshot } = message else { unreachable!() };
    let invitation = snapshot.party_state.unwrap().invitations.remove(0).id;
    h2.send(ClientMessage::Command {
        sequence: 1,
        command: GameCommand::Social {
            action: SocialAction::AcceptInvitation { invitation_id: invitation },
        },
    });
    let message=h1.until(|message|matches!(message,ServerMessage::Social{snapshot}if snapshot.party_state.as_ref().is_some_and(|party|party.party.member_player_ids.len()==2)));
    let ServerMessage::Social { snapshot } = message else { unreachable!() };
    let state = snapshot.party_state.unwrap();
    let names: Vec<_> = state.party_roster.iter().map(|row| row.display_name.clone()).collect();
    let mut hub_names: Vec<_> =
        state.hub_players.iter().map(|row| row.display_name.clone()).collect();
    hub_names.sort();
    rows.push(json!({"case":"global-party-merge","leaderRetained":state.party.leader_player_id==gamma,"memberNames":names,"hubNames":hub_names}));
    let (status, body) =
        host.request("POST", "/admin/join/resolve", json!({"code":state.party.join_code}), true);
    let intent = body["intentId"].as_str().unwrap();
    rows.push(json!({"case":"code-resolution","status":status,"target":body["target"],"intentOpaque":intent.len()==32&&intent.bytes().all(|byte|byte.is_ascii_alphanumeric()||matches!(byte,b'_'|b'-'))}));
    let (_, mut health) = host.request("GET", "/health", Value::Null, true);
    health.as_object_mut().unwrap().remove("protocol");
    assert_eq!(health.as_object_mut().unwrap().remove("deploymentProtocol"), Some(json!(2)));
    rows.push(json!({"case":"health","body":health}));
    a.send(ClientMessage::Hosted {
        command: HostedCommand::StartMatch { boneyard_id: "default-random".into() },
    });
    let loaded = a.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }));
    let ServerMessage::BoneyardLoaded { boneyard: loaded } = loaded else { unreachable!() };
    rows.push(json!({"case":"actual-loaded-world","runAndSeedDistinct":loaded.run_id!=loaded.seed,
        "choice":loaded.choice_id,"source":loaded.choice_source,"named":!loaded.name.is_empty(),
        "sourceDigest":loaded.source_sha256.len()==64,"geometryDigest":loaded.geometry_sha256.len()==64,
        "authoredObjects":!loaded.scene.objects.is_empty(),"authoredSprites":!loaded.scene.sprites.is_empty()}));
    a.until(|message|matches!(message,ServerMessage::Snapshot {frame} if !frame.snapshot["boneyard"].is_null()));
    let (status, matches) = host.request("GET", "/admin/matches", Value::Null, true);
    let observed_match = matches["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["players"].as_array().unwrap().iter().any(|name| name == "Alpha"))
        .unwrap();
    rows.push(json!({"case":"observer-directory","status":status,"players":observed_match["players"],
        "partyLeader":observed_match["partyLeader"],"playerCount":observed_match["playerCount"],
        "session":observed_match["session"],"visibility":observed_match["visibility"],
        "boneyardNamed":!observed_match["boneyardName"].as_str().unwrap().is_empty(),"waveNumber":observed_match["waveNumber"]}));
    let (status, endpoint) = host.request(
        "POST",
        "/admin/observers",
        json!({"matchId":observed_match["id"],"observer":{"userId":71,"username":"Auditor"}}),
        true,
    );
    let mut observer = Client::connect(&host, &endpoint);
    observer.send(ClientMessage::Hosted {
        command: HostedCommand::ObserverHello {
            version: PROTOCOL_VERSION,
            credential: endpoint["credential"].as_str().unwrap().into(),
        },
    });
    let message = observer.until(|message| matches!(message, ServerMessage::Welcome { .. }));
    let ServerMessage::Welcome { credentials, hosted: Some(admitted), .. } = message else {
        panic!("observer welcome missing")
    };
    rows.push(json!({"case":"observer-hello","status":status,"observer":admitted.observer,
        "developer":admitted.developer_access,"existingView":credentials.player_id=="player-1","session":kind(admitted.session_kind)}));
    observer.send(ClientMessage::Command {
        sequence: 1,
        command: GameCommand::Social { action: SocialAction::LeaveParty },
    });
    let message = observer.until(|message| matches!(message, ServerMessage::Error { .. }));
    let ServerMessage::Error { code, reason } = message else { unreachable!() };
    assert_eq!(code, solomon_protocol::HostErrorCode::InvalidMessage);
    let (_, health) = host.request("GET", "/health", Value::Null, true);
    rows.push(json!({"case":"observer-readonly","code":"invalid-message","reason":reason,"humanPlayers":health["privatePlayers"]}));
    let (_, presence) = host.request("GET", "/admin/presence", Value::Null, true);
    let mut items:Vec<_>=presence["items"].as_array().unwrap().iter().map(|row|json!({"displayName":row["displayName"],"activity":row["activity"],"partySize":row["partySize"],"developer":row["developer"],"session":row["session"]})).collect();
    items.sort_by(|left, right| {
        left["displayName"].as_str().unwrap().cmp(right["displayName"].as_str().unwrap())
    });
    rows.push(json!({"case":"presence","items":items}));

    let mut missing_field = fixture["authoredContent"].clone();
    missing_field["mods"][0]["files"] = json!([]);
    missing_field["mods"][0].as_object_mut().unwrap().remove("entryScript");
    let (status, body) =
        host.request("POST", "/admin/sessions", json!({"content":missing_field}), true);
    rows.push(json!({"case":"package-missing-nullable-field","status":status,"body":body}));
    let mut invalid_lua = fixture["authoredContent"].clone();
    invalid_lua["mods"][0]["files"] = json!([]);
    invalid_lua["mods"][0]["entryScript"] = json!("sd.kit.itme{key=\"bad\"}");
    let (status, body) =
        host.request("POST", "/admin/sessions", json!({"content":invalid_lua}), true);
    rows.push(json!({"case":"package-lua-rejected","status":status,"body":body}));
    let (status, endpoint) = host.request(
        "POST",
        "/admin/sessions",
        json!({"content":fixture["authoredContent"],"leaderboardUserId":1.0}),
        true,
    );
    assert_eq!(status, 201, "actual Lua package must provision: {endpoint}");
    let (mut authored_client, welcome) = Client::join_with_welcome(&host, &endpoint, "Authored");
    let ServerMessage::Welcome { hosted: Some(authored), .. } = welcome else {
        panic!("the authored College returns its actual prepared content")
    };
    rows.push(json!({"case":"package-lua-catalog","status":status,
        "content":authored.content,"catalog":authored.mod_catalog,"assets":authored.mod_assets}));

    h1.send(ClientMessage::Hosted {
        command: HostedCommand::StartMatch { boneyard_id: "default-random".into() },
    });
    h1.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }));
    let message = h2.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }));
    let ServerMessage::BoneyardLoaded { boneyard: party_world } = message else { unreachable!() };
    let message = h2.until(|message| {
        matches!(message, ServerMessage::Hosted {
            message: HostedServerMessage::SaveCheckpoint { save, .. }
        } if serde_json::from_str::<Value>(save).unwrap()["continuation"]["summary"]["worldKind"]=="boneyard")
    });
    let ServerMessage::Hosted {
        message: HostedServerMessage::SaveCheckpoint { save: party_save, .. },
    } = message
    else {
        unreachable!()
    };
    let document: Value = serde_json::from_str(&party_save).unwrap();
    let token = document["continuation"]["summary"]["partyRejoinToken"].as_str().unwrap();
    let rejoin_request = json!({"content":content()["content"],"token":token,"save":party_save});
    let (status, body) = host.request("POST", "/admin/rejoin", rejoin_request.clone(), true);
    rows.push(json!({"case":"rejoin-connected","status":status,"body":body}));
    let mut normalized = rejoin_request.clone();
    normalized["save"] = json!(format!("{party_save} "));
    let (status, body) = host.request("POST", "/admin/rejoin", normalized, true);
    rows.push(json!({"case":"rejoin-normalized-document","status":status,"body":body}));
    let mut changed = document.clone();
    changed["continuation"]["summary"]["savedAtTick"] =
        json!(document["continuation"]["summary"]["savedAtTick"].as_u64().unwrap() + 1);
    let mut invalid_document = rejoin_request.clone();
    invalid_document["save"] = json!(changed.to_string());
    let (status, body) = host.request("POST", "/admin/rejoin", invalid_document, true);
    rows.push(json!({"case":"rejoin-invalid-document","status":status,"body":body}));
    let mut wrong_account = rejoin_request.clone();
    wrong_account["leaderboardUserId"] = json!(17);
    let (status, body) = host.request("POST", "/admin/rejoin", wrong_account, true);
    rows.push(json!({"case":"rejoin-wrong-account","status":status,"body":body}));
    let mut legacy = rejoin_request.clone();
    legacy["token"] = json!("r".repeat(43));
    let (status, body) = host.request("POST", "/admin/rejoin", legacy, true);
    rows.push(json!({"case":"rejoin-legacy-ended","status":status,"body":body}));
    let departed = document["continuation"]["summary"]["playerId"].as_str().unwrap();
    h2.close();
    h2.finish_close();
    h1.until(|message| {
        matches!(message,ServerMessage::Social{snapshot}
        if snapshot.party_state.as_ref().is_some_and(|party| party.party_roster.iter()
            .any(|row| row.player_id==departed&&!row.connected)))
    });
    let (status, reserved) = host.request("POST", "/admin/rejoin", rejoin_request.clone(), true);
    rows.push(json!({"case":"rejoin-reserved","status":status,"session":reserved["sessionKind"],
        "sameWorld":reserved["path"]=="/game-hub"}));
    let (status, body) = host.request("POST", "/admin/rejoin", rejoin_request, true);
    rows.push(json!({"case":"rejoin-already-reserved","status":status,"body":body}));
    let (rejoined, welcome) =
        Client::join_with_save(&host, &reserved, "Delta", Some(party_save.clone()));
    h2 = rejoined;
    let ServerMessage::Welcome { credentials, frame, .. } = welcome else { unreachable!() };
    let message = h2.until(|message| matches!(message, ServerMessage::Social { .. }));
    let ServerMessage::Social { snapshot } = message else { unreachable!() };
    let names: Vec<_> = snapshot
        .party_state
        .unwrap()
        .party_roster
        .into_iter()
        .map(|row| row.display_name)
        .collect();
    rows.push(json!({"case":"rejoin-owner","samePlayer":credentials.player_id==departed,
        "sameRun":frame.snapshot["run"]["run_id"]==party_world.run_id,"partyNames":names}));

    let target_revision = "b".repeat(40);
    let address = host.address;
    let revision = target_revision.clone();
    let restart = thread::spawn(move || {
        request_at(
            address,
            "POST",
            "/admin/deployments/restart",
            json!({"targetRevision":revision}),
            true,
        )
    });
    let mut clients = [&mut a, &mut b, &mut h1, &mut h2, &mut authored_client];
    let mut checkpoint_sequences = Vec::new();
    let mut deployment_saves = vec![None; clients.len()];
    for (index, client) in clients.iter_mut().enumerate() {
        if matches!(index, 0 | 2 | 3) {
            let message = client.until(|message| {
                let ServerMessage::Hosted {
                    message: HostedServerMessage::SaveCheckpoint { save, .. },
                } = message
                else {
                    return false;
                };
                let document: Value = serde_json::from_str(save).unwrap();
                let Some(token) = document["continuation"]["summary"]["partyRejoinToken"].as_str()
                else {
                    return false;
                };
                solomon_launcher::host::recovery::decode_party_recovery_claim(SECRET, token)
                    .is_some_and(|claim| {
                        claim.target_revision.as_deref() == Some(target_revision.as_str())
                    })
            });
            let ServerMessage::Hosted { message: HostedServerMessage::SaveCheckpoint { save, .. } } =
                message
            else {
                unreachable!()
            };
            deployment_saves[index] = Some(save);
        }
        let message = client.until(|message| {
            matches!(
                message,
                ServerMessage::Hosted {
                    message: solomon_protocol::hosted::HostedServerMessage::DeploymentRestart { .. }
                }
            )
        });
        let ServerMessage::Hosted {
            message:
                solomon_protocol::hosted::HostedServerMessage::DeploymentRestart {
                    checkpoint_sequence,
                    target_revision: announced_revision,
                    deployment_id,
                    deployment_sequence,
                },
        } = message
        else {
            unreachable!()
        };
        assert_eq!(announced_revision, target_revision);
        assert_eq!(deployment_id, "d1".repeat(16));
        assert_eq!(deployment_sequence, 1);
        checkpoint_sequences.push(checkpoint_sequence);
    }
    let mut request = format!("ws://{}/game-hub", host.address).into_client_request().unwrap();
    request.headers_mut().insert("origin", ORIGIN.parse().unwrap());
    let Error::Http(rejected) = tungstenite::connect(request).unwrap_err() else {
        panic!("deployment rejects the HTTP upgrade before opening a WebSocket")
    };
    let (status, body) = host.request("POST", "/admin/hub/tickets", content(), true);
    rows.push(json!({"case":"deployment-draining","upgradeStatus":rejected.status().as_u16(),
        "admission":{"status":status,"body":body}}));
    clients[3].send(ClientMessage::Hosted {
        command: HostedCommand::DeploymentReady {
            checkpoint_sequence: checkpoint_sequences[3] + 1,
            target_revision: target_revision.clone(),
        },
    });
    let message = clients[3].until(|message| matches!(message, ServerMessage::Error { .. }));
    let ServerMessage::Error { code, reason } = message else { unreachable!() };
    assert_eq!(code, solomon_protocol::HostErrorCode::InvalidMessage);
    rows.push(json!({"case":"deployment-stale-ack","code":"invalid-message","reason":reason}));
    clients[3].finish_close();
    for index in [0, 1, 2, 4] {
        clients[index].send(ClientMessage::Hosted {
            command: HostedCommand::DeploymentReady {
                checkpoint_sequence: checkpoint_sequences[index],
                target_revision: target_revision.clone(),
            },
        });
    }
    for index in [0, 1, 2, 4] {
        clients[index].finish_close();
    }
    let (status, mut body) = restart.join().unwrap();
    assert_eq!(body.as_object_mut().unwrap().remove("deploymentId"), Some(json!("d1".repeat(16))));
    assert_eq!(body.as_object_mut().unwrap().remove("deploymentSequence"), Some(json!(1)));
    rows.push(json!({"case":"deployment-ready","status":status,"body":body}));
    let (_, health) = host.request("GET", "/health", Value::Null, true);
    rows.push(json!({"case":"deployment-closed-colleges","draining":health["draining"],
        "privateSessions":health["privateSessions"]}));

    // A replacement process starts with no hidden player/world restoration. The first
    // signed browser continuation seeds one actual run; other members wait for Hello.
    host.finish();
    host = Host::start_with_revision(&target_revision);
    let previous_recovery = json!({"content":content()["content"],
        "save":party_save,"token":document["continuation"]["summary"]["partyRejoinToken"]});
    let (status, body) = host.request("POST", "/admin/rejoin", previous_recovery, true);
    rows.push(json!({"case":"rejoin-wrong-deployment","status":status,"body":body}));
    let recovery_request = |index: usize| {
        let save = deployment_saves[index].as_ref().unwrap();
        let document: Value = serde_json::from_str(save).unwrap();
        json!({"content":content()["content"],"save":save,
            "token":document["continuation"]["summary"]["partyRejoinToken"]})
    };
    let (status, seed) = host.request("POST", "/admin/rejoin", recovery_request(3), true);
    rows.push(json!({"case":"recovery-global-seed","status":status,"session":seed["sessionKind"],
        "globalPath":seed["path"]=="/game-hub"}));
    let address = host.address;
    let member_request = recovery_request(2);
    let waiting_member =
        thread::spawn(move || request_at(address, "POST", "/admin/rejoin", member_request, true));
    let (mut recovered_delta, welcome) =
        Client::join_with_save(&host, &seed, "Delta", deployment_saves[3].clone());
    let ServerMessage::Welcome { credentials, frame, .. } = welcome else { unreachable!() };
    let message = recovered_delta.until(|message| matches!(message, ServerMessage::Social { .. }));
    let ServerMessage::Social { snapshot } = message else { unreachable!() };
    let recovered_roster = snapshot.party_state.unwrap();
    let seed_names: Vec<_> = frame.snapshot["participant_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| {
            frame.snapshot["participants"][id.as_str().unwrap()]["player"]["selection"]["name"]
                .clone()
        })
        .collect();
    let offline: Vec<_> = recovered_roster
        .party_roster
        .iter()
        .filter(|row| !row.connected)
        .map(|row| row.display_name.clone())
        .collect();
    rows.push(
        json!({"case":"recovery-offline-leader","samePlayer":credentials.player_id==departed,
        "sameRun":frame.snapshot["run"]["run_id"]==party_world.run_id,
        "leaderPreserved":recovered_roster.party.leader_player_id==gamma,
        "onlySeedActor":seed_names,"offline":offline}),
    );
    let (status, second_endpoint) = waiting_member.join().unwrap();
    rows.push(json!({"case":"recovery-waiting-member","status":status,"samePath":second_endpoint["path"]==seed["path"]}));
    let (mut recovered_gamma, welcome) =
        Client::join_with_save(&host, &second_endpoint, "Gamma", deployment_saves[2].clone());
    let ServerMessage::Welcome { credentials, frame, .. } = welcome else { unreachable!() };
    let message = recovered_gamma.until(|message| matches!(message, ServerMessage::Social { .. }));
    let ServerMessage::Social { snapshot } = message else { unreachable!() };
    let recovered_party = snapshot.party_state.unwrap();
    let mut names: Vec<_> = frame.snapshot["participants"]
        .as_object()
        .unwrap()
        .values()
        .map(|participant| participant["player"]["selection"]["name"].as_str().unwrap().to_owned())
        .collect();
    names.sort();
    let party_names: Vec<_> =
        recovered_party.party_roster.iter().map(|row| row.display_name.clone()).collect();
    let (_, health) = host.request("GET", "/health", Value::Null, true);
    rows.push(json!({"case":"recovery-shared-world","samePlayer":credentials.player_id==gamma,
        "sameRun":frame.snapshot["run"]["run_id"]==party_world.run_id,"actorNames":names,
        "partyNames":party_names,"onlyOneRun":health["runs"]==1}));
    let (status, private_seed) = host.request("POST", "/admin/rejoin", recovery_request(0), true);
    let (_recovered_alpha, welcome) =
        Client::join_with_save(&host, &private_seed, "Alpha", deployment_saves[0].clone());
    let ServerMessage::Welcome { credentials, frame, .. } = welcome else { unreachable!() };
    let (_, health) = host.request("GET", "/health", Value::Null, true);
    rows.push(json!({"case":"recovery-private-seed","status":status,"session":private_seed["sessionKind"],
        "privatePath":private_seed["path"].as_str().unwrap().starts_with("/game-sessions/"),
        "samePlayer":credentials.player_id==alpha,"sameRun":frame.snapshot["run"]["run_id"]==loaded.run_id,
        "privateSessions":health["privateSessions"]}));
    // Source JSON has one Number type: a frame width serialized as 1.0 carries
    // exactly the same value as 1. Arrays and every numeric value remain exact.
    let expected = fixture["rows"].as_array().unwrap();
    assert_eq!(rows.len(), expected.len(), "every independent Source observation is retained");
    for (actual, expected) in rows.into_iter().zip(expected) {
        let case = expected["case"].as_str().unwrap();
        let actual: solomon_sim::modding::LuaValue = serde_json::from_value(actual).unwrap();
        let expected: solomon_sim::modding::LuaValue =
            serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(actual, expected, "Source supervisor observation {case}");
    }
    host.finish();
}
