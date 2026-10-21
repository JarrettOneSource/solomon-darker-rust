//! Ordered status and cooldown transactions retain their Source checkpoints.

mod common;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::engines::{
    affix::ModAffixEngine, spells::ModSpellEngine, statuses::ModStatusEngine,
};
use solomon_sim::modding::content::{PreparedModContentCatalog, PreparedModContentDefinition};
use solomon_sim::modding::runtime::{ModSpellCheckpoint, ModStatusCheckpoint};
use std::sync::Arc;

#[derive(Deserialize)]
struct Fixture {
    catalog: Vec<PreparedModContentDefinition>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    status: ModStatusCheckpoint,
    spells: ModSpellCheckpoint,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Input {
    Apply {
        content_id: String,
        target_id: String,
        tick: u64,
        duration_ms: Option<f64>,
    },
    Damage {
        target_id: String,
        amount: f64,
        tick: u64,
    },
    Mana {
        target_id: String,
        delta: f64,
        tick: u64,
    },
    StatusTick {
        tick: u64,
    },
    StatusRestore {
        checkpoint: ModStatusCheckpoint,
    },
    Admit {
        player_id: String,
        content_id: String,
        tick: u64,
        current_mana: f64,
        mana_cost: Option<f64>,
    },
    SpellRestore {
        checkpoint: ModSpellCheckpoint,
    },
    Roll {
        pool_id: String,
        equipment_type: String,
        seed: u64,
    },
}

#[test]
fn authored_statuses_affixes_and_spell_admission_retain_source_transactions() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-engines-goldens.json"
    ))
    .unwrap();
    let catalog = Arc::new(PreparedModContentCatalog::from_prepared(fixture.catalog).unwrap());
    let mut statuses = ModStatusEngine::new(Arc::clone(&catalog), 100).unwrap();
    let mut spells = ModSpellEngine::new(Arc::clone(&catalog), 100).unwrap();
    let affixes = ModAffixEngine::new(catalog);
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let result: Result<Value, String> = match case.input {
            Input::Apply { content_id, target_id, tick, duration_ms } => statuses.apply(&content_id, &target_id, tick, duration_ms).map(|value| json!(value)),
            Input::Damage { target_id, amount, tick } => statuses.filter_damage(&target_id, amount, tick).map(|value| json!(value)),
            Input::Mana { target_id, delta, tick } => statuses.filter_mana(&target_id, delta, tick).map(|value| json!(value)),
            Input::StatusTick { tick } => statuses.tick(tick).map(|value| json!(value)),
            Input::StatusRestore { checkpoint } => statuses.restore(checkpoint).map(|()| Value::Null),
            Input::Admit { player_id, content_id, tick, current_mana, mana_cost } => spells.admit(&player_id, &content_id, tick, current_mana, mana_cost).map(|admission| json!({"manaCost":admission.mana_cost,"readyTick":admission.ready_tick,"contentId":admission.spell.common.content_id})),
            Input::SpellRestore { checkpoint } => spells.restore(checkpoint).map(|()| Value::Null),
            Input::Roll { pool_id, equipment_type, seed } => affixes.roll(&pool_id, &equipment_type, seed).map(|value| json!(value)),
        };
        match result {
            Ok(output) => {
                assert!(case.error.is_none(), "transaction{index} accepted");
                common::assert_json_values(&output, &case.output, &format!("transaction{index}"));
            }
            Err(error) => assert_eq!(Some(error), case.error, "transaction{index} error"),
        }
        common::assert_json_values(
            &json!(statuses.checkpoint()),
            &json!(case.status),
            &format!("transaction{index}.statuses"),
        );
        assert_eq!(spells.checkpoint(), case.spells, "transaction{index}.spells");
    }
}
