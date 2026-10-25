//! Complete v7 descriptor rows over canonical native and authored stat books.

use serde_json::Value;
use solomon_sim::ml_bot::{
    MlBotPolicySkillDescriptorInput, describe_ml_bot_policy_skill, ml_bot_policy_skill_coverage_key,
};
use solomon_sim::profile::{
    NativeSkillBookState, NativeSkillStatBookValue, PlayerStatBookComponent, WizardDiscipline,
    WizardElement,
};

#[test]
fn all_native_skill_and_weld_descriptors_match_source_rows_and_boundaries() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/ml-bot-skills-goldens.json")).unwrap();
    let mut book = NativeSkillBookState::new(WizardElement::Fire, WizardDiscipline::Arcane);
    for (index, value) in fixture["book"]["permanentRanks"].as_array().unwrap().iter().enumerate() {
        book.permanent_ranks[index] = value.as_u64().unwrap() as u32;
    }
    for (index, value) in fixture["book"]["effectiveRanks"].as_array().unwrap().iter().enumerate() {
        book.effective_ranks[index] = value.as_u64().unwrap() as u32;
    }
    for case in fixture["cases"].as_array().unwrap() {
        let input = &case["input"];
        let input = MlBotPolicySkillDescriptorInput {
            apply_count: input["applyCount"].as_f64().unwrap(),
            skill_id: input["skillId"].as_u64().unwrap() as usize,
            target_rank: input["targetRank"].as_f64().unwrap(),
            weld_build_id: input.get("weldBuildId").map(|value| value.as_u64().unwrap() as u32),
        };
        let mut stat_book = PlayerStatBookComponent::default();
        if let Some(overrides) = case["override"].as_object() {
            for (name, value) in overrides {
                let property = if let Some(values) = value.as_array() {
                    NativeSkillStatBookValue::Ranks(
                        values.iter().map(|value| value.as_f64().unwrap()).collect(),
                    )
                } else {
                    NativeSkillStatBookValue::Scalar(value.as_f64().unwrap())
                };
                std::sync::Arc::make_mut(&mut stat_book.entries)[input.skill_id]
                    .numeric_properties
                    .insert(name.clone(), property);
            }
        }
        let result = describe_ml_bot_policy_skill(&book, &stat_book, input);
        if let Some(error) = case["error"].as_str() {
            assert_eq!(result.unwrap_err(), error, "{}", case["name"]);
            continue;
        }
        let actual = result.unwrap();
        let expected = case["output"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "{}", case["name"]);
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                actual.to_bits(),
                (expected.as_f64().unwrap() as f32).to_bits(),
                "{}[{index}]",
                case["name"]
            );
        }
        assert_eq!(
            ml_bot_policy_skill_coverage_key(input.skill_id, input.weld_build_id),
            case["coverageKey"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}
