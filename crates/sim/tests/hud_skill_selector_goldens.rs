//! Source HUD target admission, learned option filtering and native strip placement.
use serde_json::Value;
use solomon_sim::hud::skill_selector::*;
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};

#[test]
fn hud_selectors_exclude_the_other_slot_unlearned_welds_and_exhausted_skills() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hud-skill-selector-goldens.json"))
            .unwrap();
    for case in fixture["options"].as_array().unwrap() {
        let target =
            NativeHudSkillSelectorTarget::from_binding(case["binding"].as_u64().unwrap() as usize)
                .unwrap();
        let slots = std::array::from_fn(|slot| case["slots"][slot].as_u64().map(|id| id as usize));
        let mut book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
        book.learned_skill_order.clear();
        book.permanent_ranks.fill(0);
        book.effective_ranks.fill(0);
        for row in case["learnedSkills"].as_array().unwrap() {
            let id = row[0].as_u64().unwrap() as usize;
            book.learned_skill_order.push(id);
            book.permanent_ranks[id] = row[1].as_u64().unwrap() as u32;
            book.effective_ranks[id] = row[2].as_u64().unwrap() as u32;
        }
        book.weld_build_id = case["weldBuildId"].as_u64().map(|id| id as u32);
        assert_eq!(target.title(), case["title"].as_str().unwrap());
        let actual = native_hud_skill_selector_options(&book, &slots, target);
        let expected = case["options"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.skill_id, expected["skillId"].as_u64().unwrap() as usize);
            assert_eq!(actual.icon_record, expected["iconRecord"].as_u64().unwrap() as usize);
            assert_eq!(actual.name, expected["name"].as_str().unwrap());
        }
    }
    for case in fixture["layouts"].as_array().unwrap() {
        let actual = native_hud_skill_selector_layout(
            case["count"].as_u64().unwrap() as usize,
            case["width"].as_f64().unwrap(),
        );
        let expected = &case["layout"];
        for (actual, key) in [
            (actual.panel.x, "panelLeft"),
            (actual.panel.y, "panelTop"),
            (actual.panel.width, "panelWidth"),
            (actual.panel.height, "panelHeight"),
            (actual.option_left, "optionLeft"),
            (actual.option_top, "optionTop"),
            (actual.strip_width, "stripWidth"),
            (actual.title_y, "titleY"),
        ] {
            assert_eq!(actual, expected[key].as_f64().unwrap(), "{key}");
        }
    }
}
