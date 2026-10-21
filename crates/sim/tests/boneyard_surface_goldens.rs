//! Authored runtime geometry, including field edge ownership and road f32 stores.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::surface::*;
use solomon_sim::boneyard::{BoneyardBounds, BoneyardRoad};

#[path = "support/website_rows.rs"]
mod website_rows;

fn bounds(value: &Value) -> BoneyardBounds {
    BoneyardBounds::new(
        value["x"].as_f64().unwrap(),
        value["y"].as_f64().unwrap(),
        value["w"].as_f64().unwrap(),
        value["h"].as_f64().unwrap(),
    )
}

fn points(value: &Value) -> Vec<Vec2> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|value| Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap()))
        .collect()
}

fn road(value: &Value) -> BoneyardRoad {
    BoneyardRoad {
        eid: value["eid"].as_str().unwrap().into(),
        type_id: 2050,
        link_mask: value["linkMask"].as_u64().unwrap_or(0) as u8,
        points: points(&value["points"]),
        style: value["style"].as_u64().map(|style| style as u32),
        start_width_scale: value["startWidthScale"].as_f64(),
        end_width_scale: value["endWidthScale"].as_f64(),
        quad: value.get("quad").map(points),
    }
}

fn flat(points: &[Vec2]) -> Vec<f64> {
    let mut values = Vec::with_capacity(points.len() * 2);
    for point in points {
        values.extend([point.x, point.y]);
    }
    values
}

fn mesh(value: &NativeBoneyardSurfaceMesh) -> Value {
    let mut colors = Vec::with_capacity(value.colors.len() * 4);
    for color in &value.colors {
        colors.extend(*color);
    }
    json!({"positions":flat(&value.positions),"uvs":flat(&value.uvs),
        "colors":colors,"indices":value.indices})
}

#[test]
fn current_field_lattice_and_all_road_programs_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/boneyard-surface-goldens.json"))
            .unwrap();
    for case in fixture["fields"].as_array().unwrap() {
        let lattice = create_native_arena_field_lattice(bounds(&case["bounds"])).unwrap();
        website_rows::compare(
            &json!({"x":lattice.x,"y":lattice.y}),
            &case["lattice"],
            "field recurrence",
        );
        let range = native_arena_field_visible_range(&lattice, bounds(&case["view"])).unwrap();
        website_rows::compare(
            &json!({"xStart":range.x_start,"xEnd":range.x_end,"yStart":range.y_start,
                "yEnd":range.y_end,"tileCount":range.tile_count}),
            &case["range"],
            "strict field range",
        );
        let mode = case["mode"].as_u64().unwrap() as u8;
        let descriptor = native_arena_field_descriptor(mode);
        website_rows::compare(
            &json!({"atlas":"DeadHawg","entry":descriptor.entry,
                "frame":{"x":descriptor.frame[0],"y":descriptor.frame[1],
                    "width":descriptor.frame[2],"height":descriptor.frame[3]},
                "page":{"width":2048,"height":2048},"width":350,"height":350,
                "uvs":flat(&descriptor.uvs)}),
            &case["descriptor"],
            "original field descriptor",
        );
        website_rows::compare(
            &mesh(&native_arena_field_mesh_plan(&lattice, range, mode).unwrap()),
            &case["mesh"],
            "field mesh",
        );
    }
    for case in fixture["roads"].as_array().unwrap() {
        let input = road(&case["input"]);
        let plan = native_road_mesh_plan(&input).unwrap();
        assert_eq!(plan.style, case["style"].as_u64().unwrap() as usize);
        website_rows::compare(
            &json!(native_road_endpoint_alphas(input.link_mask)),
            &case["endpointAlphas"],
            "road links",
        );
        website_rows::compare(&mesh(&plan.mesh), &case["mesh"], &input.eid);
    }
    for value in fixture["invalidFields"].as_array().unwrap() {
        assert!(create_native_arena_field_lattice(bounds(value)).is_err());
    }
    for value in fixture["invalidRoads"].as_array().unwrap() {
        assert!(native_road_mesh_plan(&road(value)).is_err());
    }
}
