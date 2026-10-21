//! Revelation's secondary rank admission uses the canonical effective-rank producer.

use serde_json::Value;
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};
use solomon_sim::secondary::{
    NativeSecondaryAbilityId, effective_secondary_ability_rank_stats,
    native_secondary_ability_rank_stats,
};

#[test]
fn every_current_secondary_rank_including_single_rank_revelation_replays_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-secondary-rank-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let skill_id = case["skillId"].as_u64().unwrap() as usize;
        let ability = NativeSecondaryAbilityId::from_skill_id(skill_id).unwrap();
        let rank = case["rank"].as_u64().unwrap() as u32;
        let mut book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
        book.effective_ranks[skill_id] = rank;
        let stats = effective_secondary_ability_rank_stats(&book, ability);
        assert_eq!(stats, native_secondary_ability_rank_stats(ability, rank));
        let expected = case["values"].as_object().unwrap();
        assert_eq!(stats.values.len(), expected.len(), "skill{skill_id} rank{rank} properties");
        for property in stats.values {
            assert_eq!(
                Some(property.value),
                expected[property.name].as_f64(),
                "skill{skill_id} rank{rank} {}",
                property.name
            );
        }
    }
}
