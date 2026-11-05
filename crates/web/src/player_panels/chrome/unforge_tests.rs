//! The actual footer producer, real stock records and egui stage transforms.
//! GPU pixels and real game contexts remain separate acceptance requirements.

use super::super::*;
use super::PanelCanvas;
use crate::gpu_ui::unforge::{Frame, Paint};
use solomon_sim::profile::{WizardDiscipline, WizardElement};

fn assets() {
    solomon_graphics::assets::configure(
        solomon_graphics::assets::NativeAssets::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web"
        ))
        .unwrap(),
    );
}

fn stock() -> StockAtlas {
    assets();
    futures_lite::future::block_on(StockAtlas::load("UI")).unwrap()
}

#[test]
fn unforge_scroll_uses_every_native_phase_signed_division_and_wrap() {
    let ui = stock();
    let image = ui.record(77).unwrap();
    let mask = ui.record(76).unwrap();
    assert_eq!((image.x, image.y, image.width, image.height), (959, 130, 55, 55));
    assert_eq!((mask.x, mask.y, mask.width, mask.height), (160, 983, 51, 39));
    let marker = ui.record(75).unwrap();
    assert_eq!((marker.x, marker.y, marker.width, marker.height), (502, 775, 68, 57));
    for phase in 0..55 {
        for tick in phase * 8..phase * 8 + 8 {
            let frame = Frame::new(tick, image, mask);
            assert_eq!(
                frame.image_centers,
                [[128.0, 128.0 - phase as f32], [128.0, 182.0 - phase as f32]]
            );
            assert_eq!(frame.clip, [103, 109, 51, 39]);
        }
    }
    for (tick, y) in [(440, 128.0), (-7, 128.0), (-8, 129.0)] {
        assert_eq!(Frame::new(tick, image, mask).image_centers[0], [128.0, y]);
    }
    for (tick, red) in [(0, 153), (90, 204), (270, 102), (360, 153)] {
        assert_eq!(Frame::new(tick, image, mask).marker_red, red as f32 / 255.0);
    }
}

fn footer_paints(
    panel: PlayerPanel,
    now: f64,
    reveal: f32,
    rolled: bool,
    scale: f32,
) -> Vec<Paint> {
    assets();
    let mut panels =
        futures_lite::future::block_on(PlayerPanels::attach(HotbarSelection::for_test())).unwrap();
    panels.panel = Some(panel);
    panels.now = now;
    let selection = NewGameSelection {
        name: "Unforge".into(),
        element: WizardElement::Fire,
        discipline: WizardDiscipline::Arcane,
    };
    let book = NativeSkillBookState::new(selection.element, selection.discipline);
    let mut economy = HubEconomyState::new(1, selection.element);
    economy.dowsing_rolled = rolled;
    let snapshot = PlayerPanelSnapshot {
        selection: &selection,
        book: &book,
        stat_book: &PlayerStatBookComponent::default(),
        economy: &economy,
        mod_content: None,
        mod_assets: None,
        belt: &PlayerBeltComponent { entries: [None; 24] },
        dye_session_id: None,
        progression: &PlayerProgressionState::new(1),
        resources: &PlayerCombatState::default(),
        offensive: &NativeOffensiveSpellFactors::default(),
        attributes: InventoryAttributes::default(),
        concentrations: &[None; 2],
        mind_chug_active: false,
        perk_removal_enabled: true,
        kills: 0,
        awesomeness: 0,
    };
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    fonts.families.insert(
        egui::FontFamily::Name("Georgia".into()),
        fonts.families[&egui::FontFamily::Proportional].clone(),
    );
    ctx.set_fonts(fonts);
    let shapes = crate::gpu_ui::record_shapes(&ctx, &["assets/stock/UI.png"], |frame| {
        let layer = panels.layer_id();
        super::native_area(
            &ctx,
            layer,
            egui::emath::TSTransform::new(egui::vec2(13.0, 17.0), scale),
        )
        .show(&ctx, |ui| {
            let hits = RefCell::new(Vec::new());
            let events = RefCell::new(Vec::new());
            let canvas = PanelCanvas::new(frame, ui, &hits, &events, reveal, true);
            panels.draw_footer(&canvas, &snapshot).unwrap();
        });
    });
    shapes.iter().filter_map(|shape| Paint::from_shape(&shape.shape)).collect()
}

#[test]
fn unforge_actual_footer_submits_one_complete_program_for_all_inventory_owners() {
    for panel in [
        PlayerPanel::Inventory,
        PlayerPanel::Service(HubTraderId::Hagatha),
        PlayerPanel::Service(HubTraderId::Fomentius),
        PlayerPanel::Service(HubTraderId::Luthacus),
        PlayerPanel::Service(HubTraderId::Shlorio),
    ] {
        for rolled in [false, true] {
            let paints = footer_paints(panel, 80.0, 1.0, rolled, 1.0);
            assert_eq!(paints.len(), 1, "{panel:?}, rolled={rolled}");
            assert_eq!(paints[0].frame.image_centers, [[128.0, 127.0], [128.0, 181.0]]);
            assert_eq!(paints[0].opacity, 1.0);
        }
    }
    for panel in [
        PlayerPanel::Skills,
        PlayerPanel::Dialogue,
        PlayerPanel::SkillSelector(
            solomon_sim::hud::skill_selector::NativeHudSkillSelectorTarget::Primary,
        ),
    ] {
        assert!(footer_paints(panel, 80.0, 1.0, false, 1.0).is_empty(), "{panel:?}");
    }
}

#[test]
fn unforge_reveal_and_stage_transform_move_target_and_marker_together_without_resetting_time() {
    for reveal in [0.0, 0.5, 1.0] {
        for scale in [0.65, 1.0] {
            let paints = footer_paints(PlayerPanel::Inventory, 900.0, reveal, false, scale);
            assert_eq!(paints.len(), 1);
            let paint = &paints[0];
            let anchor = egui::pos2(1562.0 + (1.0 - reveal) * 25.0, 868.0);
            let expected = |point: egui::Pos2| egui::pos2(13.0, 17.0) + point.to_vec2() * scale;
            assert_eq!(paint.target_positions[0], expected(anchor - egui::vec2(128.5, 128.5)));
            assert_eq!(paint.marker_positions[0], expected(anchor - egui::vec2(34.0, 28.5)));
            assert_eq!(paint.opacity, reveal);
            assert_eq!(paint.frame.marker_red, 204.0 / 255.0);
            assert_eq!(paint.frame.image_centers, [[128.0, 117.0], [128.0, 171.0]]);
        }
    }
}
