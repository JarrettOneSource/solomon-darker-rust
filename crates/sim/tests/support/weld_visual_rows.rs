use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::spells::presentation::{SpellAtlas, SpellBlend};
use solomon_sim::spells::weld::{
    NativeWeldMeshUvSpace, NativeWeldPresentationState, NativeWeldSpriteDraw, NativeWeldVisualPlan,
    native_weld_visual_plan, native_weld_visual_plan_at_age,
};

pub fn sample(case: &Value, tick: u64) -> Option<&Value> {
    case["visuals"].as_array()?.iter().find(|sample| sample["tick"] == tick)
}

fn point(value: Vec2) -> [f64; 2] {
    [value.x, value.y]
}

fn blend(value: SpellBlend) -> &'static str {
    if value == SpellBlend::Add { "add" } else { "normal" }
}

fn sprite(draw: NativeWeldSpriteDraw) -> Value {
    let sprite = draw.sprite;
    let matrix = draw.matrix.map(|matrix| {
        json!({"a":matrix.a,"b":matrix.b,"c":matrix.c,"d":matrix.d,"tx":matrix.tx,"ty":matrix.ty})
    });
    json!({
        "alpha":sprite.alpha,"atlas":if sprite.atlas==SpellAtlas::BadGuys {"BadGuys"} else {"DeadHawg"},
        "blend":blend(sprite.blend),"diffuse":draw.diffuse,"matrix":matrix,"offset":point(sprite.offset),
        "record":sprite.record,"rotation":sprite.rotation,"scale":point(sprite.scale),"tint":sprite.tint,
    })
}

pub fn plan(plan: NativeWeldVisualPlan) -> Value {
    let lines: Vec<_> = plan
        .lines
        .into_iter()
        .map(|line| {
            json!({"alpha":line.alpha,"color":line.color,"end":point(line.end),
            "start":point(line.start),"width":line.width})
        })
        .collect();
    let meshes: Vec<_> = plan.meshes.into_iter().map(|mesh| {
        let mut row = json!({"alpha":mesh.alpha,"blend":blend(mesh.blend),"indices":mesh.mesh.indices,
            "record":mesh.record,"tint":mesh.tint,"uvs":mesh.mesh.uvs.into_iter().map(point).collect::<Vec<_>>(),
            "vertices":mesh.mesh.vertices.into_iter().map(point).collect::<Vec<_>>()});
        if let Some(colors) = mesh.colors {
            row["colors"] = json!(colors);
        }
        if mesh.uv_space == NativeWeldMeshUvSpace::BadGuysAtlas {
            row["uvSpace"] = json!("badguys-atlas");
        }
        row
    }).collect();
    let mut row = json!({"lines":lines,"meshes":meshes,"position":point(plan.position),
        "regionLightPoint":plan.region_light_point.map(point),"sortBias":plan.sort_bias,
        "sprites":plan.sprites.into_iter().map(sprite).collect::<Vec<_>>(),
        "underlays":plan.underlays.into_iter().map(sprite).collect::<Vec<_>>(),"worldY":plan.world_y});
    if let Some(bounds) = plan.split_bounds {
        row["splitBounds"] =
            json!({"x":bounds.x,"y":bounds.y,"width":bounds.width,"height":bounds.height});
    }
    row
}

pub fn compare<'a>(
    states: impl IntoIterator<Item = NativeWeldPresentationState<'a>>,
    expected: &Value,
    path: &str,
) {
    let frame = expected["presentationFrame"].as_u64().unwrap();
    let hit = expected["puppetHit"].as_bool().unwrap();
    let mut actual = Vec::new();
    for state in states {
        let draw = if let Some(age) = expected["age"].as_f64() {
            native_weld_visual_plan_at_age(state, frame, age, hit)
        } else {
            native_weld_visual_plan(state, frame, hit)
        };
        actual.push(plan(draw));
    }
    compare_draws(&json!(actual), &expected["plans"], path);
}

// Compositor doubles retain the established libm tolerance. Integer records,
// packed colors and native float32 mesh buffers still compare exactly.
fn compare_draws(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            if actual.is_i64() && expected.is_i64() {
                assert_eq!(actual, expected, "{path}");
            } else {
                let actual = actual.as_f64().unwrap();
                let expected = expected.as_f64().unwrap();
                if path.contains(".vertices[") || path.contains(".uvs[") {
                    assert_eq!(actual, expected, "{path}");
                } else {
                    assert!((actual - expected).abs() < 1e-9, "{path}: {actual} != {expected}");
                }
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                compare_draws(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (key, expected) in expected {
                compare_draws(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
