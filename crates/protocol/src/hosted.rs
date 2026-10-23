//! Hosted admission and control-plane messages.
//!
//! Source contracts: Website `protocol/game-client-messages.ts`,
//! `protocol/game-server-messages.ts` and `host/game-session-supervisor.ts`.
//! The paired Rust endpoints retain this crate's wire spelling while preserving the
//! source fields. Account privileges and content always come from a claimed host ticket.

use serde::{Deserialize, Serialize};
use solomon_sim::boneyard::BoneyardChoice;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::identity::GameContentManifest;
use solomon_sim::modding::inventory::ModConsumableCatalogEntry;
use solomon_sim::modding::value::{LuaObject, LuaValue};
use solomon_sim::profile::NewGameSelection;
use solomon_sim::social::{
    GameCollegeInvitation, GameOnlinePreferences, GamePlayerCardProfile, GameSessionKind,
    PartyActionRejection, PlayerSocialProfile,
};

pub const GAME_HUB_PATH: &str = "/game-hub";
pub const GAME_SESSION_PATH_PREFIX: &str = "/game-sessions/";
/// Publication identities are independent of the signed game recovery revision.
pub const DEPLOYMENT_PROTOCOL_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameSaveIntent {
    NewGame,
    Resume,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedHello {
    pub version: u32,
    pub credential: String,
    pub selection: NewGameSelection,
    pub profile: PlayerSocialProfile,
    pub online_preferences: GameOnlinePreferences,
    pub resume_token: Option<String>,
    /// The actual Website schema-37 document, including its profile and content metadata.
    pub save: Option<String>,
    pub save_intent: Option<GameSaveIntent>,
    pub begin_college_intro: bool,
    pub decline_tutorial: bool,
    pub cheats_enabled: bool,
    pub enhanced_effects: bool,
    pub allow_mod_mismatch: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedWelcome {
    pub session_kind: GameSessionKind,
    pub developer_access: bool,
    pub cheats_enabled: bool,
    pub observer: bool,
    pub content: GameContentManifest,
    pub mod_assets: Vec<GameModAsset>,
    pub mod_catalog: Vec<ModConsumableCatalogEntry>,
    pub boneyards: Vec<BoneyardChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostedCommand {
    Hello { hello: HostedHello },
    ObserverHello { version: u32, credential: String },
    OnlinePreferences { preferences: GameOnlinePreferences },
    RequestPlayerCard { request_id: u64, player_reference: String },
    InviteCollege { player_reference: String },
    DismissCollegeInvitation { invitation_id: String },
    StartMatch { boneyard_id: String },
    SetCheats { enabled: bool },
    SetEnhancedEffects { enabled: bool },
    LuaExecute { request_id: u64, code: String },
    DeploymentReady { checkpoint_sequence: u64, target_revision: String },
    ModAction { action: ModAction, arguments: LuaObject, request_id: u64, target: String },
    ModCast { content_id: String, request_id: u64, target_x: f64, target_y: f64 },
}

/// Source `MOD_ACTIONS`: payload admission and capabilities remain with the host and
/// prepared content owner; renderers only submit these exact authored intents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModAction {
    PortalEnter,
    QuickbarBind,
    Reforge,
    SceneRoom,
    SceneReturn,
    ShopBuy,
    SkillChoose,
    UiAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SaveCheckpointReason {
    GameOver,
    Progress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PartyAction {
    AcceptInvitation,
    DenyInvitation,
    Invite,
    InviteCollege,
    Kick,
    Leave,
    RequestAccept,
    RequestDeny,
    RotateCode,
    Settings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostedServerMessage {
    SaveCheckpointChunk {
        data: Vec<u16>,
        offset: usize,
        total_length: usize,
        reason: SaveCheckpointReason,
        sequence: u64,
    },
    SaveCheckpoint {
        save: String,
        reason: SaveCheckpointReason,
        sequence: u64,
    },
    SaveBeforeLeave {
        checkpoint_sequence: u64,
        request_id: u64,
    },
    DeploymentRestart {
        checkpoint_sequence: u64,
        target_revision: String,
        deployment_id: String,
    },
    LeaderboardReceipt {
        receipt: String,
    },
    PlayerCard {
        profile: Option<GamePlayerCardProfile>,
        request_id: u64,
    },
    CollegeInvitations {
        invitations: Vec<GameCollegeInvitation>,
    },
    PartyAction {
        action: PartyAction,
        accepted: bool,
        reason: Option<PartyActionRejection>,
    },
    CheatMode {
        enabled: bool,
    },
    LuaResult {
        error: Option<String>,
        ok: bool,
        output: Vec<String>,
        request_id: u64,
        values: Vec<LuaValue>,
    },
    ModCatalog {
        items: Vec<ModConsumableCatalogEntry>,
    },
    /// Source `server-mod-content`: the prepared owner's accepted content graph.
    /// Runtime actor/state rows remain in the separate `ModRuntime` projection.
    ModContent {
        projection: LuaObject,
    },
    ModRuntime {
        projection: LuaObject,
        revision: u64,
    },
}

/// Source `MainMenuScene` settings projection. Only the accepted host owner may
/// change this value; local renderer quality settings have separate ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnhancedEffectsAuthority {
    pub enabled: bool,
    pub can_change: bool,
    pub scope: EnhancedEffectsScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnhancedEffectsScope {
    Game,
    NextPartyRun,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClientMessage;

    #[test]
    fn hosted_hello_cannot_claim_a_developer_capability() {
        let request = HostedHello {
            enhanced_effects: true,
            version: crate::PROTOCOL_VERSION,
            credential: "one-use-control-plane-ticket".into(),
            selection: NewGameSelection {
                name: "Sirmin".into(),
                element: solomon_sim::profile::WizardElement::Ether,
                discipline: solomon_sim::profile::WizardDiscipline::Arcane,
            },
            profile: PlayerSocialProfile::default(),
            online_preferences: GameOnlinePreferences::default(),
            resume_token: None,
            save: Some("the exact owner save document".into()),
            save_intent: Some(GameSaveIntent::Resume),
            begin_college_intro: false,
            decline_tutorial: false,
            cheats_enabled: false,
            allow_mod_mismatch: false,
        };
        let message = ClientMessage::Hosted { command: HostedCommand::Hello { hello: request } };
        let mut encoded = serde_json::to_value(message).unwrap();
        assert!(serde_json::from_value::<ClientMessage>(encoded.clone()).is_ok());
        encoded["command"]["hello"]["developer_access"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ClientMessage>(encoded).is_err());
    }
}
