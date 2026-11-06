//! Plain mutable records of mod runtime state and their checkpoints.
//! Prepared native owners execute their rules; these rows cross save and client
//! boundaries without Lua handles or a second stock player/world model.

use super::definition::{ResolvedWebLuaContentReference, WebLuaSchemaDefinition, WebLuaScopeKind};
use super::{LuaObject, LuaValue};

/// An active mod status on a target: its content, mod, instance ID and the ticks it starts
/// and expires.
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

/// Saved mod statuses: the active instances, the next instance ID and a revision.
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

/// An affix rolled onto an item: its content, name, modifiers and roll.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct RolledModAffix {
    pub content_id: String,
    pub modifiers: LuaObject,
    pub name: String,
    pub roll: u32,
}

/// The tick at which a player's mod spell is ready again.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSpellCooldown {
    pub content_id: String,
    pub player_id: String,
    pub ready_tick: u64,
}

/// Saved mod spell cooldowns and a revision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSpellCheckpoint {
    pub cooldowns: Vec<ModSpellCooldown>,
    pub revision: u64,
}

/// A player's mod skill bound to a quickbar slot.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSkillBinding {
    pub content_id: String,
    pub player_id: String,
    pub slot: u64,
}

/// A player's rank in a mod skill.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSkillRank {
    pub content_id: String,
    pub player_id: String,
    pub rank: u64,
}

/// A mod skill offer made to a player: the offered skills and the offer's sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSkillOffer {
    pub content_ids: Vec<String>,
    pub player_id: String,
    pub sequence: u64,
}

/// Saved mod skill bindings, ranks and pending offers, and a revision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSkillCheckpoint {
    pub bindings: Vec<ModSkillBinding>,
    pub offers: Vec<ModSkillOffer>,
    pub ranks: Vec<ModSkillRank>,
    pub revision: u64,
}

/// One mod shop purchase: the shop, stock row, item, quantity and price.
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

/// A player's remaining stock in one shop row, and the tick it restocks.
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

/// Saved mod shop stock and a revision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModShopCheckpoint {
    pub revision: u64,
    pub stock: Vec<ModShopStockState>,
}

/// The scope a piece of mod state belongs to: its kind and ID.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModStateScope {
    pub id: String,
    pub kind: WebLuaScopeKind,
}

/// One semantic state value a mod keeps under a key in a scope.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSemanticValue {
    pub key: String,
    pub mod_id: String,
    pub scope: ModStateScope,
    pub value: LuaValue,
}

/// Saved mod semantic values and a revision.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModSemanticStateCheckpoint {
    pub revision: u64,
    pub values: Vec<ModSemanticValue>,
}

/// A mod enemy's life state. The save decoder accepts any nonempty life-state string
/// before engine restore. The zero-health/null-death witness also passes the engine's two
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

/// A player's ID and position, as mod actors see it.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModActorPlayer {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

/// A live mod enemy: its content, health, position, heading, targets, attack timing and life
/// state.
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

/// One mod enemy attack on a player and its damage.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModEnemyAttack {
    pub amount: f64,
    pub enemy_id: u64,
    pub player_id: String,
}

/// Saved mod enemies, the next enemy ID and a revision.
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

/// A spawned mod powerup: its content, mod, ID, spawn tick and position.
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

/// Saved mod powerups, the next powerup ID and a revision.
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

/// A player collecting a mod powerup: the powerup, player, tick and position.
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

/// A mod scene open for an owner: the scene, its parent, the current room and its epoch.
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

/// An owner's stack of entered mod scenes and their room indexes.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSceneStack {
    pub owner_id: String,
    pub room_indexes: Vec<u64>,
    pub scene_content_ids: Vec<String>,
}

/// Saved mod scenes, scene stacks and the next scene epoch.
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

/// Whether any party member may use a mod portal, or the leader must confirm it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModPortalPolicy {
    AnyMember,
    LeaderConfirms,
}

/// A prepared mod portal: its ID, scene, object kind, prompt, destination and use policy.
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

/// The shape of a mod spell effect: an area, a channel or a projectile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModSpellEffectKind {
    Area,
    Channel,
    Projectile,
}

/// One effect a mod spell effect applies: its kind and fields.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModEffectTemplate {
    pub fields: LuaObject,
    pub kind: String,
}

/// A live mod spell effect: its owner, kind, position and target, radius, speed, pulse timing,
/// effects and the targets it has hit.
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

/// Saved mod spell effects, the next effect ID and intent sequence, and a revision.
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

/// Whether a mod spell effect target is a mod enemy or a stock enemy. Mods, saves and hit
/// keys spell a stock enemy `native-enemy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModSpellEffectTargetKind {
    ModEnemy,
    #[cfg_attr(feature = "persistence", serde(rename = "native-enemy"))]
    StockEnemy,
}

impl ModSpellEffectTargetKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModEnemy => "mod-enemy",
            Self::StockEnemy => "native-enemy",
        }
    }
}

/// A target a mod spell effect can hit: its kind, ID and position.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModSpellEffectTarget {
    pub id: u64,
    pub kind: ModSpellEffectTargetKind,
    pub x: f64,
    pub y: f64,
}

/// An intent a mod produced: its kind, fields, owner, scope and sequence.
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

/// A batch of mod intents, with their context and scope.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModSpellEffectBatch {
    pub context: LuaObject,
    pub intents: Vec<ModSpellEffectIntent>,
    pub scope: ModStateScope,
}

/// One mod state cell's value, by mod, key and scope, with its schema version.
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

/// Saved mod state cells and a revision.
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

/// A state cell's new value and the revision it was written at.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModStateUpdate {
    pub revision: u64,
    pub value: LuaValue,
}

pub type ModIntent = ModSpellEffectIntent;

/// An event dispatched to mod rules: its name, payload, context, scope and tick.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModRuleDispatchInput {
    pub context: LuaObject,
    pub event: String,
    pub payload: LuaValue,
    pub scope: ModStateScope,
    pub tick: u64,
}

/// What a rule dispatch produced: intents, errors, the invocation count and whether the budget
/// was exceeded.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModRuleDispatchResult {
    pub budget_exceeded: bool,
    pub errors: Vec<String>,
    pub intents: Vec<ModIntent>,
    pub invocations: u32,
}

/// A saved mod rule timer: its event, due tick, interval, remaining runs, payload, context and
/// scope.
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

/// A reducer's saved failure count and whether it has been disabled.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModReducerHealthCheckpoint {
    pub disabled: bool,
    pub failures: u64,
    pub key: String,
    pub mod_id: String,
}

/// Whether a reducer run succeeded or failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModReducerTraceOutcome {
    Error,
    Ok,
}

/// A trace of one reducer run: its event, scope and tick, outcome and error, intent count and
/// failure state.
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

/// Saved rule engine state: reducer state and health, timers, and the next timer ID and intent
/// sequence.
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

/// The result of a scheduled rule run, with its context and scope.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModScheduledRuleResult {
    pub context: LuaObject,
    pub result: ModRuleDispatchResult,
    pub scope: ModStateScope,
}

/// The saved rule engine state and the graph digest of each prepared mod.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModCheckpoint {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub rules: ModRuleEngineCheckpoint,
    pub graph_sha256: Vec<String>,
}

/// What one prepared mod step produced: whether it was accepted, intents, errors, the
/// invocation count and whether the budget was exceeded.
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

/// An event delivered to prepared mods: its name, payload, context and scope.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PreparedModEvent {
    pub context: LuaObject,
    pub event: String,
    pub payload: LuaValue,
    pub scope: ModStateScope,
}

/// One prepared mod step's input: its tick and events.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PreparedModStepInput {
    pub events: Vec<PreparedModEvent>,
    pub tick: u64,
}

/// The compiled mod catalog and mod state projected for one viewer.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModProjection {
    pub catalog: Vec<super::definition::CompiledWebLuaMod>,
    pub state: ModStateCheckpoint,
    pub viewer_id: String,
}

/// The saved host state of prepared mods: all mutable authored engines plus reducer state.
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

/// A presentation intent from a mod: its kind, fields, sequence and tick.
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

/// The kind of a presentation intent; `Present` is the only one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum PreparedModPresentationKind {
    Present,
}

/// A player action delivered to prepared mods: its name, payload, context, scope, request ID
/// and tick.
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
