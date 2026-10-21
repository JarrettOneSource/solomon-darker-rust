//! Immutable authored definitions from `modding/definition/web-lua-definition-types.ts`.
//! Reference identities use `web-lua-content-identity.ts`'s length-prefixed FNV-1a hash.

use super::{LuaObject, LuaValue};

pub const WEB_LUA_DEFINITION_API_VERSION: &str = "1.0.0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum WebLuaContentKind {
    Affix,
    AffixPool,
    Boneyard,
    Boast,
    Enemy,
    Item,
    Potion,
    Powerup,
    Room,
    Scene,
    SceneExtension,
    Shop,
    Skill,
    Spell,
    Status,
    Ui,
}

impl WebLuaContentKind {
    pub const ALL: [Self; 16] = [
        Self::Affix,
        Self::AffixPool,
        Self::Boneyard,
        Self::Boast,
        Self::Enemy,
        Self::Item,
        Self::Potion,
        Self::Powerup,
        Self::Room,
        Self::Scene,
        Self::SceneExtension,
        Self::Shop,
        Self::Skill,
        Self::Spell,
        Self::Status,
        Self::Ui,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Affix => "affix",
            Self::AffixPool => "affix-pool",
            Self::Boneyard => "boneyard",
            Self::Boast => "boast",
            Self::Enemy => "enemy",
            Self::Item => "item",
            Self::Potion => "potion",
            Self::Powerup => "powerup",
            Self::Room => "room",
            Self::Scene => "scene",
            Self::SceneExtension => "scene-extension",
            Self::Shop => "shop",
            Self::Skill => "skill",
            Self::Spell => "spell",
            Self::Status => "status",
            Self::Ui => "ui",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    #[must_use]
    pub const fn art_slots(self) -> &'static [&'static str] {
        match self {
            Self::Affix | Self::AffixPool | Self::SceneExtension | Self::Status | Self::Ui => &[],
            Self::Boneyard | Self::Scene => &["ambience", "layout", "loop", "music"],
            Self::Boast | Self::Potion | Self::Skill => &["icon"],
            Self::Enemy => &["atlas", "attack_sound", "death_sound", "sound"],
            Self::Item => &["icon", "icon_trim", "worn", "worn_trim"],
            Self::Powerup => &["sound", "world"],
            Self::Room => &["ambience", "loop", "music"],
            Self::Shop => &["npc"],
            Self::Spell => &["effect", "icon", "sound"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum WebLuaAssetKind {
    Boneyard,
    Music,
    Scene,
    Sheet,
    Sound,
    Sprite,
}

impl WebLuaAssetKind {
    pub const ALL: [Self; 6] =
        [Self::Boneyard, Self::Music, Self::Scene, Self::Sheet, Self::Sound, Self::Sprite];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boneyard => "boneyard",
            Self::Music => "music",
            Self::Scene => "scene",
            Self::Sheet => "sheet",
            Self::Sound => "sound",
            Self::Sprite => "sprite",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum WebLuaScopeKind {
    Entity,
    ParticipantProfile,
    ParticipantRun,
    PartyRun,
    Scene,
    Session,
}

impl WebLuaScopeKind {
    pub const ALL: [Self; 6] = [
        Self::Entity,
        Self::ParticipantProfile,
        Self::ParticipantRun,
        Self::PartyRun,
        Self::Scene,
        Self::Session,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::ParticipantProfile => "participant-profile",
            Self::ParticipantRun => "participant-run",
            Self::PartyRun => "party-run",
            Self::Scene => "scene",
            Self::Session => "session",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

pub const WEB_LUA_RULE_EVENT_NAMES: [&str; 19] = [
    "action.content.cast",
    "action.content.pickup",
    "action.content.use",
    "action.portal.enter",
    "action.scene.room",
    "action.shop.purchase",
    "action.ui.action",
    "enemy.death",
    "enemy.spawned",
    "gold.changed",
    "level.up",
    "mod.enemy.damaged",
    "mod.enemy.died",
    "player.control",
    "run.ended",
    "run.started",
    "session.started",
    "wave.completed",
    "wave.started",
];

pub const WEB_LUA_RULE_NAMES: [&str; 6] = ["on", "all", "first", "when", "after", "every"];
pub const WEB_LUA_EFFECT_NAMES: [&str; 9] =
    ["damage", "resource", "status", "spawn", "grant", "state", "present", "input", "select_skill"];
pub const WEB_LUA_PREFAB_NAMES: [&str; 5] = ["projectile", "area", "channel", "minimap", "portal"];
pub const WEB_LUA_SCHEMA_NAMES: [&str; 7] =
    ["boolean", "integer", "number", "string", "enum", "array", "object"];
pub const WEB_LUA_ART_ALIAS_NAMES: [&str; 5] = ["sprite", "sheet", "wearable", "sound", "music"];
pub const WEB_LUA_PREDICATE_FIELDS: [&str; 11] = [
    "above",
    "all",
    "any",
    "at_least",
    "at_most",
    "below",
    "context",
    "equals",
    "event",
    "none",
    "not_equals",
];

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WebLuaModIdentity {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "persistence",
    serde(tag = "kind", rename = "content-reference", rename_all = "camelCase")
)]
pub struct WebLuaContentReference {
    pub key: String,
    pub mod_id: Option<String>,
    pub target_kind: WebLuaContentKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename = "asset-reference"))]
pub struct WebLuaAssetReference {
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct WebLuaDefinitionSource {
    pub column: Option<u32>,
    pub file: String,
    pub line: Option<u32>,
    /// Creation order is diagnostic only and never enters a graph digest.
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub node: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "persistence",
    serde(tag = "kind", rename = "asset-definition", rename_all = "camelCase")
)]
pub struct WebLuaAssetDefinition {
    pub asset_kind: WebLuaAssetKind,
    pub fields: LuaObject,
    pub key: String,
    pub source: WebLuaDefinitionSource,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "persistence",
    serde(tag = "kind", rename = "content-definition", rename_all = "camelCase")
)]
pub struct WebLuaContentDefinition {
    pub content_kind: WebLuaContentKind,
    pub fields: LuaObject,
    pub key: String,
    pub source: WebLuaDefinitionSource,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename = "rule-definition"))]
pub struct WebLuaRuleDefinition {
    pub fields: LuaObject,
    pub operation: String,
    pub source: WebLuaDefinitionSource,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "persistence",
    serde(tag = "kind", rename = "schema-definition", rename_all = "camelCase")
)]
pub struct WebLuaSchemaDefinition {
    pub fields: LuaObject,
    pub schema_kind: String,
    pub source: WebLuaDefinitionSource,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "persistence",
    serde(tag = "kind", rename = "intent-definition", rename_all = "camelCase")
)]
pub struct WebLuaIntentDefinition {
    pub fields: LuaObject,
    pub intent_kind: String,
    pub source: WebLuaDefinitionSource,
}

/// Declarative reducer fields. Its function/migration handles remain in the VM,
/// indexed by `key`, rather than entering immutable graphs or saved values.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct WebLuaReducerDefinition {
    pub key: String,
    pub on: Vec<String>,
    pub schema_version: u64,
    pub scope: WebLuaScopeKind,
    pub source: WebLuaDefinitionSource,
    pub state: WebLuaSchemaDefinition,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WebLuaModDefinition {
    pub api: String,
    pub assets: Vec<WebLuaAssetDefinition>,
    pub content: Vec<WebLuaContentDefinition>,
    pub reducers: Vec<WebLuaReducerDefinition>,
    pub rules: Vec<WebLuaRuleDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "persistence",
    serde(tag = "kind", rename = "resolved-content-reference", rename_all = "camelCase")
)]
pub struct ResolvedWebLuaContentReference {
    pub content_id: String,
    pub key: String,
    pub mod_id: String,
    pub target_kind: WebLuaContentKind,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct CompiledWebLuaAsset {
    pub asset_kind: WebLuaAssetKind,
    pub fields: LuaObject,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct CompiledWebLuaContent {
    pub content_id: String,
    pub content_kind: WebLuaContentKind,
    pub fields: LuaObject,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct CompiledWebLuaReducer {
    pub key: String,
    pub on: Vec<String>,
    pub schema_version: u64,
    pub scope: WebLuaScopeKind,
    pub state: WebLuaSchemaDefinition,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct CompiledWebLuaMod {
    pub api_version: String,
    pub assets: Vec<CompiledWebLuaAsset>,
    pub canonical_json: String,
    pub capabilities: Vec<String>,
    pub content: Vec<CompiledWebLuaContent>,
    pub graph_sha256: String,
    pub identity: WebLuaModIdentity,
    pub reducers: Vec<CompiledWebLuaReducer>,
    pub rules: Vec<WebLuaRuleDefinition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct WebLuaDefinitionLimits {
    pub maximum_assets: usize,
    pub maximum_content: usize,
    pub maximum_depth: usize,
    pub maximum_nodes: usize,
    pub maximum_reducers: usize,
    pub maximum_rules: usize,
    pub maximum_string_bytes: usize,
}

impl Default for WebLuaDefinitionLimits {
    fn default() -> Self {
        Self {
            maximum_assets: 256,
            maximum_content: 4_096,
            maximum_depth: 32,
            maximum_nodes: 65_536,
            maximum_reducers: 128,
            maximum_rules: 4_096,
            maximum_string_bytes: 16 * 1_024,
        }
    }
}

/// `WEB_LUA_CONTENT_KEY_PATTERN`: ASCII key, 1..128 bytes, no edge separator.
#[must_use]
pub fn valid_web_lua_content_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 128 {
        return false;
    }
    let is_letter_or_digit = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    if !is_letter_or_digit(bytes[0]) || !is_letter_or_digit(bytes[bytes.len() - 1]) {
        return false;
    }
    bytes.iter().all(|&byte| is_letter_or_digit(byte) || matches!(byte, b'.' | b'_' | b'-'))
}

/// Stable decimal content identity, including its reserved bit-62 namespace.
#[must_use]
pub fn stable_web_lua_content_id(mod_id: &str, key: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut append = |bytes: &[u8]| {
        for &byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    append(b"sd.content.v1");
    for value in [mod_id, key] {
        append(&(value.len() as u32).to_le_bytes());
        append(value.as_bytes());
    }
    ((hash & ((1_u64 << 62) - 1)) | (1_u64 << 62)).to_string()
}

/// Definition tokens use the same JSON-compatible value representation as
/// reducer state. The compiler validates each token's discriminant and fields.
pub type WebLuaDefinitionValue = LuaValue;
