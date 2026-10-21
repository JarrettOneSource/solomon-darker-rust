//! Actual Source host checkpoints and borrowed engine filters remain one owner.

mod common;
#[path = "common/inventory.rs"]
mod inventory;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::host_runtime::PreparedModRuntime;
use solomon_modding::package::{
    compile_web_session_content_definitions, materialize_web_session_content_value,
};
use solomon_modding::session::{PreparedModSession, PreparedModSource};
use solomon_sim::economy::{BoastSelection, HubEquipmentState};
use solomon_sim::enemies::EnemyToken;
use solomon_sim::modding::LuaValue;
use solomon_sim::modding::extensions::*;
use solomon_sim::modding::runtime::PreparedModHostCheckpoint;

#[derive(Deserialize)]
struct Fixture {
    payload: LuaValue,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    checkpoint: PreparedModHostCheckpoint,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Input {
    Checkpoint,
    Restore { checkpoint: Box<PreparedModHostCheckpoint> },
    Damage { input: Damage },
    Mana { input: Mana },
    Loot { input: Loot },
    HasConsumable { content_id: String },
    Boast { selection: BoastSelection },
    Equipment { value: Value },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Damage {
    amount: f64,
    source_actor_id: Option<usize>,
    target_player_id: String,
    tick: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mana {
    current_mana: f64,
    delta: f64,
    maximum_mana: f64,
    player_id: String,
    tick: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Loot {
    actor_seed: u64,
    enemy_token: String,
}

#[test]
fn one_prepared_runtime_restores_atomically_and_filters_actual_statuses_skills_and_equipment() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-host-runtime-goldens.json"
    ))
    .unwrap();
    let package = compile_web_session_content_definitions(
        materialize_web_session_content_value(&fixture.payload).unwrap(),
    )
    .unwrap();
    let mut runtime = PreparedModRuntime::prepare(&package).unwrap();
    let sources: Vec<_> = package
        .mod_sources
        .iter()
        .zip(&package.compiled_mods)
        .map(|(source, compiled)| PreparedModSource {
            compiled: compiled.clone(),
            entry_script: source.entry_script.clone(),
            entry_script_path: "scripts/main.lua".into(),
            identity: source.identity.clone(),
        })
        .collect();
    let mut session = PreparedModSession::with_clock(&sources, || 0.0).unwrap();
    let mut equipment = HubEquipmentState::default();
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let path = format!("transaction{index}");
        let result: Result<Value, String> = match case.input {
            Input::Checkpoint => runtime.checkpoint(&session).map(|value| json!(value)),
            Input::Restore { checkpoint } => {
                runtime.restore(&mut session, &checkpoint).map(|()| Value::Null)
            }
            Input::Damage { input } => runtime
                .filter_damage(
                    GameSimulationDamageFilterInput {
                        amount: input.amount,
                        damage_kind: GameSimulationDamageKind::Physical,
                        source_actor_id: input.source_actor_id,
                        target_player_id: &input.target_player_id,
                        tick: input.tick,
                    },
                    &equipment,
                )
                .map(|value| json!(value)),
            Input::Mana { input } => runtime
                .filter_mana(
                    GameSimulationManaFilterInput {
                        current_mana: input.current_mana,
                        delta: input.delta,
                        maximum_mana: input.maximum_mana,
                        player_id: &input.player_id,
                        source: GameSimulationManaSource::PrimaryCast,
                        tick: input.tick,
                    },
                    &equipment,
                )
                .map(|value| json!(value)),
            Input::Loot { input } => {
                let enemy_token = EnemyToken::from_id(&input.enemy_token).unwrap();
                let items = runtime.create_loot_items(GameSimulationLootInput {
                    actor_seed: input.actor_seed,
                    enemy_token,
                });
                Ok(json!(items.iter().map(inventory::item_value).collect::<Vec<_>>()))
            }
            Input::HasConsumable { content_id } => Ok(json!(runtime.has_consumable(&content_id))),
            Input::Boast { selection } => {
                Ok(json!(runtime.resolve_boast(&selection).map(|rules| {
                    json!({"failureProducers":rules.failure_producers,"label":rules.label,
                    "randomSkillChoices":rules.random_skill_choices,
                    "scoreMultiplier":rules.score_multiplier,"successWave":rules.success_wave})
                })))
            }
            Input::Equipment { value } => {
                equipment = inventory::equipment_from_source(value);
                Ok(Value::Null)
            }
        };
        match result {
            Ok(output) => {
                assert!(case.error.is_none(), "{path} unexpectedly accepted");
                common::assert_json_values(&output, &case.output, &path);
            }
            Err(error) => assert_eq!(Some(error), case.error, "{path} rejection"),
        }
        common::assert_json_values(
            &json!(runtime.checkpoint(&session).unwrap()),
            &json!(case.checkpoint),
            &format!("{path}.checkpoint"),
        );
    }
}
