//! The desktop `run-game-host.ts` scope: one ordinary standalone authority, a
//! reusable shared credential and a fresh simulation after its last human leaves.
//! The supervisor's existing dispatcher retains Console, mods and source saves.

use std::sync::Arc;

use solomon_protocol::hosted::HostedServerMessage;
use solomon_protocol::{HostErrorCode, ServerMessage};
use solomon_sim::social::GameSessionKind;

use super::{HostTicket, HostedSupervisor, HostedSupervisorConfig};
use crate::host::authority::send_control;
use crate::host::connection::HostConnection;
use crate::host::hosted::{HostedAdmission, HostedGameHost};
use crate::host::signing::credentials_equal;

pub(super) const DESKTOP_GAME_PATH: &str = "/game";

impl HostedSupervisor {
    /// Unlike Website tickets, the desktop bootstrap credential intentionally
    /// admits every invited peer until this particular native process exits.
    pub fn new_standalone(
        config: HostedSupervisorConfig,
        credential: String,
    ) -> Result<Self, String> {
        if credential.trim().is_empty() {
            return Err("SDR_GAME_BOOTSTRAP_CREDENTIAL must be configured".into());
        }
        if config.memorial_path.is_some() {
            return Err("Desktop standalone does not own the Global Memorial file".into());
        }
        Self::create(config, Some(credential))
    }

    pub(super) fn standalone_ticket(
        &self,
        host: &HostedGameHost,
        candidate: &str,
        now_ms: f64,
    ) -> Option<HostTicket> {
        let credential = self.standalone_credential.as_ref()?;
        if !credentials_equal(candidate, credential) {
            return None;
        }
        Some(HostTicket {
            admission: HostedAdmission {
                content: Arc::clone(&host.content),
                developer_access: false,
                leaderboard_user_id: None,
                party_id: None,
                reservation_id: None,
                party_rejoin_token: None,
                party_recovery_seed: false,
                observer: None,
            },
            expires_at_ms: now_ms,
        })
    }

    pub(super) fn reset_standalone_if_empty(&mut self, now_ms: f64) -> Result<(), String> {
        if self.standalone_credential.is_none() {
            return Ok(());
        }
        let college = &mut self.colleges[0];
        if !college.claimed || !college.host.players.is_empty() {
            return Ok(());
        }
        let previous = &mut college.host;
        let mut fresh = HostedGameHost::new(
            previous.id.clone(),
            GameSessionKind::Standalone,
            self.config.maximum_players,
            Arc::clone(&previous.content),
            &self.config.asset_directory,
            self.config.admin_secret.clone(),
            self.config.revision.clone(),
        )?;
        if let Some(policy) = &self.config.ml_bot_policy {
            fresh.install_ml_bot_policy(Arc::clone(policy))?;
        }
        fresh.runtime_events = self.runtime_events.as_ref().map(|publisher| publisher.sink());
        // Pending, not-yet-admitted sockets survive the last player's close.
        // The retained catalog also keeps Source's entry-lifetime navigation cache.
        fresh.connections = std::mem::take(&mut previous.connections);
        std::mem::swap(&mut fresh.content_loader, &mut previous.content_loader);
        college.host = fresh;
        college.claimed = false;
        college.created_at_ms = now_ms;
        self.shared_host_tick = 0;
        Ok(())
    }
}

impl HostedGameHost {
    /// Standalone cheat visibility belongs to each client. Private College policy
    /// remains shared; toggling one desktop peer never changes another's setting.
    pub(in crate::host) fn set_standalone_cheats(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        enabled: bool,
    ) -> Result<(), (HostErrorCode, String)> {
        let developer = self.player(id).is_some_and(|player| player.developer_access);
        if enabled && developer {
            return Ok(());
        }
        connection.cheats_enabled = enabled && !developer;
        if connection.cheats_enabled {
            let player = self.player_mut(id).expect("the standalone client is admitted");
            player.save_metadata.integrity = solomon_assets::website_save::SaveIntegrity::LocalOnly;
            player.global_score_eligible = false;
            player.tutorial_eligible = false;
            self.taint_ineligible_client_runs();
        }
        send_control(
            &connection.peer,
            ServerMessage::Hosted {
                message: HostedServerMessage::CheatMode { enabled: connection.cheats_enabled },
            },
        )
        .map_err(|reason| (HostErrorCode::ServerError, reason))
    }
}
