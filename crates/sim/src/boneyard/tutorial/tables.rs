//! The Tutorial's authored tables: narration cues, monster recipes, UID groups, wave spawn
//! batches, scripted fires and IDs. The differential fixture pins all seven recipes, six
//! UID groups, six waves and 24 cues.

use crate::Vec2;
use crate::enemies::config::{
    AuthoredEnemyFamily, AuthoredEnemyRecipe, EnemyClassification, EnemyLootPolicies,
};
use crate::enemies::{EnemyToken, SpawnPositionPolicy};

pub const STOCK_TUTORIAL_BONEYARD_ID: &str = "stock-tutorial";
pub const TUTORIAL_ACID_RAIN_SKILL_ID: usize = 72;
pub const TUTORIAL_AMULET_DESCRIPTION: &str = "A dull trinket, carved with a few beneficial runes";
pub const TUTORIAL_TRIGGER_IDS: [u32; 13] =
    [10001, 10003, 10047, 10049, 10054, 10057, 10063, 10072, 10074, 10079, 10081, 10083, 642218];
pub const TUTORIAL_SCRIPT_IDS: [u32; 12] =
    [10000, 10002, 10048, 10050, 10055, 10058, 10064, 10073, 10075, 10080, 10084, 642219];
pub const TUTORIAL_SCRIPT_COMMAND_IDS: [u32; 18] = [
    1002, 1004, 1005, 1006, 1007, 1008, 1010, 1013, 1020, 1032, 1033, 1048, 1051, 1058, 1059, 1061,
    1065, 1066,
];

/// Who speaks a Tutorial narration cue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum TutorialSpeaker {
    Sirmin,
    Solomon,
}
impl TutorialSpeaker {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sirmin => "sirmin",
            Self::Solomon => "solomon",
        }
    }
}

/// One of the 24 Tutorial narration cues; each has a stable ID and an authored definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum TutorialCue {
    ShowYourself,
    OhBoyAnotherWizard,
    BeenDispatched,
    DoTheDispatching,
    YourPerversions,
    ToDeathExactly,
    SolomonLaugh1,
    CowardComeBack,
    SolomonGetHimBoys,
    IAmSirmin,
    NeverHeardOfYou,
    EasilyVanquished,
    CamePrepared,
    AcidRainHuh,
    Surrender,
    CarelessFool,
    Unredeemable,
    SoundLikeMother,
    AcceptYourFate,
    MakeMeStronger,
    LevellingUp,
    LookingBeatUp,
    FaceTheWrath,
    ImBored,
}

/// A narration cue's speaker, spoken text and PCM-derived duration in ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TutorialCueDefinition {
    pub duration_ticks: u64,
    pub speaker: TutorialSpeaker,
    pub text: &'static str,
}

impl TutorialCue {
    pub const ALL: [Self; 24] = [
        Self::ShowYourself,
        Self::OhBoyAnotherWizard,
        Self::BeenDispatched,
        Self::DoTheDispatching,
        Self::YourPerversions,
        Self::ToDeathExactly,
        Self::SolomonLaugh1,
        Self::CowardComeBack,
        Self::SolomonGetHimBoys,
        Self::IAmSirmin,
        Self::NeverHeardOfYou,
        Self::EasilyVanquished,
        Self::CamePrepared,
        Self::AcidRainHuh,
        Self::Surrender,
        Self::CarelessFool,
        Self::Unredeemable,
        Self::SoundLikeMother,
        Self::AcceptYourFate,
        Self::MakeMeStronger,
        Self::LevellingUp,
        Self::LookingBeatUp,
        Self::FaceTheWrath,
        Self::ImBored,
    ];
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ShowYourself => "tutorial-show-yourself",
            Self::OhBoyAnotherWizard => "tutorial-oh-boy-another-wizard",
            Self::BeenDispatched => "tutorial-been-dispatched",
            Self::DoTheDispatching => "tutorial-do-the-dispatching",
            Self::YourPerversions => "tutorial-your-perversions",
            Self::ToDeathExactly => "tutorial-to-death-exactly",
            Self::SolomonLaugh1 => "solomon-laugh-1",
            Self::CowardComeBack => "tutorial-coward-come-back",
            Self::SolomonGetHimBoys => "solomon-get-him-boys",
            Self::IAmSirmin => "tutorial-i-am-sirmin",
            Self::NeverHeardOfYou => "tutorial-never-heard-of-you",
            Self::EasilyVanquished => "tutorial-easily-vanquished",
            Self::CamePrepared => "tutorial-came-prepared",
            Self::AcidRainHuh => "tutorial-acid-rain-huh",
            Self::Surrender => "tutorial-surrender",
            Self::CarelessFool => "tutorial-careless-fool",
            Self::Unredeemable => "tutorial-unredeemable",
            Self::SoundLikeMother => "tutorial-sound-like-mother",
            Self::AcceptYourFate => "tutorial-accept-your-fate",
            Self::MakeMeStronger => "tutorial-make-me-stronger",
            Self::LevellingUp => "tutorial-levelling-up",
            Self::LookingBeatUp => "tutorial-looking-beat-up",
            Self::FaceTheWrath => "tutorial-face-the-wrath",
            Self::ImBored => "tutorial-im-bored",
        }
    }
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|cue| cue.id() == id)
    }
    #[must_use]
    pub const fn definition(self) -> TutorialCueDefinition {
        match self {
            Self::ShowYourself => TutorialCueDefinition {
                duration_ticks: 280,
                speaker: TutorialSpeaker::Sirmin,
                text: "SOLOMON DARK!  Show yourself!",
            },
            Self::OhBoyAnotherWizard => TutorialCueDefinition {
                duration_ticks: 844,
                speaker: TutorialSpeaker::Solomon,
                text: "Oh boy, another pointy head from Wizard School.  What is it this time?",
            },
            Self::BeenDispatched => TutorialCueDefinition {
                duration_ticks: 547,
                speaker: TutorialSpeaker::Sirmin,
                text: "Solomon Dark, I have been dispatched here by the College to make you answer for your crimes!",
            },
            Self::DoTheDispatching => TutorialCueDefinition {
                duration_ticks: 267,
                speaker: TutorialSpeaker::Solomon,
                text: "I'll do the dispatching",
            },
            Self::YourPerversions => TutorialCueDefinition {
                duration_ticks: 782,
                speaker: TutorialSpeaker::Sirmin,
                text: "For your perversions against nature, and for the foul murders of the junior mages Morth, Aliss, and Lucritius, I sentence you... TO DEATH!",
            },
            Self::ToDeathExactly => TutorialCueDefinition {
                duration_ticks: 614,
                speaker: TutorialSpeaker::Solomon,
                text: "To death!  Exactly what I had in mind!  Let's get it on!",
            },
            Self::SolomonLaugh1 => TutorialCueDefinition {
                duration_ticks: 247,
                speaker: TutorialSpeaker::Solomon,
                text: "(Laughs Wildly)",
            },
            Self::CowardComeBack => TutorialCueDefinition {
                duration_ticks: 220,
                speaker: TutorialSpeaker::Sirmin,
                text: "Coward!  Come back!",
            },
            Self::SolomonGetHimBoys => TutorialCueDefinition {
                duration_ticks: 245,
                speaker: TutorialSpeaker::Solomon,
                text: "Get him, boys!",
            },
            Self::IAmSirmin => TutorialCueDefinition {
                duration_ticks: 597,
                speaker: TutorialSpeaker::Sirmin,
                text: "Your foul necromancy cannot stand against me, for I am Sirmin the Wizard!",
            },
            Self::NeverHeardOfYou => TutorialCueDefinition {
                duration_ticks: 621,
                speaker: TutorialSpeaker::Solomon,
                text: "I've never heard of you, and nobody else will either.",
            },
            Self::EasilyVanquished => TutorialCueDefinition {
                duration_ticks: 446,
                speaker: TutorialSpeaker::Sirmin,
                text: "Ha!  Your so called skeletal hordes are easily vanquished!",
            },
            Self::CamePrepared => TutorialCueDefinition {
                duration_ticks: 299,
                speaker: TutorialSpeaker::Sirmin,
                text: "You see that I came prepared!  I am no neophyte mage, to be awed by a few undead!",
            },
            Self::AcidRainHuh => TutorialCueDefinition {
                duration_ticks: 406,
                speaker: TutorialSpeaker::Solomon,
                text: "Acid rain, huh?  That's kid stuff!",
            },
            Self::Surrender => TutorialCueDefinition {
                duration_ticks: 555,
                speaker: TutorialSpeaker::Sirmin,
                text: "I tire of this!  Surrender and your death will be painless, I swear it.",
            },
            Self::CarelessFool => TutorialCueDefinition {
                duration_ticks: 295,
                speaker: TutorialSpeaker::Sirmin,
                text: "You fool!  Your slave has dropped something precious, I see!",
            },
            Self::Unredeemable => TutorialCueDefinition {
                duration_ticks: 691,
                speaker: TutorialSpeaker::Sirmin,
                text: "Now I bear the amulet!  You are too clumsy and careless to call yourself a wizard!",
            },
            Self::SoundLikeMother => TutorialCueDefinition {
                duration_ticks: 250,
                speaker: TutorialSpeaker::Solomon,
                text: "Geez, you sound like my mother.",
            },
            Self::AcceptYourFate => TutorialCueDefinition {
                duration_ticks: 307,
                speaker: TutorialSpeaker::Sirmin,
                text: "You cannot prevail.  Accept your fate!",
            },
            Self::MakeMeStronger => TutorialCueDefinition {
                duration_ticks: 337,
                speaker: TutorialSpeaker::Sirmin,
                text: "Your depraved magic only serves to make me stronger, Dark Lord!",
            },
            Self::LevellingUp => TutorialCueDefinition {
                duration_ticks: 469,
                speaker: TutorialSpeaker::Solomon,
                text: "Levelling up is not going to save you.",
            },
            Self::LookingBeatUp => TutorialCueDefinition {
                duration_ticks: 578,
                speaker: TutorialSpeaker::Solomon,
                text: "You're looking a little beat up there, son.",
            },
            Self::FaceTheWrath => TutorialCueDefinition {
                duration_ticks: 727,
                speaker: TutorialSpeaker::Sirmin,
                text: "Now face the wrath of a fully restored and angry wizard!",
            },
            Self::ImBored => TutorialCueDefinition {
                duration_ticks: 693,
                speaker: TutorialSpeaker::Solomon,
                text: "And... I'm bored.  Destroy him, my skeletons!",
            },
        }
    }
}

/// An authored Tutorial enemy: its enemy token, flanking flag, pathfinding mode and recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct TutorialMonsterRecipe {
    pub enemy_token: EnemyToken,
    pub flanking: bool,
    pub pathfinding_mode: u8,
    pub recipe: AuthoredEnemyRecipe,
}

pub const TUTORIAL_MONSTER_UIDS: [usize; 7] = [10004, 10051, 10059, 10065, 10076, 10077, 10085];

#[must_use]
pub fn tutorial_monster_recipe(uid: usize) -> Option<TutorialMonsterRecipe> {
    let (enemy_token, pathfinding_mode, recipe) = match uid {
        10004 => (
            EnemyToken::Skeleton,
            2,
            AuthoredEnemyRecipe {
                uid: 10004,
                name: "Starter SKELETON".to_owned(),
                classification: EnemyClassification::Normal,
                maximum_health: 2.0,
                primary_damage: 1.0,
                secondary_damage: 0.0,
                tertiary_damage: 0.0,
                extra_damage: 0.0,
                attack_speed: 1.0,
                chase_speed: 1.100000023841858,
                movement_scale: 1.0,
                experience_bonus: 0.0,
                archer_accuracy_mode: 0,
                family: AuthoredEnemyFamily::Default,
                loot_policies: EnemyLootPolicies {
                    gold: 4,
                    item: 4,
                    orb: 4,
                    potion: 4,
                    powerup: 4,
                    specific_item: 0,
                },
                miniboss_die_program: false,
            },
        ),
        10051 => (
            EnemyToken::Skeleton,
            2,
            AuthoredEnemyRecipe {
                uid: 10051,
                name: "Item Skeleton".to_owned(),
                classification: EnemyClassification::Normal,
                maximum_health: 2.0,
                primary_damage: 1.0,
                secondary_damage: 0.0,
                tertiary_damage: 0.0,
                extra_damage: 0.0,
                attack_speed: 1.0,
                chase_speed: 1.100000023841858,
                movement_scale: 1.0,
                experience_bonus: 0.0,
                archer_accuracy_mode: 0,
                family: AuthoredEnemyFamily::Default,
                loot_policies: EnemyLootPolicies {
                    gold: 4,
                    item: 4,
                    orb: 4,
                    potion: 4,
                    powerup: 4,
                    specific_item: 0,
                },
                miniboss_die_program: false,
            },
        ),
        10059 => (
            EnemyToken::SkeletonArcher,
            2,
            AuthoredEnemyRecipe {
                uid: 10059,
                name: "SKELETAL ARCHER".to_owned(),
                classification: EnemyClassification::Normal,
                maximum_health: 3.0,
                primary_damage: 2.0,
                secondary_damage: 0.0,
                tertiary_damage: 0.0,
                extra_damage: 0.0,
                attack_speed: 1.0,
                chase_speed: 1.0,
                movement_scale: 1.0,
                experience_bonus: 0.0,
                archer_accuracy_mode: 1,
                family: AuthoredEnemyFamily::Default,
                loot_policies: EnemyLootPolicies {
                    gold: 4,
                    item: 4,
                    orb: 4,
                    potion: 4,
                    powerup: 4,
                    specific_item: 0,
                },
                miniboss_die_program: false,
            },
        ),
        10065 => (
            EnemyToken::Skeleton,
            1,
            AuthoredEnemyRecipe {
                uid: 10065,
                name: "Potion SKELETON".to_owned(),
                classification: EnemyClassification::Normal,
                maximum_health: 4.0,
                primary_damage: 1.0,
                secondary_damage: 0.0,
                tertiary_damage: 0.0,
                extra_damage: 0.0,
                attack_speed: 1.0,
                chase_speed: 1.0,
                movement_scale: 1.0,
                experience_bonus: 0.0,
                archer_accuracy_mode: 0,
                family: AuthoredEnemyFamily::Default,
                loot_policies: EnemyLootPolicies {
                    gold: 4,
                    item: 4,
                    orb: 4,
                    potion: 4,
                    powerup: 4,
                    specific_item: 0,
                },
                miniboss_die_program: false,
            },
        ),
        10076 => (
            EnemyToken::Skeleton,
            1,
            AuthoredEnemyRecipe {
                uid: 10076,
                name: "SKELETON".to_owned(),
                classification: EnemyClassification::Normal,
                maximum_health: 3.0,
                primary_damage: 1.0,
                secondary_damage: 0.0,
                tertiary_damage: 0.0,
                extra_damage: 0.0,
                attack_speed: 1.0,
                chase_speed: 1.0,
                movement_scale: 1.0,
                experience_bonus: 0.0,
                archer_accuracy_mode: 0,
                family: AuthoredEnemyFamily::Default,
                loot_policies: EnemyLootPolicies {
                    gold: 0,
                    item: 0,
                    orb: 0,
                    potion: 4,
                    powerup: 4,
                    specific_item: 0,
                },
                miniboss_die_program: false,
            },
        ),
        10077 => (
            EnemyToken::SkeletonArcher,
            1,
            AuthoredEnemyRecipe {
                uid: 10077,
                name: "SKELETAL ARCHER".to_owned(),
                classification: EnemyClassification::Normal,
                maximum_health: 4.0,
                primary_damage: 3.0,
                secondary_damage: 0.0,
                tertiary_damage: 0.0,
                extra_damage: 0.0,
                attack_speed: 1.0,
                chase_speed: 1.0,
                movement_scale: 1.0,
                experience_bonus: 0.0,
                archer_accuracy_mode: 0,
                family: AuthoredEnemyFamily::Default,
                loot_policies: EnemyLootPolicies {
                    gold: 0,
                    item: 0,
                    orb: 0,
                    potion: 1,
                    powerup: 4,
                    specific_item: 0,
                },
                miniboss_die_program: false,
            },
        ),
        10085 => (
            EnemyToken::SkeletonArcher,
            1,
            AuthoredEnemyRecipe {
                uid: 10085,
                name: "DEADLY SKELETAL ARCHER".to_owned(),
                classification: EnemyClassification::Normal,
                maximum_health: 4.0,
                primary_damage: 3.0,
                secondary_damage: 0.0,
                tertiary_damage: 0.0,
                extra_damage: 0.0,
                attack_speed: 1.0,
                chase_speed: 1.0,
                movement_scale: 1.0,
                experience_bonus: 0.0,
                archer_accuracy_mode: 1,
                family: AuthoredEnemyFamily::Default,
                loot_policies: EnemyLootPolicies {
                    gold: 0,
                    item: 0,
                    orb: 0,
                    potion: 0,
                    powerup: 4,
                    specific_item: 0,
                },
                miniboss_die_program: false,
            },
        ),
        _ => return None,
    };
    Some(TutorialMonsterRecipe { enemy_token, flanking: true, pathfinding_mode, recipe })
}

/// An authored Tutorial spawn group: its member recipe UIDs, whether the members share one
/// placement group, and a recorded position-cache word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TutorialUidGroup {
    pub member_uids: &'static [usize],
    pub position_cache_dword: u32,
    pub share_final_root: bool,
}

pub const TUTORIAL_GROUP_UIDS: [usize; 6] = [10010, 10052, 10060, 10061, 10078, 10086];
#[must_use]
pub fn tutorial_uid_group(uid: usize) -> Option<TutorialUidGroup> {
    Some(match uid {
        10010 => TutorialUidGroup {
            member_uids: &[10004, 10004, 10004, 10004, 10004],
            position_cache_dword: 0x0,
            share_final_root: false,
        },
        10052 => TutorialUidGroup {
            member_uids: &[10051, 10051, 10051, 10051, 10051],
            position_cache_dword: 0x0,
            share_final_root: false,
        },
        10060 => TutorialUidGroup {
            member_uids: &[10059, 10059, 10004, 10004, 10004],
            position_cache_dword: 0x0,
            share_final_root: false,
        },
        10061 => TutorialUidGroup {
            member_uids: &[10059, 10059, 10059],
            position_cache_dword: 0xcdcdcdcd,
            share_final_root: true,
        },
        10078 => TutorialUidGroup {
            member_uids: &[10076, 10077, 10076],
            position_cache_dword: 0x0,
            share_final_root: false,
        },
        10086 => TutorialUidGroup {
            member_uids: &[10085, 10076],
            position_cache_dword: 0x0,
            share_final_root: false,
        },
        _ => return None,
    })
}

/// One authored Tutorial wave spawn: a recipe or UID group, an optional count, its position
/// policy and the tick after the wave starts at which it spawns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TutorialSpawnBatch {
    pub count: Option<usize>,
    pub group_uid: Option<usize>,
    pub position_policy: SpawnPositionPolicy,
    pub recipe_uid: Option<usize>,
    pub tick: u64,
}

#[must_use]
pub fn tutorial_wave_batches(wave_ordinal: u64) -> &'static [TutorialSpawnBatch] {
    match wave_ordinal {
        1 => &[
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10010),
                position_policy: SpawnPositionPolicy::Dark,
                recipe_uid: None,
                tick: 0,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10010),
                position_policy: SpawnPositionPolicy::Dark,
                recipe_uid: None,
                tick: 0,
            },
        ],
        2 => &[
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10052),
                position_policy: SpawnPositionPolicy::Offscreen,
                recipe_uid: None,
                tick: 0,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10052),
                position_policy: SpawnPositionPolicy::Offscreen,
                recipe_uid: None,
                tick: 200,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10052),
                position_policy: SpawnPositionPolicy::Offscreen,
                recipe_uid: None,
                tick: 400,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10052),
                position_policy: SpawnPositionPolicy::Light,
                recipe_uid: None,
                tick: 1300,
            },
        ],
        3 => &[
            TutorialSpawnBatch {
                count: Some(3),
                group_uid: Some(10010),
                position_policy: SpawnPositionPolicy::Dark,
                recipe_uid: None,
                tick: 0,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10010),
                position_policy: SpawnPositionPolicy::Offscreen,
                recipe_uid: None,
                tick: 0,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10010),
                position_policy: SpawnPositionPolicy::Offscreen,
                recipe_uid: None,
                tick: 400,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10010),
                position_policy: SpawnPositionPolicy::Offscreen,
                recipe_uid: None,
                tick: 800,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10010),
                position_policy: SpawnPositionPolicy::Offscreen,
                recipe_uid: None,
                tick: 1200,
            },
        ],
        4 => &[
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10010),
                position_policy: SpawnPositionPolicy::Dark,
                recipe_uid: None,
                tick: 0,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10060),
                position_policy: SpawnPositionPolicy::Dark,
                recipe_uid: None,
                tick: 0,
            },
            TutorialSpawnBatch {
                count: None,
                group_uid: Some(10061),
                position_policy: SpawnPositionPolicy::Dark,
                recipe_uid: None,
                tick: 500,
            },
        ],
        5 => &[TutorialSpawnBatch {
            count: None,
            group_uid: None,
            position_policy: SpawnPositionPolicy::Light,
            recipe_uid: Some(10065),
            tick: 0,
        }],
        6 => &[],
        _ => &[],
    }
}

/// A scripted fire placed when a Tutorial run starts: its position, radius, damage and
/// lifetime.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TutorialFire {
    pub damage: f64,
    pub lifetime_ticks: u64,
    pub position: Vec2,
    pub radius: f64,
}

pub const TUTORIAL_FIRES: [TutorialFire; 2] = [
    TutorialFire {
        damage: 1.0,
        lifetime_ticks: 1000,
        position: Vec2::new(1766.1005859375, 147.63815307617188),
        radius: 100.0,
    },
    TutorialFire {
        damage: 1.0,
        lifetime_ticks: 1000,
        position: Vec2::new(1852.1005859375, 199.63815307617188),
        radius: 100.0,
    },
];
