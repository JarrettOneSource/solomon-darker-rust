//! Actual current Source ribbon/flare plans and clipped Region band membership.
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::spells::weld::*;

#[path = "support/website_rows.rs"]
mod website_rows;
#[path = "support/weld_visual_rows.rs"]
mod weld_visual_rows;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn bounds(value: &Value) -> NativeFlameLashBounds {
    NativeFlameLashBounds {
        x: value["x"].as_f64().unwrap(),
        y: value["y"].as_f64().unwrap(),
        width: value["width"].as_f64().unwrap(),
        height: value["height"].as_f64().unwrap(),
    }
}
fn bounds_row(value: NativeFlameLashBounds) -> Value {
    json!({"x":value.x,"y":value.y,"width":value.width,"height":value.height})
}
fn common(state: &Value) -> NativeWeldActorBase {
    let vector: Vec<_> =
        state["vector"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    NativeWeldActorBirth {
        build_id: 1003,
        direction: point(&state["direction"]),
        id: state["id"].as_u64().unwrap(),
        origin: point(&state["origin"]),
        owner_id: state["ownerId"].as_str().unwrap(),
        tick: state["birthTick"].as_u64().unwrap(),
        vector: &vector,
        world_key: state["worldKey"].as_str().unwrap(),
    }
    .actor()
}
fn channel_visual(value: &Value) -> NativeFlameChannelVisual {
    if value["kind"] == "legacy-suppressed" {
        return NativeFlameChannelVisual::LegacySuppressed;
    }
    let path = &value["path"];
    NativeFlameChannelVisual::NativeBirth(NativeFlameChannelBirth {
        role: if value["role"] == "main" {
            NativeFlameChannelRole::Main
        } else {
            NativeFlameChannelRole::Chain
        },
        camera_displacement: value["cameraDisplacement"]
            .as_object()
            .map(|_| point(&value["cameraDisplacement"])),
        start: point(&value["start"]),
        control: point(&value["control"]),
        end: point(&value["end"]),
        offset: point(&value["offset"]),
        width: value["width"].as_f64().unwrap(),
        ribbon: if value["ribbon"]["kind"] == "short" {
            NativeFlameRibbonBirth::Short {
                width_factor: value["ribbon"]["widthFactor"].as_f64().unwrap(),
            }
        } else {
            NativeFlameRibbonBirth::Long {
                phase_degrees: value["ribbon"]["phaseDegrees"].as_f64().unwrap(),
            }
        },
        path: NativeFlamePathBirth {
            intensity: path["intensity"].as_f64().unwrap(),
            radius_factors: path["radiusFactors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect(),
            creator_registration: PainterRegistration {
                manager_lane: WorldManagerLane::Actor,
                registration_ordinal: path["creatorRegistration"]["registrationOrdinal"]
                    .as_u64()
                    .unwrap() as usize,
            },
            misc_append_ordinal: path["miscAppendOrdinal"].as_u64().unwrap() as usize,
        },
    })
}
fn actor(state: &Value) -> NativeWeldWorldActor {
    let common = common(state);
    if state["kind"] == "weld-channel" {
        return NativeWeldWorldActor::Channel(NativeWeldChannelActorState {
            common,
            enhanced_effects: state["enhancedEffects"].as_bool().unwrap(),
            flame_visual: Some(channel_visual(&state["flameVisual"])),
            endpoint: state["endpoint"].as_object().map(|_| point(&state["endpoint"])),
            midpoint: state["midpoint"].as_object().map(|_| point(&state["midpoint"])),
            target_id: state["targetId"].as_str().map(String::from),
            underpowered: state["underpowered"].as_bool().unwrap(),
            variant: state["variant"].as_u64().unwrap() as u32,
        });
    }
    NativeWeldWorldActor::FlameLashFade(NativeWeldFlameLashFadeState {
        common,
        alpha: state["alpha"].as_f64().unwrap(),
        alpha_step: state["alphaStep"].as_f64().unwrap(),
        base_scale: state["baseScale"].as_f64().unwrap(),
        color_green: state["colorGreen"].as_f64().unwrap(),
        flame_visual: if state["flameVisual"]["kind"] == "legacy-suppressed" {
            NativeFlameFadeVisual::LegacySuppressed
        } else {
            NativeFlameFadeVisual::NativeBirth(NativeFlameFadeBirth {
                birth_alpha: state["flameVisual"]["birthAlpha"].as_f64().unwrap(),
            })
        },
        position: point(&state["position"]),
        record: state["record"].as_u64().unwrap() as usize,
        rotation_degrees: state["rotationDegrees"].as_f64().unwrap(),
        variant: match state["variant"].as_str().unwrap() {
            "chain" => NativeWeldFlameLashFadeVariant::Chain,
            "source" => NativeWeldFlameLashFadeVariant::Source,
            "endpoint" => NativeWeldFlameLashFadeVariant::Endpoint,
            other => panic!("unexpected Flame fade {other}"),
        },
        wrapper_scalar: state["wrapperScalar"].as_f64().unwrap(),
    })
}

#[test]
fn current_flame_draws_keep_birth_geometry_native_colors_and_region_bands() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/flame-presentation-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let state = actor(&case["state"]);
        weld_visual_rows::compare(
            [NativeWeldPresentationState::World(&state)],
            weld_visual_rows::sample(case, 0).unwrap(),
            case["name"].as_str().unwrap(),
        );
    }
    for case in fixture["bands"].as_array().unwrap() {
        let bands = build_native_flame_lash_split_bands(
            "flame:1",
            bounds(&case["bounds"]),
            case["enhancedEffects"].as_bool().unwrap(),
            case["regionView"].as_object().map(|_| bounds(&case["regionView"])),
        )
        .unwrap();
        let rows: Vec<_> = bands
            .into_iter()
            .map(
                |band| json!({"id":band.id,"clip":bounds_row(band.clip),"painterY":band.painter_y}),
            )
            .collect();
        website_rows::compare(
            &json!(rows),
            &case["result"],
            "Region changes mask origin, never the padded progression count",
        );
    }
    for input in fixture["errors"].as_array().unwrap() {
        assert!(build_native_flame_lash_split_bands("flame:1", bounds(input), true, None).is_err());
    }
    assert_eq!(
        native_flame_region_clipped_band(
            NativeFlameLashBounds { x: 0.0, y: 10.0, width: 10_000.0, height: 25.0 },
            Some(NativeFlameLashBounds { x: 100.0, y: 20.0, width: 80.0, height: 100.0 })
        ),
        NativeFlameLashBounds { x: 100.0, y: 20.0, width: 80.0, height: 15.0 }
    );
}
