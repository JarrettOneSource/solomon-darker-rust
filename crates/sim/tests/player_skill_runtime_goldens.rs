//! Player resource, concentration, equipment and recovery formulas against Website.

#[path = "support/stat_book.rs"]
mod stat_book;

use serde_json::{Value, json};
use solomon_sim::economy::{
    DOWSING_EQUIPMENT_RECIPES, EquipmentType, HubEconomyState, NativeEquipmentEffect,
    create_equipment_inventory_item,
};
use solomon_sim::equipment::{NativeEquipmentModifiers, NativeEquipmentScalarTransform};
use solomon_sim::profile::primary::native_primary_spell_summary_with_stat_book;
use solomon_sim::profile::*;
use solomon_sim::{NativeRng, combat::PlayerCombatState};

fn transform(value: NativeEquipmentScalarTransform) -> Value {
    json!({"offset":value.offset,"scale":value.scale})
}

fn modifiers(value: &NativeEquipmentModifiers) -> Value {
    json!({
        "castSpeedFlat":value.cast_speed_flat,"castSpeedMultiplier":value.cast_speed_multiplier,
        "classCastSpeedFlat":value.class_cast_speed_flat,"classCastSpeedMultiplier":value.class_cast_speed_multiplier,
        "classDamageFlat":value.class_damage_flat,"classDamageMultiplier":value.class_damage_multiplier,
        "classManaCostFlat":value.class_mana_cost_flat,"classManaCostMultiplier":value.class_mana_cost_multiplier,
        "classRecharge":value.class_recharge.map(transform),"damageResistance":value.damage_resistance,
        "featureBits":value.feature_bits,"globalDamageFlat":value.global_damage_flat,
        "globalDamageMultiplier":value.global_damage_multiplier,"globalManaCostFlat":value.global_mana_cost_flat,
        "globalManaCostMultiplier":value.global_mana_cost_multiplier,"goldMultiplier":value.gold_multiplier,
        "healthRecovery":transform(value.health_recovery),"magicResistance":value.magic_resistance,
        "manaRecovery":transform(value.mana_recovery),"maximumHealth":transform(value.maximum_health),
        "maximumMana":transform(value.maximum_mana),"meleeDamageFlat":value.melee_damage_flat,
        "meleeDamageMultiplier":value.melee_damage_multiplier,"orbPullMultiplier":value.orb_pull_multiplier,
        "poisonResistance":value.poison_resistance,"recharge":transform(value.recharge),
        "skillDamageFlat":&value.skill_damage_flat[..],"skillDamageMultiplier":&value.skill_damage_multiplier[..],
        "walkSpeed":transform(value.walk_speed),"weldEffect":value.weld_effect,
    })
}

fn assert_snapshot(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}")
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_snapshot(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} fields");
            for (key, expected) in expected {
                assert_snapshot(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn book_snapshot(value: &NativeSkillBookState) -> Value {
    json!({"advancedUnlocks":value.advanced_unlocks,"disciplineRoot":value.discipline_root,
        "effectiveRanks":&value.effective_ranks[..],"elementRoot":value.element_root,
        "learnedSkillOrder":value.learned_skill_order,"permanentRanks":&value.permanent_ranks[..],
        "primarySkillId":value.primary_skill_id,"weldBuildId":value.weld_build_id,"weldComponentRanks":value.weld_component_ranks})
}

fn rng_snapshot(rng: &NativeRng) -> Value {
    json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":&rng.words[..]})
}

fn runtime_snapshot(value: &PlayerSkillRuntimeState) -> Value {
    json!({
    "concentrationSkillIdA":value.concentration_skill_id_a,
    "concentrationSkillIdB":value.concentration_skill_id_b,
    "equipmentModifiers":modifiers(&value.equipment_modifiers),
    "harden":json!({"armor":value.harden.armor,"coating":value.harden.coating}),
    "hardenArmorMaximum":value.harden_armor_maximum,
    "hardenArmorPerTick":value.harden_armor_per_tick,
    "hurricaneCharge":value.hurricane_charge,
    "hurricaneEnabled":value.hurricane_enabled,
    "hurricaneRefreshed":value.hurricane_refreshed,
    "meditationActivityRampTicks":value.meditation_activity_ramp_ticks,
    "meditationIdleElapsedTicks":value.meditation_idle_elapsed_ticks,
    "mindstarActive":value.mindstar_active,
    "nextConcentrationReplacementSlot":match value.next_concentration_replacement_slot {PlayerConcentrationSlot::A=>"a",PlayerConcentrationSlot::B=>"b"},
    "staffMeleeAlternate":value.staff_melee_alternate,
    })
}

fn derived_snapshot(value: &PlayerSkillDerivedStats) -> Value {
    json!({
    "castProgressFactor":value.cast_progress_factor,
    "damageResistance":value.damage_resistance,
    "deflectChancePercent":value.deflect_chance_percent,
    "experienceBonus":value.experience_bonus,
    "flashChancePercent":value.flash_chance_percent,
    "flashDurationTicks":value.flash_duration_ticks,
    "focusInstantRechargeChancePercent":value.focus_instant_recharge_chance_percent,
    "flailingChancePercent":value.flailing_chance_percent,
    "goldAmountMultiplier":value.gold_amount_multiplier,
    "healthRecoveryPerTick":value.health_recovery_per_tick,
    "incomingDamageFactor":value.incoming_damage_factor,
    "magicResistance":value.magic_resistance,
    "manaRecoveryPerTick":value.mana_recovery_per_tick,
    "manaRecoveryPerSecond":value.mana_recovery_per_second,
    "maximumHealth":value.maximum_health,
    "maximumMana":value.maximum_mana,
    "meditationConcentrated":value.meditation_concentrated,
    "meditationIdleDelayTicks":value.meditation_idle_delay_ticks,
    "meditationRecoveryMultiplier":value.meditation_recovery_multiplier,
    "meleeDamageMaximum":value.melee_damage_maximum,
    "meleeDamageMinimum":value.melee_damage_minimum,
    "movementFactor":value.movement_factor,
    "offensiveDamageFactor":value.offensive_damage_factor,
    "offensiveDamageFlat":value.offensive_damage_flat,
    "offensiveManaCostFactor":value.offensive_mana_cost_factor,
    "offensiveManaCostReduction":value.offensive_mana_cost_reduction,
    "orbPullMultiplier":value.orb_pull_multiplier,
    "pickupRangeScalar":value.pickup_range_scalar,
    "poisonResistance":value.poison_resistance,
    "poisonDamageFactor":value.poison_damage_factor,
    "pushStrengthFactor":value.push_strength_factor,
    "secondaryRechargeFactor":value.secondary_recharge_factor,
    "staffActionTimingFactor":value.staff_action_timing_factor,
    "staffDamagePrimary":value.staff_damage_primary,
    "staffDamageSecondary":value.staff_damage_secondary,
    "staffEquipped":value.staff_equipped,
    })
}

fn setup(
    value: &Value,
) -> (NativeSkillBookState, HubEconomyState, PlayerSkillRuntimeState, PlayerProgressionState) {
    let element = WizardElement::ALL
        .into_iter()
        .find(|element| Some(element.as_str()) == value["element"].as_str())
        .unwrap();
    let discipline = WizardDiscipline::ALL
        .into_iter()
        .find(|discipline| Some(discipline.as_str()) == value["discipline"].as_str())
        .unwrap();
    let mut book = NativeSkillBookState::new(element, discipline);
    if let Some(id) = value["weldBuild"].as_u64() {
        book.grant_weld_build(id as u32);
    }
    for grant in value["learned"].as_array().unwrap() {
        book.grant_ranks(grant[0].as_u64().unwrap() as usize, grant[1].as_u64().unwrap() as u32);
    }
    let mut economy = HubEconomyState::new(77, element);
    let mut ring = 0;
    for (index, recipe) in value["recipes"].as_array().unwrap().iter().enumerate() {
        let item = create_equipment_inventory_item(
            &DOWSING_EQUIPMENT_RECIPES[recipe.as_u64().unwrap() as usize],
            1000 + index as u64,
        );
        match item.equipment_type.unwrap() {
            EquipmentType::Staff | EquipmentType::Wand => economy.equipment.weapon = Some(item),
            EquipmentType::Hat => economy.equipment.hat = Some(item),
            EquipmentType::Robe => economy.equipment.robe = Some(item),
            EquipmentType::Amulet => economy.equipment.amulet = Some(item),
            EquipmentType::Ring => {
                economy.equipment.rings[ring % 3] = Some(item);
                ring += 1;
            }
        }
    }
    if !value["weapon"].as_bool().unwrap() {
        economy.equipment.weapon = None;
    }
    if let Some(effects) = value["effects"].as_array() {
        let mut configured = Vec::new();
        for effect in effects {
            configured.push(NativeEquipmentEffect {
                kind: effect["kind"].as_u64().unwrap() as u32,
                magnitude: effect["magnitude"].as_f64().unwrap(),
                operator: effect["operator"].as_u64().unwrap() as u32,
                target: effect["target"].as_u64().unwrap() as u32,
            });
        }
        economy.equipment.hat.as_mut().unwrap().native_effects = Some(configured);
    }
    economy.owned_perk_selectors =
        value["owned"].as_array().unwrap().iter().map(|id| id.as_u64().unwrap() as u32).collect();
    if value["unforge"].is_object() {
        let bonus = &value["unforge"];
        economy.unforge_bonuses.experience = bonus["experience"].as_f64().unwrap_or(0.0);
        economy.unforge_bonuses.mana_cost_reduction =
            bonus["manaCostReduction"].as_f64().unwrap_or(0.0);
        economy.unforge_bonuses.maximum_health = bonus["maximumHealth"].as_f64().unwrap_or(0.0);
        economy.unforge_bonuses.maximum_mana = bonus["maximumMana"].as_f64().unwrap_or(0.0);
        economy.unforge_bonuses.offensive_damage = bonus["offensiveDamage"].as_f64().unwrap_or(0.0);
    }
    let mut runtime = PlayerSkillRuntimeState::new(&mut book, &economy);
    runtime.concentration_skill_id_a = value["concentrationA"].as_u64().map(|id| id as usize);
    runtime.concentration_skill_id_b = value["concentrationB"].as_u64().map(|id| id as usize);
    runtime.mindstar_active = value["mindstar"].as_bool().unwrap();
    runtime.refresh(&mut book, &economy);
    let mut progression = PlayerProgressionState::new(417);
    progression.damage_x4_ticks_remaining = value["quad"].as_u64().unwrap();
    progression.mind_chug_ticks_remaining = value["mindChug"].as_u64().unwrap();
    progression.hagatha_runtime.purchase(&economy.owned_perk_selectors);
    (book, economy, runtime, progression)
}
fn verify_derived(
    runtime: &PlayerSkillRuntimeState,
    book: &NativeSkillBookState,
    progression: &PlayerProgressionState,
    economy: &HubEconomyState,
    expected: &Value,
    context: &str,
) {
    let stat_book = stat_book::read(expected.get("statBook"));
    let derived =
        player_skill_derived_stats_with_stat_book(runtime, book, &stat_book, progression, economy);
    assert_snapshot(&derived_snapshot(&derived), &expected["derived"], context);
    let mut combat = PlayerCombatState::new();
    combat.current_health = 12.3;
    combat.current_mana = 45.6;
    refresh_player_combat_from_skill_stats(&mut combat, &derived);
    assert_snapshot(
        &json!({"currentHealth":combat.current_health,"currentMana":combat.current_mana,"maximumHealth":combat.maximum_health,"maximumMana":combat.maximum_mana}),
        &expected["vitals"],
        context,
    );
    let primary = native_primary_spell_summary_with_stat_book(
        book,
        &stat_book,
        &derived.offensive_factors(runtime),
    );
    assert_snapshot(
        &json!({"damageMinimum":primary.damage_minimum,"damageMaximum":primary.damage_maximum,"manaCost":primary.mana_cost}),
        &expected["primary"],
        context,
    );
    let poison: Vec<_> = [0.0, 0.01, 1.234, 9.999]
        .into_iter()
        .map(|seconds| player_poison_duration_seconds(&derived, seconds))
        .collect();
    assert_snapshot(&json!(poison), &expected["poison"], context);
}
#[test]
fn player_equipment_concentrations_resources_and_live_recovery_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-skill-runtime-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let (mut book, economy, mut runtime, progression) = setup(&case["setup"]);
        if let Some(value) = case.get("statBook") {
            let stat_book = stat_book::read(Some(value));
            runtime.refresh_with_stat_book(&mut book, &stat_book, &economy);
        }
        let context = case["name"].as_str().unwrap();
        assert_snapshot(&book_snapshot(&book), &case["book"], context);
        assert_snapshot(&runtime_snapshot(&runtime), &case["runtime"], context);
        verify_derived(&runtime, &book, &progression, &economy, case, context);
    }
    for journey in fixture["selectionJourneys"].as_array().unwrap() {
        let (mut book, economy, mut runtime, progression) = setup(&journey["initial"]["setup"]);
        let mut rng = NativeRng::new(77);
        assert_snapshot(
            &runtime_snapshot(&runtime),
            &journey["initial"]["runtime"],
            "selection initial",
        );
        for (index, record) in journey["records"].as_array().unwrap().iter().enumerate() {
            let action = &record["action"];
            let accepted = match action["type"].as_str().unwrap() {
                "autofill" => {
                    runtime.autofill_selections(&mut book, &economy, &mut rng);
                    true
                }
                "concentration" => runtime.set_concentration(
                    &mut book,
                    &economy,
                    action["id"].as_u64().unwrap() as usize,
                ),
                "slot" => runtime.set_concentration_slot(
                    &mut book,
                    &economy,
                    action["id"].as_u64().unwrap() as usize,
                    if action["slot"].as_u64() == Some(0) {
                        PlayerConcentrationSlot::A
                    } else {
                        PlayerConcentrationSlot::B
                    },
                ),
                "mindstar" => {
                    runtime.set_mindstar_active(
                        action["active"].as_bool().unwrap(),
                        &mut book,
                        &economy,
                    );
                    true
                }
                "toggle-staff" => {
                    runtime.toggle_staff_melee_lane();
                    true
                }
                other => panic!("unknown skill selection {other}"),
            };
            let context = format!("selection {index}");
            assert_eq!(Some(accepted), record["accepted"].as_bool(), "{context}");
            assert_snapshot(&book_snapshot(&book), &record["book"], &context);
            assert_snapshot(&runtime_snapshot(&runtime), &record["runtime"], &context);
            assert_snapshot(&rng_snapshot(&rng), &record["rng"], &context);
            verify_derived(&runtime, &book, &progression, &economy, record, &context);
        }
    }
    for journey in fixture["tickJourneys"].as_array().unwrap() {
        let (book, economy, mut runtime, progression) = setup(&journey["initial"]["setup"]);
        let mut tick = 0;
        let mut records = journey["records"].as_array().unwrap().iter().peekable();
        for stage in journey["stages"].as_array().unwrap() {
            let input = &stage["activity"];
            let primary_channel = WizardElement::ALL
                .into_iter()
                .find(|element| Some(element.as_str()) == input["primaryChannel"].as_str());
            let activity = PlayerSkillActivity {
                acting: input["acting"].as_bool().unwrap(),
                moving: input["moving"].as_bool().unwrap(),
                primary_channel,
                primary_active: input["primaryActive"].as_bool().unwrap_or(false),
                primary_underpowered: input["primaryUnderpowered"].as_bool().unwrap_or(false),
            };
            for _ in 0..stage["ticks"].as_u64().unwrap() {
                tick += 1;
                let derived = player_skill_derived_stats(&runtime, &book, &progression, &economy);
                let result = runtime.step(&derived, activity);
                if records.peek().is_none_or(|record| record["tick"].as_u64() != Some(tick)) {
                    continue;
                }
                let record = records.next().unwrap();
                assert_snapshot(
                    &runtime_snapshot(&runtime),
                    &record["runtime"],
                    &format!("runtime tick {tick}"),
                );
                assert_eq!(
                    result.base_mana_recovery_per_tick,
                    record["baseManaRecoveryPerTick"].as_f64().unwrap()
                );
                assert_eq!(
                    result.meditation_mana_recovery_per_tick,
                    record["meditationManaRecoveryPerTick"].as_f64().unwrap()
                );
            }
        }
        assert!(records.next().is_none());
    }
}

fn staff_outcome(name: &str) -> PlayerStaffAttackOutcome {
    match name {
        "normal" => PlayerStaffAttackOutcome::Normal,
        "knockback" => PlayerStaffAttackOutcome::Knockback,
        "disabling-hit" => PlayerStaffAttackOutcome::DisablingHit,
        "critical-hit" => PlayerStaffAttackOutcome::CriticalHit,
        "whirl" => PlayerStaffAttackOutcome::Whirl,
        other => panic!("unknown staff outcome {other}"),
    }
}

fn offer(value: &Value) -> PlayerSkillOffer {
    PlayerSkillOffer {
        automatic_choice_index: value["automaticChoiceIndex"].as_u64().map(|index| index as usize),
        level: value["level"].as_u64().unwrap() as u32,
        sequence: value["sequence"].as_u64().unwrap(),
        options: value["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| PlayerSkillOfferOption {
                insight: option["insight"].as_bool().unwrap_or(false),
                skill_id: option["skillId"].as_u64().unwrap() as usize,
                target_rank: option["targetRank"].as_u64().unwrap() as u32,
                weld_build_id: option["weldBuildId"].as_u64().map(|id| id as u32),
            })
            .collect(),
    }
}
fn offer_snapshot(offer: &PlayerSkillOffer) -> Value {
    let options = offer
        .options
        .iter()
        .map(|option| {
            let mut result =
                json!({ "skillId": option.skill_id, "targetRank": option.target_rank });
            if option.insight {
                result["insight"] = json!(true);
            }
            if let Some(build) = option.weld_build_id {
                result["weldBuildId"] = json!(build);
            }
            result
        })
        .collect::<Vec<_>>();
    let mut result =
        json!({ "level": offer.level, "sequence": offer.sequence, "options": options });
    if let Some(choice) = offer.automatic_choice_index {
        result["automaticChoiceIndex"] = json!(choice);
    }
    result
}

#[test]
fn staff_proc_damage_and_creativity_insight_preserve_native_rng_and_rank_gates() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-skill-runtime-goldens.json"))
            .unwrap();
    for case in fixture["staffAttacks"].as_array().unwrap() {
        let (book, economy, runtime, progression) = setup(&case["setup"]);
        let mut derived = player_skill_derived_stats(&runtime, &book, &progression, &economy);
        derived.flailing_chance_percent = case["chance"].as_f64().unwrap();
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let actual = resolve_player_staff_attack(&derived, &mut rng);
        assert_eq!(actual.outcome, staff_outcome(case["result"]["outcome"].as_str().unwrap()));
        assert_eq!(
            actual.action_timing_factor,
            case["result"]["actionTimingFactor"].as_f64().unwrap()
        );
        assert_snapshot(&rng_snapshot(&rng), &case["result"]["rng"], "staff proc RNG");
        for damage in case["damages"].as_array().unwrap() {
            assert_eq!(
                player_staff_damage(
                    &runtime,
                    &derived,
                    &progression,
                    staff_outcome(damage["outcome"].as_str().unwrap())
                ),
                damage["value"].as_f64().unwrap(),
                "staff damage",
            );
        }
    }
    for case in fixture["insightCases"].as_array().unwrap() {
        let (book, _economy, runtime, _progression) = setup(&case["setup"]);
        let mut offer = offer(&case["offer"]);
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let stat_book = stat_book::read(case.get("statBook"));
        runtime.mark_creativity_insight_with_stat_book(&mut offer, &book, &stat_book, &mut rng);
        assert_snapshot(
            &offer_snapshot(&offer),
            &case["result"]["offer"],
            case["name"].as_str().unwrap(),
        );
        assert_snapshot(&rng_snapshot(&rng), &case["result"]["rng"], "Creativity RNG");
    }
}

#[test]
fn temporary_skills_retain_visible_order_across_provider_and_permanent_changes() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-skill-runtime-goldens.json"))
            .unwrap();
    for journey in fixture["equipmentJourneys"].as_array().unwrap() {
        let (mut book, mut economy, mut runtime, _) = setup(&journey["initial"]["setup"]);
        let mut rng = NativeRng::new(9);
        for (index, record) in journey["records"].as_array().unwrap().iter().enumerate() {
            let action = &record["action"];
            let mut increased = None;
            match action["type"].as_str().unwrap() {
                "equipment" => {
                    let mut effects = Vec::new();
                    for effect in action["effects"].as_array().unwrap() {
                        effects.push(NativeEquipmentEffect {
                            kind: effect["kind"].as_u64().unwrap() as u32,
                            magnitude: effect["magnitude"].as_f64().unwrap(),
                            operator: effect["operator"].as_u64().unwrap() as u32,
                            target: effect["target"].as_u64().unwrap() as u32,
                        });
                    }
                    let item = match action["slot"].as_str().unwrap() {
                        "hat" => economy.equipment.hat.as_mut().unwrap(),
                        "robe" => economy.equipment.robe.as_mut().unwrap(),
                        other => panic!("unknown Source slot {other}"),
                    };
                    item.native_effects = Some(effects);
                }
                "primary" => book.select_primary(action["skillId"].as_u64().unwrap() as usize),
                "grant" => book.grant_ranks(action["skillId"].as_u64().unwrap() as usize, 1),
                "increase" => increased = book.increase_random_learned_skill(&mut rng),
                other => panic!("unknown Source operation {other}"),
            }
            runtime.refresh(&mut book, &economy);
            let context = format!("{} command {index}", journey["name"]);
            assert_eq!(json!(increased), record["skillId"], "{context}");
            assert_snapshot(&book_snapshot(&book), &record["book"], &context);
            assert_snapshot(&runtime_snapshot(&runtime), &record["runtime"], &context);
            assert_snapshot(&rng_snapshot(&rng), &record["rng"], &context);
        }
    }
}
