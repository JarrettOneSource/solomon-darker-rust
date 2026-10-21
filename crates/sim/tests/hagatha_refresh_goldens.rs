//! Owned Hagatha refresh preserves one-use charges and only reseeds actual acquisitions.

mod support;

use serde_json::{Value, json};
use solomon_sim::NativeRng;
use solomon_sim::player::PlayerState;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};

#[path = "support/player_snapshot.rs"]
mod player_snapshot;

fn snapshot(player: &PlayerState, rng: &NativeRng, granted: Option<usize>) -> Value {
    let book = &player.book;
    let progression = &player.progression;
    json!({
        "book": {
            "advancedUnlocks": book.advanced_unlocks,
            "disciplineRoot": book.discipline_root,
            "effectiveRanks": &book.effective_ranks[..],
            "elementRoot": book.element_root,
            "learnedSkillOrder": book.learned_skill_order,
            "permanentRanks": &book.permanent_ranks[..],
            "primarySkillId": book.primary_skill_id,
            "weldBuildId": book.weld_build_id,
            "weldComponentRanks": book.weld_component_ranks,
        },
        "belt": player.belt.entries.map(|entry| entry.map(|entry| match entry {
            solomon_sim::belt::NativeBeltEntry::Skill { skill_id } => json!({"kind": "skill", "skillId": skill_id}),
            solomon_sim::belt::NativeBeltEntry::HealthPotion => json!({"kind": "health-potion"}),
            solomon_sim::belt::NativeBeltEntry::ManaPotion => json!({"kind": "mana-potion"}),
            solomon_sim::belt::NativeBeltEntry::Item { .. } => unreachable!("this source journey binds no items"),
        })),
        "concentrations": [player.skill_runtime.concentration_skill_id_a, player.skill_runtime.concentration_skill_id_b],
        "gameRng": {"indexA": rng.index_a, "indexB": rng.index_b, "words": &rng.words[..]},
        "grantedSkillId": granted,
        "hagathaRuntime": {
            "cheatDeathCharges": progression.hagatha_runtime.cheat_death_charges,
            "reverieActive": progression.hagatha_runtime.reverie_active,
            "serendipityActive": progression.hagatha_runtime.serendipity_active,
        },
        "offerSeed": progression.offer_seed,
        "pendingOffer": player_snapshot::snapshot(player, rng)["progression"]["pendingOffer"],
        "progressionRevision": progression.revision,
    })
}

#[test]
fn owned_hagatha_refresh_matches_source_without_replaying_purchase_charges() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hagatha-refresh-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut player = PlayerState::new(
            NewGameSelection {
                name: "Retained".into(),
                element: WizardElement::Fire,
                discipline: WizardDiscipline::Body,
            },
            &mut rng,
        );
        if case["pending"].as_bool().unwrap() {
            player.grant_experience(95.0, &mut rng);
        }
        support::assert_website_json(
            &snapshot(&player, &rng, None),
            &case["initial"],
            "initial",
            &[],
        );
        player.economy.owned_perk_selectors = case["owned"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        for (index, expected) in case["steps"].as_array().unwrap().iter().enumerate() {
            let granted = player.refresh_hagatha_skill_effects(&mut rng);
            support::assert_website_json(
                &snapshot(&player, &rng, granted),
                expected,
                &format!("{} refresh {index}", case["name"]),
                &[],
            );
        }
    }
}

#[test]
fn replacing_a_loadout_retains_unlocks_and_rearms_owned_charges() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hagatha-refresh-goldens.json"))
            .unwrap();
    for (index, case) in fixture["loadouts"].as_array().unwrap().iter().enumerate() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut player = PlayerState::new(
            NewGameSelection {
                name: "Retained".into(),
                element: WizardElement::Fire,
                discipline: WizardDiscipline::Body,
            },
            &mut rng,
        );
        for skill_id in [72, 77] {
            assert!(player.book.unlock_advanced(skill_id));
        }
        player.economy.owned_perk_selectors = case["owned"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        support::assert_website_json(
            &snapshot(&player, &rng, None),
            &case["initial"],
            "initial",
            &[],
        );
        player.replace_loadout(
            NewGameSelection {
                name: "Next".into(),
                element: WizardElement::Water,
                discipline: WizardDiscipline::Fire,
            },
            case["offerSeed"].as_u64().unwrap() as u32,
            None,
        );
        support::assert_website_json(
            &snapshot(&player, &rng, None),
            &case["replaced"],
            &format!("loadout {index}"),
            &[],
        );
        let granted = player.refresh_hagatha_skill_effects(&mut rng);
        support::assert_website_json(
            &snapshot(&player, &rng, granted),
            &case["refreshed"],
            &format!("loadout {index} effects"),
            &[],
        );
    }
}
