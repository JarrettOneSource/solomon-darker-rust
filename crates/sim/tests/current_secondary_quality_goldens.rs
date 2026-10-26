//! Global quality controls current births independently of retained owner/actor flags.
#[path = "support/mod.rs"]
mod exact;
#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use solomon_sim::belt::{NativeBeltEntry, PlayerBeltComponent};
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::secondary::{NativeSecondarySimulationState, NativeSecondaryTickContext};
use solomon_sim::{HubWizard, Vec2};
use std::collections::BTreeMap;
#[test]
fn active_secondary_births_follow_current_global_quality_and_retained_owner_fallback() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-secondary-quality-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let declared = &case["player"];
        let book = support::read_book(&declared["skillBook"]);
        let mut belt = PlayerBeltComponent::new(&book);
        let skill_id = case["skillId"].as_u64().unwrap() as usize;
        belt.entries[0] = Some(NativeBeltEntry::Skill { skill_id });
        let wizard = HubWizard::at(Vec2::ZERO);
        // Source's omitted scenery callback returns an empty collection.
        let empty = json!({"scenery":[]});
        let mut world = support::World { case: &empty, targets: Vec::new() };
        let mut state = NativeSecondarySimulationState::new(91);
        let mut order = WorldPainterOrder::default();
        let initial_quality = case["initialQuality"].as_bool().unwrap();
        let frames = case["frames"].as_array().unwrap();
        let mut next_frame = 0;
        for tick in 1..=420 {
            let mut authority = support::authority_from_fixture(declared, &book, &belt, &wizard);
            authority.input.aim = Some(Vec2::new(100.0, 0.0));
            authority.input.primary = skill_id == 12 && tick % 40 < 20;
            authority.input.quickbar = if tick == 1 { Some(0) } else { None };
            let players = BTreeMap::from([("player".to_owned(), authority)]);
            state.step(
                &NativeSecondaryTickContext {
                    enhanced_effects: Some(if tick <= 200 {
                        initial_quality
                    } else {
                        !initial_quality
                    }),
                    players: &players,
                    tick,
                },
                &mut world,
                Some(&mut order),
            );
            if frames.get(next_frame).is_none_or(|frame| frame["tick"].as_u64() != Some(tick)) {
                continue;
            }
            let actual = json!({"actors":state.actors.iter().map(|actor| support::actor_snapshot(&actor)).collect::<Vec<_>>(),
                "player":support::player_snapshot(&state.players["player"]),
                "events":state.events.iter().map(support::event_snapshot).collect::<Vec<_>>(),
                "rng":support::rng_snapshot(&state.rng),"nextActorId":state.next_actor_id,"nextEventId":state.next_event_id,
                "order":{"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}});
            exact::assert_website_json(
                &actual,
                &frames[next_frame]["expected"],
                &format!("skill{skill_id} quality{initial_quality} tick{tick}"),
                &[],
            );
            next_frame += 1;
        }
        assert_eq!(next_frame, frames.len());
    }
}
