//! Real HUD paint submission, using decoded stock atlases and bitmap fonts.

use super::*;
use solomon_sim::hud::HudBoss;
use solomon_sim::ui_text::{UiFontName, measure_ui_text};

fn hud() -> Hud {
    solomon_graphics::assets::configure(
        solomon_graphics::assets::NativeAssets::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web"
        ))
        .unwrap(),
    );
    futures_lite::future::block_on(Hud::attach(HotbarSelection::for_test())).unwrap()
}

fn paint(hud: &mut Hud, state: &HudState) -> Vec<egui::epaint::ClippedShape> {
    hud.update(state, 1600, 900).unwrap();
    crate::gpu_ui::record_shapes(
        &egui::Context::default(),
        &[
            "assets/stock/UI.png",
            "assets/stock/Fonts.png",
            "assets/stock/Skills.png",
            "assets/stock/Inventory.png",
            "assets/stock/College.png",
            "assets/stock/ControlPanel.png",
            "assets/stock/LevelPicker.png",
            "assets/stock/hub-hud-key-backing.png",
        ],
        |frame| hud.paint(frame).unwrap(),
    )
}

#[test]
fn featured_boss_uses_stock_strips_name_width_and_clipped_health() {
    let mut hud = hud();
    hud.set_mode(HudMode::Run).unwrap();
    for name in ["A", "Grand Necromancer"] {
        let width = measure_ui_text(name, UiFontName::WorldAndRoster, 1.0) as f32 + 50.0;
        let left = (1600.0 - width) / 2.0;
        for health in [100.0, 25.0, 0.0] {
            let mut state = HudState::new_game();
            state.boss =
                Some(HudBoss { name: name.into(), current_health: health, maximum_health: 100.0 });
            let frame = paint(&mut hud, &state);
            let meshes: Vec<_> = frame
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) => Some(mesh),
                    _ => None,
                })
                .collect();
            let strip_bounds = |top: f32, bottom: f32| {
                meshes
                    .iter()
                    .filter_map(|mesh| {
                        let bounds = mesh.calc_bounds();
                        (bounds.top() == top && bounds.bottom() == bottom).then_some(bounds)
                    })
                    .fold(egui::Rect::NOTHING, |all, bounds| all.union(bounds))
            };
            assert_eq!(
                strip_bounds(787.0, 807.0),
                egui::Rect::from_min_size(
                    egui::pos2(left - 4.5, 787.0),
                    egui::vec2(width + 11.0, 20.0)
                ),
                "boss background: {name}"
            );
            let fill = strip_bounds(791.0, 802.0);
            if health == 0.0 {
                assert_eq!(fill, egui::Rect::NOTHING);
            } else {
                assert_eq!(
                    fill,
                    egui::Rect::from_min_size(
                        egui::pos2(left, 791.0),
                        egui::vec2(width * health as f32 / 100.0, 11.0)
                    )
                );
            }
            let name_vertices = meshes
                .iter()
                .flat_map(|mesh| &mesh.vertices)
                .filter(|vertex| vertex.color == egui::Color32::from_rgb(191, 191, 191))
                .count();
            assert_eq!(name_vertices, name.chars().filter(|ch| *ch != ' ').count() * 4);
        }
    }
}

#[test]
fn featured_boss_obeys_hub_and_hud_visibility_and_clears_with_snapshot() {
    let mut hud = hud();
    let mut state = HudState::new_game();
    state.boss = Some(HudBoss {
        name: "Heartmonger".into(),
        current_health: 2000.0,
        maximum_health: 2000.0,
    });
    let has_boss_strip = |frame: &Vec<egui::epaint::ClippedShape>| {
        frame.iter().any(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) => mesh.calc_bounds().top() == 787.0,
            _ => false,
        })
    };
    assert!(!has_boss_strip(&paint(&mut hud, &state)));
    hud.set_mode(HudMode::Run).unwrap();
    assert!(has_boss_strip(&paint(&mut hud, &state)));
    hud.set_visible(false);
    assert!(!has_boss_strip(&paint(&mut hud, &state)));
    hud.set_visible(true);
    state.boss = None;
    assert!(!has_boss_strip(&paint(&mut hud, &state)));
}
