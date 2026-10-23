//! Authenticated Website supervisor routes. Incoming JSON is narrowed into the exact
//! request records; identity privileges never come from a public WebSocket hello.

use std::sync::mpsc::SyncSender;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde_json::{Value, json};
use solomon_protocol::ServerMessage;
use solomon_protocol::hosted::{GAME_SESSION_PATH_PREFIX, HostedServerMessage};
use solomon_sim::modding::LuaValue;
use solomon_sim::social::{GameSessionKind, PartyJoinRequester, PartyVisibility};

use super::preparation::AdmissionPurpose;
use super::{
    CollegeRecord, DeploymentRestart, HostTicket, HostedSupervisor, JOIN_INTENT_TIMEOUT_MS,
    JoinIntent, PartyLocator, SupervisorJoinRequest,
};
use crate::host::authority::send_control;
use crate::host::hosted::{
    ExternalPartyJoinRequestStatus, HostedAdmission, HostedGameHost, HostedObserverTarget,
    HostedPartyTarget, PartyStatus, PlayerSaveRequest,
};
use crate::host::signing::{credentials_equal, random_token};
use crate::transport::{HostedHttpRequest, HostedHttpResponse, NativeServer};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AdmissionRequest {
    pub content: LuaValue,
    #[serde(default)]
    pub developer_access: bool,
    pub leaderboard_user_id: Option<f64>,
}
#[derive(Deserialize)]
struct CodeRequest {
    code: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListingRequest {
    listing_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Requester {
    account_username: Option<String>,
    display_name: String,
    requester_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinRequest {
    listing_id: String,
    requester: Requester,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinAdmission {
    #[serde(flatten)]
    admission: AdmissionRequest,
    intent_id: String,
    active_mods: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObserverRequest {
    match_id: String,
    observer: ObserverIdentity,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObserverIdentity {
    user_id: u32,
    username: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestartRequest {
    target_revision: String,
}

impl HostedSupervisor {
    pub(super) fn http(
        &mut self,
        request: HostedHttpRequest,
        server: &NativeServer,
        now_ms: f64,
        respond: SyncSender<HostedHttpResponse>,
    ) -> Result<Option<HostedHttpResponse>, String> {
        if request.method == "GET" && request.path == "/health" {
            return Ok(Some(self.health()));
        }
        if !request.path.starts_with("/admin/") {
            return Ok(Some(HostedHttpResponse::empty(404)));
        }
        let expected = format!("Bearer {}", self.config.admin_secret);
        if request.authorization.as_ref().is_none_or(|value| !credentials_equal(value, &expected)) {
            return Ok(Some(error(401, "Unauthorized.")));
        }
        if request.method == "POST" && request.path == "/admin/deployments/restart" {
            if self.deployment.is_some() {
                return Ok(Some(error(409, "A deployment restart is already in progress.")));
            }
            let Ok(body) = decode::<RestartRequest>(&request.body) else {
                return Ok(Some(error(500, "The game update could not prepare active players.")));
            };
            let target_revision = body.target_revision.trim().to_lowercase();
            if !valid_revision(&target_revision) {
                return Ok(Some(error(500, "The game update could not prepare active players.")));
            }
            let Some(deployment_id) = request
                .deployment_id
                .filter(|id| id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit()))
            else {
                return Ok(Some(error(
                    400,
                    "The game update requires a valid publication identity.",
                )));
            };
            let deployment_id = deployment_id.to_lowercase();
            server.begin_hosted_deployment();
            let mut checkpoints = std::collections::BTreeMap::new();
            for college in &mut self.colleges {
                college.tickets.clear();
                // Disposable previews close with the supervisor, but never enter
                // its save barrier or contribute a restartable wizard count.
                if college.host.editor_test {
                    continue;
                }
                let peers: Vec<_> = college.host.connections.keys().copied().collect();
                for peer_id in peers {
                    let mut connection = college.host.connections.remove(&peer_id).unwrap();
                    connection.input.neutralize();
                    if let Some(id) = connection
                        .player_id
                        .clone()
                        .filter(|_| !college.host.observers.contains_key(&peer_id))
                    {
                        let sequence = college.host.publish_player_save(
                            &mut connection,
                            &id,
                            PlayerSaveRequest {
                                target_revision: Some(&target_revision),
                                force: true,
                                include_terminal_profile: true,
                            },
                        )?;
                        send_control(
                            &connection.peer,
                            ServerMessage::Hosted {
                                message: HostedServerMessage::DeploymentRestart {
                                    checkpoint_sequence: sequence,
                                    target_revision: target_revision.clone(),
                                    deployment_id: deployment_id.clone(),
                                },
                            },
                        )?;
                        checkpoints.insert(peer_id, sequence);
                    }
                    college.host.connections.insert(peer_id, connection);
                }
            }
            self.deployment = Some(DeploymentRestart {
                target_revision,
                deployment_id,
                completed: false,
                closing: false,
                deadline_ms: now_ms + self.config.deployment_save_timeout.as_secs_f64() * 1_000.0,
                pending: checkpoints.keys().copied().collect(),
                checkpoints,
                acknowledged: std::collections::BTreeSet::new(),
                respond,
            });
            return Ok(None);
        }
        if self.deployment.is_some() && request.method == "POST" {
            return Ok(Some(retry(error(503, "The game is updating."))));
        }
        if request.method == "POST" && request.path == "/admin/rejoin" {
            return self.prepare_rejoin_request(&request.body, respond);
        }
        let response = match (request.method.as_str(), request.path.as_str()) {
            ("POST", "/admin/editor-test") => {
                if request.body.len() > 12 * 1024 * 1024 {
                    error(400, crate::host::editor_test::EDITOR_TEST_REQUEST_ERROR)
                } else {
                    match decode::<LuaValue>(&request.body) {
                        Ok(value) => {
                            self.prepare_editor_test(value, respond)?;
                            return Ok(None);
                        }
                        Err(_) => error(400, crate::host::editor_test::EDITOR_TEST_REQUEST_ERROR),
                    }
                }
            }
            ("GET", "/admin/presence") => self.presence(),
            ("GET", "/admin/matches") => self.matches(),
            ("POST", "/admin/observers") => match decode::<ObserverRequest>(&request.body) {
                Ok(body) => self.admit_observer_request(body, now_ms)?,
                Err(_) => error(400, "A valid observer request is required."),
            },
            ("GET", "/admin/hub/parties") => json(
                200,
                json!({"items":self.colleges.iter().flat_map(|college|college.host.public_parties()).collect::<Vec<_>>()}),
            ),
            ("POST", "/admin/sessions" | "/admin/hub/tickets") => {
                match decode::<AdmissionRequest>(&request.body) {
                    Ok(admission) => {
                        let purpose = if request.path == "/admin/sessions" {
                            AdmissionPurpose::PrivateCollege
                        } else {
                            AdmissionPurpose::GlobalHub
                        };
                        self.prepare_admission(admission, purpose, respond)?;
                        return Ok(None);
                    }
                    Err(_) => error(400, "A valid game admission is required."),
                }
            }
            ("POST", "/admin/join/resolve") => match decode::<CodeRequest>(&request.body)
                .and_then(|body| normalize_join_code(&body.code))
            {
                Ok(code) => self.resolve_join(PartyLocator::Code(code), now_ms)?,
                Err(_) => error(400, "A valid party join request is required."),
            },
            ("POST", "/admin/join/public") => match decode::<ListingRequest>(&request.body)
                .and_then(|body| locator(body.listing_id))
            {
                Ok(listing) => self.resolve_join(PartyLocator::Public(listing), now_ms)?,
                Err(_) => error(400, "A valid party join request is required."),
            },
            ("POST", "/admin/join/requests") => match decode::<JoinRequest>(&request.body) {
                Ok(body) => self.request_join(body, now_ms)?,
                Err(_) => error(400, "A valid party join request is required."),
            },
            ("POST", "/admin/join/admit") => match decode::<JoinAdmission>(&request.body) {
                Ok(body) => {
                    if let Err(response) = self.join_target(&body.intent_id, now_ms) {
                        return Ok(Some(response));
                    }
                    let purpose = AdmissionPurpose::JoinParty {
                        intent_id: body.intent_id,
                        active_mods: body.active_mods,
                    };
                    self.prepare_admission(body.admission, purpose, respond)?;
                    return Ok(None);
                }
                Err(_) => error(400, "A valid party admission is required."),
            },
            ("GET", path) if path.starts_with("/admin/join/requests/") => {
                let token = path.trim_start_matches("/admin/join/requests/");
                if (32..=128).contains(&token.len()) && ascii_locator(token) {
                    self.join_request_status(token, now_ms)?
                } else {
                    HostedHttpResponse::empty(404)
                }
            }
            _ => HostedHttpResponse::empty(404),
        };
        Ok(Some(response))
    }

    fn presence(&self) -> HostedHttpResponse {
        let mut items = Vec::new();
        for college in &self.colleges {
            let host = &college.host;
            if host.editor_test {
                continue;
            }
            for player in &host.players {
                let Some(state) = host.worlds.state_for_player(&player.id) else { continue };
                let Some(participant) = state.participant(&player.id) else { continue };
                let party = host
                    .worlds
                    .parties()
                    .party_for_player(&player.id)
                    .filter(|party| party.member_player_ids.len() >= 2);
                let leader = party
                    .and_then(|party| host.player(&party.leader_player_id))
                    .and_then(|leader| host.retained_participant(&leader.id))
                    .map(|participant| participant.player.selection.name.as_str());
                items.push(json!({"accountUsername":player.profile.account_username,
                    "activity":if state.boneyard.is_some(){"boneyard"}else{"hub"},
                    "boneyardName":state.boneyard.as_ref().map(|world|&world.loaded.name),
                    "bot":false,"developer":player.developer_access,
                    "displayName":participant.player.selection.name,"partyLeader":leader,
                    "partySize":party.filter(|_|leader.is_some()).map(|party|party.member_player_ids.len()),
                    "session":session_kind(host.kind),
                    "waveNumber":state.boneyard.as_ref().map(|world|world.waves.as_ref().map_or(0,|waves|waves.state.wave_ordinal))}));
            }
            for (id, display_name, profile) in host.bot_players() {
                let Some(state) = host.worlds.state_for_player(id) else { continue };
                let party = host
                    .worlds
                    .parties()
                    .party_for_player(id)
                    .filter(|party| party.member_player_ids.len() >= 2);
                let leader = party
                    .and_then(|party| host.retained_participant(&party.leader_player_id))
                    .map(|participant| participant.player.selection.name.as_str());
                items.push(json!({"accountUsername":profile.account_username,
                    "activity":if state.boneyard.is_some(){"boneyard"}else{"hub"},
                    "boneyardName":state.boneyard.as_ref().map(|world|&world.loaded.name),
                    "bot":true,"developer":false,"displayName":display_name,
                    "partyLeader":leader,
                    "partySize":party.filter(|_|leader.is_some()).map(|party|party.member_player_ids.len()),
                    "session":session_kind(host.kind),
                    "waveNumber":state.boneyard.as_ref().map(|world|world.waves.as_ref().map_or(0,|waves|waves.state.wave_ordinal))}));
            }
        }
        json(200, json!({"items":items}))
    }

    fn matches(&self) -> HostedHttpResponse {
        let mut items = Vec::new();
        for college in &self.colleges {
            for target in college.host.observation_targets() {
                items.push(json!({"boneyardName":target.boneyard_name,
                    "id":observation_match_id(&college.host.id,&target.run_id),
                    "partyLeader":target.party_leader,"playerCount":target.player_count,
                    "players":target.players,"session":session_kind(college.host.kind),
                    "visibility":target.visibility,"waveNumber":target.wave_number}));
            }
        }
        json(200, json!({"items":items}))
    }

    fn admit_observer_request(
        &mut self,
        body: ObserverRequest,
        now_ms: f64,
    ) -> Result<HostedHttpResponse, String> {
        if !(8..=256).contains(&body.match_id.len())
            || !ascii_locator(&body.match_id)
            || body.observer.user_id == 0
            || body.observer.user_id > i32::MAX as u32
            || !valid_name(&body.observer.username)
        {
            return Ok(error(400, "A valid observer request is required."));
        }
        for college in &mut self.colleges {
            if let Some(target) = college.host.observation_targets().into_iter().find(|target| {
                observation_match_id(&college.host.id, &target.run_id) == body.match_id
            }) {
                let details = serde_json::json!({"boneyardName":target.boneyard_name,
                    "matchId":body.match_id,"observerUserId":body.observer.user_id,
                    "observerUsername":body.observer.username,"sessionId":college.host.id});
                let admission = HostedAdmission {
                    party_recovery_seed: false,
                    content: college.host.content.clone(),
                    developer_access: true,
                    leaderboard_user_id: None,
                    party_id: None,
                    reservation_id: None,
                    party_rejoin_token: None,
                    observer: Some(HostedObserverTarget {
                        run_id: target.run_id,
                        user_id: body.observer.user_id,
                        username: body.observer.username,
                    }),
                };
                let response = issue_ticket(
                    college,
                    admission,
                    now_ms,
                    self.config.unclaimed_timeout.as_secs_f64() * 1_000.0,
                )?;
                self.emit_runtime_event(
                    "observer.admission_issued",
                    "A developer observer admission was issued for an active match.",
                    details,
                );
                return Ok(response);
            }
        }
        Ok(error(404, "That match is no longer active."))
    }

    fn health(&self) -> HostedHttpResponse {
        let hub = &self.colleges[0].host;
        json(
            200,
            json!({"status":"ok","protocol":"solomon-game-v1",
            "deploymentProtocol":solomon_protocol::hosted::DEPLOYMENT_PROTOCOL_VERSION,
            "draining":self.deployment.is_some(),
            "sessions":self.colleges.len()-1+usize::from(hub.capacity_participant_count()>0),
            "privateSessions":self.colleges.len()-1,
            "privatePlayers":self.colleges.iter().skip(1).map(|college|college.host.players.len()).sum::<usize>(),
            "hubPlayers":hub.worlds.hub().participants.len(),"hubHumanPlayers":hub.players.len(),"bots":hub.bot_count(),
            "parties":hub.worlds.parties().parties.len(),
            "runs":match &hub.worlds {crate::host::hosted::HostedWorlds::Global(worlds)=>worlds.runs.len(),_=>0},
            "players":hub.capacity_participant_count()}),
        )
    }

    pub(super) fn provision(
        &mut self,
        admission: HostedAdmission,
        server: &NativeServer,
        now_ms: f64,
    ) -> Result<HostedHttpResponse, String> {
        self.provision_college(admission, None, server, now_ms)
    }

    pub(super) fn provision_editor(
        &mut self,
        admission: HostedAdmission,
        entry: solomon_assets::boneyard::ModBoneyardEntry,
        server: &NativeServer,
        now_ms: f64,
    ) -> Result<HostedHttpResponse, String> {
        self.provision_college(admission, Some(entry), server, now_ms)
    }

    fn provision_college(
        &mut self,
        admission: HostedAdmission,
        editor_test: Option<solomon_assets::boneyard::ModBoneyardEntry>,
        server: &NativeServer,
        now_ms: f64,
    ) -> Result<HostedHttpResponse, String> {
        if self.colleges.len() > self.config.maximum_sessions {
            return Ok(retry(error(503, "Game session capacity is exhausted.")));
        }
        let recovery_seed = admission.party_recovery_seed;
        let id = random_token::<24>()?;
        let path = format!("{GAME_SESSION_PATH_PREFIX}{id}");
        let maximum_players = if editor_test.is_some() { 1 } else { self.config.maximum_players };
        let mut host = HostedGameHost::new(
            id,
            if editor_test.is_some() {
                GameSessionKind::Standalone
            } else {
                GameSessionKind::PrivateCollege
            },
            maximum_players,
            admission.content.clone(),
            &self.config.asset_directory,
            self.config.admin_secret.clone(),
            self.config.revision.clone(),
        )?;
        if let Some(entry) = editor_test {
            host.configure_editor_test(entry)?;
        } else {
            host.runtime_events = self.runtime_events.as_ref().map(|publisher| publisher.sink());
            if let Some(archives) = &self.run_archives {
                host.install_run_archives(archives.sink(), now_ms);
            }
        }
        let mut college = CollegeRecord {
            host,
            path: path.clone(),
            tickets: Default::default(),
            created_at_ms: now_ms,
            claimed: false,
        };
        let mut response = issue_ticket(
            &mut college,
            admission,
            now_ms,
            self.config.unclaimed_timeout.as_secs_f64() * 1_000.0,
        )?;
        // Registration precedes publication of its one-use endpoint.
        server.register_hosted_route(path, maximum_players).map_err(|error| error.to_string())?;
        if !recovery_seed {
            let mut body: Value =
                serde_json::from_slice(&response.body).expect("the endpoint is JSON");
            body["sessionId"] = Value::String(college.host.id.clone());
            response.body = serde_json::to_vec(&body).expect("endpoint JSON is serializable");
        }
        self.colleges.push(college);
        self.emit_runtime_event(
            "session.provisioned",
            "An isolated browser game session was provisioned.",
            serde_json::json!({"kind":"private", "sessionId":self.colleges.last().unwrap().host.id,
                "sessionCount":self.colleges.len()-1}),
        );
        Ok(response)
    }

    fn resolve_join(
        &mut self,
        locator: PartyLocator,
        now_ms: f64,
    ) -> Result<HostedHttpResponse, String> {
        let Some((index, target)) = self.resolve_target(&locator) else {
            return Ok(error(404, "That party has ended or is no longer available."));
        };
        if matches!(&locator, PartyLocator::Public(_))
            && target.visibility != PartyVisibility::Public
        {
            return Ok(error(409, "That party requires a join request."));
        }
        if target.status != PartyStatus::Hub {
            return Ok(error(409, "That party is in a Boneyard. Wait for it to return."));
        }
        Ok(json(201, self.create_intent(index, target, locator, None, now_ms)?))
    }

    fn resolve_target(&self, locator: &PartyLocator) -> Option<(usize, HostedPartyTarget)> {
        let mut matches = self.colleges.iter().enumerate().filter_map(|(index, college)| {
            if college.host.editor_test {
                return None;
            }
            let target = match locator {
                PartyLocator::Code(code) => college.host.party_target_by_code(code),
                PartyLocator::Public(listing) | PartyLocator::Request(listing) => {
                    college.host.party_target_by_listing(listing)
                }
            }?;
            Some((index, target))
        });
        let first = matches.next()?;
        if matches.next().is_some() { None } else { Some(first) }
    }

    fn create_intent(
        &mut self,
        index: usize,
        target: HostedPartyTarget,
        locator: PartyLocator,
        request_token: Option<String>,
        now_ms: f64,
    ) -> Result<Value, String> {
        let intent_id = random_token::<24>()?;
        let payload = intent_payload(&intent_id, &self.colleges[index], &target);
        self.join_intents.insert(
            intent_id,
            JoinIntent {
                expires_at_ms: now_ms + JOIN_INTENT_TIMEOUT_MS,
                locator,
                party_id: target.id,
                request_token,
                session_id: self.colleges[index].host.id.clone(),
            },
        );
        Ok(payload)
    }

    fn request_join(
        &mut self,
        body: JoinRequest,
        now_ms: f64,
    ) -> Result<HostedHttpResponse, String> {
        if locator(body.listing_id.clone()).is_err()
            || !valid_name(&body.requester.display_name)
            || body.requester.account_username.as_ref().is_some_and(|name| !valid_name(name))
            || locator(body.requester.requester_id.clone()).is_err()
        {
            return Ok(error(400, "A valid party join request is required."));
        }
        let Some((index, target)) =
            self.resolve_target(&PartyLocator::Request(body.listing_id.clone()))
        else {
            return Ok(error(404, "That party has ended or is no longer available."));
        };
        if target.visibility != PartyVisibility::InviteOnly {
            return Ok(error(409, "That party does not accept join requests."));
        }
        if target.status != PartyStatus::Hub {
            return Ok(error(409, "That party is in a Boneyard. Wait for it to return."));
        }
        let token = random_token::<24>()?;
        let id = format!("request-{}", random_token::<18>()?);
        let requester = PartyJoinRequester {
            account_username: body.requester.account_username,
            display_name: body.requester.display_name,
            requester_id: body.requester.requester_id,
        };
        let result = self.colleges[index].host.create_party_join_request(
            &body.listing_id,
            id,
            token.clone(),
            requester,
            now_ms + JOIN_INTENT_TIMEOUT_MS,
            now_ms,
        );
        if let Err(reason) = result {
            return Ok(error(409, join_error(reason.id())));
        }
        self.join_requests.insert(
            token.clone(),
            SupervisorJoinRequest {
                expires_at_ms: now_ms + JOIN_INTENT_TIMEOUT_MS,
                intent_id: None,
                listing_id: body.listing_id,
                session_id: self.colleges[index].host.id.clone(),
            },
        );
        Ok(json(201, json!({"requestToken":token,"status":"pending"})))
    }

    fn join_request_status(
        &mut self,
        token: &str,
        now_ms: f64,
    ) -> Result<HostedHttpResponse, String> {
        let Some(tracked) =
            self.join_requests.get(token).filter(|tracked| tracked.expires_at_ms > now_ms)
        else {
            self.join_requests.remove(token);
            return Ok(error(404, "That join request has expired."));
        };
        let Some(index) =
            self.colleges.iter().position(|college| college.host.id == tracked.session_id)
        else {
            return Ok(error(404, "That join request has expired."));
        };
        let Some(request) = self.colleges[index]
            .host
            .external_join_requests
            .iter()
            .find(|request| request.token == token)
        else {
            return Ok(error(404, "That join request has expired."));
        };
        match request.status {
            ExternalPartyJoinRequestStatus::Pending => {
                return Ok(json(200, json!({"status":"pending"})));
            }
            ExternalPartyJoinRequestStatus::Denied => {
                return Ok(json(200, json!({"status":"denied"})));
            }
            ExternalPartyJoinRequestStatus::Accepted => {}
        }
        let listing = tracked.listing_id.clone();
        let Some(target) = self.colleges[index]
            .host
            .party_target_by_listing(&listing)
            .filter(|target| target.id == request.party_id && target.status == PartyStatus::Hub)
        else {
            return Ok(error(409, "That party is no longer available."));
        };
        let existing = tracked
            .intent_id
            .as_ref()
            .filter(|id| {
                self.join_intents.get(*id).is_some_and(|intent| intent.expires_at_ms > now_ms)
            })
            .cloned();
        let mut payload = if let Some(id) = existing {
            intent_payload(&id, &self.colleges[index], &target)
        } else {
            let payload = self.create_intent(
                index,
                target,
                PartyLocator::Request(listing),
                Some(token.into()),
                now_ms,
            )?;
            self.join_requests.get_mut(token).unwrap().intent_id =
                payload["intentId"].as_str().map(str::to_owned);
            payload
        };
        payload["status"] = Value::String("accepted".into());
        Ok(json(200, payload))
    }

    fn join_target(
        &mut self,
        intent_id: &str,
        now_ms: f64,
    ) -> Result<(usize, HostedPartyTarget, Option<String>), HostedHttpResponse> {
        let Some(intent) =
            self.join_intents.get(intent_id).filter(|intent| intent.expires_at_ms > now_ms)
        else {
            return Err(error(404, "That party join has expired."));
        };
        let Some(index) =
            self.colleges.iter().position(|college| college.host.id == intent.session_id)
        else {
            return Err(error(404, "That party join has expired."));
        };
        let target = match &intent.locator {
            PartyLocator::Code(code) => self.colleges[index].host.party_target_by_code(code),
            PartyLocator::Public(listing) | PartyLocator::Request(listing) => {
                self.colleges[index].host.party_target_by_listing(listing)
            }
        };
        let Some(target) = target
            .filter(|target| target.id == intent.party_id && target.status == PartyStatus::Hub)
        else {
            self.join_intents.remove(intent_id);
            return Err(error(409, "That party is no longer available."));
        };
        if matches!(intent.locator, PartyLocator::Public(_))
            && target.visibility != PartyVisibility::Public
        {
            self.join_intents.remove(intent_id);
            return Err(error(409, "That party now requires a join request."));
        }
        Ok((index, target, intent.request_token.clone()))
    }

    pub(super) fn admit_prepared_join(
        &mut self,
        intent_id: &str,
        active_mods: bool,
        requested: HostedAdmission,
        now_ms: f64,
    ) -> Result<HostedHttpResponse, String> {
        // Package execution can yield while the leader moves, changes visibility or
        // revokes the code. Re-resolve the actual party before reserving a place.
        let (index, target, request_token) = match self.join_target(intent_id, now_ms) {
            Ok(target) => target,
            Err(response) => return Ok(response),
        };
        let college = &mut self.colleges[index];
        if college.host.kind == GameSessionKind::GlobalHub && active_mods {
            return Ok(error(409, "Disable active mods before joining the global Hub."));
        }
        if college.host.capacity_participant_count() + player_ticket_count(college)
            >= self.config.maximum_players
        {
            return Ok(error(409, "That College is full."));
        }
        let reservation_id = random_token::<24>()?;
        if let Err(reason) = college.host.reserve_party_join(
            &target.id,
            reservation_id.clone(),
            now_ms + self.config.unclaimed_timeout.as_secs_f64() * 1_000.0,
            now_ms,
        ) {
            return Ok(error(409, join_error(reason.id())));
        }
        let content = if college.host.kind == GameSessionKind::GlobalHub {
            requested.content
        } else {
            college.host.content.clone()
        };
        let admission = HostedAdmission {
            party_recovery_seed: false,
            content,
            developer_access: requested.developer_access,
            leaderboard_user_id: requested.leaderboard_user_id,
            party_id: Some(target.id),
            reservation_id: Some(reservation_id.clone()),
            party_rejoin_token: None,
            observer: None,
        };
        let result = issue_ticket(
            college,
            admission,
            now_ms,
            self.config.unclaimed_timeout.as_secs_f64() * 1_000.0,
        );
        if result.is_err() {
            college.host.cancel_party_reservation(&reservation_id)
        }
        let response = result?;
        self.join_intents.remove(intent_id);
        if let Some(token) = request_token {
            self.join_requests.remove(&token);
        }
        Ok(response)
    }
}

pub(super) fn issue_ticket(
    college: &mut CollegeRecord,
    admission: HostedAdmission,
    now_ms: f64,
    timeout_ms: f64,
) -> Result<HostedHttpResponse, String> {
    let credential = random_token::<32>()?;
    college
        .tickets
        .insert(credential.clone(), HostTicket { admission, expires_at_ms: now_ms + timeout_ms });
    Ok(json(
        201,
        json!({"credential":credential,"path":college.path,"protocol":"solomon-game-v1",
        "sessionKind":if college.host.editor_test {"private-college"} else {session_kind(college.host.kind)}}),
    ))
}
pub(super) fn player_ticket_count(college: &CollegeRecord) -> usize {
    college.tickets.values().filter(|ticket| ticket.admission.observer.is_none()).count()
}
fn intent_payload(id: &str, college: &CollegeRecord, target: &HostedPartyTarget) -> Value {
    json!({"intentId":id,"target":{"cheatsEnabled":target.cheats_enabled,"content":target.content,
        "kind":session_kind(college.host.kind),"leader":target.leader,"memberCount":target.member_count,
        "status":target.status,"visibility":visibility(target.visibility)}})
}
pub(super) fn session_kind(kind: GameSessionKind) -> &'static str {
    match kind {
        GameSessionKind::GlobalHub => "global-hub",
        GameSessionKind::PrivateCollege => "private-college",
        GameSessionKind::Standalone => "standalone",
    }
}
pub(in crate::host) fn visibility(value: PartyVisibility) -> &'static str {
    match value {
        PartyVisibility::Public => "public",
        PartyVisibility::InviteOnly => "invite-only",
        PartyVisibility::Private => "private",
    }
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}
fn normalize_join_code(value: &str) -> Result<String, String> {
    if value.encode_utf16().count() > 128 {
        return Err("party code is invalid".into());
    }
    let mut chars: Vec<_> = value
        .to_uppercase()
        .chars()
        .filter(|character| "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(*character))
        .collect();
    if chars.len() < 8 {
        return Err("party code is invalid".into());
    }
    chars.drain(..chars.len() - 8);
    Ok(format!(
        "{}-{}",
        chars[..4].iter().collect::<String>(),
        chars[4..].iter().collect::<String>()
    ))
}
fn locator(value: String) -> Result<String, String> {
    if (8..=128).contains(&value.len()) && ascii_locator(&value) {
        Ok(value)
    } else {
        Err("party locator is invalid".into())
    }
}
fn ascii_locator(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
fn valid_name(value: &str) -> bool {
    (1..=64).contains(&value.encode_utf16().count())
}
fn valid_revision(value: &str) -> bool {
    value.len() == 40
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn join_error(reason: &str) -> &'static str {
    match reason {
        "party-full" => "That party is full.",
        "not-in-hub" => "That party is in a Boneyard. Wait for it to return.",
        "already-requested" => "A join request is already pending for that party.",
        "party-private" => "That party is private.",
        _ => "That party is no longer available.",
    }
}
pub(super) fn json(status: u16, body: Value) -> HostedHttpResponse {
    HostedHttpResponse {
        status,
        headers: vec![
            ("cache-control".into(), "no-store".into()),
            ("content-type".into(), "application/json; charset=utf-8".into()),
        ],
        body: serde_json::to_vec(&body).expect("supervisor response contains JSON values"),
    }
}
pub(super) fn error(status: u16, reason: &str) -> HostedHttpResponse {
    json(status, json!({"error":reason}))
}
pub(super) fn retry(mut response: HostedHttpResponse) -> HostedHttpResponse {
    response.headers.push(("retry-after".into(), "5".into()));
    response
}

fn observation_match_id(session_id: &str, run_id: &str) -> String {
    URL_SAFE_NO_PAD.encode(format!("{session_id}\0{run_id}").as_bytes())
}
