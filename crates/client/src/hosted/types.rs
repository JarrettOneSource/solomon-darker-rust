//! Records of the Website control-plane API. These are transport values; gameplay,
//! membership and content decisions remain with the authority.

use serde::{Deserialize, Serialize};
use solomon_sim::hall_of_fame::{HallOfFameEntry, HallOfFameSkill};
use solomon_sim::profile::{WizardDiscipline, WizardElement};
use solomon_sim::social::{GameSessionKind, PartyVisibility};

/// A signed-in Website account: ID, username, email, school, developer access and creation
/// time.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub developer_access: bool,
    pub id: u64,
    pub username: String,
    pub email: Option<String>,
    #[serde(default, deserialize_with = "optional_element")]
    pub school: Option<WizardElement>,
    pub created_at_utc: String,
}

/// A sign-in response: the session token and the account.
#[derive(Deserialize)]
pub(super) struct AuthResponse {
    pub token: String,
    pub user: Account,
}

/// The current account response.
#[derive(Deserialize)]
pub(super) struct AccountResponse {
    pub user: Account,
}

/// A content author's public identity: ID, username and school.
#[derive(Clone, Debug, Deserialize)]
pub struct Author {
    pub id: u64,
    pub username: String,
    #[serde(default, deserialize_with = "optional_element")]
    pub school: Option<WizardElement>,
}

/// The order of a mod catalog listing.
#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModSort {
    #[default]
    Newest,
    Downloads,
    Updated,
    Name,
}

impl ModSort {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Newest => "newest",
            Self::Downloads => "downloads",
            Self::Updated => "updated",
            Self::Name => "name",
        }
    }
}

/// Who can find a published mod.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModVisibility {
    Public,
    Unlisted,
    Private,
}

/// A published mod's catalog entry: identity, author, latest version, tags and download
/// counts.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSummary {
    pub id: u64,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub package_id: Option<String>,
    pub visibility: ModVisibility,
    pub tags: Vec<String>,
    pub author: Author,
    pub latest_version: String,
    pub downloads: u64,
    pub recent_downloads: Option<u64>,
    pub thumbnail_url: Option<String>,
    pub created_at_utc: String,
    pub updated_at_utc: String,
}

/// One published version of a mod: its manifest and package hashes, changelog, size and
/// downloads.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModVersion {
    pub id: u64,
    pub version: String,
    pub manifest_version: Option<String>,
    pub package_sha256: Option<String>,
    pub content_sha256: Option<String>,
    pub changelog: String,
    pub file_size: u64,
    pub downloads: u64,
    pub created_at_utc: String,
}

/// One screenshot on a mod's page.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModScreenshot {
    pub id: u64,
    pub url: String,
    pub sort_order: i64,
}

/// A mod's full page: its summary, description, screenshots and versions.
#[derive(Clone, Debug, Deserialize)]
pub struct ModDetail {
    #[serde(flatten)]
    pub summary: ModSummary,
    pub description: String,
    pub screenshots: Vec<ModScreenshot>,
    pub versions: Vec<ModVersion>,
}

/// One comment on a mod's page.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModComment {
    pub id: u64,
    pub body: String,
    pub created_at_utc: String,
    pub author: Author,
}

/// A mod's comments and their total count.
#[derive(Clone, Debug, Deserialize)]
pub struct ModComments {
    pub items: Vec<ModComment>,
    pub total: u64,
}

/// The author of a shared mobile control layout.
#[derive(Clone, Debug, Deserialize)]
pub struct LayoutAuthor {
    pub username: String,
}

/// A mobile control layout shared by code: the layout, its author and creation time.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMobileUiLayout {
    pub code: String,
    /// Validated by the mobile-layout owner before replacing local controls.
    pub layout: serde_json::Value,
    pub author: LayoutAuthor,
    pub created_at_utc: String,
}

/// One page of a mod catalog listing.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModList {
    pub items: Vec<ModSummary>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

/// The player's subscription to a mod and whether it is enabled.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSubscription {
    pub enabled: bool,
    pub created_at_utc: String,
    pub updated_at_utc: String,
    #[serde(rename = "mod")]
    pub content: ModSummary,
}

pub use solomon_sim::modding::assets::GameModAsset as GameContentAsset;

/// An enabled mod in the player's active set: identity, version, content hash, load
/// priority and assets.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveWebMod {
    pub assets: Vec<GameContentAsset>,
    pub id: String,
    pub name: String,
    pub slug: String,
    pub version: String,
    pub content_sha256: String,
    pub priority: i64,
    pub has_lua: bool,
    pub boneyard_count: u64,
}

/// A subscribed mod that was disabled automatically, with the reason.
#[derive(Clone, Debug, Deserialize)]
pub struct DisabledWebMod {
    pub error: String,
    pub name: String,
    pub slug: String,
}

/// The player's active mod set: the enabled mods, the manifest hash and any mods disabled
/// automatically.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveWebModSet {
    pub disabled_mods: Vec<DisabledWebMod>,
    pub manifest_sha256: String,
    pub mods: Vec<ActiveWebMod>,
}

/// A party in the party directory: leader, members, session kind, visibility, mods and
/// activity.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicGameParty {
    pub cheats_enabled: bool,
    pub id: String,
    pub leader: String,
    pub members: Vec<String>,
    pub member_count: usize,
    pub max_members: usize,
    pub mod_count: usize,
    #[serde(with = "SessionKindWire")]
    pub session_kind: GameSessionKind,
    #[serde(with = "VisibilityWire")]
    pub visibility: PartyVisibility,
    #[serde(flatten)]
    pub activity: PartyActivity,
}

/// What a listed party is doing: in the Hub or playing a named Boneyard.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum PartyActivity {
    Hub,
    Playing {
        #[serde(rename = "boneyardName")]
        boneyard_name: String,
    },
}

/// One mod a party runs, with the assets a joining player downloads.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyJoinMod {
    pub assets: Vec<GameContentAsset>,
    pub content_sha256: String,
    pub id: String,
    pub name: String,
    pub slug: String,
    pub version: String,
}

/// The content a party runs: its manifest hash and mods.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyContent {
    pub manifest_sha256: String,
    pub mods: Vec<PartyJoinMod>,
}

/// Whether a party is in the Hub or playing.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartyStatus {
    Hub,
    Playing,
}

/// The party a join resolves to: its content, session kind, leader, size, status and
/// visibility.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyJoinTarget {
    pub cheats_enabled: bool,
    pub content: PartyContent,
    #[serde(with = "SessionKindWire")]
    pub kind: GameSessionKind,
    pub leader: String,
    pub member_count: usize,
    pub status: PartyStatus,
    #[serde(with = "VisibilityWire")]
    pub visibility: PartyVisibility,
}

/// A resolved party join: the join intent's ID and its target.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyJoinResolution {
    pub intent_id: String,
    pub target: PartyJoinTarget,
}

/// A player connected to a game: name, account, session, party and activity.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedGamePlayer {
    pub display_name: String,
    pub account_username: Option<String>,
    pub bot: bool,
    pub developer: bool,
    #[serde(with = "SessionKindWire")]
    pub session: GameSessionKind,
    pub party_leader: Option<String>,
    pub party_size: Option<u64>,
    #[serde(flatten)]
    pub activity: ConnectedPlayerActivity,
}

/// What a connected player is doing: in the Hub, or in a named Boneyard at a wave.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "activity", rename_all = "lowercase")]
pub enum ConnectedPlayerActivity {
    Hub,
    Boneyard {
        #[serde(rename = "boneyardName")]
        boneyard_name: String,
        #[serde(rename = "waveNumber")]
        wave_number: u64,
    },
}

/// A running Boneyard match as listed for developers: players, session, visibility and wave.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperGameMatch {
    pub boneyard_name: String,
    pub id: String,
    pub party_leader: String,
    pub player_count: u64,
    pub players: Vec<String>,
    #[serde(with = "SessionKindWire")]
    pub session: GameSessionKind,
    #[serde(with = "VisibilityWire")]
    pub visibility: PartyVisibility,
    pub wave_number: u64,
}

/// The state of a request to join a party: pending, denied, or accepted with its target.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum PartyJoinRequestStatus {
    Pending,
    Denied,
    Accepted {
        #[serde(rename = "intentId")]
        intent_id: String,
        target: PartyJoinTarget,
    },
}

/// A party join request's token and current status.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyJoinRequestReceipt {
    pub request_token: String,
    #[serde(flatten)]
    pub status: PartyJoinRequestStatus,
}

/// The kind of a provisioned game endpoint; only remote servers exist.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemoteEndpointKind {
    Remote,
}

// Credentials intentionally have no Debug implementation: endpoint values never belong
// in diagnostics or browser evidence logs.
/// A game server endpoint provisioned by the Website: URL, credential, session kind and
/// whether it is for observing.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionedGameEndpoint {
    pub kind: RemoteEndpointKind,
    #[serde(with = "SessionKindWire")]
    pub session_kind: GameSessionKind,
    pub url: String,
    pub credential: String,
    #[serde(default)]
    pub observer: bool,
}

/// A cloud save slot: its revision and save document.
#[derive(Clone, Deserialize)]
pub struct WebGameSave {
    pub slot: u32,
    pub revision: u64,
    pub document: String,
}

/// One row of the global leaderboard: its rank and Hall of Fame entry.
#[derive(Clone, Debug, Deserialize)]
pub struct GlobalLeaderboardEntry {
    pub rank: Option<u64>,
    #[serde(flatten, with = "HallEntryWire")]
    pub entry: HallOfFameEntry,
}

/// A response that wraps its list in `items`.
#[derive(Deserialize)]
pub(super) struct Items<T> {
    pub items: Vec<T>,
}

#[derive(Deserialize)]
#[serde(remote = "WizardElement", rename_all = "lowercase")]
enum ElementWire {
    Air,
    Earth,
    Ether,
    Fire,
    Water,
}

#[derive(Deserialize)]
#[serde(remote = "WizardDiscipline", rename_all = "lowercase")]
enum DisciplineWire {
    Arcane,
    Body,
    Mind,
}

#[derive(Deserialize)]
#[serde(remote = "GameSessionKind", rename_all = "kebab-case")]
enum SessionKindWire {
    GlobalHub,
    PrivateCollege,
    Standalone,
}

#[derive(Deserialize)]
#[serde(remote = "PartyVisibility", rename_all = "kebab-case")]
enum VisibilityWire {
    Public,
    InviteOnly,
    Private,
}

fn optional_element<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<WizardElement>, D::Error> {
    #[derive(Deserialize)]
    struct Element(#[serde(with = "ElementWire")] WizardElement);
    Option::<Element>::deserialize(deserializer).map(|value| value.map(|element| element.0))
}

#[derive(Deserialize)]
#[serde(remote = "HallOfFameSkill", rename_all = "camelCase")]
struct HallSkillWire {
    pub rank: u64,
    pub skill_id: u64,
}

fn hall_skills<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<HallOfFameSkill>, D::Error> {
    #[derive(Deserialize)]
    struct Skill(#[serde(with = "HallSkillWire")] HallOfFameSkill);
    Vec::<Skill>::deserialize(deserializer).map(|rows| rows.into_iter().map(|row| row.0).collect())
}

#[derive(Deserialize)]
#[serde(remote = "HallOfFameEntry", rename_all = "camelCase")]
struct HallEntryWire {
    pub account_username: Option<String>,
    pub awesomeness: u64,
    pub awesomest_kill: Option<String>,
    pub completed_at_utc: String,
    #[serde(with = "DisciplineWire")]
    pub discipline: WizardDiscipline,
    pub elapsed_ticks: u64,
    #[serde(with = "ElementWire")]
    pub element: WizardElement,
    pub heading_index: u64,
    #[serde(deserialize_with = "hall_skills")]
    pub highest_skills: Vec<HallOfFameSkill>,
    pub level: u64,
    pub monsters_killed: u64,
    pub perks_used: Vec<u64>,
    pub portrait_scale: f64,
    pub run_id: String,
    pub wave: u64,
    pub wizard_name: String,
}
