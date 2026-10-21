//! All current secondary constructors and admission paths against the public Website tick.
#![cfg(feature = "persistence")]

#[path = "support/secondary_current.rs"]
mod current;
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;

#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod support;

use current::{result_snapshot, state_snapshot};
use native_bits::source_value;
use serde_json::Value;
use solomon_sim::HubWizard;
use solomon_sim::belt::{NativeBeltCommand, PlayerBeltComponent};
use solomon_sim::economy::HubEconomyState;
use solomon_sim::profile::WizardElement;
use solomon_sim::secondary::*;
use support::*;

#[test]
fn all_twenty_three_secondary_constructors_and_admission_paths_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-cast-goldens.json")).unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let declared = &case["authority"];
        let book = read_book(&declared["skillBook"]);
        let skill_id = case["skillId"].as_u64().unwrap() as usize;
        let mut belt = PlayerBeltComponent::new(&book);
        assert!(belt.apply(
            NativeBeltCommand::BindSkill { skill_id: Some(skill_id), slot: 0 },
            &book,
            &HubEconomyState::new(77, WizardElement::Ether)
        ));
        let mut wizard = HubWizard::at(point(&declared["character"]["position"]));
        wizard.set_facing(declared["character"]["headingIndex"].as_f64().unwrap() * 15.0);
        wizard.velocity = point(&declared["character"]["velocity"]);
        let authority = authority_from_fixture(declared, &book, &belt, &wizard);
        let mut state = read_state(&case["initial"]);
        let mut world = World {
            case,
            targets: case["targets"].as_array().unwrap().iter().map(target).collect(),
        };
        let players = std::collections::BTreeMap::from([("player".to_owned(), authority)]);
        let result = state.step(
            &NativeSecondaryTickContext { enhanced_effects: None, players: &players, tick: 1 },
            &mut world,
            None,
        );
        let context = case["name"].as_str().unwrap();
        assert_snapshot(
            &source_value(&result_snapshot(&result)),
            &source_value(&case["result"]),
            context,
        );
        assert_snapshot(
            &source_value(&state_snapshot(&state)),
            &source_value(&case["state"]),
            context,
        );
    }
}
