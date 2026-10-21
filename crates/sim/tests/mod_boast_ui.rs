#[cfg(feature = "persistence")]
#[test]
fn source_native_and_mod_boast_selectors() {
    use solomon_sim::economy::{BoastSelection, BoastState, HubInventoryAction, NativeHubNpcState};
    use solomon_sim::hub::interaction::{
        HubNpcChatContent, HubNpcChatNext, HubNpcListSelector, HubNpcSelectorId,
        HubNpcSelectorPresentation, hub_boast_failure_text, hub_boast_instruction,
        hub_npc_selector_action_with_mods, hub_npc_selector_response_with_mods,
        hub_npc_selector_rows_with_mods,
    };
    use solomon_sim::modding::{LuaObject, projection::ModContentProjection};

    fn metadata(value: &serde_json::Value) -> Option<ModContentProjection> {
        if value.is_null() {
            return None;
        }
        let value: LuaObject = serde_json::from_value(value.clone()).unwrap();
        Some(ModContentProjection::read(&value).unwrap())
    }
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/website/mod-boast-ui-goldens.json")).unwrap();
    for case in fixture["selectors"].as_array().unwrap() {
        let selector = match case["selector"].as_str().unwrap() {
            "boast" => HubNpcListSelector::Boast,
            "books" => HubNpcListSelector::Books,
            "teacher-spells" => HubNpcListSelector::TeacherSpells,
            _ => unreachable!(),
        };
        let mods = metadata(&case["metadata"]);
        let npc = NativeHubNpcState {
            librarian_lace_read: case["read"].as_bool().unwrap(),
            ..NativeHubNpcState::default()
        };
        let advanced: [bool; 8] = serde_json::from_value(case["advancedUnlocks"].clone()).unwrap();
        let expected: Vec<HubNpcSelectorPresentation> =
            serde_json::from_value(case["rows"].clone()).unwrap();
        assert_eq!(
            hub_npc_selector_rows_with_mods(selector, &npc, &advanced, mods.as_ref()),
            expected
        );
        for row in case["responses"].as_array().unwrap() {
            let id: HubNpcSelectorId = serde_json::from_value(row["id"].clone()).unwrap();
            let action = hub_npc_selector_action_with_mods(selector, &id).unwrap();
            match action {
                HubInventoryAction::SelectBoast { boast_id } => {
                    let expected: BoastSelection =
                        serde_json::from_value(row["action"]["boastId"].clone()).unwrap();
                    assert_eq!(boast_id, expected);
                }
                HubInventoryAction::ReadLibrarianBook { book_id } => {
                    assert_eq!(Some(u64::from(book_id)), row["action"]["bookId"].as_u64())
                }
                HubInventoryAction::BuyTeacherSpell { skill_id } => {
                    assert_eq!(Some(skill_id as u64), row["action"]["skillId"].as_u64())
                }
                _ => unreachable!(),
            }
            let expected = &row["response"];
            let actual = hub_npc_selector_response_with_mods(selector, &id, mods.as_ref());
            assert_eq!(actual.is_none(), expected.is_null());
            if let Some(HubNpcChatContent::Speech { key, lines, next }) = actual {
                assert_eq!(Some(key.as_str()), expected["key"].as_str());
                assert_eq!(serde_json::json!(lines), expected["lines"]);
                assert_eq!(
                    Some(match next {
                        HubNpcChatNext::Close => "close",
                        HubNpcChatNext::Choices => "choices",
                    }),
                    expected["next"].as_str()
                );
            }
        }
    }
    for case in fixture["selections"].as_array().unwrap() {
        let mods = metadata(&case["metadata"]);
        let selected: Option<BoastSelection> =
            serde_json::from_value(case["selected"].clone()).unwrap();
        let state = BoastState {
            failed: case["failed"].as_bool().unwrap(),
            selected,
            ..BoastState::default()
        };
        assert_eq!(
            hub_boast_instruction(state.selected.as_ref(), mods.as_ref()),
            case["instruction"].as_str()
        );
        assert_eq!(
            hub_boast_failure_text(&state, mods.as_ref()).as_deref(),
            case["failure"].as_str()
        );
    }
}
