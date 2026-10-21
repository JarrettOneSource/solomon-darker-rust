//! Native Spider calls and Silk's generated spline state, force handoff and status rules.
use super::*;
use solomon_sim::enemies::model::*;
use solomon_sim::enemies::silk::*;
use solomon_sim::enemies::silk_force::*;
use solomon_sim::enemies::spider_movement::*;
use solomon_sim::enemies::spider_remains::*;
use solomon_sim::enemies::webbed::*;
use solomon_sim::enemies::world::EnemyWorld;
use solomon_sim::spline::{NaturalSpline, NaturalSplineAxis};

pub(super) fn natural_spline_json(spline: &NaturalSpline) -> Value {
    json!({"extent":spline.extent,"x":{"points":spline.x.points,"coefficients":spline.x.coefficients},
        "y":{"points":spline.y.points,"coefficients":spline.y.coefficients}})
}
fn spline(value: &Value) -> NaturalSpline {
    let axis = |value: &Value| NaturalSplineAxis {
        points: serde_json::from_value(value["points"].clone()).unwrap(),
        coefficients: serde_json::from_value(value["coefficients"].clone()).unwrap(),
    };
    NaturalSpline { extent: number(value, "extent"), x: axis(&value["x"]), y: axis(&value["y"]) }
}
pub(super) fn silk_state_json(state: &NativeSilkState) -> Value {
    json!({"forceAccumulator":state.force_accumulator,"ageTicks":state.age_ticks,"phase":state.phase,"ended":state.ended,
        "position":point_json(state.position),"center":natural_spline_json(&state.center),"wave":natural_spline_json(&state.wave),
        "height":state.height,"alpha":state.alpha,"waveScale":state.wave_scale,"drift":point_json(state.drift),"driftVelocity":point_json(state.drift_velocity),"cocoonHealth":state.cocoon_health})
}
fn silk_state(value: &Value) -> NativeSilkState {
    NativeSilkState {
        force_accumulator: number(value, "forceAccumulator"),
        age_ticks: value["ageTicks"].as_u64().unwrap(),
        phase: number(value, "phase"),
        ended: boolean(value, "ended"),
        position: point_from_json(&value["position"]),
        center: spline(&value["center"]),
        wave: spline(&value["wave"]),
        height: number(value, "height"),
        alpha: number(value, "alpha"),
        wave_scale: number(value, "waveScale"),
        drift: point_from_json(&value["drift"]),
        drift_velocity: point_from_json(&value["driftVelocity"]),
        cocoon_health: number(value, "cocoonHealth"),
    }
}
fn silk_creation_json(created: &NativeSilkCreation) -> Value {
    json!({"state":silk_state_json(&created.state),"soundIndex":created.sound_index,"soundPitch":created.sound_pitch,"soundPosition":point_json(created.sound_position)})
}
fn assert_silk_creation_equal(created: &NativeSilkCreation, expected: &Value, label: &str) {
    let mut actual = silk_creation_json(created);
    for axis in ["x", "y"] {
        assert_trig(
            number(&actual["soundPosition"], axis),
            number(&expected["soundPosition"], axis),
            label,
        );
        assert_trig(
            number(&actual["state"]["position"], axis),
            number(&expected["state"]["position"], axis),
            label,
        );
    }
    actual["soundPosition"] = expected["soundPosition"].clone();
    actual["state"]["position"] = expected["state"]["position"].clone();
    assert_json_equal(&actual, expected, label);
}
fn spider_state(value: &Value) -> SpiderBrain {
    SpiderBrain {
        action_state: integer(value, "actionState"),
        preferred_distance: number(value, "preferredDistance"),
        body_heading_deg: number(value, "bodyHeadingDeg"),
        move_ticks_remaining: value["moveTicksRemaining"].as_i64().unwrap(),
        strafe_sign: number(value, "strafeSign"),
        strafe_ticks_remaining: value["strafeTicksRemaining"].as_i64().unwrap(),
        action_ticks_remaining: value["actionTicksRemaining"].as_i64().unwrap(),
        frame: number(value, "frame"),
        spit_ticks_remaining: value["spitTicksRemaining"].as_u64().unwrap(),
        attached: boolean(value, "attached"),
        vertical_offset: number(value, "verticalOffset"),
        attachment_angle_deg: number(value, "attachmentAngleDeg"),
        attachment_distance: number(value, "attachmentDistance"),
        suck_ticks: value["suckTicks"].as_u64().unwrap(),
        phase: SpiderPhase::Active,
    }
}
fn assert_spider_sample(
    actual: &SpiderMovementResult,
    expected: &Value,
    rng: &solomon_sim::NativeRng,
    label: &str,
) {
    let mut state = brain_json(&EnemyBrain::Spider(actual.state.clone()));
    for field in ["bodyHeadingDeg", "attachmentAngleDeg", "attachmentDistance"] {
        assert_trig(number(&state, field), number(&expected["state"], field), label);
        state[field] = expected["state"][field].clone();
    }
    assert_json_equal(&state, &expected["state"], label);
    for (point, key) in [(actual.position, "position"), (actual.goal, "goal")] {
        assert_trig(point.x, number(&expected[key], "x"), label);
        assert_trig(point.y, number(&expected[key], "y"), label);
    }
    assert_trig(actual.heading_deg, number(expected, "headingDeg"), label);
    assert_eq!(actual.bite, boolean(expected, "bite"));
    assert_eq!(actual.suck, boolean(expected, "suck"));
    assert_json_equal(&json!(actual.bite_sounds), &expected["biteSounds"], label);
    assert_json_equal(
        &json!(actual.shared_spit_ticks_remaining),
        &expected["sharedSpitTicksRemaining"],
        label,
    );
    if let Some(created) = &actual.silk {
        assert_silk_creation_equal(created, &expected["silk"], &format!("{label} Silk birth"));
    } else {
        assert!(expected["silk"].is_null());
    }
    assert_json_equal(&json!(rng_record(rng)), &expected["rng"], &format!("{label} RNG"));
}
fn fade_line_json(state: &NativeFadeLineState) -> Value {
    json!({"start":point_json(state.start),"middle":point_json(state.middle),"end":point_json(state.end),"velocity":point_json(state.velocity),
        "opacity":state.opacity,"opacityLossPerTick":state.opacity_loss_per_tick})
}
pub(super) fn silk_actor_json(actor: &EnemySilkActor) -> Value {
    json!({"id":actor.id,"ownerActorId":actor.owner_actor_id,"spawnTick":actor.spawn_tick,"state":silk_state_json(&actor.state),
        "nativeRegistrationOrder":actor.native_registration_order,"nativeCellBindingOrder":actor.native_cell_binding_order,"painterRegistration":painter_registration_json(actor.painter_registration)})
}
fn webbed_json(state: Option<WebbedState>) -> Value {
    state.map_or(Value::Null,|state|json!({"severity":state.severity,"strength":state.strength,"cocoonHealth":state.cocoon_health,"hitPulse":state.hit_pulse}))
}
pub(super) fn dead_spider_json(state: &DeadSpiderState) -> Value {
    let decal=state.decal.as_ref().map(|decal|json!({"entry":decal.entry,"position":point_json(decal.position),"rotationDeg":decal.rotation_deg,
        "scale":decal.scale,"alpha":decal.alpha}));
    json!({"position":point_json(state.position),"headingDeg":state.heading_deg,"slideSpeed":point_json(state.slide_speed),"frame":state.frame,
        "life":state.life,"decalGrowth":state.decal_growth,"decal":decal})
}

#[test]
fn spider_movement_silk_splines_force_fragments_and_webbed_status_match_the_website() {
    let goldens: Value = serde_json::from_str(&fixture("enemy-spider-goldens.json")).unwrap();
    for (index, case) in goldens["silkCases"].as_array().unwrap().iter().enumerate() {
        let mut rng = solomon_sim::NativeRng::new(case["seed"].as_i64().unwrap());
        let created = create_native_silk(
            point_from_json(&case["position"]),
            point_from_json(&case["target"]["position"]),
            point_from_json(&case["target"]["velocityPerTick"]),
            45.0,
            &mut rng,
        );
        let mut expected = case["creation"].clone();
        expected.as_object_mut().unwrap().remove("rngState");
        assert_json_equal(&silk_creation_json(&created), &expected, &format!("Silk birth {index}"));
        assert_json_equal(&json!(rng_record(&rng)), &case["rng"], "Silk constructor RNG");
        let mut state = Some(created.state);
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for step in 0..=1800 {
            if step > 0 {
                state = state.and_then(step_native_silk);
            }
            if let Some(expected) =
                samples.peek().filter(|sample| sample["step"].as_u64() == Some(step))
            {
                assert_json_equal(
                    &state.as_ref().map_or(Value::Null, silk_state_json),
                    &expected["state"],
                    &format!("Silk {index} step {step}"),
                );
                samples.next();
            }
            if state.is_none() {
                break;
            }
        }
        assert!(samples.next().is_none(), "all Silk {index} samples consumed");
    }
    for case in goldens["movements"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut rng = solomon_sim::NativeRng::new(case["seed"].as_i64().unwrap());
        let mut state = spider_state(&case["initial"]);
        let mut position = Vec2::new(150.0, 250.0);
        let mut heading = 0.0;
        let mut shared = 0_u64;
        let value = &case["targetState"];
        let target = if value.is_null() {
            None
        } else {
            Some(SpiderTarget {
                position: point_from_json(&value["position"]),
                heading_deg: number(value, "headingDeg"),
                radius: number(value, "radius"),
                is_player: boolean(value, "isPlayer"),
                web_severity: number(value, "webSeverity"),
                velocity_per_tick: point_from_json(&value["velocityPerTick"]),
            })
        };
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for step in 0..300 {
            shared = shared.saturating_sub(1);
            let mut route =
                |goal| SpiderRouteGoal { goal, following_route: true, turn_around: step % 50 == 0 };
            let result = move_native_spider(
                &state,
                SpiderMovementRequest {
                    route_goal: if case["route"].as_bool() == Some(true) {
                        Some(&mut route)
                    } else {
                        None
                    },
                    heading_deg: heading,
                    position,
                    speed: 3.25,
                    status_factor: 0.75,
                    light: number(case, "light"),
                    target,
                    wander_heading_deg: 210.0,
                    spit_webs: true,
                    cocoon_health: 45.0,
                    live_silk_count: 0,
                    shared_spit_ticks_remaining: shared,
                },
                &mut rng,
            );
            if let Some(expected) =
                samples.peek().filter(|sample| sample["step"].as_u64() == Some(step))
            {
                assert_spider_sample(&result, expected, &rng, &format!("{name} step {step}"));
                samples.next();
            }
            state = result.state;
            position = result.goal;
            heading = solomon_sim::enemies::movement::positive_modulo(result.heading_deg, 360.0);
            shared = result.shared_spit_ticks_remaining;
        }
        assert!(samples.next().is_none());
        assert_json_equal(&json!(rng_record(&rng)), &case["rng"], &format!("{name} final RNG"));
        assert_spider_sample(
            &SpiderMovementResult {
                state,
                heading_deg: heading,
                position,
                goal: position,
                bite: false,
                suck: false,
                silk: None,
                bite_sounds: None,
                shared_spit_ticks_remaining: shared,
            },
            &json!({"state":case["finalState"],"headingDeg":case["finalHeadingDeg"],"position":case["finalPosition"],"goal":case["finalPosition"],"bite":false,"suck":false,
                "silk":null,"biteSounds":null,"sharedSpitTicksRemaining":shared,"rng":case["rng"]}),
            &rng,
            &format!("{name} final state"),
        );
    }
    for case in goldens["forces"].as_array().unwrap() {
        let mode = case["lightMode"].as_str().unwrap();
        let mut world = EnemyWorld::new(&format!("silk-force:{mode}"), 50);
        world.silks.push(EnemySilkActor {
            id: 44,
            owner_actor_id: 1,
            spawn_tick: 0,
            state: silk_state(&case["initial"]),
            native_registration_order: 50,
            native_cell_binding_order: 50,
            painter_registration: solomon_sim::painter::PainterRegistration {
                manager_lane: solomon_sim::painter::WorldManagerLane::Actor,
                registration_ordinal: 17,
            },
        });
        let mut rng = solomon_sim::NativeRng::new(777);
        let queries = std::cell::RefCell::new(Vec::new());
        let light_at = |position: Vec2| {
            let light = match mode {
                "bright" => 0.8,
                "dark" => 0.0,
                _ => {
                    if position.x >= 100.0 {
                        0.5
                    } else {
                        0.0
                    }
                }
            };
            queries.borrow_mut().push(json!({"position":point_json(position),"light":light}));
            light
        };
        for sample in case["samples"].as_array().unwrap() {
            let tick = sample["tick"].as_u64().unwrap();
            if tick <= 103 {
                world.apply_silk_force(
                    44,
                    [0.4, 0.6, 0.001, 2.0][(tick - 100) as usize],
                    Vec2::new(2.0, -1.0),
                    tick,
                    &mut rng,
                    &light_at,
                );
            } else {
                let previous = case["samples"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|sample| sample["tick"].as_u64())
                    .filter(|previous| *previous < tick)
                    .max()
                    .unwrap();
                for _ in previous + 1..=tick {
                    world.step_silk_fragments();
                }
            }
            let silks: Vec<_> = world.silks.iter().map(silk_actor_json).collect();
            assert_json_equal(&json!(silks), &sample["silks"], mode);
            let fragments:Vec<_>=world.silk_fragments.iter().map(|actor|json!({"id":actor.id,"spawnTick":actor.spawn_tick,"state":fade_line_json(&actor.state)})).collect();
            assert_json_equal(
                &json!(fragments),
                &sample["fragments"],
                &format!("{mode} fragments tick {tick}"),
            );
            assert_json_equal(&json!(world.next_death_effect_id), &sample["nextId"], mode);
            assert_json_equal(&json!(rng_record(&rng)), &sample["rng"], mode);
        }
        assert_json_equal(
            &json!(*queries.borrow()),
            &case["queries"],
            &format!("{mode} light queries"),
        );
    }
    let mut web = None;
    for sample in goldens["webSamples"].as_array().unwrap() {
        web = if sample["kind"] == "apply" {
            Some(apply_native_webbed(web, number(sample, "strength")))
        } else {
            web.and_then(|web| step_native_webbed(web, Vec2::new(1.0, 0.0)))
        };
        assert_json_equal(&webbed_json(web), &sample["state"], "webbed sequence");
        assert_json_equal(
            &json!(native_webbed_movement_scale(web.as_ref())),
            &sample["scale"],
            "webbed movement scale",
        );
    }
    for sample in goldens["cocoon"].as_array().unwrap() {
        let heading = number(sample, "heading");
        assert_json_equal(
            &point_json(native_cocoon_position(Vec2::new(100.0, 200.0), heading)),
            &sample["position"],
            "Cocoon position",
        );
        assert_json_equal(&point_json(native_cocoon_tilt(heading)), &sample["tilt"], "Cocoon tilt");
    }
    for case in goldens["remains"].as_array().unwrap() {
        let heading = number(case, "heading");
        let mut state = Some(create_native_dead_spider(Vec2::new(150.0, 250.0), heading));
        let mut rng = solomon_sim::NativeRng::new(731);
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for step in 0..=2100 {
            if step > 0 {
                state = state.and_then(|state| step_native_dead_spider(state, &mut rng));
            }
            if let Some(sample) =
                samples.peek().filter(|sample| sample["step"].as_u64() == Some(step))
            {
                assert_json_equal(
                    &state.as_ref().map_or(Value::Null, dead_spider_json),
                    &sample["state"],
                    &format!("dead Spider heading {heading} step {step}"),
                );
                assert_json_equal(
                    &json!(state.as_ref().and_then(native_dead_spider_entry)),
                    &sample["entry"],
                    "dead Spider record",
                );
                assert_json_equal(
                    &json!(rng_record(&rng)),
                    &sample["rng"],
                    "dead Spider decal RNG",
                );
                samples.next();
            }
            if state.is_none() {
                break;
            }
        }
        assert!(samples.next().is_none(), "all corpse samples consumed");
    }
}
