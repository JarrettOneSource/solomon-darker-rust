//! Imported layouts may contain Solomon's dig without owning the retail encounter.
use serde_json::Value;
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_sim::boneyard::{BoneyardSource, BoneyardWorld};

#[test]
fn admitted_source_kind_controls_encounters_and_survives_world_checkpoints() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/boneyard-source-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let tutorial = case["tutorial"].as_bool().unwrap();
        let mut loaded = if tutorial {
            catalog.materialize_tutorial([0; 16])
        } else {
            catalog.materialize([0; 16])
        };
        loaded.choice_id = case["id"].as_str().unwrap().to_owned();
        if case["source"] == "mod" {
            loaded.choice_source = BoneyardSource::Mod;
            loaded.mod_id = Some("example".to_owned());
            loaded.mod_name = Some("Example".to_owned());
        }
        let world = BoneyardWorld::new(loaded.clone());
        assert_eq!(
            world.encounter.is_some(),
            case["encounter"].as_bool().unwrap(),
            "{}",
            case["id"]
        );
        assert_eq!(
            world.arena_transition.is_some(),
            case["arena"].as_bool().unwrap(),
            "{}",
            case["id"]
        );
        assert_eq!(world.waves.is_some(), case["waves"].as_bool().unwrap(), "{}", case["id"]);
        assert_eq!(
            world.lantern_position.is_some(),
            case["lantern"].as_bool().unwrap(),
            "{}",
            case["id"]
        );
        let checkpoint = world.checkpoint();
        assert_eq!(checkpoint.choice_source, loaded.choice_source);
        assert_eq!(checkpoint.mod_id, loaded.mod_id);
        assert_eq!(checkpoint.mod_name, loaded.mod_name);
        let encoded = serde_json::to_string(&world.snapshot()).unwrap();
        let snapshot: solomon_sim::boneyard::world::BoneyardCheckpoint =
            serde_json::from_str(&encoded).unwrap();
        assert!(BoneyardWorld::restore(loaded.clone(), snapshot).is_ok());
        let mut changed = loaded.clone();
        changed.choice_source = if loaded.choice_source == BoneyardSource::Default {
            BoneyardSource::Mod
        } else {
            BoneyardSource::Default
        };
        assert!(BoneyardWorld::restore(changed, checkpoint).is_err());
    }
}
