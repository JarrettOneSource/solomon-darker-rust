//! Golem assembly and destruction retain actual child actors, painter owners and RNG.
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
use solomon_sim::secondary::golem::NativeGolemIncomingDamage;
use solomon_sim::secondary::presentation::{
    NativeGolemDeathMember, NativeSecondaryBlend, NativeSecondaryPresentationPlan,
    NativeSecondaryQueueFamily, native_golem_death_member_plan,
    native_golem_death_presentation_plan,
};
use solomon_sim::secondary::{
    NativeSecondaryActorKind, NativeSecondarySimulationState, NativeSecondaryTarget,
    NativeSecondaryTickContext, NativeSecondaryWorld,
};
use solomon_sim::{HubWizard, NativeRng, Vec2};
use std::collections::BTreeMap;

struct OpenWorld;
impl NativeSecondaryWorld for OpenWorld {
    fn dampen_candidates(
        &self,
        _: &str,
        _: Vec2,
    ) -> solomon_sim::secondary::NativeSecondaryDampenCandidates {
        Default::default()
    }
    fn phasing_destination(&self, _: &str, _: Vec2, _: Vec2) -> Option<Vec2> {
        None
    }
    fn target_exists(&self, _: &str, _: usize) -> bool {
        false
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
fn snapshot(
    state: &NativeSecondarySimulationState,
    order: WorldPainterOrder,
    received: Value,
) -> Value {
    let mut death_plans = Vec::new();
    for actor in &state.actors {
        if actor.kind != NativeSecondaryActorKind::GolemDeath {
            continue;
        }
        for (member, plan) in [
            (json!("all"), native_golem_death_presentation_plan(actor)),
            (
                json!(0),
                native_golem_death_member_plan(
                    actor,
                    actor.age_ticks as f64,
                    NativeGolemDeathMember::Fragment(0),
                ),
            ),
            (
                json!("star"),
                native_golem_death_member_plan(
                    actor,
                    actor.age_ticks as f64,
                    NativeGolemDeathMember::Star,
                ),
            ),
        ] {
            death_plans
                .push(json!({"id": actor.id, "member": member, "plan": plan_projection(&plan)}));
        }
    }
    let actors: Vec<_> = state
        .actors
        .iter()
        .map(|actor| {
            let mut value = support::actor_snapshot(actor);
            if let Some(animation) = &actor.golem_death {
                value["golemDeath"] = serde_json::to_value(animation).unwrap();
            }
            value
        })
        .collect();
    native_bits::source_value(&json!({
        "actors": actors, "deathPlans": death_plans,
        "events": state.events.iter().map(support::event_snapshot).collect::<Vec<_>>(),
        "rng": support::rng_snapshot(&state.rng), "nextActorId": state.next_actor_id,
        "nextEventId": state.next_event_id, "received": received,
        "order": {"nextRegistrationOrdinal": {
            "actor": order.next_actor_ordinal, "transient": order.next_transient_ordinal,
        }},
    }))
}

fn plan_projection(plan: &NativeSecondaryPresentationPlan) -> Value {
    let draws: Vec<_> = plan
        .draws
        .iter()
        .map(|draw| {
            json!({
                "alpha": draw.alpha, "atlas": draw.atlas.as_str(),
                "blend": if draw.blend == NativeSecondaryBlend::Add { "add" } else { "normal" },
                "entry": draw.entry, "offset": draw.offset, "role": draw.role,
                "rotationRadians": draw.rotation_radians, "scaleX": draw.scale_x,
                "scaleY": draw.scale_y, "tint": draw.tint,
            })
        })
        .collect();
    json!({
        "root": plan.root, "worldY": plan.world_y, "draws": draws,
        "queueFamily": if plan.queue_family == NativeSecondaryQueueFamily::OrdinaryDynamic {
            "ordinary-dynamic"
        } else { "zanim" },
    })
}

#[test]
fn current_golem_assembly_and_death_keep_real_motion_after_the_owner_leaves() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-golem-lifecycle-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let declared = &case["player"];
        let book = support::read_book(&declared["skillBook"]);
        let mut belt = PlayerBeltComponent::new(&book);
        belt.entries[0] = Some(NativeBeltEntry::Skill { skill_id: 45 });
        let wizard = HubWizard::at(Vec2::ZERO);
        let mut state = NativeSecondarySimulationState::new(91);
        let mut order = WorldPainterOrder::default();
        let frames = case["frames"].as_array().unwrap();
        let mut frame_index = 0;
        for tick in 1..=1030 {
            if !case["iron"].as_bool().unwrap() && tick == 311 {
                state.remove_owner("player");
            }
            let mut authority = support::authority_from_fixture(declared, &book, &belt, &wizard);
            authority.input.aim = Some(Vec2::new(250.0, 250.0));
            authority.input.quickbar = if tick == 1 { Some(0) } else { None };
            let players = if !case["iron"].as_bool().unwrap() && tick > 310 {
                BTreeMap::new()
            } else {
                BTreeMap::from([("player".to_owned(), authority)])
            };
            state.step(
                &NativeSecondaryTickContext { enhanced_effects: None, players: &players, tick },
                &mut OpenWorld,
                Some(&mut order),
            );
            let mut received = Value::Null;
            if tick == 2 || tick == 310 {
                let actor_id = state
                    .actors
                    .iter()
                    .find(|actor| actor.kind == NativeSecondaryActorKind::Golem)
                    .unwrap()
                    .id;
                let result = state.apply_golem_damage(
                    actor_id,
                    NativeGolemIncomingDamage {
                        primary_damage: 0.0,
                        reflectable_physical_source_in_range: false,
                        secondary_damage: 100000.0,
                    },
                    tick,
                    case["enhanced"].as_bool().unwrap(),
                    Some(&mut order),
                );
                received = json!({"ignored": result.ignored, "killed": result.killed,
                    "ownerId": result.owner_id, "reflectedDamage": result.reflected_damage});
            }
            if frames.get(frame_index).is_none_or(|frame| frame["tick"].as_u64() != Some(tick)) {
                continue;
            }
            native_bits::compare(
                &snapshot(&state, order, received),
                &native_bits::source_value(&frames[frame_index]["expected"]),
                &format!("quality{} iron{} tick{tick}", case["enhanced"], case["iron"]),
            );
            frame_index += 1;
        }
        assert_eq!(frame_index, frames.len());
    }
}
