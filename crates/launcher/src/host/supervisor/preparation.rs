//! `materializeGameAdmission` from `host/game-session-supervisor.ts`.
//! Package bytes and temporary Lua definition VMs are owned by a worker. Only the
//! accepted plain content crosses back to the thread that owns rooms and tickets.

use std::sync::Arc;
use std::sync::mpsc::SyncSender;
use std::thread::{self, JoinHandle};

use solomon_assets::boneyard::ModBoneyardEntry;
use solomon_modding::package::{
    compile_web_session_content_definitions, materialize_web_session_content_value,
};
use solomon_sim::modding::LuaValue;

use super::HostedSupervisor;
use super::http::{AdmissionRequest, error, issue_ticket, player_ticket_count, retry};
use crate::host::hosted::HostedAdmission;
use crate::transport::{HostedHttpResponse, NativeServer};

pub(super) enum AdmissionPurpose {
    PrivateCollege,
    EditorTest,
    GlobalHub,
    JoinParty { intent_id: String, active_mods: bool },
    Rejoin { token: String, save: String },
}

pub(super) struct AdmissionPreparation {
    purpose: AdmissionPurpose,
    respond: SyncSender<HostedHttpResponse>,
    pub worker: JoinHandle<Result<PreparedAdmission, String>>,
}

pub(super) struct PreparedAdmission {
    admission: HostedAdmission,
    editor_test: Option<ModBoneyardEntry>,
}

impl HostedSupervisor {
    pub(super) fn prepare_admission(
        &mut self,
        request: AdmissionRequest,
        purpose: AdmissionPurpose,
        respond: SyncSender<HostedHttpResponse>,
    ) -> Result<(), String> {
        let worker = thread::Builder::new()
            .name("game-content-admission".into())
            .spawn(move || {
                materialize_admission(request)
                    .map(|admission| PreparedAdmission { admission, editor_test: None })
            })
            .map_err(|error| format!("Could not start content admission: {error}"))?;
        self.preparations.push(AdmissionPreparation { purpose, respond, worker });
        Ok(())
    }

    pub(super) fn prepare_editor_test(
        &mut self,
        value: LuaValue,
        respond: SyncSender<HostedHttpResponse>,
    ) -> Result<(), String> {
        // Binary projection is bounded but can be expensive. It shares the existing
        // admission worker boundary, so previews never hold up socket ACKs or ticks.
        let worker = thread::Builder::new()
            .name("game-editor-admission".into())
            .spawn(move || {
                let editor_test =
                    crate::host::editor_test::materialize_editor_test_boneyard(&value)?;
                Ok(PreparedAdmission {
                    admission: HostedAdmission {
                        content: Arc::new(super::stock_content("0".repeat(64))),
                        developer_access: false,
                        leaderboard_user_id: None,
                        party_id: None,
                        reservation_id: None,
                        party_rejoin_token: None,
                        party_recovery_seed: false,
                        observer: None,
                    },
                    editor_test: Some(editor_test),
                })
            })
            .map_err(|error| format!("Could not start editor admission: {error}"))?;
        self.preparations.push(AdmissionPreparation {
            purpose: AdmissionPurpose::EditorTest,
            respond,
            worker,
        });
        Ok(())
    }

    pub(super) fn complete_admissions(
        &mut self,
        server: &NativeServer,
        now_ms: f64,
    ) -> Result<(), String> {
        let mut index = 0;
        while index < self.preparations.len() {
            if !self.preparations[index].worker.is_finished() {
                index += 1;
                continue;
            }
            let preparation = self.preparations.remove(index);
            let result = preparation
                .worker
                .join()
                .map_err(|_| "The content admission worker stopped unexpectedly".to_owned())
                .and_then(|content| content);
            let invalid_reason = match &preparation.purpose {
                AdmissionPurpose::EditorTest => crate::host::editor_test::EDITOR_TEST_REQUEST_ERROR,
                AdmissionPurpose::JoinParty { .. } => "A valid party admission is required.",
                AdmissionPurpose::Rejoin { .. } => "A valid active-party rejoin is required.",
                _ => "A valid game admission is required.",
            };
            let response = if self.deployment.is_some() {
                Some(retry(error(503, "The game is updating.")))
            } else {
                match result {
                    Err(_) => Some(error(400, invalid_reason)),
                    Ok(PreparedAdmission { admission, editor_test }) => match preparation.purpose {
                        AdmissionPurpose::EditorTest => Some(
                            match self.provision_editor(
                                admission,
                                editor_test
                                    .expect("editor admission projects its actual native scene"),
                                server,
                                now_ms,
                            ) {
                                Ok(response) => response,
                                Err(reason) => {
                                    eprintln!("An editor preview could not be started: {reason}");
                                    retry(error(503, "A game session could not be started."))
                                }
                            },
                        ),
                        AdmissionPurpose::PrivateCollege => {
                            Some(match self.provision(admission, server, now_ms) {
                                Ok(response) => response,
                                Err(reason) => {
                                    eprintln!("A private College could not be started: {reason}");
                                    retry(error(503, "A game session could not be started."))
                                }
                            })
                        }
                        AdmissionPurpose::GlobalHub => {
                            let college = &mut self.colleges[0];
                            if college.host.capacity_participant_count()
                                + player_ticket_count(college)
                                >= self.config.maximum_players
                            {
                                Some(retry(error(503, "The shared Hub is full.")))
                            } else {
                                Some(issue_ticket(
                                    college,
                                    admission,
                                    now_ms,
                                    self.config.unclaimed_timeout.as_secs_f64() * 1_000.0,
                                )?)
                            }
                        }
                        AdmissionPurpose::JoinParty { intent_id, active_mods } => Some(
                            self.admit_prepared_join(&intent_id, active_mods, admission, now_ms)?,
                        ),
                        AdmissionPurpose::Rejoin { token, save } => self.admit_prepared_rejoin(
                            admission,
                            token,
                            &save,
                            server,
                            now_ms,
                            preparation.respond.clone(),
                        )?,
                    },
                }
            };
            // A disconnected HTTP caller does not undo an already issued ticket.
            // The source retains that ticket until its ordinary unclaimed timeout.
            if let Some(response) = response {
                let _ = preparation.respond.send(response);
            }
        }
        Ok(())
    }
}

fn materialize_admission(request: AdmissionRequest) -> Result<HostedAdmission, String> {
    if request.leaderboard_user_id.is_some_and(|id| {
        !id.is_finite() || id.fract() != 0.0 || !(1.0..=f64::from(i32::MAX)).contains(&id)
    }) {
        return Err("leaderboard user id is invalid".into());
    }
    // Decode the actual JSON value through the source validator. In particular,
    // integer-valued JSON numbers such as 1.0 are valid, while missing nullable
    // entryScript fields and unknown package fields are rejected there.
    let content = materialize_web_session_content_value(&request.content)?;
    let content = compile_web_session_content_definitions(content)?;
    Ok(HostedAdmission {
        content: Arc::new(content),
        developer_access: request.developer_access,
        leaderboard_user_id: request.leaderboard_user_id.map(|id| id as u32),
        party_id: None,
        reservation_id: None,
        party_rejoin_token: None,
        party_recovery_seed: false,
        observer: None,
    })
}
