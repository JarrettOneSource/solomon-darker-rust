//! Direct-join messages shared by the native authority and the WebAssembly client.
//!
//! This is the Rust port's own versioned protocol, not the Website's version 129 wire
//! format. Admission, input sequencing, pause and readiness follow `host/game-host.ts`;
//! gameplay payloads reuse the actual simulation types rather than a second player model.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use solomon_sim::Vec2;
use solomon_sim::audio::AudioEvent;
use solomon_sim::belt::NativeBeltCommand;
use solomon_sim::economy::{HubEconomyResult, HubInventoryAction};
use solomon_sim::profile::{NewGameSelection, PlayerConcentrationSlot, PlayerSkillChoice};
use solomon_sim::session::GameSessionInput;
use solomon_sim::social::{GameChatMessage, GameChatRejection, SocialAction, SocialSnapshot};
use solomon_sim::tutorial_surface::NativeTutorialSurfaceAction;

pub mod client_input;
pub mod flame_presentation;
pub mod hosted;
pub mod snapshot;
pub mod transfer;

pub const PROTOCOL_VERSION: u32 = 2;
pub const WEBSOCKET_PATH: &str = "/session";
pub const GAME_TICK_RATE: u32 = 100;
pub const SNAPSHOT_RATE: u32 = 20;
pub const MAX_SAVE_BYTES: usize = 96 * 1024 * 1024;
pub const GAME_WEBSOCKET_MAX_PAYLOAD_BYTES: usize = MAX_SAVE_BYTES * 2 + 64 * 1024;

/// A reconnect credential belongs to the host, never the simulation or its RNG streams.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeCredentials {
    pub player_id: String,
    pub token: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseSource {
    Inventory,
    PauseMenu,
    SkillBook,
    SkillSelector,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HubActivity {
    Paused,
    Occupied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeGraceReason {
    GameRejoined,
    GameRestarted,
    GameStarted,
    InventoryClosed,
    PauseMenuClosed,
    SkillBookClosed,
    SkillPickerClosed,
    PartyRejoinWait,
}

impl PauseSource {
    #[must_use]
    pub const fn resume_grace_reason(self) -> Option<ResumeGraceReason> {
        // `gameplay-resume-grace.ts`: closing a selector has no countdown.
        match self {
            Self::Inventory => Some(ResumeGraceReason::InventoryClosed),
            Self::PauseMenu => Some(ResumeGraceReason::PauseMenuClosed),
            Self::SkillBook => Some(ResumeGraceReason::SkillBookClosed),
            Self::SkillSelector => None,
        }
    }
}

/// Commands cross the reliable channel and execute against the same participant owner
/// as the next tick. IDs are requests; the session still checks range, phase and ownership.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameCommand {
    Inventory {
        action: HubInventoryAction,
    },
    ActivateBeltSlot {
        slot: usize,
        #[serde(default)]
        aim: Option<solomon_sim::Vec2>,
    },
    InteractGoodie,
    BindBelt {
        command: NativeBeltCommand,
    },
    SelectPrimary {
        skill_id: usize,
    },
    SelectConcentration {
        skill_id: usize,
    },
    SelectConcentrationSlot {
        skill_id: usize,
        slot: PlayerConcentrationSlot,
    },
    ChooseSkill {
        choice: PlayerSkillChoice,
    },
    RerollSkillOffer {
        offer_sequence: u64,
    },
    DeferSkillChoice {
        offer_sequence: u64,
    },
    ArmCollegeIntro,
    AcknowledgeCollegeIntroDialogue,
    DeclineTutorial,
    ConfirmLoadout {
        selection: NewGameSelection,
    },
    TutorialAction {
        action: NativeTutorialSurfaceAction,
    },
    ContinueGameOver {
        run_id: String,
        event_id: u64,
    },
    /// Retire the existing character and create its replacement through canonical
    /// profile hydration. The host rejects this while other players share an active run.
    ResetWizard {
        selection: NewGameSelection,
    },
    Social {
        action: SocialAction,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    Hosted {
        command: hosted::HostedCommand,
    },
    /// A one-shot title-screen probe. The host sends SessionStatus and closes cleanly;
    /// ordinary gameplay still starts with a fresh Hello connection and its deadline.
    InspectSession {
        resume: Option<ResumeCredentials>,
    },
    Hello {
        version: u32,
        selection: NewGameSelection,
        resume: Option<ResumeCredentials>,
        /// An existing standalone document may seed an empty host. It cannot replace a
        /// live shared session; later joins use host-owned credentials and participants.
        initial_save: Option<String>,
        begin_college_intro: bool,
        enhanced_effects: bool,
    },
    Input {
        sequence: u64,
        target_tick: u64,
        input: GameSessionInput,
    },
    SnapshotAck {
        sequence: u64,
    },
    WelcomeChunkAck {
        next_offset: usize,
    },
    SaveCheckpointChunkAck {
        sequence: u64,
        next_offset: usize,
    },
    /// Actual client camera for source audio gain. This presentation value never
    /// changes simulation visibility, aiming, collision or random streams.
    ListenerView {
        center: Vec2,
        world_width: f64,
    },
    ReadyBoneyard {
        run_id: String,
    },
    ResumeReady {
        sequence: u64,
    },
    CollegeIntroReady,
    HubActivity {
        activity: Option<HubActivity>,
    },
    GameplayPause {
        paused: bool,
        source: PauseSource,
    },
    Command {
        sequence: u64,
        command: GameCommand,
    },
    StartRun {
        tutorial: bool,
    },
    SaveBeforeLeave {
        request_id: u64,
    },
    Ping {
        nonce: u64,
    },
    Disconnect,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameplayPause {
    pub owner_player_id: String,
    pub owner_display_name: String,
    pub source: PauseSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameplayResumeGrace {
    pub reason: ResumeGraceReason,
    /// None means waiting for the named renderer/picker-ready acknowledgements.
    pub remaining_ms: Option<u64>,
    pub sequence: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostSnapshot {
    pub authority_player_id: Option<String>,
    pub connected_player_ids: Vec<String>,
    #[serde(default)]
    pub materializing_player_ids: Vec<String>,
    pub hub_activities: BTreeMap<String, HubActivity>,
    pub pause: Option<GameplayPause>,
    pub resume_grace: Option<GameplayResumeGrace>,
    pub loading_run_id: Option<String>,
}

/// Peers with the same retained presentation cursor share a borrowed world encoding.
/// Per-participant input ACKs require no personalized copy of the gameplay state.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotFrame<S> {
    pub sequence: u64,
    #[serde(default)]
    pub flame_presentation: Option<flame_presentation::FlamePresentationFrame>,
    pub input_acknowledgements: BTreeMap<String, u64>,
    pub host: HostSnapshot,
    pub snapshot: S,
}

/// The host supplies `GameSnapshot<'_>` (borrowed); the browser decodes `GameCheckpoint`
/// (owned). Their canonical field shape is identical, without a frame-time checkpoint clone.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerMessage<S> {
    /// Source chunks count UTF-16 units. A chunk may end between a surrogate pair;
    /// paired Rust endpoints carry those units losslessly until final assembly.
    WelcomeChunk {
        data: Vec<u16>,
        offset: usize,
        total_length: usize,
    },
    SessionStatus {
        version: u32,
        can_resume: bool,
        /// Present only when the caller supplied the current credential for that wizard.
        selection: Option<NewGameSelection>,
        fresh_join_available: bool,
    },
    Welcome {
        version: u32,
        credentials: ResumeCredentials,
        tick_rate: u32,
        snapshot_rate: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hosted: Option<hosted::HostedWelcome>,
        frame: SnapshotFrame<S>,
    },
    Hosted {
        message: hosted::HostedServerMessage,
    },
    /// Actual retained run geometry, reliably published before a snapshot needs it.
    /// Imported/authored scenes cannot be reconstructed from a stock seed or catalog.
    BoneyardLoaded {
        boneyard: Box<solomon_sim::boneyard::LoadedBoneyard>,
    },
    Snapshot {
        frame: SnapshotFrame<S>,
    },
    /// Tick-owned one-shots cannot use replaceable snapshots: a slow renderer still
    /// receives every admitted cue once, in order. Continuous loops project from state.
    Audio {
        tick: u64,
        events: Vec<AudioEvent>,
    },
    Social {
        snapshot: SocialSnapshot,
    },
    Chat {
        message: GameChatMessage,
    },
    ChatRejected {
        rejection: GameChatRejection,
    },
    CommandResult {
        sequence: u64,
        /// Release feedback after accepting this frame or a newer complete frame.
        /// Replaceable snapshots may supersede the first publication of the change.
        snapshot_sequence: u64,
        accepted: bool,
        reason: Option<String>,
        /// Actual transaction feedback, including Unforge outcomes and dowsing pitch.
        /// Other command families leave this absent.
        inventory_result: Option<HubEconomyResult>,
    },
    SaveCommitted {
        request_id: u64,
        tick: u64,
    },
    Pong {
        nonce: u64,
    },
    Error {
        code: HostErrorCode,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostErrorCode {
    InvalidMessage,
    UnsupportedVersion,
    HostFull,
    InvalidCredentials,
    RunInProgress,
    CommandRejected,
    SaveFailed,
    ServerError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untrusted_envelopes_reject_unknown_fields_and_preserve_canonical_commands() {
        let text = r#"{"type":"command","sequence":7,"command":{"kind":"bind_belt","command":{"BindSkill":{"skill_id":82,"slot":2}}}}"#;
        let message: ClientMessage = serde_json::from_str(text).unwrap();
        let ClientMessage::Command { sequence: 7, command } = message else {
            panic!("the canonical belt command must survive the shared envelope");
        };
        assert!(matches!(
            command,
            GameCommand::BindBelt {
                command: NativeBeltCommand::BindSkill { skill_id: Some(82), slot: 2 }
            }
        ));
        let invalid = text.replace("\"sequence\":7", "\"sequence\":7,\"player_id\":\"other\"");
        assert!(serde_json::from_str::<ClientMessage>(&invalid).is_err());
        assert!(
            serde_json::from_str::<ClientMessage>(
                r#"{"type":"input","sequence":-1,"target_tick":0,"input":{}}"#
            )
            .is_err()
        );

        let interact = r#"{"type":"command","sequence":8,"command":{"kind":"interact_goodie"}}"#;
        assert!(matches!(
            serde_json::from_str::<ClientMessage>(interact).unwrap(),
            ClientMessage::Command { command: GameCommand::InteractGoodie, .. }
        ));
        let transaction = HubEconomyResult {
            accepted: true,
            reason: None,
            dowsing_pitch: Some(1.125),
            dye_session: None,
            unforge_outcome: Some(solomon_sim::economy::NativeUnforgeOutcome {
                amount: Some(3.5),
                item_name: "Ring".into(),
                kind: solomon_sim::economy::NativeUnforgeOutcomeKind::MaximumMana,
            }),
        };
        let message = ServerMessage::<()>::CommandResult {
            sequence: 9,
            snapshot_sequence: 14,
            accepted: true,
            reason: None,
            inventory_result: Some(transaction.clone()),
        };
        let encoded = serde_json::to_string(&message).unwrap();
        let ServerMessage::<()>::CommandResult { inventory_result, .. } =
            serde_json::from_str(&encoded).unwrap()
        else {
            panic!("a command receipt must retain its canonical transaction");
        };
        assert_eq!(inventory_result, Some(transaction));
    }
}
