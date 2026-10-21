use serde::Deserialize;
use solomon_sim::economy::{EquipmentSlot, HubEconomyRejection, HubInventoryAction};
use solomon_sim::player_panels::{
    format_native_skill_tooltip_line, native_inventory_rejection_notice,
    native_skill_book_page_placements,
};
use solomon_sim::profile::{
    NATIVE_SKILL_CATALOG, NativeSkillBookState, WizardDiscipline, WizardElement,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    cases: Vec<Case>,
    formatted_stats: Vec<Stat>,
    rejection_notices: Vec<Notice>,
}
#[derive(Deserialize)]
struct Notice {
    action: String,
    reason: String,
    selector: Option<i32>,
    slot: Option<String>,
    title: String,
    body: String,
}

#[test]
fn inventory_rejections_use_authored_notices_and_keep_ordinary_failures_non_modal() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/player-panels-goldens.json")).unwrap();
    for expected in fixture.rejection_notices {
        let action = match expected.action.as_str() {
            "dowse" => HubInventoryAction::Dowse { reference_item_id: None },
            "buy-hagatha" => {
                HubInventoryAction::BuyHagatha { selector: expected.selector.unwrap() }
            }
            "unequip" => HubInventoryAction::Unequip {
                slot: EquipmentSlot::ALL
                    .into_iter()
                    .find(|slot| Some(slot.as_str()) == expected.slot.as_deref())
                    .unwrap(),
                destination_sack_id: None,
                destination_slot: None,
            },
            _ => panic!("unknown source action"),
        };
        let reason = match expected.reason.as_str() {
            "insufficient-gold" => HubEconomyRejection::InsufficientGold,
            "perk-capacity-full" => HubEconomyRejection::PerkCapacityFull,
            "required-clothing" => HubEconomyRejection::RequiredClothing,
            _ => panic!("unknown source rejection"),
        };
        let actual = native_inventory_rejection_notice(&action, reason).unwrap();
        assert_eq!(actual.title, expected.title);
        assert_eq!(actual.body, expected.body);
    }
    let action = HubInventoryAction::Move {
        item_id: 1,
        destination_sack_id: None,
        destination_slot: Some(0),
    };
    for reason in [
        HubEconomyRejection::CapacityFull,
        HubEconomyRejection::InvalidTarget,
        HubEconomyRejection::IneligibleItem,
        HubEconomyRejection::InvalidInventory,
        HubEconomyRejection::ItemNotFound,
        HubEconomyRejection::SlotLocked,
    ] {
        assert!(
            native_inventory_rejection_notice(&action, reason).is_none(),
            "{reason:?} uses native bad-action feedback, not reason-code copy"
        );
    }
}
#[derive(Deserialize)]
struct Stat {
    id: usize,
    rank: u32,
    source: String,
    expected: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    element: String,
    discipline: String,
    extra_skills: Vec<usize>,
    placements: Vec<Placement>,
}

#[test]
fn authored_tooltip_templates_match_website_formatting() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/player-panels-goldens.json")).unwrap();
    for stat in fixture.formatted_stats {
        assert_eq!(
            format_native_skill_tooltip_line(
                &stat.source,
                &NATIVE_SKILL_CATALOG[stat.id],
                stat.rank
            ),
            stat.expected,
            "{} rank {}: {}",
            stat.id,
            stat.rank,
            stat.source
        );
    }
}
#[derive(Debug, Deserialize, PartialEq)]
struct Placement {
    root: usize,
    skills: Vec<usize>,
    width: f64,
    x: f64,
    y: f64,
}

#[test]
fn learned_dependency_pages_match_the_website_in_order_and_geometry() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/player-panels-goldens.json")).unwrap();
    for case in fixture.cases {
        let element =
            WizardElement::ALL.into_iter().find(|value| value.as_str() == case.element).unwrap();
        let discipline = WizardDiscipline::ALL
            .into_iter()
            .find(|value| value.as_str() == case.discipline)
            .unwrap();
        let mut book = NativeSkillBookState::new(element, discipline);
        for id in case.extra_skills {
            book.grant_ranks(id, 1);
        }
        let actual: Vec<Placement> = native_skill_book_page_placements(&book)
            .into_iter()
            .map(|page| Placement {
                root: page.root_skill_id,
                skills: page.skills,
                width: page.width,
                x: page.x,
                y: page.y,
            })
            .collect();
        assert_eq!(actual, case.placements, "{} {}", case.element, case.discipline);
    }
}
