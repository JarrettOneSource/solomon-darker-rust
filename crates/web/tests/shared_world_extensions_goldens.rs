//! Per-party extension selection at the actual shared-world tick boundary.

#[path = "../../sim/tests/support/mod.rs"]
mod support;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Value, json};
use solomon_assets::boneyard::decode_scene;
use solomon_assets::navigation::decode_navigation;
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};
use solomon_sim::economy::{BoastSelection, HubEquipmentState, HubInventoryItem};
use solomon_sim::hub::interaction::BoastRules;
use solomon_sim::modding::extensions::*;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::shared_worlds::{SharedGameWorlds, SharedWorldsTickContext};
use solomon_sim::social::PartyIdentity;

struct PartyRules<'a> {
    owner: &'a str,
    mana_spend: f64,
    error: Option<&'a str>,
    calls: &'a RefCell<Vec<Value>>,
}

impl GameSimulationExtensions for PartyRules<'_> {
    fn create_loot_items(&self, _: GameSimulationLootInput) -> Vec<HubInventoryItem> {
        panic!("the initial scene cannot award enemy loot")
    }

    fn filter_damage(
        &self,
        input: GameSimulationDamageFilterInput<'_>,
        _: &HubEquipmentState,
    ) -> Result<f64, String> {
        self.calls.borrow_mut().push(json!({"owner":self.owner,"kind":"damage",
            "player":input.target_player_id,"value":input.amount}));
        Ok(input.amount)
    }

    fn filter_mana(
        &self,
        input: GameSimulationManaFilterInput<'_>,
        _: &HubEquipmentState,
    ) -> Result<f64, String> {
        self.calls.borrow_mut().push(json!({"owner":self.owner,"kind":"mana",
            "player":input.player_id,"value":input.delta}));
        if let Some(reason) = self.error {
            return Err(reason.into());
        }
        Ok(input.delta - self.mana_spend)
    }

    fn has_consumable(&self, _: &str) -> bool {
        false
    }

    fn resolve_boast(&self, _: &BoastSelection) -> Option<BoastRules<'_>> {
        None
    }
}

#[test]
fn each_actual_party_uses_its_admitted_rules_and_callback_errors_end_the_epoch() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/shared-world-extensions-goldens.json"
    ))
    .unwrap();
    let source: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/shared-worlds-goldens.json"
    ))
    .unwrap();
    let loaded = &source["loaded"];
    let template = LoadedBoneyard {
        choice_id: "default-random".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: "Random Boneyard".into(),
        seed: loaded["seed"].as_str().unwrap().into(),
        run_id: "fixture".into(),
        source_sha256: loaded["sourceSha256"].as_str().unwrap().into(),
        geometry_sha256: loaded["geometrySha256"].as_str().unwrap().into(),
        scene: decode_scene(&loaded["scene"].to_string()).unwrap().into(),
    };
    let navigation_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/assets/navigation")
        .join(format!("{}.json", template.source_sha256));
    let navigation =
        decode_navigation(&std::fs::read_to_string(navigation_file).unwrap(), &template).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut worlds = SharedGameWorlds::new(0);
        for id in ["resident", "zeta", "alpha"] {
            worlds.add_hub_player(
                id.into(),
                NewGameSelection {
                    name: "Oracle".into(),
                    discipline: WizardDiscipline::Arcane,
                    element: WizardElement::Ether,
                },
                PartyIdentity {
                    id: format!("party-{id}"),
                    join_code: format!("CODE-{id}"),
                    listing_id: format!("listing-{id}"),
                },
            );
        }
        for id in ["zeta", "alpha"] {
            let mut loaded = template.clone();
            loaded.run_id = format!("run-{id}");
            worlds.start_party_run(id, loaded).unwrap();
            worlds
                .state_for_player_mut(id)
                .unwrap()
                .participants
                .get_mut(id)
                .unwrap()
                .player
                .combat
                .set_mana(10.0);
        }
        let calls = RefCell::new(Vec::new());
        let zeta = PartyRules {
            owner: "zeta",
            mana_spend: 0.125,
            error: case["spec"]["error"].as_str(),
            calls: &calls,
        };
        let alpha = PartyRules { owner: "alpha", mana_spend: 0.25, error: None, calls: &calls };
        let extensions: BTreeMap<String, &dyn GameSimulationExtensions> = BTreeMap::from([
            ("party-zeta".into(), &zeta as _),
            ("party-alpha".into(), &alpha as _),
        ]);
        let paused: BTreeSet<_> = case["spec"]["paused"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap().to_owned())
            .collect();
        let navigation = BTreeMap::from([
            ("party-zeta".into(), navigation.clone()),
            ("party-alpha".into(), navigation.clone()),
        ]);
        let result = worlds.step_with_extensions(
            &BTreeMap::new(),
            SharedWorldsTickContext {
                paused_party_ids: Some(&paused),
                navigation_by_party: Some(&navigation),
                ..SharedWorldsTickContext::default()
            },
            Some(&extensions),
        );
        let error = result.err();
        let players = if error.is_some() {
            Value::Null
        } else {
            Value::Array(
                ["resident", "zeta", "alpha"]
                    .iter()
                    .map(|&id| {
                        let state = worlds.state_for_player(id).unwrap();
                        let player = &state.participants[id].player;
                        json!({"id":id,"tick":state.ticks,"mana":player.combat.current_mana,
                            "health":player.combat.current_health})
                    })
                    .collect(),
            )
        };
        let actual = json!({"calls":*calls.borrow(),"error":error,"players":players});
        let expected =
            json!({"calls":case["calls"],"error":case["error"],"players":case["players"]});
        support::assert_website_json(
            &actual,
            &expected,
            case["spec"]["name"].as_str().unwrap(),
            &[],
        );
    }
}
