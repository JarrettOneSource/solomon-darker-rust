//! The admission controller replays the pinned Website before/after the existing Hub
//! character commit. Static/dynamic actor collision is owned by the enclosing world.
use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::hub::HubWizard;
use solomon_sim::hub::college_intro::*;
use solomon_sim::hub::regions::{HubRegion, HubTransition, HubTransitionPhase};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/college-intro-goldens.json")).unwrap()
}
fn number(value: &Value) -> f64 {
    value.as_f64().expect("fixture number")
}
fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value[0]), number(&value[1]))
}
fn same_number(actual: f64, expected: f64, name: &str) {
    assert_eq!(actual, expected, "{name}");
}
fn same_point(actual: Vec2, expected: &Value, name: &str) {
    let expected = point(expected);
    same_number(actual.x, expected.x, name);
    same_number(actual.y, expected.y, name);
}
fn phase(value: &Value) -> NativeCollegeIntroPhase {
    match value.as_str().unwrap() {
        "courtyard-walk" => NativeCollegeIntroPhase::CourtyardWalk,
        "office-walk" => NativeCollegeIntroPhase::OfficeWalk,
        "arch-dialogue" => NativeCollegeIntroPhase::ArchDialogue,
        other => panic!("fixture phase {other}"),
    }
}
fn intro(value: &Value) -> Option<NativeCollegeIntroState> {
    if value.is_null() {
        return None;
    }
    Some(NativeCollegeIntroState {
        contact_counter: value["contactCounter"].as_u64().unwrap() as u32,
        cover_alpha: number(&value["coverAlpha"]),
        dialogue_sequence: value["dialogueSequence"].as_u64().unwrap(),
        office_speed: number(&value["officeSpeed"]),
        path_cursor: number(&value["pathCursor"]),
        phase: phase(&value["phase"]),
        title_cursor: number(&value["titleCursor"]),
    })
}
fn transition_phase(value: &Value) -> HubTransitionPhase {
    match value.as_str().unwrap() {
        "college-intro" => HubTransitionPhase::CollegeIntro,
        "college-loadout" => HubTransitionPhase::CollegeLoadout,
        "outgoing" => HubTransitionPhase::Outgoing,
        "incoming" => HubTransitionPhase::Incoming,
        other => panic!("fixture transition {other}"),
    }
}
fn read_wizard(value: &Value) -> HubWizard {
    let mut wizard = HubWizard::spawn();
    wizard.region = HubRegion::from_id(value["region"].as_str().unwrap()).unwrap();
    if !value["transition"].is_null() {
        let value = &value["transition"];
        wizard.transition = Some(HubTransition {
            alpha: number(&value["alpha"]),
            destination: HubRegion::from_id(value["destination"].as_str().unwrap()).unwrap(),
            phase: transition_phase(&value["phase"]),
            scripted_speed: number(&value["scriptedSpeed"]),
            scripted_target: Vec2::new(
                number(&value["scriptedTarget"]["x"]),
                number(&value["scriptedTarget"]["y"]),
            ),
            source_region: HubRegion::from_id(value["sourceRegion"].as_str().unwrap()).unwrap(),
        });
    }
    wizard
}
fn same_intro(actual: Option<NativeCollegeIntroState>, expected: &Value) {
    let expected = intro(expected);
    assert_eq!(actual.is_some(), expected.is_some());
    if let (Some(actual), Some(expected)) = (actual, expected) {
        assert_eq!(actual.phase, expected.phase);
        assert_eq!(actual.contact_counter, expected.contact_counter);
        assert_eq!(actual.dialogue_sequence, expected.dialogue_sequence);
        for (actual, expected, name) in [
            (actual.cover_alpha, expected.cover_alpha, "cover"),
            (actual.office_speed, expected.office_speed, "office speed"),
            (actual.path_cursor, expected.path_cursor, "path cursor"),
            (actual.title_cursor, expected.title_cursor, "title cursor"),
        ] {
            same_number(actual, expected, name);
        }
    }
}

#[test]
fn paths_titles_contact_and_arm_operations_match_the_pinned_source() {
    let golden = fixture();
    for case in golden["paths"].as_array().unwrap() {
        let actual = native_college_path_target(
            phase(&case["phase"]),
            number(&case["cursor"]),
            point(&case["position"]),
        );
        same_number(actual.path_cursor, number(&case["pathCursor"]), "path cursor");
        same_point(actual.target, &case["target"], "path target");
        assert_eq!(
            native_college_path_heading_index(
                phase(&case["phase"]),
                number(&case["cursor"]),
                point(&case["position"])
            ),
            case["heading"].as_u64().unwrap() as usize
        );
    }
    for case in golden["titles"].as_array().unwrap() {
        let actual =
            native_college_title_presentation(number(&case["cursor"]), number(&case["cover"]));
        same_number(actual.alpha, number(&case["result"]["alpha"]), "title alpha");
        assert_eq!(actual.record, case["result"]["record"].as_u64().unwrap() as usize);
        same_number(actual.y, number(&case["result"]["y"]), "title y");
    }
    for case in golden["speeds"].as_array().unwrap() {
        same_number(
            native_college_office_speed(number(&case["cursor"]), number(&case["speed"])),
            number(&case["result"]),
            "office speed",
        );
    }
    for case in golden["contacts"].as_array().unwrap() {
        let actual = native_college_contact_step(
            case["counter"].as_u64().unwrap() as u32,
            case["eligible"].as_bool().unwrap(),
        );
        assert_eq!(actual.activate, case["result"]["activate"].as_bool().unwrap());
        assert_eq!(actual.counter, case["result"]["counter"].as_u64().unwrap() as u32);
    }
    for case in golden["predicates"].as_array().unwrap() {
        let wizard = read_wizard(&case["state"]);
        let intro = intro(&case["state"]["collegeIntro"]);
        let pending = case["pending"].as_bool().unwrap();
        assert_eq!(
            hub_college_intro_unstarted(intro.as_ref()),
            case["unstarted"].as_bool().unwrap()
        );
        assert_eq!(
            hub_college_admission_pre_loadout(&wizard, pending),
            case["preLoadout"].as_bool().unwrap()
        );
        assert_eq!(
            hub_college_admission_primary_unset(&wizard, intro.as_ref(), pending),
            case["primaryUnset"].as_bool().unwrap()
        );
        assert_eq!(
            hub_participant_onboarding_collision_bypass_active(&wizard, intro.as_ref(), pending),
            case["collisionBypass"].as_bool().unwrap()
        );
    }
    for case in golden["arms"].as_array().unwrap() {
        let wizard = read_wizard(&case["state"]);
        let intro = intro(&case["state"]["collegeIntro"]);
        let pending = case["pending"].as_bool().unwrap();
        let actual = plan_hub_college_intro_arm(
            &wizard,
            intro.as_ref(),
            pending,
            case["seed"].as_u64().unwrap() as u32,
            case["selectedPrimaryId"].as_i64().unwrap() as i32,
        );
        assert_eq!(actual.is_some(), pending);
        if let Some(actual) = actual {
            assert_eq!(actual.reset_primary_cast, case["reset"].as_bool().unwrap());
            assert_eq!(actual.spawn.is_some(), case["begin"].as_bool().unwrap());
            if let Some(spawn) = actual.spawn {
                same_point(spawn.position, &case["spawn"]["position"], "arm spawn");
                assert_eq!(
                    spawn.heading_index,
                    case["spawn"]["heading"].as_u64().unwrap() as usize
                );
            }
            assert_eq!(
                actual.appearance.primary_tint,
                case["appearance"]["primaryTint"].as_u64().unwrap() as u32
            );
            assert_eq!(
                actual.appearance.secondary_tint,
                case["appearance"]["secondaryTint"].as_u64().unwrap() as u32
            );
        }
    }
}

#[test]
fn readiness_walk_dialogue_loadout_and_return_share_one_source_epoch() {
    let golden = fixture();
    let journey = &golden["journey"];
    let mut wizard = HubWizard::at(NATIVE_COLLEGE_COURTYARD_PATH[0]);
    wizard.heading_index = native_college_path_heading_index(
        NativeCollegeIntroPhase::CourtyardWalk,
        0.0,
        wizard.position,
    );
    let mut intro = Some(NativeCollegeIntroState::new());
    let mut pending = true;
    let mut exiting = false;
    let mut actions = journey["actions"].as_array().unwrap().iter().peekable();
    let mut samples = journey["samples"].as_array().unwrap().iter().peekable();
    let completed = journey["completedTick"].as_u64().unwrap();
    for tick in 0..=completed {
        if actions.peek().is_some_and(|action| action[0].as_u64().unwrap() == tick) {
            match actions.next().unwrap()[1].as_str().unwrap() {
                "acknowledge" => {
                    assert!(acknowledge_hub_college_intro_dialogue(&mut intro));
                    exiting = true;
                }
                "confirm" => assert!(confirm_hub_college_intro_loadout(&mut wizard)),
                other => panic!("fixture action {other}"),
            }
        }
        let ready = tick >= 12;
        let movement = plan_hub_participant_movement(&wizard, intro.as_ref(), ready);
        let plan = movement.plan.unwrap_or_else(|| {
            wizard.movement_plan(if exiting { Vec2::new(0.0, 1.0) } else { Vec2::ZERO })
        });
        let position =
            Vec2::new(wizard.position.x + plan.delta.x, wizard.position.y + plan.delta.y);
        wizard.commit(&plan, position);
        let result = step_hub_participant_movement(
            &mut wizard,
            &mut intro,
            movement.college_target,
            HubParticipantStepOptions { college_intro_pending: pending, renderer_ready: ready },
        );
        if result.clear_college_intro_pending {
            pending = false;
        }
        if samples.peek().is_some_and(|sample| sample["tick"].as_u64().unwrap() == tick) {
            let sample = samples.next().unwrap();
            same_point(wizard.position, &sample["position"], "position");
            same_point(wizard.velocity, &sample["velocity"], "velocity");
            same_number(wizard.gait_degrees, number(&sample["gait"]), "gait");
            same_number(wizard.walk_cycle_primary, number(&sample["walk"]), "walk");
            assert_eq!(
                wizard.heading_index,
                sample["heading"].as_u64().unwrap() as usize,
                "tick {tick}"
            );
            let expected = read_wizard(&sample["participant"]);
            assert_eq!(wizard.region, expected.region, "tick {tick}");
            assert_eq!(wizard.transition.is_some(), expected.transition.is_some(), "tick {tick}");
            if let (Some(actual), Some(expected)) = (wizard.transition, expected.transition) {
                assert_eq!(actual.phase, expected.phase, "tick {tick}");
                assert_eq!(actual.source_region, expected.source_region);
                assert_eq!(actual.destination, expected.destination);
                same_number(actual.alpha, expected.alpha, "transition alpha");
                same_number(actual.scripted_speed, expected.scripted_speed, "transition speed");
                same_number(
                    actual.scripted_target.x,
                    expected.scripted_target.x,
                    "transition target",
                );
                same_number(
                    actual.scripted_target.y,
                    expected.scripted_target.y,
                    "transition target",
                );
            }
            same_intro(intro, &sample["participant"]["collegeIntro"]);
            assert_eq!(pending, sample["pending"].as_bool().unwrap(), "tick {tick}");
            assert_eq!(
                result.clear_college_intro_pending,
                sample["clearPending"].as_bool().unwrap()
            );
            assert_eq!(movement.plan.is_some(), !sample["plan"].is_null());
            if let Some(plan) = movement.plan {
                same_point(plan.delta, &sample["plan"]["delta"], "planned delta");
                same_point(
                    plan.requested_velocity,
                    &sample["plan"]["requested"],
                    "requested velocity",
                );
                same_point(
                    plan.retained_velocity,
                    &sample["plan"]["retained"],
                    "retained velocity",
                );
                assert_eq!(plan.movement_active, sample["plan"]["active"].as_bool().unwrap());
            }
        }
    }
    assert!(!pending && intro.is_none() && wizard.transition.is_none());
    assert!(samples.next().is_none());
}
