//! A hosted College above canonical simulation owners. The supervisor holds one of
//! these per admitted College; the persistent Global College owns independent party runs.
//! Source: Website `host/game-host.ts` and `host/shared-game-worlds.ts`.

mod access;
mod activity;
mod admission;
mod archive;
mod bots;
mod console;
mod effects;
mod flame;
mod leaderboard;
mod lifecycle;
mod membership;
mod mod_actions;
mod mod_epochs;
mod mods;
mod observer;
mod publication;
mod recovery;
mod rejoin;
mod saves;
mod social;
mod socket;
mod worlds;
pub use access::{
    ExternalPartyJoinRequest, ExternalPartyJoinRequestStatus, HostedPartyTarget, PartyStatus,
    PublicPartyDirectoryEntry,
};
pub use lifecycle::HostedWorldTick;
pub(in crate::host) use mod_actions::HostedModActionRequest;
pub use observer::{HostedObservationTarget, HostedObserver};
pub use rejoin::{
    HostedRecoveryLineage, HostedRejoinRejection, HostedRejoinSlot, HostedRejoinStatus,
    HostedRejoinTarget,
};
pub(in crate::host) use saves::PlayerSaveRequest;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use solomon_assets::website_save::SaveMetadata;
use solomon_modding::host::PreparedModHost;
use solomon_modding::package::MaterializedWebSessionContent;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::modding::content::PreparedModContentCatalog;
use solomon_sim::session::GameSession;
use solomon_sim::social::broker::GameSocialConnection;
use solomon_sim::social::{GameOnlinePreferences, GameSessionKind, PlayerSocialProfile};

use super::authority::random_bytes;
use super::connection::HostConnection;
use super::content::HostContent;
use super::control::GameplayControl;
use crate::transport::PeerId;

pub use worlds::HostedWorlds;

/// Capabilities are received only through an authenticated, one-use supervisor ticket.
/// The public browser hello never supplies any of these authority fields.
pub struct HostedAdmission {
    pub content: Arc<MaterializedWebSessionContent>,
    pub developer_access: bool,
    pub leaderboard_user_id: Option<u32>,
    pub party_id: Option<String>,
    pub reservation_id: Option<String>,
    pub party_rejoin_token: Option<String>,
    pub party_recovery_seed: bool,
    pub observer: Option<HostedObserverTarget>,
}

pub struct HostedObserverTarget {
    pub run_id: String,
    pub user_id: u32,
    pub username: String,
}

pub struct HostedPlayer {
    pub id: String,
    pub resume_token: String,
    pub content: Arc<MaterializedWebSessionContent>,
    pub peer_id: PeerId,
    pub player_reference: String,
    pub social_connection: Option<GameSocialConnection>,
    pub profile: PlayerSocialProfile,
    pub preferences: GameOnlinePreferences,
    pub developer_access: bool,
    pub leaderboard_user_id: Option<u32>,
    pub save_metadata: SaveMetadata,
    pub global_score_eligible: bool,
    pub tutorial_eligible: bool,
    pub terminal_saved_run_id: Option<String>,
    pub(super) last_save_sequence: u64,
    pub(super) last_save_document: Option<String>,
}

pub struct PartyReservation {
    pub party_id: String,
    pub reservation_id: String,
    pub expires_at_ms: f64,
}

pub struct HostedGameHost {
    pub id: String,
    pub kind: GameSessionKind,
    /// Source editor previews own one disposable wizard and never enter hosted services.
    pub(in crate::host) editor_test: bool,
    pub worlds: HostedWorlds,
    flame_histories: super::flame::HostFlameHistories,
    run_archives: Option<super::run_archive::HostRunArchives>,
    pub content: Arc<MaterializedWebSessionContent>,
    pub maximum_players: usize,
    pub mod_content: PreparedModContentCatalog,
    pub(super) private_mod_host: Option<PreparedModHost>,
    pub(super) private_mod_events: Vec<solomon_sim::modding::events::WebLuaDerivedEvent>,
    party_mod_runtimes: Vec<mods::PartyModRuntimeScope>,
    pub(super) source_collator: solomon_modding::json::SourceCollator,
    bots: bots::HostedBots,
    console: Option<console::HostedConsole>,
    next_lua_run_seed: Option<u32>,
    pub(super) memorial_persistence: Option<super::memorial::MemorialPersistence>,
    pub(super) runtime_events: Option<super::runtime_events::RuntimeEventSink>,
    runtime_activity: Vec<activity::RuntimeActivityEpoch>,
    pub(super) recovery_secret: String,
    pub(super) recovery_revision: String,
    leaderboard_runs: leaderboard::LeaderboardRunState,
    leaderboard_previous_runs: Vec<leaderboard::LeaderboardRunSnapshot>,
    pub players: Vec<HostedPlayer>,
    pub(super) observers: BTreeMap<PeerId, HostedObserver>,
    pub(super) connections: BTreeMap<PeerId, HostConnection>,
    pub(super) content_loader: HostContent,
    pub(super) navigation: BTreeMap<String, NavigationSet>,
    pub(super) controls: BTreeMap<String, GameplayControl>,
    pub(super) college_intro_ready: BTreeSet<String>,
    pub(super) reservations: Vec<PartyReservation>,
    pub external_join_requests: Vec<ExternalPartyJoinRequest>,
    pub rejoin_slots: Vec<HostedRejoinSlot>,
    pub recovery_lineages: Vec<HostedRecoveryLineage>,
    pub(super) pending_player_saves: BTreeSet<String>,
    pub(super) next_player_id: u64,
    pub(super) next_snapshot_sequence: u64,
    pub(super) next_chat_sequence: u64,
    pub(super) retired_recoveries: Vec<(String, bool)>,
    pub(super) publish_dirty: bool,
    pub(super) social_dirty: bool,
    pub(super) cheats_enabled: bool,
    pub(super) cheat_policy_initialized: bool,
}

impl HostedGameHost {
    pub fn new(
        id: String,
        kind: GameSessionKind,
        maximum_players: usize,
        content: Arc<MaterializedWebSessionContent>,
        asset_directory: &Path,
        recovery_secret: String,
        recovery_revision: String,
    ) -> Result<Self, String> {
        if maximum_players == 0 {
            return Err("maxPlayers must be positive".into());
        }
        if kind == GameSessionKind::GlobalHub && !content.manifest.mods.is_empty() {
            return Err("Mods require a private College.".into());
        }
        let sources: Vec<_> = content
            .mod_sources
            .iter()
            .map(|source| solomon_modding::assets::ModAssetSource {
                files: source.files.clone(),
                identity: source.identity.clone(),
            })
            .collect();
        let assets = solomon_modding::assets::compile_mod_assets(
            &content.assets,
            &content.compiled_mods,
            &sources,
        )?;
        let mod_content =
            solomon_modding::content::compile_mod_content_catalog(&content.compiled_mods, &assets)?;
        let seed = u32::from_le_bytes(random_bytes::<4>()?);
        let content_loader =
            HostContent::load_with_mod_entries(asset_directory, content.boneyards.clone())?;
        let mut worlds = HostedWorlds::new(kind, seed)?;
        let private_mod_host = match &mut worlds {
            HostedWorlds::Private { state, .. } if !content.mod_sources.is_empty() => {
                Some(PreparedModHost::prepare(&content, state)?)
            }
            _ => None,
        };
        Ok(Self {
            id,
            kind,
            editor_test: false,
            worlds,
            flame_histories: super::flame::HostFlameHistories::default(),
            run_archives: None,
            content,
            maximum_players,
            mod_content,
            private_mod_host,
            private_mod_events: Vec::new(),
            party_mod_runtimes: Vec::new(),
            source_collator: solomon_modding::json::SourceCollator::default(),
            bots: bots::HostedBots::default(),
            console: None,
            next_lua_run_seed: None,
            memorial_persistence: None,
            runtime_events: None,
            runtime_activity: Vec::new(),
            recovery_secret,
            recovery_revision,
            leaderboard_runs: leaderboard::LeaderboardRunState::default(),
            leaderboard_previous_runs: Vec::new(),
            players: Vec::new(),
            connections: BTreeMap::new(),
            observers: BTreeMap::new(),
            content_loader,
            navigation: BTreeMap::new(),
            controls: BTreeMap::new(),
            college_intro_ready: BTreeSet::new(),
            reservations: Vec::new(),
            external_join_requests: Vec::new(),
            rejoin_slots: Vec::new(),
            recovery_lineages: Vec::new(),
            pending_player_saves: BTreeSet::new(),
            next_player_id: 1,
            next_snapshot_sequence: 1,
            next_chat_sequence: 1,
            retired_recoveries: Vec::new(),
            publish_dirty: true,
            social_dirty: true,
            cheats_enabled: false,
            cheat_policy_initialized: false,
        })
    }

    pub fn state_for_player(&self, id: &str) -> Option<&GameSession> {
        self.worlds.state_for_player(id)
    }

    pub(in crate::host) fn configure_editor_test(
        &mut self,
        entry: solomon_assets::boneyard::ModBoneyardEntry,
    ) -> Result<(), String> {
        if self.kind != GameSessionKind::Standalone || self.maximum_players != 1 {
            return Err("Editor tests require a standalone one-player authority.".into());
        }
        self.content_loader = self.content_loader.with_mod_entries(vec![entry])?;
        self.editor_test = true;
        Ok(())
    }

    fn boneyard_choices(&self) -> Vec<solomon_sim::boneyard::BoneyardChoice> {
        self.content_loader
            .choices()
            .into_iter()
            .filter(|choice| {
                !self.editor_test || choice.id == super::editor_test::EDITOR_TEST_BONEYARD_ID
            })
            .collect()
    }

    pub fn player(&self, id: &str) -> Option<&HostedPlayer> {
        self.players.iter().find(|player| player.id == id)
    }

    pub fn player_mut(&mut self, id: &str) -> Option<&mut HostedPlayer> {
        self.players.iter_mut().find(|player| player.id == id)
    }
}
