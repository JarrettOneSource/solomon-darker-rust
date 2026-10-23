use solomon_web::gpu_ui::{canvas_point, clip_scissor, key_from_browser};

#[test]
fn quickbar_cooldown_uses_the_stock_square_perimeter_and_svg_formatter() {
    let points = solomon_web::gpu_ui::cooldown_sector_points(1.0, 4.0);
    assert_eq!(
        points,
        vec![
            egui::pos2(26.5, 26.5),
            egui::pos2(26.5, 53.0),
            egui::pos2(53.0, 53.0),
            egui::pos2(53.0, 26.5)
        ]
    );
    assert_eq!(
        solomon_sim::hud::quickbar::native_cooldown_sector_path(1.0, 4.0),
        "M 26.5 26.5 L 26.5 53 L 53 53 L 53 26.5 Z"
    );
    assert!(solomon_web::gpu_ui::cooldown_sector_points(0.0, 4.0).is_empty());
    assert!(solomon_web::gpu_ui::cooldown_sector_points(1.0, 0.0).is_empty());
    assert_eq!(solomon_web::gpu_ui::cooldown_sector_points(4.0, 4.0).len(), 10);
}

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
