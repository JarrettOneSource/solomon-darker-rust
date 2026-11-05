//! Real staff actions through marker crossings, ordered contacts and terminal VFX.

#[path = "support/staff.rs"]
mod support;

use serde_json::{Value, json};
use solomon_sim::NativeRng;
use solomon_sim::economy::HubEconomyState;
use solomon_sim::profile::runtime::*;
use solomon_sim::profile::{
    NativeSkillBookState, PlayerProgressionState, WizardDiscipline, WizardElement,
};
use solomon_sim::secondary::presentation::NativeSecondaryBlend;
use solomon_sim::spells::staff::presentation::*;
use solomon_sim::spells::staff::*;
use std::collections::BTreeMap;
use support::*;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/staff-action-goldens.json")).unwrap()
}

fn derived() -> PlayerSkillDerivedStats {
    let mut book = NativeSkillBookState::new(WizardElement::Air, WizardDiscipline::Arcane);
    let economy = HubEconomyState::new(77, WizardElement::Air);
    let runtime = PlayerSkillRuntimeState::new(&mut book, &economy);
    player_skill_derived_stats(&runtime, &book, &PlayerProgressionState::new(417), &economy)
}

#[test]
fn staff_birth_rng_marker_crossing_spin_and_both_pose_lanes_replay_the_source() {
    let fixture = fixture();
    for (index, case) in fixture["actions"].as_array().unwrap().iter().enumerate() {
        let input = &case["input"];
        let mut stats = derived();
        stats.staff_equipped = true;
        stats.staff_action_timing_factor = case["timing"].as_f64().unwrap();
        stats.flailing_chance_percent = case["chance"].as_f64().unwrap();
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let action = create_native_player_staff_action(
            NativePlayerStaffActionInput {
                derived: &stats,
                heading_degrees: input["headingDegrees"].as_f64().unwrap(),
                id: input["id"].as_u64().unwrap(),
                lane: lane(case["lane"].as_str().unwrap()),
                origin: point(&input["origin"]),
                owner_id: input["ownerId"].as_str().unwrap(),
                world_key: input["worldKey"].as_str().unwrap(),
            },
            &mut rng,
        );
        assert_snapshot(
            &json!({ "action": action_snapshot(&action), "rng": rng_snapshot(&rng) }),
            &case["birth"],
            &format!("action {index} birth"),
        );
        let mut active = Some(action);
        for (tick, sample) in case["steps"].as_array().unwrap().iter().enumerate() {
            let result = step_native_player_staff_action(
                active.as_ref().unwrap(),
                point(&sample["ownerPosition"]),
            );
            assert_snapshot(
                &json!({ "action": result.action.as_ref().map(action_snapshot),
                "contact": result.contact, "sample": action_snapshot(&result.sample) }),
                &sample["result"],
                &format!("action {index} tick {tick}"),
            );
            assert_eq!(
                native_player_staff_action_pose(&result.sample),
                sample["pose"].as_u64().unwrap() as usize
            );
            active = result.action;
        }
        assert!(active.is_none(), "action {index} retires on the source tick");
    }
}

#[test]
fn admission_polygons_arcs_and_damage_division_preserve_source_boundaries_and_order() {
    let fixture = fixture();
    let targets =
        fixture["geometryTargets"].as_array().unwrap().iter().map(read_target).collect::<Vec<_>>();
    for (index, case) in fixture["geometry"].as_array().unwrap().iter().enumerate() {
        let player = NativeStaffPlayer {
            collision_radius: case["player"]["collisionRadius"].as_f64().unwrap(),
            heading_degrees: case["player"]["headingDegrees"].as_f64().unwrap(),
            position: point(&case["player"]["position"]),
        };
        assert_snapshot(
            &json!(native_staff_admission_target(&player, &targets).map(|target| &target.id)),
            &case["admission"],
            "admission",
        );
        let physical = native_staff_physical_contact_targets(&player, &targets);
        assert_snapshot(
            &json!(physical.iter().map(|target| &target.id).collect::<Vec<_>>()),
            &case["physical"],
            "physical contact admission",
        );
        for damage in case["damage"].as_array().unwrap() {
            let action = read_action(&damage["action"]);
            let admitted = native_staff_damage_targets(&action, &targets);
            let knockback = native_staff_knockback_targets(&action, &targets);
            assert_snapshot(
                &json!(admitted.iter().map(|target| &target.id).collect::<Vec<_>>()),
                &damage["admitted"],
                &format!("geometry {index} damage"),
            );
            assert_snapshot(
                &json!(knockback.iter().map(|target| &target.id).collect::<Vec<_>>()),
                &damage["knockback"],
                &format!("geometry {index} knockback"),
            );
        }
    }
    for case in fixture["damage"].as_array().unwrap() {
        assert_eq!(
            native_staff_contact_damage_per_target(
                case["amount"].as_f64().unwrap(),
                case["count"].as_u64().unwrap() as usize,
                case["radial"].as_bool().unwrap()
            ),
            case["result"].as_f64().unwrap()
        );
    }
    for (index, case) in fixture["registrationQueries"].as_array().unwrap().iter().enumerate() {
        let action = read_action(&case["action"]);
        let targets =
            case["targets"].as_array().unwrap().iter().map(read_target).collect::<Vec<_>>();
        assert_snapshot(
            &json!(
                native_staff_knockback_targets(&action, &targets)
                    .iter()
                    .map(|target| &target.id)
                    .collect::<Vec<_>>()
            ),
            &case["targetIds"],
            &format!("knockback registration order {index}"),
        );
    }
}

#[test]
fn physical_impacts_keep_ether_pike_snapshot_and_fifty_word_advance_exact() {
    let fixture = fixture();
    let action = read_action(&fixture["physicalAction"]);
    let targets = fixture["physicalTargets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| NativeStaffPhysicalTarget {
            target: read_target(value),
            pike: value["pike"].as_bool().unwrap(),
        })
        .collect::<Vec<_>>();
    for (index, case) in fixture["physical"].as_array().unwrap().iter().enumerate() {
        let element = match case["element"].as_str().unwrap() {
            "air" => WizardElement::Air,
            "earth" => WizardElement::Earth,
            "ether" => WizardElement::Ether,
            "fire" => WizardElement::Fire,
            "water" => WizardElement::Water,
            other => panic!("unknown element {other}"),
        };
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let impacts = resolve_native_staff_physical_contacts(
            &action,
            &targets,
            element,
            case["chance"].as_f64().unwrap(),
            &mut rng,
        );
        assert_snapshot(
            &json!({ "impacts": impacts.iter().map(impact_snapshot).collect::<Vec<_>>(),
            "rng": rng_snapshot(&rng) }),
            &case["result"],
            &format!("physical case {index}"),
        );
    }
}

#[test]
fn every_staff_proc_sound_particle_and_contact_lifetime_replays_the_source() {
    let fixture = fixture();
    for (index, case) in fixture["presentations"].as_array().unwrap().iter().enumerate() {
        let action = read_action(&case["action"]);
        let target_ids = strings(&case["targetIds"]);
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let result = create_native_staff_contact_presentation(
            NativeStaffContactPresentationInput {
                first_id: 100,
                action: &action,
                target_ids: &target_ids,
                target_mean: solomon_sim::Vec2::new(144.125, -80.25),
                element_tint: 0x12abef,
                impact_sound_pitches: &[0.95, 1.075],
                pike_break_sound_indexes: &[1],
            },
            &mut rng,
        );
        assert_snapshot(
            &json!({ "event": event_snapshot(&result.event), "nextId": result.next_id,
            "rng": rng_snapshot(&rng), "vfx": result.vfx.iter().map(vfx_snapshot).collect::<Vec<_>>() }),
            &case["result"],
            &format!("presentation {index}"),
        );
        for journey in case["vfxJourneys"].as_array().unwrap() {
            let mut actor = Some(result.vfx[journey["index"].as_u64().unwrap() as usize].clone());
            for sample in journey["samples"].as_array().unwrap() {
                assert_snapshot(
                    &actor.as_ref().map(vfx_snapshot).unwrap_or(Value::Null),
                    sample,
                    &format!("presentation {index} particle lifetime"),
                );
                actor = actor.as_ref().and_then(step_native_player_staff_vfx);
            }
            assert!(actor.is_none());
        }
        let mut event = Some(result.event);
        for sample in case["eventJourney"].as_array().unwrap() {
            assert_snapshot(
                &event.as_ref().map(event_snapshot).unwrap_or(Value::Null),
                sample,
                &format!("presentation {index} event lifetime"),
            );
            event = event.as_ref().and_then(step_native_staff_contact_event);
        }
        assert!(event.is_none());
    }
}

#[test]
fn knockback_final_heading_words_missing_targets_and_short_contacts_replay_the_source() {
    let fixture = fixture();
    let mut positions = fixture["positions"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, value)| (id.clone(), point(value)))
        .collect::<BTreeMap<_, _>>();
    let target_ids = strings(&fixture["knockbackTargetIds"]);
    for (index, case) in fixture["knockbacks"].as_array().unwrap().iter().enumerate() {
        let action = read_action(&case["action"]);
        let mut actor = create_native_staff_knockback(
            300,
            &action,
            &target_ids,
            case["factor"].as_f64().unwrap(),
        );
        assert_snapshot(
            &actor.as_ref().map(knockback_snapshot).unwrap_or(Value::Null),
            &case["birth"],
            "knockback birth",
        );
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        for sample in case["samples"].as_array().unwrap() {
            let result =
                step_native_staff_knockback(actor.as_ref().unwrap(), &mut positions, &mut rng);
            assert_snapshot(
                &knockback_step_snapshot(&result, &rng),
                sample,
                &format!("knockback {index}"),
            );
            actor = result.actor;
        }
        assert!(actor.is_none());
    }
    let action = read_action(&fixture["physicalAction"]);
    for case in fixture["contactKnockbacks"].as_array().unwrap() {
        let mut actor = Some(create_native_staff_contact_knockback(
            400,
            &action,
            "pike",
            solomon_sim::Vec2::new(1.25, -5.5),
        ));
        assert_snapshot(
            &contact_knockback_snapshot(actor.as_ref().unwrap()),
            &case["birth"],
            "contact knockback birth",
        );
        for sample in case["samples"].as_array().unwrap() {
            let result = step_native_staff_contact_knockback(
                actor.as_ref().unwrap(),
                case["targetExists"].as_bool().unwrap(),
            );
            assert_snapshot(
                &json!({ "actor": result.actor.as_ref().map(contact_knockback_snapshot),
                "displacement": result.displacement.map(position) }),
                sample,
                "contact knockback tick",
            );
            actor = result.actor;
        }
        assert!(actor.is_none());
    }
    let target = read_target(&fixture["physicalTargets"][1]);
    let mut pike = Some(create_native_staff_pike_break_vfx(
        500,
        &action,
        &target,
        NativeRng::new(77),
        -812.5,
        true,
    ));
    for sample in fixture["pikeBreak"].as_array().unwrap() {
        assert_snapshot(
            &pike.as_ref().map(pike_snapshot).unwrap_or(Value::Null),
            sample,
            "pike break lifetime",
        );
        pike = pike.as_ref().and_then(step_native_staff_pike_break_vfx);
    }
    assert!(pike.is_none());
}

#[test]
fn staff_effect_plans_and_private_pike_bounces_match_every_source_draw() {
    let presentation: Value =
        serde_json::from_str(include_str!("fixtures/website/staff-presentation-goldens.json"))
            .unwrap();
    for (index, case) in presentation["effects"].as_array().unwrap().iter().enumerate() {
        let state = read_vfx(&case["actor"]);
        let plan = native_player_staff_vfx_render_plan(&state);
        assert_snapshot(
            &json!({ "alpha": plan.alpha, "blendMode": "add", "entry": plan.entry,
            "light": null, "position": position(plan.position), "rotationRadians": plan.rotation_radians,
            "scale": plan.scale, "tint": plan.tint }),
            &case["plan"],
            &format!("staff VFX {index}"),
        );
        assert_eq!(plan.blend, NativeSecondaryBlend::Add);
    }
    let initial_action = read_action(&fixture()["physicalAction"]);
    for (index, case) in presentation["pikes"].as_array().unwrap().iter().enumerate() {
        let value = &case["actor"];
        let mut rng = NativeRng::new(0);
        rng.index_a = value["presentationRng"]["indexA"].as_u64().unwrap() as usize;
        rng.index_b = value["presentationRng"]["indexB"].as_u64().unwrap() as usize;
        for (word, source) in
            rng.words.iter_mut().zip(value["presentationRng"]["words"].as_array().unwrap())
        {
            *word = source.as_u64().unwrap() as u32;
        }
        let original = rng.clone();
        let mut state = create_native_staff_pike_break_vfx(
            500,
            &initial_action,
            &NativeStaffTarget {
                id: value["targetId"].as_str().unwrap().into(),
                collision_radius: 0.0,
                position: point(&value["position"]),
                registration_order: None,
            },
            rng,
            value["headingDegrees"].as_f64().unwrap(),
            value["enhancedShadow"].as_bool().unwrap(),
        );
        state.common.age_ticks = value["ageTicks"].as_u64().unwrap();
        let mut draws = Vec::new();
        for draw in native_player_staff_pike_break_draws(&state) {
            draws.push(json!({ "alpha": draw.alpha, "blendMode": if draw.blend == NativeSecondaryBlend::Add {"add"} else {"normal"},
                "entry": draw.entry, "offset": position(draw.offset), "role": draw.role,
                "rotationRadians": draw.rotation_radians, "scaleX": draw.scale_x, "scaleY": draw.scale_y,
                "tint": draw.tint }));
        }
        assert_snapshot(&json!(draws), &case["draws"], &format!("pike plan {index}"));
        assert_eq!(
            state.presentation_rng, original,
            "drawing never mutates gameplay or saved private RNG"
        );
    }
}
