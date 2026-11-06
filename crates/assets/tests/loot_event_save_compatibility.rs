//! Public save compatibility for loot events, sound IDs and numeric Bonus rewards.

use serde_json::{Value, json};
use solomon_assets::website_save::{self, CreateSaveOptions, SaveIntegrity, SaveMetadata};
use solomon_sim::Vec2;
use solomon_sim::boneyard::loot::{BoneyardLootEventType, BoneyardLootSound};
use solomon_sim::loot::{BonusKind, LootDropKind, LootDropSource, LootDropSpec};
use solomon_sim::modding::LuaObject;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

fn document(bonus: Option<BonusKind>) -> Value {
    let mut session = GameSession::new(
        "local".into(),
        NewGameSelection {
            name: "Felix".into(),
            element: WizardElement::Earth,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let loaded = solomon_assets::boneyard::load_boneyard([0; 16]).unwrap();
    session.install_boneyard(loaded.clone()).unwrap();
    if let Some(bonus) = bonus {
        let mut spec = LootDropSpec::new(
            LootDropKind::Bonus,
            1,
            Vec2::new(500.0, 600.0),
            LootDropSource::Enemy,
        );
        spec.bonus_kind = Some(bonus);
        assert_eq!(
            session.boneyard.as_mut().unwrap().loot.spawn_specs(
                vec![spec],
                0,
                &mut session.painter_order,
            ),
            0,
        );
    }

    let metadata = SaveMetadata {
        integrity: SaveIntegrity::LocalOnly,
        mods: Vec::new(),
        mod_state: LuaObject::default(),
        native_source: None,
    };
    serde_json::from_str(
        &website_save::create(CreateSaveOptions {
            state: &session,
            player_id: "local",
            loaded_boneyard: Some(&loaded),
            metadata: &metadata,
            party_rejoin_token: None,
        })
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn supported_loot_event_tags_and_all_sound_ids_restore_and_reexport() {
    use BoneyardLootEventType::{GoodieKeyNeeded, GoodiePhase, LootDropSound, LootPickup};
    use BoneyardLootSound::{
        DropBag1, DropBag2, DropCoins, DropPotion, GotoOrb, PickupBag, PickupCoin,
    };
    let cases = [
        ("goodie-key-needed", None, GoodieKeyNeeded, None),
        ("goodie-phase", None, GoodiePhase, None),
        ("loot-drop-sound", Some("drop-bag-1"), LootDropSound, Some(DropBag1)),
        ("loot-drop-sound", Some("drop-bag-2"), LootDropSound, Some(DropBag2)),
        ("loot-drop-sound", Some("drop-coins"), LootDropSound, Some(DropCoins)),
        ("loot-drop-sound", Some("drop-potion"), LootDropSound, Some(DropPotion)),
        ("loot-pickup", Some("goto-orb"), LootPickup, Some(GotoOrb)),
        ("loot-pickup", Some("pickup-bag"), LootPickup, Some(PickupBag)),
        ("loot-pickup", Some("pickup-coin"), LootPickup, Some(PickupCoin)),
    ];
    for version in [50, 51] {
        for rust_spelling in [false, true] {
            let mut document = document(None);
            document["schemaVersion"] = json!(version);
            let events: Vec<_> = cases
                .iter()
                .enumerate()
                .map(|(index, (kind, sound, _, _))| {
                    let mut event = json!({
                        "actorId": index + 1, "eventId": index + 1,
                        "position": {"x": 500.0, "y": 600.0}, "tick": 0,
                        "type": kind, "sound": sound,
                    });
                    if rust_spelling {
                        let fields = event.as_object_mut().unwrap();
                        let kind = fields.remove("type").unwrap();
                        fields.insert("eventType".into(), kind);
                        // Earlier Rust exports used the generic enum spelling.
                        if let Some(sound) = sound {
                            event["sound"] = json!(match *sound {
                                "drop-bag-1" => "drop-bag1",
                                "drop-bag-2" => "drop-bag2",
                                sound => sound,
                            });
                        }
                    }
                    event
                })
                .collect();
            document["continuation"]["simulation"]["world"]["lootEvents"] = json!(events);
            let restored = website_save::restore(&document.to_string()).unwrap();
            let actual = &restored.session.boneyard.as_ref().unwrap().loot_events;
            assert_eq!(actual.len(), cases.len());
            for (event, (_, _, kind, sound)) in actual.iter().zip(cases) {
                assert_eq!((event.event_type, event.sound), (kind, sound));
            }
            let reencoded = website_save::create(CreateSaveOptions {
                state: &restored.session,
                player_id: &restored.player_id,
                loaded_boneyard: restored.loaded_boneyard.as_ref(),
                metadata: &restored.metadata,
                party_rejoin_token: None,
            })
            .unwrap();
            let encoded: Value = serde_json::from_str(&reencoded).unwrap();
            for (event, (kind, sound, _, _)) in
                encoded["continuation"]["simulation"]["world"]["lootEvents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(cases)
            {
                assert_eq!(event["type"], kind);
                assert_eq!(event["sound"], json!(sound));
                assert!(event.get("eventType").is_none());
            }
            let roundtrip = website_save::restore(&reencoded).unwrap();
            assert_eq!(roundtrip.session.boneyard.as_ref().unwrap().loot_events, *actual);
        }
    }
}

#[test]
fn source_bonus_ids_and_prior_rust_names_restore_and_export_numeric_rewards() {
    for version in [50, 51, 52] {
        for (id, kind, previous) in [
            (0, BonusKind::SkillChoice, "skill-choice"),
            (1, BonusKind::SkillRank, "skill-rank"),
            (2, BonusKind::DamageX4, "damage-x4"),
        ] {
            for spelling in [json!(id), json!(format!("{kind:?}")), json!(previous)] {
                let mut document = document(Some(kind));
                document["schemaVersion"] = json!(version);
                document["continuation"]["simulation"]["world"]["loot"]["actors"][0]["bonusKind"] =
                    spelling;
                let restored = website_save::restore(&document.to_string()).unwrap();
                let loot = &restored.session.boneyard.as_ref().unwrap().loot;
                assert_eq!(loot.actors[0].bonus_kind, Some(kind));
                let encoded = website_save::create(CreateSaveOptions {
                    state: &restored.session,
                    player_id: &restored.player_id,
                    loaded_boneyard: restored.loaded_boneyard.as_ref(),
                    metadata: &restored.metadata,
                    party_rejoin_token: None,
                })
                .unwrap();
                let exported: Value = serde_json::from_str(&encoded).unwrap();
                assert_eq!(
                    exported["continuation"]["simulation"]["world"]["loot"]["actors"][0]["bonusKind"],
                    id,
                );
                let roundtrip = website_save::restore(&encoded).unwrap();
                assert_eq!(roundtrip.session.boneyard.as_ref().unwrap().loot, *loot);
            }
        }
    }
}

#[test]
fn unknown_or_fractional_bonus_ids_are_rejected() {
    for invalid in [json!(-1), json!(3), json!(0.5)] {
        let mut document = document(Some(BonusKind::SkillRank));
        document["continuation"]["simulation"]["world"]["loot"]["actors"][0]["bonusKind"] = invalid;
        assert!(website_save::restore(&document.to_string()).is_err());
    }
}
