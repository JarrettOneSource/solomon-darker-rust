//! Stateful current-source secondary journeys through the public ordered game tick.
#![cfg(feature = "persistence")]

#[path = "support/secondary_current.rs"]
mod current;
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;

#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod support;

use current::{result_snapshot, state_snapshot};
use native_bits::source_value;
use serde_json::{Value, json};
use solomon_sim::HubWizard;
use solomon_sim::belt::{NativeBeltCommand, PlayerBeltComponent};
use solomon_sim::economy::HubEconomyState;
use solomon_sim::profile::WizardElement;
use solomon_sim::secondary::golem::NativeGolemIncomingDamage;
use solomon_sim::secondary::*;
use std::collections::BTreeMap;
use support::*;

fn fixture() -> Value {
    let stored = include_bytes!("fixtures/website/secondary-tick-goldens.json.zlib");
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(stored)
        .expect("complete secondary recording is valid zlib");
    serde_json::from_slice(&bytes).expect("complete secondary recording is valid JSON")
}

// Both complete Source JSON objects and serialized private payloads share these
// names after adaptation. Numeric values are still compared exactly.
fn assert_current(actual: &Value, expected: &Value, path: &str) {
    assert_snapshot(&source_value(actual), &source_value(expected), path);
}

#[test]
fn every_secondary_family_replays_its_ordered_website_lifetime() {
    let fixture = fixture();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let declared = &case["authority"];
        let book = read_book(&declared["skillBook"]);
        let skill_id = case["skillId"].as_u64().unwrap() as usize;
        let mut belt = PlayerBeltComponent::new(&book);
        assert!(belt.apply(
            NativeBeltCommand::BindSkill { skill_id: Some(skill_id), slot: 0 },
            &book,
            &HubEconomyState::new(77, WizardElement::Ether)
        ));
        let mut wizard = HubWizard::at(point(&declared["character"]["position"]));
        wizard.set_facing(declared["character"]["headingIndex"].as_f64().unwrap() * 15.0);
        wizard.velocity = point(&declared["character"]["velocity"]);
        let mut authority = authority_from_fixture(declared, &book, &belt, &wizard);
        let mut world = World {
            case,
            targets: case["targets"].as_array().unwrap().iter().map(target).collect(),
        };
        let mut state = read_state(&case["initial"]);
        let mut sample = case["samples"].as_array().unwrap().iter().peekable();
        let mode = case["mode"].as_str().unwrap();
        for tick in 1..=2200 {
            authority.input.quickbar =
                if tick == 1 || mode == "toggle-off" && (tick == 201 || tick == 751) {
                    Some(0)
                } else {
                    None
                };
            authority.input.primary = skill_id == 12 && tick % 200 < 100;
            if mode == "live-equipment" && (tick == 800 || tick == 1500) {
                let fixture_factors = &fixture["liveEquipmentFactors"];
                authority.offensive_factors = factors(if tick == 800 {
                    &fixture_factors["equipped"]
                } else {
                    &fixture_factors["plain"]
                });
            }
            if mode == "overload" && tick >= 200 {
                authority.maximum_mana = 1.0;
            }
            if mode == "ineligible-toggle" && tick >= 200 {
                authority.eligible = false;
            }
            if mode == "world-switch" && tick >= 30 {
                authority.world_key = "boneyard:other";
            }
            if matches!(mode, "shield-break" | "shield-explode") && tick == 2 {
                state.apply_player_damage(
                    "player",
                    100000.0,
                    tick,
                    solomon_sim::Vec2::ZERO,
                    authority.world_key,
                    NativeSecondaryPlayerDamagePresentation::default(),
                );
            }
            if mode == "golem-death" && tick == 450 {
                let id = state
                    .actors
                    .iter()
                    .find(|actor| actor.kind == NativeSecondaryActorKind::Golem)
                    .unwrap()
                    .id;
                state.apply_golem_damage(
                    id,
                    NativeGolemIncomingDamage {
                        primary_damage: 1000000.0,
                        secondary_damage: 0.0,
                        reflectable_physical_source_in_range: true,
                    },
                    tick,
                    true,
                    None,
                );
            }
            if mode == "effects" && tick == 2 {
                state.apply_ether_burn(NativeSecondaryEtherBurnInput {
                    owner_id: "player".into(),
                    rank: 2,
                    target: world.targets[0].clone(),
                    world_key: authority.world_key.into(),
                });
                apply_native_secondary_target_effect(
                    &mut state.target_effects,
                    authority.world_key,
                    7,
                    NativeSecondaryTargetEffectPatch {
                        prismatic_ticks: Some(1),
                        frost_burn_ticks: Some(1),
                        frost_burn_damage_per_tick: Some(0.01),
                        frost_burn_owner_id: Some("player".into()),
                        frost_burn_skill_id: Some(35),
                        frost_burn_source_actor_id: Some(1),
                        electric_burn: Some(Some(NativeSecondaryElectricBurnEffectState {
                            arc_count: 2,
                            damage_per_tick: 1.0,
                            owner_id: "player".into(),
                            source_actor_id: 1,
                            stun_factor: 0.8,
                            ticks: 1,
                        })),
                        steamed: Some(Some(NativeSecondarySteamedEffectState {
                            damage_per_tick: 1.0,
                            ember_damage: 2.0,
                            ember_fragments: 3,
                            explode_damage: 4.0,
                            explode_radius: 5.0,
                            owner_id: "player".into(),
                            source_actor_id: 1,
                            ticks: 1,
                        })),
                        ..NativeSecondaryTargetEffectPatch::default()
                    },
                );
            }
            let mut players = BTreeMap::new();
            players.insert("player".to_owned(), authority);
            let result = state.step(
                &NativeSecondaryTickContext { enhanced_effects: None, players: &players, tick },
                &mut world,
                None,
            );
            authority = players.remove("player").unwrap();
            if sample.peek().is_some_and(|sample| sample["tick"].as_u64() == Some(tick)) {
                let expected = sample.next().unwrap();
                let label = format!("{} tick {tick}", case["name"].as_str().unwrap());
                assert_current(&result_snapshot(&result), &expected["result"], &label);
                assert_current(&state_snapshot(&state), &expected["state"], &label);
                let target_positions = Value::Array(
                    world
                        .targets
                        .iter()
                        .map(|target| json!([target.id, point_snapshot(target.position)]))
                        .collect(),
                );
                assert_current(&target_positions, &expected["targetPositions"], &label);
            }
        }
    }
}

#[test]
fn flash_mindblast_and_script_fires_keep_source_birth_and_presentation_order() {
    use solomon_sim::Vec2;
    use solomon_sim::harmful::PlayerFlashResponse;
    use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
    let fixture = fixture();
    for case in fixture["feedback"].as_array().unwrap() {
        let declared = &case["authority"];
        let book = read_book(&declared["skillBook"]);
        let belt = PlayerBeltComponent::new(&book);
        let wizard = HubWizard::at(Vec2::ZERO);
        let mut authority = authority_from_fixture(declared, &book, &belt, &wizard);
        authority.input.quickbar = None;
        let mut players = BTreeMap::new();
        players.insert("player".into(), authority);
        let mut world = World {
            case,
            targets: case["targets"].as_array().unwrap().iter().map(target).collect(),
        };
        let mut order = WorldPainterOrder { next_actor_ordinal: 10, next_transient_ordinal: 20 };
        let mut state = NativeSecondarySimulationState::new(77);
        let mode = case["mode"].as_str().unwrap();
        match mode {
            "flash" => {
                let response = &case["response"];
                let response = PlayerFlashResponse {
                    camera_displacement: point(&response["cameraDisplacement"]),
                    duration_ticks: response["durationTicks"].as_u64().unwrap(),
                    grow_scales: response["growScales"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_f64().unwrap())
                        .collect(),
                    pitch: response["pitch"].as_f64().unwrap(),
                };
                state.materialize_player_flash_response(NativePlayerFlashResponseInput {
                    owner_id: "player",
                    position: Vec2::ZERO,
                    response: &response,
                    target_ids: &[7, 8],
                    tick: 0,
                    world_key: declared["worldKey"].as_str().unwrap(),
                });
                let mut screen_flash =
                    NativeSecondaryScreenFlashState::new(1.0, 0.5, 0.25, 0.1, true, 1.0);
                // emitNativePlayerScreenFlash preserves its supplied payload rather
                // than applying the private screenFlash factory's float32 stores.
                screen_flash.decay_per_tick = 0.1;
                state.emit_player_screen_flash(
                    "player",
                    declared["worldKey"].as_str().unwrap(),
                    Vec2::ZERO,
                    screen_flash,
                    0,
                );
            }
            "script-fires" => {
                let fires = case["fires"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| NativeScriptFire {
                        damage: value["damage"].as_f64().unwrap(),
                        lifetime_ticks: value["lifetimeTicks"].as_u64().unwrap(),
                        position: point(&value["position"]),
                        radius: value["radius"].as_f64().unwrap(),
                    })
                    .collect::<Vec<_>>();
                state.spawn_script_fires(
                    "player",
                    declared["worldKey"].as_str().unwrap(),
                    &fires,
                    &mut order,
                );
            }
            "mindblast" | "mindblast-gameover" => {
                let element = match case["element"].as_str().unwrap() {
                    "air" => WizardElement::Air,
                    "earth" => WizardElement::Earth,
                    "ether" => WizardElement::Ether,
                    "fire" => WizardElement::Fire,
                    "water" => WizardElement::Water,
                    other => panic!("unknown element {other}"),
                };
                let result = state.trigger_player_mindblast(
                    NativePlayerMindblastInput {
                        direct_damage: None,
                        element,
                        level: 7,
                        light_registration: PainterRegistration {
                            manager_lane: WorldManagerLane::Actor,
                            registration_ordinal: 2,
                        },
                        owner_id: "player",
                        position: Vec2::ZERO,
                        presentation_scale: Some(4.5),
                        world_key: declared["worldKey"].as_str().unwrap(),
                    },
                    Some(&mut order),
                );
                assert_snapshot(
                    &json!({"directDamage":result.direct_damage,"directRadius":result.direct_radius}),
                    &case["result"],
                    "Mindblast direct result",
                );
            }
            other => panic!("unknown feedback mode {other}"),
        }
        assert_current(&state_snapshot(&state), &case["initial"], mode);
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for tick in 1..=250 {
            let outbox = if mode == "mindblast-gameover" {
                state.step_mindblast_presentation(tick);
                Value::Null
            } else {
                result_snapshot(&state.step(
                    &NativeSecondaryTickContext { enhanced_effects: None, players: &players, tick },
                    &mut world,
                    Some(&mut order),
                ))
            };
            if samples.peek().is_some_and(|sample| sample["tick"].as_u64() == Some(tick)) {
                let expected = samples.next().unwrap();
                let label = format!("{mode} {tick}");
                assert_current(&state_snapshot(&state), &expected["state"], &label);
                assert_current(&outbox, &expected["outbox"], &label);
            }
        }
    }
}
