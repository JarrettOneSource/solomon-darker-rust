use serde_json::{Value, json};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::spells::weld::flame_state::{NativeFlameChannelVisual, NativeFlameFadeVisual};
use solomon_sim::spells::weld::*;
use solomon_sim::{NativeRng, Vec2};

#[path = "support/website_rows.rs"]
mod website_rows;

#[path = "support/weld_visual_rows.rs"]
mod weld_visual_rows;

fn point(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}
fn vector(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn random(rng: &NativeRng) -> Value {
    json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":rng.words.as_slice()})
}
fn registration(value: PainterRegistration) -> Value {
    json!({"managerLane":if value.manager_lane == WorldManagerLane::Actor {"actor"} else {"transient"},
        "registrationOrdinal":value.registration_ordinal})
}

fn row(actor: &NativeWeldWorldActor) -> Value {
    let common = actor.common();
    let mut row = json!({"ageTicks":common.owner.age_ticks,"birthTick":common.birth_tick,"buildId":common.build_id,
        "direction":point(common.direction),"id":common.owner.id,"lightRegistration":common.owner.light_registration.map(registration),"origin":point(common.origin),
        "ownerId":common.owner.owner_id,"vector":common.vector,"worldKey":common.owner.world_key});
    if !common.owner.painter_registrations.is_empty() {
        row["painterRegistrations"] = json!(
            common
                .owner
                .painter_registrations
                .iter()
                .copied()
                .map(registration)
                .collect::<Vec<_>>()
        );
    }
    let fields = match actor {
        NativeWeldWorldActor::Channel(actor) => {
            let mut fields = json!({"kind":"weld-channel","endpoint":actor.endpoint.map(point),
            "enhancedEffects":actor.enhanced_effects,
            "midpoint":actor.midpoint.map(point),"targetId":actor.target_id,"underpowered":actor.underpowered,"variant":actor.variant});
            if let Some(visual) = &actor.flame_visual {
                assert_eq!(
                    *visual,
                    NativeFlameChannelVisual::LegacySuppressed,
                    "this constructor fixture supplies no native Flame birth sample"
                );
                fields["flameVisual"] = json!({"kind":"legacy-suppressed"});
            }
            fields
        }
        NativeWeldWorldActor::FlameLashFade(actor) => {
            let visual = match actor.flame_visual {
                NativeFlameFadeVisual::LegacySuppressed => json!({"kind":"legacy-suppressed"}),
                NativeFlameFadeVisual::NativeBirth(birth) => {
                    json!({"kind":"native-birth-v1","birthAlpha":birth.birth_alpha})
                }
            };
            json!({"kind":"weld-flame-lash-fade","alpha":actor.alpha,
            "alphaStep":actor.alpha_step,"baseScale":actor.base_scale,"colorGreen":actor.color_green,
            "flameVisual":visual,
            "position":point(actor.position),"record":actor.record,"rotationDegrees":actor.rotation_degrees,
            "variant":if actor.variant==NativeWeldFlameLashFadeVariant::Chain {"chain"} else {"endpoint"},"wrapperScalar":actor.wrapper_scalar})
        }
        NativeWeldWorldActor::Impact(actor) => json!({"kind":"weld-impact","alpha":actor.alpha,
            "boulderTerminalCharge":actor.boulder_terminal_charge,"impactSoundPitch":actor.impact_sound_pitch,
            "impactSoundVariant":actor.impact_sound_variant,"position":point(actor.position),
            "presentationRotationDegrees":actor.presentation_rotation_degrees,"presentationScale":actor.presentation_scale}),
        NativeWeldWorldActor::BlizzardGlow(actor) => {
            json!({"kind":"weld-blizzard-glow","rotationDegrees":actor.rotation_degrees,
            "scale":actor.scale,"variant":actor.variant})
        }
        NativeWeldWorldActor::BlizzardChainFrost(_) => json!({"kind":"weld-blizzard-chain-frost"}),
        NativeWeldWorldActor::FrostFade(actor) => {
            json!({"kind":"weld-frost-fade","scale":actor.scale})
        }
        _ => panic!("this fixture records channel and impact epochs"),
    };
    row.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    row
}

#[test]
fn weld_channels_native_fades_and_blizzard_contact_geometry_match_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/weld-effects-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = &case["input"];
        let values: Vec<f64> =
            input["vector"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let birth = NativeWeldActorBirth {
            build_id: match input["family"].as_str().unwrap() {
                "lash" => 1003,
                "glows" | "chain" => 1004,
                _ => input["buildId"].as_u64().unwrap() as u32,
            },
            direction: vector(&input["direction"]),
            id: 300,
            origin: vector(&input["origin"]),
            owner_id: "hero",
            tick: input["tick"].as_u64().unwrap(),
            vector: &values,
            world_key: "boneyard:weld",
        };
        let mut rng = NativeRng::new(input["seed"].as_i64().unwrap_or(17));
        let mut order = WorldPainterOrder { next_actor_ordinal: 500, next_transient_ordinal: 70 };
        let mut actors = match input["family"].as_str().unwrap() {
            "lash" => {
                vec![NativeWeldWorldActor::FlameLashFade(create_native_weld_flame_lash_fade(
                    NativeWeldFlameLashFadeInput {
                        birth,
                        alpha: input["alpha"].as_f64().unwrap(),
                        variant: if input["variant"] == "chain" {
                            NativeWeldFlameLashFadeVariant::Chain
                        } else {
                            NativeWeldFlameLashFadeVariant::Endpoint
                        },
                        enhanced_effects: true,
                    },
                    &mut rng,
                    None,
                ))]
            }
            "impact" => vec![NativeWeldWorldActor::Impact(create_primary_spell_weld_impact(
                birth, &mut rng,
            ))],
            "glows" => {
                let glows = create_native_weld_blizzard_source_glows(birth, &mut rng);
                let mut actors: Vec<_> =
                    glows.into_iter().map(NativeWeldWorldActor::BlizzardGlow).collect();
                actors.push(NativeWeldWorldActor::BlizzardGlow(
                    create_native_weld_blizzard_contact_glow(
                        NativeWeldActorBirth { id: birth.id + 2, ..birth },
                        &mut rng,
                        &mut order,
                    ),
                ));
                actors
            }
            "chain" => {
                let result = create_native_weld_blizzard_chain_effects(
                    birth,
                    vector(&input["castDirection"]),
                    &mut rng,
                    &mut order,
                );
                let mut actors = vec![NativeWeldWorldActor::FrostFade(result.fade)];
                if let Some(chain) = result.chain_frost {
                    actors.push(NativeWeldWorldActor::BlizzardChainFrost(chain));
                }
                actors
            }
            "channel" => vec![NativeWeldWorldActor::Channel(create_native_weld_channel_actor(
                NativeWeldChannelInput {
                    birth,
                    endpoint: Some(vector(&input["endpoint"])),
                    midpoint: Some(vector(&input["midpoint"])),
                    target_id: input["targetId"].as_str().map(str::to_owned),
                    underpowered: input["underpowered"].as_bool().unwrap(),
                    enhanced_effects: true,
                    flame_visual: None,
                },
            ))],
            _ => unreachable!(),
        };
        for tick in 0..=50 {
            if tick > 0 {
                actors.retain_mut(|actor| {
                    step_native_weld_world_actor(actor, &mut rng, &|_, _, _| true).alive
                });
            }
            if let Some(expected) =
                case["frames"].as_array().unwrap().iter().find(|frame| frame["tick"] == tick)
            {
                let actual = json!({"tick":tick,"actors":actors.iter().map(row).collect::<Vec<_>>(),"rng":random(&rng)});
                website_rows::compare(&actual, expected, &format!("{} tick{tick}", case["name"]));
                if let Some(sample) = weld_visual_rows::sample(case, tick) {
                    weld_visual_rows::compare(
                        actors.iter().map(NativeWeldPresentationState::World),
                        sample,
                        "Weld channel and fade compositor",
                    );
                }
            }
            if actors.is_empty() {
                break;
            }
        }
        assert_eq!(order.next_actor_ordinal, case["order"]["actor"].as_u64().unwrap() as usize);
    }
    for case in fixture["geometry"].as_array().unwrap() {
        let input = &case["input"];
        let endpoint = vector(&input["endpoint"]);
        let source = vector(&input["source"]);
        let underpowered = input["underpowered"].as_bool().unwrap();
        let widen = input["widen"].as_f64().unwrap();
        let beam = native_weld_blizzard_beam_plan(
            input["birthTick"].as_u64().unwrap(),
            endpoint,
            source,
            underpowered,
            widen,
        );
        let contact = native_weld_blizzard_contact_polygon(endpoint, source, underpowered, widen);
        let actual = json!({"jitter":point(beam.jitter),"quads":beam.quads.map(|quad|json!({"record":quad.record,"vertices":quad.vertices})),
            "tint":beam.tint,"width":beam.width});
        website_rows::compare(&actual, &case["beam"], "Blizzard beam");
        website_rows::compare(
            &json!({"beamWidth":contact.beam_width,"halfWidth":contact.half_width,
            "points":contact.points.map(point)}),
            &case["contact"],
            "Blizzard contact polygon",
        );
    }
}
