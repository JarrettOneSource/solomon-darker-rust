use serde_json::{Value, json};
use solomon_sim::enemies::SpawnPositionPolicy;
use solomon_sim::enemies::config::{
    ArrowType, AuthoredEnemyFamily, AuthoredEnemyRecipe, EnemyClassification, SkeletonWeapon,
};

pub fn policy_id(policy: SpawnPositionPolicy) -> &'static str {
    match policy {
        SpawnPositionPolicy::Dark => "dark",
        SpawnPositionPolicy::Direct => "direct",
        SpawnPositionPolicy::Edge => "edge",
        SpawnPositionPolicy::Light => "light",
        SpawnPositionPolicy::Offscreen => "offscreen",
    }
}

pub fn recipe_snapshot(recipe: &AuthoredEnemyRecipe) -> Value {
    let classification = match recipe.classification {
        EnemyClassification::Normal => "normal",
        EnemyClassification::Boss => "boss",
        EnemyClassification::Miniboss => "miniboss",
        EnemyClassification::MultipleBoss => "multiple-boss",
    };
    let family = match &recipe.family {
        AuthoredEnemyFamily::Default => json!({ "kind": "default" }),
        AuthoredEnemyFamily::DemonSkull { capabilities } => {
            json!({ "kind": "demon-skull", "capabilities": capabilities })
        }
        AuthoredEnemyFamily::Faculty(appearance) => json!({ "kind": "faculty",
            "bodyColor": appearance.body_color, "headColor": appearance.head_color,
            "female": appearance.female, "headgear": appearance.headgear,
            "primary": appearance.primary, "secondary": appearance.secondary,
        }),
        AuthoredEnemyFamily::Heartmonger { crow_count, blind_mode, summon_mode } => {
            json!({ "kind": "heartmonger", "crowCount": crow_count, "blindMode": blind_mode, "summonMode": summon_mode })
        }
        AuthoredEnemyFamily::Skeleton { armor, headgear, weapon } => {
            let weapon = match weapon {
                SkeletonWeapon::Claw => "claw",
                SkeletonWeapon::Sword => "sword",
                SkeletonWeapon::Mace => "mace",
                SkeletonWeapon::Flail => "flail",
                SkeletonWeapon::Axe => "axe",
                SkeletonWeapon::Pike => "pike",
            };
            json!({ "kind": "skeleton", "armor": armor, "headgear": headgear, "weapon": weapon })
        }
        AuthoredEnemyFamily::Archer {
            arrow_type,
            extra_arrows,
            headgear,
            multi_arrow_mode,
            range_mode,
            strafing,
        } => {
            let arrow = match arrow_type {
                ArrowType::Normal => "normal",
                ArrowType::Fire => "fire",
                ArrowType::Poison => "poison",
            };
            json!({ "kind": "archer", "arrowType": arrow, "extraArrows": extra_arrows, "headgear": headgear, "multiArrowMode": multi_arrow_mode, "rangeMode": range_mode, "strafing": strafing })
        }
        AuthoredEnemyFamily::Portal { frequency } => {
            json!({ "kind": "portal", "frequency": frequency })
        }
        AuthoredEnemyFamily::Zombie {
            body_type,
            flyblown,
            poison_duration,
            poison_pool_damage,
            poison_punch_damage,
        } => {
            json!({ "kind": "zombie", "bodyType": body_type, "flyblown": flyblown, "poisonDuration": poison_duration, "poisonPoolDamage": poison_pool_damage, "poisonPunchDamage": poison_punch_damage })
        }
    };
    let loot = &recipe.loot_policies;
    json!({
        "archerAccuracyMode": recipe.archer_accuracy_mode, "attackSpeed": recipe.attack_speed,
        "chaseSpeed": recipe.chase_speed, "classification": classification,
        "experienceBonus": recipe.experience_bonus, "extraDamage": recipe.extra_damage,
        "family": family,
        "lootPolicies": { "gold": loot.gold, "item": loot.item, "orb": loot.orb,
            "potion": loot.potion, "powerup": loot.powerup, "specificItem": loot.specific_item },
        "maximumHealth": recipe.maximum_health, "movementScale": recipe.movement_scale,
        "name": recipe.name, "onDeathProgram": if recipe.miniboss_die_program { Some("miniboss-die") } else { None },
        "primaryDamage": recipe.primary_damage, "secondaryDamage": recipe.secondary_damage,
        "tertiaryDamage": recipe.tertiary_damage, "uid": recipe.uid,
    })
}
