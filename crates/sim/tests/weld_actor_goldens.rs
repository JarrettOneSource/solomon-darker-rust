use serde_json::{Value, json};
use solomon_sim::spells::earth_debris::NativeWeldMeteorDebrisSeed;
use solomon_sim::spells::weld::*;
use solomon_sim::{NativeRng, Vec2};

#[path = "support/website_rows.rs"]
mod website_rows;

#[path = "support/weld_visual_rows.rs"]
mod weld_visual_rows;

#[path = "weld_actor_goldens/persistent.rs"]
mod persistent;

#[path = "weld_actor_goldens/hail.rs"]
mod hail;

fn point(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}

fn vector(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn random(rng: &NativeRng) -> Value {
    json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":rng.words.as_slice()})
}

fn common_row(common: &NativeWeldActorBase) -> Value {
    json!({
        "ageTicks":common.owner.age_ticks,"birthTick":common.birth_tick,"buildId":common.build_id,
        "direction":point(common.direction),"id":common.owner.id,"origin":point(common.origin),
        "ownerId":common.owner.owner_id,"vector":common.vector,"worldKey":common.owner.world_key,
        "lightRegistration":common.owner.light_registration.map(|registration| json!({
            "managerLane":"actor","registrationOrdinal":registration.registration_ordinal,
        })),
    })
}

fn marker_row(marker: &NativeWeldMeteorMarkerState) -> Value {
    let mut row = common_row(&marker.common);
    let fields = json!({
        "alpha":marker.alpha,"colorGreen":marker.color_green,"growthFactor":marker.growth_factor,
        "kind":"weld-meteor-marker","rotationDegrees":marker.rotation_degrees,"scale":marker.scale,
    });
    row.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    row
}

fn debris_row(debris: &NativeWeldMeteorDebrisSeed) -> Value {
    json!({
        "alpha":debris.alpha,"colorGreen":debris.color_green,"height":debris.height,"index":debris.index,
        "position":point(debris.position),"record":debris.record,"rotationDegrees":debris.rotation_degrees,
        "rotationStepDegrees":debris.rotation_step_degrees,"scale":debris.scale,
        "velocity":point(debris.velocity),"verticalVelocity":debris.vertical_velocity,
    })
}

fn meteor_row(actor: &NativeWeldMeteorActorState) -> Value {
    let mut row = common_row(&actor.common);
    let fields = json!({
        "landingPosition":point(actor.landing_position),"bodyScale":actor.body_scale,
        "cameraDisplacement":actor.camera_displacement.map(point),"damage":actor.damage,
        "debris":actor.debris.iter().map(debris_row).collect::<Vec<_>>(),
        "fallHeadingDegrees":actor.fall_heading_degrees,"fallHeight":actor.fall_height,"fallStep":actor.fall_step,
        "impactDue":actor.impact_due,"impactAgeTicks":actor.impact_age_ticks,
        "impactRadiusScalar":actor.impact_radius_scalar,"impactRotationDegrees":actor.impact_rotation_degrees,
        "impactSoundPitch":actor.impact_sound_pitch,"impactThrowFirePitch":actor.impact_throw_fire_pitch,
        "impactTicksRemaining":actor.impact_ticks_remaining,"kind":"weld-meteor",
        "phase":if actor.phase==NativeWeldMeteorPhase::Fall {"fall"} else {"impact"},
        "position":point(actor.position),"privateSeed":actor.private_seed,"pulseDue":actor.pulse_due,
        "pulseSequence":actor.pulse_sequence,"pulseTicksRemaining":actor.pulse_ticks_remaining,
        "underpowered":actor.underpowered,
    });
    row.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    row
}

fn steam_row(actor: &NativeWeldSteamActorState) -> Value {
    let mut row = common_row(&actor.common);
    let fields = json!({
        "alphaMultiplier":actor.alpha_multiplier,"blue":actor.blue,"colorRise":actor.color_rise,
        "contactDamage":actor.contact_damage,"contactDue":actor.contact_due,"contactEnabled":actor.contact_enabled,
        "contactTicksRemaining":actor.contact_ticks_remaining,"kind":"weld-steam","life":actor.life,
        "lifeLoss":actor.life_loss,"phase":actor.phase,"position":point(actor.position),
        "remainingDistance":actor.remaining_distance,"rotationDegrees":actor.rotation_degrees,"scale":actor.scale,
        "stretch":actor.stretch,"terminalPosition":point(actor.terminal_position),"tintFade":actor.tint_fade,
        "variant":if actor.variant==NativeWeldSteamVariant::Normal {"normal"} else {"over"},
        "velocity":point(actor.velocity),
    });
    row.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    row
}

fn compare_steam_actor(actual: &Value, expected: &Value, path: &str) {
    if expected.is_null() {
        assert_eq!(actual, expected, "{path}");
        return;
    }
    let actual_angle = actual["rotationDegrees"].as_f64().unwrap();
    let expected_angle = expected["rotationDegrees"].as_f64().unwrap();
    // Steam rotation is an unrounded atan2 result; all float32 particle state
    // and random words below still use exact equality.
    assert!((actual_angle - expected_angle).abs() < 1e-9, "{path}: rotation");
    let mut expected = expected.clone();
    expected["rotationDegrees"] = actual["rotationDegrees"].clone();
    website_rows::compare(actual, &expected, path);
}

fn steam_birth<'a>(input: &Value, values: &'a [f64]) -> NativeWeldActorBirth<'a> {
    NativeWeldActorBirth {
        build_id: 1005,
        direction: vector(&input["direction"]),
        id: input["id"].as_u64().unwrap(),
        origin: vector(&input["origin"]),
        owner_id: "hero",
        tick: input["tick"].as_u64().unwrap(),
        vector: values,
        world_key: "boneyard:weld",
    }
}

fn values(input: &Value) -> Vec<f64> {
    input["vector"].as_array().unwrap().iter().map(|value| value.as_f64().unwrap()).collect()
}

#[test]
fn steam_native_update_gate_obstruction_contacts_and_private_fragment_fan_match_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/weld-actor-goldens.json")).unwrap();
    for case in fixture["steamCases"].as_array().unwrap() {
        let input = &case["input"];
        let values = values(input);
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let obstruction = |start: Vec2, end: Vec2| match case["obstruction"].as_str().unwrap() {
            "none" => None,
            "before" => Some(start),
            "ahead" => Some(Vec2::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0)),
            _ => unreachable!(),
        };
        let mut actor = spawn_native_weld_steam_actor(
            SpawnNativeWeldSteamInput {
                birth: steam_birth(input, &values),
                damage: input["damage"].as_f64().unwrap(),
                query_origin: vector(&input["queryOrigin"]),
                underpowered: input["underpowered"].as_bool().unwrap(),
                obstruction_point: &obstruction,
            },
            &mut rng,
        )
        .unwrap();
        website_rows::compare(&random(&rng), &case["rng"], "steam birth rng");
        let mut alive = true;
        for tick in 0..=700 {
            if tick > 0 {
                alive = step_native_weld_steam_actor(&mut actor);
            }
            if let Some(expected) =
                case["frames"].as_array().unwrap().iter().find(|frame| frame["tick"] == tick)
            {
                compare_steam_actor(
                    &json!(alive.then(|| steam_row(&actor))),
                    &expected["actor"],
                    &format!("steam {} {} tick{tick}", case["seed"], case["obstruction"]),
                );
                if let Some(sample) = weld_visual_rows::sample(case, tick) {
                    let actor = NativeWeldWorldActor::Steam(actor.clone());
                    weld_visual_rows::compare(
                        alive.then_some(NativeWeldPresentationState::World(actor.view())),
                        sample,
                        "Steam compositor",
                    );
                }
            }
            if !alive {
                break;
            }
        }
    }
    let input = &fixture["oddSteam"]["input"];
    let values = values(input);
    let mut rng = NativeRng::new(9);
    let actor = spawn_native_weld_steam_actor(
        SpawnNativeWeldSteamInput {
            birth: steam_birth(input, &values),
            damage: input["damage"].as_f64().unwrap(),
            query_origin: vector(&input["queryOrigin"]),
            underpowered: false,
            obstruction_point: &|_, _| None,
        },
        &mut rng,
    );
    website_rows::compare(
        &json!({"actor":actor.as_ref().map(steam_row),"rng":random(&rng)}),
        &fixture["oddSteam"]["result"],
        "odd Steam tick",
    );
    for case in fixture["steamDetonations"].as_array().unwrap() {
        let input = &case["input"];
        let mut rng = NativeRng::new(17);
        let result = create_native_weld_steam_detonation(
            NativeWeldSteamDetonationInput {
                explode_damage: input["explodeDamage"].as_f64().unwrap(),
                explode_radius: input["explodeRadius"].as_f64().unwrap(),
                fragment_count: input["fragmentCount"].as_u64().unwrap() as u32,
                fragment_damage: input["fragmentDamage"].as_f64().unwrap(),
                first_fragment_id: 500,
                origin: vector(&input["origin"]),
                owner_id: "hero",
                private_seed: 731,
                tick: 51,
                world_key: "boneyard:weld",
            },
            &mut rng,
        );
        let explosion = result.explosion.as_ref().map(|steam| {
            let explosion = &steam.explosion;
            json!({"burnDamage":explosion.burn_damage,"damage":explosion.damage,
                "footprintDimension":explosion.footprint_dimension,"origin":point(explosion.origin),
                "ownerId":explosion.owner_id,"presentation":"steam","soundPitch":steam.sound_pitch,
                "visualScale":explosion.visual_scale,"worldKey":explosion.world_key})
        });
        let mut actual = json!({"explosion":explosion,"fragments":result.fragments.iter().map(steam_row).collect::<Vec<_>>(),
            "nextId":result.next_id,"rng":random(&rng)});
        let mut expected = case["result"].clone();
        let actual_fragments = actual["fragments"].as_array().unwrap();
        let expected_fragments = expected["fragments"].as_array().unwrap();
        assert_eq!(actual_fragments.len(), expected_fragments.len());
        for (actual, expected) in actual_fragments.iter().zip(expected_fragments) {
            compare_steam_actor(actual, expected, "Steam fragment");
        }
        actual["fragments"] = Value::Null;
        expected["fragments"] = Value::Null;
        website_rows::compare(&actual, &expected, "Steam detonation");
    }
}

#[test]
fn meteor_marker_fall_impact_pulses_and_retirement_match_the_native_program() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/weld-actor-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = &case["birth"];
        let values: Vec<f64> = input["vector"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect();
        let birth = NativeWeldActorBirth {
            build_id: 1007,
            direction: vector(&input["direction"]),
            id: input["id"].as_u64().unwrap(),
            origin: vector(&input["origin"]),
            owner_id: "hero",
            tick: input["tick"].as_u64().unwrap(),
            vector: &values,
            world_key: "boneyard:weld",
        };
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut marker = spawn_native_weld_meteor_marker(birth, &mut rng);
        for frame in case["markerFrames"].as_array().unwrap() {
            let alive = frame["tick"] == 0 || step_native_weld_meteor_marker(&mut marker);
            website_rows::compare(
                &json!(alive.then(|| marker_row(&marker))),
                &frame["marker"],
                "marker",
            );
            let marker_samples = json!({"visuals":case["markerVisuals"]});
            if let Some(sample) =
                weld_visual_rows::sample(&marker_samples, frame["tick"].as_u64().unwrap())
            {
                let actor = NativeWeldWorldActor::MeteorMarker(marker.clone());
                weld_visual_rows::compare(
                    alive.then_some(NativeWeldPresentationState::World(actor.view())),
                    sample,
                    "Meteor marker compositor",
                );
            }
        }
        let underpowered = case["underpowered"].as_bool().unwrap();
        let spawn = create_native_weld_meteor_spawn_program(
            birth.direction,
            birth.origin,
            &|candidate| Vec2::new(candidate.x.max(50.0), candidate.y.min(250.0)),
            underpowered,
            &values,
            &mut rng,
        );
        let actual = json!({
            "bodyScale":spawn.body_scale,"fallHeadingDegrees":spawn.fall_heading_degrees,
            "fallHeight":spawn.fall_height,"fallStep":spawn.fall_step,"impactTicks":spawn.impact_ticks,
            "position":point(spawn.position),"privateSeed":spawn.private_seed,"rng":random(&rng),
        });
        website_rows::compare(&actual, &case["spawn"], "spawn");
        let mut actor = create_native_weld_meteor(
            birth,
            spawn,
            case["damage"].as_f64().unwrap(),
            underpowered,
            None,
        );
        let mut alive = true;
        for tick in 0..=700 {
            let mut debris = Vec::new();
            if tick > 0 {
                (alive, debris) = step_native_weld_meteor(&mut actor, &mut rng);
            }
            if let Some(expected) =
                case["frames"].as_array().unwrap().iter().find(|frame| frame["tick"] == tick)
            {
                let mut actual = json!({"tick":tick,"actor":alive.then(||meteor_row(&actor)),"rng":random(&rng)});
                if tick > 0 {
                    actual["debris"] = json!(debris.iter().map(debris_row).collect::<Vec<_>>());
                }
                website_rows::compare(&actual, expected, &format!("{} tick{tick}", case["name"]));
                if let Some(sample) = weld_visual_rows::sample(case, tick) {
                    let actor = NativeWeldWorldActor::Meteor(actor.clone());
                    weld_visual_rows::compare(
                        alive.then_some(NativeWeldPresentationState::World(actor.view())),
                        sample,
                        "Meteor compositor",
                    );
                }
            }
            if !alive {
                break;
            }
        }
    }
}
