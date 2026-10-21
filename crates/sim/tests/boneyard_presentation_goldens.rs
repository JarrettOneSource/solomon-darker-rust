use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::boneyard::presentation::{
    BoneyardPainterEntry, BoneyardPainterProxy, TreeOcclusion, boneyard_painter_order,
    native_gate_art,
};
use solomon_sim::hub::painter::{PainterLane, PainterLayer};

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}

#[test]
fn tree_fading_gate_art_and_proxy_order_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/boneyard-presentation-goldens.json"))
            .unwrap();
    for case in fixture["trees"].as_array().unwrap() {
        let bounds = &case["bounds"];
        let polygon = case["polygon"].as_array().unwrap().iter().map(point).collect();
        let mut tree = TreeOcclusion::new(
            case["tree"]["eid"].as_str().unwrap(),
            point(&case["tree"]["position"]),
            [
                number(&bounds["x"]),
                number(&bounds["y"]),
                number(&bounds["w"]),
                number(&bounds["h"]),
            ],
            polygon,
        );
        for tick in case["ticks"].as_array().unwrap() {
            let position = point(&tick["position"]);
            assert_eq!(tree.contains(position), tick["inside"].as_bool().unwrap());
            tree.step(Some(tree.contains(position)));
            assert_eq!(tree.countdown, tick["countdown"].as_u64().unwrap() as u32);
            assert_eq!(tree.current_alpha, number(&tick["currentAlpha"]));
            assert_eq!(tree.target_alpha, number(&tick["targetAlpha"]));
        }
    }
    for case in fixture["gates"].as_array().unwrap() {
        let art = native_gate_art(point(&case["hinge"]), point(&case["tip"]));
        let expected = case["vertices"].as_array().unwrap();
        for (index, vertex) in art.vertices.iter().enumerate() {
            assert_eq!(
                *vertex,
                Vec2::new(number(&expected[index * 2]), number(&expected[index * 2 + 1]))
            );
        }
        assert_eq!(art.hinge_sprite, point(&case["hingeSprite"]));
        assert_eq!(art.painter_y, number(&case["painterY"]));
        for (index, line) in art.lines.iter().enumerate() {
            assert_eq!(line[0], point(&case["lines"][index]["start"]));
            assert_eq!(line[1], point(&case["lines"][index]["end"]));
        }
    }
    for case in fixture["orders"].as_array().unwrap() {
        let entries: Vec<_> = case["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                let registration = &entry["registration"];
                BoneyardPainterEntry {
                    draw_id: entry["id"].as_str().unwrap().parse().unwrap(),
                    layer: PainterLayer {
                        visible: true,
                        is_insertion: false,
                        manager_lane: match registration["managerLane"].as_str().unwrap() {
                            "actor" => PainterLane::Actor,
                            "scenery" => PainterLane::Scenery,
                            "transient" => PainterLane::Transient,
                            _ => unreachable!(),
                        },
                        registration_ordinal: registration["registrationOrdinal"].as_u64().unwrap()
                            as usize,
                        world_y: number(&entry["worldY"]),
                        sort_bias: number(&entry["sortBias"]),
                    },
                    proxy: entry["insertions"].as_array().map(|children| {
                        let child = &children[0];
                        BoneyardPainterProxy {
                            draw_id: child["id"].as_str().unwrap().parse().unwrap(),
                            world_y: number(&child["worldY"]),
                            sort_bias: number(&child["sortBias"]),
                        }
                    }),
                }
            })
            .collect();
        let expected: Vec<usize> = case["order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(boneyard_painter_order(&entries, number(&case["referenceY"])), expected);
    }
}
