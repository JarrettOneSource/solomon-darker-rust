//! Actual host mana previews retain authored and rank-zero upgrade rows.

use serde::Deserialize;
use solomon_sim::profile::offensive::OffensiveSpellFactors;
use solomon_sim::profile::{
    PlayerStatBookComponent, SkillBookState, WizardDiscipline, WizardElement,
};
use solomon_sim::secondary::{SecondaryAbilityId, secondary_ability_mana_cost_with_stat_book};

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
        let mut book = SkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
        book.effective_ranks.copy_from_slice(&case.ranks);
        for (skill_id, expected) in case.costs {
            let actual = secondary_ability_mana_cost_with_stat_book(
                &book,
                &case.stat_book,
                SecondaryAbilityId::from_skill_id(skill_id).unwrap(),
                &OffensiveSpellFactors::default(),
            );
            assert_eq!(
                actual, expected,
                "skill{skill_id} authored{} upgrade{}",
                case.authored, case.upgrade_rank
            );
        }
    }
}
