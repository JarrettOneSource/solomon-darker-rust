//! Seeker loot projections and the shared native line geometry/color contract.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::line::{native_line_color, native_line_vertices};
use solomon_sim::profile::seeker::*;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn point_snapshot(value: Vec2) -> Value {
    json!({ "x": value.x, "y": value.y })
}
fn kind(value: &Value) -> NativeHagathaSeekerTargetKind {
    match value.as_str().unwrap() {
        "bonus" => NativeHagathaSeekerTargetKind::Bonus,
        "gold" => NativeHagathaSeekerTargetKind::Gold,
        "sack" => NativeHagathaSeekerTargetKind::Sack,
        other => panic!("unknown Seeker target {other}"),
    }
}
fn segment_snapshot(value: NativeHagathaSeekerSegment) -> Value {
    json!({
        "alpha": value.alpha,
        "end": point_snapshot(value.end),
        "endVisible": value.end_visible,
        "start": point_snapshot(value.start),
        "startVisible": value.start_visible,
        "targetId": value.target_id,
        "targetKind": value.target_kind.as_str(),
        "width": value.width,
    })
}
fn mesh_snapshot(value: NativeHagathaSeekerMeshPlan) -> Value {
    json!({ "alpha": value.alpha, "alphaByte": value.alpha_byte, "uvs": value.uvs, "vertices": value.vertices })
}
fn assert_snapshot(actual: &Value, expected: &Value, label: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64(), "{label}"),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{label} length");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                assert_snapshot(a, b, &format!("{label}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{label} fields");
            for (key, b) in b {
                assert_snapshot(&a[key], b, &format!("{label}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{label}"),
    }
}

#[test]
fn seeker_loot_admission_float_pulses_ramp_meshes_and_line_colors_replay_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hagatha-seeker-goldens.json")).unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let targets = case["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|target| NativeHagathaSeekerTarget {
                id: target["id"].as_u64().unwrap() as usize,
                kind: kind(&target["kind"]),
                position: point(&target["position"]),
            })
            .collect::<Vec<_>>();
        let segments = native_hagatha_seeker_segments(
            point(&case["position"]),
            &targets,
            case["tick"].as_f64().unwrap(),
        );
        assert_snapshot(
            &json!(segments.iter().copied().map(segment_snapshot).collect::<Vec<_>>()),
            &case["segments"],
            &format!("Seeker segments {index}"),
        );
        let meshes = segments
            .into_iter()
            .map(native_hagatha_seeker_mesh_plan)
            .map(mesh_snapshot)
            .collect::<Vec<_>>();
        assert_snapshot(&json!(meshes), &case["meshes"], &format!("Seeker meshes {index}"));
    }
    for case in fixture["lines"].as_array().unwrap() {
        assert_snapshot(
            &json!(native_line_vertices(
                point(&case["start"]),
                point(&case["end"]),
                case["width"].as_f64().unwrap()
            )),
            &case["vertices"],
            "shared line vertices",
        );
    }
    for case in fixture["colors"].as_array().unwrap() {
        let channels = case["channels"].as_array().unwrap();
        assert_eq!(
            native_line_color(
                channels[0].as_f64().unwrap(),
                channels[1].as_f64().unwrap(),
                channels[2].as_f64().unwrap(),
                channels[3].as_f64().unwrap()
            ),
            case["color"].as_u64().unwrap() as u32
        );
    }
    for case in fixture["alphaMeshes"].as_array().unwrap() {
        let value = &case["segment"];
        let segment = NativeHagathaSeekerSegment {
            alpha: value["alpha"].as_f64().unwrap(),
            end: point(&value["end"]),
            end_visible: value["endVisible"].as_bool().unwrap(),
            start: point(&value["start"]),
            start_visible: value["startVisible"].as_bool().unwrap(),
            target_id: value["targetId"].as_u64().unwrap() as usize,
            target_kind: kind(&value["targetKind"]),
            width: value["width"].as_f64().unwrap(),
        };
        assert_snapshot(
            &mesh_snapshot(native_hagatha_seeker_mesh_plan(segment)),
            &case["mesh"],
            "alpha-byte mesh",
        );
    }
    let program = NATIVE_HAGATHA_SEEKER_PROGRAM;
    assert_snapshot(
        &json!({
            "distanceCap": program.distance_cap,
            "distanceCutoff": program.distance_cutoff,
            "endDistanceFactor": program.end_distance_factor,
            "idPhaseDegrees": program.id_phase_degrees,
            "innerRadius": program.inner_radius,
            "joinRadius": program.join_radius,
            "pulseAmplitude": program.pulse_amplitude,
            "pulseBase": program.pulse_base,
            "tickPhaseDegrees": program.tick_phase_degrees,
            "transparentColor": program.transparent_color,
            "visibleColor": program.visible_color,
            "width": program.width,
        }),
        &fixture["program"],
        "authored Seeker program",
    );
    assert_eq!(json!(NATIVE_HAGATHA_SEEKER_RAMP_RGBA), fixture["ramp"]);
    assert_snapshot(
        &json!({ "transparent": NATIVE_HAGATHA_SEEKER_TRANSPARENT_U, "visible": NATIVE_HAGATHA_SEEKER_VISIBLE_U }),
        &fixture["rampU"],
        "ramp coordinates",
    );
}
