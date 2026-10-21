//! Native host admission and accepted-frame installation. The browser keeps presentation
//! resources; `GameSession::apply_checkpoint` replaces authority without a second tick.
//! Readiness and modal holds follow `client/game-client-session.ts` and `MainMenuScene.tsx`.

use super::*;
use crate::network::{self, NativeSession, NetworkEvent, ReceivedNativeSnapshot};
use crate::presentation::{NativePresentationTimeline, PresentationFrame};
use crate::shared_gameplay::{ResumeGraceView, SharedGameplaySnapshot, SharedPauseView};
use solomon_protocol::{
    ClientMessage, GameCommand, HubActivity, PauseSource, ResumeCredentials, ResumeGraceReason,
    ServerMessage,
};
use solomon_sim::economy::HubEconomyResult;
use solomon_sim::social::{GameChatRejection, SocialSnapshot, append_game_chat_message};

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativePhase {
    Inspecting,
    Title,
    Joining,
    Playing,
    Leaving,
    Updating,
}

pub(super) enum CommandFeedback {
    None,
    Inventory(HubInventoryAction),
    SkillPicker,
    CollegeIntro,
    CollegeDialogue,
    Loadout,
    Social,
}

struct CommandReceipt {
    sequence: u64,
    snapshot_sequence: u64,
    accepted: bool,
    reason: Option<String>,
    inventory_result: Option<HubEconomyResult>,
}

pub(super) struct NativeGame {
    connection: Option<NativeSession>,
    phase: NativePhase,
    resume: Option<ResumeCredentials>,
    selection: Option<NewGameSelection>,
    can_resume: bool,
    reset_after_join: Option<NewGameSelection>,
    tutorial_after_join: bool,
    loaded_boneyard: Option<LoadedBoneyard>,
    // Welcome precedes BoneyardLoaded. This frame waits for scene metadata; the
    // separate pending_snapshot belongs to an already-started world materialization.
    waiting_boneyard_frame: Option<ReceivedNativeSnapshot>,
    pub(super) pending_snapshot: Option<ReceivedNativeSnapshot>,
    presentation: NativePresentationTimeline,
    pub(super) display: Option<PresentationFrame>,
    pending_commands: BTreeMap<u64, CommandFeedback>,
    receipts: Vec<CommandReceipt>,
    pause: Option<PauseSource>,
    hub_activity: Option<HubActivity>,
    college_ready: bool,
    resume_ready_snapshot: Option<u64>,
    pub(super) social: Option<SocialSnapshot>,
    pub(super) chat_rejections: Vec<GameChatRejection>,
    listener: Option<(Vec2, f64)>,
    save_request: u64,
    leave_request: Option<u64>,
    pub(super) hosted_welcome: Option<solomon_protocol::hosted::HostedWelcome>,
    pub(super) hosted_messages: Vec<(solomon_protocol::hosted::HostedServerMessage, f64)>,
}

impl NativeGame {
    pub(super) fn presentation_timeline(&self) -> &NativePresentationTimeline {
        &self.presentation
    }

    pub(super) fn is_playing(&self) -> bool {
        self.phase == NativePhase::Playing
    }
    pub(super) fn is_leaving(&self) -> bool {
        self.phase == NativePhase::Leaving
    }

    pub(super) fn begin_deployment(&mut self) {
        self.phase = NativePhase::Updating;
        self.leave_request = None;
        if let Some(connection) = &mut self.connection {
            connection.stop_input();
        }
    }

    pub(super) fn acknowledge_deployment(
        &self,
        command: solomon_protocol::hosted::HostedCommand,
    ) -> Result<(), String> {
        if let Some(connection) = &self.connection
            && connection.is_open()
        {
            connection.send(&ClientMessage::Hosted { command })?;
        }
        Ok(())
    }

    pub(super) fn expects_leave_receipt(&self, request_id: u64) -> bool {
        self.leave_request == Some(request_id)
    }

    pub(super) fn cancel_leave(&mut self) {
        self.phase = NativePhase::Playing;
        self.leave_request = None;
    }

    pub(super) fn ping_ms(&self) -> Option<f64> {
        self.connection.as_ref().and_then(NativeSession::ping_ms)
    }

    pub(super) fn host_snapshot(&self) -> Option<&solomon_protocol::HostSnapshot> {
        self.connection.as_ref().map(|connection| &connection.host)
    }

    pub(super) fn inspect(window: &web_sys::Window) -> Result<Self, String> {
        let resume = network::read_resume(window)?;
        let mut native = Self::idle();
        native.connection = Some(NativeSession::connect(
            window,
            ClientMessage::InspectSession { resume: resume.clone() },
        )?);
        native.phase = NativePhase::Inspecting;
        native.resume = resume;
        Ok(native)
    }

    pub(super) fn idle() -> Self {
        Self {
            connection: None,
            phase: NativePhase::Title,
            resume: None,
            selection: None,
            can_resume: false,
            reset_after_join: None,
            tutorial_after_join: false,
            loaded_boneyard: None,
            waiting_boneyard_frame: None,
            pending_snapshot: None,
            presentation: NativePresentationTimeline::new(
                solomon_protocol::GAME_TICK_RATE,
                solomon_protocol::SNAPSHOT_RATE,
            ),
            display: None,
            pending_commands: BTreeMap::new(),
            receipts: Vec::new(),
            pause: None,
            hub_activity: None,
            college_ready: false,
            resume_ready_snapshot: None,
            social: None,
            chat_rejections: Vec::new(),
            listener: None,
            save_request: 0,
            leave_request: None,
            hosted_welcome: None,
            hosted_messages: Vec::new(),
        }
    }

    pub(super) fn set_saved_selection(&mut self, selection: Option<NewGameSelection>) {
        self.can_resume = selection.is_some();
        self.selection = selection;
    }
}

impl Game {
    /// Shared waiting and countdown surfaces describe the accepted host hold. Their
    /// display clock never starts or ends a pause in the authoritative session.
    pub(super) fn update_shared_gameplay(&mut self, now: f64) -> Result<(), String> {
        let host = self.native.as_ref().and_then(NativeGame::host_snapshot);
        self.shared_gameplay.update(
            SharedGameplaySnapshot {
                local_player_id: &self.local_player_id,
                has_game: self.has_game && !self.is_hosted_observer(),
                menu_covers_world: self.menu.covers_world(),
                settings_open: self.menu.settings_open(),
                pause: host.and_then(|host| host.pause.as_ref()).map(|pause| SharedPauseView {
                    owner_player_id: &pause.owner_player_id,
                    owner_display_name: &pause.owner_display_name,
                    activity: match pause.source {
                        PauseSource::Inventory => Some("Inventory"),
                        PauseSource::SkillBook => Some("the Skill Book"),
                        PauseSource::SkillSelector => Some("the skill selector"),
                        PauseSource::PauseMenu => None,
                    },
                }),
                resume_grace: host.and_then(|host| host.resume_grace.as_ref()).map(|grace| {
                    ResumeGraceView {
                        reason: match grace.reason {
                            ResumeGraceReason::GameRejoined => "game-rejoined",
                            ResumeGraceReason::GameRestarted => "game-restarted",
                            ResumeGraceReason::GameStarted => "game-started",
                            ResumeGraceReason::InventoryClosed => "inventory-closed",
                            ResumeGraceReason::PauseMenuClosed => "pause-menu-closed",
                            ResumeGraceReason::SkillBookClosed => "skill-book-closed",
                            ResumeGraceReason::SkillPickerClosed => "skill-picker-closed",
                            ResumeGraceReason::PartyRejoinWait => "party-rejoin-wait",
                        },
                        remaining_ms: grace.remaining_ms,
                        sequence: grace.sequence,
                    }
                }),
            },
            now,
            self.canvas.client_width(),
            self.canvas.client_height(),
        )
    }

    pub(super) fn poll_native(&mut self, now: f64) -> Result<(), String> {
        let Some(native) = &mut self.native else { return Ok(()) };
        let Some(connection) = &mut native.connection else { return Ok(()) };
        let poll = connection.poll()?;
        for event in poll.events {
            match event {
                NetworkEvent::Opened => {
                    if let Some(loading) = &mut self.match_loading {
                        loading.advance(MatchLoadingStage::AuthenticatingSession);
                    }
                }
                NetworkEvent::Failed(reason) => {
                    if self
                        .hosted
                        .as_ref()
                        .is_some_and(|hosted| hosted.deployment_update.restarting())
                    {
                        self.finish_deployment_connection();
                    } else {
                        self.native_disconnected(&reason)?;
                    }
                }
                NetworkEvent::Closed { code, reason } => {
                    if self
                        .hosted
                        .as_ref()
                        .is_some_and(|hosted| hosted.deployment_update.restarting())
                    {
                        self.finish_deployment_connection();
                        continue;
                    }
                    if self.native.as_ref().is_some_and(|native| {
                        !matches!(native.phase, NativePhase::Title | NativePhase::Inspecting)
                    }) {
                        self.native_disconnected(if reason.is_empty() {
                            if code == 1000 {
                                "Disconnected from the game"
                            } else {
                                "The game connection closed"
                            }
                        } else {
                            &reason
                        })?;
                    }
                }
                NetworkEvent::Message { message, received_at_ms } => match *message {
                    ServerMessage::SessionStatus {
                        version,
                        can_resume,
                        selection,
                        fresh_join_available,
                    } => {
                        if version != solomon_protocol::PROTOCOL_VERSION {
                            return Err("The game host uses an unsupported protocol".into());
                        }
                        let native = self.native.as_mut().unwrap();
                        native.phase = NativePhase::Title;
                        native.can_resume = can_resume;
                        native.selection = selection;
                        native.connection = None;
                        self.menu.show_title(can_resume)?;
                        if !can_resume
                            && fresh_join_available
                            && !storage::tutorial_declined().unwrap_or(true)
                        {
                            self.menu.offer_tutorial()?;
                        }
                        set_status("");
                    }
                    ServerMessage::Welcome {
                        version,
                        credentials,
                        tick_rate,
                        snapshot_rate,
                        frame,
                        hosted,
                    } => {
                        if let Some(loading) = &mut self.match_loading {
                            loading.advance(MatchLoadingStage::ReceivingHostCheckpoint);
                        }
                        if version != solomon_protocol::PROTOCOL_VERSION
                            || tick_rate != solomon_protocol::GAME_TICK_RATE
                            || snapshot_rate != solomon_protocol::SNAPSHOT_RATE
                        {
                            return Err("The game host uses an unsupported simulation rate".into());
                        }
                        if !frame.snapshot.participants.contains_key(&credentials.player_id) {
                            return Err("The game host did not include your wizard".into());
                        }
                        for participant in frame.snapshot.participants.values() {
                            save_document::validate_participant(participant)?;
                        }
                        let observing = match self.accept_observer_welcome(
                            hosted.as_ref(),
                            &credentials,
                            &frame.snapshot,
                        ) {
                            Ok(observing) => observing,
                            Err(reason) => return self.native_disconnected(&reason),
                        };
                        if !observing && !self.is_editor_test() {
                            network::remember_resume(
                                &web_sys::window().ok_or("no window")?,
                                &credentials,
                            )?;
                        }
                        let native = self.native.as_mut().unwrap();
                        native.resume = Some(credentials.clone());
                        if let Some(owner) = &mut self.hosted
                            && hosted.is_some()
                        {
                            owner.content_pending = true;
                        }
                        native.hosted_welcome = hosted;
                        if !observing {
                            native.can_resume = true;
                        }
                        native
                            .connection
                            .as_mut()
                            .ok_or("The game connection closed during admission")?
                            .credentials = Some(credentials.clone());
                        native.connection.as_mut().unwrap().accept_welcome()?;
                        native.connection.as_mut().unwrap().start_ping(observing)?;
                        self.accept_native_snapshot(ReceivedNativeSnapshot {
                            frame: Box::new(frame),
                            received_at_ms,
                        })?;
                    }
                    ServerMessage::BoneyardLoaded { boneyard } => {
                        let native = self.native.as_mut().unwrap();
                        let waiting = native.waiting_boneyard_frame.as_ref().is_some_and(|frame| {
                            frame
                                .frame
                                .snapshot
                                .boneyard
                                .as_ref()
                                .is_some_and(|world| world.run_id == boneyard.run_id)
                        });
                        native.loaded_boneyard = Some(*boneyard);
                        if waiting {
                            let frame = native.waiting_boneyard_frame.take().unwrap();
                            self.accept_native_snapshot(frame)?;
                        }
                    }
                    ServerMessage::Snapshot { frame } => {
                        self.accept_native_snapshot(ReceivedNativeSnapshot {
                            frame: Box::new(frame),
                            received_at_ms,
                        })?
                    }
                    ServerMessage::Audio { events, .. } => {
                        for event in events {
                            self.audio.apply(event)?;
                        }
                    }
                    ServerMessage::CommandResult {
                        sequence,
                        snapshot_sequence,
                        accepted,
                        reason,
                        inventory_result,
                    } => {
                        self.native.as_mut().unwrap().receipts.push(CommandReceipt {
                            sequence,
                            snapshot_sequence,
                            accepted,
                            reason,
                            inventory_result,
                        });
                    }
                    ServerMessage::SaveCommitted { request_id, .. } => {
                        if self.native.as_ref().unwrap().leave_request == Some(request_id) {
                            if let Some(connection) = &self.native.as_ref().unwrap().connection {
                                connection.send(&ClientMessage::Disconnect)?;
                            }
                            self.native_disconnected("")?;
                        }
                    }
                    ServerMessage::Error { code, reason } => {
                        if matches!(
                            code,
                            solomon_protocol::HostErrorCode::CommandRejected
                                | solomon_protocol::HostErrorCode::RunInProgress
                        ) && !self.world_loading
                            && self.session.boneyard.is_none()
                            && self
                                .match_loading
                                .is_some_and(|loading| loading.flow == MatchLoadingFlow::Boneyard)
                        {
                            self.match_loading = None;
                        }
                        let rejected_identity =
                            code == solomon_protocol::HostErrorCode::InvalidCredentials;
                        if rejected_identity && !self.is_hosted_observer() && !self.is_editor_test()
                        {
                            let native = self.native.as_mut().unwrap();
                            if let Some(rejected) = native.resume.take() {
                                network::forget_rejected_resume(
                                    &web_sys::window().ok_or("no window")?,
                                    &rejected,
                                )?;
                            }
                            native.can_resume = false;
                            native.selection = None;
                        }
                        if rejected_identity
                            || self
                                .native
                                .as_ref()
                                .is_some_and(|native| native.phase == NativePhase::Joining)
                        {
                            self.native_disconnected(&reason)?;
                        } else {
                            set_status(&reason);
                        }
                    }
                    // The connection measures these on receipt, including while rAF is suspended.
                    ServerMessage::Pong { .. } => {}
                    ServerMessage::Social { mut snapshot } => {
                        // Hosted PartyState publishes membership independently of accepted chat.
                        if self.hosted.is_some()
                            && let Some(previous) = &mut self.native.as_mut().unwrap().social
                        {
                            snapshot.chat_messages = std::mem::take(&mut previous.chat_messages);
                        }
                        self.native.as_mut().unwrap().social = Some(snapshot);
                    }
                    ServerMessage::Chat { message } => {
                        self.world_overlays.consume_chat(&message, received_at_ms);
                        if let Some(observer) =
                            self.hosted.as_mut().and_then(|hosted| hosted.observer.as_mut())
                        {
                            append_game_chat_message(&mut observer.chat_messages, message);
                            self.update_hosted_observer_ui()?;
                            continue;
                        }
                        let social = self
                            .native
                            .as_mut()
                            .unwrap()
                            .social
                            .as_mut()
                            .ok_or("The game host sent chat before its social state")?;
                        append_game_chat_message(&mut social.chat_messages, message);
                    }
                    ServerMessage::ChatRejected { rejection } => {
                        self.native.as_mut().unwrap().chat_rejections.push(rejection);
                    }
                    ServerMessage::Hosted { message } => {
                        self.native
                            .as_mut()
                            .unwrap()
                            .hosted_messages
                            .push((message, received_at_ms));
                    }
                    ServerMessage::WelcomeChunk { .. } => {
                        return Err("The game received an unassembled welcome".into());
                    }
                },
            }
        }
        if let Some(snapshot) = poll.snapshot
            && self.native.as_ref().is_some_and(|native| native.connection.is_some())
        {
            self.accept_native_snapshot(snapshot)?;
        }
        if poll.ping_changed {
            self.update_hosted_observer_ui()?;
        }
        self.release_native_receipts(now)
    }

    pub(super) fn native_disconnected(&mut self, reason: &str) -> Result<(), String> {
        self.finish_desktop_close(Err(if reason.is_empty() {
            "The game connection closed before the final save completed.".into()
        } else {
            reason.to_owned()
        }));
        if self.is_editor_test() {
            return self.editor_test_failed(reason);
        }
        self.match_loading = None;
        let create_pending = self.menu.is_create_pending();
        self.cheats.close();
        self.pending_cheat_open = false;
        self.social.set_hosted_session(false);
        let observing = self.is_hosted_observer();
        self.observer_ui.hide();
        if observing {
            self.hosted.as_mut().unwrap().observer = None;
            self.native.as_mut().unwrap().resume = None;
        }
        self.reset_hosted_presentation()?;
        self.restore_hosted_saved_selection();
        if let Some(hosted) = &mut self.hosted {
            hosted.end_connection();
        }
        self.world_overlays.reset();
        let native = self.native.as_mut().unwrap();
        native.connection = None;
        native.phase = NativePhase::Title;
        native.pending_commands.clear();
        native.receipts.clear();
        native.waiting_boneyard_frame = None;
        native.pending_snapshot = None;
        native.loaded_boneyard = None;
        native.presentation.reset();
        native.display = None;
        native.leave_request = None;
        self.world_generation += 1;
        self.pending_world = None;
        self.world_loading = false;
        self.panels.reset()?;
        if observing {
            self.menu.show_dark_cloud()?;
        } else if create_pending {
            self.menu.complete_create_loadout(false)?;
        } else {
            self.menu.show_title(native.can_resume)?;
        }
        self.has_game = false;
        self.reset_world_views()?;
        set_status(if observing && reason == "observed match ended" {
            "The observed match ended."
        } else {
            reason
        });
        Ok(())
    }

    fn finish_deployment_connection(&mut self) {
        let native = self.native.as_mut().expect("the deployment owns its native session");
        native.connection = None;
        native.pending_commands.clear();
        native.receipts.clear();
        native.waiting_boneyard_frame = None;
        native.pending_snapshot = None;
        native.hosted_messages.clear();
        self.cheats.close();
    }

    pub(super) fn cancel_native_leave(&mut self) -> Result<(), String> {
        let native = self.native.as_mut().ok_or("The game connection is closed")?;
        native.cancel_leave();
        self.menu.show_pause()
    }

    pub(super) fn native_menu_action(&mut self, action: &MenuAction) -> Result<bool, String> {
        if self.native.is_none() {
            return Ok(false);
        }
        match action {
            MenuAction::Start(selection) => {
                let native = self.native.as_ref().unwrap();
                if native.phase == NativePhase::Playing {
                    self.send_native_command(
                        GameCommand::ResetWizard { selection: selection.clone() },
                        CommandFeedback::None,
                    )?;
                } else {
                    let resume = native.can_resume.then(|| native.resume.clone()).flatten();
                    self.join_native(selection.clone(), resume, Some(selection.clone()), false)?;
                }
            }
            MenuAction::Continue => {
                let native = self.native.as_ref().unwrap();
                let selection =
                    native.selection.clone().ok_or("Your saved wizard is not available")?;
                let resume =
                    native.resume.clone().ok_or("Your saved wizard credential is not available")?;
                self.join_native(selection, Some(resume), None, false)?;
            }
            MenuAction::StartTutorial => {
                self.join_native(
                    NewGameSelection {
                        name: "Sirmin".into(),
                        element: WizardElement::Ether,
                        discipline: WizardDiscipline::Arcane,
                    },
                    None,
                    None,
                    true,
                )?;
            }
            MenuAction::ConfirmLoadout(selection) => {
                self.send_native_command(
                    GameCommand::ConfirmLoadout { selection: selection.clone() },
                    CommandFeedback::None,
                )?;
            }
            MenuAction::Leave => {
                let native = self.native.as_mut().unwrap();
                native.save_request += 1;
                native.leave_request = Some(native.save_request);
                native.phase = NativePhase::Leaving;
                native
                    .connection
                    .as_ref()
                    .ok_or("The game connection is closed")?
                    .send(&ClientMessage::SaveBeforeLeave { request_id: native.save_request })?;
                set_status("Saving game…");
            }
            MenuAction::SettingsChanged(_)
            | MenuAction::SetEnhancedEffects { .. }
            | MenuAction::CancelCreate
            | MenuAction::Cheats
            | MenuAction::SaveMobileLayout(_)
            | MenuAction::TutorialDeclined
            | MenuAction::Resume => {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn join_native(
        &mut self,
        selection: NewGameSelection,
        resume: Option<ResumeCredentials>,
        reset: Option<NewGameSelection>,
        tutorial: bool,
    ) -> Result<(), String> {
        let reset = resume.as_ref().and(reset);
        let begin_college_intro = resume.is_none() && !tutorial;
        let connection = NativeSession::connect(
            &web_sys::window().ok_or("no window")?,
            ClientMessage::Hello {
                version: solomon_protocol::PROTOCOL_VERSION,
                enhanced_effects: self.menu.settings().enhanced_effects,
                selection,
                resume,
                initial_save: None,
                begin_college_intro,
            },
        )?;
        self.begin_native_connection(connection, reset, tutorial)
    }

    pub(super) fn begin_hosted_connection(
        &mut self,
        endpoint: &str,
        hello: ClientMessage,
        tutorial: bool,
    ) -> Result<(), String> {
        self.reset_hosted_presentation()?;
        self.social.set_hosted_session(false);
        self.social.set_hosted_session(true);
        let connection =
            NativeSession::connect_to(&web_sys::window().ok_or("no window")?, endpoint, hello)?;
        self.begin_native_connection(connection, None, tutorial)
    }

    fn begin_native_connection(
        &mut self,
        connection: NativeSession,
        reset: Option<NewGameSelection>,
        tutorial: bool,
    ) -> Result<(), String> {
        self.begin_match_loading(
            if tutorial { MatchLoadingFlow::Boneyard } else { MatchLoadingFlow::Hub },
            MatchLoadingStage::ConnectingTransport,
        );
        self.cheats.close();
        self.pending_cheat_open = false;
        let native = self.native.as_mut().unwrap();
        native.connection = Some(connection);
        self.world_overlays.reset();
        native.phase = NativePhase::Joining;
        native.presentation.reset();
        native.display = None;
        native.reset_after_join = reset;
        native.tutorial_after_join = tutorial;
        native.loaded_boneyard = None;
        native.waiting_boneyard_frame = None;
        native.pending_snapshot = None;
        native.pending_commands.clear();
        native.receipts.clear();
        native.social = None;
        native.chat_rejections.clear();
        native.listener = None;
        native.pause = None;
        native.hub_activity = None;
        native.college_ready = false;
        native.resume_ready_snapshot = None;
        native.hosted_welcome = None;
        native.hosted_messages.clear();
        set_status("Joining game…");
        Ok(())
    }

    pub(super) fn send_native_command(
        &mut self,
        command: GameCommand,
        feedback: CommandFeedback,
    ) -> Result<(), String> {
        if self.is_hosted_observer() {
            return Err("An observer cannot send player commands.".into());
        }
        let native = self.native.as_mut().ok_or("No native game host")?;
        let feedback = match command {
            GameCommand::ResetWizard { .. } | GameCommand::ArmCollegeIntro => {
                CommandFeedback::CollegeIntro
            }
            GameCommand::AcknowledgeCollegeIntroDialogue => CommandFeedback::CollegeDialogue,
            GameCommand::ConfirmLoadout { .. } => CommandFeedback::Loadout,
            _ => feedback,
        };
        let sequence = native
            .connection
            .as_mut()
            .ok_or("The game connection is closed")?
            .send_command(command)?;
        native.pending_commands.insert(sequence, feedback);
        Ok(())
    }

    pub(super) fn accept_native_snapshot(
        &mut self,
        received: ReceivedNativeSnapshot,
    ) -> Result<(), String> {
        if let Some(hosted) = &mut self.hosted
            && hosted.content_pending
        {
            hosted.pending_content_frame = Some(received);
            return Ok(());
        }
        let frame = &received.frame;
        let observer_player = match self.observer_snapshot_player(&frame.snapshot) {
            Ok(player) => player,
            Err(reason) => return self.native_disconnected(&reason),
        };
        let local_id = observer_player.as_ref().unwrap_or(
            &self
                .native
                .as_ref()
                .unwrap()
                .resume
                .as_ref()
                .ok_or("The host sent a snapshot before admission")?
                .player_id,
        );
        if !frame.snapshot.participants.contains_key(local_id) {
            return Err("The game host snapshot did not include your wizard".into());
        }
        for participant in frame.snapshot.participants.values() {
            save_document::validate_participant(participant)?;
        }
        let incoming_run = frame.snapshot.boneyard.as_ref().map(|world| world.run_id.as_str());
        let current_run = self.session.boneyard.as_ref().map(|world| world.loaded.run_id.as_str());
        self.native.as_mut().unwrap().waiting_boneyard_frame = None;
        if incoming_run.is_some() && (incoming_run != current_run || self.battle.is_none()) {
            let pending_run = self
                .native
                .as_ref()
                .unwrap()
                .pending_snapshot
                .as_ref()
                .and_then(|received| received.frame.snapshot.boneyard.as_ref())
                .map(|world| world.run_id.as_str());
            if pending_run != incoming_run {
                let Some(loaded) = self
                    .native
                    .as_ref()
                    .unwrap()
                    .loaded_boneyard
                    .as_ref()
                    .filter(|loaded| Some(loaded.run_id.as_str()) == incoming_run)
                else {
                    let native = self.native.as_mut().unwrap();
                    native.waiting_boneyard_frame = Some(received);
                    if native.pending_snapshot.take().is_some() {
                        self.world_generation += 1;
                        self.pending_world = None;
                        self.world_loading = false;
                    }
                    return Ok(());
                };
                solomon_sim::boneyard::world::BoneyardWorld::restore(
                    loaded.clone(),
                    *frame.snapshot.boneyard.as_ref().unwrap().clone(),
                )?;
                let loaded = loaded.clone();
                self.reset_native_world_ui()?;
                self.pending_world = Some(loaded);
                self.world_generation += 1;
                self.world_loading = true;
                self.begin_match_loading(
                    MatchLoadingFlow::Boneyard,
                    MatchLoadingStage::MaterializingWorld,
                );
                set_status("Loading Boneyard…");
            }
            self.native.as_mut().unwrap().pending_snapshot = Some(received);
            return Ok(());
        }
        if incoming_run.is_none() && (self.battle.is_some() || self.world_loading) {
            self.reset_native_world_ui()?;
            self.world_generation += 1;
            self.pending_world = None;
            self.world_loading = false;
            self.native.as_mut().unwrap().pending_snapshot = None;
            self.reset_world_views()?;
        }
        self.apply_native_frame(received, None)
    }

    fn reset_native_world_ui(&mut self) -> Result<(), String> {
        self.panels.reset()?;
        // The host still commits accepted transactions; old surfaces no longer consume
        // their feedback after a world switch (the source scene has unmounted).
        self.native.as_mut().unwrap().pending_commands.retain(|_, feedback| {
            !matches!(feedback, CommandFeedback::Inventory(_) | CommandFeedback::SkillPicker)
        });
        Ok(())
    }

    pub(super) fn apply_native_frame(
        &mut self,
        received: ReceivedNativeSnapshot,
        loaded: Option<LoadedBoneyard>,
    ) -> Result<(), String> {
        let frame = *received.frame;
        let observing = self.is_hosted_observer();
        self.local_player_id =
            if let Some(player) = self.observer_snapshot_player(&frame.snapshot)? {
                player
            } else {
                self.native
                    .as_ref()
                    .unwrap()
                    .resume
                    .as_ref()
                    .ok_or("The host sent a snapshot before admission")?
                    .player_id
                    .clone()
            };
        let sequence = frame.sequence;
        let acknowledgement =
            frame.input_acknowledgements.get(&self.local_player_id).copied().unwrap_or(0);
        let previous_hub_owner = self.session.boneyard.is_none().then(|| {
            (
                self.session.run.phase,
                self.session
                    .participants
                    .get(&self.local_player_id)
                    .map(|participant| participant.lighting.light_registration),
            )
        });
        self.session.apply_checkpoint(*frame.snapshot, loaded)?;
        let next_hub_owner = self.session.boneyard.is_none().then(|| {
            (
                self.session.run.phase,
                self.session
                    .participants
                    .get(&self.local_player_id)
                    .map(|participant| participant.lighting.light_registration),
            )
        });
        if previous_hub_owner != next_hub_owner {
            self.world_screen_feedback.reset();
        }
        self.has_game = true;
        let native = self.native.as_mut().unwrap();
        let joining = native.phase == NativePhase::Joining;
        if joining {
            native.phase = NativePhase::Playing;
        }
        native.selection =
            Some(self.session.participants[&self.local_player_id].player.selection.clone());
        native.presentation.push(
            &self.session,
            received.received_at_ms,
            &self.local_player_id,
            frame.flame_presentation.as_ref(),
        )?;
        let connection = native
            .connection
            .as_mut()
            .ok_or("The game connection closed before its snapshot arrived")?;
        connection.acknowledge_snapshot(
            sequence,
            acknowledgement,
            frame.host,
            self.session.level_up_barrier.is_some(),
        )?;
        let intro_active = self.session.participants[&self.local_player_id].college_intro.is_some();
        if !intro_active {
            native.college_ready = false;
        }
        if intro_active
            && !observing
            && !native.college_ready
            && native.reset_after_join.is_none()
            && !native
                .pending_commands
                .values()
                .any(|feedback| matches!(feedback, CommandFeedback::CollegeIntro))
        {
            connection.send(&ClientMessage::CollegeIntroReady)?;
            native.college_ready = true;
        }
        if let Some(selection) = native.reset_after_join.take() {
            let sequence = connection.send_command(GameCommand::ResetWizard { selection })?;
            native.pending_commands.insert(sequence, CommandFeedback::CollegeIntro);
        }
        if std::mem::take(&mut native.tutorial_after_join) {
            connection.send(&ClientMessage::StartRun { tutorial: true })?;
        }
        if joining {
            let waiting_for_create_reset = self.menu.is_create_pending()
                && native
                    .pending_commands
                    .values()
                    .any(|feedback| matches!(feedback, CommandFeedback::CollegeIntro));
            if !waiting_for_create_reset {
                self.menu.complete_create_loadout(true)?;
                self.menu.hide()?;
            }
            self.panels.reset()?;
            set_status("");
        }
        if let Some(world) = &self.session.boneyard {
            self.battle_views.consume_events(&world.enemy_events);
            self.world_screen_feedback
                .consume_enemy_events(&world.loaded.run_id, &world.enemy_events);
        }
        self.refresh_hud();
        self.update_combat_audio()?;
        self.update_hosted_observer_ui()?;
        self.update_hosted_presentation(false)
    }

    fn release_native_receipts(&mut self, now: f64) -> Result<(), String> {
        let native = self.native.as_mut().unwrap();
        let accepted_sequence =
            native.connection.as_ref().map_or(0, |connection| connection.snapshot_sequence);
        let mut waiting = Vec::new();
        for receipt in std::mem::take(&mut native.receipts) {
            if receipt.snapshot_sequence > accepted_sequence {
                waiting.push(receipt);
                continue;
            }
            let Some(feedback) = native.pending_commands.remove(&receipt.sequence) else {
                continue;
            };
            if matches!(feedback, CommandFeedback::CollegeIntro | CommandFeedback::Loadout) {
                self.menu.complete_create_loadout(receipt.accepted)?;
            }
            match feedback {
                CommandFeedback::CollegeIntro if receipt.accepted => {
                    native.college_ready = false;
                    if self.session.participants[&self.local_player_id].college_intro.is_some() {
                        native
                            .connection
                            .as_ref()
                            .ok_or("The game connection closed")?
                            .send(&ClientMessage::CollegeIntroReady)?;
                        native.college_ready = true;
                    }
                }
                CommandFeedback::Inventory(action)
                    if receipt.accepted || receipt.inventory_result.is_some() =>
                {
                    let result =
                        receipt.inventory_result.ok_or("The host omitted the inventory result")?;
                    self.panels.inventory_action_feedback(
                        &action,
                        &result,
                        now,
                        self.mod_content.current(),
                    );
                }
                CommandFeedback::SkillPicker if !receipt.accepted => {
                    self.skill_picker.reject_action(now)
                }
                CommandFeedback::Social if !receipt.accepted => {
                    if let Some(reason) = &receipt.reason {
                        self.social.party_error(reason);
                    }
                }
                _ if !receipt.accepted => {
                    if let Some(reason) = receipt.reason {
                        set_status(&reason);
                    }
                    self.audio.apply(solomon_sim::audio::AudioEvent::Play(
                        solomon_sim::audio::AudioCue::BadAction,
                        Default::default(),
                    ))?;
                }
                _ => {}
            }
        }
        native.receipts = waiting;
        Ok(())
    }

    /// The display owner samples once per animation frame. UI commands, audio, saves and
    /// acknowledgements continue to read the accepted session, never this projection.
    pub(super) fn sample_native_presentation(
        &mut self,
        now: f64,
        input: GameSessionInput,
        held: bool,
    ) -> Result<(), String> {
        if let Some(native) = &mut self.native {
            native.display = if native.phase == NativePhase::Playing {
                native.presentation.sample(now, &self.local_player_id, input, held)?
            } else {
                None
            };
        }
        Ok(())
    }

    pub(super) fn native_input_frame(&mut self, input: GameSessionInput) -> Result<(), String> {
        if self.is_hosted_observer() {
            return Ok(());
        }
        let (camera, viewport) = self.scene_view();
        let width = viewport.logical_size.x / camera.scale;
        let center = Vec2::new(
            camera.origin.x + width / 2.0,
            camera.origin.y + viewport.logical_size.y / camera.scale / 2.0,
        );
        let picker_open = self.skill_selection_active();
        let social_open = self.social.input_active();
        let cheats_open = self.cheats.is_open();
        let native = self.native.as_mut().unwrap();
        if native.phase != NativePhase::Playing || self.world_loading {
            return Ok(());
        }
        let connection = native.connection.as_mut().ok_or("The game connection closed")?;
        if native.listener != Some((center, width)) {
            connection.send(&ClientMessage::ListenerView { center, world_width: width })?;
            native.listener = Some((center, width));
        }
        let in_boneyard = self.session.boneyard.is_some();
        let pause = if in_boneyard {
            match self.panels.panel() {
                Some(PlayerPanel::Inventory) => Some(PauseSource::Inventory),
                Some(PlayerPanel::Skills) => Some(PauseSource::SkillBook),
                Some(PlayerPanel::SkillSelector(_)) => Some(PauseSource::SkillSelector),
                _ if self.menu.is_blocking() => Some(PauseSource::PauseMenu),
                _ => None,
            }
        } else {
            None
        };
        if pause != native.pause {
            if let Some(source) = pause.or(native.pause) {
                connection
                    .send(&ClientMessage::GameplayPause { paused: pause.is_some(), source })?;
            }
            native.pause = pause;
        }
        let activity = if in_boneyard {
            None
        } else if self.menu.is_blocking() {
            Some(HubActivity::Paused)
        } else if self.panels.is_open() || picker_open || cheats_open {
            Some(HubActivity::Occupied)
        } else {
            None
        };
        if activity != native.hub_activity {
            connection.send(&ClientMessage::HubActivity { activity })?;
            native.hub_activity = activity;
        }
        // Source MainMenuScene retries readiness for each accepted runtime snapshot.
        // A new hosted world can reuse a grace sequence on this same connection.
        if let Some(grace) = &connection.host.resume_grace
            && grace.remaining_ms.is_none()
            && in_boneyard
            && !picker_open
            && native.resume_ready_snapshot != Some(connection.snapshot_sequence)
        {
            connection.send(&ClientMessage::ResumeReady { sequence: grace.sequence })?;
            native.resume_ready_snapshot = Some(connection.snapshot_sequence);
        }
        let player = &self.session.participants[&self.local_player_id].player;
        let held = connection.host.pause.is_some()
            || connection.host.resume_grace.is_some()
            || connection.host.loading_run_id.is_some()
            || pause.is_some()
            || self.session.level_up_barrier.is_some()
            || picker_open
            || social_open
            || cheats_open
            || player.combat.life_state != solomon_sim::combat::PlayerLifeState::Alive
            || self.session.run.phase == GameRunPhase::GameOver;
        connection.send_input(self.session.ticks, input, held)
    }

    pub(super) fn native_gameplay_held(&self) -> bool {
        self.native.as_ref().is_some_and(|native| {
            native.phase != NativePhase::Playing
                || native.connection.as_ref().is_none_or(|connection| {
                    connection.host.pause.is_some()
                        || connection.host.resume_grace.is_some()
                        || connection.host.loading_run_id.is_some()
                })
        })
    }

    pub(super) fn native_ui_waiting(&self) -> bool {
        self.native.as_ref().is_some_and(|native| {
            native.pending_commands.values().any(|feedback| {
                matches!(
                    feedback,
                    CommandFeedback::CollegeIntro
                        | CommandFeedback::CollegeDialogue
                        | CommandFeedback::Loadout
                )
            })
        })
    }

    pub(super) fn native_modal_available(&self) -> bool {
        if self.is_hosted_observer() {
            return false;
        }
        self.native.as_ref().is_none_or(|native| {
            native.phase == NativePhase::Playing
                && native.connection.as_ref().is_some_and(|connection| {
                    connection.host.resume_grace.is_none()
                        && connection.host.loading_run_id.is_none()
                        && connection
                            .host
                            .pause
                            .as_ref()
                            .is_none_or(|pause| pause.owner_player_id == self.local_player_id)
                })
        })
    }

    pub(super) fn send_native_message(&self, message: &ClientMessage) -> Result<(), String> {
        self.native
            .as_ref()
            .and_then(|native| native.connection.as_ref())
            .ok_or("The game connection is closed")?
            .send(message)
    }

    pub(super) fn native_start_run(&mut self, tutorial: bool) -> Result<(), String> {
        let connection = self
            .native
            .as_ref()
            .and_then(|native| native.connection.as_ref())
            .ok_or("The game connection is closed")?;
        connection.send(&ClientMessage::StartRun { tutorial })?;
        self.begin_match_loading(MatchLoadingFlow::Boneyard, MatchLoadingStage::PreparingHost);
        Ok(())
    }
}
