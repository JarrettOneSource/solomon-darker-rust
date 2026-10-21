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
