//! Network frames retain live menu state, durable authority and immutable scene allocations.

use std::collections::BTreeMap;
use std::path::PathBuf;

use solomon_assets::boneyard::BoneyardCatalog;
use solomon_assets::navigation::decode_navigation;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{
    GameCheckpoint, GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};

fn session() -> GameSession {
    let players = [("zeta", WizardElement::Fire), ("alpha", WizardElement::Water)]
        .into_iter()
        .map(|(id, element)| {
            (
                id.into(),
                NewGameSelection { name: id.into(), element, discipline: WizardDiscipline::Arcane },
            )
        })
        .collect();
    GameSession::new_participants(players, GameSessionOptions::default()).unwrap()
}

fn navigation(session: &GameSession) -> NavigationSet {
    let loaded = &session.boneyard.as_ref().unwrap().loaded;
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/assets/navigation")
        .join(format!("{}.json", loaded.source_sha256));
    decode_navigation(&std::fs::read_to_string(path).unwrap(), loaded).unwrap()
}

#[test]
fn display_copies_share_scenery_while_gate_updates_remain_private() {
    let mut source = session();
    source
        .install_boneyard(BoneyardCatalog::load_embedded().unwrap().materialize([0; 16]))
        .unwrap();
    let before = serde_json::to_value(source.snapshot()).unwrap();
    let mut display = source.clone();
    let original = source.boneyard.as_ref().unwrap();
    let projected = display.boneyard.as_mut().unwrap();
    assert_eq!(projected.loaded.scene.objects.as_ptr(), original.loaded.scene.objects.as_ptr());
    assert_eq!(projected.collision.circles().as_ptr(), original.collision.circles().as_ptr());
    assert_eq!(projected.collision.polygons().as_ptr(), original.collision.polygons().as_ptr());
    assert_eq!(
        projected.collision.segments().next().unwrap() as *const _,
        original.collision.segments().next().unwrap() as *const _
    );
    assert!(!projected.gate_leaves.is_empty());
    projected.gate_leaves[0].tip.x += 20.0;
    projected.dynamic_collision.set_gate_leaves(&projected.gate_leaves);
    assert_ne!(projected.dynamic_collision, original.dynamic_collision);
    assert_eq!(projected.collision, original.collision);
    assert_eq!(serde_json::to_value(source.snapshot()).unwrap(), before);
}

#[test]
fn borrowed_frames_decode_as_owned_checkpoints_and_reject_invalid_membership_atomically() {
    let mut source = session();
    source.inventory_dye_sessions.insert(
        "zeta".into(),
        solomon_sim::economy::NativeInventoryDyeSession {
            id: "dye-frame-1".into(),
            dye_item_id: 7,
            kit_consumed: false,
        },
    );
    let borrowed = serde_json::to_value(source.snapshot()).unwrap();
    let mut durable = serde_json::to_value(source.checkpoint()).unwrap();
    assert!(durable.get("inventory_dye_sessions").is_none());
    let live_dye = serde_json::json!({"zeta": {
        "id": "dye-frame-1", "dye_item_id": 7, "kit_consumed": false,
    }});
    assert_eq!(borrowed["inventory_dye_sessions"], live_dye);
    durable["inventory_dye_sessions"] = live_dye;
    assert_eq!(borrowed, durable);
    let checkpoint: GameCheckpoint = serde_json::from_value(borrowed.clone()).unwrap();
    let restored = GameSession::restore(checkpoint, None).unwrap();
    assert_eq!(serde_json::to_value(restored.snapshot()).unwrap(), borrowed);
    assert_eq!(restored.participant_order, ["zeta", "alpha"]);

    let mut invalid = source.checkpoint();
    invalid.participant_order[1] = "zeta".into();
    assert!(source.apply_checkpoint(invalid, None).is_err());
    assert_eq!(serde_json::to_value(source.snapshot()).unwrap(), borrowed);
}

#[test]
fn applying_completed_world_frames_retains_geometry_and_accepts_owned_wave_tables() {
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    let loaded = catalog.materialize([0; 16]);
    let mut source = session();
    source.install_boneyard(loaded.clone()).unwrap();
    let navigation = navigation(&source);
    let mut client = GameSession::restore(source.checkpoint(), Some(loaded)).unwrap();
    let world = client.boneyard.as_ref().unwrap();
    let objects = world.loaded.scene.objects.as_ptr();
    let collision = world.collision.segments().next().unwrap() as *const _;
    let inputs = BTreeMap::from([
        ("zeta".into(), GameSessionInput::default()),
        ("alpha".into(), GameSessionInput::default()),
    ]);
    for index in 0..20 {
        if index == 5 {
            // Current saves own their compiled graphs, including retained older tables.
            source.boneyard.as_mut().unwrap().waves.as_mut().unwrap().compiled_schedule[0]
                .lull_threshold += 1;
        }
        source.step(
            &inputs,
            GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
        );
        let wire = serde_json::to_vec(&source.snapshot()).unwrap();
        let checkpoint: GameCheckpoint = serde_json::from_slice(&wire).unwrap();
        client.apply_checkpoint(checkpoint, None).unwrap();
        let world = client.boneyard.as_ref().unwrap();
        assert_eq!(world.loaded.scene.objects.as_ptr(), objects);
        assert_eq!(
            world.waves.as_ref().unwrap().compiled_schedule,
            source.boneyard.as_ref().unwrap().waves.as_ref().unwrap().compiled_schedule
        );
        assert_eq!(world.collision.segments().next().unwrap() as *const _, collision);
        assert_eq!(
            serde_json::to_value(client.snapshot()).unwrap(),
            serde_json::to_value(source.snapshot()).unwrap()
        );
    }
    let before = serde_json::to_value(client.snapshot()).unwrap();
    let mut invalid = source.checkpoint();
    invalid.boneyard.as_mut().unwrap().waves.as_mut().unwrap().state.active_section =
        Some(solomon_sim::boneyard::waves::director::WaveSection::Schedule { index: usize::MAX });
    assert!(client.apply_checkpoint(invalid, None).is_err());
    assert_eq!(serde_json::to_value(client.snapshot()).unwrap(), before);

    let mut other = source.checkpoint();
    other.boneyard.as_mut().unwrap().run_id = "unloaded-world".into();
    other.run.run_id = Some("unloaded-world".into());
    assert!(client.apply_checkpoint(other, None).is_err());
    assert_eq!(serde_json::to_value(client.snapshot()).unwrap(), before);
}

#[test]
fn entity_ids_reject_corrupt_continuations_and_hydrate_earlier_standalone_documents() {
    let mut source = session();
    let before = serde_json::to_value(source.snapshot()).unwrap();
    let mut duplicate = source.checkpoint();
    duplicate.participants.get_mut("alpha").unwrap().entity_id =
        duplicate.participants.get("zeta").expect("participant exists").entity_id;
    assert!(source.apply_checkpoint(duplicate, None).is_err());
    let mut stale_counter = source.checkpoint();
    stale_counter.next_entity_id =
        source.participants.get("alpha").expect("participant exists").entity_id;
    assert!(source.apply_checkpoint(stale_counter, None).is_err());
    assert_eq!(serde_json::to_value(source.snapshot()).unwrap(), before);

    let mut earlier_document = before;
    earlier_document.as_object_mut().unwrap().remove("next_entity_id");
    for participant in earlier_document["participants"].as_object_mut().unwrap().values_mut() {
        participant.as_object_mut().unwrap().remove("entity_id");
    }
    let checkpoint: GameCheckpoint = serde_json::from_value(earlier_document).unwrap();
    let restored = GameSession::restore(checkpoint, None).unwrap();
    assert_eq!(restored.participants.get("zeta").expect("participant exists").entity_id, 1);
    assert_eq!(restored.participants.get("alpha").expect("participant exists").entity_id, 2);
    assert_eq!(restored.next_entity_id, 3);
}
