//! The asynchronous Source `host/ml-bot-host-controller.ts` state machine.
//!
//! The host owns model execution and real participant admission. This controller
//! borrows one shared Session and returns the same intents as the Source adapter;
//! it never steps a second player or retains a copy of the world.

use crate::game_run::GameRunPhase;
use crate::profile::NewGameSelection;
use crate::session::GameSession;

use super::{
    BoneyardEntranceNavigator, ML_BOT_DECISION_INTERVAL_TICKS, MlBotEntranceNavigationError,
    MlBotHostIntent, MlBotPolicyActionContext, MlBotPolicyActionIndices, MlBotPolicyActionMaskPlan,
    MlBotPolicyChoiceMode, MlBotPolicyFrame, MlBotPolicyObservationContext, MlBotPolicyObserver,
    choose_ml_bot_policy_scripted_skill, describe_ml_bot_policy_skill_offer,
};

pub struct MlBotHostControllerContext<'a> {
    pub session: &'a GameSession,
    pub observation: MlBotPolicyObservationContext<'a>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MlBotHostControllerError {
    Policy(String),
    /// Points remain typed so the native host can use its existing ECMAScript
    /// JSON writer when reproducing the Source navigation error text.
    Entrance(MlBotEntranceNavigationError),
}

impl From<String> for MlBotHostControllerError {
    fn from(error: String) -> Self {
        Self::Policy(error)
    }
}

impl From<MlBotEntranceNavigationError> for MlBotHostControllerError {
    fn from(error: MlBotEntranceNavigationError) -> Self {
        Self::Entrance(error)
    }
}

/// One owned worker request. Observation/descriptors are small numeric buffers;
/// the immutable checkpoint is shared separately by the host.
#[derive(Clone, Debug)]
pub enum MlBotHostInferenceRequest {
    Actions { id: u64, observation: Vec<f32>, plan: MlBotPolicyActionMaskPlan },
    Choice { id: u64, observation: Vec<f32>, descriptors: Vec<f32>, mask: Vec<u8> },
}

impl MlBotHostInferenceRequest {
    #[must_use]
    pub const fn id(&self) -> u64 {
        match self {
            Self::Actions { id, .. } | Self::Choice { id, .. } => *id,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MlBotHostInferenceResult {
    Actions(MlBotPolicyActionIndices),
    Choice(usize),
}

#[derive(Clone, Debug, Default)]
pub struct MlBotHostPoll {
    pub intent: Option<MlBotHostIntent>,
    pub request: Option<MlBotHostInferenceRequest>,
}

struct PendingAction {
    context: MlBotPolicyActionContext,
    frame: MlBotPolicyFrame,
}

enum PendingKind {
    Actions(Box<PendingAction>),
    Choice { generation: u64 },
}

struct PendingInference {
    id: u64,
    world_key: String,
    kind: PendingKind,
}

pub struct MlBotHostController {
    character: NewGameSelection,
    choice_mode: MlBotPolicyChoiceMode,
    dispatched_choice_generation: Option<u64>,
    entrance: BoneyardEntranceNavigator,
    failed: bool,
    loadout_key: Option<String>,
    next_decision_tick: u64,
    next_request_id: u64,
    observer: MlBotPolicyObserver,
    pending: Option<PendingInference>,
    player_id: String,
    world_key: Option<String>,
}

impl MlBotHostController {
    pub fn new(
        player_id: String,
        character: NewGameSelection,
        choice_mode: MlBotPolicyChoiceMode,
    ) -> Result<Self, String> {
        Ok(Self {
            character,
            choice_mode,
            dispatched_choice_generation: None,
            entrance: BoneyardEntranceNavigator::default(),
            failed: false,
            loadout_key: None,
            next_decision_tick: 0,
            next_request_id: 1,
            observer: MlBotPolicyObserver::new(player_id.clone())?,
            pending: None,
            player_id,
            world_key: None,
        })
    }

    pub fn poll(
        &mut self,
        context: Option<MlBotHostControllerContext<'_>>,
    ) -> Result<MlBotHostPoll, MlBotHostControllerError> {
        let mut output = MlBotHostPoll::default();
        if self.failed {
            return Ok(output);
        }
        let Some(context) = context else {
            return Ok(output);
        };
        let session = context.session;
        let world_key = policy_world_key(session);
        if self.world_key.as_ref() != Some(&world_key) {
            self.world_key = Some(world_key.clone());
            self.observer.reset();
            self.next_decision_tick = session.ticks;
            self.loadout_key = None;
            self.dispatched_choice_generation = None;
            // Source does not cancel an in-flight Promise here. Its completion
            // checks the current world key and finally releases the pending slot.
        }
        if session.boneyard.is_none() {
            return Ok(output);
        }
        if session.run.phase == GameRunPhase::Loadout {
            let key = session.run.last_completed_run_id.as_deref().unwrap_or("post-run-loadout");
            if self.loadout_key.as_deref() != Some(key) {
                self.loadout_key = Some(key.to_owned());
                output.intent = Some(MlBotHostIntent::ConfirmLoadout(self.character.clone()));
            }
            return Ok(output);
        }
        if let Some(input) = self.entrance.input(session, &self.player_id)? {
            output.intent = Some(MlBotHostIntent::ScriptedInput(input));
            return Ok(output);
        }
        if let Some(offer) = describe_ml_bot_policy_skill_offer(session, &self.player_id)? {
            if self.dispatched_choice_generation == Some(offer.generation) {
                return Ok(output);
            }
            if self.choice_mode == MlBotPolicyChoiceMode::Learned {
                if self.pending.is_some() {
                    return Ok(output);
                }
                let frame = self.observe(&context)?;
                let id = self.allocate_request_id();
                self.pending = Some(PendingInference {
                    id,
                    world_key,
                    kind: PendingKind::Choice { generation: offer.generation },
                });
                output.request = Some(MlBotHostInferenceRequest::Choice {
                    id,
                    observation: frame.values,
                    descriptors: offer.descriptors,
                    mask: offer.mask,
                });
                return Ok(output);
            }
            let Some(choice) = choose_ml_bot_policy_scripted_skill(session, &self.player_id)?
            else {
                self.failed = true;
                return Err(MlBotHostControllerError::Policy(
                    "scripted ML bot could not resolve a pending skill offer".into(),
                ));
            };
            self.dispatched_choice_generation = Some(choice.offer_sequence);
            output.intent = Some(MlBotHostIntent::SelectSkill {
                choice_index: choice.choice_index,
                offer_sequence: choice.offer_sequence,
                skill_id: choice.skill_id,
            });
            return Ok(output);
        }
        self.dispatched_choice_generation = None;
        if session.run.phase != GameRunPhase::Active
            || self.pending.is_some()
            || session.ticks < self.next_decision_tick
        {
            return Ok(output);
        }
        let frame = self.observe(&context)?;
        let action_context = MlBotPolicyActionContext::capture(session, &self.player_id)?;
        let plan = action_context.masks(&frame)?;
        let id = self.allocate_request_id();
        output.request = Some(MlBotHostInferenceRequest::Actions {
            id,
            observation: frame.values.clone(),
            plan,
        });
        self.pending = Some(PendingInference {
            id,
            world_key,
            kind: PendingKind::Actions(Box::new(PendingAction { context: action_context, frame })),
        });
        Ok(output)
    }

    /// Complete exactly one worker operation. A stale/duplicate token cannot
    /// consume a newer request belonging to this controller.
    pub fn complete(
        &mut self,
        request_id: u64,
        current: Option<&GameSession>,
        result: Result<MlBotHostInferenceResult, String>,
    ) -> Result<Option<MlBotHostIntent>, MlBotHostControllerError> {
        if self.pending.as_ref().is_none_or(|pending| pending.id != request_id) {
            return Ok(None);
        }
        let pending = self.pending.take().expect("the matching request was checked above");
        let result = match result {
            Ok(result) => result,
            Err(error) => return self.fail(error),
        };
        let Some(current) = current else {
            return Ok(None);
        };
        if policy_world_key(current) != pending.world_key {
            return Ok(None);
        }
        match (pending.kind, result) {
            (PendingKind::Actions(action), MlBotHostInferenceResult::Actions(indices)) => {
                // The Source Promise closes over the request-time state/frame.
                // Only admission and the next deadline use the current Session.
                let decision = match action.context.resolve(&action.frame, indices) {
                    Ok(decision) => decision,
                    Err(error) => return self.fail(error),
                };
                self.observer.commit(decision.committed_action, decision.target_id);
                self.next_decision_tick = current.ticks + ML_BOT_DECISION_INTERVAL_TICKS;
                Ok(Some(match decision.hub_action {
                    Some(action) => MlBotHostIntent::HubAction(action),
                    None => MlBotHostIntent::Input(decision.input),
                }))
            }
            (PendingKind::Choice { generation }, MlBotHostInferenceResult::Choice(index)) => {
                let offer = match describe_ml_bot_policy_skill_offer(current, &self.player_id) {
                    Ok(offer) => offer,
                    Err(error) => return self.fail(error),
                };
                let Some(offer) = offer else {
                    return Ok(None);
                };
                if offer.generation != generation
                    || offer.mask.get(index) != Some(&1)
                    || index >= offer.option_ids.len()
                {
                    return Ok(None);
                }
                self.dispatched_choice_generation = Some(generation);
                Ok(Some(MlBotHostIntent::SelectSkill {
                    choice_index: index,
                    offer_sequence: generation,
                    skill_id: offer.option_ids[index],
                }))
            }
            _ => self.fail("ML bot worker returned a result for the wrong request kind".into()),
        }
    }

    fn observe(
        &mut self,
        context: &MlBotHostControllerContext<'_>,
    ) -> Result<MlBotPolicyFrame, String> {
        self.observer.observe(
            context.session,
            &MlBotPolicyObservationContext {
                active_inputs: context.observation.active_inputs,
                controllers: context.observation.controllers,
                // The generic observer supports mod consumables. The pinned ML
                // host does not supply that callback, so its controller does not.
                has_consumable: None,
                compare_player_ids: context.observation.compare_player_ids,
            },
        )
    }

    fn allocate_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    fn fail(&mut self, error: String) -> Result<Option<MlBotHostIntent>, MlBotHostControllerError> {
        if self.failed {
            return Ok(None);
        }
        self.failed = true;
        Err(MlBotHostControllerError::Policy(error))
    }
}

fn policy_world_key(session: &GameSession) -> String {
    match &session.boneyard {
        Some(world) => format!("boneyard:{}", world.loaded.run_id),
        None => "hub".to_owned(),
    }
}
