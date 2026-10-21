//! Real standalone desktop scope replay over its public /game WebSocket.

use super::*;
use solomon_protocol::HostErrorCode;

const DESKTOP_ORIGIN: &str = "solomon-darker://app";
const DESKTOP_CREDENTIAL: &str = "desktop-authority-public-test-credential";

fn start_desktop() -> Host {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web");
    let config = HostedSupervisorConfig::new(assets, DESKTOP_CREDENTIAL.into(), "a".repeat(40));
    let server = NativeServer::start_hosted(
        TcpListener::bind(("127.0.0.1", 0)).unwrap(),
        TransportConfig {
            allowed_origins: vec![DESKTOP_ORIGIN.into()],
            max_peers: 32,
            ..TransportConfig::default()
        },
    )
    .unwrap();
    let address = server.local_addr();
    let stop = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stop);
    let (ready, received_ready) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("desktop-authority-journey".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            HostedSupervisor::new_standalone(config, DESKTOP_CREDENTIAL.into())?.run_when_ready(
                server,
                signal,
                |_| {
                    ready.send(()).unwrap();
                },
            )
        })
        .unwrap();
    received_ready.recv_timeout(Duration::from_secs(8)).unwrap();
    Host { address, stop, worker: Some(worker) }
}

fn connect_desktop(host: &Host, name: &str, credential: &str, save: Option<String>) -> Client {
    let mut client = Client::connect_with_origin(host, &json!({"path":"/game"}), DESKTOP_ORIGIN);
    client.send(ClientMessage::Hosted {
        command: HostedCommand::Hello {
            hello: HostedHello {
                version: PROTOCOL_VERSION,
                credential: credential.into(),
                selection: NewGameSelection {
                    name: name.into(),
                    discipline: WizardDiscipline::Arcane,
                    element: WizardElement::Ether,
                },
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
    client
}

fn welcome_row(client: &mut Client, case: &str) -> Value {
    let message = client.until(|message| matches!(message, ServerMessage::Welcome { .. }));
    let ServerMessage::Welcome { credentials, hosted: Some(hosted), frame, .. } = message else {
        panic!("The desktop endpoint requires an ordinary hosted welcome");
    };
    let id = credentials.player_id;
    // Native wire enums keep their Rust spelling; compare the Source projection
    // using its public labels without changing the canonical transport schema.
    let kind = match hosted.session_kind {
        GameSessionKind::Standalone => "standalone",
        GameSessionKind::PrivateCollege => "private-college",
        GameSessionKind::GlobalHub => "global-hub",
    };
    json!({"case":case,"id":id,"kind":kind,"cheats":hosted.cheats_enabled,
        "gold":frame.snapshot["participants"][&id]["player"]["economy"]["gold"],
        "players":frame.snapshot["participant_order"].as_array().unwrap().len()})
}

#[test]
fn desktop_authority_retains_source_bootstrap_console_save_and_owner_lifetimes() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../sim/tests/fixtures/website/desktop-authority-goldens.json"
    ))
    .unwrap();
    let mut host = start_desktop();
    let mut invalid = connect_desktop(&host, "Rejected", "wrong", None);
    let error = invalid.until(|message| matches!(message, ServerMessage::Error { .. }));
    assert!(matches!(error, ServerMessage::Error { code: HostErrorCode::InvalidCredentials, .. }));
    invalid.finish_close();
    let mut rows = vec![json!({"case":"wrong-bootstrap","code":"authentication-failed"})];
    let mut owner = connect_desktop(&host, "Owner", DESKTOP_CREDENTIAL, None);
    rows.push(welcome_row(&mut owner, "owner"));
    let mut guest = connect_desktop(&host, "Guest", DESKTOP_CREDENTIAL, None);
    rows.push(welcome_row(&mut guest, "same-bootstrap-admits-peer"));
    rows.push(
        json!({"case":"guest-console-rejected","result":execute_lua(&mut guest,1,"return 17")}),
    );
    guest.send(ClientMessage::Hosted { command: HostedCommand::SetCheats { enabled: true } });
    let enabled = guest.until(|message| {
        matches!(message, ServerMessage::Hosted { message: HostedServerMessage::CheatMode { .. } })
    });
    let ServerMessage::Hosted { message: HostedServerMessage::CheatMode { enabled } } = enabled
    else {
        unreachable!()
    };
    rows.push(json!({"case":"guest-cheat-mode-is-local","enabled":enabled}));
    rows.push(json!({"case":"owner-console","result":execute_lua(&mut owner,1,
        "counter = 3; sd.player.set_gold(4321); return counter, sd.player.get_state().display_name")}));
    owner.send(ClientMessage::SaveBeforeLeave { request_id: 1 });
    let ack = owner.until(|message| {
        matches!(
            message,
            ServerMessage::Hosted {
                message: HostedServerMessage::SaveBeforeLeave { request_id: 1, .. }
            }
        )
    });
    let ServerMessage::Hosted {
        message: HostedServerMessage::SaveBeforeLeave { checkpoint_sequence, .. },
    } = ack
    else {
        unreachable!()
    };
    let save = owner.until(|message| matches!(message, ServerMessage::Hosted {
        message:HostedServerMessage::SaveCheckpoint { sequence,.. } } if *sequence==checkpoint_sequence));
    let ServerMessage::Hosted { message: HostedServerMessage::SaveCheckpoint { save, .. } } = save
    else {
        unreachable!()
    };
    let parsed = solomon_assets::website_save::parse(&save).unwrap();
    let summary = parsed.continuation.unwrap().summary;
    rows.push(
        json!({"case":"durable-source-save-before-leave","integrity":parsed.metadata.integrity,
        "schemaVersion":parsed.source_schema_version,"gold":parsed.profile_economy["gold"],
        "worldKind":summary.world_kind,"playerId":summary.player_id}),
    );
    owner.close();
    owner.finish_close();
    // The guest's next accepted snapshot proves the source close callback ran;
    // no arbitrary delay or test-only world mutation advances ownership.
    guest.until(|message| {
        matches!(message, ServerMessage::Snapshot { frame }
        if frame.snapshot["participant_order"]==json!(["player-2"]))
    });
    rows.push(json!({"case":"next-peer-becomes-console-owner",
        "result":execute_lua(&mut guest,2,"return counter == nil, 23")}));
    guest.close();
    guest.finish_close();
    let mut fresh = connect_desktop(&host, "Fresh", DESKTOP_CREDENTIAL, None);
    rows.push(welcome_row(&mut fresh, "last-human-resets-owner"));
    fresh.close();
    fresh.finish_close();
    let mut resumed = connect_desktop(&host, "Owner", DESKTOP_CREDENTIAL, Some(save));
    rows.push(welcome_row(&mut resumed, "source-save-resumes-on-fresh-owner"));
    resumed.close();
    resumed.finish_close();
    let expected_rows = fixture["rows"].as_array().unwrap();
    assert_eq!(rows.len(), expected_rows.len());
    for (actual, expected) in rows.into_iter().zip(expected_rows) {
        let case = actual["case"].as_str().unwrap().to_owned();
        let actual: solomon_sim::modding::LuaValue = serde_json::from_value(actual).unwrap();
        let expected: solomon_sim::modding::LuaValue =
            serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(actual, expected, "{case}");
    }
    host.finish();
}
