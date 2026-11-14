use serde_json::Value;
use solomon_sim::painter::overlays::{PainterOverlayOrder, PainterOwnerOverlay};

#[test]
fn owner_promotion_retains_pixis_display_order_across_frames_and_empty_views() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/painter-overlay-goldens.json"))
            .unwrap();
    let mut retained = PainterOverlayOrder::default();
    for frame in fixture["frames"].as_array().unwrap() {
        let painters: Vec<_> = frame["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (row[0].as_u64().unwrap() as usize, row[1].as_f64().unwrap()))
            .collect();
        let live: Vec<_> = frame["liveViews"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (row[0].as_u64().unwrap(), row[1].as_u64().unwrap()))
            .collect();
        let overlays: Vec<_> = painters
            .iter()
            .filter(|(id, _)| *id < 100)
            .map(|&(id, _)| PainterOwnerOverlay {
                draw_id: id,
                owner_draw_id: 100,
                view_id: id as u64,
            })
            .collect();
        let expected: Vec<_> = frame["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(retained.apply(&painters, &overlays, &live), expected, "{}", frame["name"]);
    }
}

#[test]
fn sparse_draw_ids_and_equal_depths_keep_the_planned_order() {
    let mut order = PainterOverlayOrder::default();
    let painters = [(9001, 8.0), (73, 2.0), (400, 8.0), (19, 2.0), (800, 1.0)];
    assert_eq!(order.apply(&painters, &[], &[]), [800, 73, 19, 9001, 400]);

    let overlays = [
        PainterOwnerOverlay { draw_id: 73, owner_draw_id: 9001, view_id: 5 },
        PainterOwnerOverlay { draw_id: 800, owner_draw_id: 73, view_id: 8 },
    ];
    assert_eq!(
        order.apply(&painters, &overlays, &[(5, 0), (8, 1)]),
        [19, 9001, 400, 73, 800],
        "a later overlay sees the already promoted owner depth"
    );
}
