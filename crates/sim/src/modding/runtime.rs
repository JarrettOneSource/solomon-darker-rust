//! Plain mutable authored-content records from `modding/content/*-engine.ts`.
//! Prepared native owners execute their rules; these rows cross save and client
//! boundaries without Lua handles or a second stock player/world model.

use super::definition::{ResolvedWebLuaContentReference, WebLuaSchemaDefinition, WebLuaScopeKind};
use super::{LuaObject, LuaValue};

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ActiveModStatus {
    pub content_id: String,
    pub expires_tick: u64,
    pub instance_id: u64,
    pub mod_id: String,
    pub started_tick: u64,
    pub target_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModStatusCheckpoint {
    pub instances: Vec<ActiveModStatus>,
    pub next_instance_id: u64,
    pub revision: u64,
}

impl Default for ModStatusCheckpoint {
    fn default() -> Self {
        Self { instances: Vec::new(), next_instance_id: 1, revision: 0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct RolledModAffix {
    pub content_id: String,
    pub modifiers: LuaObject,
    pub name: String,
    pub roll: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSpellCooldown {
    pub content_id: String,
    pub player_id: String,
    pub ready_tick: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSpellCheckpoint {
    pub cooldowns: Vec<ModSpellCooldown>,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSkillBinding {
    pub content_id: String,
    pub player_id: String,
    pub slot: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSkillRank {
    pub content_id: String,
    pub player_id: String,
    pub rank: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSkillOffer {
    pub content_ids: Vec<String>,
    pub player_id: String,
    pub sequence: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSkillCheckpoint {
    pub bindings: Vec<ModSkillBinding>,
    pub offers: Vec<ModSkillOffer>,
    pub ranks: Vec<ModSkillRank>,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModShopPurchase {
    pub item_content_id: String,
    pub price: u64,
    pub quantity: u32,
    pub row: usize,
    pub shop_content_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModShopStockState {
    pub player_id: String,
    pub remaining: u64,
    pub restock_tick: Option<u64>,
    pub row: u64,
    pub shop_content_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModShopCheckpoint {
    pub revision: u64,
    pub stock: Vec<ModShopStockState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModStateScope {
    pub id: String,
    pub kind: WebLuaScopeKind,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSemanticValue {
    pub key: String,
    pub mod_id: String,
    pub scope: ModStateScope,
    pub value: LuaValue,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModSemanticStateCheckpoint {
    pub revision: u64,
    pub values: Vec<ModSemanticValue>,
}

/// Source's save decoder accepts a nonempty life-state string before engine
/// restore. The zero-health/null-death witness also passes the engine's two
/// biconditionals, so an unrecognized value must survive unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModEnemyLifeState {
    Alive,
    Dying,
    Unknown(String),
}

impl ModEnemyLifeState {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Alive => "alive",
            Self::Dying => "dying",
            Self::Unknown(value) => value,
        }
    }

    #[must_use]
    pub fn from_source(value: String) -> Self {
        match value.as_str() {
            "alive" => Self::Alive,
            "dying" => Self::Dying,
            _ => Self::Unknown(value),
        }
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for ModEnemyLifeState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for ModEnemyLifeState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self::from_source(value))
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModActorPlayer {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ActiveModEnemy {
    pub content_id: String,
    pub current_health: f64,
    pub death_tick: Option<u64>,
    pub heading_index: u64,
    pub id: u64,
    pub last_attack_tick: Option<u64>,
    pub last_damaged_by_player_id: Option<String>,
    pub life_state: ModEnemyLifeState,
    pub maximum_health: f64,
    pub moving: bool,
    pub next_attack_tick: u64,
    pub spawned_tick: u64,
    pub target_player_id: Option<String>,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModEnemyAttack {
    pub amount: f64,
    pub enemy_id: u64,
    pub player_id: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModEnemyCheckpoint {
    pub enemies: Vec<ActiveModEnemy>,
    pub next_id: u64,
    pub revision: u64,
}

impl Default for ModEnemyCheckpoint {
    fn default() -> Self {
        Self { enemies: Vec::new(), next_id: 1, revision: 0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ActiveModPowerup {
    pub content_id: String,
    pub id: u64,
    pub mod_id: String,
    pub spawned_tick: u64,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModPowerupCheckpoint {
    pub instances: Vec<ActiveModPowerup>,
    pub next_id: u64,
    pub revision: u64,
}

impl Default for ModPowerupCheckpoint {
    fn default() -> Self {
        Self { instances: Vec::new(), next_id: 1, revision: 0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModPowerupCollectionEvent {
    pub content_id: String,
    pub id: u64,
    pub player_id: String,
    pub tick: u64,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ActiveModScene {
    pub epoch: u64,
    pub owner_id: String,
    pub parent_content_id: Option<String>,
    pub room_index: u64,
    pub scene_content_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSceneStack {
    pub owner_id: String,
    pub room_indexes: Vec<u64>,
    pub scene_content_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSceneCheckpoint {
    pub next_epoch: u64,
    pub scenes: Vec<ActiveModScene>,
    pub stacks: Vec<ModSceneStack>,
}

impl Default for ModSceneCheckpoint {
    fn default() -> Self {
        Self { next_epoch: 1, scenes: Vec::new(), stacks: Vec::new() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModPortalPolicy {
    AnyMember,
    LeaderConfirms,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModPortal {
    pub destination: ResolvedWebLuaContentReference,
    pub id: String,
    pub object_kind: String,
    pub policy: ModPortalPolicy,
    pub prompt: String,
    pub scene: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModSpellEffectKind {
    Area,
    Channel,
    Projectile,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModEffectTemplate {
    pub fields: LuaObject,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ActiveModSpellEffect {
    pub content_id: String,
    pub effects: Vec<ModEffectTemplate>,
    pub expires_tick: u64,
    pub hit_targets: Vec<String>,
    pub id: u64,
    pub interval_ticks: u64,
    pub kind: ModSpellEffectKind,
    pub last_tick: u64,
    pub mod_id: String,
    pub next_pulse_tick: u64,
    pub owner_player_id: String,
    pub radius: f64,
    pub scope: ModStateScope,
    pub speed_per_tick: f64,
    pub started_tick: u64,
    pub target_x: f64,
    pub target_y: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSpellEffectCheckpoint {
    pub effects: Vec<ActiveModSpellEffect>,
    pub next_id: u64,
    pub next_sequence: u64,
    pub revision: u64,
}

impl Default for ModSpellEffectCheckpoint {
    fn default() -> Self {
        Self { effects: Vec::new(), next_id: 1, next_sequence: 1_000_000_000, revision: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModSpellEffectTargetKind {
    ModEnemy,
    NativeEnemy,
}

impl ModSpellEffectTargetKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModEnemy => "mod-enemy",
            Self::NativeEnemy => "native-enemy",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModSpellEffectTarget {
    pub id: u64,
    pub kind: ModSpellEffectTargetKind,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSpellEffectIntent {
    pub fields: LuaObject,
    pub kind: String,
    pub mod_id: String,
    pub owner: String,
    pub scope: ModStateScope,
    pub sequence: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModSpellEffectBatch {
    pub context: LuaObject,
    pub intents: Vec<ModSpellEffectIntent>,
    pub scope: ModStateScope,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModStateCellSnapshot {
    pub key: String,
    pub mod_id: String,
    pub schema_version: u64,
    pub scope: ModStateScope,
    pub value: LuaValue,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModStateCheckpoint {
    pub cells: Vec<ModStateCellSnapshot>,
    pub revision: u64,
}

/// Runtime function handles stay in the native VM. These accepted definitions
/// identify their migration versions without putting executable callbacks in saves.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModStateCellDefinition {
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub default_value: Option<LuaValue>,
    pub key: String,
    pub migration_versions: Vec<u64>,
    pub mod_id: String,
    pub schema: WebLuaSchemaDefinition,
    pub schema_version: u64,
    pub scope: WebLuaScopeKind,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModStateUpdate {
    pub revision: u64,
    pub value: LuaValue,
}

pub type ModIntent = ModSpellEffectIntent;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModRuleDispatchInput {
    pub context: LuaObject,
    pub event: String,
    pub payload: LuaValue,
    pub scope: ModStateScope,
    pub tick: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModRuleDispatchResult {
    pub budget_exceeded: bool,
    pub errors: Vec<String>,
    pub intents: Vec<ModIntent>,
    pub invocations: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModRuleTimerCheckpoint {
    pub context: LuaObject,
    pub due_tick: u64,
    pub event: String,
    pub id: u64,
    pub interval_ticks: Option<u64>,
    pub mod_id: String,
    pub node_id: String,
    pub payload: LuaValue,
    pub remaining: u64,
    pub scope: ModStateScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModReducerHealthCheckpoint {
    pub disabled: bool,
    pub failures: u64,
    pub key: String,
    pub mod_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModReducerTraceOutcome {
    Error,
    Ok,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModReducerTrace {
    pub disabled: bool,
    pub error: Option<String>,
    pub event: String,
    pub failures: u64,
    pub intents: u32,
    pub key: String,
    pub mod_id: String,
    pub outcome: ModReducerTraceOutcome,
    pub scope: ModStateScope,
    pub tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModRuleEngineCheckpoint {
    pub next_intent_sequence: u64,
    pub next_timer_id: u64,
    pub reducer_health: Vec<ModReducerHealthCheckpoint>,
    pub state: ModStateCheckpoint,
    pub timers: Vec<ModRuleTimerCheckpoint>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModScheduledRuleResult {
    pub context: LuaObject,
    pub result: ModRuleDispatchResult,
    pub scope: ModStateScope,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModCheckpoint {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub rules: ModRuleEngineCheckpoint,
    pub graph_sha256: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModStepResult {
    pub accepted: bool,
    pub budget_exceeded: bool,
    pub errors: Vec<String>,
    pub intents: Vec<ModIntent>,
    pub invocations: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PreparedModEvent {
    pub context: LuaObject,
    pub event: String,
    pub payload: LuaValue,
    pub scope: ModStateScope,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PreparedModStepInput {
    pub events: Vec<PreparedModEvent>,
    pub tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModProjection {
    pub catalog: Vec<super::definition::CompiledWebLuaMod>,
    pub state: ModStateCheckpoint,
    pub viewer_id: String,
}

/// `PreparedModHostCheckpoint`: all mutable authored engines plus reducer state.
/// The VM and immutable package catalogs are reconstructed by the native owner.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModHostCheckpoint {
    pub enemies: ModEnemyCheckpoint,
    pub powerups: ModPowerupCheckpoint,
    pub scenes: ModSceneCheckpoint,
    pub semantic_state: ModSemanticStateCheckpoint,
    pub shops: ModShopCheckpoint,
    pub skills: ModSkillCheckpoint,
    pub spells: ModSpellCheckpoint,
    pub spell_effects: ModSpellEffectCheckpoint,
    pub session: PreparedModCheckpoint,
    pub statuses: ModStatusCheckpoint,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModPresentationIntent {
    pub fields: LuaObject,
    pub kind: PreparedModPresentationKind,
    pub mod_id: String,
    pub sequence: u64,
    pub tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum PreparedModPresentationKind {
    Present,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModActionInput {
    pub context: LuaObject,
    pub payload: LuaValue,
    pub scope: ModStateScope,
    pub action: String,
    pub request_id: u64,
    pub tick: u64,
}
