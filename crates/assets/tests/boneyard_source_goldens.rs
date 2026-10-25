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

#[test]
fn display_copies_share_imported_scenery_tables_until_cleanup_changes_membership() {
    use solomon_sim::boneyard::arena::ArenaPhase;
    use solomon_sim::boneyard::world::BoneyardSceneryTargetRetention;

    let mut world =
        BoneyardWorld::new(BoneyardCatalog::load_embedded().unwrap().materialize([0; 16]));
    let bounds = world.arena_transition.as_ref().unwrap().combat_bounds;
    let outside = world
        .loaded
        .scene
        .objects
        .iter()
        .position(|object| {
            object.position.x < bounds.x
                || object.position.x > bounds.x + bounds.width
                || object.position.y < bounds.y
                || object.position.y > bounds.y + bounds.height
        })
        .expect("the entrance contains scenery outside the sealed arena");
    let id = format!("scenery:{}", world.loaded.scene.objects[outside].eid);
    world.scenery_target_retention = Some(
        BoneyardSceneryTargetRetention {
            earthquake_ids: [outside].into(),
            earthquake_variants: [(outside, 3)].into(),
            primary_ids: [id.clone()].into(),
            spell_ids: [id.clone()].into(),
        }
        .into(),
    );
    let original = serde_json::to_value(world.snapshot()).unwrap();
    let mut displayed = world.clone();
    assert!(
        std::ptr::eq(
            &world.scenery_target_retention.as_ref().unwrap().primary_ids,
            &displayed.scenery_target_retention.as_ref().unwrap().primary_ids,
        ),
        "display sampling must reuse unchanged imported target tables"
    );
    displayed.arena_transition.as_mut().unwrap().phase = ArenaPhase::Sealed;
    displayed.retire_goodies_after_sealing();
    assert!(!displayed.retains_primary_scenery_target(&id));
    assert!(!displayed.retains_scenery_spell_target(&id));
    assert!(!displayed.retains_earthquake_scenery_target(outside));
    assert!(world.retains_primary_scenery_target(&id));
    assert!(world.retains_scenery_spell_target(&id));
    assert!(world.retains_earthquake_scenery_target(outside));
    assert_eq!(serde_json::to_value(world.snapshot()).unwrap(), original);
    let retained_display = displayed.clone();
    displayed.retire_goodies_after_sealing();
    assert!(
        std::ptr::eq(
            &retained_display.scenery_target_retention.as_ref().unwrap().primary_ids,
            &displayed.scenery_target_retention.as_ref().unwrap().primary_ids,
        ),
        "repeating completed cleanup must not detach unchanged cached tables"
    );
    let checkpoint =
        serde_json::from_value(serde_json::to_value(displayed.snapshot()).unwrap()).unwrap();
    let restored = BoneyardWorld::restore(displayed.loaded.clone(), checkpoint).unwrap();
    assert_eq!(
        serde_json::to_value(restored.snapshot()).unwrap(),
        serde_json::to_value(displayed.snapshot()).unwrap()
    );
}
