//! Native Boneyard enemy identities and construction requests.
//!
//! Reference: Website `core-kernels/boneyard-wave-schema.ts`, `boneyard-wave-types.ts`
//! and `boneyard-enemy-config-model.ts`. Spawn requests retain the authored wave fields;
//! gameplay configuration is evaluated once when an actor materializes.

use crate::Vec2;

pub mod actor_presentation;
pub mod actor_update;
pub mod archer;
pub mod attack_presentation;
pub mod boss_presentation;
pub mod boss_spell;
pub mod boss_spells;
pub mod cocoon;
pub mod coffin;
pub mod coffin_emergence;
pub mod combat;
pub mod config;
mod construction;
pub mod crow;
mod damage;
pub mod dampen;
mod death;
pub mod death_presentation;
pub mod deaths;
mod deferred_painters;
pub mod demon;
pub mod demon_skull;
pub mod demon_skull_actions;
mod demon_skull_attachments;
pub mod demon_skull_controller;
pub mod demon_skull_death;
pub mod demon_skull_effects;
pub mod demon_skull_spells;
pub mod effects;
pub mod faculty;
mod faculty_attachments;
pub mod faculty_controller;
pub mod faculty_death;
pub mod faculty_effects;
pub mod faculty_spells;
pub mod guided_missile;
pub mod heartmonger;
pub mod imp;
pub mod lightning_presentation;
pub mod mage;
mod mage_attachments;
pub mod maggots;
pub mod model;
pub mod movement;
pub mod path;
pub mod portal;
pub mod presentation;
pub mod projectile_effects;
pub mod projectile_emission;
pub mod projectile_hazards;
mod projectile_motion;
pub mod projectile_presentation;
pub mod projectiles;
pub mod puppet_hits;
pub mod rain_presentation;
pub mod ranged;
pub mod silk;
pub mod silk_force;
pub mod skeleton;
mod skeleton_death;
pub mod spider;
pub mod spider_movement;
pub mod spider_remains;
mod step;
mod storage;
pub mod targeting;
pub mod tendrils;
mod terminal_effects;
pub mod ultra_banish;
pub mod underlay;
pub mod voices;
pub mod web_presentation;
pub mod webbed;
pub mod world;
pub mod wraith;
pub mod zombie;

use config::AuthoredEnemyRecipe;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyToken {
    Coffin,
    Cocoon,
    Demon,
    DemonSkull,
    DireFaculty,
    Heartmonger,
    Imp,
    Portal,
    Skeleton,
    SkeletonArcher,
    SkeletonMage,
    Spider,
    Wraith,
    Zombie,
}

impl EnemyToken {
    pub const ALL: [Self; 14] = [
        Self::Coffin,
        Self::Cocoon,
        Self::Demon,
        Self::DemonSkull,
        Self::DireFaculty,
        Self::Heartmonger,
        Self::Imp,
        Self::Portal,
        Self::Skeleton,
        Self::SkeletonArcher,
        Self::SkeletonMage,
        Self::Spider,
        Self::Wraith,
        Self::Zombie,
    ];
    #[must_use]
    pub const fn native_type_id(self) -> u16 {
        match self {
            Self::Coffin => 1013,
            Self::Cocoon => 2058,
            Self::Demon => 1009,
            Self::DemonSkull => 1008,
            Self::DireFaculty => 1010,
            Self::Heartmonger => 1011,
            Self::Imp => 1004,
            Self::Portal => 5021,
            Self::Skeleton => 1001,
            Self::SkeletonArcher => 1002,
            Self::SkeletonMage => 1003,
            Self::Spider => 2057,
            Self::Wraith => 1007,
            Self::Zombie => 1006,
        }
    }
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Coffin => "COFFIN",
            Self::Cocoon => "COCOON",
            Self::Demon => "DEMON",
            Self::DemonSkull => "DEMONSKULL",
            Self::DireFaculty => "DIREFACULTY",
            Self::Heartmonger => "HEARTMONGER",
            Self::Imp => "IMP",
            Self::Portal => "PORTAL",
            Self::Skeleton => "SKELETON",
            Self::SkeletonArcher => "SKELETONARCHER",
            Self::SkeletonMage => "SKELETONMAGE",
            Self::Spider => "SPIDER",
            Self::Wraith => "WRAITH",
            Self::Zombie => "ZOMBIE",
        }
    }
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|token| token.id() == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyFlag {
    HpUp,
    HpDown,
    Strong,
    Weak,
    Fast,
    Slow,
    XpBonus,
    Burning,
    Helm,
    Horned,
    Hooded,
    Leading,
    Scattershot,
    RangeUp,
    RangeDown,
    RangeEasy,
    Shield,
    ShieldOthers,
    ShieldStrong,
    ShieldFast,
    Split,
    SplitMany,
    ManyMaggots,
    StrongMaggots,
    PoisonArrow,
    FireArrow,
    Armor,
    Sword,
    Mace,
    Flail,
    Axe,
    Pike,
    CastFire,
    CastLightning,
    CastFrost,
    CastPoison,
    Rotten,
    DeathImps,
    DeathImpsMany,
    ArmorMaybe,
    NoSkeletons,
    MoreSkeletons,
    RandomShot,
    Ignite,
    Immortalize,
    Cocoon,
    NoSpit,
}

impl EnemyFlag {
    pub const ALL: [Self; 47] = [
        Self::HpUp,
        Self::HpDown,
        Self::Strong,
        Self::Weak,
        Self::Fast,
        Self::Slow,
        Self::XpBonus,
        Self::Burning,
        Self::Helm,
        Self::Horned,
        Self::Hooded,
        Self::Leading,
        Self::Scattershot,
        Self::RangeUp,
        Self::RangeDown,
        Self::RangeEasy,
        Self::Shield,
        Self::ShieldOthers,
        Self::ShieldStrong,
        Self::ShieldFast,
        Self::Split,
        Self::SplitMany,
        Self::ManyMaggots,
        Self::StrongMaggots,
        Self::PoisonArrow,
        Self::FireArrow,
        Self::Armor,
        Self::Sword,
        Self::Mace,
        Self::Flail,
        Self::Axe,
        Self::Pike,
        Self::CastFire,
        Self::CastLightning,
        Self::CastFrost,
        Self::CastPoison,
        Self::Rotten,
        Self::DeathImps,
        Self::DeathImpsMany,
        Self::ArmorMaybe,
        Self::NoSkeletons,
        Self::MoreSkeletons,
        Self::RandomShot,
        Self::Ignite,
        Self::Immortalize,
        Self::Cocoon,
        Self::NoSpit,
    ];
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::HpUp => "FLAG_HPUP",
            Self::HpDown => "FLAG_HPDOWN",
            Self::Strong => "FLAG_STRONG",
            Self::Weak => "FLAG_WEAK",
            Self::Fast => "FLAG_FAST",
            Self::Slow => "FLAG_SLOW",
            Self::XpBonus => "FLAG_XPBONUS",
            Self::Burning => "FLAG_BURNING",
            Self::Helm => "FLAG_HELM",
            Self::Horned => "FLAG_HORNED",
            Self::Hooded => "FLAG_HOODED",
            Self::Leading => "FLAG_LEADING",
            Self::Scattershot => "FLAG_SCATTERSHOT",
            Self::RangeUp => "FLAG_RANGEUP",
            Self::RangeDown => "FLAG_RANGEDOWN",
            Self::RangeEasy => "FLAG_RANGEEASY",
            Self::Shield => "FLAG_SHIELD",
            Self::ShieldOthers => "FLAG_SHIELDOTHERS",
            Self::ShieldStrong => "FLAG_SHIELDSTRONG",
            Self::ShieldFast => "FLAG_SHIELDFAST",
            Self::Split => "FLAG_SPLIT",
            Self::SplitMany => "FLAG_SPLITMANY",
            Self::ManyMaggots => "FLAG_MANYMAGGOTS",
            Self::StrongMaggots => "FLAG_STRONGMAGGOTS",
            Self::PoisonArrow => "FLAG_POISONARROW",
            Self::FireArrow => "FLAG_FIREARROW",
            Self::Armor => "FLAG_ARMOR",
            Self::Sword => "FLAG_SWORD",
            Self::Mace => "FLAG_MACE",
            Self::Flail => "FLAG_FLAIL",
            Self::Axe => "FLAG_AXE",
            Self::Pike => "FLAG_PIKE",
            Self::CastFire => "FLAG_CASTFIRE",
            Self::CastLightning => "FLAG_CASTLIGHTNING",
            Self::CastFrost => "FLAG_CASTFROST",
            Self::CastPoison => "FLAG_CASTPOISON",
            Self::Rotten => "FLAG_ROTTEN",
            Self::DeathImps => "FLAG_DEATHIMPS",
            Self::DeathImpsMany => "FLAG_DEATHIMPSMANY",
            Self::ArmorMaybe => "FLAG_ARMORMAYBE",
            Self::NoSkeletons => "FLAG_NOSKELETONS",
            Self::MoreSkeletons => "FLAG_MORESKELETONS",
            Self::RandomShot => "FLAG_RANDOMSHOT",
            Self::Ignite => "FLAG_IGNITE",
            Self::Immortalize => "FLAG_IMMORTALIZE",
            Self::Cocoon => "FLAG_COCOON",
            Self::NoSpit => "FLAG_NOSPIT",
        }
    }
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|flag| flag.id() == id)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SpawnLocationPolicy {
    #[default]
    Anywhere,
    NearPlayer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SpawnPositionPolicy {
    Dark,
    #[default]
    Direct,
    Edge,
    Light,
    Offscreen,
}

/// Native portal children carry inherited damage and their direct launch trajectory.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PortalEjection {
    pub child_heading_deg: f64,
    pub inherited_primary_damage: f64,
    pub parent_heading_deg: f64,
    pub parent_position: Vec2,
    pub vertical_velocity: f64,
}

/// `BoneyardEnemySpawnIntent`: one consumed authored wave/group member.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemySpawnIntent {
    pub enemy_token: EnemyToken,
    pub flags: Vec<EnemyFlag>,
    pub id: usize,
    pub location_policy: SpawnLocationPolicy,
    pub native_type_id: u16,
    pub position: Vec2,
    pub position_policy: SpawnPositionPolicy,
    pub spawn_tick: u64,
    pub wave_ordinal: u64,
    pub authored_recipe: Option<AuthoredEnemyRecipe>,
    pub flanking: bool,
    pub pathfinding_mode: u8,
    pub mage_cloak: bool,
    pub archer_strafing: bool,
    pub zombie_body_type: u8,
    pub placement_group_id: Option<usize>,
    pub placement_radius: Option<f64>,
    pub navigation_clearance: Option<f64>,
    pub reachability_radius: Option<f64>,
    pub portal_ejection: Option<PortalEjection>,
    pub green_imp_spit_damage: Option<f64>,
    pub cocoon_target_player_id: Option<String>,
    pub enable_discorporeal_health_gates: bool,
}

impl EnemySpawnIntent {
    #[must_use]
    pub fn new(
        enemy_token: EnemyToken,
        position: Vec2,
        id: usize,
        spawn_tick: u64,
        wave_ordinal: u64,
    ) -> Self {
        Self {
            enemy_token,
            position,
            id,
            spawn_tick,
            wave_ordinal,
            native_type_id: enemy_token.native_type_id(),
            flags: Vec::new(),
            location_policy: SpawnLocationPolicy::Anywhere,
            position_policy: SpawnPositionPolicy::Direct,
            authored_recipe: None,
            flanking: true,
            pathfinding_mode: 1,
            mage_cloak: false,
            archer_strafing: false,
            zombie_body_type: 0,
            placement_group_id: None,
            placement_radius: None,
            navigation_clearance: None,
            reachability_radius: None,
            portal_ejection: None,
            green_imp_spit_damage: None,
            cocoon_target_player_id: None,
            enable_discorporeal_health_gates: false,
        }
    }
}
