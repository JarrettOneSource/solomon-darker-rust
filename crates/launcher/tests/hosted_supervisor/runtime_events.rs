//! Public current-Source socket actions must reach the real HTTP operator sink.

use std::collections::BTreeMap;

use solomon_launcher::host::runtime_events::RuntimeEventConfig;
use solomon_launcher::transport::{HostedHttpResponse, TransportEvent};
use solomon_sim::modding::LuaValue;
use solomon_sim::social::PartyVisibility;

use super::*;

struct OperatorEndpoint {
    address: SocketAddr,
    incoming: Receiver<Value>,
    rows: Vec<Value>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl OperatorEndpoint {
    fn new() -> Self {
        let mut server = NativeServer::start_hosted(
            TcpListener::bind(("127.0.0.1", 0)).unwrap(),
            TransportConfig::default(),
        )
        .unwrap();
        let address = server.local_addr();
        let (entries, incoming) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            while !signal.load(Ordering::Acquire) {
                match server.recv_timeout(Duration::from_millis(10)) {
                    Ok(TransportEvent::Http { request, respond }) => {
                        assert_eq!(request.path, "/runtime-events");
                        assert_eq!(
                            request.authorization.as_deref(),
                            Some(format!("Bearer {SECRET}").as_str())
                        );
                        entries.send(serde_json::from_slice(&request.body).unwrap()).unwrap();
                        respond.send(HostedHttpResponse::empty(200)).unwrap();
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    _ => panic!("the operator endpoint only accepts actual HTTP requests"),
                }
            }
            server.shutdown();
        });
        Self { address, incoming, rows: Vec::new(), stop, worker: Some(worker) }
    }

    fn config(&self) -> RuntimeEventConfig {
        RuntimeEventConfig::new(&format!("http://{}/runtime-events", self.address), SECRET.into())
            .unwrap()
    }

    #[track_caller]
    fn until(&mut self, event: &str, count: usize) {
        let end = Instant::now() + Duration::from_secs(8);
        while self.rows.iter().filter(|row| row["event"] == event).count() < count {
            self.rows.push(
                self.incoming.recv_timeout(end.saturating_duration_since(Instant::now())).unwrap(),
            );
        }
    }
}

impl Drop for OperatorEndpoint {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn party_id(client: &mut Client) -> String {
    let ServerMessage::Social { snapshot } =
        client.until(|message| matches!(message, ServerMessage::Social { .. }))
    else {
        unreachable!()
    };
    snapshot.party_state.unwrap().party.id
}

fn normalize(value: Value, key: &str, aliases: &BTreeMap<String, String>) -> Value {
    match (key, value) {
        ("durationMs", Value::Number(value)) => json!({"nonnegative":value.as_f64().unwrap()>=0.0}),
        ("serverTick", Value::Number(value)) => {
            json!({"nonnegativeInteger":value.as_f64().is_some_and(|value|value>=0.0&&value.fract()==0.0)})
        }
        (_, Value::String(value)) => Value::String(aliases.get(&value).cloned().unwrap_or(value)),
        (_, Value::Array(values)) => {
            Value::Array(values.into_iter().map(|value| normalize(value, "", aliases)).collect())
        }
        (_, Value::Object(values)) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    let value = normalize(value, &key, aliases);
                    (key, value)
                })
                .collect(),
        ),
        (_, value) => value,
    }
}

#[test]
fn accepted_socket_lifecycle_reaches_the_source_operator_http_events() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../sim/tests/fixtures/website/hosted-runtime-events-goldens.json"
    ))
    .unwrap();
    let mut endpoint = OperatorEndpoint::new();
    let mut host = Host::start_with_events(&"a".repeat(40), false, Some(endpoint.config()));
    let mut aliases = BTreeMap::new();
    let (_, admission) = host.request("POST", "/admin/hub/tickets", content(), true);
    let (mut alpha, alpha_id, _) = Client::join(&host, &admission, "Alpha");
    aliases.insert(alpha_id.clone(), "player:Alpha".into());
    aliases.insert(party_id(&mut alpha), "party:Alpha".into());
    let (_, admission) = host.request("POST", "/admin/hub/tickets", content(), true);
    let (mut beta, beta_id, _) = Client::join(&host, &admission, "Beta");
    aliases.insert(beta_id.clone(), "player:Beta".into());
    aliases.insert(party_id(&mut beta), "party:Beta".into());
    let (_, admission) = host.request("POST", "/admin/sessions", content(), true);
    let (mut private, private_id, _) = Client::join(&host, &admission, "Private");
    aliases.insert(private_id, "player:Private".into());
    aliases.insert(party_id(&mut private), "party:Private".into());
    endpoint.until("session.provisioned", 1);
    let id =
        endpoint.rows.iter().find(|row| row["event"] == "session.provisioned").unwrap()["details"]
            ["sessionId"]
            .as_str()
            .unwrap()
            .to_owned();
    aliases.insert(id, "private:one".into());
    alpha.send(ClientMessage::Command {
        sequence: 1,
        command: GameCommand::Social { action: SocialAction::Invite { player_id: beta_id } },
    });
    let ServerMessage::Social{snapshot} = beta.until(|message|matches!(message,ServerMessage::Social{snapshot} if snapshot.party_state.as_ref().is_some_and(|party|!party.invitations.is_empty()))) else {unreachable!()};
    let invitation = snapshot.party_state.unwrap().invitations[0].id.clone();
    aliases.insert(invitation.clone(), "invitation:one".into());
    beta.send(ClientMessage::Command {
        sequence: 1,
        command: GameCommand::Social {
            action: SocialAction::AcceptInvitation { invitation_id: invitation },
        },
    });
    endpoint.until("party.invitation_accepted", 1);
    alpha.send(ClientMessage::Command {
        sequence: 2,
        command: GameCommand::Social {
            action: SocialAction::SetPartyVisibility { visibility: PartyVisibility::InviteOnly },
        },
    });
    endpoint.until("party.visibility_changed", 1);
    alpha.send(ClientMessage::Command {
        sequence: 3,
        command: GameCommand::Social { action: SocialAction::RotatePartyCode },
    });
    endpoint.until("party.join_code_rotated", 1);
    beta.send(ClientMessage::Command {
        sequence: 2,
        command: GameCommand::Social { action: SocialAction::LeaveParty },
    });
    endpoint.until("party.member_left", 1);
    for (index, client) in [&mut private, &mut beta, &mut alpha].into_iter().enumerate() {
        client.close_with_reason(Some("fixture close".into()));
        client.finish_close();
        endpoint.until("player.disconnected", index + 1);
        if index == 0 {
            endpoint.until("session.closed", 1);
        }
    }
    host.finish();
    let rows: Vec<_> = std::mem::take(&mut endpoint.rows)
        .into_iter()
        .map(|row| {
            let timestamp = row["occurredAtUtc"].as_str().unwrap();
            let separators =
                [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':'), (19, b'.'), (23, b'Z')];
            let valid = timestamp.len() == 24
                && timestamp.bytes().enumerate().all(|(index, byte)| {
                    separators
                        .iter()
                        .find(|(offset, _)| *offset == index)
                        .map_or(byte.is_ascii_digit(), |(_, expected)| byte == *expected)
                });
            json!({"component":row["component"],"event":row["event"],"message":row["message"],
            "timestampIsSourceUtc":valid,"details":normalize(row["details"].clone(),"",&aliases)})
        })
        .collect();
    let actual: LuaValue = serde_json::from_value(json!(rows)).unwrap();
    let expected: LuaValue = serde_json::from_value(fixture["rows"].clone()).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn supervisor_shutdown_releases_actual_clients_and_drains_the_source_operator_events() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../sim/tests/fixtures/website/hosted-shutdown-goldens.json"
    ))
    .unwrap();
    let mut endpoint = OperatorEndpoint::new();
    let mut host = Host::start_with_events(&"a".repeat(40), false, Some(endpoint.config()));
    let mut aliases = BTreeMap::new();
    let mut clients = Vec::new();
    for (route, name) in
        [("/admin/hub/tickets", "Shutdown Hub"), ("/admin/sessions", "Shutdown Private")]
    {
        let (_, admission) = host.request("POST", route, content(), true);
        let (client, id, _) = Client::join(&host, &admission, name);
        aliases.insert(id, format!("player:{name}"));
        clients.push(client);
    }
    endpoint.until("player.connected", 2);
    let private =
        endpoint.rows.iter().find(|row| row["event"] == "session.provisioned").unwrap()["details"]
            ["sessionId"]
            .as_str()
            .unwrap();
    aliases.insert(private.into(), "private:one".into());
    let college = clients.last_mut().unwrap();
    college.send(ClientMessage::Hosted {
        command: HostedCommand::StartMatch { boneyard_id: "default-random".into() },
    });
    let ServerMessage::BoneyardLoaded { boneyard } =
        college.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }))
    else {
        unreachable!()
    };
    aliases.insert(boneyard.run_id, "run:private".into());
    let (_, matches) = host.request("GET", "/admin/matches", Value::Null, true);
    let target = matches["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["players"].as_array().unwrap().iter().any(|name| name == "Shutdown Private")
        })
        .unwrap();
    let (status, admission) = host.request(
        "POST",
        "/admin/observers",
        json!({"matchId":target["id"],"observer":{"userId":71,"username":"Shutdown auditor"}}),
        true,
    );
    assert_eq!(status, 201, "{admission}");
    let mut observer = Client::connect(&host, &admission);
    observer.send(ClientMessage::Hosted {
        command: HostedCommand::ObserverHello {
            version: PROTOCOL_VERSION,
            credential: admission["credential"].as_str().unwrap().into(),
        },
    });
    observer.until(|message| matches!(message, ServerMessage::Welcome { .. }));
    endpoint.until("observer.connected", 1);
    let id = endpoint.rows.iter().find(|row| row["event"] == "observer.connected").unwrap()
        ["details"]["observerId"]
        .as_str()
        .unwrap();
    aliases.insert(id.into(), "observer:one".into());
    host.finish();
    endpoint.until("player.disconnected", 2);
    endpoint.until("observer.disconnected", 1);
    endpoint.until("session.closed", 1);
    let mut actual: Vec<_> = endpoint
        .rows
        .iter()
        .filter(|row| {
            matches!(
                row["event"].as_str(),
                Some("player.disconnected" | "observer.disconnected" | "session.closed")
            )
        })
        .map(|row| {
            json!({"component":row["component"],"event":row["event"],"message":row["message"],
                "details":normalize(row["details"].clone(),"",&aliases)})
        })
        .collect();
    let mut expected = fixture["shutdown"].as_array().unwrap().clone();
    // Source closes private HTTP owners concurrently with WebSocket release. Their
    // callback interleaving is not an order contract; every emitted field is exact.
    let key = |row: &Value| {
        (
            row["event"].as_str().unwrap().to_owned(),
            row["details"]["displayName"].as_str().unwrap_or("").to_owned(),
        )
    };
    actual.sort_by_key(key);
    expected.sort_by_key(key);
    let actual: LuaValue = serde_json::from_value(json!(actual)).unwrap();
    let expected: LuaValue = serde_json::from_value(json!(expected)).unwrap();
    assert_eq!(actual, expected);
    drop(observer);
    drop(clients);
}
