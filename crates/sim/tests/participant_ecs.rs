#![cfg(feature = "persistence")]

use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{
    GameCheckpoint, GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};

fn selection(id: &str) -> NewGameSelection {
    NewGameSelection {
        name: id.into(),
        element: WizardElement::Fire,
        discipline: WizardDiscipline::Arcane,
    }
}

#[test]
fn membership_changes_preserve_admission_identity_and_saved_continuation() {
    let mut live = GameSession::new_participants(
        ["zeta", "middle", "alpha"].into_iter().map(|id| (id.into(), selection(id))).collect(),
        GameSessionOptions { gameplay_rng_seed: 718, ..GameSessionOptions::default() },
    )
    .unwrap();
    let original = live.checkpoint();
    assert!(live.remove_participant("middle"));
    assert!(live.add_participant("beta".into(), selection("beta")));
    assert_eq!(live.participant_order, ["zeta", "alpha", "beta"]);
    assert_eq!(live.participant("zeta").unwrap().entity_id, 1);
    assert_eq!(live.participant("alpha").unwrap().entity_id, 3);
    assert_eq!(live.participant("beta").unwrap().entity_id, 4);
    for id in ["zeta", "alpha"] {
        assert_eq!(
            serde_json::to_value(live.participant(id).unwrap()).unwrap(),
            serde_json::to_value(&original.participants[id]).unwrap(),
            "membership changes preserve every retained component for {id}"
        );
    }

    let saved = serde_json::to_vec(&live.checkpoint()).unwrap();
    let checkpoint: GameCheckpoint = serde_json::from_slice(&saved).unwrap();
    let mut restored = GameSession::restore(checkpoint, None).unwrap();
    assert_eq!(serde_json::to_vec(&restored.checkpoint()).unwrap(), saved);
    let inputs = BTreeMap::from([
        ("zeta".into(), GameSessionInput { movement: Vec2::new(1.0, 0.0), ..Default::default() }),
        ("alpha".into(), GameSessionInput { movement: Vec2::new(0.0, 1.0), ..Default::default() }),
        ("beta".into(), GameSessionInput { movement: Vec2::new(-1.0, 0.0), ..Default::default() }),
    ]);
    let start = live.wizard("zeta").unwrap().position;
    for tick in 1..=60 {
        assert!(live.step(&inputs, GameSessionTickContext::default()).advanced);
        assert!(restored.step(&inputs, GameSessionTickContext::default()).advanced);
        assert_eq!(
            serde_json::to_vec(&restored.snapshot()).unwrap(),
            serde_json::to_vec(&live.snapshot()).unwrap(),
            "restored participant systems diverged at tick {tick}"
        );
    }
    assert_ne!(live.wizard("zeta").unwrap().position, start);
}
