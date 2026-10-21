use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::compact_masks::NativeCompactGroundSurface;
use solomon_sim::boneyard::ground_glyph::{NativeGroundPlayer, native_player_ground_glyph};
use solomon_sim::boneyard::terrain_surface::*;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardRoad, BoneyardScene, BoneyardSpawn, BoneyardSprite, BoneyardTerrain,
};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::enemies::spider_remains::{SpiderDecal, SpiderRemains, create_native_dead_spider};

#[path = "support/website_rows.rs"]
mod website_rows;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn point_json(point: Vec2) -> Value {
    json!({"x":point.x,"y":point.y})
}
fn area(value: &Value) -> BoneyardBounds {
    BoneyardBounds::new(
        value["x"].as_f64().unwrap(),
        value["y"].as_f64().unwrap(),
        value["w"].as_f64().unwrap(),
        value["h"].as_f64().unwrap(),
    )
}
fn terrain(value: &Value) -> BoneyardTerrain {
    BoneyardTerrain {
        eid: value["eid"].as_str().unwrap().into(),
        position: point(&value["pos"]),
        points: value["points"].as_array().map(|values| values.iter().map(point).collect()),
        style: value["style"].as_u64().map(|n| n as u32),
        entry: None,
        uid: value["uid"].as_f64(),
        profile_samples: value["profileSamples"]
            .as_array()
            .map(|values| values.iter().map(|n| n.as_f64().unwrap()).collect()),
        side_sign: value["sideSign"].as_f64(),
    }
}
fn road(value: &Value) -> BoneyardRoad {
    BoneyardRoad {
        eid: value["eid"].as_str().unwrap().into(),
        type_id: value["typeId"].as_u64().unwrap() as u32,
        link_mask: value["linkMask"].as_u64().unwrap() as u8,
        points: value["points"].as_array().unwrap().iter().map(point).collect(),
        style: value["style"].as_u64().map(|n| n as u32),
        start_width_scale: value["startWidthScale"].as_f64(),
        end_width_scale: value["endWidthScale"].as_f64(),
        quad: None,
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/terrain-surface-goldens.json")).unwrap()
}

#[test]
fn terrain_inner_strips_and_first_crossing_bridges_match_native_source() {
    let fixture = fixture();
    let roads: Vec<_> = fixture["roads"].as_array().unwrap().iter().map(road).collect();
    for case in fixture["cases"].as_array().unwrap() {
        let surface = build_native_terrain_surface(&terrain(&case["input"]));
        let encoded = json!({"cursors":surface.cursors,"quads":surface.quads.iter().map(|quad|quad.map(point_json)).collect::<Vec<_>>()});
        website_rows::compare(&encoded, &case["surface"], case["input"]["eid"].as_str().unwrap());
        let bridges = build_native_bridge_surfaces(std::slice::from_ref(&surface), &roads);
        let encoded:Value=bridges.iter().map(|bridge|json!({"quad":bridge.quad.map(point_json),"bounds":{"x":bridge.bounds.x,"y":bridge.bounds.y,"w":bridge.bounds.width,"h":bridge.bounds.height}})).collect();
        website_rows::compare(&encoded, &case["bridges"], "bridges");
        for query in case["queries"].as_array().unwrap() {
            let point = point(&query["point"]);
            let quads: Vec<_> = surface
                .quads
                .iter()
                .map(|quad| native_surface_quad_contains(quad, point))
                .collect();
            let actual: Vec<_> = bridges
                .iter()
                .map(|bridge| native_bridge_surface_contains(bridge, point))
                .collect();
            assert_eq!(json!(quads), query["quads"], "{} quad {point:?}", case["input"]["eid"]);
            assert_eq!(
                json!(actual),
                query["bridges"],
                "{} bridge {point:?}",
                case["input"]["eid"]
            );
        }
    }
}

fn scene(value: &Value) -> BoneyardScene {
    BoneyardScene {
        name: value["name"].as_str().unwrap().into(),
        environment_mode: 0,
        bounds: area(&value["bounds"]),
        spawn: BoneyardSpawn { position: Vec2::ZERO, facing_degrees: 0.0 },
        objects: Vec::new(),
        fences: Vec::new(),
        solomon_dig: None,
        terrain: value["terrain"].as_array().unwrap().iter().map(terrain).collect(),
        roads: value["roads"].as_array().unwrap().iter().map(road).collect(),
        sprites: value["sprites"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| BoneyardSprite {
                eid: row["eid"].as_str().unwrap().into(),
                atlas_entry: row["atlasEntry"].as_u64().unwrap() as usize,
                dead_hawg_entry: None,
                position: point(&row["pos"]),
                s0: row["s0"].as_f64().unwrap(),
                s1: row["s1"].as_f64().unwrap(),
                s2: row["s2"].as_f64().unwrap(),
                flags: row["flags"].as_u64().unwrap() as u32,
            })
            .collect(),
    }
}

#[test]
fn special_surface_queries_and_player_ground_glyphs_use_native_ownership() {
    let fixture = fixture();
    let sizes = |entry| {
        let row = fixture["sizes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["entry"].as_u64() == Some(entry as u64))
            .unwrap();
        Vec2::new(row["width"].as_f64().unwrap(), row["height"].as_f64().unwrap())
    };
    let mut state = create_native_dead_spider(Vec2::new(200.0, 200.0), 0.0);
    state.decal = Some(SpiderDecal {
        entry: 140,
        position: state.position,
        rotation_deg: 91.0,
        scale: 0.1,
        alpha: 0.2,
    });
    let remains = [SpiderRemains { id: 1, spawn_tick: 0, state }];
    for case in fixture["queries"].as_array().unwrap() {
        let surface = NativeCompactGroundSurface::new(&scene(&case["scene"]));
        let point = point(&case["point"]);
        assert_eq!(
            surface.contains(point, &[], &sizes),
            case["withoutRemains"].as_bool().unwrap(),
            "{} {point:?} ordinary",
            case["shape"]
        );
        assert_eq!(
            surface.contains(point, &remains, &sizes),
            case["withRemains"].as_bool().unwrap(),
            "{} {point:?} Spider",
            case["shape"]
        );
    }
    for case in fixture["glyphs"].as_array().unwrap() {
        let player = &case["player"];
        let player = NativeGroundPlayer {
            position: point(&player["position"]),
            heading_degrees: player["headingDegrees"].as_f64().unwrap(),
            scale: player["scale"].as_f64().unwrap(),
            life_state: match player["lifeState"].as_str().unwrap() {
                "alive" => PlayerLifeState::Alive,
                "lethal-pending" => PlayerLifeState::LethalPending,
                "dying" => PlayerLifeState::Dying,
                "spectating" => PlayerLifeState::Spectating,
                other => panic!("lifeState{other}"),
            },
            death_tick: player["deathTick"].as_u64().unwrap(),
            corpse_consumed: player["corpseConsumed"].as_bool().unwrap(),
        };
        for (special, key) in [(false, "ordinary"), (true, "special")] {
            let actual=native_player_ground_glyph(player,special).map_or(Value::Null,|plan|json!({"position":point_json(plan.position),"scaleX":plan.scale_x,"scaleY":plan.scale_y,"alpha":plan.alpha}));
            website_rows::compare(&actual, &case[key], key);
        }
    }
}
