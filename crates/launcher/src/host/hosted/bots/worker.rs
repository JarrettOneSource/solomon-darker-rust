//! Source's single inference worker. Jobs contain only captured numeric policy
//! inputs; the authority keeps every controller and the sole live Session.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use solomon_sim::ml_bot::{
    MlBotHostInferenceRequest, MlBotHostInferenceResult, MlBotPolicyChoiceMode, MlBotPolicyRuntime,
    MlBotPolicySelectionOptions,
};

pub(super) struct InferenceJob {
    pub player_id: String,
    pub request: MlBotHostInferenceRequest,
}

pub(super) struct InferenceCompletion {
    pub player_id: String,
    pub request_id: u64,
    pub result: Result<MlBotHostInferenceResult, String>,
}

pub(super) struct BotInferenceWorker {
    pub choice_mode: MlBotPolicyChoiceMode,
    requests: Option<Sender<InferenceJob>>,
    completions: Receiver<InferenceCompletion>,
    thread: Option<JoinHandle<()>>,
}

impl BotInferenceWorker {
    pub fn new(policy: Arc<MlBotPolicyRuntime>) -> Result<Self, String> {
        let (requests, incoming) = mpsc::channel::<InferenceJob>();
        let (completed, completions) = mpsc::channel();
        let choice_mode = policy.choice_mode;
        let thread = thread::Builder::new()
            .name("solomon-bot-policy".into())
            .spawn(move || {
                for job in incoming {
                    let request_id = job.request.id();
                    let options = MlBotPolicySelectionOptions::default();
                    let result = match job.request {
                        MlBotHostInferenceRequest::Actions { observation, plan, .. } => policy
                            .infer_autoregressive(&observation, &plan, options, None)
                            .map(|output| MlBotHostInferenceResult::Actions(output.head.actions)),
                        MlBotHostInferenceRequest::Choice {
                            observation,
                            descriptors,
                            mask,
                            ..
                        } => policy
                            .choose(&observation, &descriptors, &mask, options, None)
                            .map(|output| MlBotHostInferenceResult::Choice(output.selected_option)),
                    };
                    if completed
                        .send(InferenceCompletion { player_id: job.player_id, request_id, result })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Cannot start the ML bot policy worker: {error}"))?;
        Ok(Self { choice_mode, requests: Some(requests), completions, thread: Some(thread) })
    }

    pub fn submit(&self, job: InferenceJob) -> Result<(), String> {
        self.requests
            .as_ref()
            .expect("the retained worker owns its sender")
            .send(job)
            .map_err(|_| "ML bot policy worker closed before the request was sent".into())
    }

    pub fn drain(&self) -> Vec<InferenceCompletion> {
        self.completions.try_iter().collect()
    }
}

impl Drop for BotInferenceWorker {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
