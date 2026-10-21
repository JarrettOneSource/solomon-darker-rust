//! Ordered `project-boneyard.ts` values used by the source geometry digest.

use solomon_sim::Vec2;
use solomon_sim::boneyard::{BoneyardObject, BoneyardScene};
use solomon_sim::modding::{LuaObject, LuaValue};

fn number(value: f64) -> LuaValue {
    // JSON.stringify maps nonfinite numeric fields to null, including raw f32s
    // retained by the editor. Admission validates playable geometry separately.
    if value.is_finite() { LuaValue::Number(value) } else { LuaValue::Null }
}

fn point(value: Vec2) -> LuaValue {
    object([("x", number(value.x)), ("y", number(value.y))])
}

fn object<const N: usize>(fields: [(&str, LuaValue); N]) -> LuaValue {
    let fields = fields.into_iter().map(|(key, value)| (key.to_owned(), value));
    LuaValue::Object(fields.collect())
}

fn points(values: &[Vec2]) -> LuaValue {
    LuaValue::Array(values.iter().copied().map(point).collect())
}

fn optional_number(fields: &mut LuaObject, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        fields.insert(key.to_owned(), number(value));
    }
}

fn placed_object(value: &BoneyardObject) -> LuaValue {
    let mut fields = LuaObject::new();
    fields.insert("eid".into(), LuaValue::String(value.eid.clone()));
    fields.insert("typeId".into(), number(f64::from(value.type_id)));
    fields.insert("pos".into(), point(value.position));
    optional_number(&mut fields, "variant", value.variant.map(f64::from));
    optional_number(&mut fields, "rot", value.rotation_degrees);
    optional_number(&mut fields, "scale", value.scale);
    optional_number(&mut fields, "sortBias", value.sort_bias);
    optional_number(&mut fields, "atlasEntry", value.atlas_entry.map(|value| value as f64));
    optional_number(
        &mut fields,
        "secondaryAtlasEntry",
        value.secondary_atlas_entry.map(|value| value as f64),
    );
    optional_number(&mut fields, "secondaryVariant", value.secondary_variant.map(f64::from));
    if let Some(value) = value.secondary_visible {
        fields.insert("secondaryVisible".into(), LuaValue::Boolean(value));
    }
    optional_number(
        &mut fields,
        "overlayAtlasEntry",
        value.overlay_atlas_entry.map(|value| value as f64),
    );
    optional_number(&mut fields, "overlayVariant", value.overlay_variant.map(f64::from));
    if let Some(values) = &value.atlas_entries {
        fields.insert(
            "atlasEntries".into(),
            LuaValue::Array(values.iter().map(|value| number(*value as f64)).collect()),
        );
    }
    LuaValue::Object(fields)
}

/// Preserve compact field insertion order and omit absent optional fields.
/// Serializing this value with `json_value::stringify` matches the geometry
/// bytes consumed by `boneyardGeometrySha256`, rather than a sorted JSON map.
pub fn boneyard_scene_source_value(scene: &BoneyardScene) -> LuaValue {
    let sprites = scene
        .sprites
        .iter()
        .map(|value| {
            let mut fields = LuaObject::new();
            fields.insert("eid".into(), LuaValue::String(value.eid.clone()));
            fields.insert("atlasEntry".into(), number(value.atlas_entry as f64));
            optional_number(
                &mut fields,
                "deadHawgEntry",
                value.dead_hawg_entry.map(|value| value as f64),
            );
            fields.insert("pos".into(), point(value.position));
            fields.insert("s0".into(), number(value.s0));
            fields.insert("s1".into(), number(value.s1));
            fields.insert("s2".into(), number(value.s2));
            fields.insert("flags".into(), number(f64::from(value.flags)));
            LuaValue::Object(fields)
        })
        .collect();
    let roads = scene
        .roads
        .iter()
        .map(|value| {
            let mut fields = LuaObject::new();
            fields.insert("eid".into(), LuaValue::String(value.eid.clone()));
            fields.insert("typeId".into(), number(f64::from(value.type_id)));
            fields.insert("points".into(), points(&value.points));
            optional_number(&mut fields, "style", value.style.map(f64::from));
            optional_number(&mut fields, "startWidthScale", value.start_width_scale);
            optional_number(&mut fields, "endWidthScale", value.end_width_scale);
            if let Some(quad) = &value.quad {
                fields.insert("quad".into(), points(quad));
            }
            fields.insert("linkMask".into(), number(f64::from(value.link_mask)));
            LuaValue::Object(fields)
        })
        .collect();
    let fences = scene
        .fences
        .iter()
        .map(|value| {
            let mut fields = LuaObject::new();
            fields.insert("eid".into(), LuaValue::String(value.eid.clone()));
            fields.insert("typeId".into(), number(f64::from(value.type_id)));
            fields.insert("points".into(), points(&value.points));
            optional_number(&mut fields, "style", value.style.map(f64::from));
            optional_number(&mut fields, "segmentCode", value.segment_code.map(f64::from));
            optional_number(
                &mut fields,
                "startPostVariant",
                value.start_post_variant.map(f64::from),
            );
            optional_number(&mut fields, "endPostVariant", value.end_post_variant.map(f64::from));
            LuaValue::Object(fields)
        })
        .collect();
    let terrain = scene
        .terrain
        .iter()
        .map(|value| {
            let mut fields = LuaObject::new();
            fields.insert("eid".into(), LuaValue::String(value.eid.clone()));
            fields.insert("pos".into(), point(value.position));
            if let Some(values) = &value.points {
                fields.insert("points".into(), points(values));
            }
            optional_number(&mut fields, "style", value.style.map(f64::from));
            optional_number(&mut fields, "entry", value.entry.map(|value| value as f64));
            optional_number(&mut fields, "uid", value.uid);
            if let Some(profile) = &value.profile_samples {
                fields.insert(
                    "profileSamples".into(),
                    LuaValue::Array(profile.iter().copied().map(number).collect()),
                );
            }
            optional_number(&mut fields, "sideSign", value.side_sign);
            LuaValue::Object(fields)
        })
        .collect();
    let solomon = scene.solomon_dig.as_ref().map_or(LuaValue::Null, |value| {
        object([
            ("gravePosition", point(value.grave_position)),
            ("lanternPosition", point(value.lantern_position)),
            ("position", point(value.position)),
            (
                "frameProgram",
                LuaValue::Array(
                    value.frame_program.iter().map(|value| number(*value as f64)).collect(),
                ),
            ),
            ("ticksPerFrame", number(value.ticks_per_frame as f64)),
        ])
    });
    object([
        ("name", LuaValue::String(scene.name.clone())),
        ("environmentMode", number(f64::from(scene.environment_mode))),
        (
            "bounds",
            object([
                ("x", number(scene.bounds.x)),
                ("y", number(scene.bounds.y)),
                ("w", number(scene.bounds.width)),
                ("h", number(scene.bounds.height)),
            ]),
        ),
        (
            "spawn",
            object([
                ("x", number(scene.spawn.position.x)),
                ("y", number(scene.spawn.position.y)),
                ("facingDeg", number(scene.spawn.facing_degrees)),
            ]),
        ),
        ("objects", LuaValue::Array(scene.objects.iter().map(placed_object).collect())),
        ("sprites", LuaValue::Array(sprites)),
        ("roads", LuaValue::Array(roads)),
        ("fences", LuaValue::Array(fences)),
        ("terrain", LuaValue::Array(terrain)),
        ("solomonDig", solomon),
    ])
}
