//! Constructor and light samples from current Source, including the shared RNG tail.
use serde_json::{Value, json};
use solomon_sim::boneyard::lighting::NativeBoneyardLightSource;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::spells::weld::*;
use solomon_sim::{NativeRng, Vec2};

#[path = "support/website_rows.rs"]
mod website_rows;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn coordinates(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}
fn registration(value: PainterRegistration) -> Value {
    json!({"managerLane":match value.manager_lane {
        WorldManagerLane::Actor => "actor", WorldManagerLane::Transient => "transient",
    },"registrationOrdinal":value.registration_ordinal})
}
fn base(actor: &NativeWeldActorBase) -> Value {
    let owner = &actor.owner;
    json!({"ageTicks":owner.age_ticks,"birthTick":actor.birth_tick,"buildId":actor.build_id,
        "direction":coordinates(actor.direction),"id":owner.id,"origin":coordinates(actor.origin),
        "ownerId":owner.owner_id,"vector":actor.vector,"worldKey":owner.world_key,
        "lightRegistration":owner.light_registration.map(registration),
        "painterRegistrations":owner.painter_registrations.iter().copied().map(registration).collect::<Vec<_>>()})
}
fn fade_row(actor: &NativeWeldFlameLashFadeState) -> Value {
    let mut row = base(&actor.common);
    let NativeFlameFadeVisual::NativeBirth(birth) = actor.flame_visual else {
        panic!("expected native birth")
    };
    let payload = json!({"kind":"weld-flame-lash-fade","alpha":actor.alpha,"alphaStep":actor.alpha_step,
        "baseScale":actor.base_scale,"colorGreen":actor.color_green,
        "flameVisual":{"kind":"native-birth-v1","birthAlpha":birth.birth_alpha},
        "position":coordinates(actor.position),"record":actor.record,"rotationDegrees":actor.rotation_degrees,
        "variant":match actor.variant {
            NativeWeldFlameLashFadeVariant::Source=>"source",NativeWeldFlameLashFadeVariant::Endpoint=>"endpoint",
            NativeWeldFlameLashFadeVariant::Chain=>"chain",
        },"wrapperScalar":actor.wrapper_scalar});
    row.as_object_mut().unwrap().extend(payload.as_object().unwrap().clone());
    row
}
fn channel_row(actor: &NativeWeldChannelActorState) -> Value {
    let mut row = base(&actor.common);
    let Some(NativeFlameChannelVisual::NativeBirth(birth)) = &actor.flame_visual else {
        panic!("expected native birth")
    };
    let ribbon = match birth.ribbon {
        NativeFlameRibbonBirth::Short { width_factor } => {
            json!({"kind":"short","widthFactor":width_factor})
        }
        NativeFlameRibbonBirth::Long { phase_degrees } => {
            json!({"kind":"long","phaseDegrees":phase_degrees})
        }
    };
    let payload = json!({"kind":"weld-channel","endpoint":actor.endpoint.map(coordinates),
        "midpoint":actor.midpoint.map(coordinates),"targetId":actor.target_id,"underpowered":actor.underpowered,
        "variant":actor.variant,"enhancedEffects":actor.enhanced_effects,
        "flameVisual":{"kind":"native-birth-v1","role":match birth.role {
            NativeFlameChannelRole::Main=>"main",NativeFlameChannelRole::Chain=>"chain",
        },"cameraDisplacement":birth.camera_displacement.map(coordinates),"start":coordinates(birth.start),
        "control":coordinates(birth.control),"end":coordinates(birth.end),"offset":coordinates(birth.offset),
        "width":birth.width,"ribbon":ribbon,"path":{"intensity":birth.path.intensity,
        "radiusFactors":birth.path.radius_factors,"creatorRegistration":registration(birth.path.creator_registration),
        "miscAppendOrdinal":birth.path.misc_append_ordinal}}});
    row.as_object_mut().unwrap().extend(payload.as_object().unwrap().clone());
    row
}
fn light_row(light: NativeBoneyardLightSource) -> Value {
    json!({"position":coordinates(light.position),"intensity":light.intensity,"radius":light.radius,
        "castsDirectionalShadow":light.casts_directional_shadow})
}
fn assert_tail(rng: &NativeRng, order: WorldPainterOrder, expected: &Value) {
    website_rows::compare(
        &json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":rng.words.as_slice()}),
        &expected["rng"],
        "Flame constructor RNG tail",
    );
    website_rows::compare(
        &json!({"nextRegistrationOrdinal":{"actor":order.next_actor_ordinal,
        "transient":order.next_transient_ordinal}}),
        &expected["order"],
        "Flame constructor manager order",
    );
}
fn birth<'a>(input: &Value, vector: &'a [f64]) -> NativeWeldActorBirth<'a> {
    NativeWeldActorBirth {
        build_id: 1003,
        direction: point(&input["direction"]),
        id: input["id"].as_u64().unwrap(),
        origin: point(&input["origin"]),
        owner_id: "flame-owner",
        tick: input["tick"].as_u64().unwrap(),
        vector,
        world_key: "boneyard:flame",
    }
}

#[test]
fn current_flame_factory_births_preserve_native_words_roots_and_light_samples() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/flame-birth-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = &case["input"];
        let channel = &input["channel"];
        let vector: Vec<f64> =
            channel["vector"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut order = WorldPainterOrder { next_actor_ordinal: 300, next_transient_ordinal: 900 };
        let enhanced_effects = channel["enhancedEffects"].as_bool().unwrap();
        let result = create_native_flame_lash_factory(
            NativeFlameLashFactoryBirthInput {
                channel: NativeWeldChannelInput {
                    birth: birth(channel, &vector),
                    endpoint: Some(point(&channel["endpoint"])),
                    midpoint: Some(point(&channel["midpoint"])),
                    target_id: Some("enemy:3".into()),
                    underpowered: channel["underpowered"].as_bool().unwrap(),
                    enhanced_effects,
                    flame_visual: None,
                },
                source_fade_id: 72,
                geometry: NativeFlameLashFactoryInput {
                    start: point(&input["geometry"]["start"]),
                    control: point(&input["geometry"]["control"]),
                    end: point(&input["geometry"]["end"]),
                    offset: point(&input["geometry"]["offset"]),
                },
                role: if input["role"] == "main" {
                    NativeFlameChannelRole::Main
                } else {
                    NativeFlameChannelRole::Chain
                },
                camera_displacement: input["cameraDisplacement"]
                    .as_object()
                    .map(|_| point(&input["cameraDisplacement"])),
                width: input["width"].as_f64().unwrap(),
                creator_registration: PainterRegistration {
                    manager_lane: WorldManagerLane::Actor,
                    registration_ordinal: 48,
                },
                misc_append_ordinal: input["miscAppendOrdinal"].as_u64().unwrap() as usize,
            },
            &mut rng,
            &mut order,
        )
        .unwrap();
        website_rows::compare(
            &channel_row(&result.channel),
            &case["channel"],
            "Flame ribbon birth",
        );
        website_rows::compare(
            &fade_row(&result.source),
            &case["source"],
            "Flame source flare birth",
        );
        let Some(NativeFlameChannelVisual::NativeBirth(visual)) = &result.channel.flame_visual
        else {
            unreachable!()
        };
        let lights: Vec<_> = native_flame_lash_path_lights(visual, enhanced_effects)
            .unwrap()
            .into_iter()
            .map(light_row)
            .collect();
        website_rows::compare(&json!(lights), &case["lights"], "Flame retained path samples");
        assert_tail(&rng, order, case);
    }
    for case in fixture["fades"].as_array().unwrap() {
        let input = &case["input"];
        let vector: Vec<f64> =
            input["vector"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let mut rng = NativeRng::new(731);
        let mut order = WorldPainterOrder { next_actor_ordinal: 300, next_transient_ordinal: 900 };
        let actor = create_native_weld_flame_lash_fade(
            NativeWeldFlameLashFadeInput {
                birth: birth(input, &vector),
                alpha: input["alpha"].as_f64().unwrap(),
                variant: match input["variant"].as_str().unwrap() {
                    "source" => NativeWeldFlameLashFadeVariant::Source,
                    "endpoint" => NativeWeldFlameLashFadeVariant::Endpoint,
                    _ => NativeWeldFlameLashFadeVariant::Chain,
                },
                enhanced_effects: input["enhancedEffects"].as_bool().unwrap(),
            },
            &mut rng,
            Some(&mut order),
        );
        let mut actor = NativeWeldWorldActor::FlameLashFade(actor);
        for frame in case["frames"].as_array().unwrap() {
            let NativeWeldWorldActor::FlameLashFade(fade) = &actor else { unreachable!() };
            website_rows::compare(&fade_row(fade), &frame["actor"], "Flame fade epoch");
            website_rows::compare(
                &native_flame_lash_fade_light(fade).map(light_row).unwrap_or(Value::Null),
                &frame["light"],
                "Flame fade wrapper light",
            );
            let alive = step_native_weld_world_actor(&mut actor, &mut rng, &|_, _, _| true).alive;
            assert_eq!(alive, frame != case["frames"].as_array().unwrap().last().unwrap());
        }
        assert_tail(&rng, order, case);
    }
}
