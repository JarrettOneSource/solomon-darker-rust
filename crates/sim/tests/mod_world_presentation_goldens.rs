//! Accepted runtime rows are projected once, with the Source asset/number/member rules.

use serde_json::{Value, json};
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::presentation::*;
use solomon_sim::modding::{LuaObject, LuaValue};

#[test]
fn mod_world_families_match_the_source_projection_boundaries() {
    let golden: Value =
        serde_json::from_str(include_str!("fixtures/website/mod-world-presentation-goldens.json"))
            .unwrap();
    let assets: Vec<_> = golden["assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|asset| GameModAsset {
            byte_length: asset["byteLength"].as_u64().unwrap(),
            content_type: asset["contentType"].as_str().unwrap().into(),
            kind: asset["kind"].as_str().unwrap().into(),
            mod_id: asset["modId"].as_str().unwrap().into(),
            path: asset["path"].as_str().unwrap().into(),
            sha256: asset["sha256"].as_str().unwrap().into(),
        })
        .collect();
    for case in golden["cases"].as_array().unwrap() {
        let runtime = lua(&case["runtime"]);
        let runtime = runtime.as_object();
        let enemies: Vec<_> = project_mod_enemies(runtime, &assets).iter().map(enemy).collect();
        assert_source_values(&json!(enemies), &case["enemies"], "enemies");
        let effects: Vec<_> =
            project_mod_spell_effects(runtime, &assets).iter().map(effect).collect();
        assert_source_values(&json!(effects), &case["effects"], "effects");
        let scene = project_mod_scene(runtime, case["ownerId"].as_str().unwrap());
        assert_source_values(
            &scene.as_ref().map(scene_json).unwrap_or(Value::Null),
            &case["scene"],
            "scene",
        );
    }
}

fn url(asset: &GameModAsset) -> String {
    format!("/api/game/content/{}", asset.sha256)
}

fn frame(frame: ModImageFrame) -> Value {
    json!({ "x": frame.x, "y": frame.y, "width": frame.width, "height": frame.height })
}

fn enemy(model: &ModEnemyModel<'_>) -> Value {
    json!({
        "currentHealth": model.current_health, "frame": frame(model.frame), "id": model.id,
        "imageHeight": model.image_size.y, "imageUrl": url(model.image), "imageWidth": model.image_size.x,
        "lifeState": model.life_state.as_str(), "lightColor": model.light_color, "lightRadius": model.light_radius,
        "maximumHealth": model.maximum_health, "name": model.name, "scale": model.scale,
        "soundEventTick": model.sound_event_tick, "soundUrl": model.sound.map(url), "soundVolume": model.sound_volume,
        "x": model.position.x, "y": model.position.y,
    })
}

fn effect(model: &ModSpellEffectModel<'_>) -> Value {
    json!({
        "frame": frame(model.frame), "id": model.id,
        "imageHeight": model.image_size.y, "imageUrl": url(model.image), "imageWidth": model.image_size.x,
        "kind": model.kind.as_str(), "radius": model.radius,
        "soundUrl": model.sound.map(url), "soundVolume": model.sound_volume, "startedTick": model.started_tick,
        "targetX": model.target.x, "targetY": model.target.y, "x": model.position.x, "y": model.position.y,
    })
}

fn scene_json(scene: &ModSceneModel<'_>) -> Value {
    let rooms: Vec<_> = scene
        .rooms
        .iter()
        .map(|room| {
            json!({
                "contentId": room.content_id, "description": room.description, "floor": room.floor,
                "height": room.height, "name": room.name, "width": room.width,
                "props": room.props.iter().map(|row| object_json(row)).collect::<Vec<_>>(),
                "walls": room.walls.iter().map(|row| object_json(row)).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({ "contentId": scene.content_id, "epoch": scene.epoch, "roomIndex": scene.room_index, "rooms": rooms })
}

fn lua(value: &Value) -> LuaValue {
    match value {
        Value::Null => LuaValue::Null,
        Value::Bool(value) => LuaValue::Boolean(*value),
        Value::Number(value) => LuaValue::Number(value.as_f64().unwrap()),
        Value::String(value) => LuaValue::String(value.clone()),
        Value::Array(values) => LuaValue::Array(values.iter().map(lua).collect()),
        Value::Object(fields) => {
            LuaValue::Object(fields.iter().map(|(key, value)| (key.clone(), lua(value))).collect())
        }
    }
}

fn object_json(object: &LuaObject) -> Value {
    Value::Object(object.iter().map(|(key, value)| (key.into(), lua_json(value))).collect())
}

fn lua_json(value: &LuaValue) -> Value {
    match value {
        LuaValue::Null => Value::Null,
        LuaValue::Boolean(value) => json!(value),
        LuaValue::Number(value) => json!(value),
        LuaValue::String(value) => json!(value),
        LuaValue::Array(values) => Value::Array(values.iter().map(lua_json).collect()),
        LuaValue::Object(object) => object_json(object),
    }
}

fn assert_source_values(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}");
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_source_values(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (key, expected) in expected {
                assert_source_values(actual.get(key).unwrap(), expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
