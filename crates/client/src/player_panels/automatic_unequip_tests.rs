//! Report 50's requested automatic destination through the real panel owner.
//! The OS subscription is detached; stock bundles, accepted inventory, panel
//! updates, emitted commands and canonical economy transactions remain real.
use super::*;
use solomon_sim::economy::{
    EQUIPMENT_RECIPES, EquipmentType, FOMENTIUS_STOCK_DEFINITIONS, HubEconomyContext,
    HubInventoryItem, create_equipment_inventory_item, create_fomentius_inventory_item,
    project_inventory_items,
};
use solomon_sim::profile::{WizardDiscipline, WizardElement};

struct Fixture {
    selection: NewGameSelection,
    book: SkillBookState,
    stat_book: PlayerStatBookComponent,
    economy: HubEconomyState,
    belt: PlayerBeltComponent,
    progression: PlayerProgressionState,
    resources: PlayerCombatState,
    offensive: OffensiveSpellFactors,
}

fn potion(id: u64, slot: usize) -> HubInventoryItem {
    let mut item = create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[0], id, 1);
    item.inventory_slot = Some(slot);
    item
}

fn sack(id: u64, slot: usize, contents: Vec<HubInventoryItem>) -> HubInventoryItem {
    let mut item = create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[5], id, 1);
    item.inventory_slot = Some(slot);
    item.contents = Some(contents);
    item
}

impl Fixture {
    fn new(slot: EquipmentSlot, kind: EquipmentType) -> Self {
        let element = WizardElement::Fire;
        let discipline = WizardDiscipline::Arcane;
        let mut economy = HubEconomyState::new(1, element);
        economy.backpack = vec![
            potion(102, 0),
            potion(103, 2),
            sack(100, 6, vec![sack(101, 0, vec![potion(105, 0), potion(106, 2)]), potion(104, 2)]),
        ];
        let recipe = EQUIPMENT_RECIPES.iter().find(|recipe| recipe.equipment_type == kind).unwrap();
        economy.equipment.set(slot, Some(create_equipment_inventory_item(recipe, 900)));
        economy.owned_perk_selectors = vec![19];
        let book = SkillBookState::new(element, discipline);
        let belt = PlayerBeltComponent::new(&book);
        Self {
            selection: NewGameSelection { name: "Report 50".into(), element, discipline },
            book,
            stat_book: PlayerStatBookComponent::default(),
            economy,
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
            dye_session_id: None,
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

fn panels(fixture: &Fixture, panel: PlayerPanel, depth: usize) -> PlayerPanels {
    solomon_graphics::assets::configure(
        solomon_graphics::assets::NativeAssets::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web"
        ))
        .unwrap(),
    );
    let mut panels =
        futures_lite::future::block_on(PlayerPanels::attach(HotbarSelection::for_test())).unwrap();
    panels.show(panel, 0.0).unwrap();
    for (index, id) in [100, 101].into_iter().take(depth).enumerate() {
        let now = (index + 1) as f64 * 500.0;
        update(&mut panels, fixture, now, Some(Target::Item(id)));
        update(&mut panels, fixture, now + 10.0, Some(Target::Item(id)));
        update(&mut panels, fixture, now + 380.0, None);
    }
    assert_eq!(panels.sack_path, [100, 101][..depth]);
    panels.take_actions();
    panels
}

fn update(panels: &mut PlayerPanels, fixture: &Fixture, now: f64, target: Option<Target>) {
    panels.events.extend(target.map(PanelEvent::Activate));
    panels.update(&fixture.snapshot(), now, 1600, 900).unwrap();
}

#[test]
fn automatic_unequip_uses_the_open_root_and_its_first_hole_for_every_removable_sink() {
    for panel in [
        PlayerPanel::Inventory,
        PlayerPanel::Service(HubTraderId::Fomentius),
        PlayerPanel::Service(HubTraderId::Hagatha),
        PlayerPanel::Service(HubTraderId::Luthacus),
        PlayerPanel::Service(HubTraderId::Shlorio),
    ] {
        for depth in 0..=2 {
            for (slot, kind) in [
                (EquipmentSlot::Ring0, EquipmentType::Ring),
                (EquipmentSlot::Ring1, EquipmentType::Ring),
                (EquipmentSlot::Ring2, EquipmentType::Ring),
                (EquipmentSlot::Amulet, EquipmentType::Amulet),
                (EquipmentSlot::Weapon, EquipmentType::Staff),
                (EquipmentSlot::Weapon, EquipmentType::Wand),
            ] {
                let mut fixture = Fixture::new(slot, kind);
                let mut panels = panels(&fixture, panel, depth);
                update(&mut panels, &fixture, 1500.0, Some(Target::Equipment(slot)));
                assert!(panels.take_actions().is_empty(), "the first activation selects gear");
                update(&mut panels, &fixture, 1510.0, Some(Target::Equipment(slot)));
                let actions = panels.take_actions();
                let [PlayerPanelAction::Inventory(action)] = actions.as_slice() else {
                    panic!("{panel:?} depth {depth} {kind:?}/{slot:?}: {actions:?}");
                };
                let accepted = fixture.economy.apply(action, &HubEconomyContext::default());
                assert!(accepted.accepted, "{panel:?} depth {depth} {kind:?}/{slot:?}");
                let placement: Vec<_> = project_inventory_items(&fixture.economy.backpack)
                    .into_iter()
                    .filter(|entry| entry.item.id == 900)
                    .map(|entry| (entry.depth, entry.parent_sack_id, entry.slot))
                    .collect();
                assert_eq!(
                    placement,
                    [(depth, [None, Some(100), Some(101)][depth], 1)],
                    "{panel:?} depth {depth} {kind:?}/{slot:?} must use its open root"
                );
                assert!(fixture.economy.equipment.at(slot).is_none());
                assert!(fixture.economy.inventory_is_valid());
                update(&mut panels, &fixture, 1520.0, None);
                assert_eq!(panels.sack_path, [100, 101][..depth]);
                assert!(
                    panels.take_actions().is_empty(),
                    "accepted feedback cannot repeat removal"
                );
            }
        }
    }
}

#[test]
fn automatic_unequip_keeps_required_clothing_and_page_transition_ownership() {
    for (slot, kind) in
        [(EquipmentSlot::Hat, EquipmentType::Hat), (EquipmentSlot::Robe, EquipmentType::Robe)]
    {
        let fixture = Fixture::new(slot, kind);
        let mut panels = panels(&fixture, PlayerPanel::Inventory, 2);
        update(&mut panels, &fixture, 1500.0, Some(Target::Equipment(slot)));
        update(&mut panels, &fixture, 1510.0, Some(Target::Equipment(slot)));
        assert!(panels.take_actions().is_empty());
        assert!(panels.notice.is_some(), "{slot:?} keeps its required-clothing notice");
        assert_eq!(fixture.economy.equipment.at(slot).as_ref().unwrap().id, 900);
    }

    let fixture = Fixture::new(EquipmentSlot::Ring0, EquipmentType::Ring);
    let mut panels = panels(&fixture, PlayerPanel::Inventory, 0);
    update(&mut panels, &fixture, 500.0, Some(Target::Item(100)));
    update(&mut panels, &fixture, 510.0, Some(Target::Item(100)));
    assert!(panels.inventory_transition_locked());
    update(&mut panels, &fixture, 520.0, Some(Target::Equipment(EquipmentSlot::Ring0)));
    update(&mut panels, &fixture, 530.0, Some(Target::Equipment(EquipmentSlot::Ring0)));
    assert!(panels.take_actions().is_empty(), "page motion owns input until settling");
    assert_eq!(fixture.economy.equipment.rings[0].as_ref().unwrap().id, 900);
}
