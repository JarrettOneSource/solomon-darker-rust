//! Reliable commands call the canonical participant API once. Source counterparts:
//! `host/game-host.ts` command branches and `core-server/game-simulation.ts` commands.

use solomon_protocol::{GameCommand, HostErrorCode, PauseSource, ResumeGraceReason, ServerMessage};
use solomon_sim::belt::NativeBeltCommand;
use solomon_sim::economy::{HubEconomyResult, HubInventoryAction};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::modding::extensions::GameSimulationExtensions;
use solomon_sim::modding::inventory::GameSimulationModConsumption;
use solomon_sim::session::GameSession;
use solomon_sim::social::SocialAction;

use super::admission::validate_selection;
use super::authority::{NativeAuthority, encode, send_reliable};
use super::connection::HostConnection;
use super::control::GameplayControl;

/// Both native front doors use the same command-to-session mapping. Admission,
/// reliable receipts and party membership remain owned by their respective hosts.
pub(super) struct SessionCommandResult {
    pub accepted: bool,
    pub inventory_result: Option<HubEconomyResult>,
    pub mod_consumption: Option<GameSimulationModConsumption>,
}

impl NativeAuthority {
    pub(super) fn apply_command(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        sequence: u64,
        command: GameCommand,
        now_ms: f64,
    ) -> Result<(), (HostErrorCode, String)> {
        if sequence == 0 {
            return Err((HostErrorCode::InvalidMessage, "Command sequences start at one".into()));
        }
        if sequence <= connection.last_command_sequence {
            if sequence == connection.last_command_sequence
                && let Some(receipt) = &connection.last_command_receipt
            {
                send_reliable(&connection.peer, receipt.clone()).map_err(server_error)?;
            }
            return Ok(());
        }
        let previous_barrier = self.session.as_ref().and_then(|session| {
            session.level_up_barrier.as_ref().map(|barrier| barrier.barrier_id)
        });
        let previous_world = self
            .session
            .as_ref()
            .and_then(|session| session.boneyard.as_ref().map(|world| world.loaded.run_id.clone()));
        let mut inventory_result: Option<HubEconomyResult> = None;
        let mut reason = None;
        let social_command = matches!(&command, GameCommand::Social { .. });
        let staged_command = self.staged_detached(id).is_some();
        let accepted = if !surface_allows_command(&self.control, id, &command, now_ms) {
            false
        } else if staged_command {
            let accepted = self.apply_detached_command(id, command).map_err(server_error)?;
            if accepted {
                self.finish_staged_rejoin(id).map_err(server_error)?;
            }
            accepted
        } else {
            match command {
                GameCommand::Social { action } => {
                    let (accepted, rejection) =
                        self.apply_social_action(connection, id, action, now_ms)?;
                    reason = rejection;
                    accepted
                }
                GameCommand::ResetWizard { selection } => {
                    validate_selection(&selection)?;
                    let session = self.session.as_mut().expect("an admitted wizard has a session");
                    let shared_active = self
                        .metadata
                        .party
                        .as_ref()
                        .is_some_and(|party| party.member_player_ids.len() > 1)
                        && (session.boneyard.is_some() || session.run.phase != GameRunPhase::Hub);
                    if shared_active {
                        reason =
                            Some("Return to the College before replacing a party wizard".into());
                        false
                    } else if session.reset_participant(id, selection) {
                        session.arm_college_intro(id);
                        self.control.college_intro_ready.remove(id);
                        connection.hub_activity = None;
                        connection.combat_audio = None;
                        true
                    } else {
                        false
                    }
                }
                command => {
                    let session = self.session.as_mut().expect("an admitted wizard has a session");
                    let outcome = apply_session_command(session, id, command)?;
                    inventory_result = outcome.inventory_result;
                    outcome.accepted
                }
            }
        };
        if let Some(transaction) = &inventory_result {
            reason = transaction.reason.map(|reason| reason.as_str().to_owned());
        } else if !accepted && reason.is_none() {
            reason = Some("This action is unavailable right now".into());
        }

        // Keep the entire receipt: retrying an Unforge or purchase must return its real
        // outcome without consuming a second item or repeating any native random draw.
        let receipt = encode(&ServerMessage::<()>::CommandResult {
            sequence,
            snapshot_sequence: self.next_snapshot_sequence,
            accepted,
            reason,
            inventory_result,
        })
        .map_err(server_error)?;
        connection.last_command_sequence = sequence;
        connection.last_command_receipt = Some(receipt.clone());
        send_reliable(&connection.peer, receipt).map_err(server_error)?;
        self.publish_dirty = true;
        if !accepted || social_command {
            return Ok(());
        }

        connection.input.neutralize();
        let next_world = self.session.as_ref().and_then(|session| {
            session.boneyard.as_ref().map(|world| world.loaded.run_id.as_str())
        });
        if previous_world.as_deref() != next_world {
            self.navigation = None;
            self.control = super::control::GameplayControl::default();
            self.retire_offline_run_slots().map_err(server_error)?;
            self.neutralize_inputs();
        }
        let facts = self.control_facts();
        if previous_barrier.is_some() && !facts.context().skill_barrier {
            self.control.begin_surface_grace(
                id,
                ResumeGraceReason::SkillPickerClosed,
                &facts.context(),
                now_ms,
            );
            self.neutralize_inputs();
        }
        self.changed();
        self.require_checkpoint();
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(())
    }
}

/// Apply only player-domain commands. Host-owned reset and social operations must be
/// resolved against the actual party/world before reaching this boundary.
pub(super) fn apply_session_command(
    session: &mut GameSession,
    id: &str,
    command: GameCommand,
) -> Result<SessionCommandResult, (HostErrorCode, String)> {
    apply_session_command_with_extensions(session, id, command, None)
}

pub(super) fn apply_session_command_with_extensions(
    session: &mut GameSession,
    id: &str,
    command: GameCommand,
    extensions: Option<&dyn GameSimulationExtensions>,
) -> Result<SessionCommandResult, (HostErrorCode, String)> {
    let mut inventory_result: Option<HubEconomyResult> = None;
    let mut mod_consumption = None;
    let accepted = match command {
        GameCommand::Inventory { action } => {
            let result = session
                .apply_inventory_with_extensions(id, &action, extensions)
                .map_err(server_error)?;
            mod_consumption = result.mod_consumption;
            inventory_result = Some(result.transaction);
            inventory_result.as_ref().unwrap().accepted
        }
        GameCommand::ActivateBeltSlot { slot, aim } => {
            let result = session
                .activate_belt_slot_with_extensions(id, slot, aim, extensions)
                .map_err(server_error)?;
            mod_consumption = result.mod_consumption;
            inventory_result = Some(result.transaction);
            inventory_result.as_ref().unwrap().accepted
        }
        GameCommand::InteractGoodie => {
            inventory_result = Some(session.interact_goodie(id).transaction);
            inventory_result.as_ref().unwrap().accepted
        }
        GameCommand::BindBelt { command } => session.bind_belt(id, command),
        GameCommand::SelectPrimary { skill_id } => session.select_primary(id, skill_id),
        GameCommand::SelectConcentration { skill_id } => session.select_concentration(id, skill_id),
        GameCommand::SelectConcentrationSlot { skill_id, slot } => {
            session.select_concentration_slot(id, skill_id, slot)
        }
        GameCommand::ChooseSkill { choice } => session.choose_skill(id, choice),
        GameCommand::RerollSkillOffer { offer_sequence } => {
            session.reroll_skill_offer(id, offer_sequence)
        }
        GameCommand::DeferSkillChoice { offer_sequence } => {
            session.defer_skill_choice(id, offer_sequence)
        }
        GameCommand::ArmCollegeIntro => session.arm_college_intro(id),
        GameCommand::AcknowledgeCollegeIntroDialogue => {
            session.acknowledge_college_intro_dialogue(id)
        }
        GameCommand::DeclineTutorial => session.decline_tutorial(id),
        GameCommand::ConfirmLoadout { selection } => {
            validate_selection(&selection)?;
            session.confirm_loadout(id, selection)
        }
        GameCommand::TutorialAction { action } => session.apply_tutorial_action(action),
        GameCommand::ContinueGameOver { run_id, event_id } => {
            session.continue_game_over(&run_id, event_id)
        }
        GameCommand::ResetWizard { .. } | GameCommand::Social { .. } => {
            unreachable!("host-owned admission commands are handled above")
        }
    };
    Ok(SessionCommandResult { accepted, inventory_result, mod_consumption })
}

/// Network holds are outside `GameSession`. Preserve the source resume-grace
/// admission branch, `pauseAllowsInventoryAction` and selection permissions here.
pub(super) fn surface_allows_command(
    control: &GameplayControl,
    id: &str,
    command: &GameCommand,
    now_ms: f64,
) -> bool {
    if control.resume_grace(now_ms).is_some()
        && !matches!(
            command,
            GameCommand::ChooseSkill { .. } | GameCommand::RerollSkillOffer { .. }
            | GameCommand::DeferSkillChoice { .. }
            | GameCommand::Social { action: SocialAction::SendChat { .. } }
            // Native lifecycle commands must still retire a solo game or report
            // the real Tutorial surface while its authoritative clock is held.
            | GameCommand::ResetWizard { .. } | GameCommand::TutorialAction { .. }
        )
    {
        return false;
    }
    let Some(pause) = control.pause() else { return true };
    if matches!(
        command,
        GameCommand::Social { .. }
            | GameCommand::ChooseSkill { .. }
            | GameCommand::RerollSkillOffer { .. }
            | GameCommand::DeferSkillChoice { .. }
            | GameCommand::TutorialAction { .. }
    ) {
        return true;
    }
    if pause.owner_player_id != id {
        return false;
    }
    match command {
        GameCommand::Inventory { action } => {
            pause.source == PauseSource::Inventory
                && matches!(
                    action,
                    HubInventoryAction::Consume { .. }
                        | HubInventoryAction::OpenDye { .. }
                        | HubInventoryAction::CloseDye { .. }
                        | HubInventoryAction::Dye { .. }
                        | HubInventoryAction::Equip { .. }
                        | HubInventoryAction::Move { .. }
                        | HubInventoryAction::ReadSkillBook { .. }
                        | HubInventoryAction::RenameSack { .. }
                        | HubInventoryAction::Unequip { .. }
                        | HubInventoryAction::Unforge { .. }
                )
        }
        GameCommand::ActivateBeltSlot { .. } => pause.source == PauseSource::Inventory,
        GameCommand::BindBelt { command: NativeBeltCommand::BindItem { .. } } => {
            pause.source == PauseSource::Inventory
        }
        GameCommand::BindBelt { command: NativeBeltCommand::BindSkill { skill_id, .. } } => {
            pause.source == PauseSource::SkillBook
                || (pause.source == PauseSource::Inventory && skill_id.is_none())
        }
        GameCommand::SelectPrimary { .. } => {
            matches!(pause.source, PauseSource::SkillBook | PauseSource::SkillSelector)
        }
        GameCommand::SelectConcentration { .. } => pause.source == PauseSource::SkillBook,
        GameCommand::SelectConcentrationSlot { .. } => pause.source == PauseSource::SkillSelector,
        // Native New Game is an explicit authority command replacing this solo wizard.
        GameCommand::ResetWizard { .. } => true,
        _ => false,
    }
}

fn server_error(reason: String) -> (HostErrorCode, String) {
    (HostErrorCode::ServerError, reason)
}
