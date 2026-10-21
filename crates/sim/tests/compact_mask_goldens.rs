//! Actual view admission, authored stamps, Spider decals and per-player flicker.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::compact_masks::{
    NativeCompactMasks, native_direct_environment_light_alpha,
};
use solomon_sim::boneyard::{BoneyardBounds, BoneyardScene, BoneyardSpawn, BoneyardSprite};
use solomon_sim::enemies::spider_remains::{SpiderDecal, SpiderRemains, create_native_dead_spider};

#[path = "support/website_rows.rs"]
mod website_rows;

#[test]
fn compact_ground_masks_and_player_cell_admission_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/compact-mask-goldens.json")).unwrap();
    let scene = &fixture["scene"];
    let scene = BoneyardScene {
        name: "compact-mask-replay".into(),
        environment_mode: 0,
        bounds: bounds(&scene["bounds"]),
        spawn: BoneyardSpawn { position: Vec2::ZERO, facing_degrees: 0.0 },
        objects: Vec::new(),
        sprites: scene["sprites"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sprite| BoneyardSprite {
                eid: sprite["eid"].as_str().unwrap().into(),
                atlas_entry: sprite["atlasEntry"].as_u64().unwrap() as usize,
                dead_hawg_entry: None,
                position: point(&sprite["pos"]),
                s0: number(&sprite["s0"]),
                s1: number(&sprite["s1"]),
                s2: 0.0,
                flags: sprite["flags"].as_u64().unwrap() as u32,
            })
            .collect(),
        roads: Vec::new(),
        fences: Vec::new(),
        terrain: Vec::new(),
        solomon_dig: None,
    };
    let remains: Vec<_> = fixture["remains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|remains| {
            let mut state = create_native_dead_spider(Vec2::ZERO, 0.0);
            let decal = &remains["state"]["decal"];
            if !decal.is_null() {
                state.decal = Some(SpiderDecal {
                    entry: decal["entry"].as_u64().unwrap() as usize,
                    position: point(&decal["position"]),
                    rotation_deg: number(&decal["rotationDeg"]),
                    scale: number(&decal["scale"]),
                    alpha: number(&decal["alpha"]),
                });
            }
            SpiderRemains { id: remains["id"].as_u64().unwrap() as usize, spawn_tick: 0, state }
        })
        .collect();
    let masks = NativeCompactMasks::new(&scene);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let players: Vec<_> = case["players"].as_array().unwrap().iter().map(point).collect();
        let plan = masks.plan(
            &players,
            &remains,
            bounds(&case["bounds"]),
            number(&case["frame"]),
            &|entry| {
                let size = &fixture["sizes"][entry.to_string()];
                Vec2::new(number(&size["width"]), number(&size["height"]))
            },
        );
        let mask_rows: Vec<_> = plan
            .masks
            .iter()
            .map(|mask| {
                json!([
                    mask.id,
                    mask.entry,
                    mask.position.x,
                    mask.position.y,
                    mask.rotation_degrees,
                    mask.scale.x,
                    mask.scale.y,
                    mask.alpha,
                    mask.dynamic,
                ])
            })
            .collect();
        let player_rows: Vec<_> = plan
            .players
            .iter()
            .map(|player| {
                let ids: Vec<_> =
                    player.mask_indices.iter().map(|index| &plan.masks[*index].id).collect();
                json!([player.slot, player.position.x, player.position.y, player.alpha, ids])
            })
            .collect();
        website_rows::compare(
            &json!({ "masks": mask_rows, "players": player_rows }),
            &case["plan"],
            &format!("compact[{index}]"),
        );
    }
    // Only the source's unrounded sine may differ between Darwin libm and V8.
    const TRIG_TOLERANCE: f64 = 1e-15;
    for aperture in fixture["apertures"].as_array().unwrap() {
        let alpha = native_direct_environment_light_alpha(
            number(&aperture[0]),
            aperture[1].as_u64().unwrap() as usize,
        );
        assert!((alpha - number(&aperture[2])).abs() < TRIG_TOLERANCE);
    }
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}

fn bounds(value: &Value) -> BoneyardBounds {
    BoneyardBounds::new(
        number(&value["x"]),
        number(&value["y"]),
        number(&value["w"]),
        number(&value["h"]),
    )
}
