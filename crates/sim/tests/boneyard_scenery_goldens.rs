use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::scenery::*;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardFence, BoneyardObject, BoneyardScene, BoneyardSpawn, BoneyardSprite,
};

#[path = "support/website_rows.rs"]
mod website_rows;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn optional_u32(value: &Value) -> Option<u32> {
    value.as_u64().map(|value| value as u32)
}
fn optional_usize(value: &Value) -> Option<usize> {
    value.as_u64().map(|value| value as usize)
}
fn point_json(point: Vec2) -> Value {
    json!({"x":point.x,"y":point.y})
}

fn scene(value: &Value) -> BoneyardScene {
    BoneyardScene {
        name: "authored".into(),
        environment_mode: 0,
        bounds: BoneyardBounds::new(-500.0, -500.0, 2000.0, 2000.0),
        spawn: BoneyardSpawn { position: Vec2::ZERO, facing_degrees: 0.0 },
        objects: value["objects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| BoneyardObject {
                eid: row["eid"].as_str().unwrap().into(),
                type_id: row["typeId"].as_u64().unwrap() as u32,
                position: point(&row["pos"]),
                variant: optional_u32(&row["variant"]),
                rotation_degrees: None,
                scale: None,
                sort_bias: row["sortBias"].as_f64(),
                atlas_entry: optional_usize(&row["atlasEntry"]),
                secondary_atlas_entry: optional_usize(&row["secondaryAtlasEntry"]),
                secondary_variant: optional_u32(&row["secondaryVariant"]),
                secondary_visible: row["secondaryVisible"].as_bool(),
                overlay_atlas_entry: optional_usize(&row["overlayAtlasEntry"]),
                overlay_variant: optional_u32(&row["overlayVariant"]),
                atlas_entries: row["atlasEntries"].as_array().map(|entries| {
                    entries.iter().map(|entry| entry.as_u64().unwrap() as usize).collect()
                }),
            })
            .collect(),
        sprites: value["sprites"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| BoneyardSprite {
                eid: row["eid"].as_str().unwrap().into(),
                atlas_entry: row["atlasEntry"].as_u64().unwrap() as usize,
                dead_hawg_entry: row["deadHawgEntry"].as_u64(),
                position: point(&row["pos"]),
                s0: row["s0"].as_f64().unwrap(),
                s1: row["s1"].as_f64().unwrap(),
                s2: row["s2"].as_f64().unwrap(),
                flags: row["flags"].as_u64().unwrap() as u32,
            })
            .collect(),
        fences: value["fences"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| BoneyardFence {
                eid: row["eid"].as_str().unwrap().into(),
                type_id: 2031,
                points: row["points"].as_array().unwrap().iter().map(point).collect(),
                style: optional_u32(&row["style"]),
                segment_code: optional_u32(&row["segmentCode"]),
                start_post_variant: optional_u32(&row["startPostVariant"]),
                end_post_variant: optional_u32(&row["endPostVariant"]),
            })
            .collect(),
        roads: Vec::new(),
        terrain: Vec::new(),
        solomon_dig: None,
    }
}

fn half(value: Option<NativeBrokenFenceHalf>) -> Value {
    let Some(half) = value else { return Value::Null };
    json!({"corners":half.corners.map(point_json),"root":point_json(half.root),"seed":half.seed,
        "shadowStart":point_json(half.shadow_start),"shadowEnd":point_json(half.shadow_end),
        "shadowStep":point_json(half.shadow_step),"shadowCount":half.shadow_count})
}

fn layers(values: &[NativeSceneryLayer]) -> Value {
    Value::Array(values.iter().map(|layer| {
        let source = match layer.source {
            NativeScenerySource::Object { index, entry } => json!({"kind":"object","index":index,"entry":entry}),
            NativeScenerySource::FencePost { index, entry } => json!({"kind":"post","index":index,"entry":entry}),
            NativeScenerySource::FenceBody { index, piece, broken } => json!({"kind":"body","index":index,"piece":piece,"broken":half(broken)}),
        };
        json!({"source":source,"position":point_json(layer.position),"worldY":layer.world_y,"sortBias":layer.sort_bias,"sourceOrder":layer.source_order})
    }).collect())
}

#[test]
fn authored_placements_shared_posts_and_private_broken_halves_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/boneyard-scenery-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let scene = scene(&case["input"]);
        let empty = |entry| {
            fixture["emptyEntries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value.as_u64() == Some(entry as u64))
        };
        let plan = native_scenery_plan(&scene, &empty);
        website_rows::compare(&json!(plan.underlays), &case["underlays"], "underlays");
        website_rows::compare(&json!(plan.compact), &case["compact"], "compact");
        website_rows::compare(&layers(&plan.main), &case["main"], "main");
        website_rows::compare(&layers(&plan.walls), &case["walls"], "walls");
        website_rows::compare(&layers(&plan.proxies), &case["proxies"], "proxies");
        let posts: Vec<_> = plan.posts.iter().map(|post| json!({"fenceIndex":post.fence_index,"position":point_json(post.position),"variant":post.variant,"style":post.style})).collect();
        website_rows::compare(&json!(posts), &case["posts"], "shared post selectors");
        for (fence, expected) in scene.fences.iter().zip(case["grates"].as_array().unwrap()) {
            let actual = native_fence_grate(&fence.points).map_or(Value::Null, |grate| {
                json!({
                "topStart":point_json(grate.corners[0]),"topEnd":point_json(grate.corners[1]),
                "bottomStart":point_json(grate.corners[2]),"bottomEnd":point_json(grate.corners[3]),
                "length":grate.length,"uSpan":grate.u_span})
            });
            website_rows::compare(&actual, expected, "intact grate");
        }
    }
}

fn shadow_glyph(fixture: &Value, entry: usize) -> Option<NativeSceneryShadowGlyph> {
    let glyph = &fixture["glyphs"][entry];
    if glyph.is_null() {
        return None;
    }
    Some(NativeSceneryShadowGlyph {
        width: glyph["width"].as_f64().unwrap(),
        height: glyph["height"].as_f64().unwrap(),
        anchor_x: glyph["anchorX"].as_f64().unwrap(),
        anchor_y: glyph["anchorY"].as_f64().unwrap(),
    })
}

fn shadow_quad_json(quad: NativeSceneryShadowQuad) -> Value {
    let texture = match quad.texture {
        NativeSceneryShadowTexture::Glyph(entry) => json!(entry),
        NativeSceneryShadowTexture::FenceGrate => json!("fence-grate"),
        NativeSceneryShadowTexture::White => json!("white"),
    };
    json!({"alphas":quad.alphas,"role":quad.role.id(),"texture":texture,"tint":quad.tint,
        "uvs":quad.uvs.map(point_json),"vertices":quad.vertices.map(point_json)})
}

#[test]
fn original_scenery_masks_flat_glyphs_and_wall_skirts_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/scenery-shadow-goldens.json")).unwrap();
    let glyph = |entry| shadow_glyph(&fixture, entry);
    for case in fixture["scenes"].as_array().unwrap() {
        let scene = scene(&case["input"]);
        let plan = native_scenery_plan(&scene, &|entry| glyph(entry).is_none());
        for mode in case["modes"].as_array().unwrap() {
            let complex = mode["complex"].as_bool().unwrap();
            let actual: Value = plan
                .main
                .iter()
                .map(|layer| {
                    native_scenery_shadow_plan(&scene, &plan, layer, complex, &glyph)
                        .into_iter()
                        .map(shadow_quad_json)
                        .collect::<Value>()
                })
                .collect();
            website_rows::compare(&actual, &mode["layers"], "scenery auxiliary shadows");
        }
    }
    for case in fixture["goodies"].as_array().unwrap() {
        let goodie = &case["goodie"];
        let actual: Value = native_goodie_shadow_plan(
            point(&goodie["position"]),
            goodie["subtype"].as_u64().unwrap() as u32,
            goodie["phase"].as_u64().unwrap() as usize,
            case["complex"].as_bool().unwrap(),
            &glyph,
        )
        .into_iter()
        .map(shadow_quad_json)
        .collect();
        website_rows::compare(&actual, &case["quads"], "Goodie auxiliary shadows");
    }
    for case in fixture["walls"].as_array().unwrap() {
        let actual = native_wall_skirt_plan(point(&case["start"]), point(&case["end"]));
        website_rows::compare(&shadow_quad_json(actual), &case["quad"], "wall skirt");
    }
}

#[test]
fn scrub_shadow_clock_and_lazy_surface_ownership_match_source() {
    use std::cell::RefCell;

    use solomon_sim::boneyard::shadows::NativeShadowRecord;

    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/scenery-shadow-goldens.json")).unwrap();
    let records: Vec<_> = fixture["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| NativeShadowRecord {
            base_alpha: record["baseAlpha"].as_f64().unwrap(),
            behind_scalar: record["behindScalar"].as_f64().unwrap(),
            direction: point(&record["direction"]),
            distance_fraction: record["distanceFraction"].as_f64().unwrap(),
            projection_distance: record["projectionDistance"].as_f64().unwrap(),
            source_position: point(&record["sourcePosition"]),
            source_radius: record["sourceRadius"].as_f64().unwrap(),
        })
        .collect();
    for case in fixture["scrubs"].as_array().unwrap() {
        let input = &case["input"];
        let entry = 264 + input["variant"].as_u64().unwrap() as usize;
        let mut state = NativeScrubShadowState::new(
            input["seed"].as_u64().unwrap() as usize,
            input["initialTick"].as_u64().unwrap(),
        );
        for frame in case["frames"].as_array().unwrap() {
            state.advance_to(frame["tick"].as_f64().unwrap());
            let queries = RefCell::new(Vec::new());
            let surface_at = |point| {
                let mut queries = queries.borrow_mut();
                queries.push(point_json(point));
                frame["surface"][queries.len() - 1].as_bool().unwrap()
            };
            let complex = frame["complex"].as_bool().unwrap();
            let quads = if frame["visible"].as_bool().unwrap() {
                native_scrub_shadow_plan(
                    entry,
                    shadow_glyph(&fixture, entry).unwrap(),
                    point(&input["position"]),
                    &mut state,
                    if complex && frame["lit"].as_bool().unwrap() { &records } else { &[] },
                    complex,
                    &surface_at,
                    shadow_glyph(&fixture, 21).unwrap(),
                )
            } else {
                Vec::new()
            };
            website_rows::compare(&json!(*queries.borrow()), &frame["queries"], "surface calls");
            website_rows::compare(
                &json!({"phase":state.phase,"uvInset":state.uv_inset,"specialSurface":state.special_surface}),
                &json!({"phase":frame["phase"],"uvInset":frame["uvInset"],"specialSurface":frame["specialSurface"]}),
                "Scrub visual state",
            );
            let actual: Value = quads.into_iter().map(shadow_quad_json).collect();
            website_rows::compare(&actual, &frame["quads"], "Scrub shadow quads");
        }
    }
}
