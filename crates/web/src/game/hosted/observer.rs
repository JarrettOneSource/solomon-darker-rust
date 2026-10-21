//! Read-only hosted observation from `client/game-observer-session.ts`.
//! The observed wizard is a camera target, never a participant owned by this connection.

use super::*;
use crate::observer::{ObserverAction, ObserverViewSnapshot};
use solomon_protocol::ResumeCredentials;
use solomon_protocol::hosted::HostedWelcome;
use solomon_sim::social::{GameChatMessage, GameSessionKind};

pub(in crate::game) struct HostedObserver {
    deadline: Option<f64>,
    run_id: Option<String>,
    pub player_id: String,
    pub chat_messages: Vec<GameChatMessage>,
}

impl Game {
    pub(in crate::game) fn is_hosted_observer(&self) -> bool {
        self.hosted.as_ref().is_some_and(|hosted| hosted.observer.is_some())
    }

    pub(super) fn begin_hosted_observer(
        &mut self,
        endpoint: ProvisionedGameEndpoint,
    ) -> Result<(), String> {
        let now = self.last_time.unwrap_or(0.0);
        let hosted = self.hosted.as_mut().unwrap();
        hosted.generation += 1;
        hosted.connecting = true;
        self.begin_hosted_connection(
            &endpoint.url,
            ClientMessage::Hosted {
                command: HostedCommand::ObserverHello {
                    version: solomon_protocol::PROTOCOL_VERSION,
                    credential: endpoint.credential,
                },
            },
            false,
        )?;
        self.hosted.as_mut().unwrap().observer = Some(HostedObserver {
            deadline: Some(now + 5_000.0),
            run_id: None,
            player_id: String::new(),
            chat_messages: Vec::new(),
        });
        self.panels.reset()?;
        Ok(())
    }

    pub(in crate::game) fn accept_observer_welcome(
        &mut self,
        welcome: Option<&HostedWelcome>,
        credentials: &ResumeCredentials,
        snapshot: &GameCheckpoint,
    ) -> Result<bool, String> {
        let Some(observer) = self.hosted.as_mut().and_then(|hosted| hosted.observer.as_mut())
        else {
            if welcome.is_some_and(|welcome| welcome.observer) {
                return Err("The server returned an unexpected observer welcome.".into());
            }
            return Ok(false);
        };
        let valid = observer.run_id.is_none()
            && welcome.is_some_and(|welcome| {
                welcome.observer && welcome.session_kind != GameSessionKind::Standalone
            })
            && snapshot.boneyard.is_some()
            && snapshot.participants.contains_key(&credentials.player_id);
        if !valid {
            return Err("The server returned an invalid observer welcome.".into());
        }
        observer.run_id = Some(snapshot.boneyard.as_ref().unwrap().run_id.clone());
        observer.player_id = credentials.player_id.clone();
        observer.deadline = None;
        Ok(true)
    }

    pub(in crate::game) fn observer_snapshot_player(
        &mut self,
        snapshot: &GameCheckpoint,
    ) -> Result<Option<String>, String> {
        let Some(observer) = self.hosted.as_mut().and_then(|hosted| hosted.observer.as_mut())
        else {
            return Ok(None);
        };
        let run_id = snapshot.boneyard.as_ref().map(|world| world.run_id.as_str());
        if run_id.is_none() || run_id != observer.run_id.as_deref() {
            return Err("The observed match changed identity.".into());
        }
        if !snapshot.participants.contains_key(&observer.player_id) {
            observer.player_id = snapshot
                .participant_order
                .first()
                .cloned()
                .ok_or("The observed match no longer has a viewable player.")?;
        }
        Ok(Some(observer.player_id.clone()))
    }

    pub(in crate::game) fn update_hosted_observer_ui(&mut self) -> Result<(), String> {
        let Some(observer) = self.hosted.as_ref().and_then(|hosted| hosted.observer.as_ref())
        else {
            return Ok(());
        };
        let Some(world) = &self.session.boneyard else { return Ok(()) };
        if self.world_loading || !self.has_game {
            return Ok(());
        }
        self.observer_ui.update(ObserverViewSnapshot {
            session: &self.session,
            view_player_id: &observer.player_id,
            boneyard_name: &world.loaded.name,
            chat_messages: &observer.chat_messages,
            ping_ms: self.native.as_ref().and_then(NativeGame::ping_ms),
        })
    }

    pub(in crate::game) fn exit_hosted_observer(&mut self) -> Result<(), String> {
        self.send_native_message(&ClientMessage::Disconnect)?;
        self.native_disconnected("")
    }

    pub(in crate::game) fn frame_hosted_observer(&mut self, now: f64) -> Result<(), String> {
        for action in self.observer_ui.take_actions() {
            match action {
                ObserverAction::Exit => return self.exit_hosted_observer(),
                ObserverAction::SelectPlayer(player_id) => {
                    if self.session.participants.contains_key(&player_id) {
                        self.hosted.as_mut().unwrap().observer.as_mut().unwrap().player_id =
                            player_id.clone();
                        self.local_player_id = player_id;
                        self.update_hosted_observer_ui()?;
                    }
                }
            }
        }
        let observer = self.hosted.as_mut().unwrap().observer.as_mut().unwrap();
        if observer.deadline.is_some_and(|deadline| now >= deadline) {
            return self.native_disconnected("The observer handshake timed out.");
        }
        self.menu.update(now, self.canvas.client_width(), self.canvas.client_height())?;
        if self.has_game && self.menu.is_blocking() {
            return self.exit_hosted_observer();
        }
        if self.hud.take_actions().iter().any(|action| matches!(action, HudAction::Pause)) {
            return self.exit_hosted_observer();
        }
        // Source observer callbacks cannot mutate skills, inventory, Game Over or input.
        self.game_over.take_actions();
        self.panels.take_actions();
        self.input.set_blocked(true)?;
        self.input.sample()?;
        self.input.resize(
            self.canvas.client_width(),
            self.canvas.client_height(),
            self.menu.settings().ui_scale_percent,
        )?;
        self.update_social(now)?;
        if !self.has_game || self.world_loading {
            self.hud.set_visible(false);
            return self.render();
        }
        self.sample_native_presentation(now, GameSessionInput::default(), true)?;
        self.native_input_frame(GameSessionInput::default())?;
        self.refresh_hud();
        self.hud.set_visible(true);
        self.update_audio()?;
        self.update_overlays(now)?;
        self.hud.set_action_enabled(HudAction::Pause, true)?;
        self.render()
    }
}
