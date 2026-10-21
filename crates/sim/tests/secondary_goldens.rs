//! Public secondary profiles, mana, reserve and action/recharge clocks from Website.

use serde_json::{Value, json};
use solomon_sim::NativeRng;
use solomon_sim::equipment::{NativeEquipmentModifiers, NativeEquipmentScalarTransform};
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};
use solomon_sim::secondary::*;
use solomon_sim::spells::casting::NativePlayerWeaponKind;

fn skillbook(value: &Value) -> NativeSkillBookState {
    let mut book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
    for (rank, expected) in
        book.effective_ranks.iter_mut().zip(value["effectiveRanks"].as_array().unwrap())
    {
        *rank = expected.as_u64().unwrap() as u32;
    }
    book
}
fn factors(value: &Value) -> NativeOffensiveSpellFactors {
    let equipment = value.get("equipment").map(|value| {
        let mut modifiers = NativeEquipmentModifiers {
            global_damage_flat: value["globalDamageFlat"].as_f64().unwrap(),
            global_damage_multiplier: value["globalDamageMultiplier"].as_f64().unwrap(),
            global_mana_cost_flat: value["globalManaCostFlat"].as_f64().unwrap(),
            global_mana_cost_multiplier: value["globalManaCostMultiplier"].as_f64().unwrap(),
            ..NativeEquipmentModifiers::default()
        };
        for id in 0..8 {
            modifiers.class_damage_flat[id] = value["classDamageFlat"][id].as_f64().unwrap();
            modifiers.class_damage_multiplier[id] =
                value["classDamageMultiplier"][id].as_f64().unwrap();
            modifiers.class_mana_cost_flat[id] = value["classManaCostFlat"][id].as_f64().unwrap();
            modifiers.class_mana_cost_multiplier[id] =
                value["classManaCostMultiplier"][id].as_f64().unwrap();
            modifiers.class_recharge[id] = NativeEquipmentScalarTransform {
                offset: value["classRecharge"][id]["offset"].as_f64().unwrap(),
                scale: value["classRecharge"][id]["scale"].as_f64().unwrap(),
            };
        }
        for id in 0..83 {
            modifiers.skill_damage_flat[id] = value["skillDamageFlat"][id].as_f64().unwrap();
            modifiers.skill_damage_multiplier[id] =
                value["skillDamageMultiplier"][id].as_f64().unwrap();
        }
        modifiers
    });
    NativeOffensiveSpellFactors {
        damage: value["damage"].as_f64().unwrap(),
        mana_cost: value["manaCost"].as_f64().unwrap(),
        global_flat_damage: value["globalFlatDamage"].as_f64().unwrap_or(0.0),
        global_mana_reduction: value["globalManaReduction"].as_f64().unwrap_or(0.0),
        equipment,
    }
}
fn weapon(value: &Value) -> Option<NativePlayerWeaponKind> {
    match value.as_str() {
        Some("staff") => Some(NativePlayerWeaponKind::Staff),
        Some("wand") => Some(NativePlayerWeaponKind::Wand),
        None => None,
        other => panic!("unknown weapon {other:?}"),
    }
}
fn read_player(value: &Value) -> NativeSecondaryPlayerState {
    let mut player = NativeSecondaryPlayerState {
        cast_action: value["castAction"].as_object().map(|_| NativeSecondaryCastAction {
            weapon_kind: weapon(&value["castAction"]["weaponKind"]),
            progress: value["castAction"]["progress"].as_f64().unwrap(),
        }),
        cast_sequence: value["castSequence"].as_u64().unwrap(),
        cast_spin_ticks_remaining: value["castSpinTicksRemaining"].as_u64().unwrap(),
        firewalker: value["firewalker"].as_bool().unwrap(),
        fizzle_sequence: value["fizzleSequence"].as_u64().unwrap(),
        global_cooldown_ticks: value["globalCooldownTicks"].as_f64().unwrap(),
        held_slot: value["heldSlot"].as_u64().map(|id| id as usize),
        last_skill_id: value["lastSkillId"]
            .as_u64()
            .map(|id| NativeSecondaryAbilityId::from_skill_id(id as usize).unwrap()),
        magic_shield_absorb: value["magicShieldAbsorb"].as_f64().unwrap(),
        magic_shield_explosion_damage: value["magicShieldExplosionDamage"].as_f64().unwrap(),
        magic_shield_maximum: value["magicShieldMaximum"].as_f64().unwrap(),
        magic_shield_pulse_ticks: value["magicShieldPulseTicks"].as_u64().unwrap(),
        mindstar: value["mindstar"].as_bool().unwrap(),
        plane_orb_held: value["planeOrbHeld"].as_bool().unwrap(),
        planewalker_ticks_remaining: value["planewalkerTicksRemaining"].as_u64().unwrap(),
        regenerate: value["regenerate"].as_bool().unwrap(),
        reserved_mana: value["reservedMana"].as_f64().unwrap(),
        stoneskin_ticks_remaining: value["stoneskinTicksRemaining"].as_u64().unwrap(),
        ..NativeSecondaryPlayerState::default()
    };
    for id in 0..83 {
        player.cooldown_maximum_ticks_by_skill[id] =
            value["cooldownMaximumTicksBySkill"][id].as_f64().unwrap();
        player.cooldown_ticks_by_skill[id] = value["cooldownTicksBySkill"][id].as_f64().unwrap();
    }
    player
}
fn action_snapshot(action: Option<NativeSecondaryCastAction>) -> Value {
    match action {
        Some(action) => json!({"progress":action.progress,"weaponKind":match action.weapon_kind {
            Some(NativePlayerWeaponKind::Staff)=>Some("staff"),
            Some(NativePlayerWeaponKind::Wand)=>Some("wand"), None=>None,
        }}),
        None => Value::Null,
    }
}
fn player_snapshot(player: &NativeSecondaryPlayerState) -> Value {
    json!({
        "castAction":action_snapshot(player.cast_action),
        "castSequence":player.cast_sequence,"castSpinTicksRemaining":player.cast_spin_ticks_remaining,
        "cooldownMaximumTicksBySkill":&player.cooldown_maximum_ticks_by_skill[..],
        "cooldownTicksBySkill":&player.cooldown_ticks_by_skill[..],
        "firewalker":player.firewalker,"fizzleSequence":player.fizzle_sequence,
        "globalCooldownTicks":player.global_cooldown_ticks,"heldSlot":player.held_slot,
        "lastSkillId":player.last_skill_id.map(NativeSecondaryAbilityId::skill_id),
        "magicShieldAbsorb":player.magic_shield_absorb,"magicShieldExplosionDamage":player.magic_shield_explosion_damage,
        "magicShieldMaximum":player.magic_shield_maximum,"magicShieldPulseTicks":player.magic_shield_pulse_ticks,
        "mindstar":player.mindstar,"planeOrbHeld":player.plane_orb_held,
        "planewalkerTicksRemaining":player.planewalker_ticks_remaining,"regenerate":player.regenerate,
        "reservedMana":player.reserved_mana,"stoneskinTicksRemaining":player.stoneskin_ticks_remaining,
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

#[test]
fn every_secondary_rank_cost_reserve_and_cast_clock_replays_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-goldens.json")).unwrap();
    for case in fixture["profiles"].as_array().unwrap() {
        let book = skillbook(&case["book"]);
        let ability =
            NativeSecondaryAbilityId::from_skill_id(case["skillId"].as_u64().unwrap() as usize)
                .unwrap();
        let stats = effective_secondary_ability_rank_stats(&book, ability);
        let mut values = json!({});
        for property in stats.values {
            values[property.name] = json!(property.value);
        }
        assert_snapshot(
            &json!({"rank":stats.rank,"skillId":ability.skill_id(),"values":values}),
            &case["stats"],
            "ranked stats",
        );
        assert_eq!(
            native_secondary_cooldown_capacity_ticks(&book, ability),
            case["cooldown"].as_f64().unwrap(),
            "{ability:?} cooldown"
        );
        assert_eq!(
            native_secondary_ability_mana_cost(
                &book,
                ability,
                &factors(&case["factors"]),
                NativeSecondaryManaCostBonuses {
                    explosive_shield: 3.0,
                    golem: 7.0,
                    magic_storm: 11.0,
                }
            ),
            case["manaCost"].as_f64().unwrap(),
            "{ability:?} mana"
        );
    }
    for case in fixture["castActions"].as_array().unwrap() {
        let mut action = Some(NativeSecondaryCastAction {
            weapon_kind: weapon(&case["weaponKind"]),
            progress: 0.0,
        });
        let records = case["records"].as_array().unwrap();
        for (index, record) in records.iter().enumerate() {
            assert_snapshot(&action_snapshot(action), record, &format!("cast action {index}"));
            action = step_native_secondary_cast_action(
                action,
                case["fasterCasterPercent"].as_f64().unwrap(),
            );
        }
        assert!(action.is_none());
    }
    for case in fixture["reserves"].as_array().unwrap() {
        let book = skillbook(&case["book"]);
        let mut player = read_player(&case["player"]);
        player.reserved_mana = player.mana_reserve(&book, case["maximumMana"].as_f64().unwrap());
        assert_eq!(player.reserved_mana, case["reserve"].as_f64().unwrap());
        assert_eq!(
            player.mana_ceiling(case["maximumMana"].as_f64().unwrap()),
            case["ceiling"].as_f64().unwrap()
        );
    }
    for case in fixture["cooldownJourneys"].as_array().unwrap() {
        let book = skillbook(&case["book"]);
        let factors = factors(&case["factors"]);
        let mut player = read_player(&case["initial"]);
        for (index, record) in case["records"].as_array().unwrap().iter().enumerate() {
            player.step(&book, case["rechargeFactor"].as_f64().unwrap(), &factors);
            assert_snapshot(
                &player_snapshot(&player),
                record,
                &format!("cooldown tick {}", index + 1),
            );
        }
    }
    for case in fixture["focusCases"].as_array().unwrap() {
        let mut book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
        for ability in NativeSecondaryAbilityId::ALL {
            book.effective_ranks[ability.skill_id()] = 1;
        }
        let mut player = read_player(&case["initial"]);
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        player.step(&book, 0.0, &NativeOffensiveSpellFactors::default());
        player.begin_cooldown(
            NativeSecondaryAbilityId::Stoneskin,
            10000.0,
            case["chance"].as_f64().unwrap(),
            &mut rng,
        );
        assert_snapshot(
            &json!(&player.cooldown_ticks_by_skill[..]),
            &case["result"]["cooldownTicksBySkill"],
            "Focus cooldown",
        );
        assert_snapshot(
            &json!(&player.cooldown_maximum_ticks_by_skill[..]),
            &case["result"]["cooldownMaximumTicksBySkill"],
            "Focus maximum",
        );
        assert_eq!(
            player.global_cooldown_ticks,
            case["result"]["globalCooldownTicks"].as_f64().unwrap()
        );
        assert_snapshot(
            &json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":&rng.words[..]}),
            &case["rng"],
            "Focus RNG",
        );
    }
    let book = skillbook(&fixture["reserves"][0]["book"]);
    assert_eq!(native_plane_orb_damage(&book), fixture["planeOrbDamage"].as_f64().unwrap());
}
