//! Match-loading lifecycle presentation from Website `match-loading.ts`.
//! The session coordinator supplies stages and time; this state never advances a load.

pub const MATCH_LOADING_PRESENTATION_DELAY_MS: f64 = 150.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchLoadingFlow {
    Boneyard,
    Hub,
}

impl MatchLoadingFlow {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Boneyard => "boneyard",
            Self::Hub => "hub",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchLoadingStage {
    ConnectingTransport,
    AuthenticatingSession,
    EstablishingRoute,
    SynchronizingHostSettings,
    ReceivingHostCheckpoint,
    PreparingHost,
    ReceivingRunPlan,
    PreparingBoneyard,
    GeneratingBoneyard,
    SerializingBoneyard,
    ReadingBoneyard,
    MaterializingWorld,
    ReceivingWorldCheckpoint,
    ReceivingWaveCheckpoint,
    MaterializingParticipants,
    WaitingForParticipants,
    ConfirmingParticipants,
    GameplayReady,
}

impl MatchLoadingStage {
    pub const ALL: [Self; 18] = [
        Self::ConnectingTransport,
        Self::AuthenticatingSession,
        Self::EstablishingRoute,
        Self::SynchronizingHostSettings,
        Self::ReceivingHostCheckpoint,
        Self::PreparingHost,
        Self::ReceivingRunPlan,
        Self::PreparingBoneyard,
        Self::GeneratingBoneyard,
        Self::SerializingBoneyard,
        Self::ReadingBoneyard,
        Self::MaterializingWorld,
        Self::ReceivingWorldCheckpoint,
        Self::ReceivingWaveCheckpoint,
        Self::MaterializingParticipants,
        Self::WaitingForParticipants,
        Self::ConfirmingParticipants,
        Self::GameplayReady,
    ];

    #[must_use]
    pub const fn definition(self) -> MatchLoadingStageDefinition {
        use MatchLoadingStage as Stage;
        match self {
            Stage::ConnectingTransport => MatchLoadingStageDefinition::new(
                "connecting_transport",
                "Waking the multiplayer transport...",
                0.44,
            ),
            Stage::AuthenticatingSession => MatchLoadingStageDefinition::new(
                "authenticating_session",
                "Proving your sigil to the host...",
                0.52,
            ),
            Stage::EstablishingRoute => {
                MatchLoadingStageDefinition::new("establishing_route", "Opening the route...", 0.56)
            }
            Stage::SynchronizingHostSettings => MatchLoadingStageDefinition::new(
                "synchronizing_host_settings",
                "Receiving the host's settings...",
                0.60,
            ),
            Stage::ReceivingHostCheckpoint => MatchLoadingStageDefinition::new(
                "receiving_host_checkpoint",
                "Receiving the host's checkpoint...",
                0.66,
            ),
            Stage::PreparingHost => {
                MatchLoadingStageDefinition::new("preparing_host", "Preparing the host...", 0.66)
            }
            Stage::ReceivingRunPlan => MatchLoadingStageDefinition::new(
                "receiving_run_plan",
                "Receiving the host's boneyard...",
                0.70,
            ),
            Stage::PreparingBoneyard => MatchLoadingStageDefinition::new(
                "preparing_boneyard",
                "Preparing the boneyard...",
                0.73,
            ),
            Stage::GeneratingBoneyard => MatchLoadingStageDefinition::new(
                "generating_boneyard",
                "Raising the boneyard...",
                0.77,
            ),
            Stage::SerializingBoneyard => MatchLoadingStageDefinition::new(
                "serializing_boneyard",
                "Sealing the boneyard...",
                0.80,
            ),
            Stage::ReadingBoneyard => MatchLoadingStageDefinition::new(
                "reading_boneyard",
                "Loading the boneyard...",
                0.83,
            ),
            Stage::MaterializingWorld => MatchLoadingStageDefinition::new(
                "materializing_world",
                "Awakening the world...",
                0.87,
            ),
            Stage::ReceivingWorldCheckpoint => MatchLoadingStageDefinition::new(
                "receiving_world_checkpoint",
                "Receiving the living world...",
                0.90,
            ),
            Stage::ReceivingWaveCheckpoint => MatchLoadingStageDefinition::new(
                "receiving_wave_checkpoint",
                "Aligning the host's wave...",
                0.91,
            ),
            Stage::MaterializingParticipants => MatchLoadingStageDefinition::new(
                "materializing_participants",
                "Gathering the coven...",
                0.92,
            ),
            Stage::WaitingForParticipants => MatchLoadingStageDefinition::new(
                "waiting_for_participants",
                "Waiting for the coven...",
                0.95,
            ),
            Stage::ConfirmingParticipants => MatchLoadingStageDefinition::new(
                "confirming_participants",
                "Binding the coven...",
                0.98,
            ),
            Stage::GameplayReady => {
                MatchLoadingStageDefinition::new("gameplay_ready", "Entering the boneyard...", 1.00)
            }
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stage| stage.definition().id == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchLoadingStageDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub progress: f64,
}

impl MatchLoadingStageDefinition {
    const fn new(id: &'static str, label: &'static str, progress: f64) -> Self {
        Self { id, label, progress }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchLoadingState {
    pub flow: MatchLoadingFlow,
    pub stage: MatchLoadingStage,
    pub started_at_ms: f64,
    pub progress: f64,
}

impl MatchLoadingState {
    #[must_use]
    pub const fn begin(
        flow: MatchLoadingFlow,
        stage: MatchLoadingStage,
        started_at_ms: f64,
    ) -> Self {
        Self { flow, stage, started_at_ms, progress: stage.definition().progress }
    }

    pub fn advance(&mut self, stage: MatchLoadingStage) {
        let definition = stage.definition();
        // Two authored stages share .66. Source retains the first label at equal progress.
        if definition.progress <= self.progress {
            return;
        }
        self.stage = stage;
        self.progress = definition.progress;
    }

    pub fn complete(&mut self) {
        self.advance(MatchLoadingStage::GameplayReady);
    }

    #[must_use]
    pub fn should_present(&self, now_ms: f64) -> bool {
        now_ms - self.started_at_ms >= MATCH_LOADING_PRESENTATION_DELAY_MS
    }

    #[must_use]
    pub const fn label(&self) -> &'static str {
        self.stage.definition().label
    }
}
