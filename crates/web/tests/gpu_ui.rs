use solomon_web::gpu_ui::{canvas_point, clip_scissor, key_from_browser};

#[test]
fn canvas_input_accounts_for_embedded_canvas_position_and_css_scale() {
    let point = canvas_point([180.0, 125.0], [100.0, 50.0, 400.0, 300.0], [800.0, 600.0]);
    assert_eq!(point, egui::pos2(160.0, 150.0));
}

#[test]
fn gpu_clips_round_outward_and_invert_only_the_physical_y_axis() {
    let rect = egui::Rect::from_min_max(egui::pos2(10.25, 20.25), egui::pos2(30.75, 45.75));
    assert_eq!(clip_scissor(rect, [800, 600], [400.0, 300.0]), Some([20, 508, 42, 52]));
    assert_eq!(
        clip_scissor(
            egui::Rect::from_min_max(egui::pos2(-5.0, -5.0), egui::pos2(3.0, 7.0)),
            [800, 600],
            [400.0, 300.0]
        ),
        Some([0, 586, 6, 14])
    );
    assert_eq!(
        clip_scissor(
            egui::Rect::from_min_max(egui::pos2(500.0, 0.0), egui::pos2(510.0, 20.0)),
            [800, 600],
            [400.0, 300.0]
        ),
        None
    );
}

#[test]
fn keyboard_mapping_keeps_navigation_and_rebinding_keys_distinct() {
    assert_eq!(key_from_browser("ArrowLeft"), Some(egui::Key::ArrowLeft));
    assert_eq!(key_from_browser(" "), Some(egui::Key::Space));
    assert_eq!(key_from_browser("a"), Some(egui::Key::A));
    assert_eq!(key_from_browser("F12"), Some(egui::Key::F12));
    assert_eq!(key_from_browser("Dead"), None);
    assert_eq!(key_from_browser("Unidentified"), None);
}
