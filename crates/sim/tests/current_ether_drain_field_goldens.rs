//! Complete current EtherDrain constructor/epoch program through the public secondary owner.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
mod native_bits;
#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod support;

use native_bits::{compare, input, source_value};
use serde_json::{Value, json};
use solomon_sim::belt::{NativeBeltEntry, PlayerBeltComponent};
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::secondary::ether_drain::*;
use solomon_sim::secondary::*;
use solomon_sim::{HubWizard, NativeRng, Vec2};
use std::collections::BTreeMap;

struct FieldWorld {
    targets: Vec<NativeEtherDrainTarget>,
    animations: Vec<NativeEtherDrainWorldAnimationTarget>,
    scenery: Vec<NativeSecondarySceneryTarget>,
}
impl NativeSecondaryWorld for FieldWorld {
    fn target_exists(&self, _: &str, _: usize) -> bool {
        false
    }
    fn dampen_candidates(&self, _: &str, _: Vec2) -> NativeSecondaryDampenCandidates {
        NativeSecondaryDampenCandidates::default()
    }
    fn phasing_destination(&self, _: &str, _: Vec2, _: Vec2) -> Option<Vec2> {
        None
    }
    fn golem_movement(&self, _: &str, _: &str, _: Vec2, requested: Vec2, _: f64) -> Vec2 {
        requested
    }
    fn golem_placement(&self, _: &str, _: &str, requested: Vec2, _: &mut NativeRng) -> Vec2 {
        requested
    }
    fn teleport_destination(&self, _: &str, _: &mut NativeRng) -> Vec2 {
        Vec2::ZERO
    }
    fn target(&self, _: &str, _: usize) -> Option<NativeSecondaryTarget> {
        None
    }
    fn targets(&self, _: &str, _: Vec2, _: f64) -> Vec<NativeSecondaryTarget> {
        Vec::new()
    }
    fn ether_drain_targets(&self, _: &str) -> Option<Vec<NativeEtherDrainTarget>> {
        Some(self.targets.clone())
    }
    fn ether_drain_world_animations(&self, _: &str) -> Vec<NativeEtherDrainWorldAnimationTarget> {
        self.animations.clone()
    }
    fn scenery_targets(&self, _: &str, _: Vec2, _: f64) -> Vec<NativeSecondarySceneryTarget> {
        self.scenery.clone()
    }
}
fn snapshot(
    state: &NativeSecondarySimulationState,
    order: WorldPainterOrder,
    result: &NativeSecondaryTickResult,
) -> Value {
    let actors: Vec<_> = state
        .actors
        .iter()
        .map(|actor| {
            let mut value = support::actor_snapshot(&actor);
            if actor.kind == NativeSecondaryActorKind::EtherDrain || actor.ether_drain().is_some() {
                value["etherDrain"] = serde_json::to_value(actor.ether_drain()).unwrap();
            }
            value
        })
        .collect();
    source_value(
        &json!({"actors":actors,"events":state.events.iter().map(support::event_snapshot).collect::<Vec<_>>(),
        "nextActorId":state.next_actor_id,"nextEventId":state.next_event_id,"rng":support::rng_snapshot(&state.rng),
        "contacts":result.ether_drain_contacts,"animationContacts":result.ether_drain_world_animation_contacts,
        "order":{"nextRegistrationOrdinal":{"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}}}),
    )
}

#[test]
fn current_field_children_private_manager_captures_and_shared_birth_order_match_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-ether-drain-field-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 4);
    for case in fixture["cases"].as_array().unwrap() {
        let declared = &case["player"];
        let book = support::read_book(&declared["skillBook"]);
        let mut belt = PlayerBeltComponent::new(&book);
        belt.entries[0] = Some(NativeBeltEntry::Skill { skill_id: 74 });
        let wizard = HubWizard::at(Vec2::ZERO);
        let mut state = NativeSecondarySimulationState::new(91);
        let mut order = WorldPainterOrder::default();
        let mut world = FieldWorld {
            targets: input(&case["targets"]),
            animations: input(&case["animationTargets"]),
            scenery: case["scenery"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| NativeSecondarySceneryTarget {
                    id: row["id"].as_u64().unwrap() as usize,
                    position: input(&row["position"]),
                    type_id: row["typeId"].as_u64().unwrap() as u32,
                    secondary_variant: row["secondaryVariant"].as_u64().map(|value| value as u32),
                })
                .collect(),
        };
        let mut frame_index = 0;
        for tick in 1..=1080 {
            if tick == 170 {
                world.targets.retain(|target| {
                    !matches!(target.reference, NativeEtherDrainTargetRef::Loot { .. })
                });
            }
            let mut authority = support::authority_from_fixture(declared, &book, &belt, &wizard);
            authority.input.aim = Some(Vec2::new(250.0, 250.0));
            authority.input.quickbar = if tick == 1 { Some(0) } else { None };
            let players = BTreeMap::from([("player".to_owned(), authority)]);
            let result = state.step(
                &NativeSecondaryTickContext { enhanced_effects: None, players: &players, tick },
                &mut world,
                Some(&mut order),
            );
            if case["spatial"].as_bool().unwrap() && tick == 10 {
                state.capture_ether_drain_image(
                    1,
                    Some(NativeEtherDrainCapturedImage {
                        atlas: NativeEtherDrainAtlas::Demon,
                        entry: 80,
                        tint: 0xffffff,
                        body_y_offset: 0,
                    }),
                    tick,
                );
            }
            if case["spatial"].as_bool().unwrap() && tick == 16 {
                state.capture_ether_drain_image(1, None, tick);
            }
            let frames = case["frames"].as_array().unwrap();
            if frames.get(frame_index).is_some_and(|frame| frame["tick"].as_u64() == Some(tick)) {
                compare(
                    &snapshot(&state, order, &result),
                    &source_value(&frames[frame_index]["expected"]),
                    &format!("quality{} spatial{} tick{tick}", case["enhanced"], case["spatial"]),
                );
                frame_index += 1;
            }
        }
        assert_eq!(frame_index, case["frames"].as_array().unwrap().len());
    }
}
