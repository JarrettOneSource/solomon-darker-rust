use serde_json::Value;
use solomon_sim::economy::{
    FOMENTIUS_STOCK_DEFINITIONS, HubEconomyState, HubItemKind, create_fomentius_inventory_item,
};
use solomon_sim::hud::HudRect;
use solomon_sim::player_panels::sack_rename::{
    InventorySackRenameOwner, InventorySackRenameSelection, inventory_sack_name_needs_browser_font,
    inventory_sack_rename_dialog_layout, inventory_sack_rename_target,
};
use solomon_sim::profile::WizardElement;

#[test]
fn sack_rename_selection_readable_dialog_and_unicode_detection_match_current_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/sack-rename-goldens.json")).unwrap();
    let potion = create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[0], 1, 1);
    let definition =
        FOMENTIUS_STOCK_DEFINITIONS.iter().find(|row| row.kind == HubItemKind::Sack).unwrap();
    let mut inner = create_fomentius_inventory_item(definition, 2, 1);
    inner.contents = Some(vec![potion]);
    let mut outer = create_fomentius_inventory_item(definition, 3, 1);
    outer.contents = Some(vec![inner]);
    let mut stored = create_fomentius_inventory_item(definition, 4, 1);
    stored.contents = Some(Vec::new());
    let mut economy = HubEconomyState::new(1, WizardElement::Ether);
    economy.backpack = vec![outer];
    economy.storage = vec![stored];
    for case in fixture["targets"].as_array().unwrap() {
        let selection = if case["selection"].is_null() {
            None
        } else {
            let value = &case["selection"];
            let owner = match value["owner"].as_str() {
                Some("backpack") => InventorySackRenameOwner::Backpack,
                Some("storage") => InventorySackRenameOwner::Storage,
                Some("equipment") => InventorySackRenameOwner::Equipment,
                None => InventorySackRenameOwner::Unowned,
                other => panic!("unknown sack selection owner {other:?}"),
            };
            Some(InventorySackRenameSelection { item_id: value["id"].as_u64().unwrap(), owner })
        };
        let path: Vec<_> =
            case["path"].as_array().unwrap().iter().map(|id| id.as_u64().unwrap()).collect();
        assert_eq!(
            inventory_sack_rename_target(&economy, selection, &path).map(|item| item.id),
            case["expected"].as_u64(),
            "{case}"
        );
    }
    for case in fixture["layouts"].as_array().unwrap() {
        let layout = inventory_sack_rename_dialog_layout(
            case["size"]["width"].as_f64().unwrap(),
            case["size"]["height"].as_f64().unwrap(),
            fixture["headerWidth"].as_f64().unwrap(),
            fixture["arrowHeight"].as_f64().unwrap(),
        );
        let expected = &case["expected"];
        assert_eq!(layout.art_scale, expected["artScale"].as_f64().unwrap());
        assert_eq!(layout.button_scale, expected["buttonScale"].as_f64().unwrap());
        assert_eq!(layout.title_baseline, expected["titleBaseline"].as_f64().unwrap());
        assert_eq!(layout.limit_baseline, expected["limitBaseline"].as_f64().unwrap());
        for (rect, key) in [
            (layout.body, "body"),
            (layout.cancel, "cancel"),
            (layout.field, "field"),
            (layout.save, "save"),
        ] {
            assert_rect(rect, &expected[key]);
        }
    }
    for case in fixture["names"].as_array().unwrap() {
        assert_eq!(
            inventory_sack_name_needs_browser_font(case["name"].as_str().unwrap()),
            case["expected"].as_bool().unwrap(),
            "{case}"
        );
    }
}

fn assert_rect(rect: HudRect, expected: &Value) {
    assert_eq!(rect.x, expected["left"].as_f64().unwrap());
    assert_eq!(rect.y, expected["top"].as_f64().unwrap());
    assert_eq!(rect.width, expected["width"].as_f64().unwrap());
    assert_eq!(rect.height, expected["height"].as_f64().unwrap());
}
