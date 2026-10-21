//! Logical enemy-spawn queries and renderer light sources share these source replays.

use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::boneyard::lighting::{
    NativeBoneyardLightIndex, NativeBoneyardLightManagerView, NativeBoneyardLightSource,
    NativePlayerLightOwner, native_lantern_light_source, native_missile_light_source,
    native_player_light_source,
};

fn number(value: &Value) -> f64 {
    value.as_f64().expect("recorded light number")
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}

fn source(value: &Value) -> NativeBoneyardLightSource {
    NativeBoneyardLightSource {
        position: point(&value["position"]),
        radius: number(&value["radius"]),
        intensity: number(&value["intensity"]),
        casts_directional_shadow: value["castsDirectionalShadow"].as_bool().unwrap(),
        raster_scale: value["rasterScale"].as_f64(),
    }
}

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/boneyard-light-goldens.json"))
        .expect("recorded Boneyard lighting")
}

#[test]
fn player_lantern_and_missile_lights_keep_native_pulse_and_rounding() {
    for case in fixture()["factories"].as_array().unwrap() {
        let tick = case["tick"].as_u64().unwrap();
        let kind = case["kind"].as_str().unwrap();
        let actual = match kind {
            "player" => {
                let player = &case["player"];
                native_player_light_source(
                    NativePlayerLightOwner {
                        id: player["id"].as_str().unwrap(),
                        position: point(&player["position"]),
                        heading_index: player["headingIndex"].as_u64().unwrap() as usize,
                        drive_active: false,
                        overlay_effect_phase: number(&player["lighting"]["overlayEffectPhase"]),
                    },
                    tick,
                    true,
                )
                .unwrap()
            }
            "lantern" => native_lantern_light_source(Vec2::new(100.0, 300.0), tick, true),
            "missile" => native_missile_light_source(17, Vec2::new(100.0, 300.0), tick, true),
            _ => panic!("unknown light fixture kind {kind}"),
        };
        let expected = source(&case["expected"]);
        if kind == "player" {
            // Player positions use platform trigonometry; every other field is exact.
            assert!((actual.position.x - expected.position.x).abs() <= 1e-9);
            assert!((actual.position.y - expected.position.y).abs() <= 1e-9);
        } else {
            assert_eq!(actual.position, expected.position);
        }
        assert_eq!(actual.intensity, expected.intensity, "{kind} tick {tick}");
        assert_eq!(actual.radius, expected.radius, "{kind} tick {tick}");
        assert_eq!(actual.raster_scale, expected.raster_scale, "{kind} tick {tick}");
        assert_eq!(actual.casts_directional_shadow, expected.casts_directional_shadow);
    }
}

#[test]
fn native_light_acceptance_grid_and_surface_queries_match_the_website() {
    let mut index = NativeBoneyardLightIndex::new(Vec2::new(2100.0, 3700.0));
    for case in fixture()["grids"].as_array().unwrap() {
        let view = &case["view"];
        let providers: Vec<_> = case["providers"].as_array().unwrap().iter().map(source).collect();
        let tail: Vec<_> = case["tail"].as_array().unwrap().iter().map(source).collect();
        index.rebuild(
            &providers,
            &tail,
            NativeBoneyardLightManagerView {
                camera_position: point(&view["camera"]),
                zoom: number(&view["camera"]["zoom"]),
                viewport: Vec2::new(
                    number(&view["viewport"]["width"]),
                    number(&view["viewport"]["height"]),
                ),
            },
            number(&case["quality"]),
        );
        let expected: Vec<_> = case["accepted"].as_array().unwrap().iter().map(source).collect();
        assert_eq!(index.accepted_sources(), expected, "accepted provider order");
        for query in case["queries"].as_array().unwrap() {
            let position = point(&query["position"]);
            let expected_indices: Vec<usize> = query["indices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(index.source_indices_at(position), expected_indices, "{position:?}");
            assert_eq!(index.scalar_at(position), number(&query["scalar"]), "{position:?}");
            assert_eq!(
                index.surface_scalar_at(position),
                number(&query["surface"]),
                "{position:?}"
            );
        }
    }
}
