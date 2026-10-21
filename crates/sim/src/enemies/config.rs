//! Authored enemy recipe data from Website `boneyard-enemy-config-model.ts`.

use super::{EnemyFlag, EnemySpawnIntent, EnemyToken};
use crate::float::fround;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyClassification {
    #[default]
    Normal,
    Boss,
    Miniboss,
    MultipleBoss,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SkeletonWeapon {
    #[default]
    Claw,
    Sword,
    Mace,
    Flail,
    Axe,
    Pike,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum ArrowType {
    #[default]
    Normal,
    Fire,
    Poison,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum MageElement {
    #[default]
    Fire,
    Lightning,
    Frost,
    Poison,
}

/// Native drop selectors 0..5; specific_item retains the authored item id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyLootPolicies {
    pub gold: u8,
    pub item: u8,
    pub orb: u8,
    pub potion: u8,
    pub powerup: u8,
    pub specific_item: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct FacultyAppearance {
    pub body_color: [f64; 4],
    pub head_color: [f64; 4],
    pub female: bool,
    pub headgear: u8,
    pub primary: u8,
    pub secondary: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum AuthoredEnemyFamily {
    #[default]
    Default,
    DemonSkull {
        capabilities: u8,
    },
    Faculty(FacultyAppearance),
    Heartmonger {
        crow_count: usize,
        blind_mode: u8,
        summon_mode: u8,
    },
    Skeleton {
        armor: bool,
        headgear: u8,
        weapon: SkeletonWeapon,
    },
    Archer {
        arrow_type: ArrowType,
        extra_arrows: usize,
        headgear: u8,
        multi_arrow_mode: u8,
        range_mode: u8,
        strafing: bool,
    },
    Portal {
        frequency: u8,
    },
    Zombie {
        body_type: u8,
        flyblown: bool,
        poison_duration: f64,
        poison_pool_damage: f64,
        poison_punch_damage: f64,
    },
}

/// A parsed retail MonsterRecipe. The wave director supplies data; enemy construction
/// applies the recipe, then source flags in order, then arena scalars.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct AuthoredEnemyRecipe {
    pub uid: usize,
    pub name: String,
    pub classification: EnemyClassification,
    pub maximum_health: f64,
    pub primary_damage: f64,
    pub secondary_damage: f64,
    pub tertiary_damage: f64,
    pub extra_damage: f64,
    pub attack_speed: f64,
    pub chase_speed: f64,
    pub movement_scale: f64,
    pub experience_bonus: f64,
    pub archer_accuracy_mode: u8,
    pub family: AuthoredEnemyFamily,
    pub loot_policies: EnemyLootPolicies,
    pub miniboss_die_program: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyArenaScalars {
    pub attack_speed: f64,
    pub chase_speed: f64,
    pub experience: f64,
    pub extra_damage: f64,
    pub health: f64,
    pub primary_damage: f64,
    pub secondary_damage: f64,
    pub tertiary_damage: f64,
}

impl Default for EnemyArenaScalars {
    fn default() -> Self {
        Self {
            attack_speed: 1.0,
            chase_speed: 1.0,
            experience: 1.0,
            extra_damage: 1.0,
            health: 1.0,
            primary_damage: 1.0,
            secondary_damage: 1.0,
            tertiary_damage: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnemyConfigRandom {
    pub base_speed_unit: f64,
    pub collision_radius_unit: f64,
    pub random_armor: bool,
    pub split_many_gate_unit: f64,
    pub split_many_unit: f64,
    pub split_unit: u8,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyFamilyConfig {
    Skeleton {
        armor: bool,
        headgear: u8,
        weapon: SkeletonWeapon,
    },
    Archer {
        accuracy_mode: u8,
        arrow_type: ArrowType,
        extra_arrows: usize,
        headgear: u8,
        multi_arrow_mode: u8,
        range_mode: u8,
        strafing: bool,
    },
    Mage {
        cloak: bool,
        element: MageElement,
        headgear: u8,
        other_shield: bool,
        other_shield_health: f64,
        range_mode: u8,
        self_shield: bool,
        self_shield_health: f64,
        shield_interval: f64,
    },
    Imp {
        split_depth: i64,
    },
    Zombie {
        body_type: u8,
        poison_duration: f64,
        poison_pool_damage: f64,
        poison_punch_damage: f64,
        rotten: bool,
    },
    Wraith,
    Demon {
        split_count: i64,
    },
    Coffin {
        maggot_damage: f64,
        maggot_health: f64,
        maggot_poison_damage: f64,
        maximum_maggots: usize,
    },
    Portal {
        frequency: u8,
    },
    Heartmonger {
        crow_count: usize,
        blind_mode: u8,
        summon_mode: u8,
    },
    Faculty(FacultyAppearance),
    DemonSkull {
        capabilities: u8,
    },
    Spider {
        cocoon_health: f64,
        spit_webs: bool,
        suck_damage_per_second: f64,
    },
    Cocoon,
}

/// The one evaluated configuration an enemy retains for its entire lifetime.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyConfig {
    pub enemy_token: EnemyToken,
    pub native_type_id: u16,
    pub family: EnemyFamilyConfig,
    pub attack_speed: f64,
    pub base_speed: f64,
    pub burning: bool,
    pub chase_speed: f64,
    pub classification: EnemyClassification,
    pub collision_radius: f64,
    pub experience: f64,
    pub extra_damage: f64,
    pub flags: Vec<EnemyFlag>,
    pub flanking: bool,
    pub ignored_source_flags: Vec<EnemyFlag>,
    pub maximum_health: f64,
    pub pathfinding_mode: u8,
    pub primary_damage: Option<f64>,
    pub recipe_name: Option<String>,
    pub recipe_uid: Option<usize>,
    pub loot_policies: EnemyLootPolicies,
    pub scale: f64,
    pub secondary_damage: f64,
    pub tertiary_damage: f64,
    pub miniboss_die_program: bool,
}

/// `evaluateBoneyardEnemyConfig`: constructor stats, authored recipe, flags in order,
/// then arena scalars. Numeric narrowing follows the Website, not the older base-speed
/// helper whose native float-store contract is used by the native movement fixtures.
#[must_use]
pub fn evaluate_enemy_config(
    intent: &EnemySpawnIntent,
    scalars: EnemyArenaScalars,
    random: EnemyConfigRandom,
) -> EnemyConfig {
    let token = intent.enemy_token;
    let (health, damage, chase_speed) = match token {
        EnemyToken::Coffin => (100.0, None, 1.0),
        EnemyToken::Cocoon => (999_999.0, None, 1.0),
        EnemyToken::Demon => (400.0, Some(20.0), 1.0),
        EnemyToken::Imp => (1.0, Some(3.0), 1.0),
        EnemyToken::Portal => (1.0, Some(2.0), 1.0),
        EnemyToken::Skeleton => (5.0, Some(3.0), 1.0),
        EnemyToken::SkeletonArcher => (5.0, Some(4.0), 1.0),
        EnemyToken::SkeletonMage => (5.0, Some(3.0), 0.8),
        EnemyToken::Spider => (15.0, Some(8.0), 1.0),
        EnemyToken::Wraith => (2.0, Some(4.0), 1.0),
        EnemyToken::Zombie => (105.0, Some(35.0), 1.0),
        EnemyToken::DireFaculty | EnemyToken::Heartmonger => (8.0, Some(25.0), 1.0),
        EnemyToken::DemonSkull => (50.0, Some(25.0), 0.5),
    };
    let mut mutable = MutableEnemyConfig::new(intent, health, damage.unwrap_or(0.0), chase_speed);
    if let Some(recipe) = &intent.authored_recipe {
        mutable.accuracy_mode = recipe.archer_accuracy_mode;
        mutable.attack_speed = recipe.attack_speed;
        mutable.chase_speed = recipe.chase_speed;
        mutable.experience_bonus = recipe.experience_bonus;
        mutable.extra_damage = recipe.extra_damage;
        mutable.maximum_health = recipe.maximum_health;
        mutable.primary_damage = recipe.primary_damage;
        mutable.secondary_damage = recipe.secondary_damage;
        mutable.tertiary_damage = recipe.tertiary_damage;
        mutable.apply_authored_family(&recipe.family);
    }
    for flag in &intent.flags {
        mutable.apply_flag(*flag, random, intent.wave_ordinal);
    }
    mutable.maximum_health *= scalars.health;
    mutable.primary_damage *= scalars.primary_damage;
    mutable.secondary_damage *= scalars.secondary_damage;
    mutable.tertiary_damage *= scalars.tertiary_damage;
    mutable.extra_damage *= scalars.extra_damage;
    mutable.chase_speed *= scalars.chase_speed;
    mutable.attack_speed *= scalars.attack_speed;
    mutable.experience_bonus *= scalars.experience;
    assert!(!mutable.dormant_skeleton_policy, "unsupported dormant skeleton policy");
    assert!(
        token != EnemyToken::Portal
            || matches!(
                intent.authored_recipe.as_ref().map(|r| &r.family),
                Some(AuthoredEnemyFamily::Portal { .. })
            ),
        "Portal requires an authored Portal recipe"
    );
    let authored_zombie = token == EnemyToken::Zombie
        && matches!(
            intent.authored_recipe.as_ref().map(|r| &r.family),
            Some(AuthoredEnemyFamily::Zombie { .. })
        );
    assert!(
        authored_zombie
            || matches!(token, EnemyToken::Spider | EnemyToken::DemonSkull)
            || (mutable.tertiary_damage == 0.0 && mutable.extra_damage == 0.0),
        "unsupported dormant tertiary/extra enemy damage payload"
    );
    let family = mutable.family(token);
    let recipe = intent.authored_recipe.as_ref();
    EnemyConfig {
        enemy_token: token,
        native_type_id: token.native_type_id(),
        family,
        attack_speed: mutable.attack_speed,
        base_speed: constructor_base_speed(token, random.base_speed_unit),
        burning: mutable.burning,
        chase_speed: mutable.chase_speed,
        classification: recipe.map_or(EnemyClassification::Normal, |r| r.classification),
        collision_radius: constructor_collision_radius(token, random.collision_radius_unit),
        experience: if token == EnemyToken::Cocoon {
            0.0
        } else {
            fround((mutable.maximum_health + mutable.experience_bonus) * (0.85_f32 as f64))
        },
        extra_damage: mutable.extra_damage,
        flags: intent.flags.clone(),
        flanking: intent.flanking,
        ignored_source_flags: intent
            .flags
            .iter()
            .copied()
            .filter(|flag| matches!(flag, EnemyFlag::Ignite | EnemyFlag::Immortalize))
            .collect(),
        maximum_health: mutable.maximum_health,
        pathfinding_mode: intent.pathfinding_mode,
        primary_damage: damage.map(|_| mutable.primary_damage),
        recipe_name: recipe.map(|r| r.name.clone()),
        recipe_uid: recipe.map(|r| r.uid),
        loot_policies: recipe.map_or(EnemyLootPolicies::default(), |r| r.loot_policies),
        scale: recipe.map_or(1.0, |r| r.movement_scale),
        secondary_damage: mutable.secondary_damage,
        tertiary_damage: mutable.tertiary_damage,
        miniboss_die_program: recipe.is_some_and(|r| r.miniboss_die_program),
    }
}

struct MutableEnemyConfig {
    spit_webs: bool,
    demon_skull_capabilities: u8,
    faculty: FacultyAppearance,
    crow_count: usize,
    blind_mode: u8,
    summon_mode: u8,
    armor: bool,
    attack_speed: f64,
    burning: bool,
    chase_speed: f64,
    cloak: bool,
    experience_bonus: f64,
    extra_damage: f64,
    headgear: u8,
    maximum_health: f64,
    primary_damage: f64,
    secondary_damage: f64,
    dormant_skeleton_policy: bool,
    tertiary_damage: f64,
    weapon: SkeletonWeapon,
    accuracy_mode: u8,
    arrow_type: ArrowType,
    extra_arrows: usize,
    multi_arrow_mode: u8,
    range_mode: u8,
    strafing: bool,
    mage_element: MageElement,
    other_shield: bool,
    other_shield_health: f64,
    self_shield: bool,
    self_shield_health: f64,
    shield_interval: f64,
    split_count: i64,
    body_type: u8,
    poison_duration: f64,
    poison_pool_damage: f64,
    poison_punch_damage: f64,
    portal_frequency: u8,
    rotten: bool,
    maggot_damage: f64,
    maggot_health: f64,
    maggot_poison_damage: f64,
    maximum_maggots: usize,
}

impl MutableEnemyConfig {
    fn new(intent: &EnemySpawnIntent, health: f64, damage: f64, chase_speed: f64) -> Self {
        Self {
            spit_webs: true,
            demon_skull_capabilities: 0,
            faculty: FacultyAppearance {
                body_color: desaturate_color([1.0, 0.5, 0.0, 1.0], 0.6_f32 as f64),
                head_color: [1.0; 4],
                female: false,
                headgear: 0,
                primary: 0,
                secondary: 0,
            },
            crow_count: 1,
            blind_mode: 2,
            summon_mode: 0,
            armor: false,
            attack_speed: 1.0,
            burning: false,
            chase_speed,
            cloak: intent.mage_cloak,
            experience_bonus: 0.0,
            extra_damage: if intent.enemy_token == EnemyToken::DemonSkull { 2.0 } else { 0.0 },
            headgear: 0,
            maximum_health: health,
            primary_damage: damage,
            secondary_damage: if matches!(
                intent.enemy_token,
                EnemyToken::Spider | EnemyToken::DemonSkull
            ) {
                10.0
            } else {
                0.0
            },
            dormant_skeleton_policy: false,
            tertiary_damage: match intent.enemy_token {
                EnemyToken::Spider => 2.0,
                EnemyToken::DemonSkull => 6.0,
                _ => 0.0,
            },
            weapon: SkeletonWeapon::Claw,
            accuracy_mode: 0,
            arrow_type: ArrowType::Normal,
            extra_arrows: 0,
            multi_arrow_mode: 0,
            range_mode: 0,
            strafing: intent.archer_strafing,
            mage_element: MageElement::Fire,
            other_shield: false,
            other_shield_health: 0.0,
            self_shield: false,
            self_shield_health: 0.0,
            shield_interval: 0.0,
            split_count: 0,
            body_type: if intent.zombie_body_type == 1 { 3 } else { 0 },
            poison_duration: 0.0,
            poison_pool_damage: 0.0,
            poison_punch_damage: 0.0,
            portal_frequency: 0,
            rotten: false,
            maggot_damage: 2.0,
            maggot_health: 2.0,
            maggot_poison_damage: 0.0,
            maximum_maggots: 20,
        }
    }

    fn apply_authored_family(&mut self, family: &AuthoredEnemyFamily) {
        match family {
            AuthoredEnemyFamily::Default => {}
            AuthoredEnemyFamily::DemonSkull { capabilities } => {
                self.demon_skull_capabilities = *capabilities
            }
            AuthoredEnemyFamily::Faculty(appearance) => {
                self.faculty = appearance.clone();
                self.faculty.body_color = desaturate_color(appearance.body_color, 0.7_f32 as f64);
                self.faculty.head_color = desaturate_color(appearance.head_color, 0.7_f32 as f64);
            }
            AuthoredEnemyFamily::Heartmonger { crow_count, blind_mode, summon_mode } => {
                self.crow_count = *crow_count;
                self.blind_mode = *blind_mode;
                self.summon_mode = *summon_mode;
            }
            AuthoredEnemyFamily::Skeleton { armor, headgear, weapon } => {
                self.armor = *armor;
                self.headgear = *headgear;
                self.weapon = *weapon;
            }
            AuthoredEnemyFamily::Archer {
                arrow_type,
                extra_arrows,
                headgear,
                multi_arrow_mode,
                range_mode,
                strafing,
            } => {
                self.arrow_type = *arrow_type;
                self.extra_arrows = *extra_arrows;
                self.headgear = *headgear;
                self.multi_arrow_mode = *multi_arrow_mode;
                self.range_mode = *range_mode;
                self.strafing = *strafing;
            }
            AuthoredEnemyFamily::Portal { frequency } => self.portal_frequency = *frequency,
            AuthoredEnemyFamily::Zombie {
                body_type,
                flyblown,
                poison_duration,
                poison_pool_damage,
                poison_punch_damage,
            } => {
                self.body_type = if *body_type == 1 { 3 } else { 0 };
                self.rotten = *flyblown;
                self.poison_duration = *poison_duration;
                self.poison_pool_damage = *poison_pool_damage;
                self.poison_punch_damage = *poison_punch_damage;
            }
        }
    }

    fn apply_flag(&mut self, flag: EnemyFlag, random: EnemyConfigRandom, wave_ordinal: u64) {
        match flag {
            EnemyFlag::Cocoon => self.secondary_damage *= 5.0,
            EnemyFlag::NoSpit => self.spit_webs = false,
            EnemyFlag::HpUp => self.maximum_health *= 1.5,
            EnemyFlag::HpDown => self.maximum_health *= 0.5,
            EnemyFlag::Strong => self.multiply_damage(1.5),
            EnemyFlag::Weak => self.multiply_damage(0.5),
            EnemyFlag::Fast => self.chase_speed *= 1.25,
            EnemyFlag::Slow => {
                self.chase_speed *= 0.5;
                self.attack_speed *= 0.5;
            }
            EnemyFlag::XpBonus => self.experience_bonus = 2.0,
            EnemyFlag::Burning => {
                self.burning = true;
                self.chase_speed *= 1.5;
                self.attack_speed *= 1.5;
            }
            EnemyFlag::Helm => {
                self.headgear = 1;
                self.maximum_health += 6.0;
            }
            EnemyFlag::Horned => {
                self.headgear = 2;
                self.maximum_health += 10.0;
            }
            EnemyFlag::Hooded => {
                self.headgear = 3;
                self.maximum_health += 3.0;
            }
            EnemyFlag::Leading => self.accuracy_mode = 1,
            EnemyFlag::Scattershot => self.accuracy_mode = 2,
            EnemyFlag::RandomShot => self.accuracy_mode = 3,
            EnemyFlag::RangeUp => self.range_mode = 2,
            EnemyFlag::RangeDown => self.range_mode = 1,
            EnemyFlag::RangeEasy => self.range_mode = 3,
            EnemyFlag::Shield => {
                self.self_shield = true;
                self.self_shield_health = 50.0;
                self.shield_interval = 10.0;
            }
            EnemyFlag::ShieldOthers => {
                self.other_shield = true;
                self.other_shield_health = 50.0;
                self.shield_interval = 10.0;
            }
            EnemyFlag::ShieldStrong => {
                self.self_shield_health *= 9.0;
                self.other_shield_health *= 9.0;
            }
            EnemyFlag::ShieldFast => self.shield_interval *= 0.5,
            EnemyFlag::Split => self.split_count = i64::from(random.split_unit) + 1,
            EnemyFlag::SplitMany => {
                let lower = ((wave_ordinal as f64 - 25.0) / 5.0).trunc() as i64 + 1;
                let gate = lower + inclusive_three_way_index(random.split_many_gate_unit);
                self.split_count = if gate < 2 {
                    2
                } else {
                    lower + inclusive_three_way_index(random.split_many_unit)
                };
            }
            EnemyFlag::ManyMaggots => self.maximum_maggots = 50,
            EnemyFlag::StrongMaggots => {
                self.maggot_health = 5.0;
                self.maggot_damage = 5.0;
            }
            EnemyFlag::PoisonArrow => {
                self.arrow_type = ArrowType::Poison;
                self.secondary_damage = self.primary_damage * 3.0;
            }
            EnemyFlag::FireArrow => {
                self.arrow_type = ArrowType::Fire;
                self.secondary_damage = self.primary_damage;
            }
            EnemyFlag::Armor => self.apply_armor(),
            EnemyFlag::Sword => self.apply_weapon(SkeletonWeapon::Sword, 15.0, 10.0),
            EnemyFlag::Mace => self.apply_weapon(SkeletonWeapon::Mace, 25.0, 10.0),
            EnemyFlag::Flail => self.apply_weapon(SkeletonWeapon::Flail, 35.0, 10.0),
            EnemyFlag::Axe => self.apply_weapon(SkeletonWeapon::Axe, 18.0, 10.0),
            EnemyFlag::Pike => self.apply_weapon(SkeletonWeapon::Pike, 25.0, 35.0),
            EnemyFlag::CastFire => {
                self.mage_element = MageElement::Fire;
                self.primary_damage *= 8.0;
            }
            EnemyFlag::CastLightning => {
                self.mage_element = MageElement::Lightning;
                self.primary_damage *= 4.0;
            }
            EnemyFlag::CastFrost => {
                self.mage_element = MageElement::Frost;
                self.primary_damage *= 2.0;
            }
            EnemyFlag::CastPoison => {
                self.mage_element = MageElement::Poison;
                self.primary_damage *= 8.0;
            }
            EnemyFlag::Rotten => {
                self.rotten = true;
                self.poison_punch_damage = self.primary_damage / 6.0;
                self.poison_pool_damage = self.primary_damage / 5.0;
                self.poison_duration = 10.0;
            }
            EnemyFlag::DeathImps => self.split_count = 5,
            EnemyFlag::DeathImpsMany => self.split_count = 15,
            EnemyFlag::ArmorMaybe => {
                if random.random_armor {
                    self.apply_armor();
                }
            }
            EnemyFlag::NoSkeletons => self.dormant_skeleton_policy = true,
            EnemyFlag::MoreSkeletons => {
                self.dormant_skeleton_policy = true;
                self.self_shield = false;
                self.rotten = false;
            }
            EnemyFlag::Ignite | EnemyFlag::Immortalize => {}
        }
    }

    fn multiply_damage(&mut self, multiplier: f64) {
        self.primary_damage *= multiplier;
        self.secondary_damage *= multiplier;
        self.tertiary_damage *= multiplier;
        self.extra_damage *= multiplier;
    }

    fn apply_armor(&mut self) {
        self.armor = true;
        self.maximum_health = (self.maximum_health + 10.0) * 2.0;
    }

    fn apply_weapon(&mut self, weapon: SkeletonWeapon, damage: f64, health: f64) {
        self.weapon = weapon;
        self.maximum_health = (self.maximum_health + health) * 2.0;
        self.primary_damage += damage;
    }

    fn family(&self, token: EnemyToken) -> EnemyFamilyConfig {
        match token {
            EnemyToken::Skeleton => EnemyFamilyConfig::Skeleton {
                armor: self.armor,
                headgear: self.headgear,
                weapon: self.weapon,
            },
            EnemyToken::SkeletonArcher => EnemyFamilyConfig::Archer {
                accuracy_mode: self.accuracy_mode,
                arrow_type: self.arrow_type,
                extra_arrows: self.extra_arrows,
                headgear: self.headgear,
                multi_arrow_mode: self.multi_arrow_mode,
                range_mode: self.range_mode,
                strafing: self.strafing,
            },
            EnemyToken::SkeletonMage => EnemyFamilyConfig::Mage {
                cloak: self.cloak,
                element: self.mage_element,
                headgear: self.headgear,
                other_shield: self.other_shield,
                other_shield_health: self.other_shield_health,
                range_mode: self.range_mode,
                self_shield: self.self_shield,
                self_shield_health: self.self_shield_health,
                shield_interval: self.shield_interval,
            },
            EnemyToken::Imp => EnemyFamilyConfig::Imp { split_depth: self.split_count },
            EnemyToken::Zombie => EnemyFamilyConfig::Zombie {
                body_type: self.body_type,
                poison_duration: self.poison_duration,
                poison_pool_damage: self.poison_pool_damage,
                poison_punch_damage: self.poison_punch_damage,
                rotten: self.rotten,
            },
            EnemyToken::Wraith => EnemyFamilyConfig::Wraith,
            EnemyToken::Demon => EnemyFamilyConfig::Demon { split_count: self.split_count },
            EnemyToken::Coffin => EnemyFamilyConfig::Coffin {
                maggot_damage: self.maggot_damage,
                maggot_health: self.maggot_health,
                maggot_poison_damage: self.maggot_poison_damage,
                maximum_maggots: self.maximum_maggots,
            },
            EnemyToken::Portal => EnemyFamilyConfig::Portal { frequency: self.portal_frequency },
            EnemyToken::Heartmonger => EnemyFamilyConfig::Heartmonger {
                crow_count: self.crow_count,
                blind_mode: self.blind_mode,
                summon_mode: self.summon_mode,
            },
            EnemyToken::DireFaculty => EnemyFamilyConfig::Faculty(self.faculty.clone()),
            EnemyToken::DemonSkull => {
                EnemyFamilyConfig::DemonSkull { capabilities: self.demon_skull_capabilities }
            }
            EnemyToken::Spider => EnemyFamilyConfig::Spider {
                cocoon_health: self.secondary_damage,
                spit_webs: self.spit_webs,
                suck_damage_per_second: self.tertiary_damage,
            },
            EnemyToken::Cocoon => EnemyFamilyConfig::Cocoon,
        }
    }
}

fn constructor_base_speed(token: EnemyToken, unit: f64) -> f64 {
    let skeleton = (1.25 + unit) * 1.25_f64.powi(2);
    match token {
        EnemyToken::Cocoon | EnemyToken::Wraith | EnemyToken::Portal => 1.0,
        EnemyToken::Spider => 3.0 + unit * 2.0,
        EnemyToken::DemonSkull => 4.0,
        EnemyToken::DireFaculty => 2.75,
        EnemyToken::Skeleton => skeleton,
        EnemyToken::SkeletonArcher => skeleton * 0.75,
        EnemyToken::SkeletonMage => skeleton * 0.75 * 0.65,
        EnemyToken::Heartmonger => fround(fround(skeleton * (0.65_f32 as f64)) * 0.75),
        EnemyToken::Imp => 4.5,
        EnemyToken::Zombie => 0.85,
        EnemyToken::Demon | EnemyToken::Coffin => 0.75,
    }
}

fn constructor_collision_radius(token: EnemyToken, unit: f64) -> f64 {
    match token {
        EnemyToken::Cocoon | EnemyToken::DemonSkull => 40.0,
        EnemyToken::Spider | EnemyToken::Wraith => 15.0,
        EnemyToken::DireFaculty | EnemyToken::SkeletonMage | EnemyToken::Heartmonger => 25.0,
        EnemyToken::Skeleton => 20.0 - unit * 8.0,
        EnemyToken::SkeletonArcher => 20.0,
        EnemyToken::Imp => 10.0 - unit * 2.5,
        EnemyToken::Portal => 5.0,
        EnemyToken::Zombie => 25.0 - unit * 8.0,
        EnemyToken::Demon => 35.0,
        EnemyToken::Coffin => 45.0,
    }
}

fn inclusive_three_way_index(unit: f64) -> i64 {
    (unit * 3.0).floor().min(2.0) as i64
}

/// `nativeDesaturateColor`, also used by the save boundary's Faculty palette migration.
#[must_use]
pub fn desaturate_color(color: [f64; 4], amount: f64) -> [f64; 4] {
    let luminance = fround(
        color[0] * (0.3086_f32 as f64)
            + color[1] * (0.6094_f32 as f64)
            + color[2] * (0.082_f32 as f64),
    );
    let base = luminance * amount;
    let mix = |channel| fround(base + channel * (1.0 - amount)).clamp(0.0, 1.0);
    [mix(color[0]), mix(color[1]), mix(color[2]), color[3].clamp(0.0, 1.0)]
}
