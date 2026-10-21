//! Current Create roots preserve primary-owned starters and native class titles.

use serde_json::{Value, json};
use solomon_sim::NativeRng;
use solomon_sim::profile::create::{
    NATIVE_CREATE_ENTRY_FADE_MS, NATIVE_CREATE_RANDOMIZE_FINALIZE_MS, NativeCreateLoadoutSelection,
    finalize_native_create_loadout,
};
use solomon_sim::profile::{
    NativeSkillBookState, WizardDiscipline, WizardElement, native_wizard_class_title,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/create-loadout-goldens.json")).unwrap()
}

fn element(value: &Value) -> Option<WizardElement> {
    value.as_str().map(|value| {
        WizardElement::ALL.into_iter().find(|element| element.as_str() == value).unwrap()
    })
}

fn discipline(value: &Value) -> Option<WizardDiscipline> {
    value.as_str().map(|value| {
        WizardDiscipline::ALL.into_iter().find(|discipline| discipline.as_str() == value).unwrap()
    })
}

#[test]
fn all_current_create_root_pairs_preserve_their_source_book_and_class() {
    for case in fixture()["identities"].as_array().unwrap() {
        let element = WizardElement::ALL
            .into_iter()
            .find(|element| Some(element.as_str()) == case["config"]["element"].as_str())
            .unwrap();
        let discipline = WizardDiscipline::ALL
            .into_iter()
            .find(|discipline| Some(discipline.as_str()) == case["config"]["discipline"].as_str())
            .unwrap_or_else(|| panic!("unsupported current root: {}", case["config"]));
        let book = NativeSkillBookState::new(element, discipline);
        assert_eq!(
            json!({
                "advancedUnlocks": book.advanced_unlocks,
                "disciplineRoot": book.discipline_root,
                "effectiveRanks": &book.effective_ranks[..],
                "elementRoot": book.element_root,
                "learnedSkillOrder": book.learned_skill_order,
                "permanentRanks": &book.permanent_ranks[..],
                "primarySkillId": book.primary_skill_id,
                "weldBuildId": book.weld_build_id,
                "weldComponentRanks": book.weld_component_ranks,
            }),
            case["book"],
            "{}",
            case["config"],
        );
        assert_eq!(native_wizard_class_title(element, discipline), case["classTitle"]);
    }
}

#[test]
fn current_create_finalization_preserves_missing_choice_draw_order() {
    let fixture = fixture();
    assert_eq!(NATIVE_CREATE_ENTRY_FADE_MS, fixture["entryFadeMs"]);
    assert_eq!(NATIVE_CREATE_RANDOMIZE_FINALIZE_MS, fixture["randomizeFinalizeMs"]);
    for case in fixture["finalizations"].as_array().unwrap() {
        let source = &case["state"];
        let mut rng = NativeRng::new(0);
        rng.index_a = source["indexA"].as_u64().unwrap() as usize;
        rng.index_b = source["indexB"].as_u64().unwrap() as usize;
        for (index, word) in source["words"].as_array().unwrap().iter().enumerate() {
            rng.words[index] = word.as_u64().unwrap() as u32;
        }
        let selection = NativeCreateLoadoutSelection {
            name: case["selection"]["displayName"].as_str().unwrap().into(),
            element: element(&case["selection"]["element"]),
            discipline: discipline(&case["selection"]["discipline"]),
        };
        let disabled = case["disabled"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        let config = finalize_native_create_loadout(&mut rng, &selection, &disabled);
        assert_eq!(
            json!({
                "config": {
                    "displayName": config.name,
                    "element": config.element.as_str(),
                    "discipline": config.discipline.as_str(),
                },
                "state": {
                    "indexA": rng.index_a,
                    "indexB": rng.index_b,
                    "words": &rng.words[..],
                },
            }),
            case["result"],
            "{}",
            case["name"],
        );
    }
}
