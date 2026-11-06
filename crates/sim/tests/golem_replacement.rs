//! Resummoning a Golem retires the oldest one and births a fresh one at full health.
//! The visible HUD row change must not be mistaken for healing the surviving Golem.
#![cfg(feature = "persistence")]

#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod support;

use std::collections::BTreeMap;

use serde_json::Value;
use solomon_sim::belt::{NativeBeltEntry, PlayerBeltComponent};
use solomon_sim::secondary::golem::GolemIncomingDamage;
use solomon_sim::secondary::{SecondaryActorKind, SecondarySimulationState, SecondaryTickContext};
use solomon_sim::social::{AllyGolemProjection, AllyHudRow, derive_golem_ally_hud_rows};
use solomon_sim::{HubWizard, Vec2};

fn health(state: &SecondarySimulationState) -> Vec<(u64, f64, f64)> {
    state
        .actors
        .iter()
        .filter(|actor| actor.kind == SecondaryActorKind::Golem)
        .map(|actor| {
            let golem = actor.golem.as_ref().unwrap();
            (actor.id, golem.current_health, golem.maximum_health)
        })
        .collect()
}

fn rows(state: &SecondarySimulationState) -> Vec<AllyHudRow> {
    let golems: Vec<_> = state
        .actors
        .iter()
        .filter(|actor| actor.kind == SecondaryActorKind::Golem)
        .map(|actor| {
            let golem = actor.golem.as_ref().unwrap();
            AllyGolemProjection {
                id: actor.id,
                world_key: &actor.world_key,
                current_health: golem.current_health,
                maximum_health: golem.maximum_health,
            }
        })
        .collect();
    derive_golem_ally_hud_rows(&golems, "boneyard:oracle")
}

#[test]
fn resummoning_with_or_without_the_set_preserves_the_survivors_health_and_hud_identity() {
    // Reuse the unchanged admitted Raise Golem input, not its generated expectations.
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-cast-goldens.json")).unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "45-normal-plain")
        .unwrap();
    let declared = &case["authority"];
    let book = support::read_book(&declared["skillBook"]);
    let mut belt = PlayerBeltComponent::new(&book);
    belt.entries[0] = Some(NativeBeltEntry::Skill { skill_id: 45 });
    let wizard = HubWizard::at(Vec2::ZERO);
    let mut world = support::World { case, targets: Vec::new() };
    let mut step = |state: &mut SecondarySimulationState, tick, slot, maximum_golem| {
        let mut authority = support::authority_from_fixture(declared, &book, &belt, &wizard);
        authority.input.quickbar = slot;
        authority.maximum_golem = maximum_golem;
        let players = BTreeMap::from([("player".to_owned(), authority)]);
        state.step(
            &SecondaryTickContext { enhanced_effects: None, players: &players, tick },
            &mut world,
            None,
        );
    };

    let mut state = SecondarySimulationState::new(77);
    for tick in 1..=3401 {
        step(&mut state, tick, (tick == 1 || tick == 3001).then_some(0), true);
    }
    let original = health(&state);
    assert_eq!(original.len(), 2, "two actual casts retain two assembled Golems");
    let retired_id = original[0].0;
    let survivor_id = original[1].0;
    assert_eq!(
        original.iter().map(|actor| (actor.1, actor.2)).collect::<Vec<_>>(),
        [(100.0, 100.0), (100.0, 100.0),]
    );
    for (id, damage) in [(retired_id, 80.0), (survivor_id, 20.0)] {
        let result = state.apply_golem_damage(
            id,
            GolemIncomingDamage {
                primary_damage: damage,
                secondary_damage: 0.0,
                reflectable_physical_source_in_range: false,
            },
            3401,
            false,
            None,
        );
        assert!(!result.ignored && !result.killed, "nonlethal damage for actor {id}");
    }
    assert_eq!(health(&state), [(retired_id, 20.0, 100.0), (survivor_id, 80.0, 100.0)]);
    let before_rows = rows(&state);
    assert_eq!(before_rows.iter().map(|row| row.health_ratio).collect::<Vec<_>>(), [0.2, 0.8]);

    for maximum_golem in [true, false] {
        let mut replacement = state.clone();
        for tick in 3402..=6000 {
            step(&mut replacement, tick, None, maximum_golem);
        }
        assert_eq!(health(&replacement), health(&state), "set removal does not heal live actors");
        step(&mut replacement, 6001, Some(0), maximum_golem);
        let after = health(&replacement);
        assert_eq!(after.len(), 2, "one retirement and one birth, set={maximum_golem}");
        assert!(!after.iter().any(|actor| actor.0 == retired_id));
        assert_eq!(after[0], (survivor_id, 80.0, 100.0));
        assert!(after[1].0 > survivor_id, "the new summon has a fresh actor ID");
        assert_eq!((after[1].1, after[1].2), (100.0, 100.0));
        let after_rows = rows(&replacement);
        assert_eq!(before_rows[1], after_rows[0], "the survivor's row follows its actor ID");
        assert_eq!(after_rows[1].id, format!("golem:{}", after[1].0));
        assert_eq!(after_rows[1].health_ratio, 1.0);
    }
}
