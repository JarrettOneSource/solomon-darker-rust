//! Native offer RNG ownership, level-up acquisition and experience scaling.

use serde_json::{Value, json};
use solomon_sim::profile::{
    NativePotionKind, NativeSkillBookState, PlayerProgressionState, PlayerSkillChoice,
    PlayerSkillOffer, WizardDiscipline, WizardElement, boneyard_enemy_experience_award,
    build_player_skill_offer, draw_native_player_creation_offer_seed,
};
use solomon_sim::{NativeRng, combat::PlayerCombatState};

fn read_rng(value: &Value) -> NativeRng {
    let mut rng = NativeRng::new(0);
    rng.index_a = value["indexA"].as_u64().unwrap() as usize;
    rng.index_b = value["indexB"].as_u64().unwrap() as usize;
    rng.words = value["words"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u32)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    rng
}
fn rng_snapshot(value: &NativeRng) -> Value {
    json!({"indexA":value.index_a,"indexB":value.index_b,"words":&value.words[..]})
}
fn read_book(value: &Value) -> NativeSkillBookState {
    let ranks = |key: &str| {
        value[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    };
    NativeSkillBookState {
        advanced_unlocks: value["advancedUnlocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_bool().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        discipline_root: value["disciplineRoot"].as_u64().unwrap() as usize,
        effective_ranks: ranks("effectiveRanks"),
        element_root: value["elementRoot"].as_u64().unwrap() as usize,
        learned_skill_order: value["learnedSkillOrder"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect(),
        permanent_ranks: ranks("permanentRanks"),
        primary_skill_id: value["primarySkillId"].as_u64().unwrap() as usize,
        weld_build_id: value["weldBuildId"].as_u64().map(|value| value as u32),
        weld_component_ranks: value["weldComponentRanks"].as_array().map(|values| {
            values
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        }),
    }
}
fn book_snapshot(value: &NativeSkillBookState) -> Value {
    json!({"advancedUnlocks":value.advanced_unlocks,"disciplineRoot":value.discipline_root,
        "effectiveRanks":&value.effective_ranks[..],"elementRoot":value.element_root,
        "learnedSkillOrder":value.learned_skill_order,"permanentRanks":&value.permanent_ranks[..],
        "primarySkillId":value.primary_skill_id,"weldBuildId":value.weld_build_id,"weldComponentRanks":value.weld_component_ranks})
}
fn offer_snapshot(value: &PlayerSkillOffer) -> Value {
    let options: Vec<_> = value
        .options
        .iter()
        .map(|option| {
            let mut result = json!({"skillId":option.skill_id,"targetRank":option.target_rank});
            if option.insight {
                result["insight"] = json!(true);
            }
            if let Some(id) = option.weld_build_id {
                result["weldBuildId"] = json!(id);
            }
            result
        })
        .collect();
    let mut result = json!({"level":value.level,"options":options,"sequence":value.sequence});
    if let Some(index) = value.automatic_choice_index {
        result["automaticChoiceIndex"] = json!(index);
    }
    result
}
fn read_progression(value: &Value) -> PlayerProgressionState {
    assert!(value["pendingOffer"].is_null(), "golden input starts outside an offer");
    let mut state = PlayerProgressionState::new(value["offerSeed"].as_u64().unwrap() as u32);
    state.hagatha_runtime.cheat_death_charges =
        value["hagathaRuntime"]["cheatDeathCharges"].as_u64().unwrap() as u32;
    state.hagatha_runtime.reverie_active =
        value["hagathaRuntime"]["reverieActive"].as_bool().unwrap();
    state.hagatha_runtime.serendipity_active =
        value["hagathaRuntime"]["serendipityActive"].as_bool().unwrap();
    state.deferred_skill_choices = value["deferredSkillChoices"].as_u64().unwrap() as usize;
    state.damage_x4_ticks_remaining = value["damageX4TicksRemaining"].as_u64().unwrap();
    state.discipline_offer_bias = value["disciplineOfferBias"].as_bool().unwrap();
    state.exclude_active_weld_build_from_offers =
        value["excludeActiveWeldBuildFromOffers"].as_bool().unwrap();
    state.experience = value["experience"].as_f64().unwrap();
    state.forced_offer_skill_ids = value["forcedOfferSkillIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as usize)
        .collect();
    state.level = value["level"].as_u64().unwrap() as u32;
    state.mind_chug_ticks_remaining = value["mindChugTicksRemaining"].as_u64().unwrap();
    state.next_threshold = value["nextThreshold"].as_f64().unwrap();
    state.offer_cycle = value["offerCycle"].as_u64().unwrap() as u32;
    state.pending_levels = value["pendingLevels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u32)
        .collect();
    state.previous_threshold = value["previousThreshold"].as_f64().unwrap();
    state.poison_immunity_ticks_remaining = value["poisonImmunityTicksRemaining"].as_u64().unwrap();
    state.revision = value["revision"].as_u64().unwrap();
    state.sorcerors_charm_available = value["sorcerorsCharmAvailable"].as_bool().unwrap();
    state.weld_offer_marker = value["weldOfferMarker"].as_u64().unwrap() as u32;
    state.welding_offer_bias = value["weldingOfferBias"].as_bool().unwrap();
    state
}
fn progression_snapshot(value: &PlayerProgressionState, combat: &PlayerCombatState) -> Value {
    json!({"deferredSkillChoices":value.deferred_skill_choices,"damageX4TicksRemaining":value.damage_x4_ticks_remaining,
        "disciplineOfferBias":value.discipline_offer_bias,"excludeActiveWeldBuildFromOffers":value.exclude_active_weld_build_from_offers,
        "experience":value.experience,"forcedOfferSkillIds":value.forced_offer_skill_ids,
        "hagathaRuntime":{"cheatDeathCharges":value.hagatha_runtime.cheat_death_charges,"reverieActive":value.hagatha_runtime.reverie_active,"serendipityActive":value.hagatha_runtime.serendipity_active},
        "level":value.level,"mindChugTicksRemaining":value.mind_chug_ticks_remaining,"nextThreshold":value.next_threshold,
        "offerCycle":value.offer_cycle,"offerSeed":value.offer_seed,"pendingLevels":value.pending_levels,
        "pendingOffer":value.pending_offer.as_ref().map(offer_snapshot),"previousThreshold":value.previous_threshold,
        "poisonImmunityTicksRemaining":value.poison_immunity_ticks_remaining,"revision":value.revision,
        "sorcerorsCharmAvailable":value.sorcerors_charm_available,"weldOfferMarker":value.weld_offer_marker,"weldingOfferBias":value.welding_offer_bias,
        "currentHealth":combat.current_health,"currentMana":combat.current_mana,"maximumHealth":combat.maximum_health,"maximumMana":combat.maximum_mana,
        "poisonBeforeCold":combat.poison_before_cold,"poisonDamagePerTick":combat.poison_damage_per_tick,"poisonTicksRemaining":combat.poison_ticks_remaining})
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
fn native_skill_offers_level_journeys_and_experience_rewards_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/progression-goldens.json")).unwrap();
    for (index, case) in fixture["offers"].as_array().unwrap().iter().enumerate() {
        let state = read_progression(&case["state"]);
        let book = read_book(&case["book"]);
        let mut rng = read_rng(&case["rng"]);
        let offer = build_player_skill_offer(
            &state,
            &book,
            case["state"]["maximumMana"].as_f64().unwrap(),
            7,
            &mut rng,
        );
        assert_snapshot(
            &json!({"offer":offer_snapshot(&offer),"rng":rng_snapshot(&rng)}),
            &case["result"],
            &format!("offer {index}"),
        );
    }
    for journey in fixture["journeys"].as_array().unwrap() {
        let initial = &journey["initial"];
        let element = WizardElement::ALL
            .into_iter()
            .find(|element| Some(element.as_str()) == journey["element"].as_str())
            .unwrap();
        let mut rng = NativeRng::new(12345);
        let offer_seed = draw_native_player_creation_offer_seed(&mut rng);
        let mut state = PlayerProgressionState::new(offer_seed);
        let mut combat = PlayerCombatState::new();
        let mut book = NativeSkillBookState::new(element, WizardDiscipline::Arcane);
        assert_snapshot(
            &progression_snapshot(&state, &combat),
            &initial["state"],
            "player creation",
        );
        assert_snapshot(&rng_snapshot(&rng), &initial["rng"], "creation seed stream");
        for (index, record) in journey["records"].as_array().unwrap().iter().enumerate() {
            match record["type"].as_str().unwrap() {
                "experience" => state.grant_experience(
                    &book,
                    &mut combat,
                    record["amount"].as_f64().unwrap(),
                    &mut rng,
                    true,
                ),
                "open" => state.open_next_offer(&book, combat.maximum_mana, &mut rng, true),
                "reroll" => assert!(state.reroll_offer(
                    &book,
                    combat.maximum_mana,
                    record["sequence"].as_u64().unwrap(),
                    record["seed"].as_u64().unwrap() as u32,
                    &mut rng
                )),
                "choice" => {
                    let choice = &record["selection"];
                    assert!(state.choose_skill(
                        &mut book,
                        PlayerSkillChoice {
                            choice_index: choice["choiceIndex"].as_u64().unwrap() as usize,
                            offer_sequence: choice["offerSequence"].as_u64().unwrap(),
                            skill_id: choice["skillId"].as_u64().unwrap() as usize
                        },
                        &mut rng,
                        &[6]
                    ));
                }
                other => panic!("unknown progression record {other}"),
            }
            let context = format!("{} record {index}", journey["element"]);
            assert_snapshot(&progression_snapshot(&state, &combat), &record["state"], &context);
            assert_snapshot(&book_snapshot(&book), &record["book"], &context);
            assert_snapshot(&rng_snapshot(&rng), &record["rng"], &context);
        }
    }
    for case in fixture["rewards"].as_array().unwrap() {
        let input = &case["input"];
        let actual = boneyard_enemy_experience_award(
            input["evaluatedActorReward"].as_f64().unwrap(),
            input["arenaPlayerCount"].as_u64().unwrap() as u32,
            input["receiverLevel"].as_u64().unwrap() as u32,
            input["gameplayScalar"].as_f64().unwrap(),
            input["receiverXpBonus"].as_f64().unwrap(),
        );
        assert_eq!(actual, case["experience"].as_f64().unwrap());
    }
}

#[test]
fn native_potions_purchase_effects_and_deferred_skill_choices_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/progression-goldens.json")).unwrap();
    for journey in fixture["auxiliaryJourneys"].as_array().unwrap() {
        let initial = &journey["initial"];
        let mut state = read_progression(&initial["state"]);
        let mut book = read_book(&initial["book"]);
        let mut rng = read_rng(&initial["rng"]);
        let mut combat = PlayerCombatState::new();
        combat.current_health = initial["state"]["currentHealth"].as_f64().unwrap();
        combat.current_mana = initial["state"]["currentMana"].as_f64().unwrap();
        combat.maximum_health = initial["state"]["maximumHealth"].as_f64().unwrap();
        combat.maximum_mana = initial["state"]["maximumMana"].as_f64().unwrap();
        combat.poison_before_cold = initial["state"]["poisonBeforeCold"].as_bool().unwrap();
        combat.poison_damage_per_tick = initial["state"]["poisonDamagePerTick"].as_f64().unwrap();
        combat.poison_ticks_remaining = initial["state"]["poisonTicksRemaining"].as_u64().unwrap();
        for (index, record) in journey["records"].as_array().unwrap().iter().enumerate() {
            let action = &record["action"];
            let mut accepted = None;
            let mut recovered_health = None;
            let mut skill_id = None;
            match action["type"].as_str().unwrap() {
                "potion" => state.apply_potion(
                    NativePotionKind::from_native_subtype(
                        action["subtype"].as_u64().unwrap() as u32
                    )
                    .unwrap(),
                    &mut combat,
                ),
                "step-potions" => {
                    for _ in 0..action["ticks"].as_u64().unwrap() {
                        state.step_potion_effects();
                    }
                }
                "reset-potions" => state.reset_potion_effects(),
                "damage-bonus" => state.apply_damage_x4_bonus(),
                "mind-dredge" => state.grant_mind_dredge(),
                "bonus-choice" => state.grant_bonus_skill_choice(
                    &book,
                    combat.maximum_mana,
                    &mut rng,
                    action["charmOwned"].as_bool().unwrap(),
                ),
                "automatic-choice" => {
                    accepted = Some(
                        state.set_automatic_choice(action["index"].as_u64().unwrap() as usize),
                    );
                }
                "defer" => {
                    accepted = Some(state.defer_choice(
                        &book,
                        combat.maximum_mana,
                        action["sequence"].as_u64().unwrap(),
                        &mut rng,
                        action["charmOwned"].as_bool().unwrap(),
                    ));
                }
                "reroll" => {
                    accepted = Some(state.reroll_offer(
                        &book,
                        combat.maximum_mana,
                        action["sequence"].as_u64().unwrap(),
                        action["seed"].as_u64().unwrap() as u32,
                        &mut rng,
                    ));
                }
                "choice" => {
                    accepted = Some(state.choose_skill(
                        &mut book,
                        PlayerSkillChoice {
                            choice_index: action["index"].as_u64().unwrap() as usize,
                            offer_sequence: action["sequence"].as_u64().unwrap(),
                            skill_id: action["skillId"].as_u64().unwrap() as usize,
                        },
                        &mut rng,
                        &[],
                    ));
                }
                "open" => state.open_next_offer(
                    &book,
                    combat.maximum_mana,
                    &mut rng,
                    action["charmOwned"].as_bool().unwrap(),
                ),
                "random-skill" => skill_id = book.increase_random_learned_skill(&mut rng),
                "weird-caster" => {
                    let owned: Vec<_> = action["owned"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|selector| selector.as_u64().unwrap() as u32)
                        .collect();
                    skill_id = book.grant_weird_caster_skill(&mut rng, &owned);
                }
                "revelation" => {
                    let element = WizardElement::ALL
                        .into_iter()
                        .find(|element| Some(element.as_str()) == action["element"].as_str())
                        .unwrap();
                    book.apply_starting_revelation(element);
                }
                "hagatha-purchase" => {
                    let selectors: Vec<_> = action["selectors"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|selector| selector.as_u64().unwrap() as u32)
                        .collect();
                    state.hagatha_runtime.purchase(&selectors);
                }
                "hagatha-remove" => {
                    state.hagatha_runtime.remove(action["selector"].as_u64().unwrap() as u32)
                }
                "hagatha-hurt" => state.hagatha_runtime.hurt(action["damage"].as_f64().unwrap()),
                "cheat-death" => {
                    recovered_health =
                        state.hagatha_runtime.consume_cheat_death(combat.maximum_health)
                }
                other => panic!("unknown auxiliary progression action {other}"),
            }
            let context = format!("{} record {index} {}", journey["name"], action["type"]);
            assert_eq!(json!(accepted), record["accepted"], "{context} accepted");
            assert_eq!(json!(recovered_health), record["recoveredHealth"], "{context} recovery");
            assert_eq!(json!(skill_id), record["skillId"], "{context} skill");
            assert_snapshot(&progression_snapshot(&state, &combat), &record["state"], &context);
            assert_snapshot(&book_snapshot(&book), &record["book"], &context);
            assert_snapshot(&rng_snapshot(&rng), &record["rng"], &context);
        }
    }
}

#[test]
fn purchased_hagatha_modifiers_bosses_and_drinker_boundaries_replay_the_website() {
    use solomon_sim::profile::{
        NATIVE_HAGATHA_FACTORS, NativeHagathaRuntimeState, native_hagatha_boss_damage_factor,
        native_hagatha_derived_modifiers, native_hagatha_drinker_should_use_health_potion,
        native_hagatha_drinker_should_use_mana_potion, native_hagatha_revelation_rank,
    };

    fn selectors(value: &Value) -> Vec<u32> {
        let mut selectors = Vec::new();
        for selector in value.as_array().unwrap() {
            selectors.push(selector.as_u64().unwrap() as u32);
        }
        selectors
    }
    fn number(value: &Value) -> f64 {
        if let Some(number) = value.as_f64() {
            return number;
        }
        match value.as_str().unwrap() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("unknown non-finite input {other}"),
        }
    }

    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/progression-goldens.json")).unwrap();
    let fixture = &fixture["hagathaEffects"];
    let factors = NATIVE_HAGATHA_FACTORS;
    assert_snapshot(
        &json!({
            "bareHandsDamage": factors.bare_hands_damage,
            "bareHandsMana": factors.bare_hands_mana,
            "bruteMelee": factors.brute_melee,
            "brutePush": factors.brute_push,
            "cheatDeathRecovery": factors.cheat_death_recovery,
            "curingPoison": factors.curing_poison,
            "focusRecharge": factors.focus_recharge,
            "glassCannon": factors.glass_cannon,
            "life": factors.life,
            "mana": factors.mana,
            "serendipityDamage": factors.serendipity_damage,
            "speed": factors.speed,
            "warMana": factors.war_mana,
        }),
        &fixture["factors"],
        "native Hagatha factors",
    );
    for case in fixture["modifiers"].as_array().unwrap() {
        let runtime = NativeHagathaRuntimeState {
            cheat_death_charges: case["runtime"]["cheatDeathCharges"].as_u64().unwrap() as u32,
            reverie_active: case["runtime"]["reverieActive"].as_bool().unwrap(),
            serendipity_active: case["runtime"]["serendipityActive"].as_bool().unwrap(),
        };
        let modifiers = native_hagatha_derived_modifiers(
            &selectors(&case["owned"]),
            runtime,
            case["weaponEquipped"].as_bool().unwrap(),
        );
        assert_snapshot(
            &json!({
                "castSpeedFactor": modifiers.cast_speed_factor,
                "incomingDamageFactor": modifiers.incoming_damage_factor,
                "meleeDamageFactor": modifiers.melee_damage_factor,
                "movementFactor": modifiers.movement_factor,
                "offensiveManaFactor": modifiers.offensive_mana_factor,
                "poisonDamageFactor": modifiers.poison_damage_factor,
                "pushStrengthFactor": modifiers.push_strength_factor,
                "rechargeFactor": modifiers.recharge_factor,
                "spellDamageFactor": modifiers.spell_damage_factor,
            }),
            &case["result"],
            &format!("modifiers {} {}", case["owned"], case["runtime"]),
        );
    }
    for case in fixture["bosses"].as_array().unwrap() {
        assert_eq!(
            native_hagatha_boss_damage_factor(
                &selectors(&case["owned"]),
                case["nativeTypeId"].as_u64().unwrap() as u32,
            ),
            case["result"].as_f64().unwrap(),
        );
    }
    for case in fixture["drinkerHealth"].as_array().unwrap() {
        assert_eq!(
            native_hagatha_drinker_should_use_health_potion(
                &selectors(&case["owned"]),
                number(&case["currentHealth"]),
            ),
            case["result"].as_bool().unwrap(),
        );
    }
    for case in fixture["drinkerMana"].as_array().unwrap() {
        assert_eq!(
            native_hagatha_drinker_should_use_mana_potion(
                &selectors(&case["owned"]),
                number(&case["currentMana"]),
                number(&case["maximumMana"]),
                number(&case["cost"]),
            ),
            case["result"].as_bool().unwrap(),
        );
    }
    for case in fixture["revelation"].as_array().unwrap() {
        assert_eq!(
            native_hagatha_revelation_rank(
                case["rank"].as_u64().unwrap() as u32,
                &selectors(&case["owned"]),
            ),
            case["result"].as_u64().unwrap() as u32,
        );
    }
}

#[test]
fn native_imported_threshold_fractions_survive_until_ordinary_xp_replaces_them() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/progression-import-threshold-goldens.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let initial = &case["initial"];
        let mut state = read_progression(&initial["state"]);
        let book = read_book(&initial["book"]);
        let mut rng = read_rng(&initial["rng"]);
        let mut combat = PlayerCombatState::new();
        combat.current_health = initial["state"]["currentHealth"].as_f64().unwrap();
        combat.current_mana = initial["state"]["currentMana"].as_f64().unwrap();
        combat.maximum_health = initial["state"]["maximumHealth"].as_f64().unwrap();
        combat.maximum_mana = initial["state"]["maximumMana"].as_f64().unwrap();
        assert_snapshot(
            &progression_snapshot(&state, &combat),
            &initial["state"],
            case["name"].as_str().unwrap(),
        );
        assert_eq!(
            state.experience_progress(),
            initial["progress"].as_f64().unwrap(),
            "{}",
            case["name"]
        );
        for record in case["records"].as_array().unwrap() {
            state.grant_experience(
                &book,
                &mut combat,
                record["amount"].as_f64().unwrap(),
                &mut rng,
                false,
            );
            assert_snapshot(
                &progression_snapshot(&state, &combat),
                &record["state"],
                case["name"].as_str().unwrap(),
            );
            assert_snapshot(&rng_snapshot(&rng), &record["rng"], case["name"].as_str().unwrap());
            assert_eq!(
                state.experience_progress(),
                record["progress"].as_f64().unwrap(),
                "{}",
                case["name"]
            );
        }
    }
}
