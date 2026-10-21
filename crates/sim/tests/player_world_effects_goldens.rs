//! Actual Website death crossings, cosmetic weapon bounces and bounded level-up particles.
use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::appearance::equipment::PlayerWeaponAppearance;
use solomon_sim::combat::PlayerCombatState;
use solomon_sim::player_presentation::*;
use solomon_sim::spells::casting::NativePlayerWeaponKind;

fn number(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap()
}
fn integer(value: &Value, key: &str) -> u64 {
    value[key].as_u64().unwrap()
}
fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap()
}
fn point(value: &Value) -> Vec2 {
    Vec2::new(number(value, "x"), number(value, "y"))
}
fn trig_equal(actual: f64, expected: f64) {
    // V8 and Rust may differ in the final bit of trigonometry and pow.
    assert!((actual - expected).abs() <= 1.0e-10, "{actual} != {expected}");
}

#[test]
fn player_death_and_level_up_world_effects_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-world-effects-goldens.json"))
            .unwrap();
    for case in fixture["bursts"].as_array().unwrap() {
        let source = &case["trigger"];
        let trigger = PlayerDeathBurstTrigger {
            death_epoch: integer(source, "deathEpoch"),
            player_id: text(source, "playerId").into(),
            run_id: text(source, "runId").into(),
            position: point(&source["position"]),
        };
        let age = if case["startedPresentationTick"].is_number() {
            number(case, "presentationTick") - number(case, "startedPresentationTick")
        } else {
            number(case, "age")
        };
        let actual = player_death_burst_layers(&trigger, age);
        let expected = case["layers"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.alpha, number(expected, "alpha"));
            assert_eq!(
                actual.scale,
                Vec2::new(number(expected, "scaleX"), number(expected, "scaleY"))
            );
            let offset = point(&expected["offset"]);
            trig_equal(actual.offset.x, offset.x);
            trig_equal(actual.offset.y, offset.y);
        }
    }
    for case in fixture["weapons"].as_array().unwrap() {
        let source = &case["trigger"];
        let trigger = PlayerDeathWeaponTrigger {
            death_epoch: integer(source, "deathEpoch"),
            heading_index: integer(source, "headingIndex") as usize,
            player_id: text(source, "playerId").into(),
            run_id: text(source, "runId").into(),
            weapon: PlayerWeaponAppearance {
                kind: if text(&source["weapon"], "kind") == "staff" {
                    NativePlayerWeaponKind::Staff
                } else {
                    NativePlayerWeaponKind::Wand
                },
                selector: integer(&source["weapon"], "selector") as usize,
            },
        };
        let actual = player_death_weapon_sample(&trigger, integer(case, "age"));
        let expected = &case["sample"];
        assert_eq!(actual.height, number(expected, "height"));
        assert_eq!(actual.settled, expected["settled"].as_bool().unwrap());
        trig_equal(actual.offset.x, point(&expected["offset"]).x);
        trig_equal(actual.offset.y, point(&expected["offset"]).y);
        trig_equal(actual.rotation_radians, number(expected, "rotationRadians"));
    }
    for case in fixture["levelUps"].as_array().unwrap() {
        let actual = native_level_up_presentation_frame(
            integer(case, "presentationId"),
            number(case, "elapsedMs"),
            number(case, "playerScreenY"),
        );
        let expected = &case["frame"];
        assert_eq!(actual.tick, integer(expected, "tick"));
        assert_eq!(actual.emitting, expected["emitting"].as_bool().unwrap());
        trig_equal(actual.light_radius, number(expected, "lightRadius"));
        let particles = expected["particles"].as_array().unwrap();
        assert_eq!(actual.particles.len(), particles.len());
        for (actual, expected) in actual.particles.iter().zip(particles) {
            assert_eq!(actual.offset_x, number(expected, "offsetX"));
            assert_eq!(actual.offset_y, number(expected, "offsetY"));
            trig_equal(actual.alpha, number(expected, "alpha"));
            trig_equal(actual.rotation_radians, number(expected, "rotationRadians"));
            trig_equal(actual.scale, number(expected, "scale"));
        }
    }
    let mut tracker = PlayerDeathBurstCrossingTracker::new("run", &[]);
    for case in fixture["crossings"].as_array().unwrap() {
        let players = case["players"].as_array().unwrap();
        let combat: Vec<_> = players
            .iter()
            .map(|player| PlayerCombatState {
                death_epoch: integer(&player["progression"], "deathEpoch"),
                death_tick: integer(&player["progression"], "deathTick"),
                ..PlayerCombatState::new()
            })
            .collect();
        let observations: Vec<_> = players
            .iter()
            .zip(&combat)
            .map(|(player, combat)| PlayerDeathObservation {
                player_id: text(player, "id"),
                combat,
                position: point(&player["position"]),
            })
            .collect();
        let actual = tracker.update(text(case, "runId"), &observations);
        let expected = case["triggers"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.death_epoch, integer(expected, "deathEpoch"));
            assert_eq!(actual.player_id, text(expected, "playerId"));
            assert_eq!(actual.run_id, text(expected, "runId"));
            assert_eq!(actual.position, point(&expected["position"]));
        }
    }
}

#[test]
fn level_up_admission_and_wall_clock_replay_the_source_renderer() {
    use solomon_sim::NativeRng;
    use solomon_sim::player::PlayerState;
    use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};

    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/level-up-lifecycle-goldens.json"))
            .unwrap();
    let mut rng = NativeRng::new(91);
    let mut player = PlayerState::new(
        NewGameSelection {
            name: "hero".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        &mut rng,
    );
    player.grant_experience(1000.0, &mut rng);
    let offer =
        player.progression.pending_offer.clone().expect("experience created an actual offer");
    let mut tracker = NativeLevelUpPresentationTracker::default();
    for case in fixture["cases"].as_array().unwrap() {
        let input = &case["input"];
        player.progression.pending_offer =
            if input["pendingOffer"].as_bool().unwrap() { Some(offer.clone()) } else { None };
        let frame = tracker.sample(
            text(input, "worldKey"),
            &player,
            input["barrierId"].as_u64(),
            number(input, "frameAtMs"),
            number(input, "playerScreenY"),
        );
        let output = &case["output"];
        if output.is_null() {
            assert!(frame.is_none());
            assert_eq!(tracker.presentation_id(), None);
            continue;
        }
        let frame = frame.expect("source retained its level-up presentation");
        assert_eq!(tracker.presentation_id(), Some(integer(output, "id")));
        assert_eq!(frame.tick, integer(output, "tick"));
        assert_eq!(frame.emitting, output["emitting"].as_bool().unwrap());
        assert_eq!(frame.particles.len(), integer(output, "particleCount") as usize);
        trig_equal(frame.light_radius, number(output, "lightRadius"));
        let radius = native_level_up_player_light_radius(number(input, "overlayPhase"), &frame);
        match output["playerLightRadius"].as_f64() {
            Some(expected) => trig_equal(radius.expect("active local emitter"), expected),
            None => assert_eq!(radius, None),
        }
    }
}
