//! Source summon construction, Golem lifetime/combat and articulated stock geometry.

use serde_json::{Value, json};
use solomon_sim::secondary::golem::*;
use solomon_sim::secondary::leviathan::*;
use solomon_sim::{NativeRng, Vec2};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn point_snapshot(point: Vec2) -> Value {
    json!({"x":point.x,"y":point.y})
}
fn rng_snapshot(rng: &NativeRng) -> Value {
    json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":&rng.words[..]})
}
fn assert_snapshot(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual.as_f64().unwrap();
            let expected = expected.as_f64().unwrap();
            // Only unrounded trigonometric angles need the established Website tolerance.
            if path.ends_with("rotationRadians")
                || path.ends_with("orbitHeadingRadians")
                || path.ends_with("headingToTarget")
            {
                assert!((actual - expected).abs() <= 1e-9, "{path}: {actual} != {expected}");
            } else {
                assert_eq!(actual, expected, "{path}");
            }
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
fn leviathan_births_appendages_muzzles_and_active_lane_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-summon-goldens.json"))
            .unwrap();
    for case in fixture["births"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let birth = create_native_leviathan_birth(
            &mut rng,
            case["maximum"].as_f64().unwrap(),
            case["forced"].as_bool().unwrap(),
        );
        let mut appendages = Vec::new();
        for appendage in birth.appendages {
            appendages.push(
                json!({"bank":appendage.bank,"baseOffset":point_snapshot(appendage.base_offset),
                "countdown":appendage.countdown,"headingDegrees":appendage.heading_degrees,
                "spinDegrees":appendage.spin_degrees,"spinStepDegrees":appendage.spin_step_degrees,
                "spriteScale":appendage.sprite_scale}),
            );
        }
        assert_snapshot(
            &json!({"appendages":appendages,"maximumScale":birth.maximum_scale,
            "quantity":birth.quantity,"rotationRadians":birth.rotation_radians,"rng":rng_snapshot(&rng)}),
            &case["result"],
            "Leviathan birth",
        );
    }
    for case in fixture["scales"].as_array().unwrap() {
        let age = case["age"].as_u64().unwrap();
        assert_eq!(
            native_leviathan_current_scale(age),
            case["scale"].as_f64().unwrap(),
            "scale age {age}"
        );
        assert_eq!(u64::from(native_leviathan_phase(age)), case["phase"].as_u64().unwrap());
        assert_eq!(native_leviathan_active(age), case["active"].as_bool().unwrap());
    }
    for case in fixture["geometry"].as_array().unwrap() {
        let heading = case["heading"].as_f64().unwrap();
        let bank = case["bank"].as_u64().unwrap() as u32;
        let parent = point(&case["parent"]);
        let local = native_leviathan_appendage_local_root(
            point(&case["base"]),
            point(&case["recoil"]),
            heading,
            case["deployment"].as_f64().unwrap(),
        );
        assert_snapshot(
            &point_snapshot(native_leviathan_heading_vector(heading)),
            &case["vector"],
            "heading vector",
        );
        assert_eq!(
            native_leviathan_direction_frame(heading) as u64,
            case["frame"].as_u64().unwrap()
        );
        assert_eq!(
            native_leviathan_appendage_record(bank, heading) as u64,
            case["record"].as_u64().unwrap()
        );
        assert_snapshot(&point_snapshot(local), &case["local"], "appendage local");
        assert_snapshot(
            &point_snapshot(native_leviathan_appendage_presentation_root(
                parent,
                case["compositeScale"].as_f64().unwrap(),
                local,
            )),
            &case["presentation"],
            "appendage presentation",
        );
        assert_snapshot(
            &point_snapshot(native_leviathan_muzzle_position(
                parent,
                local,
                bank,
                heading,
                case["spriteScale"].as_f64().unwrap(),
            )),
            &case["muzzle"],
            "appendage muzzle",
        );
    }
    for case in fixture["lanes"].as_array().unwrap() {
        let origin = point(&case["origin"]);
        let target = point(&case["target"]);
        assert_snapshot(
            &json!(native_leviathan_heading_degrees(origin, target)),
            &case["headingToTarget"],
            "headingToTarget",
        );
        assert_eq!(
            native_leviathan_inside_target_lane(origin, case["heading"].as_f64().unwrap(), target),
            case["inside"].as_bool().unwrap(),
            "{case}"
        );
    }
}

struct World {
    wall: bool,
    origin: Vec2,
}
impl NativeGolemWorld for World {
    fn resolve_movement(&mut self, requested: Vec2) -> Vec2 {
        if self.wall && requested.x > 120.0 { self.origin } else { requested }
    }
    fn resolve_foot_target(&mut self, current: Vec2, requested: Vec2) -> Vec2 {
        if self.wall && requested.x > 120.0 { current } else { requested }
    }
}
fn read_actor(value: &Value, wall: bool) -> NativeGolemKernelActor {
    let state = &value["golem"];
    let position = point(&value["position"]);
    let rotation = value["rotationRadians"].as_f64().unwrap();
    let mut world = World { wall, origin: position };
    let mut golem = NativeSecondaryGolemState::new(
        position,
        rotation,
        NativeGolemBirthStats {
            health: state["maximumHealth"].as_f64().unwrap(),
            damage_maximum: state["damageMaximum"].as_f64().unwrap(),
            iron: state["iron"].as_bool().unwrap(),
            reflect_factor: state["reflectFactor"].as_f64().unwrap(),
            pose_variant: state["poseVariant"].as_u64().unwrap() as u32,
        },
        &mut world,
    );
    golem.action_duration_ticks = state["actionDurationTicks"].as_u64().unwrap();
    golem.action_tick = state["actionTick"].as_u64().unwrap();
    golem.circle_slow_ticks = state["circleSlowTicks"].as_u64().unwrap();
    golem.current_health = state["currentHealth"].as_f64().unwrap();
    golem.gait_tick = state["gaitTick"].as_u64().unwrap();
    golem.left_foot_progress = state["leftFootProgress"].as_f64().unwrap();
    golem.right_foot_progress = state["rightFootProgress"].as_f64().unwrap();
    golem.orbit_direction = state["orbitDirection"].as_f64().unwrap();
    golem.orbit_heading_radians = state["orbitHeadingRadians"].as_f64();
    golem.phase = match state["phase"].as_str().unwrap() {
        "active" => NativeGolemPhase::Active,
        "assembly" => NativeGolemPhase::Assembly,
        "attack" => NativeGolemPhase::Attack,
        "provoke" => NativeGolemPhase::Provoke,
        other => panic!("unknown Golem phase {other}"),
    };
    golem.provoke_roll_bound = state["provokeRollBound"].as_i64().unwrap() as i32;
    golem.target_poll_ticks_remaining = state["targetPollTicksRemaining"].as_i64().unwrap() as i32;
    NativeGolemKernelActor {
        age_ticks: value["ageTicks"].as_u64().unwrap(),
        damage_minimum: value["damageMinimum"].as_f64().unwrap(),
        golem,
        id: value["id"].as_u64().unwrap(),
        owner_id: value["ownerId"].as_str().unwrap().to_owned(),
        position,
        rotation_radians: rotation,
        target_id: value["targetId"].as_u64().map(|id| id as usize),
    }
}
fn actor_snapshot(actor: &NativeGolemKernelActor) -> Value {
    let golem = actor.golem;
    let phase = match golem.phase {
        NativeGolemPhase::Active => "active",
        NativeGolemPhase::Assembly => "assembly",
        NativeGolemPhase::Attack => "attack",
        NativeGolemPhase::Provoke => "provoke",
    };
    json!({"ageTicks":actor.age_ticks,"damageMinimum":actor.damage_minimum,"id":actor.id,"ownerId":actor.owner_id,
    "position":point_snapshot(actor.position),"rotationRadians":actor.rotation_radians,"targetId":actor.target_id,
    "golem":{
        "actionHeadingOffsetDegrees":golem.action_heading_offset_degrees,"actionDurationTicks":golem.action_duration_ticks,
        "actionTick":golem.action_tick,"circleSlowTicks":golem.circle_slow_ticks,"currentHealth":golem.current_health,
        "damageMaximum":golem.damage_maximum,"iron":golem.iron,"gaitTick":golem.gait_tick,
        "leftConnectorOffset":point_snapshot(golem.left_connector_offset),"leftFoot":point_snapshot(golem.left_foot),
        "leftFootBob":point_snapshot(golem.left_foot_bob),"leftFootNext":point_snapshot(golem.left_foot_next),
        "leftFootPrevious":point_snapshot(golem.left_foot_previous),"leftFootProgress":golem.left_foot_progress,
        "leftFootRotationDegrees":golem.left_foot_rotation_degrees,"leftLimbMode":golem.left_limb_mode,
        "maximumHealth":golem.maximum_health,"orbitDirection":golem.orbit_direction,"orbitHeadingRadians":golem.orbit_heading_radians,
        "phase":phase,"poseVariant":golem.pose_variant,"provokeRollBound":golem.provoke_roll_bound,"reflectFactor":golem.reflect_factor,
        "rightConnectorOffset":point_snapshot(golem.right_connector_offset),"rightFoot":point_snapshot(golem.right_foot),
        "rightFootBob":point_snapshot(golem.right_foot_bob),"rightFootNext":point_snapshot(golem.right_foot_next),
        "rightFootPrevious":point_snapshot(golem.right_foot_previous),"rightFootProgress":golem.right_foot_progress,
        "rightFootRotationDegrees":golem.right_foot_rotation_degrees,"rightLimbMode":golem.right_limb_mode,
        "targetPollTicksRemaining":golem.target_poll_ticks_remaining,
    }})
}

#[test]
fn golem_assembly_orbit_contacts_articulation_and_damage_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-summon-goldens.json"))
            .unwrap();
    for case in fixture["initialArticulations"].as_array().unwrap() {
        let actual = read_actor(&case["actor"], case["mode"].as_str() == Some("wall"));
        assert_snapshot(&actor_snapshot(&actual), &case["actor"], "initial articulation");
    }
    // Current attack contacts are replayed below against the fa08 Source fixture.
    for case in fixture["damages"].as_array().unwrap() {
        let actor = read_actor(&case["actor"], false);
        let damage = &case["damage"];
        let result = damage_native_secondary_golem(
            actor,
            NativeGolemIncomingDamage {
                primary_damage: damage["primaryDamage"].as_f64().unwrap(),
                secondary_damage: damage["secondaryDamage"].as_f64().unwrap(),
                reflectable_physical_source_in_range: damage["reflectablePhysicalSourceInRange"]
                    .as_bool()
                    .unwrap(),
            },
        );
        assert_snapshot(
            &json!({"actor":result.actor.as_ref().map(actor_snapshot),"ignored":result.ignored,
            "killed":result.killed,"reflectedDamage":result.reflected_damage}),
            &case["result"],
            "Golem damage",
        );
    }
    for case in fixture["deathRng"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let _ = solomon_sim::death_animations::create_native_golem_death_animation(
            &mut rng,
            Vec2::ZERO,
            true,
        );
        assert_snapshot(&rng_snapshot(&rng), &case["result"], "Golem death stream");
    }
}

#[test]
fn every_secondary_actor_preserves_native_painter_and_light_disposition() {
    use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};
    use solomon_sim::secondary::{
        NativeSecondaryActorKind, NativeSecondaryActorState, NativeSecondaryLightDisposition,
        enroll_native_secondary_painter_owners, native_secondary_light_disposition,
        native_secondary_painter_manager_lane,
    };
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-summon-goldens.json"))
            .unwrap();
    let mut actors = Vec::new();
    for case in fixture["painterCases"].as_array().unwrap() {
        let kind = NativeSecondaryActorKind::ALL
            .into_iter()
            .find(|kind| Some(kind.as_str()) == case["kind"].as_str())
            .unwrap();
        let mut actor = NativeSecondaryActorState::new(
            case["id"].as_u64().unwrap(),
            kind,
            "player".to_owned(),
            "boneyard:oracle".to_owned(),
            None,
        );
        actor.variant = case["variant"].as_i64().unwrap() as i32;
        actors.push(actor);
    }
    let mut order = WorldPainterOrder { next_actor_ordinal: 49, next_transient_ordinal: 10 };
    enroll_native_secondary_painter_owners(&mut actors, &mut order);
    for (actor, case) in actors.iter().zip(fixture["painterCases"].as_array().unwrap()) {
        let lane = match native_secondary_painter_manager_lane(actor.kind) {
            WorldManagerLane::Actor => "actor",
            WorldManagerLane::Transient => "transient",
        };
        assert_eq!(Some(lane), case["lane"].as_str());
        let disposition = match native_secondary_light_disposition(actor) {
            NativeSecondaryLightDisposition::ActorProvider => "actor-provider",
            NativeSecondaryLightDisposition::Misc => "misc",
            NativeSecondaryLightDisposition::None => "none",
            NativeSecondaryLightDisposition::TransientProvider => "transient-provider",
        };
        assert_eq!(Some(disposition), case["disposition"].as_str());
        let registration = actor.painter_registrations[0];
        assert_snapshot(
            &json!({"managerLane":lane,"registrationOrdinal":registration.registration_ordinal}),
            &case["painterRegistrations"][0],
            "secondary registration",
        );
    }
    let snapshot = |order: WorldPainterOrder| json!({"nextRegistrationOrdinal":{"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}});
    assert_snapshot(&snapshot(order), &fixture["nextPainterOrder"], "secondary manager order");
    enroll_native_secondary_painter_owners(&mut actors, &mut order);
    assert_snapshot(
        &snapshot(order),
        &fixture["repeatedPainterOrder"],
        "retained secondary manager order",
    );
}

#[path = "support/mod.rs"]
mod exact_support;
fn replay_golem_journeys(fixture: &Value) {
    for journey in fixture["journeys"].as_array().unwrap() {
        let mut actor = read_actor(&journey["initial"], journey["mode"].as_str() == Some("wall"));
        assert_snapshot(&actor_snapshot(&actor), &journey["initial"], "initial Golem");
        let mut world =
            World { wall: journey["mode"].as_str() == Some("wall"), origin: actor.position };
        let mut rng = NativeRng::new(journey["seed"].as_i64().unwrap());
        let mut samples = journey["records"].as_array().unwrap().iter().peekable();
        let mut tick = 0;
        for epoch in journey["epochs"].as_array().unwrap() {
            let owner = epoch["owner"].as_object().map(|_| point(&epoch["owner"]));
            let mut targets = Vec::new();
            for target in epoch["targets"].as_array().unwrap() {
                targets.push(NativeGolemKernelTarget {
                    id: target["id"].as_u64().unwrap() as usize,
                    position: point(&target["position"]),
                    radius: target["radius"].as_f64().unwrap(),
                    registration_order: target["registrationOrder"]
                        .as_u64()
                        .map(|order| order as usize),
                });
            }
            for _ in 0..epoch["ticks"].as_u64().unwrap() {
                tick += 1;
                world.origin = actor.position;
                let result =
                    step_native_secondary_golem(&mut actor, owner, &targets, &mut world, &mut rng);
                if samples.peek().is_none_or(|sample| sample["tick"].as_u64() != Some(tick)) {
                    continue;
                }
                let sample = samples.next().unwrap();
                let contact=result.contact.as_ref().map(|contact|json!({"damage":contact.damage,"headingDegrees":contact.heading_degrees,"movementBudget":contact.movement_budget,"origin":point_snapshot(contact.origin),"targetIds":contact.target_ids}));
                let actual = json!({"tick":tick,"actor":actor_snapshot(&actor),"assemblyMilestone":result.assembly_milestone,
                    "contact":contact,"footstep":result.footstep,"provokeStarted":result.provoke_started,"rng":rng_snapshot(&rng)});
                let path = format!("{} tick {tick}", journey["name"]);
                exact_support::assert_website_json(&actual, sample, &path, &[]);
            }
        }
        assert!(samples.next().is_none());
    }
}
#[test]
fn current_golem_attack_origin_cone_and_area_budget_replay_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-golem-contact-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    replay_golem_journeys(&fixture);
}
