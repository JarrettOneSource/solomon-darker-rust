//! Source Session effect lifetime, transfer and checkpoint ownership.
use serde::Deserialize;
use solomon_sim::modding::inventory::GameSimulationModEffect;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions, GameSessionTickContext};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Fixture {
    rows: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    operation: String,
    tick: u64,
    effects: Vec<GameSimulationModEffect>,
    next_use_id: u64,
}
fn initial() -> GameSession {
    GameSession::new_participants(
        vec![(
            "alpha".into(),
            NewGameSelection {
                name: "Effects".into(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
        )],
        GameSessionOptions { gameplay_rng_seed: 731, ..GameSessionOptions::default() },
    )
    .unwrap()
}
#[test]
fn effects_follow_source_tick_and_run_boundaries_and_survive_native_checkpoints() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/session-mod-effects-goldens.json"
    ))
    .unwrap();
    let retained = fixture.rows[0].effects.clone();
    let mut state = initial();
    state.mod_effects = retained.clone().into_iter().collect();
    state.next_mod_consumable_use_id = 5;
    for row in fixture.rows {
        match row.operation.as_str() {
            "initial" => {}
            "step" => {
                state.step(&BTreeMap::new(), GameSessionTickContext::default());
            }
            "enter" => {
                state = initial();
                state.mod_effects = retained.clone().into_iter().collect();
                state.next_mod_consumable_use_id = 5;
                state
                    .install_boneyard(solomon_assets::boneyard::load_tutorial([0; 16]).unwrap())
                    .unwrap();
            }
            "return" => {
                state.mod_effects = retained.clone().into_iter().collect();
                state.return_to_hub();
            }
            "partition-empty" => {
                state = initial();
                state.mod_effects = retained.clone().into_iter().collect();
                state.next_mod_consumable_use_id = 5;
                state = state.partition(&[]).unwrap().selected;
            }
            other => panic!("unknown source operation {other}"),
        }
        assert_eq!(state.ticks, row.tick, "{}", row.operation);
        assert_eq!(
            state.mod_effects.iter().cloned().collect::<Vec<_>>(),
            row.effects,
            "{}",
            row.operation
        );
        assert_eq!(state.next_mod_consumable_use_id, row.next_use_id, "{}", row.operation);
        {
            let bytes = serde_json::to_vec(&state.snapshot()).unwrap();
            let checkpoint = serde_json::from_slice(&bytes).unwrap();
            let loaded = state.boneyard.as_ref().map(|world| world.loaded.clone());
            let restored = GameSession::restore(checkpoint, loaded).unwrap();
            assert_eq!(restored.mod_effects, state.mod_effects);
            assert_eq!(restored.next_mod_consumable_use_id, state.next_mod_consumable_use_id);
        }
    }
}
