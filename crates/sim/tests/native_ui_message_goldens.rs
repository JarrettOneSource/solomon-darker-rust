//! Exact source standard-notice wrapping, DataLine placement and action/frame bounds.
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::NativeUiFontName;
use solomon_sim::native_ui_text::messages::{
    NativeUiMessageDataLineSpec, layout_native_ui_single_action_message,
};

#[path = "support/website_rows.rs"]
mod website_rows;

fn rect(value: HudRect) -> Value {
    json!({"left":value.x,"top":value.y,"width":value.width,"height":value.height})
}

#[test]
fn stock_notices_keep_native_msgbox_wrapping_and_measured_frames() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/native-ui-message-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut inputs = Vec::new();
        for line in case["lines"].as_array().unwrap() {
            inputs.push(NativeUiMessageDataLineSpec {
                font: match line["font"].as_str().unwrap() {
                    "menu" => NativeUiFontName::Menu,
                    "medium" => NativeUiFontName::Medium,
                    _ => unreachable!(),
                },
                text: line["text"].as_str().unwrap(),
                tint: line["tint"].as_u64().unwrap_or(0xffffff) as u32,
                gap_after: line["gapAfter"].as_f64().unwrap_or(0.0),
            });
        }
        let layout = layout_native_ui_single_action_message(
            Vec2::new(case["anchor"]["x"].as_f64().unwrap(), case["anchor"]["y"].as_f64().unwrap()),
            &inputs,
        )
        .unwrap();
        let mut lines = Vec::new();
        for line in layout.lines {
            lines.push(json!({
                "baselineY":line.baseline_y,"font":line.font.as_str(),"gapAfter":line.gap_after,
                "height":line.height,"lineHeight":line.line_height,"text":line.text,
                "tint":line.tint,"width":line.width,"x":line.x,
            }));
        }
        let actual = json!({
            "actionBounds":rect(layout.action_bounds),"actionTextBaselineY":layout.action_text_baseline_y,
            "frameBounds":rect(layout.frame_bounds),"panelBounds":rect(layout.panel_bounds),"lines":lines,
        });
        website_rows::compare(&actual, &case["expected"], case["name"].as_str().unwrap());
    }
}
