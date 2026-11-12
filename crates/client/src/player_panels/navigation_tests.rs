//! The optional Inventory and Skills navigation policy in `docs/DECISIONS.md`, tested through
//! the real panel attach/show/update/action boundary. Only the unused OS keyboard subscription
//! is detached; events are native input fixtures, and stock bundles are real.
use super::*;
use solomon_sim::economy::{
    FOMENTIUS_STOCK_DEFINITIONS, HubEconomyContext, HubInventoryItem,
    create_fomentius_inventory_item,
};
use solomon_sim::profile::{WizardDiscipline, WizardElement};

struct Fixture {
    selection: NewGameSelection,
    book: SkillBookState,
    stat_book: PlayerStatBookComponent,
    economy: HubEconomyState,
    dye_session_id: Option<String>,
    belt: PlayerBeltComponent,
    progression: PlayerProgressionState,
    resources: PlayerCombatState,
    offensive: OffensiveSpellFactors,
}

fn item(stock: usize, id: u64) -> HubInventoryItem {
    create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[stock], id, 1)
}

fn sack(id: u64, contents: Vec<HubInventoryItem>) -> HubInventoryItem {
    let mut sack = item(5, id);
    sack.contents = Some(contents);
    sack
}

impl Fixture {
    fn new() -> Self {
        let element = WizardElement::Fire;
        let discipline = WizardDiscipline::Arcane;
        let mut economy = HubEconomyState::new(1, element);
        economy.backpack = vec![
            sack(100, vec![sack(101, vec![item(0, 102), sack(104, Vec::new())])]),
            item(3, 103),
        ];
        let book = SkillBookState::new(element, discipline);
        let belt = PlayerBeltComponent::new(&book);
        Self {
            selection: NewGameSelection { name: "Report 22".into(), element, discipline },
            book,
            stat_book: PlayerStatBookComponent::default(),
            economy,
            dye_session_id: None,
            belt,
            progression: PlayerProgressionState::new(1),
            resources: PlayerCombatState::default(),
            offensive: OffensiveSpellFactors::default(),
        }
    }

    fn snapshot(&self) -> PlayerPanelSnapshot<'_> {
        PlayerPanelSnapshot {
            selection: &self.selection,
            book: &self.book,
            stat_book: &self.stat_book,
            economy: &self.economy,
            mod_content: None,
            mod_assets: None,
            belt: &self.belt,
            dye_session_id: self.dye_session_id.as_deref(),
            progression: &self.progression,
            resources: &self.resources,
            offensive: &self.offensive,
            attributes: InventoryAttributes::default(),
            concentrations: &[None; 2],
            mind_chug_active: false,
            perk_removal_enabled: true,
            kills: 0,
            awesomeness: 0,
        }
    }
}

fn panels() -> PlayerPanels {
    solomon_graphics::assets::configure(
        solomon_graphics::assets::NativeAssets::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web"
        ))
        .unwrap(),
    );
    futures_lite::future::block_on(PlayerPanels::attach(HotbarSelection::for_test())).unwrap()
}

fn update(panels: &mut PlayerPanels, fixture: &Fixture, now: f64, event: Option<PanelEvent>) {
    panels.events.extend(event);
    panels.update(&fixture.snapshot(), now, 1600, 900).unwrap();
}

fn key(code: &str, repeat: bool) -> PanelEvent {
    PanelEvent::Key(PanelKey { code: code.into(), key: code.into(), repeat, modified: false })
}

fn nested(panels: &mut PlayerPanels, fixture: &Fixture, panel: PlayerPanel) {
    panels.show(panel, 0.0).unwrap();
    for (id, now) in [(100, 500.0), (101, 1000.0)] {
        update(panels, fixture, now, Some(PanelEvent::Activate(Target::Item(id))));
        update(panels, fixture, now + 10.0, Some(PanelEvent::Activate(Target::Item(id))));
        update(panels, fixture, now + 380.0, None);
    }
    assert_eq!(panels.sack_path, [100, 101]);
    panels.take_actions();
    panels.take_audio_events();
}

fn play_count(panels: &mut PlayerPanels, cue: AudioCue) -> usize {
    panels
        .take_audio_events()
        .iter()
        .filter(|event| matches!(event, AudioEvent::Play(actual, _) if *actual == cue))
        .count()
}

#[test]
fn inventory_binding_returns_one_sack_then_closes_at_root() {
    let fixture = Fixture::new();
    for panel in [
        PlayerPanel::Inventory,
        PlayerPanel::Service(HubTraderId::Fomentius),
        PlayerPanel::Service(HubTraderId::Hagatha),
        PlayerPanel::Service(HubTraderId::Luthacus),
        PlayerPanel::Service(HubTraderId::Shlorio),
    ] {
        for binding in ["KeyI", "KeyB"] {
            let mut panels = panels();
            let controls = GameControlBindings {
                open_inventory: binding.into(),
                ..GameControlBindings::default()
            };
            panels.set_controls(controls);
            nested(&mut panels, &fixture, panel);
            update(&mut panels, &fixture, 1500.0, Some(key(binding, false)));
            assert_eq!(panels.sack_path, [100], "{panel:?} {binding} returns one root");
            assert_eq!(panels.panel(), Some(panel));
            update(&mut panels, &fixture, 1510.0, Some(key(binding, false)));
            update(&mut panels, &fixture, 1870.0, Some(key(binding, true)));
            assert_eq!(panels.sack_path, [100], "no queued or held-key return");
            assert_eq!(play_count(&mut panels, AudioCue::BackpackClose), 1);
            update(&mut panels, &fixture, 1900.0, Some(key(binding, false)));
            assert!(panels.sack_path.is_empty());
            update(&mut panels, &fixture, 2270.0, Some(key(binding, false)));
            update(&mut panels, &fixture, 2670.0, None);
            assert_eq!(panels.panel(), None, "{panel:?} root closes");
            assert!(panels.take_actions().contains(&PlayerPanelAction::Close));
        }
    }
}

#[test]
fn menu_and_escape_close_whole_optional_books_and_services() {
    let mut fixture = Fixture::new();
    for panel in [
        PlayerPanel::Inventory,
        PlayerPanel::Skills,
        PlayerPanel::Service(HubTraderId::Fomentius),
        PlayerPanel::Service(HubTraderId::Hagatha),
        PlayerPanel::Service(HubTraderId::Luthacus),
        PlayerPanel::Service(HubTraderId::Shlorio),
    ] {
        for code in ["Escape", "KeyM"] {
            for rolled in [false, true] {
                fixture.economy.dowsing_rolled = rolled;
                let mut panels = panels();
                let controls = GameControlBindings {
                    open_menu: "KeyM".into(),
                    ..GameControlBindings::default()
                };
                panels.set_controls(controls);
                if panel == PlayerPanel::Skills {
                    panels.show(panel, 0.0).unwrap();
                    panels.boasts_open = rolled;
                    panels.take_actions();
                    panels.take_audio_events();
                } else {
                    nested(&mut panels, &fixture, panel);
                }
                update(&mut panels, &fixture, 1500.0, Some(key(code, false)));
                assert_eq!(play_count(&mut panels, AudioCue::BackpackClose), 0);
                update(&mut panels, &fixture, 1900.0, None);
                assert_eq!(panels.panel(), None, "{panel:?} {code} closes whole interface");
                assert!(panels.sack_path.is_empty());
                let actions = panels.take_actions();
                assert!(actions.contains(&PlayerPanelAction::Close));
                assert_eq!(
                    actions
                        .iter()
                        .filter(|action| **action
                            == PlayerPanelAction::Inventory(HubInventoryAction::CloseDowsing))
                        .count(),
                    usize::from(panel == PlayerPanel::Service(HubTraderId::Shlorio) && rolled),
                );
                assert_eq!(
                    actions
                        .iter()
                        .filter(|action| **action
                            == PlayerPanelAction::Inventory(HubInventoryAction::CloseHagatha))
                        .count(),
                    usize::from(panel == PlayerPanel::Service(HubTraderId::Hagatha)),
                );
            }
        }
    }
    for target in [Target::Close, Target::Boasts] {
        let mut panels = panels();
        panels.show(PlayerPanel::Skills, 0.0).unwrap();
        update(&mut panels, &fixture, 500.0, Some(PanelEvent::Activate(Target::Boasts)));
        update(&mut panels, &fixture, 510.0, Some(PanelEvent::Activate(target)));
        update(&mut panels, &fixture, 1000.0, None);
        assert_eq!(panels.panel(), Some(PlayerPanel::Skills));
        assert!(!panels.boasts_open, "tome Back/toggle returns to Skills");
        assert!(!panels.take_actions().contains(&PlayerPanelAction::Close));
    }
}

#[test]
fn parent_press_returns_immediately_and_held_release_cannot_skip_a_sack() {
    let fixture = Fixture::new();
    for panel in [PlayerPanel::Inventory, PlayerPanel::Service(HubTraderId::Hagatha)] {
        let mut panels = panels();
        nested(&mut panels, &fixture, panel);
        let point = Vec2::new(60.0, 530.0);
        update(
            &mut panels,
            &fixture,
            1500.0,
            Some(PanelEvent::PointerDown(Target::Back, point, None)),
        );
        assert_eq!(panels.sack_path, [100], "first press starts return before release");
        update(&mut panels, &fixture, 1870.0, None);
        update(
            &mut panels,
            &fixture,
            2000.0,
            Some(PanelEvent::PointerUp(point, Some(Target::Back))),
        );
        assert_eq!(panels.sack_path, [100], "release cannot pop the next page");
        assert_eq!(play_count(&mut panels, AudioCue::BackpackClose), 1);
        assert!(panels.take_actions().is_empty());
        assert_eq!(panels.panel(), Some(panel));
        update(&mut panels, &fixture, 2100.0, Some(PanelEvent::Activate(Target::Back)));
        assert!(
            panels.sack_path.is_empty(),
            "keyboard/accessibility parent activation remains admitted"
        );
    }
}

#[test]
fn selected_item_parent_click_moves_the_item_without_navigating() {
    let fixture = Fixture::new();
    let original = fixture.economy.clone();
    for panel in [PlayerPanel::Inventory, PlayerPanel::Service(HubTraderId::Luthacus)] {
        let mut panels = panels();
        nested(&mut panels, &fixture, panel);
        update(&mut panels, &fixture, 1450.0, Some(PanelEvent::Activate(Target::Item(102))));
        panels.take_audio_events();
        let point = Vec2::new(60.0, 530.0);
        update(
            &mut panels,
            &fixture,
            1500.0,
            Some(PanelEvent::PointerDown(Target::Back, point, None)),
        );
        assert_eq!(panels.sack_path, [100, 101]);
        update(
            &mut panels,
            &fixture,
            1510.0,
            Some(PanelEvent::PointerUp(point, Some(Target::Back))),
        );
        assert_eq!(panels.sack_path, [100, 101]);
        assert_eq!(
            panels.take_actions(),
            [PlayerPanelAction::Inventory(HubInventoryAction::Move {
                item_id: 102,
                destination_sack_id: Some(100),
                destination_slot: None,
            })]
        );
        assert_eq!(play_count(&mut panels, AudioCue::BackpackOpen), 1);
    }
    assert_eq!(fixture.economy, original, "navigation/commands do not mutate accepted inventory");
}

#[test]
fn captured_item_drag_to_parent_retains_the_addressed_transfer() {
    let fixture = Fixture::new();
    let mut panels = panels();
    nested(&mut panels, &fixture, PlayerPanel::Inventory);
    let start = Vec2::new(60.0, 605.0);
    let parent = Vec2::new(60.0, 530.0);
    update(
        &mut panels,
        &fixture,
        1500.0,
        Some(PanelEvent::PointerDown(Target::Item(102), start, None)),
    );
    update(
        &mut panels,
        &fixture,
        1510.0,
        Some(PanelEvent::PointerMove(parent, Some(Target::Back), None)),
    );
    update(&mut panels, &fixture, 1520.0, Some(PanelEvent::PointerUp(parent, Some(Target::Back))));
    assert_eq!(panels.sack_path, [100, 101]);
    assert!(panels.take_actions().contains(&PlayerPanelAction::Inventory(
        HubInventoryAction::Move {
            item_id: 102,
            destination_sack_id: Some(100),
            destination_slot: None,
        }
    )));
}

#[test]
fn book_bindings_cancel_child_notices_and_confirmations_before_navigation() {
    let fixture = Fixture::new();
    for code in ["Escape", "KeyM", "KeyI", "KeyK"] {
        for confirmation in [false, true] {
            let mut panels = panels();
            let controls = GameControlBindings {
                open_menu: "KeyM".into(),
                open_skills: "KeyK".into(),
                ..GameControlBindings::default()
            };
            panels.set_controls(controls);
            nested(&mut panels, &fixture, PlayerPanel::Inventory);
            if confirmation {
                update(
                    &mut panels,
                    &fixture,
                    1440.0,
                    Some(PanelEvent::Activate(Target::Item(104))),
                );
                update(&mut panels, &fixture, 1450.0, Some(PanelEvent::Activate(Target::Unforge)));
                assert!(panels.unforge_item.is_some());
            } else {
                panels.show_notice("Notice", "Owned child modal");
            }
            update(&mut panels, &fixture, 1500.0, Some(key(code, false)));
            assert!(panels.notice.is_none(), "{code} cancels only the child");
            assert!(panels.unforge_item.is_none());
            assert_eq!(panels.sack_path, [100, 101]);
            update(&mut panels, &fixture, 1900.0, None);
            assert_eq!(panels.panel(), Some(PlayerPanel::Inventory));
            assert!(panels.take_actions().is_empty(), "cancellation cannot confirm a transaction");
        }
    }
}

#[test]
fn pending_dye_rename_and_suspended_owner_do_not_leak_book_keys() {
    for code in ["Escape", "KeyM", "KeyI", "KeyK"] {
        for child in ["pending dye", "dye", "rename", "suspended"] {
            let mut fixture = Fixture::new();
            let mut panels = panels();
            let controls = GameControlBindings {
                open_menu: "KeyM".into(),
                open_skills: "KeyK".into(),
                ..GameControlBindings::default()
            };
            panels.set_controls(controls);
            nested(&mut panels, &fixture, PlayerPanel::Inventory);
            match child {
                "pending dye" | "dye" => {
                    panels.open_dye(103, 1400.0).unwrap();
                    let actions = panels.take_actions();
                    if child == "dye" {
                        let PlayerPanelAction::Inventory(action) = &actions[0] else {
                            unreachable!("opening dye emits its economy command");
                        };
                        let result = fixture.economy.apply(action, &HubEconomyContext::default());
                        assert!(result.accepted, "the fixture owns a valid dye kit");
                        fixture.dye_session_id =
                            Some(result.dye_session.as_ref().unwrap().id.clone());
                        panels.inventory_action_feedback(action, &result, 1400.0, None);
                    }
                }
                "rename" => update(
                    &mut panels,
                    &fixture,
                    1400.0,
                    Some(PanelEvent::Activate(Target::RenameSack)),
                ),
                "suspended" => panels.set_input_suspended(true).unwrap(),
                _ => unreachable!(),
            }
            update(&mut panels, &fixture, 1500.0, Some(key(code, false)));
            update(&mut panels, &fixture, 1900.0, None);
            assert_eq!(panels.panel(), Some(PlayerPanel::Inventory), "{child} owns {code}");
            assert_eq!(panels.sack_path, [100, 101]);
            let actions = panels.take_actions();
            if child == "dye" {
                assert_eq!(
                    actions,
                    [PlayerPanelAction::Inventory(HubInventoryAction::CloseDye {
                        session_id: fixture.dye_session_id.unwrap(),
                    })],
                );
                assert!(panels.dye.is_none());
            } else {
                assert!(actions.is_empty());
            }
        }
    }
}

const STOCK_PANEL_IMAGES: [&str; 6] = [
    "assets/stock/UI.png",
    "assets/stock/Inventory.png",
    "assets/stock/Skills.png",
    "assets/stock/Library.png",
    "assets/stock/ControlPanel.png",
    "assets/stock/Fonts.png",
];

fn rename_paint(panels: &mut PlayerPanels) -> Option<(String, Vec<u8>)> {
    let images = [
        STOCK_PANEL_IMAGES.as_slice(),
        &[
            "assets/stock/menu/main-menu-button.png",
            "assets/stock/menu/main-menu-button-hover.png",
            "assets/stock/menu/main-menu-button-corner.png",
            "assets/stock/menu/main-menu-button-rail.png",
        ],
    ]
    .concat();
    let frame = crate::gpu_ui::record_frame(&egui::Context::default(), &images, |frame| {
        panels.paint(frame).unwrap();
    });
    let button = frame.widgets.get("panel:rename-sack")?;
    // Include the stock corner/rail overhang as well as the label. The actual
    // recorded quads carry the opacity sent to the GPU; self.alpha is not evidence.
    let bounds = button.paint_bounds.rect().expand(8.0);
    let alphas: Vec<_> = frame
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh)
                if bounds.contains_rect(mesh.calc_bounds())
                    && shape.clip_rect.intersects(mesh.calc_bounds()) =>
            {
                Some(mesh.vertices.iter().map(|vertex| vertex.color.a()))
            }
            _ => None,
        })
        .flatten()
        .collect();
    // Ten non-space letters plus the background, two corners and rail.
    assert!(alphas.len() >= 14 * 4, "complete Rename button and label must be painted");
    Some((button.label.clone(), alphas))
}

#[test]
fn rename_sack_paint_fades_from_its_visible_opacity_when_inventory_closes() {
    let fixture = Fixture::new();
    // Inventory reveals 0.025 per 10 ms. Interrupting its first 100 ms must
    // fade the visible quarter-opacity surface, without flashing to full brightness.
    for (close_at, before, samples, closed_at) in [
        (100.0, 64_u8, vec![(100.0, 64_u8), (150.0, 32_u8)], 200.0),
        (
            500.0,
            255_u8,
            vec![(500.0, 255_u8), (600.0, 191_u8), (700.0, 128_u8), (800.0, 64_u8)],
            900.0,
        ),
    ] {
        let mut panels = panels();
        panels.show(PlayerPanel::Inventory, 0.0).unwrap();
        update(&mut panels, &fixture, close_at, Some(PanelEvent::Activate(Target::Item(100))));
        assert!(
            rename_paint(&mut panels).unwrap().1.iter().all(|alpha| alpha.abs_diff(before) <= 1)
        );
        update(&mut panels, &fixture, close_at, Some(key("Escape", false)));
        for (now, expected) in samples {
            update(&mut panels, &fixture, now, None);
            let (_, alphas) =
                rename_paint(&mut panels).expect("closing Inventory still owns Rename");
            assert!(
                alphas.iter().all(|alpha| alpha.abs_diff(expected) <= 1),
                "close at {close_at} ms, frame {now} ms: expected alpha {expected}, got {alphas:?}"
            );
        }
        update(&mut panels, &fixture, closed_at, None);
        assert!(rename_paint(&mut panels).is_none(), "Rename retires with Inventory");
        assert_eq!(panels.panel(), None);
        assert!(panels.take_actions().contains(&PlayerPanelAction::Close));
    }
}

#[test]
fn interrupted_books_retire_at_zero_while_skill_selectors_close_immediately() {
    let fixture = Fixture::new();
    for panel in [PlayerPanel::Inventory, PlayerPanel::Skills] {
        let mut panels = panels();
        panels.show(panel, 0.0).unwrap();
        update(&mut panels, &fixture, 100.0, Some(key("Escape", false)));
        update(&mut panels, &fixture, 199.0, None);
        assert_eq!(panels.panel(), Some(panel), "the final nonzero tick still owns the book");
        update(&mut panels, &fixture, 200.0, None);
        assert_eq!(panels.panel(), None, "the interrupted book retires at zero");
        assert!(panels.take_actions().contains(&PlayerPanelAction::Close));
    }
    for selector in [
        HudSkillSelectorTarget::Primary,
        HudSkillSelectorTarget::ConcentrationA,
        HudSkillSelectorTarget::ConcentrationB,
    ] {
        let mut panels = panels();
        panels.show(PlayerPanel::SkillSelector(selector), 0.0).unwrap();
        update(&mut panels, &fixture, 100.0, Some(key("Escape", false)));
        assert_eq!(panels.panel(), None, "{selector:?} has no book fade");
        assert!(panels.take_actions().contains(&PlayerPanelAction::Close));
    }
}

#[test]
fn rename_sack_paint_retires_during_page_motion_and_uses_the_settled_owner() {
    let mut fixture = Fixture::new();
    fixture.economy.backpack[0].name = "Outer".into();
    fixture.economy.backpack[0].contents.as_mut().unwrap()[0].name = "Inner".into();
    for panel in [PlayerPanel::Inventory, PlayerPanel::Service(HubTraderId::Luthacus)] {
        let mut panels = panels();
        panels.show(panel, 0.0).unwrap();
        for (id, now, name) in [(100, 500.0, "Outer"), (101, 1000.0, "Inner")] {
            update(&mut panels, &fixture, now, Some(PanelEvent::Activate(Target::Item(id))));
            assert_eq!(rename_paint(&mut panels).unwrap().0, format!("Rename {name}"));
            update(&mut panels, &fixture, now + 10.0, Some(PanelEvent::Activate(Target::Item(id))));
            assert!(rename_paint(&mut panels).is_none(), "old page command retires at entry");
            update(&mut panels, &fixture, now + 200.0, None);
            assert!(rename_paint(&mut panels).is_none(), "no Rename during page motion");
            update(&mut panels, &fixture, now + 380.0, None);
            assert_eq!(rename_paint(&mut panels).unwrap().0, format!("Rename {name}"));
        }
        update(&mut panels, &fixture, 1500.0, Some(key("KeyI", false)));
        assert!(rename_paint(&mut panels).is_none(), "child command retires on return");
        update(&mut panels, &fixture, 1870.0, None);
        assert_eq!(rename_paint(&mut panels).unwrap().0, "Rename Outer");
    }
}

#[test]
fn controller_scope_is_the_layer_holding_the_dialogue_controls() {
    // Controller focus and A only search the scope's layer, so a dialogue that claims
    // another layer leaves every controller input with nothing to reach.
    let fixture = Fixture::new();
    let mut panels = panels();
    let context = HubDialogueContext {
        interaction: HubInteractionId::ArchChancellor,
        story_office: true,
        eulogy_override: None,
        random_index: 0,
    };
    panels.show_college_intro_dialogue(context, &fixture.economy.npc, 0.0).unwrap();
    // The first speech, then its questions after Skip.
    for (now, skip, control) in
        [(10.0, false, "panel:dialogue-next"), (20.0, true, "panel:dialogue-choice:0")]
    {
        if skip {
            update(&mut panels, &fixture, now, Some(PanelEvent::Activate(Target::DialogueNext)));
        }
        update(&mut panels, &fixture, now, None);
        let ctx = egui::Context::default();
        let frame = crate::gpu_ui::record_frame(&ctx, &STOCK_PANEL_IMAGES, |frame| {
            panels.paint(frame).unwrap();
        });
        let (layer, _) = frame.input_scope.expect("the open dialogue claims the controller");
        assert_eq!(
            frame.widgets[control].layer_id,
            Some(layer),
            "{control} is in the controller scope"
        );
    }
}

#[test]
fn compact_panel_navigation_paints_through_portrait_rotation_for_every_book_and_service() {
    let fixture = Fixture::new();
    let images = [
        STOCK_PANEL_IMAGES.as_slice(),
        &[
            "assets/stock/menu/main-menu-button.png",
            "assets/stock/menu/main-menu-button-hover.png",
            "assets/stock/menu/main-menu-button-corner.png",
            "assets/stock/menu/main-menu-button-rail.png",
        ],
    ]
    .concat();
    for (panel, boasts_open) in [
        (PlayerPanel::Inventory, false),
        (PlayerPanel::Skills, false),
        (PlayerPanel::Skills, true),
        (PlayerPanel::Service(HubTraderId::Fomentius), false),
        (PlayerPanel::Service(HubTraderId::Hagatha), false),
        (PlayerPanel::Service(HubTraderId::Luthacus), false),
        (PlayerPanel::Service(HubTraderId::Shlorio), false),
    ] {
        let mut panels = panels();
        let ctx = egui::Context::default();
        panels.show(panel, 0.0).unwrap();
        panels.boasts_open = boasts_open;
        let actions: &[&str] = match panel {
            PlayerPanel::Skills => &["view-previous", "view-next", "boasts", "inventory", "close"],
            PlayerPanel::Service(_) => {
                &["view-shop", "view-backpack", "view-equipment", "view-stats", "skills", "close"]
            }
            _ => &["view-stats", "view-backpack", "view-equipment", "skills", "close"],
        };
        // Open directly in portrait, rotate to landscape and back, then include
        // the narrower supported phone width without replacing the panel owner.
        for (width, height) in [(390, 844), (844, 390), (390, 844), (320, 568)] {
            panels.update(&fixture.snapshot(), 500.0, width, height).unwrap();
            panels.take_actions();
            let frame = crate::gpu_ui::record_frame(&ctx, &images, |frame| {
                panels.paint(frame).unwrap();
            });
            assert_eq!(frame.widgets["panel:navigation"].paint_bounds.rect().height(), 48.0);
            let mut visible = 0;
            for action in actions {
                let Some(control) = frame.widgets.get(&format!("panel:{action}")) else {
                    continue; // The scroller omits offscreen semantic controls.
                };
                visible += 1;
                let bounds = control.paint_bounds.rect();
                assert_eq!(bounds.height(), 40.0, "{panel:?} {width} {action}");
                assert!(bounds.width() >= 128.0, "{panel:?} {width} {action} keeps readable ink");
                if width == 844 {
                    assert!(
                        bounds.left() >= 6.0 && bounds.right() <= 838.01,
                        "{panel:?} {action} preserves the complete landscape toolbar"
                    );
                }
            }
            assert!(visible >= 2, "{panel:?} {width} exposes usable section controls");
            if width == 844 {
                assert_eq!(visible, actions.len(), "{panel:?} retains every landscape action");
            }
            assert_eq!(panels.panel(), Some(panel));
            assert!(
                panels.take_actions().is_empty(),
                "painting/resizing cannot activate navigation"
            );
        }
    }
}

#[test]
fn compact_navigation_labels_retain_at_least_half_the_native_font_size() {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(128.0, 40.0));
    for label in
        ["Stats", "Backpack", "Equip", "Skills", "Done", "Shop", "Prev", "Next", "Boasts", "Items"]
    {
        for pressed in [false, true] {
            let spec = crate::menu::widgets::stock_button_label_spec(
                label,
                rect,
                40.0 / 69.0,
                40.0 / 69.0,
                pressed,
                1.0,
            );
            assert!(spec.scale >= 0.5, "{label} must not be compressed into unreadable ink");
        }
    }
}
