//! Real secondary casts keep their source auxiliary actors and input eligibility.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;
#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use solomon_sim::belt::{NativeBeltEntry, PlayerBeltComponent};
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::secondary::presentation::{
    NativeSecondaryBlend, NativeSecondarySpriteDraw, native_secondary_presentation_plan,
};
use solomon_sim::secondary::{
    NativeSecondaryDampenCandidates, NativeSecondarySimulationState, NativeSecondaryTarget,
    NativeSecondaryTickContext, NativeSecondaryWorld,
};
use solomon_sim::{HubWizard, NativeRng, Vec2};
use std::collections::BTreeMap;

struct OpenWorld;
impl NativeSecondaryWorld for OpenWorld {
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
}

fn snapshot(state: &NativeSecondarySimulationState, order: WorldPainterOrder) -> Value {
    let plans:Vec<_>=state.actors.iter().take(2).map(|actor| {
        let plan=native_secondary_presentation_plan(&actor,actor.age_ticks as f64,1.0);
        json!({"id":actor.id,"root":plan.root,"draws":plan.draws.iter().map(draw_snapshot).collect::<Vec<_>>()})
    }).collect();
    native_bits::source_value(&json!({
        "plans":plans,
        "actors":state.actors.iter().map(|actor| support::actor_snapshot(&actor)).collect::<Vec<_>>(),
        "events":state.events.iter().map(support::event_snapshot).collect::<Vec<_>>(),
        "player":support::player_snapshot(state.players.get("player").unwrap()),
        "rng":support::rng_snapshot(&state.rng),"nextActorId":state.next_actor_id,
        "nextEventId":state.next_event_id,"order":{"nextRegistrationOrdinal":{
            "actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}},
    }))
}

fn draw_snapshot(draw: &NativeSecondarySpriteDraw) -> Value {
    json!({"alpha":draw.alpha,"atlas":draw.atlas.as_str(),
        "blend":if draw.blend==NativeSecondaryBlend::Add {"add"}else{"normal"},
        "entry":draw.entry,"offset":draw.offset,"role":draw.role,
        "rotationRadians":draw.rotation_radians,"scaleX":draw.scale_x,"scaleY":draw.scale_y,"tint":draw.tint})
}

#[test]
fn current_planewalker_auxiliaries_and_living_input_blocks_replay_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-secondary-auxiliary-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let declared = &case["player"];
        let book = support::read_book(&declared["skillBook"]);
        let mut belt = PlayerBeltComponent::new(&book);
        belt.entries[0] =
            Some(NativeBeltEntry::Skill { skill_id: case["skillId"].as_u64().unwrap() as usize });
        let wizard = HubWizard::at(Vec2::ZERO);
        let mut state = NativeSecondarySimulationState::new(91);
        let mut order = WorldPainterOrder::default();
        let frames = case["frames"].as_array().unwrap();
        let mut frame_index = 0;
        for tick in 1..=900 {
            let mut authority = support::authority_from_fixture(declared, &book, &belt, &wizard);
            authority.alive = tick < 50 || case["mode"] != "dead";
            authority.eligible = tick < 50 || case["mode"] == "eligible";
            authority.input.quickbar = if tick == 1 { Some(0) } else { None };
            let players = BTreeMap::from([("player".into(), authority)]);
            state.step(
                &NativeSecondaryTickContext { enhanced_effects: None, players: &players, tick },
                &mut OpenWorld,
                Some(&mut order),
            );
            if frames.get(frame_index).is_none_or(|frame| frame["tick"].as_u64() != Some(tick)) {
                continue;
            }
            native_bits::compare(
                &snapshot(&state, order),
                &native_bits::source_value(&frames[frame_index]["expected"]),
                &format!(
                    "skill{} quality{} {} tick{tick}",
                    case["skillId"], case["enhanced"], case["mode"]
                ),
            );
            frame_index += 1;
        }
        assert_eq!(frame_index, frames.len());
    }
}
