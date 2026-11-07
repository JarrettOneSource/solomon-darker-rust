//! Game settings: loading the stored settings, applying a change to the
//! running game, and which game an Enhanced Effects change belongs to.

use crate::{set_status, storage};
use solomon_sim::settings::GameSettings;

use super::{Game, NativeGame};

impl Game {
    pub(super) fn initialize_settings(&mut self) -> Result<(), String> {
        set_status("");
        self.update_mobile_layout()?;
        match storage::load_settings() {
            Ok(settings) => {
                self.apply_settings(&settings)?;
                self.menu.set_settings(settings)?;
            }
            Err(error) => Self::show_storage_error(&error),
        }
        Ok(())
    }

    fn apply_settings(&mut self, settings: &GameSettings) -> Result<(), String> {
        self.hud.set_ui_scale(f64::from(settings.ui_scale_percent) / 100.0);
        self.input.set_controls(settings.controls.clone())?;
        self.panels.set_controls(settings.controls.clone());
        self.social.set_controls(settings.controls.clone());
        self.input.set_secondary_at_pointer(settings.cast_secondaries_at_mouse);
        Ok(())
    }

    /// Which game an Enhanced Effects change applies to: the public College changes the
    /// next party run; an admitted private or active game changes its own mode.
    pub(super) fn enhanced_effects_authority(
        &self,
    ) -> Option<solomon_protocol::hosted::EnhancedEffectsAuthority> {
        use solomon_protocol::hosted::{EnhancedEffectsAuthority, EnhancedEffectsScope};
        if !self.has_game {
            return None;
        }
        let host = self.native.as_ref().and_then(NativeGame::host_snapshot);
        let next_party_run = self.session.boneyard.is_none()
            && self.hosted.as_ref().and_then(|hosted| hosted.welcome.as_ref()).is_some_and(
                |welcome| welcome.session_kind == solomon_sim::social::GameSessionKind::GlobalHub,
            );
        let party = self
            .native
            .as_ref()
            .and_then(|native| native.social.as_ref())
            .and_then(|social| social.party_state.as_ref());
        Some(EnhancedEffectsAuthority {
            enabled: if next_party_run {
                party.map_or(self.session.enhanced_effects, |state| state.party.enhanced_effects)
            } else {
                self.session.enhanced_effects
            },
            can_change: !self.is_hosted_observer()
                && host.is_none_or(|host| {
                    host.authority_player_id.as_deref() == Some(self.local_player_id.as_str())
                        && !host.materializing_player_ids.contains(&self.local_player_id)
                }),
            scope: if next_party_run {
                EnhancedEffectsScope::NextPartyRun
            } else {
                EnhancedEffectsScope::Game
            },
        })
    }

    pub(super) fn commit_settings(&mut self, settings: GameSettings) -> Result<(), String> {
        self.send_hosted_settings(&settings)?;
        self.apply_settings(&settings)?;
        self.menu.set_settings(settings.clone())?;
        if let Err(error) = storage::save_settings(&settings) {
            Self::show_storage_error(&error);
        }
        Ok(())
    }
}
