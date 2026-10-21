//! Native Tutorial callout geometry through the shared authored bitmap font.
use serde_json::{Value, json};
use solomon_sim::boneyard::tutorial::tutorial_callout_geometry;

#[path = "support/website_rows.rs"]
mod website_rows;

#[test]
fn tutorial_callout_frames_use_exact_glyph_measurement_and_native_baselines() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/tutorial-callout-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let geometry = tutorial_callout_geometry(
            case["text"].as_str().unwrap(),
            case["center"]["x"].as_f64().unwrap(),
            case["center"]["y"].as_f64().unwrap(),
        )
        .unwrap();
        let mut lines = Vec::new();
        for line in geometry.lines {
            lines.push(json!({"text":line.text,"width":line.width,"x":line.x,"y":line.y}));
        }
        let frame = geometry.frame;
        let actual = json!({
            "centerX":geometry.center_x,"centerY":geometry.center_y,
            "frame":{"x":frame.x,"y":frame.y,"width":frame.width,"height":frame.height},
            "lines":lines,"textHeight":geometry.text_height,"textWidth":geometry.text_width,
        });
        website_rows::compare(&actual, &case["expected"], "Tutorial bitmap frame");
    }
}
