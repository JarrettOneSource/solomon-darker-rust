use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::boneyard::lighting::{
    NativeBoneyardLightIndex, NativeBoneyardLightManagerView, NativeBoneyardLightSource,
    native_region_light_target_plan,
};
use solomon_sim::boneyard::shadows::{
    NativeShadowCaster, NativeShadowEdge, NativeShadowProgram, NativeShadowRecord,
    native_shadow_edges, native_shadow_records,
};

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}
fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}

fn caster(value: &Value) -> NativeShadowCaster {
    let program = &value["program"];
    let program = if program.is_null() {
        None
    } else {
        let start = point(&program["start"]);
        let end = point(&program["end"]);
        Some(match program["kind"].as_str().unwrap() {
            "fence-grate" if program["construction"] == "broken" => {
                NativeShadowProgram::BrokenFenceGrate {
                    start,
                    end,
                    step: point(&program["step"]),
                    count: program["count"].as_u64().unwrap() as usize,
                }
            }
            "fence-grate" => NativeShadowProgram::FenceGrate {
                start,
                end,
                gate: program["construction"] == "gate",
            },
            "rails" => NativeShadowProgram::Rails { start, end },
            "wall" => NativeShadowProgram::Wall { start, end },
            _ => unreachable!(),
        })
    };
    NativeShadowCaster {
        id: value["id"].as_str().unwrap().into(),
        position: point(&value["position"]),
        outline: value["outline"].as_array().unwrap().iter().map(point).collect(),
        program,
    }
}

fn record(value: &Value) -> NativeShadowRecord {
    NativeShadowRecord {
        base_alpha: number(&value["baseAlpha"]),
        behind_scalar: number(&value["behindScalar"]),
        direction: point(&value["direction"]),
        distance_fraction: number(&value["distanceFraction"]),
        projection_distance: number(&value["projectionDistance"]),
        source_position: point(&value["sourcePosition"]),
        source_radius: number(&value["sourceRadius"]),
    }
}

fn edge(value: &Value) -> NativeShadowEdge {
    NativeShadowEdge {
        base_alpha: number(&value["baseAlpha"]),
        base_start: point(&value["baseStart"]),
        base_end: point(&value["baseEnd"]),
        tip_alpha: number(&value["tipAlpha"]),
        tip_start: point(&value["tipStart"]),
        tip_end: point(&value["tipEnd"]),
    }
}

#[test]
fn stock_scenery_shadow_programs_and_light_targets_match_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/boneyard-shadow-goldens.json"))
            .unwrap();
    let mut lights = NativeBoneyardLightIndex::new(Vec2::new(2100.0, 3700.0));
    let providers: Vec<_> = fixture["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| {
            NativeBoneyardLightSource::new(
                point(&source["position"]),
                number(&source["radius"]),
                number(&source["intensity"]),
                source["castsDirectionalShadow"].as_bool().unwrap(),
            )
        })
        .collect();
    lights.rebuild(
        &providers,
        &[],
        NativeBoneyardLightManagerView {
            camera_position: point(&fixture["view"]["camera"]),
            zoom: 1.2,
            viewport: Vec2::new(1600.0, 900.0),
        },
        0.25,
    );
    for case in fixture["cases"].as_array().unwrap() {
        let caster = caster(&case["caster"]);
        let frame = case["frame"].as_u64().unwrap();
        let records: Vec<_> = case["records"].as_array().unwrap().iter().map(record).collect();
        assert_eq!(
            native_shadow_records(&caster, &lights, frame),
            records,
            "{} frame {frame}",
            caster.id
        );
        let edges: Vec<_> = case["edges"].as_array().unwrap().iter().map(edge).collect();
        assert_eq!(
            native_shadow_edges(&caster, &lights, frame),
            edges,
            "{} frame {frame}",
            caster.id
        );
    }
    for case in fixture["targets"].as_array().unwrap() {
        let viewport = &case["viewport"];
        let actual = native_region_light_target_plan(
            Vec2::new(number(&viewport["width"]), number(&viewport["height"])),
            number(&case["resolution"]),
            number(&case["quality"]),
        );
        assert_eq!(actual.logical_side, number(&case["expected"]["logicalSide"]));
        assert_eq!(actual.physical_side, case["expected"]["physicalSide"].as_u64().unwrap() as u32);
        assert_eq!(actual.render_resolution, number(&case["expected"]["renderResolution"]));
    }
}
