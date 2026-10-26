//! Real loopback clients exercise the packaged authority, not a second test simulation.

use std::fs;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use solomon_launcher::AssetServer;
use solomon_launcher::host::{NativeAuthority, NativeHostConfig};
use solomon_launcher::transport::{NativeServer, TransportConfig};
use solomon_protocol::{
    ClientMessage, GameCommand, HostErrorCode, PROTOCOL_VERSION, PauseSource, ResumeCredentials,
    ServerMessage, SnapshotFrame, WEBSOCKET_PATH,
};
use solomon_sim::Vec2;
use solomon_sim::economy::HubInventoryAction;
use solomon_sim::profile::{NewGameSelection, PlayerSkillChoice, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionInput, GameSessionOptions};
use solomon_sim::social::{GameChatChannel, SocialAction};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Error, Message, WebSocket};

struct HostThread {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<(), String>>>,
}

impl HostThread {
    fn start(checkpoint: PathBuf) -> Self {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web");
        let authority =
            NativeAuthority::load(NativeHostConfig::new(assets.clone(), checkpoint)).unwrap();
        let transport = NativeServer::start(
            TcpListener::bind(("127.0.0.1", 0)).unwrap(),
            AssetServer::new(&assets).unwrap(),
            TransportConfig::default(),
        )
        .unwrap();
        let address = transport.local_addr();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        // Native authority owns the complete source state. The test runner's default
        // worker stack is smaller than the launcher's main-thread stack on macOS.
        let worker = thread::Builder::new()
            .name("native-host-journey".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || authority.run(transport, stopping))
            .unwrap();
        Self { address, stop, worker: Some(worker) }
    }

    fn finish(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap().unwrap();
    }
}

impl Drop for HostThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            // Preserve the original assertion if a journey fails before explicit finish.
            let _ = worker.join();
        }
    }
}

struct Client(WebSocket<TcpStream>, solomon_protocol::snapshot::SnapshotReader);

impl Client {
    fn connect(host: &HostThread) -> Self {
        let mut request =
            format!("ws://{}{WEBSOCKET_PATH}", host.address).into_client_request().unwrap();
        request.headers_mut().insert("origin", format!("http://{}", host.address).parse().unwrap());
        let stream = TcpStream::connect(host.address).unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(250))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
        Self(tungstenite::client(request, stream).unwrap().0, Default::default())
    }

    fn send(&mut self, message: ClientMessage) {
        self.0.send(Message::Text(serde_json::to_string(&message).unwrap().into())).unwrap();
    }

    fn receive(&mut self) -> Option<ServerMessage<Value>> {
        match self.0.read() {
            Ok(Message::Text(text)) => Some(self.1.decode::<Value>(&text).unwrap()),
            Ok(Message::Ping(_)) => {
                self.0.flush().unwrap();
                None
            }
            Ok(other) => panic!("unexpected native transport output: {other:?}"),
            Err(Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                None
            }
            Err(error) => panic!("native authority closed during the journey: {error}"),
        }
    }

    fn until(&mut self, accept: impl Fn(&ServerMessage<Value>) -> bool) -> ServerMessage<Value> {
        let mut welcome = solomon_protocol::transfer::GameStringReceiver::new(
            solomon_protocol::GAME_WEBSOCKET_MAX_PAYLOAD_BYTES,
            "welcome".into(),
        );
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            let Some(mut message) = self.receive() else { continue };
            if let ServerMessage::WelcomeChunk { data, offset, total_length } = message {
                let next_offset = offset + data.len();
                let complete = welcome
                    .accept(&solomon_protocol::transfer::GameStringChunk {
                        data,
                        offset,
                        total_length,
                    })
                    .unwrap();
                let Some(complete) = complete else {
                    self.send(ClientMessage::WelcomeChunkAck { next_offset });
                    continue;
                };
                message = serde_json::from_str(&String::from_utf16(&complete).unwrap()).unwrap();
                assert!(matches!(message, ServerMessage::Welcome { .. }));
                self.send(ClientMessage::WelcomeChunkAck { next_offset });
            }
            match &message {
                ServerMessage::Welcome { frame, .. } | ServerMessage::Snapshot { frame } => {
                    self.send(ClientMessage::SnapshotAck { sequence: frame.sequence });
                }
                _ => {}
            }
            if accept(&message) {
                return message;
            }
        }
        panic!("native authority did not reach the requested observable state");
    }

    fn hello(
        &mut self,
        name: &str,
        resume: Option<ResumeCredentials>,
    ) -> (ResumeCredentials, SnapshotFrame<Value>) {
        self.send(ClientMessage::Hello {
            version: PROTOCOL_VERSION,
            selection: selection(name),
            resume,
            initial_save: None,
            begin_college_intro: false,
            enhanced_effects: true,
        });
        let ServerMessage::Welcome { credentials, frame, .. } =
            self.until(|message| matches!(message, ServerMessage::Welcome { .. }))
        else {
            unreachable!()
        };
        (credentials, frame)
    }

    fn frame(&mut self, accept: impl Fn(&SnapshotFrame<Value>) -> bool) -> SnapshotFrame<Value> {
        let ServerMessage::Snapshot { frame } = self.until(|message| {
            matches!(message,
            ServerMessage::Snapshot { frame } if accept(frame))
        }) else {
            unreachable!()
        };
        frame
    }

    fn inspect(&mut self, resume: Option<ResumeCredentials>) -> (bool, bool) {
        self.send(ClientMessage::InspectSession { resume });
        let ServerMessage::SessionStatus { can_resume, fresh_join_available, .. } =
            self.until(|message| matches!(message, ServerMessage::SessionStatus { .. }))
        else {
            unreachable!()
        };
        (can_resume, fresh_join_available)
    }

    fn save(&mut self, request_id: u64) -> u64 {
        self.send(ClientMessage::SaveBeforeLeave { request_id });
        let ServerMessage::SaveCommitted { tick, .. } = self.until(|message| {
            matches!(message,
            ServerMessage::SaveCommitted { request_id: received, .. } if *received == request_id)
        }) else {
            unreachable!()
        };
        tick
    }
}

fn selection(name: &str) -> NewGameSelection {
    NewGameSelection {
        name: name.into(),
        element: WizardElement::Fire,
        discipline: WizardDiscipline::Mind,
    }
}

fn tick(frame: &SnapshotFrame<Value>) -> u64 {
    frame.snapshot["ticks"].as_u64().unwrap()
}

fn ready(client: &mut Client, frame: &SnapshotFrame<Value>) {
    client.send(ClientMessage::ReadyBoneyard {
        run_id: frame.snapshot["run"]["run_id"].as_str().unwrap().into(),
    });
    client.send(ClientMessage::ResumeReady {
        sequence: frame.host.resume_grace.as_ref().unwrap().sequence,
    });
}

#[test]
fn interrupted_reconnect_keeps_the_last_received_credential_until_admission_is_acknowledged() {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let directory = std::env::temp_dir()
        .join(format!("solomon-interrupted-resume-{}-{unique}", std::process::id()));
    let checkpoint = directory.join("host.json");
    let mut host = HostThread::start(checkpoint.clone());
    let mut first = Client::connect(&host);
    let (received, first_frame) = first.hello("Retained", None);
    first.save(1);
    drop(first);

    for _ in 0..2 {
        let mut interrupted = Client::connect(&host);
        interrupted.send(ClientMessage::Hello {
            version: PROTOCOL_VERSION,
            selection: selection("Retained"),
            resume: Some(received.clone()),
            initial_save: None,
            begin_college_intro: false,
            enhanced_effects: true,
        });
        let deadline = Instant::now() + Duration::from_secs(8);
        let admission = loop {
            assert!(Instant::now() < deadline, "the host must start its Welcome");
            if let Some(message) = interrupted.receive() {
                break message;
            }
        };
        assert!(matches!(
            admission,
            ServerMessage::Welcome { .. } | ServerMessage::WelcomeChunk { .. }
        ));
        // The transport receives bytes, but the client never accepts or ACKs them.
        drop(interrupted);
        assert_eq!(
            Client::connect(&host).inspect(Some(received.clone())),
            (true, true),
            "a lost Welcome must not consume the only credential the client received"
        );
        host.finish();
        host = HostThread::start(checkpoint.clone());
        assert_eq!(
            Client::connect(&host).inspect(Some(received.clone())),
            (true, true),
            "interrupted admission remains recoverable after a host restart"
        );
    }

    let mut resumed = Client::connect(&host);
    let (rotated, frame) = resumed.hello("Retained", Some(received.clone()));
    assert_eq!(rotated.player_id, received.player_id);
    assert_ne!(rotated.token, received.token);
    assert_eq!(
        frame.snapshot["participants"][&received.player_id]["player"],
        first_frame.snapshot["participants"][&received.player_id]["player"]
    );
    resumed.save(2);
    assert_eq!(Client::connect(&host).inspect(Some(received.clone())), (false, true));
    assert_eq!(Client::connect(&host).inspect(Some(rotated.clone())), (true, true));
    host.finish();
    drop(resumed);
    host = HostThread::start(checkpoint);
    assert_eq!(Client::connect(&host).inspect(Some(received)), (false, true));
    assert_eq!(Client::connect(&host).inspect(Some(rotated)), (true, true));
    host.finish();
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn two_real_clients_share_commands_readiness_resume_and_durable_profiles() {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let directory =
        std::env::temp_dir().join(format!("solomon-authority-{}-{unique}", std::process::id()));
    let checkpoint = directory.join("host.json");
    let mut host = HostThread::start(checkpoint.clone());
    assert_eq!(Client::connect(&host).inspect(None), (false, true));

    let mut first = Client::connect(&host);
    let (first_credentials, first_welcome) = first.hello("First", None);
    let first_id = first_credentials.player_id.clone();
    let start_x = first_welcome.snapshot["participants"][&first_id]["wizard"]["position"]["x"]
        .as_f64()
        .unwrap();
    let mut second = Client::connect(&host);
    let (second_credentials, second_welcome) = second.hello("Second", None);
    let second_id = second_credentials.player_id.clone();
    assert_eq!(
        second_welcome.snapshot["participant_order"],
        serde_json::json!([first_id, second_id])
    );

    first.send(ClientMessage::Input {
        sequence: 1,
        target_tick: tick(&first_welcome) + 1,
        input: GameSessionInput { movement: Vec2::new(1.0, 0.0), ..Default::default() },
    });
    let moved = first.frame(|frame| {
        frame.input_acknowledgements.get(&first_id) == Some(&1)
            && frame.snapshot["participants"][&first_id]["wizard"]["position"]["x"]
                .as_f64()
                .unwrap()
                > start_x
    });
    let item_id =
        moved.snapshot["participants"][&first_id]["player"]["economy"]["backpack"][0]["id"]
            .as_u64()
            .unwrap();
    let discard = || ClientMessage::Command {
        sequence: 1,
        command: GameCommand::Inventory { action: HubInventoryAction::Discard { item_id } },
    };
    first.send(discard());
    let receipt =
        first.until(|message| matches!(message, ServerMessage::CommandResult { sequence: 1, .. }));
    let ServerMessage::CommandResult {
        accepted: true,
        snapshot_sequence,
        inventory_result: Some(ref result),
        ..
    } = receipt
    else {
        panic!("the real carried item must be discarded once")
    };
    assert!(result.accepted);
    let applied = first.frame(|frame| frame.sequence >= snapshot_sequence);
    let economy = applied.snapshot["participants"][&first_id]["player"]["economy"].clone();
    first.send(discard());
    let duplicate =
        first.until(|message| matches!(message, ServerMessage::CommandResult { sequence: 1, .. }));
    assert_eq!(serde_json::to_value(duplicate).unwrap(), serde_json::to_value(receipt).unwrap());
    let unchanged = first.frame(|frame| frame.sequence > applied.sequence);
    assert_eq!(unchanged.snapshot["participants"][&first_id]["player"]["economy"], economy);

    first.send(ClientMessage::Command {
        sequence: 2,
        command: GameCommand::Social {
            action: SocialAction::SendChat {
                channel: GameChatChannel::Party,
                text: "  shared party  ".into(),
                recipient_player_reference: None,
            },
        },
    });
    first.until(|message| matches!(message, ServerMessage::Chat { message } if message.text == "shared party"));
    second.until(|message| matches!(message, ServerMessage::Chat { message } if message.sender.player_id == first_id && message.text == "shared party"));

    second.send(ClientMessage::StartRun { tutorial: false });
    second.until(|message| {
        matches!(message, ServerMessage::Error { code: HostErrorCode::CommandRejected, .. })
    });
    second.send(ClientMessage::Ping { nonce: 17 });
    second.until(|message| matches!(message, ServerMessage::Pong { nonce: 17 }));
    first.send(ClientMessage::StartRun { tutorial: false });
    let loading = first.frame(|frame| frame.host.loading_run_id.is_some());
    let other_loading =
        second.frame(|frame| frame.host.loading_run_id == loading.host.loading_run_id);
    let stopped_tick = tick(&loading);
    first.send(ClientMessage::Command {
        sequence: 3,
        command: GameCommand::SelectPrimary {
            skill_id:
                loading.snapshot["participants"][&first_id]["player"]["book"]["primary_skill_id"]
                    .as_u64()
                    .unwrap() as usize,
        },
    });
    first.until(|message| {
        matches!(message, ServerMessage::CommandResult { sequence: 3, accepted: false, .. })
    });
    ready(&mut first, &loading);
    assert_eq!(first.save(1), stopped_tick);
    ready(&mut second, &other_loading);
    let running =
        first.frame(|frame| frame.host.resume_grace.is_none() && tick(frame) > stopped_tick);
    first.send(ClientMessage::GameplayPause { paused: true, source: PauseSource::Inventory });
    let paused = first.frame(|frame| frame.host.pause.is_some());
    assert!(tick(&paused) >= tick(&running));
    second.send(ClientMessage::GameplayPause { paused: false, source: PauseSource::Inventory });
    second.send(
        ClientMessage::Command {
            sequence: 1,
            command:
                GameCommand::SelectPrimary {
                    skill_id: second_welcome.snapshot["participants"][&second_id]["player"]["book"]
                        ["primary_skill_id"]
                        .as_u64()
                        .unwrap() as usize,
                },
        },
    );
    second.until(|message| {
        matches!(message, ServerMessage::CommandResult { sequence: 1, accepted: false, .. })
    });
    assert_eq!(first.save(2), tick(&paused));
    first.send(ClientMessage::GameplayPause { paused: false, source: PauseSource::Inventory });
    first.frame(|frame| {
        frame.host.pause.is_none()
            && frame.host.resume_grace.is_none()
            && tick(frame) > tick(&paused)
    });

    first.0.close(None).unwrap();
    let _ = first.0.flush();
    second.frame(|frame| frame.snapshot["participants"].get(&first_id).is_none());
    let mut resumed = Client::connect(&host);
    let (rotated, resumed_frame) = resumed.hello("First", Some(first_credentials.clone()));
    assert_eq!(rotated.player_id, first_id);
    assert_ne!(rotated.token, first_credentials.token);
    assert_eq!(resumed_frame.snapshot["participants"][&first_id]["player"]["economy"], economy);
    ready(&mut resumed, &resumed_frame);
    let rejoin_sequence = resumed_frame.host.resume_grace.as_ref().unwrap().sequence;
    second.frame(|frame| {
        frame.host.resume_grace.as_ref().is_some_and(|grace| grace.sequence == rejoin_sequence)
    });
    // The existing browser already owns this world's GPU resources. Only the
    // reconnecting browser sends ReadyBoneyard; both acknowledge the new grace.
    second.send(ClientMessage::ResumeReady { sequence: rejoin_sequence });
    resumed.frame(|frame| frame.host.resume_grace.is_none() && tick(frame) > tick(&resumed_frame));
    resumed.send(ClientMessage::Command {
        sequence: 1,
        command: GameCommand::Social {
            action: SocialAction::Kick { player_id: second_id.clone() },
        },
    });
    resumed.until(|message| {
        matches!(message, ServerMessage::CommandResult { sequence: 1, accepted: true, .. })
    });
    assert_eq!(Client::connect(&host).inspect(Some(second_credentials.clone())), (false, false));

    resumed.send(ClientMessage::Command {
        sequence: 2,
        command: GameCommand::ResetWizard { selection: selection("Replacement") },
    });
    let reset = resumed.frame(|frame| {
        frame.snapshot["boneyard"].is_null()
            && frame.snapshot["participants"][&first_id]["player"]["selection"]["name"]
                == "Replacement"
    });
    assert!(!reset.snapshot["participants"][&first_id]["college_intro"].is_null());
    assert_eq!(Client::connect(&host).inspect(Some(second_credentials)), (true, true));
    resumed.save(3);
    host.finish();
    assert!(checkpoint.is_file());

    let mut restarted = HostThread::start(checkpoint);
    assert_eq!(Client::connect(&restarted).inspect(Some(rotated.clone())), (true, true));
    let mut restored_client = Client::connect(&restarted);
    let (_, restored) = restored_client.hello("Replacement", Some(rotated));
    assert_eq!(
        restored.snapshot["participants"][&first_id]["player"]["selection"]["name"],
        "Replacement"
    );
    assert!(tick(&restored) >= tick(&reset));
    restored_client.save(4);
    restored_client
        .send(ClientMessage::Command { sequence: 0, command: GameCommand::InteractGoodie });
    restored_client.until(|message| {
        matches!(message, ServerMessage::Error { code: HostErrorCode::InvalidMessage, .. })
    });
    restarted.finish();
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn pending_rejoin_choices_and_the_single_player_tutorial_survive_socket_loss_and_restart() {
    for tutorial in [false, true] {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let directory =
            std::env::temp_dir().join(format!("solomon-rejoin-{}-{unique}", std::process::id()));
        let checkpoint_path = directory.join("host.json");
        // Prepare the real public-Game state, rather than exposing a network cheat or
        // constructing a second progression model just for this boundary check.
        let mut session =
            GameSession::new("saved".into(), selection("Saved"), GameSessionOptions::default());
        let loaded = if tutorial {
            solomon_assets::boneyard::load_tutorial([0; 16]).unwrap()
        } else {
            solomon_assets::boneyard::load_boneyard([0; 16]).unwrap()
        };
        session.install_boneyard(loaded).unwrap();
        session.grant_experience("saved", 91.0);
        assert!(
            session
                .participants
                .get("saved")
                .expect("participant exists")
                .player
                .progression
                .pending_offer
                .is_some()
        );
        let initial_save = solomon_assets::save_document::encode(&session.checkpoint()).unwrap();
        let mut host = HostThread::start(checkpoint_path.clone());
        let mut client = Client::connect(&host);
        client.send(ClientMessage::Hello {
            version: PROTOCOL_VERSION,
            selection: selection("Saved"),
            resume: None,
            initial_save: Some(initial_save),
            begin_college_intro: false,
            enhanced_effects: true,
        });
        let ServerMessage::Welcome { credentials, .. } =
            client.until(|message| matches!(message, ServerMessage::Welcome { .. }))
        else {
            unreachable!()
        };
        let before: Value =
            serde_json::from_str(&fs::read_to_string(&checkpoint_path).unwrap()).unwrap();
        let previous_revision = before["host"]["revision"].as_u64().unwrap();
        client.0.close(None).unwrap();
        let _ = client.0.flush();
        drop(client);
        let deadline = Instant::now() + Duration::from_secs(5);
        let offline = loop {
            let saved: Value =
                serde_json::from_str(&fs::read_to_string(&checkpoint_path).unwrap()).unwrap();
            if saved["host"]["revision"].as_u64().unwrap() > previous_revision {
                break solomon_assets::save_document::decode(
                    saved["game_document"].as_str().unwrap(),
                )
                .unwrap();
            }
            assert!(Instant::now() < deadline, "the disconnected state must commit");
            thread::sleep(Duration::from_millis(10));
        };
        // Ordinary active-party rejoin removes the actor; Tutorial keeps its sole real
        // actor because both its step and checkpoint invariants require exactly one.
        assert_eq!(offline.participants.len(), usize::from(tutorial));
        host.finish();

        let mut restarted = HostThread::start(checkpoint_path);
        let mut rejoined = Client::connect(&restarted);
        let (_, frame) = rejoined.hello("Saved", Some(credentials));
        assert_eq!(frame.snapshot["boneyard"]["ticks"], offline.boneyard.as_ref().unwrap().ticks);
        let offer =
            &frame.snapshot["participants"]["saved"]["player"]["progression"]["pending_offer"];
        let choice = PlayerSkillChoice {
            choice_index: 0,
            offer_sequence: offer["sequence"].as_u64().unwrap(),
            skill_id: offer["options"][0]["skill_id"].as_u64().unwrap() as usize,
        };
        ready(&mut rejoined, &frame);
        rejoined.send(ClientMessage::Command {
            sequence: 1,
            command: GameCommand::ChooseSkill { choice },
        });
        rejoined.until(|message| {
            matches!(message, ServerMessage::CommandResult { sequence: 1, accepted: true, .. })
        });
        let applied = rejoined.frame(|frame| {
            frame.snapshot["participants"]["saved"]["player"]["progression"]["pending_offer"]
                .is_null()
        });
        assert!(applied.snapshot["level_up_barrier"].is_null());
        assert_eq!(applied.snapshot["participants"]["saved"]["player"]["progression"]["level"], 2);
        rejoined.save(1);
        restarted.finish();
        fs::remove_dir_all(directory).unwrap();
    }
}
