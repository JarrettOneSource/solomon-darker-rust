//! Source `MainMenuScene` cheat-menu admission and `game-client-session` authority.
//! The host welcome and accepted party leader determine access; the view emits Lua
//! requests and never changes the local simulation to report a successful command.

use super::*;
use crate::cheats::CheatViewSnapshot;
use solomon_sim::menu::cheats::game_cheat_menu_available;

impl Game {
    pub(super) fn update_cheats(&mut self) -> Result<(), String> {
        let welcome = self.hosted.as_ref().and_then(|hosted| hosted.welcome.as_ref());
        let active = self.has_game && !self.is_hosted_observer() && !self.menu.covers_world();
        let host = self.native.as_ref().and_then(NativeGame::host_snapshot);
        let is_host = host.and_then(|host| host.authority_player_id.as_deref())
            == Some(self.local_player_id.as_str());
        let available = active
            && welcome.is_some_and(|welcome| {
                game_cheat_menu_available(welcome.developer_access, welcome.cheats_enabled, is_host)
            });
        self.menu.set_cheat_menu_available(available)?;
        let admission_hidden = self.session.boneyard.is_none()
            && self.session.participants[&self.local_player_id]
                .player
                .economy
                .college_intro_pending;
        let interrupted =
            self.world_loading || self.skill_selection_active() || self.social.input_active();
        let retain_open = active
            && !admission_hidden
            && !interrupted
            && !self.native_gameplay_held()
            && !self.panels.is_open()
            && !self.menu.is_blocking();
        self.cheats.update(welcome.filter(|_| active).map(|welcome| CheatViewSnapshot {
            session: &self.session,
            local_player_id: &self.local_player_id,
            open_key_code: &self.menu.settings().controls.open_cheats,
            session_kind: welcome.session_kind,
            developer_access: welcome.developer_access,
            cheats_enabled: welcome.cheats_enabled,
            is_host,
            can_open: retain_open,
            retain_open,
        }))?;
        if self.pending_cheat_open {
            if !available || admission_hidden || interrupted {
                self.pending_cheat_open = false;
            } else if !self.menu.is_blocking() && !self.native_gameplay_held() {
                // A Boneyard pause closes first; opening waits for the host's actual
                // resume grace. College pause has no shared simulation hold.
                self.pending_cheat_open = false;
                self.cheats.open()?;
            }
        }
        let input_suspended = self.social.input_active() || self.cheats.is_open();
        self.menu.set_input_suspended(input_suspended)?;
        self.panels.set_input_suspended(input_suspended)?;
        for command in self.cheats.take_commands() {
            self.send_native_message(&ClientMessage::Hosted { command })?;
        }
        Ok(())
    }
}
