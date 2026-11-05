//! Ground-item lifecycle and completion cleanup through the public store boundary.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::loot::*;
use solomon_sim::economy::{EquipmentType, HubInventoryItem, HubItemKind, ItemRarity};
use solomon_sim::enemies::model::EnemyDeathEffect;
use solomon_sim::equipment::NativeEquipmentEffect;
use solomon_sim::loot::*;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};

use super::{flags, indexes, item_snapshot, rng_snapshot, selection_input, support};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn point_snapshot(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}

fn registration(value: PainterRegistration) -> Value {
    json!({"managerLane":match value.manager_lane { WorldManagerLane::Actor => "actor", WorldManagerLane::Transient => "transient" },
        "registrationOrdinal":value.registration_ordinal})
}

fn phase_id(value: BoneyardGoodiePhase) -> u32 {
    match value {
        BoneyardGoodiePhase::Intact => 0,
        BoneyardGoodiePhase::Opening => 1,
        BoneyardGoodiePhase::Broken => 2,
    }
}

fn bonus_id(value: NativeBonusKind) -> u32 {
    match value {
        NativeBonusKind::SkillChoice => 0,
        NativeBonusKind::SkillRank => 1,
        NativeBonusKind::DamageX4 => 2,
    }
}

fn kind_name(value: NativeLootDropKind) -> &'static str {
    match value {
        NativeLootDropKind::Bonus => "bonus",
        NativeLootDropKind::Gold => "gold",
        NativeLootDropKind::Orb => "orb",
        NativeLootDropKind::Sack => "sack",
    }
}

fn source_name(value: NativeLootDropSource) -> &'static str {
    match value {
        NativeLootDropSource::Enemy => "enemy",
        NativeLootDropSource::Goodie => "goodie",
        NativeLootDropSource::Script => "script",
    }
}

fn orb_name(value: NativeOrbKind) -> &'static str {
    match value {
        NativeOrbKind::Health => "health",
        NativeOrbKind::Mana => "mana",
    }
}

fn actor_snapshot(value: &BoneyardLootActor) -> Value {
    json!({
        "activationDelayTicks":value.activation_delay_ticks,"ageTicks":value.age_ticks,"alpha":value.alpha,
        "amount":value.amount,"animationPhase":value.animation_phase,"bonusKind":value.bonus_kind.map(bonus_id),
        "bounceHeight":value.bounce_height,"bounceVelocity":value.bounce_velocity,"framePhase":value.frame_phase,
        "id":value.id,"item":value.item.as_ref().map(item_snapshot),"kind":kind_name(value.kind),
        "lastStepTick":value.last_step_tick.map_or(-1,|tick|tick as i64),"lifetimeTicksRemaining":value.lifetime_ticks_remaining,
        "nativeTypeId":value.native_type_id,"orbKind":value.orb_kind.map(orb_name),"orbValue":value.orb_value,
        "painterRegistration":registration(value.painter_registration),"position":point_snapshot(value.position),
        "rotationDeg":value.rotation_deg,"scatterSeed":value.scatter_seed,"scatterActive":value.scatter_active,
        "scatterProgress":value.scatter_progress,"settledSoundEmitted":value.settled_sound_emitted,"source":source_name(value.source),
        "spawnTick":value.spawn_tick,"tier":value.tier,
    })
}

fn effect_snapshot(value: &EnemyDeathEffect) -> Value {
    json!({"ageTicks":value.age_ticks,"alpha":value.alpha,"alphaMultiplier":value.alpha_multiplier,
        "alphaLossPerTick":value.alpha_loss_per_tick,"angularVelocityDeg":value.angular_velocity_deg,
        "atlas":value.atlas.id(),"blendMode":value.blend_mode.id(),"bounceRetention":value.bounce_retention,
        "bounceVelocity":value.bounce_velocity,"entry":value.entry,"firstEntry":value.first_entry,"frameCount":value.frame_count,
        "framePhase":value.frame_phase,"frameVelocity":value.frame_velocity,"frameVelocityDamping":value.frame_velocity_damping,
        "frameTicks":value.frame_ticks,"height":value.height,"id":value.id,"kind":value.kind.id(),
        "lastStepTick":value.last_step_tick.map_or(-1,|tick|tick as i64),"lifetimeTicks":value.lifetime_ticks,
        "opacityTimer":value.opacity_timer,"ownerActorId":value.owner_actor_id,
        "painterRegistration":value.painter_registration.map(registration),"presentationOwner":value.presentation_owner.id(),
        "position":point_snapshot(value.position),"role":value.role,"rotationDeg":value.rotation_deg,"scale":value.scale,"scaleY":value.scale_y,
        "scaleMultiplier":value.scale_multiplier,"shadow":value.shadow,"spawnTick":value.spawn_tick,"tint":value.tint,
        "verticalVelocity":value.vertical_velocity,"velocity":point_snapshot(value.velocity),"velocityDamping":value.velocity_damping,
    })
}

fn goodie_snapshot(value: &BoneyardGoodieState) -> Value {
    json!({"active":value.active,"activatedByPlayerId":value.activated_by_player_id,"eid":value.eid,
        "exhausted":value.exhausted,"id":value.id,"phase":phase_id(value.phase),"position":point_snapshot(value.position),
        "rewardSeed":value.reward_seed,"sceneryRegistrationOrdinal":value.scenery_registration_ordinal,
        "subtype":value.subtype,"timer":value.timer})
}

fn store_snapshot(value: &BoneyardLootStore) -> Value {
    json!({"actors":value.actors.iter().map(actor_snapshot).collect::<Vec<_>>(),
        "effects":value.effects.iter().map(effect_snapshot).collect::<Vec<_>>(),
        "goodies":value.goodies.iter().map(goodie_snapshot).collect::<Vec<_>>(),
        "lastStepTick":value.last_step_tick.map_or(-1,|tick|tick as i64),"lastKeyNeededTick":value.last_key_needed_tick,
        "lastSuccessfulItemLevel":value.last_successful_item_level,"nextActorId":value.next_actor_id,"nextEventId":value.next_event_id,
        "nextEffectId":value.next_effect_id,"nextGoodieId":value.next_goodie_id,"nextItemId":value.next_item_id,
        "nextKeyDropLevel":value.next_key_drop_level,"sharedRng":rng_snapshot(&value.shared_rng)})
}

fn event_snapshot(value: &BoneyardLootEvent) -> Value {
    let mut result = json!({"actorId":value.actor_id,"eventId":value.event_id,"position":point_snapshot(value.position),
        "tick":value.tick,"type":match value.event_type { BoneyardLootEventType::GoodieKeyNeeded => "goodie-key-needed",
            BoneyardLootEventType::GoodiePhase => "goodie-phase", BoneyardLootEventType::LootDropSound => "loot-drop-sound", BoneyardLootEventType::LootPickup => "loot-pickup" }});
    if let Some(id) = value.goodie_id {
        result["goodieId"] = json!(id);
    }
    if let Some(phase) = value.phase {
        result["phase"] = json!(phase_id(phase));
    }
    if let Some(rate) = value.playback_rate {
        result["playbackRate"] = json!(rate);
    }
    if let Some(id) = &value.player_id {
        result["playerId"] = json!(id);
    }
    if let Some(sound) = value.sound {
        result["sound"] = json!(sound.as_str());
    }
    if let Some(text) = &value.text {
        result["text"] = json!(text);
    }
    result
}

fn pickup_snapshot(value: &BoneyardLootPickup) -> Value {
    json!({"actorId":value.actor_id,"amount":value.amount,"bonusKind":value.bonus_kind.map(bonus_id),
        "item":value.item.as_ref().map(item_snapshot),"kind":kind_name(value.kind),"orbKind":value.orb_kind.map(orb_name),
        "orbValue":value.orb_value,"playerId":value.player_id,"sourcePosition":point_snapshot(value.source_position),"tick":value.tick})
}

fn read_item(value: &Value) -> HubInventoryItem {
    let equipment_type = value["equipmentType"].as_str().map(|id| match id {
        "hat" => EquipmentType::Hat,
        "robe" => EquipmentType::Robe,
        "staff" => EquipmentType::Staff,
        "wand" => EquipmentType::Wand,
        "ring" => EquipmentType::Ring,
        "amulet" => EquipmentType::Amulet,
        _ => panic!("unknown item type"),
    });
    let kind = match value["kind"].as_str().unwrap() {
        "equipment" => HubItemKind::Equipment,
        "health-potion" => HubItemKind::HealthPotion,
        "mana-potion" => HubItemKind::ManaPotion,
        "rejuvenation-potion" => HubItemKind::RejuvenationPotion,
        "wizard-chug" => HubItemKind::WizardChug,
        "mind-chug" => HubItemKind::MindChug,
        "antidote" => HubItemKind::Antidote,
        "dye" => HubItemKind::Dye,
        "key" => HubItemKind::Key,
        "sack" => HubItemKind::Sack,
        "skill-book" => HubItemKind::SkillBook,
        _ => panic!("unknown item kind"),
    };
    HubInventoryItem {
        mod_content: None,
        mod_item_content: None,
        mod_affixes: None,
        contents: value["contents"].as_array().map(|rows| rows.iter().map(read_item).collect()),
        equipment_type,
        generated_level: value["generatedLevel"].as_u64().map(|level| level as u32),
        icon_records: indexes(&value["iconRecords"]),
        icon_tints: value["iconTints"].as_array().map(|rows| {
            rows.iter()
                .map(|row| row.as_u64().map(|tint| tint as u32))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        }),
        id: value["id"].as_u64().unwrap(),
        inventory_slot: value["inventorySlot"].as_u64().map(|slot| slot as usize),
        kind,
        name: value["name"].as_str().unwrap().to_owned(),
        native_subtype: value["nativeSubtype"].as_u64().map(|id| id as u32),
        native_selector: value["nativeSelector"].as_u64().map(|id| id as u32),
        native_effects: value["nativeEffects"].as_array().map(|rows| {
            rows.iter()
                .map(|effect| NativeEquipmentEffect {
                    kind: effect["kind"].as_u64().unwrap() as u32,
                    magnitude: effect["magnitude"].as_f64().unwrap(),
                    operator: effect["operator"].as_u64().unwrap() as u32,
                    target: effect["target"].as_u64().unwrap() as u32,
                })
                .collect()
        }),
        native_type_id: value["nativeTypeId"].as_u64().unwrap() as u32,
        quantity: value["quantity"].as_u64().unwrap() as u32,
        rarity: value["rarity"].as_str().map(|value| match value {
            "Epic" => ItemRarity::Epic,
            "Rare" => ItemRarity::Rare,
            _ => panic!("unknown rarity"),
        }),
        recipe_index: value["recipeIndex"].as_u64().map(|id| id as usize),
    }
}

fn read_drop(value: &Value) -> NativeLootDropSpec {
    let kind = match value["kind"].as_str().unwrap() {
        "bonus" => NativeLootDropKind::Bonus,
        "gold" => NativeLootDropKind::Gold,
        "orb" => NativeLootDropKind::Orb,
        "sack" => NativeLootDropKind::Sack,
        _ => panic!("unknown ground actor kind"),
    };
    let mut result = NativeLootDropSpec::new(
        kind,
        value["id"].as_u64().unwrap(),
        point(&value["position"]),
        match value["source"].as_str().unwrap() {
            "enemy" => NativeLootDropSource::Enemy,
            "goodie" => NativeLootDropSource::Goodie,
            "script" => NativeLootDropSource::Script,
            _ => panic!("unknown drop source"),
        },
    );
    result.activation_delay_ticks = value["activationDelayTicks"].as_i64().unwrap();
    result.amount = value["amount"].as_u64();
    result.bonus_kind =
        value["bonusKind"].as_u64().map(|id| NativeBonusKind::from_id(id as u32).unwrap());
    result.item = if value["item"].is_null() { None } else { Some(read_item(&value["item"])) };
    result.orb_kind = value["orbKind"].as_str().map(|id| match id {
        "health" => NativeOrbKind::Health,
        "mana" => NativeOrbKind::Mana,
        _ => panic!("unknown orb"),
    });
    result.phase = value["phase"].as_f64().unwrap();
    result.rotation_deg = value["rotationDeg"].as_f64();
    result.scatter_seed = value["scatterSeed"].as_u64().map(|seed| seed as u32);
    result.tier = value["tier"].as_u64().map(|tier| tier as u32);
    result.value = value["value"].as_f64();
    result
}

fn goodie_input(value: &Value) -> BoneyardGoodieInput {
    BoneyardGoodieInput {
        eid: value["eid"].as_str().unwrap().to_owned(),
        position: point(&value["position"]),
        reward_seed: value["rewardSeed"].as_u64().map(|seed| seed as u32),
        scenery_registration_ordinal: value["sceneryRegistrationOrdinal"]
            .as_u64()
            .map(|value| value as usize),
        subtype: value["subtype"].as_u64().map(|value| value as u32),
    }
}

#[test]
fn ground_loot_goodie_effects_pickups_and_completion_cleanup_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/website/loot-store-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let goodies: Vec<_> =
            case["goodies"].as_array().unwrap().iter().map(goodie_input).collect();
        let mut current = BoneyardLootStore::new(case["seed"].as_str().unwrap(), &goodies);
        let mut order = WorldPainterOrder { next_actor_ordinal: 10, next_transient_ordinal: 20 };
        let name = case["name"].as_str().unwrap();
        support::assert_website_json(&store_snapshot(&current), &case["initial"], name, &[]);
        for (index, record) in case["records"].as_array().unwrap().iter().enumerate() {
            let operation = &record["operation"];
            let mut output = json!({});
            match operation["kind"].as_str().unwrap() {
                "spawn" => {
                    let specs =
                        operation["specs"].as_array().unwrap().iter().map(read_drop).collect();
                    output["rejectedCount"] = json!(current.spawn_specs(
                        specs,
                        operation["tick"].as_u64().unwrap(),
                        &mut order
                    ));
                }
                "step" => {
                    let rows = operation["participants"].as_array().unwrap();
                    let advanced: Vec<_> =
                        rows.iter().map(|row| flags(&row["advancedUnlocks"])).collect();
                    let owned: Vec<_> =
                        rows.iter().map(|row| indexes(&row["ownedRecipeIndexes"])).collect();
                    let participants: Vec<_> = rows
                        .iter()
                        .enumerate()
                        .map(|(index, row)| {
                            let value = &row["modifiers"];
                            BoneyardLootParticipant {
                                advanced_unlocks: &advanced[index],
                                alive: row["alive"].as_bool().unwrap(),
                                connected: row["connected"].as_bool().unwrap(),
                                heading_index: row["headingIndex"].as_u64().unwrap() as usize,
                                level: row["level"].as_u64().unwrap() as u32,
                                modifiers: NativeLootModifiers {
                                    gold_amount: value["goldAmount"].as_f64().unwrap(),
                                    gold_chance: value["goldChance"].as_f64().unwrap(),
                                    item_chance: value["itemChance"].as_f64().unwrap(),
                                    orb_chance: value["orbChance"].as_f64().unwrap(),
                                    orb_pull: value["orbPull"].as_f64().unwrap(),
                                    orb_value_bonus: value["orbValueBonus"].as_bool().unwrap(),
                                    pickup_factor: value["pickupFactor"].as_f64().unwrap(),
                                    powerup_chance: value["powerupChance"].as_f64().unwrap(),
                                },
                                owned_recipe_indexes: &owned[index],
                                player_id: row["playerId"].as_str().unwrap(),
                                position: point(&row["position"]),
                            }
                        })
                        .collect();
                    let placement = |position: Vec2, _: f64| {
                        operation["blocked"] != true
                            || position.x < 95.0
                            || position.x > 105.0
                            || position.y < 95.0
                            || position.y > 105.0
                    };
                    let result = current.step(
                        &participants,
                        &placement,
                        operation["tick"].as_u64().unwrap(),
                        &mut order,
                    );
                    output = json!({"events":result.events.iter().map(event_snapshot).collect::<Vec<_>>(),"pickups":result.pickups.iter().map(pickup_snapshot).collect::<Vec<_>>()});
                }
                "activate" => current.activate_goodie(
                    operation["eid"].as_str().unwrap(),
                    operation["playerId"].as_str().unwrap(),
                ),
                "key-needed" => {
                    output["event"] = current
                        .goodie_key_needed(
                            operation["goodieId"].as_u64().unwrap() as usize,
                            operation["playerId"].as_str().unwrap(),
                            operation["tick"].as_u64().unwrap(),
                        )
                        .as_ref()
                        .map_or(Value::Null, event_snapshot);
                }
                "retire" => {
                    let value = &operation["bounds"];
                    current.retire_goodies_outside_bounds(BoneyardBounds::new(
                        value["x"].as_f64().unwrap(),
                        value["y"].as_f64().unwrap(),
                        value["w"].as_f64().unwrap(),
                        value["h"].as_f64().unwrap(),
                    ));
                }
                "remove-last-word" => {
                    let last_word = current.native_hagatha_last_word_loot();
                    output = json!({"actorIds":last_word.actor_ids,"gold":last_word.gold,"items":last_word.items.iter().map(item_snapshot).collect::<Vec<_>>()});
                    current.remove_actors(&last_word.actor_ids);
                }
                "custom" => {
                    let items: Vec<_> =
                        operation["items"].as_array().unwrap().iter().map(read_item).collect();
                    output["rejectedCount"] = json!(current.spawn_custom_items(
                        &items,
                        point(&operation["position"]),
                        operation["tick"].as_u64().unwrap(),
                        &mut order
                    ));
                }
                "materialize" => {
                    let input = &operation["input"];
                    let advanced = flags(&input["advancedUnlocks"]);
                    let owned = indexes(&input["ownedRecipeIndexes"]);
                    let mut wire_input = json!({"actorSeed":input["actorSeed"],"arena":input["arena"],"explicitGoldAmount":null,"dropDelayContext":0,
                        "inventoryHasHealthPotion":input["inventoryHasHealthPotion"],"key":{"current":1,"level":0,"remaining":0},
                        "nearbyMaskTwoCount":input["nearbyMaskTwoCount"],"participant":{"advancedUnlocks":input["advancedUnlocks"],"level":input["participantLevel"],
                            "modifiers":input["modifiers"],"ownedRecipeIndexes":input["ownedRecipeIndexes"],"slot":input["participantSlot"]},
                        "policies":input["policies"],"sceneForcesHealthPotion":input["sceneForcesHealthPotion"],"sourcePosition":input["position"],
                        "worldBadguyCount":input["worldBadguyCount"],"worldHasHealthPotionSack":input["worldHasHealthPotionSack"]});
                    wire_input["arena"]["lastSuccessfulItemLevel"] = json!(-1);
                    let selection =
                        selection_input(&wire_input, &advanced, &owned, &NativeLootOpenPlacement);
                    let program = if input["onDeathProgram"].is_null() {
                        None
                    } else {
                        Some(BoneyardLootDeathProgram::MinibossDie)
                    };
                    output["rejectedCount"] = json!(current.materialize_enemy_loot(
                        selection,
                        program,
                        input["tick"].as_u64().unwrap(),
                        &mut order
                    ));
                }
                "seed" => {
                    output["seed"] =
                        json!(current.roll_loot_seed(operation["bound"].as_u64().unwrap() as u32));
                }
                other => panic!("unknown loot operation {other}"),
            }
            let path = format!("{name}[{index}]");
            support::assert_website_json(&output, &record["output"], &path, &[]);
            support::assert_website_json(
                &store_snapshot(&current),
                &record["expected"],
                &path,
                &[],
            );
            support::assert_website_json(
                &json!({"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}),
                &record["nextRegistrationOrdinal"],
                &path,
                &[],
            );
        }
    }
    let expected = &fixture["capacity"];
    for row in fixture["nearest"].as_array().unwrap() {
        let source = row["goodies"].as_array().unwrap();
        let inputs: Vec<_> = source.iter().map(goodie_input).collect();
        let mut current = BoneyardLootStore::new("nearest", &inputs);
        for (goodie, input) in current.goodies.iter_mut().zip(source) {
            goodie.id = input["id"].as_u64().unwrap() as usize;
            goodie.active = input["active"].as_bool().unwrap();
            goodie.exhausted = input["exhausted"].as_bool().unwrap();
            goodie.phase = match input["phase"].as_u64().unwrap() {
                0 => BoneyardGoodiePhase::Intact,
                1 => BoneyardGoodiePhase::Opening,
                2 => BoneyardGoodiePhase::Broken,
                _ => panic!("invalid source Goodie phase"),
            };
        }
        let selected = current.nearest_goodie(
            point(&row["actor"]["position"]),
            row["actor"]["headingIndex"].as_u64().unwrap() as usize,
        );
        assert_eq!(selected.map(|goodie| goodie.id as u64), row["expected"].as_u64());
    }
    let mut current = BoneyardLootStore::new("capacity", &[]);
    let mut order = WorldPainterOrder::default();
    current.next_actor_id = 2047;
    let mut gold = NativeLootDropSpec::new(
        NativeLootDropKind::Gold,
        0,
        Vec2::ZERO,
        NativeLootDropSource::Enemy,
    );
    gold.amount = Some(1);
    let rejected = current.spawn_specs(vec![gold.clone(); 2048], 1, &mut order);
    assert_eq!(rejected, expected["filledRejected"].as_u64().unwrap() as usize);
    assert_eq!(current.next_actor_id, expected["filledNext"].as_u64().unwrap() as usize);
    assert_eq!(
        json!(current.actors.iter().map(|actor| actor.id).collect::<Vec<_>>()),
        expected["filledIds"]
    );
    current.remove_actors(&[2047, 2]);
    let rejected = current.spawn_specs(vec![gold; 3], 2, &mut order);
    assert_eq!(rejected, expected["reusedRejected"].as_u64().unwrap() as usize);
    assert_eq!(current.next_actor_id, expected["reusedNext"].as_u64().unwrap() as usize);
    assert_eq!(
        json!(current.actors.iter().map(|actor| actor.id).collect::<Vec<_>>()),
        expected["reusedIds"]
    );
    support::assert_website_json(
        &json!({"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}),
        &expected["nextRegistrationOrdinal"],
        "capacity painter order",
        &[],
    );
}

#[test]
fn explicit_goodie_interaction_consumes_one_nested_key_and_preserves_failed_attempts() {
    use solomon_sim::economy::HubEconomyState;
    use solomon_sim::loot::items::misc_item;
    use solomon_sim::profile::WizardElement;

    let goodie = BoneyardGoodieInput {
        eid: "locked".into(),
        position: Vec2::new(0.0, -25.0),
        reward_seed: Some(0),
        scenery_registration_ordinal: Some(10),
        subtype: Some(0),
    };
    let mut store = BoneyardLootStore::new("explicit-goodie", &[goodie]);
    let mut economy = HubEconomyState::new(1, WizardElement::Ether);
    let original_economy = economy.clone();
    let first = store.interact_goodie(&mut economy, "local", Vec2::ZERO, 0, 0);
    assert!(matches!(first, BoneyardGoodieInteraction::MissingKey { event: Some(_) }));
    assert_eq!(economy, original_economy);
    assert!(matches!(
        store.interact_goodie(&mut economy, "local", Vec2::ZERO, 0, 200),
        BoneyardGoodieInteraction::MissingKey { event: None }
    ));
    let mut ids = NativeLootItemIds::new(100);
    let key = misc_item(&mut ids, 1);
    let mut sack = economy.backpack[0].clone();
    sack.native_type_id = 7008;
    sack.native_subtype = Some(0);
    sack.kind = HubItemKind::Sack;
    sack.name = "Sack".to_owned();
    sack.icon_records = vec![70];
    sack.contents = Some(vec![key]);
    assert!(economy.insert_loot_item(sack));
    assert!(economy.has_wizard_key());
    assert_eq!(
        store.interact_goodie(&mut economy, "local", Vec2::ZERO, 0, 201),
        BoneyardGoodieInteraction::Opened { goodie_id: 1 }
    );
    assert!(!economy.has_wizard_key());
    assert!(store.goodies[0].active);
    assert_eq!(store.goodies[0].activated_by_player_id.as_deref(), Some("local"));
    assert_eq!(
        store.interact_goodie(&mut economy, "local", Vec2::ZERO, 0, 202),
        BoneyardGoodieInteraction::NoTarget
    );
}
