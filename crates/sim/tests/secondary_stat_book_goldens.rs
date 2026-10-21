//! Actual host mana previews retain authored and rank-zero upgrade rows.

use serde::Deserialize;
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::{
    NativeSkillBookState, PlayerStatBookComponent, WizardDiscipline, WizardElement,
};
use solomon_sim::secondary::{
    NativeSecondaryAbilityId, native_secondary_ability_mana_cost_with_stat_book,
};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    authored: bool,
    upgrade_rank: u32,
    ranks: Vec<u32>,
    stat_book: PlayerStatBookComponent,
    costs: Vec<(usize, f64)>,
}

#[test]
fn secondary_mana_previews_use_actual_player_stat_book_supplements() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/secondary-stat-book-goldens.json"))
            .unwrap();
    for case in fixture.cases {
        let mut book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
        book.effective_ranks.copy_from_slice(&case.ranks);
        for (skill_id, expected) in case.costs {
            let actual = native_secondary_ability_mana_cost_with_stat_book(
                &book,
                &case.stat_book,
                NativeSecondaryAbilityId::from_skill_id(skill_id).unwrap(),
                &NativeOffensiveSpellFactors::default(),
            );
            assert_eq!(
                actual, expected,
                "skill{skill_id} authored{} upgrade{}",
                case.authored, case.upgrade_rank
            );
        }
    }
}
