//! Cast payloads and offensive arithmetic, against unmodified recordings from the Website
//! client.

#[path = "support/stat_book.rs"]
mod stat_book;

use serde_json::Value;
use solomon_sim::equipment::EquipmentModifiers;
use solomon_sim::profile::offensive::{
    DamageResolutionLanes, ManaCostResolutionLanes, OffensiveSpellFactors, resolve_skill_damage,
    resolve_skill_mana_cost, resolve_skill_mana_cost_value,
};
use solomon_sim::profile::primary::{
    PrimarySkillProfile, PrimarySpellDamageUnit, PrimarySpellManaUnit, SpentEmber,
    primary_skill_profile_with_stat_book, primary_spell_descriptor,
    primary_spell_summary_with_stat_book,
};
use solomon_sim::profile::{SkillBookState, WeldBuildId, WizardDiscipline, WizardElement};

fn number(value: &Value) -> f64 {
    value.as_f64().expect("a recorded finite number")
}

fn skillbook(value: &Value) -> SkillBookState {
    let mut book = SkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
    for (rank, value) in
        book.effective_ranks.iter_mut().zip(value["effectiveRanks"].as_array().unwrap())
    {
        *rank = value.as_u64().expect("a recorded skill rank") as u32;
    }
    book.primary_skill_id = value["primarySkillId"].as_u64().unwrap() as usize;
    book.weld_build_id =
        value["weldBuildId"].as_u64().map(|id| WeldBuildId::try_from(id as u32).unwrap());
    book.weld_component_ranks = value["weldComponentRanks"].as_array().map(|values| {
        let mut ranks = [0; 6];
        for (rank, value) in ranks.iter_mut().zip(values) {
            *rank = value.as_u64().unwrap() as u32;
        }
        ranks
    });
    book
}

fn equipment(value: &Value) -> EquipmentModifiers {
    let mut modifiers = EquipmentModifiers {
        global_damage_flat: number(&value["globalDamageFlat"]),
        global_damage_multiplier: number(&value["globalDamageMultiplier"]),
        global_mana_cost_flat: number(&value["globalManaCostFlat"]),
        global_mana_cost_multiplier: number(&value["globalManaCostMultiplier"]),
        weld_effect: number(&value["weldEffect"]),
        ..EquipmentModifiers::default()
    };
    for index in 0..8 {
        modifiers.class_damage_flat[index] = number(&value["classDamageFlat"][index]);
        modifiers.class_damage_multiplier[index] = number(&value["classDamageMultiplier"][index]);
        modifiers.class_mana_cost_flat[index] = number(&value["classManaCostFlat"][index]);
        modifiers.class_mana_cost_multiplier[index] =
            number(&value["classManaCostMultiplier"][index]);
    }
    for index in 0..83 {
        modifiers.skill_damage_flat[index] = number(&value["skillDamageFlat"][index]);
        modifiers.skill_damage_multiplier[index] = number(&value["skillDamageMultiplier"][index]);
    }
    modifiers
}

fn factors(value: &Value) -> OffensiveSpellFactors {
    OffensiveSpellFactors {
        damage: number(&value["damage"]),
        mana_cost: number(&value["manaCost"]),
        global_flat_damage: value["globalFlatDamage"].as_f64().unwrap_or(0.0),
        global_mana_reduction: value["globalManaReduction"].as_f64().unwrap_or(0.0),
        equipment: value.get("equipment").map(equipment),
    }
}

#[test]
fn elemental_and_welded_cast_costs_and_damage_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/primary-profile-goldens.json"))
            .expect("recorded primary profiles");
    for case in fixture["profiles"].as_array().unwrap() {
        let book = skillbook(&case["skillBook"]);
        let factors = factors(&case["factors"]);
        let stat_book = stat_book::read(case.get("statBook"));
        let profile = primary_skill_profile_with_stat_book(&book, &stat_book, &factors);
        let common = profile.common();
        let expected = &case["profile"];
        for (field, actual) in [
            ("damageMinimum", common.damage_minimum),
            ("damageMaximum", common.damage_maximum),
            ("damageRollCount", common.damage_roll_count),
            ("manaCost", common.mana_cost),
            ("rank", f64::from(common.rank)),
            ("skillId", f64::from(common.skill_id)),
        ] {
            assert_eq!(actual, number(&expected[field]), "{} {field}", case["name"]);
        }
        assert_variant(&profile, expected, case["name"].as_str().unwrap());
        let summary = primary_spell_summary_with_stat_book(&book, &stat_book, &factors);
        assert_eq!(
            [summary.damage_minimum, summary.damage_maximum, summary.mana_cost],
            [
                number(&case["summary"]["damageMinimum"]),
                number(&case["summary"]["damageMaximum"]),
                number(&case["summary"]["manaCost"]),
            ],
            "{} inventory summary",
            case["name"],
        );
        let descriptor = primary_spell_descriptor(book.primary_skill_id, book.weld_build_id);
        let expected = &case["descriptor"];
        assert_eq!(descriptor.name, expected["name"].as_str().unwrap());
        assert_eq!(descriptor.damage_range, expected["damageRange"].as_bool().unwrap());
        let damage_unit = match descriptor.damage_unit {
            PrimarySpellDamageUnit::Bolt => "bolt",
            PrimarySpellDamageUnit::Boulder => "boulder",
            PrimarySpellDamageUnit::Impact => "impact",
            PrimarySpellDamageUnit::Rock => "rock",
            PrimarySpellDamageUnit::Second => "second",
        };
        let mana_unit = match descriptor.mana_unit {
            PrimarySpellManaUnit::Cast => "cast",
            PrimarySpellManaUnit::Second => "sec",
        };
        assert_eq!(damage_unit, expected["damageUnit"].as_str().unwrap());
        assert_eq!(mana_unit, expected["manaUnit"].as_str().unwrap());
    }
}

fn assert_variant(profile: &PrimarySkillProfile, expected: &Value, label: &str) {
    let fields = match profile {
        PrimarySkillProfile::Ether(profile) => {
            assert_eq!(expected["kind"], "ether");
            assert_eq!(profile.reacquires_target, expected["reacquiresTarget"].as_bool().unwrap());
            vec![
                ("blastChargeCapacity", f64::from(profile.blast_charge_capacity)),
                ("damageRetention", profile.damage_retention),
                ("pierces", f64::from(profile.pierces)),
                ("quantity", f64::from(profile.quantity)),
                ("speedFactor", profile.speed_factor),
            ]
        }
        PrimarySkillProfile::Fire(profile) => {
            assert_eq!(expected["kind"], "fire");
            let ember = &expected["spentEmber"];
            match profile.spent_ember {
                SpentEmber::None => assert_eq!(ember["kind"], "none"),
                SpentEmber::Immolate { damage } => {
                    assert_eq!(ember["kind"], "immolate");
                    assert_eq!(damage, number(&ember["damage"]), "{label} Immolate damage");
                }
                SpentEmber::Imp { damage, lifetime_ticks } => {
                    assert_eq!(ember["kind"], "imp");
                    assert_eq!(damage, number(&ember["damage"]), "{label} Imp damage");
                    assert_eq!(lifetime_ticks, ember["lifetimeTicks"].as_u64().unwrap());
                }
            }
            vec![
                ("burnDamage", profile.burn_damage),
                ("emberDamage", profile.ember_damage),
                ("emberFragments", f64::from(profile.ember_fragments)),
                ("explodeDamage", profile.explode_damage),
                ("explodeRadius", profile.explode_radius),
            ]
        }
        PrimarySkillProfile::Air(profile) => {
            assert_eq!(expected["kind"], "air");
            vec![
                ("arcCount", f64::from(profile.arc_count)),
                ("disintegrateChance", profile.disintegrate_chance),
                ("hurricaneDamageMaximum", profile.hurricane_damage_maximum),
                ("hurricaneDamageMinimum", profile.hurricane_damage_minimum),
                ("stunMovementFactor", profile.stun_movement_factor),
            ]
        }
        PrimarySkillProfile::Water(profile) => {
            assert_eq!(expected["kind"], "water");
            vec![
                ("armorMaximum", profile.armor_maximum),
                ("armorPerSecond", profile.armor_per_second),
                ("auraRadiusScale", profile.aura_radius_scale),
                ("auraMovementFactor", profile.aura_movement_factor),
                ("auraSlowFactor", profile.aura_slow_factor),
                ("coldDurationTicks", profile.cold_duration_ticks as f64),
                ("coldMovementFactor", profile.cold_movement_factor),
                ("hailChance", profile.hail_chance),
                ("hailDamageMaximum", profile.hail_damage_maximum),
                ("hailDamageMinimum", profile.hail_damage_minimum),
                ("hailThreshold", f64::from(profile.hail_threshold)),
                ("halfAngleDegrees", profile.half_angle_degrees),
                ("minimumColdDurationTicks", profile.minimum_cold_duration_ticks as f64),
                ("pushbackFactor", profile.pushback_factor),
                ("reach", profile.reach),
                ("slowdownScale", profile.slowdown_scale),
                ("widenHalfDegrees", profile.widen_half_degrees),
            ]
        }
        PrimarySkillProfile::Earth(profile) => {
            assert_eq!(expected["kind"], "earth");
            vec![
                ("growthFactor", profile.growth_factor),
                ("maximumCharge", profile.maximum_charge),
                ("rockSurgeChance", profile.rock_surge_chance),
                ("rockSurgeManaCost", profile.rock_surge_mana_cost),
                ("toughness", profile.toughness),
            ]
        }
        PrimarySkillProfile::Weld(profile) => {
            assert_eq!(expected["kind"], "weld");
            assert_eq!(profile.build_id.id(), expected["buildId"].as_u64().unwrap() as u32);
            assert_eq!(profile.cast_kind.as_str(), expected["castKind"].as_str().unwrap());
            let values: Vec<f64> =
                expected["vector"]["values"].as_array().unwrap().iter().map(number).collect();
            assert_eq!(profile.vector.values, values, "{label} native weld vector");
            vec![("damageFactor", profile.damage_factor)]
        }
    };
    for (field, actual) in fields {
        assert_eq!(actual, number(&expected[field]), "{label} {field}");
    }
}

#[test]
fn equipment_and_offensive_bonuses_keep_the_native_lane_order() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/primary-profile-goldens.json"))
            .expect("recorded offensive resolution");
    for case in fixture["resolution"].as_array().unwrap() {
        let skill_id = case["skillId"].as_u64().unwrap() as usize;
        let factors = factors(&case["factors"]);
        let damage = &case["damageLanes"];
        let damage = DamageResolutionLanes {
            actor_base_damage: number(&damage["actorBaseDamage"]),
            base_damage: number(&damage["baseDamage"]),
            global_flat_damage: number(&damage["globalFlatDamage"]),
            skill_flat_damage: number(&damage["skillFlatDamage"]),
            class_flat_damage: number(&damage["classFlatDamage"]),
            global_multiplier: number(&damage["globalMultiplier"]),
            skill_multiplier: number(&damage["skillMultiplier"]),
            class_multiplier: number(&damage["classMultiplier"]),
            element_or_class_multiplier: number(&damage["elementOrClassMultiplier"]),
        };
        let mana = &case["manaLanes"];
        let mana = ManaCostResolutionLanes {
            base_mana_cost: number(&mana["baseManaCost"]),
            global_mana_reduction: number(&mana["globalManaReduction"]),
            global_flat_mana_cost: number(&mana["globalFlatManaCost"]),
            class_flat_mana_cost: number(&mana["classFlatManaCost"]),
            skill_flat_mana_cost: number(&mana["skillFlatManaCost"]),
            global_multiplier: number(&mana["globalMultiplier"]),
            class_multiplier: number(&mana["classMultiplier"]),
            skill_multiplier: number(&mana["skillMultiplier"]),
            element_multiplier: number(&mana["elementMultiplier"]),
        };
        assert_eq!(
            resolve_skill_damage(skill_id, &factors, damage),
            number(&case["damage"]),
            "skill {skill_id} damage order",
        );
        assert_eq!(
            resolve_skill_mana_cost(skill_id, &factors, mana),
            number(&case["mana"]),
            "skill {skill_id} mana order",
        );
        assert_eq!(
            resolve_skill_mana_cost_value(skill_id, 0.0, &factors),
            number(&case["freeMana"]),
            "a zero-cost skill stays free",
        );
    }
}
