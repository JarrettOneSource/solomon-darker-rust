//! `game-session-supervisor.ts::handlePartyRejoin`: admitted packages and signed
//! documents locate a retained run, or seed exactly one replacement after deployment.
//! Waiting for the first real Hello never blocks the authority or WebSocket framing.

use std::sync::mpsc::SyncSender;

use serde::Deserialize;
use solomon_protocol::MAX_SAVE_BYTES;
use solomon_sim::social::GameSessionKind;

use super::HostedSupervisor;
use super::http::{AdmissionRequest, error, issue_ticket, player_ticket_count, retry};
use super::preparation::AdmissionPurpose;
use crate::host::hosted::{HostedAdmission, HostedRejoinRejection, HostedRejoinStatus};
use crate::host::recovery::{
    MAX_PARTY_RECOVERY_CLAIM_LENGTH, PartyRecoveryClaim, verify_party_recovery_claim,
};
use crate::host::signing::random_token;
use crate::transport::{HostedHttpResponse, NativeServer};

pub(super) struct PartyRecoverySession {
    pub session_id: String,
    pub seed_expires_at_ms: f64,
}

pub(super) struct PendingPartyRejoin {
    admission: HostedAdmission,
    claim: PartyRecoveryClaim,
    token: String,
    session_id: String,
    deadline_ms: f64,
    respond: SyncSender<HostedHttpResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RejoinRequest {
    #[serde(flatten)]
    admission: AdmissionRequest,
    token: String,
    save: String,
}

impl HostedSupervisor {
    pub(super) fn prepare_rejoin_request(
        &mut self,
        body: &[u8],
        respond: SyncSender<HostedHttpResponse>,
    ) -> Result<Option<HostedHttpResponse>, String> {
        let request: RejoinRequest = match serde_json::from_slice(body) {
            Ok(request) => request,
            Err(_) => return Ok(Some(invalid_rejoin())),
        };
        if !valid_rejoin_token(&request.token) || request.save.len() > MAX_SAVE_BYTES {
            return Ok(Some(invalid_rejoin()));
        }
        self.prepare_admission(
            request.admission,
            AdmissionPurpose::Rejoin { token: request.token, save: request.save },
            respond,
        )?;
        Ok(None)
    }

    pub(super) fn admit_prepared_rejoin(
        &mut self,
        admission: HostedAdmission,
        token: String,
        document: &str,
        server: &NativeServer,
        now_ms: f64,
        respond: SyncSender<HostedHttpResponse>,
    ) -> Result<Option<HostedHttpResponse>, String> {
        let claim = verify_party_recovery_claim(&self.config.admin_secret, &token, document);
        if claim.is_none() && (token.len() == 43 || token.starts_with("sdrpr1.")) {
            return Ok(Some(ended_rejoin()));
        }
        let Some(claim) = claim.filter(|claim| {
            claim.session_kind != GameSessionKind::Standalone
                && claim.content_manifest_sha256 == admission.content.manifest.manifest_sha256
                && claim.leaderboard_user_id == admission.leaderboard_user_id
        }) else {
            return Ok(Some(invalid_rejoin()));
        };
        let resolved: Vec<_> = self
            .colleges
            .iter_mut()
            .enumerate()
            .filter_map(|(index, college)| {
                college.host.party_rejoin_target(&token, now_ms).map(|target| (index, target))
            })
            .collect();
        if resolved.len() > 1 {
            return Ok(Some(invalid_rejoin()));
        }
        if let Some((index, target)) = resolved.into_iter().next() {
            return self
                .reserve_prepared_rejoin(index, target.status, admission, token, claim, now_ms)
                .map(Some);
        }
        if let Some(recovery) = self.recovery_sessions.get(&claim.recovery_id) {
            if recovery.seed_expires_at_ms.is_infinite() || recovery.seed_expires_at_ms <= now_ms {
                self.recovery_sessions.remove(&claim.recovery_id);
                self.retired_recoveries.insert(claim.recovery_id.clone());
            } else {
                self.pending_rejoins.push(PendingPartyRejoin {
                    admission,
                    claim,
                    token,
                    session_id: recovery.session_id.clone(),
                    deadline_ms: recovery.seed_expires_at_ms,
                    respond,
                });
                return Ok(None);
            }
        }
        if claim.target_revision.as_deref() != Some(self.config.revision.as_str())
            || self.retired_recoveries.contains(&claim.recovery_id)
        {
            return Ok(Some(ended_rejoin()));
        }
        let mut admission = admission;
        admission.party_recovery_seed = true;
        admission.party_rejoin_token = Some(token);
        let deadline_ms = now_ms + self.config.unclaimed_timeout.as_secs_f64() * 1_000.0;
        let (session_id, response) = if claim.session_kind == GameSessionKind::GlobalHub {
            let college = &mut self.colleges[0];
            let response = issue_ticket(
                college,
                admission,
                now_ms,
                self.config.unclaimed_timeout.as_secs_f64() * 1_000.0,
            )?;
            (college.host.id.clone(), response)
        } else {
            let response = match self.provision(admission, server, now_ms) {
                Ok(response) => response,
                Err(reason) => {
                    eprintln!("A party recovery College could not be started: {reason}");
                    return Ok(Some(invalid_rejoin()));
                }
            };
            if response.status != 201 {
                return Ok(Some(response));
            }
            (self.colleges.last().unwrap().host.id.clone(), response)
        };
        self.recovery_sessions.insert(
            claim.recovery_id,
            PartyRecoverySession { session_id, seed_expires_at_ms: deadline_ms },
        );
        Ok(Some(response))
    }

    fn reserve_prepared_rejoin(
        &mut self,
        index: usize,
        status: HostedRejoinStatus,
        mut admission: HostedAdmission,
        token: String,
        claim: PartyRecoveryClaim,
        now_ms: f64,
    ) -> Result<HostedHttpResponse, String> {
        let college = &mut self.colleges[index];
        self.recovery_sessions.insert(
            claim.recovery_id,
            PartyRecoverySession {
                session_id: college.host.id.clone(),
                seed_expires_at_ms: f64::INFINITY,
            },
        );
        if status != HostedRejoinStatus::Detached {
            return Ok(error(409, "That active-party rejoin is already being claimed."));
        }
        let target = college
            .host
            .party_rejoin_target(&token, now_ms)
            .expect("the resolved target still belongs to this synchronous authority turn");
        if target.content.manifest.manifest_sha256 != admission.content.manifest.manifest_sha256 {
            return Ok(error(409, "The saved content no longer matches that party run."));
        }
        college.tickets.retain(|_, ticket| ticket.expires_at_ms > now_ms);
        if college.host.capacity_participant_count().saturating_sub(1)
            + player_ticket_count(college)
            >= self.config.maximum_players
        {
            return Ok(error(409, "That College is full."));
        }
        let reservation_id = random_token::<24>()?;
        let expiry_ms = now_ms + self.config.unclaimed_timeout.as_secs_f64() * 1_000.0;
        if let Err(rejection) =
            college.host.reserve_party_rejoin(&token, reservation_id.clone(), expiry_ms, now_ms)
        {
            return Ok(error(
                409,
                match rejection {
                    HostedRejoinRejection::PlayerConnected => "That wizard is still connected.",
                    HostedRejoinRejection::AlreadyReserved => {
                        "That active-party rejoin is already being claimed."
                    }
                    HostedRejoinRejection::RunUnavailable => "That active party run has ended.",
                },
            ));
        }
        admission.party_rejoin_token = Some(token);
        admission.reservation_id = Some(reservation_id.clone());
        let response = issue_ticket(
            college,
            admission,
            now_ms,
            self.config.unclaimed_timeout.as_secs_f64() * 1_000.0,
        );
        if response.is_err() {
            college.host.cancel_party_reservation(&reservation_id);
        }
        response
    }

    pub(super) fn complete_party_rejoins(&mut self, now_ms: f64) -> Result<(), String> {
        for pending in std::mem::take(&mut self.pending_rejoins) {
            let response = if self.deployment.is_some() {
                Some(retry(error(503, "The game is updating.")))
            } else if let Some(index) =
                self.colleges.iter().position(|college| college.host.id == pending.session_id)
            {
                if let Some(target) =
                    self.colleges[index].host.party_rejoin_target(&pending.token, now_ms)
                {
                    Some(self.reserve_prepared_rejoin(
                        index,
                        target.status,
                        pending.admission,
                        pending.token,
                        pending.claim,
                        now_ms,
                    )?)
                } else if now_ms >= pending.deadline_ms {
                    self.recovery_sessions.remove(&pending.claim.recovery_id);
                    self.retired_recoveries.insert(pending.claim.recovery_id);
                    Some(ended_rejoin())
                } else {
                    self.pending_rejoins.push(pending);
                    continue;
                }
            } else {
                self.recovery_sessions.remove(&pending.claim.recovery_id);
                self.retired_recoveries.insert(pending.claim.recovery_id);
                Some(ended_rejoin())
            };
            if let Some(response) = response {
                let _ = pending.respond.send(response);
            }
        }
        Ok(())
    }
}

fn valid_rejoin_token(token: &str) -> bool {
    let base64url = |value: &str| {
        !value.is_empty()
            && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    };
    if token.len() > MAX_PARTY_RECOVERY_CLAIM_LENGTH {
        return false;
    }
    if token.len() == 43 && base64url(token) {
        return true;
    }
    let mut parts = token.split('.');
    matches!((parts.next(),parts.next(),parts.next(),parts.next()),
        (Some("sdrpr1"|"sdrpr2"),Some(payload),Some(signature),None)
        if base64url(payload) && signature.len()==43 && base64url(signature))
}

fn invalid_rejoin() -> HostedHttpResponse {
    error(400, "A valid active-party rejoin is required.")
}
fn ended_rejoin() -> HostedHttpResponse {
    error(404, "That active party run has ended.")
}
