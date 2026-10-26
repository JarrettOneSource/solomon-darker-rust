#![cfg(feature = "persistence")]

use solomon_sim::Vec2;
use solomon_sim::boneyard::loot::{BoneyardLootParticipant, BoneyardLootStore};
use solomon_sim::economy::HubEconomyState;
use solomon_sim::loot::{
    NativeLootDropKind, NativeLootDropSource, NativeLootDropSpec, NativeLootModifiers,
};
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::profile::WizardElement;

#[test]
fn retired_pickups_do_not_reorder_survivors_or_lose_saved_contents() {
    let mut store = BoneyardLootStore::new("loot-ecs", &[]);
    let mut order = WorldPainterOrder::default();
    let item = HubEconomyState::new(77, WizardElement::Air).backpack[0].clone();
    let drop =
        |kind, id| NativeLootDropSpec::new(kind, id, Vec2::ZERO, NativeLootDropSource::Script);
    let mut gold = drop(NativeLootDropKind::Gold, 1);
    gold.activation_delay_ticks = 0;
    gold.amount = Some(7);
    let mut expired = drop(NativeLootDropKind::Orb, 2);
    expired.value = Some(0.0);
    let mut sack = drop(NativeLootDropKind::Sack, 3);
    sack.activation_delay_ticks = 0;
    sack.item = Some(item.clone());
    assert_eq!(store.spawn_specs(vec![gold.clone(), expired, sack], 0, &mut order), 0);

    let original_ids: Vec<_> = store.actors.iter().map(|actor| actor.id).collect();
    let placement = |_: Vec2, _: f64| true;
    store.step(&[], &placement, 0, &mut order);
    assert_eq!(
        store.actors.iter().map(|actor| actor.id).collect::<Vec<_>>(),
        [original_ids[0], original_ids[2]]
    );
    assert_eq!(store.spawn_specs(vec![gold], 1, &mut order), 0);
    let ids: Vec<_> = store.actors.iter().map(|actor| actor.id).collect();

    let saved = serde_json::to_vec(&store).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert!(document["actors"][1].get("body").is_none());
    assert!(document["actors"][1].get("reward").is_none());
    assert_eq!(document["actors"][1]["item"], serde_json::to_value(&item).unwrap());
    let mut restored: BoneyardLootStore = serde_json::from_slice(&saved).unwrap();
    assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);

    let participant = BoneyardLootParticipant {
        advanced_unlocks: &[false; 8],
        alive: true,
        connected: true,
        heading_index: 0,
        level: 1,
        modifiers: NativeLootModifiers::default(),
        owned_recipe_indexes: &[],
        player_id: "wizard",
        position: Vec2::ZERO,
    };
    let mut restored_order = order;
    let accepted = store.step(&[participant], &placement, 1, &mut order);
    let continued = restored.step(&[participant], &placement, 1, &mut restored_order);
    assert_eq!(accepted.pickups.iter().map(|pickup| pickup.actor_id).collect::<Vec<_>>(), ids);
    assert_eq!(continued.pickups.iter().map(|pickup| pickup.actor_id).collect::<Vec<_>>(), ids);
    assert_eq!(accepted.pickups[1].item, Some(item.clone()));
    assert_eq!(continued.pickups[1].item, Some(item));
    assert_eq!(accepted.events, continued.events);
    assert!(store.actors.is_empty());
    assert_eq!(store, restored);
    assert_eq!(order, restored_order);
}
