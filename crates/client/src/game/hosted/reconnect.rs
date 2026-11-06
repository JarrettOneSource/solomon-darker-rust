//! Hosted transport recovery never routes through New Game or saved-run admission.
use futures_util::future::{AbortHandle, AbortRegistration};
use solomon_protocol::ResumeCredentials;

use super::*;

/// One series of reconnect attempts after an unexpected disconnect: its deadline, attempt
/// count, resume credentials and pending request.
pub(super) struct ReconnectEpisode {
    deadline_ms: f64,
    attempts: u8,
    request_id: String,
    credentials: ResumeCredentials,
    account_id: Option<u64>,
    original_reason: String,
    abort: Option<AbortHandle>,
    retry_at_ms: Option<f64>,
}

/// One reconnect attempt to send: the endpoint, resume credentials, request ID and abort
/// registration.
pub(super) struct ReconnectRequest {
    pub endpoint: String,
    pub credentials: ResumeCredentials,
    pub request_id: String,
    pub registration: AbortRegistration,
}

impl ReconnectEpisode {
    pub(super) fn cancel(&mut self) {
        if let Some(abort) = self.abort.take() {
            abort.abort();
        }
    }
}

impl Game {
    pub(in crate::game) fn try_hosted_reconnect(
        &mut self,
        code: u16,
        reason: &str,
        now: f64,
    ) -> Result<bool, String> {
        if !matches!(code, 1001 | 1006) || self.is_hosted_observer() {
            return Ok(false);
        }
        #[cfg(target_arch = "wasm32")]
        if self.is_editor_test() {
            return Ok(false);
        }
        let Some(hosted) = &mut self.hosted else { return Ok(false) };
        if hosted.desktop_endpoint.is_some()
            || hosted.deployment_update.restarting()
            || hosted.endpoint.is_none()
            || hosted.welcome.is_none()
            || self.native.as_ref().is_none_or(|native| !native.can_reconnect())
        {
            return Ok(false);
        }
        if hosted.endpoint_account_id != hosted.account.as_ref().map(|account| account.id) {
            self.native_disconnected("The game account changed. Return to the menu to continue.")?;
            return Ok(true);
        }
        self.social.cancel_player_card_request();
        let credentials = self.native.as_mut().unwrap().begin_reconnect()?;
        if self.world_loading {
            // Its pending frame belonged to the retired transport. A fresh Welcome
            // will queue the actual run again; stale async preparation must not apply.
            self.world_generation += 1;
            self.pending_world = None;
            self.world_loading = false;
        }
        if let Some(episode) = &mut hosted.reconnect {
            episode.cancel();
            // A prior Hello may have consumed its ticket; this attempt replaces its reservation.
            episode.request_id = crate::platform::random_uuid()?.replace('-', "");
        } else {
            hosted.reconnect = Some(ReconnectEpisode {
                deadline_ms: now + solomon_protocol::hosted::GAME_RECONNECT_LEASE_MS as f64,
                attempts: 0,
                request_id: crate::platform::random_uuid()?.replace('-', ""),
                credentials,
                account_id: hosted.account.as_ref().map(|account| account.id),
                original_reason: format!("WebSocket {code}: {reason}"),
                abort: None,
                retry_at_ms: None,
            });
        }
        if self.hosted.as_ref().unwrap().reconnect.as_ref().unwrap().attempts > 0 {
            self.hosted.as_mut().unwrap().reconnect.as_mut().unwrap().retry_at_ms =
                Some(now + 1_000.0);
        } else {
            self.queue_hosted_reconnect(now)?;
        }
        Ok(true)
    }

    fn queue_hosted_reconnect(&mut self, now: f64) -> Result<(), String> {
        let hosted = self.hosted.as_mut().unwrap();
        let Some(episode) = &mut hosted.reconnect else { return Ok(()) };
        if now >= episode.deadline_ms
            || episode.attempts >= solomon_protocol::hosted::GAME_RECONNECT_MAX_ATTEMPTS
            || hosted.account.as_ref().map(|account| account.id) != episode.account_id
        {
            let reason = format!(
                "The live game connection could not be restored. {}",
                episode.original_reason
            );
            return self.native_disconnected(&reason);
        }
        episode.attempts += 1;
        episode.retry_at_ms = None;
        let (abort, registration) = AbortHandle::new_pair();
        episode.abort = Some(abort);
        hosted.pending.push(HostedRequest::Reconnect(reconnect::ReconnectRequest {
            endpoint: hosted.endpoint.clone().unwrap(),
            credentials: episode.credentials.clone(),
            request_id: episode.request_id.clone(),
            registration,
        }));
        set_status("Reconnecting…");
        Ok(())
    }

    pub(in crate::game) fn poll_hosted_reconnect(&mut self, now: f64) -> Result<(), String> {
        let failed = self
            .hosted
            .as_ref()
            .and_then(|hosted| {
                hosted.reconnect.as_ref().map(|episode| {
                    now >= episode.deadline_ms
                        || hosted.account.as_ref().map(|account| account.id) != episode.account_id
                })
            })
            .unwrap_or(false);
        if failed {
            let reason =
                self.hosted.as_ref().unwrap().reconnect.as_ref().unwrap().original_reason.clone();
            self.native_disconnected(&format!(
                "The live game connection could not be restored. {reason}"
            ))?;
        }
        let retry = self
            .hosted
            .as_ref()
            .and_then(|hosted| hosted.reconnect.as_ref())
            .and_then(|episode| episode.retry_at_ms)
            .is_some_and(|at| now >= at);
        if retry {
            self.queue_hosted_reconnect(now)?;
        }
        Ok(())
    }

    pub(super) fn complete_hosted_reconnect(
        &mut self,
        result: Result<ProvisionedGameEndpoint, crate::hosted::ApiError>,
    ) -> Result<(), String> {
        let now = crate::platform::monotonic_ms();
        self.poll_hosted_reconnect(now)?;
        let Some(hosted) = &mut self.hosted else { return Ok(()) };
        let Some(episode) = &mut hosted.reconnect else { return Ok(()) };
        episode.abort = None;
        let endpoint = match result {
            Ok(endpoint) => endpoint,
            Err(error) if error.transport_unavailable => {
                episode.retry_at_ms = Some(now + 1_000.0);
                return Ok(());
            }
            Err(error) => {
                let reason = format!(
                    "The live game connection could not be restored: {error}. {}",
                    episode.original_reason
                );
                return self.native_disconnected(&reason);
            }
        };
        if Some(endpoint.url.as_str()) != hosted.endpoint.as_deref()
            || hosted
                .welcome
                .as_ref()
                .is_none_or(|welcome| welcome.session_kind != endpoint.session_kind)
            || endpoint.observer
        {
            return self.native_disconnected(
                "The reconnect admission selected a different game authority.",
            );
        }
        let selection = self
            .session
            .participant(&episode.credentials.player_id)
            .ok_or("The retained wizard is no longer in the client frame")?
            .player
            .selection
            .clone();
        let hello = HostedHello {
            version: solomon_protocol::PROTOCOL_VERSION,
            credential: endpoint.credential,
            selection,
            profile: PlayerSocialProfile::default(),
            online_preferences: game_online_preferences(self.menu.settings()),
            resume_token: Some(episode.credentials.token.clone()),
            save_intent: None,
            save: None,
            begin_college_intro: false,
            decline_tutorial: false,
            cheats_enabled: false,
            enhanced_effects: self.menu.settings().enhanced_effects,
            allow_mod_mismatch: false,
        };
        self.native.as_mut().unwrap().install_reconnect(
            &endpoint.url,
            ClientMessage::Hosted { command: HostedCommand::Hello { hello } },
        )?;
        Ok(())
    }

    pub(in crate::game) fn finish_hosted_reconnect(&mut self) -> Result<(), String> {
        self.poll_hosted_reconnect(crate::platform::monotonic_ms())?;
        if let Some(hosted) = &mut self.hosted
            && let Some(mut episode) = hosted.reconnect.take()
        {
            episode.cancel();
            set_status("");
        }
        Ok(())
    }
}
