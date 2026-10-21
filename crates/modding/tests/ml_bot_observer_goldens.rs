//! Complete policy frames borrow restored canonical Sessions and the existing Source collator.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solomon_assets::website_save;
use solomon_modding::json::SourceCollator;
use solomon_sim::Vec2;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::ml_bot::spec::OBSERVATION_NAMES;
use solomon_sim::ml_bot::world_state::MlBotPolicyCommittedAction;
use solomon_sim::ml_bot::{
    MlBotPolicyControllerKind, MlBotPolicyFrame, MlBotPolicyObservationContext, MlBotPolicyObserver,
};
use solomon_sim::session::{GameSessionInput, GameSessionTickContext};

#[path = "../../sim/tests/support/mod.rs"]
mod support;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn point_json(point: Vec2) -> Value {
    json!({ "x":point.x,"y":point.y })
}

fn input(value: &Value) -> GameSessionInput {
    GameSessionInput {
        aim: if value["aim"].is_null() { None } else { Some(point(&value["aim"])) },
        movement: point(&value["movement"]),
        primary: value["cast"]["primary"].as_bool().unwrap(),
        quickbar: value["cast"]["quickbar"].as_u64().map(|slot| slot as usize),
        viewport: Vec2::new(
            value["viewportWidth"].as_f64().unwrap(),
            value["viewportHeight"].as_f64().unwrap(),
        ),
    }
}

fn frame_metadata(frame: &MlBotPolicyFrame) -> Value {
    json!({
        "enemyRows":frame.enemy_rows.iter().map(|row| json!({
            "currentHealth":row.current_health,"headingDeg":row.heading_deg,"id":row.id,
            "maximumHealth":row.maximum_health,"position":point_json(row.position),"radius":row.radius,
            "species":row.species.as_str(),"targetPlayerId":row.target_player_id,"velocity":point_json(row.velocity),
        })).collect::<Vec<_>>(),
        "targetId":frame.target_id,
        "player":{"primaryAffordable":frame.player.primary_affordable,"primaryRange":frame.player.primary_range,
            "secondarySlots":frame.player.secondary_slots.iter().map(|slot| json!({
                "affordable":slot.affordable,"occupied":slot.occupied,"ready":slot.ready,"skillId":slot.skill_id,
            })).collect::<Vec<_>>()},
        "potions":frame.inventory.potions.iter().map(|row|json!({"itemId":row.item_id,"legal":row.legal})).collect::<Vec<_>>(),
        "ownMinionTargetIds":frame.minions.own_target_ids,
        "secondaryMinionActive":frame.minions.secondary_minion_active,
    })
}

#[test]
fn complete_source_observations_preserve_real_family_clocks_and_history() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/ml-bot-observer-goldens.json"
    ))
    .unwrap();
    let collator = SourceCollator::default();
    let compare = |left: &str, right: &str| collator.compare(left, right);
    for case in fixture["cases"].as_array().unwrap() {
        let restored =
            website_save::restore(&serde_json::to_string(&case["document"]).unwrap()).unwrap();
        let navigation = restored.loaded_boneyard.as_ref().map(NavigationSet::for_scene);
        let mut session = restored.session;
        let mut observer = MlBotPolicyObserver::new("local".into()).unwrap();
        let active_inputs = case["activeInputs"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, value)| (id.clone(), input(value)))
            .collect::<BTreeMap<_, _>>();
        let controllers = case["controllers"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, value)| {
                (
                    id.clone(),
                    if value == "human" {
                        MlBotPolicyControllerKind::Human
                    } else {
                        MlBotPolicyControllerKind::Bot
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let context = MlBotPolicyObservationContext {
            active_inputs: &active_inputs,
            controllers: &controllers,
            has_consumable: None,
            compare_player_ids: &compare,
        };
        for (index, expected) in case["frames"].as_array().unwrap().iter().enumerate() {
            let tick = expected["tick"].as_u64().unwrap();
            while session.ticks < tick {
                assert!(
                    session
                        .step(
                            &active_inputs,
                            GameSessionTickContext {
                                navigation: navigation.as_ref(),
                                ..GameSessionTickContext::default()
                            }
                        )
                        .advanced
                );
            }
            let frame = observer.observe(&session, &context).unwrap();
            let path = format!("{} frame {index}", case["name"]);
            let expected_values = expected["output"]["valueBits"].as_array().unwrap();
            assert_eq!(frame.values.len(), expected_values.len(), "{path}");
            for (index, (actual, expected)) in frame.values.iter().zip(expected_values).enumerate()
            {
                assert_eq!(
                    actual.to_bits(),
                    expected.as_u64().unwrap() as u32,
                    "{path} {} [{index}]",
                    OBSERVATION_NAMES[index]
                );
            }
            let mut expected_metadata = expected["output"].clone();
            expected_metadata.as_object_mut().unwrap().remove("values");
            expected_metadata.as_object_mut().unwrap().remove("valueBits");
            support::assert_website_json(&frame_metadata(&frame), &expected_metadata, &path, &[]);
            let action = &expected["commit"]["action"];
            observer.commit(
                MlBotPolicyCommittedAction {
                    ability_action: action["abilityAction"].as_u64().unwrap() as usize,
                    movement: point(&action["movement"]),
                    target_action: action["targetAction"].as_u64().unwrap() as usize,
                    target_switched: action["targetSwitched"].as_bool().unwrap(),
                },
                expected["commit"]["targetId"].as_u64().map(|id| id as usize),
            );
        }
    }
}

fn action_plan_json(plan: &solomon_sim::ml_bot::MlBotPolicyActionMaskPlan) -> Value {
    json!({
        "abilityByTarget":plan.ability_by_target,"aimByAbility":plan.aim_by_ability,
        "movement":plan.movement,"target":plan.target,
    })
}

fn decision_json(decision: &solomon_sim::ml_bot::MlBotPolicyDecision) -> Value {
    let hub_action = match decision.hub_action {
        Some(solomon_sim::economy::HubInventoryAction::Consume { item_id }) => {
            json!({"type":"consume","itemId":item_id})
        }
        None => Value::Null,
        _ => panic!("the policy only emits Consume inventory actions"),
    };
    json!({
        "committedAction":{
            "abilityAction":decision.committed_action.ability_action,
            "movement":point_json(decision.committed_action.movement),
            "targetAction":decision.committed_action.target_action,
            "targetSwitched":decision.committed_action.target_switched,
        },
        "hubAction":hub_action,
        "input":{
            "aim":decision.input.aim.map(point_json),
            "cast":{"primary":decision.input.primary,"quickbar":decision.input.quickbar},
            "movement":point_json(decision.input.movement),
            "viewportWidth":decision.input.viewport.x,"viewportHeight":decision.input.viewport.y,
        },
        "masks":{
            "ability":decision.masks.ability,"aim":decision.masks.aim,
            "movement":decision.masks.movement,"target":decision.masks.target,
        },
        "targetId":decision.target_id,
    })
}

#[test]
fn source_legal_masks_and_all_action_heads_preserve_the_interpreted_intent() {
    use solomon_sim::combat::PlayerLifeState;
    use solomon_sim::ml_bot::player_state::MlBotPolicySecondarySlot;
    use solomon_sim::ml_bot::{
        MlBotPolicyActionContext, MlBotPolicyActionIndices, create_ml_bot_policy_action_mask_plan,
        resolve_ml_bot_policy_decision,
    };

    let observations: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/ml-bot-observer-goldens.json"
    ))
    .unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/ml-bot-actions-goldens.json"
    ))
    .unwrap();
    let collator = SourceCollator::default();
    let compare = |left: &str, right: &str| collator.compare(left, right);
    for case in fixture["cases"].as_array().unwrap() {
        let source = &observations["cases"][case["sourceIndex"].as_u64().unwrap() as usize];
        let mut session = website_save::restore(&source["document"].to_string()).unwrap().session;
        let active_inputs = source["activeInputs"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, value)| (id.clone(), input(value)))
            .collect();
        let controllers = source["controllers"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, value)| {
                (
                    id.clone(),
                    if value == "human" {
                        MlBotPolicyControllerKind::Human
                    } else {
                        MlBotPolicyControllerKind::Bot
                    },
                )
            })
            .collect();
        let context = MlBotPolicyObservationContext {
            active_inputs: &active_inputs,
            controllers: &controllers,
            has_consumable: None,
            compare_player_ids: &compare,
        };
        let mut frame =
            MlBotPolicyObserver::new("local".into()).unwrap().observe(&session, &context).unwrap();
        // These are explicit inputs to actions.ts's public frame contract, applied
        // after a real observation. They do not claim a separate gameplay trace.
        let patch = &case["patch"];
        let player = &mut session.participants.get_mut("local").unwrap().player;
        if let Some(id) = patch["primary"].as_u64() {
            player.book.primary_skill_id = id as usize;
        }
        if patch.get("weld").is_some() {
            player.book.weld_build_id = patch["weld"].as_u64().map(|id| id as u32);
        }
        if patch["continuing"] == true {
            frame.player.block_a[6] = 1.0;
        }
        if let Some(index) = patch["gated"].as_u64() {
            frame.player.block_a[index as usize] = 1.0;
        }
        if patch["dead"] == true {
            player.combat.life_state = PlayerLifeState::LethalPending;
        }
        if patch["noTargets"] == true {
            frame.enemy_rows.clear();
            frame.target_id = None;
        }
        if let Some(id) = patch["targetId"].as_u64() {
            frame.target_id = Some(id as usize);
        }
        if patch["far"] == true {
            for row in &mut frame.enemy_rows {
                row.position = Vec2::new(1900.0, 1900.0);
            }
        }
        if let Some(affordable) = patch["affordable"].as_bool() {
            frame.player.primary_affordable = affordable;
        }
        if let Some(ready) = patch["ready"].as_bool() {
            frame.player.block_a[7] = if ready { 1.0 } else { 0.0 };
        }
        if let Some(rays) = patch["rays"].as_array() {
            for (index, value) in rays.iter().enumerate() {
                frame.geometry.patch_and_rays[index] = value.as_f64().unwrap() as f32;
            }
        }
        if let Some(ids) = patch["secondaryIds"].as_array() {
            for (slot, id) in ids.iter().enumerate() {
                frame.player.secondary_slots[slot] = MlBotPolicySecondarySlot {
                    affordable: true,
                    occupied: true,
                    ready: true,
                    skill_id: Some(id.as_u64().unwrap() as usize),
                };
            }
        }
        let path = case["name"].as_str().unwrap();
        match create_ml_bot_policy_action_mask_plan(&session, "local", &frame) {
            Ok(plan) => support::assert_website_json(
                &action_plan_json(&plan),
                &case["mask"]["value"],
                path,
                &[],
            ),
            Err(error) => assert_eq!(error, case["mask"]["error"].as_str().unwrap(), "{path}"),
        }
        let captured = MlBotPolicyActionContext::capture(&session, "local").unwrap();
        for expected in case["decisions"].as_array().unwrap() {
            let action = &expected["action"];
            let action = MlBotPolicyActionIndices {
                ability: action["ability"].as_u64().unwrap() as usize,
                aim: action["aim"].as_u64().unwrap() as usize,
                movement: action["movement"].as_u64().unwrap() as usize,
                target: action["target"].as_u64().unwrap() as usize,
            };
            let actual = resolve_ml_bot_policy_decision(&session, "local", &frame, action);
            match actual {
                Ok(decision) => {
                    support::assert_website_json(
                        &decision_json(&decision),
                        &expected["value"],
                        path,
                        &[],
                    );
                    support::assert_website_json(
                        &decision_json(&captured.resolve(&frame, action).unwrap()),
                        &expected["value"],
                        path,
                        &[],
                    );
                }
                Err(error) => assert_eq!(error, expected["error"].as_str().unwrap(), "{path}"),
            }
        }
    }
}

fn host_intent_json(intent: &solomon_sim::ml_bot::MlBotHostIntent) -> Value {
    use solomon_sim::ml_bot::MlBotHostIntent;
    let input_value = |input: &GameSessionInput| {
        json!({
            "aim":input.aim.map(point_json),"cast":{"primary":input.primary,"quickbar":input.quickbar},
            "movement":point_json(input.movement),"viewportWidth":input.viewport.x,"viewportHeight":input.viewport.y,
        })
    };
    match intent {
        MlBotHostIntent::Input(input) => json!({"kind":"input","input":input_value(input)}),
        MlBotHostIntent::ScriptedInput(input) => {
            json!({"kind":"scripted-input","input":input_value(input)})
        }
        MlBotHostIntent::HubAction(solomon_sim::economy::HubInventoryAction::Consume {
            item_id,
        }) => json!({"kind":"hub-action","action":{"type":"consume","itemId":item_id}}),
        MlBotHostIntent::HubAction(_) => panic!("the policy only emits Consume inventory actions"),
        MlBotHostIntent::SelectSkill { choice_index, offer_sequence, skill_id } => json!({
            "kind":"select-skill","choiceIndex":choice_index,"offerSequence":offer_sequence,"skillId":skill_id,
        }),
        MlBotHostIntent::ConfirmLoadout(character) => json!({
            "kind":"confirm-loadout","character":{"displayName":character.name,
                "element":character.element.as_str(),"discipline":character.discipline.as_str()},
        }),
    }
}

fn request_json(request: &solomon_sim::ml_bot::MlBotHostInferenceRequest) -> Value {
    use solomon_sim::ml_bot::MlBotHostInferenceRequest;
    let words = |values: &[f32]| values.iter().map(|value| value.to_bits()).collect::<Vec<_>>();
    match request {
        MlBotHostInferenceRequest::Actions { id, observation, plan } => json!({
            "kind":"actions","id":id,"observationBits":words(observation),"plan":action_plan_json(plan),
        }),
        MlBotHostInferenceRequest::Choice { id, observation, descriptors, mask } => json!({
            "kind":"choice","id":id,"observationBits":words(observation),
            "descriptorBits":words(descriptors),"mask":mask,
        }),
    }
}

#[test]
fn real_source_controller_preserves_async_admission_deadlines_and_captured_actions() {
    use solomon_sim::game_run::GameRunPhase;
    use solomon_sim::ml_bot::{
        MlBotHostController, MlBotHostControllerContext, MlBotHostControllerError,
        MlBotHostInferenceResult, MlBotHostIntent, MlBotPolicyActionIndices, MlBotPolicyChoiceMode,
    };
    use solomon_sim::profile::{
        NewGameSelection, PlayerSkillChoice, WizardDiscipline, WizardElement,
    };

    let observations: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/ml-bot-observer-goldens.json"
    ))
    .unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/ml-bot-controller-goldens.json"
    ))
    .unwrap();
    let restore = |document: &Value| {
        let restored = website_save::restore(&document.to_string()).unwrap();
        (restored.session, restored.loaded_boneyard.as_ref().map(NavigationSet::for_scene))
    };
    let collator = SourceCollator::default();
    let compare = |left: &str, right: &str| collator.compare(left, right);
    let active_inputs = BTreeMap::new();
    let controllers = BTreeMap::from([("local".to_owned(), MlBotPolicyControllerKind::Bot)]);
    let observation_context = || MlBotPolicyObservationContext {
        active_inputs: &active_inputs,
        controllers: &controllers,
        has_consumable: None,
        compare_player_ids: &compare,
    };
    for case in fixture["cases"].as_array().unwrap() {
        let initial = case.get("initialDocument").unwrap_or(&observations["cases"][0]["document"]);
        let (mut session, mut navigation) = restore(initial);
        let mut available = true;
        let mut pending_id = None;
        let mut last_intent = None;
        let mut controller = MlBotHostController::new(
            "local".into(),
            NewGameSelection {
                name: "Policy Bot".into(),
                element: WizardElement::Fire,
                discipline: WizardDiscipline::Arcane,
            },
            if case["choiceMode"] == "scripted" {
                MlBotPolicyChoiceMode::Scripted
            } else {
                MlBotPolicyChoiceMode::Learned
            },
        )
        .unwrap();
        for (index, record) in case["records"].as_array().unwrap().iter().enumerate() {
            let operation = &record["operation"];
            let mut outputs = Vec::new();
            match operation["kind"].as_str().unwrap() {
                "poll" => match controller.poll(available.then_some(MlBotHostControllerContext {
                    session: &session,
                    observation: observation_context(),
                })) {
                    Ok(output) => {
                        if let Some(intent) = output.intent {
                            outputs.push(json!({"intent":host_intent_json(&intent)}));
                            last_intent = Some(intent);
                        }
                        if let Some(request) = output.request {
                            outputs.push(json!({"request":request_json(&request)}));
                            pending_id = Some(request.id());
                        }
                    }
                    Err(MlBotHostControllerError::Policy(error)) => {
                        outputs.push(json!({"error":error}));
                    }
                    Err(error) => panic!("unexpected navigation failure: {error:?}"),
                },
                "context" => available = operation["available"].as_bool().unwrap(),
                "switch" => {
                    (session, navigation) = restore(
                        &observations["cases"][operation["sourceIndex"].as_u64().unwrap() as usize]
                            ["document"],
                    );
                }
                "hub" => session.return_to_hub(),
                "advance" => {
                    let input = GameSessionInput {
                        movement: operation.get("movement").map_or(Vec2::ZERO, point),
                        ..GameSessionInput::default()
                    };
                    for _ in 0..operation["count"].as_u64().unwrap() {
                        session.step(
                            &BTreeMap::from([("local".to_owned(), input)]),
                            GameSessionTickContext {
                                navigation: navigation.as_ref(),
                                ..GameSessionTickContext::default()
                            },
                        );
                    }
                }
                "damage" => session.participants.get_mut("local").unwrap().player.combat.damage(
                    operation["amount"].as_f64().unwrap(),
                    session.ticks,
                    true,
                    1.0,
                ),
                "grant" => {
                    session.grant_experience("local", operation["amount"].as_f64().unwrap());
                }
                "chooseFirst" => {
                    let offer = session
                        .participant("local")
                        .unwrap()
                        .player
                        .progression
                        .pending_offer
                        .as_ref()
                        .unwrap();
                    let choice = PlayerSkillChoice {
                        choice_index: 0,
                        offer_sequence: offer.sequence,
                        skill_id: offer.options[0].skill_id,
                    };
                    assert!(session.choose_skill("local", choice));
                }
                "applyIntent" => {
                    let Some(MlBotHostIntent::SelectSkill {
                        choice_index,
                        offer_sequence,
                        skill_id,
                    }) = last_intent.as_ref()
                    else {
                        panic!("the Source operation expects a skill intent");
                    };
                    assert!(session.choose_skill(
                        "local",
                        PlayerSkillChoice {
                            choice_index: *choice_index,
                            offer_sequence: *offer_sequence,
                            skill_id: *skill_id,
                        }
                    ));
                }
                "finishRun" => {
                    // Source drives its public run kernel here. This fixture tests
                    // controller precedence, not a second combat-death journey.
                    let mut count = 0;
                    while session.run.phase != GameRunPhase::Loadout {
                        session.run.step(&[]);
                        count += 1;
                        assert!(count <= 1501);
                    }
                }
                "complete" => {
                    let result = if let Some(error) = operation["error"].as_str() {
                        Err(error.to_owned())
                    } else if let Some(action) = operation.get("action") {
                        Ok(MlBotHostInferenceResult::Actions(MlBotPolicyActionIndices {
                            ability: action["ability"].as_u64().unwrap() as usize,
                            aim: action["aim"].as_u64().unwrap() as usize,
                            movement: action["movement"].as_u64().unwrap() as usize,
                            target: action["target"].as_u64().unwrap() as usize,
                        }))
                    } else {
                        Ok(MlBotHostInferenceResult::Choice(
                            operation["selectedOption"].as_u64().unwrap() as usize,
                        ))
                    };
                    match controller.complete(
                        pending_id.take().unwrap(),
                        available.then_some(&session),
                        result,
                    ) {
                        Ok(Some(intent)) => {
                            outputs.push(json!({"intent":host_intent_json(&intent)}));
                            last_intent = Some(intent);
                        }
                        Ok(None) => {}
                        Err(MlBotHostControllerError::Policy(error)) => {
                            outputs.push(json!({"error":error}))
                        }
                        Err(error) => panic!("unexpected navigation failure: {error:?}"),
                    }
                }
                kind => panic!("unknown Source operation {kind}"),
            }
            let path = format!("{} operation {index} {}", case["name"], operation["kind"]);
            assert_eq!(session.ticks, record["tick"].as_u64().unwrap(), "{path}");
            assert_eq!(session.run.phase.id(), record["phase"].as_str().unwrap(), "{path}");
            support::assert_website_json(&json!(outputs), &record["outputs"], &path, &[]);
        }
    }
}
