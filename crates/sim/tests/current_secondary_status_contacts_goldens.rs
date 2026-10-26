//! Current modifier submissions preserve hit metadata, draw order and private births.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;
#[path = "current_secondary_status_contacts_goldens/presentation.rs"]
mod presentation;
#[path = "support/secondary_state.rs"]
#[allow(dead_code)]
mod status_support;
#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod support;

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solomon_sim::belt::PlayerBeltComponent;
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::secondary::*;
use solomon_sim::{HubWizard, NativeRng, Vec2};

struct World {
    targets: Vec<NativeSecondaryTarget>,
}
impl NativeSecondaryWorld for World {
    fn target_exists(&self, key: &str, id: usize) -> bool {
        key == "boneyard:oracle" && self.targets.iter().any(|target| target.id == id)
    }
    fn target(&self, _: &str, id: usize) -> Option<NativeSecondaryTarget> {
        self.targets.iter().find(|target| target.id == id).cloned()
    }
    fn targets(&self, _: &str, center: Vec2, radius: f64) -> Vec<NativeSecondaryTarget> {
        self.targets
            .iter()
            .filter(|target| {
                solomon_sim::float::hypot(
                    target.position.x - center.x,
                    target.position.y - center.y,
                ) <= radius + target.radius
            })
            .cloned()
            .collect()
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
}

fn fixture() -> Value {
    let value: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-secondary-status-contacts-goldens.json"
    ))
    .unwrap();
    assert_eq!(value["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    value
}

fn damage(value: &NativeSecondaryDamageContact) -> Value {
    let mut result = json!({"amount":value.amount,"kind":match value.kind {
        NativeSecondaryDamageKind::Physical=>"physical",NativeSecondaryDamageKind::Magic=>"magic",
        NativeSecondaryDamageKind::Fire=>"fire",NativeSecondaryDamageKind::Ice=>"ice",
        NativeSecondaryDamageKind::Lightning=>"lightning",NativeSecondaryDamageKind::Acid=>"acid",
    },"ownerId":value.owner_id,"sourceActorId":value.source_actor_id,"targetId":value.target_id});
    if value.ether_drain {
        result["etherDrain"] = json!(true);
    }
    if let Some(strength) = value.hit_strength {
        result["hitStrength"] = json!(strength);
    }
    if value.suppress_hit_reaction {
        result["suppressHitReaction"] = json!(true);
    }
    if value.suppress_hurt_sound {
        result["suppressHurtSound"] = json!(true);
    }
    if let Some(payload) = value.special_death_payload {
        result["specialDeathPayload"] = json!({"kind": match payload.kind {
            solomon_sim::spells::special_death::NativeSpellSpecialDeathKind::Fire=>"fire",
            solomon_sim::spells::special_death::NativeSpellSpecialDeathKind::Steam=>"steam",
        },"emberDamage":payload.ember_damage,"emberFragments":payload.ember_fragments,
        "explodeDamage":payload.explode_damage,"explodeRadius":payload.explode_radius,
        "privateSeed":payload.private_seed});
    }
    result
}

#[test]
fn current_source_status_contacts_and_private_electric_births() {
    let fixture = fixture();
    let declared: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-secondary-membership-goldens.json"
    ))
    .unwrap();
    let declared = &declared["waves"][0]["player"];
    let book = support::read_book(&declared["skillBook"]);
    let belt = PlayerBeltComponent::new(&book);
    let wizard = HubWizard::at(Vec2::ZERO);
    for case in fixture["cases"].as_array().unwrap() {
        let mut state = NativeSecondarySimulationState::new(case["seed"].as_i64().unwrap());
        if let Some(source) = case["actor"].as_object() {
            let mut actor = NativeSecondaryActorState::new(
                31,
                NativeSecondaryActorKind::ElectricBurn,
                "player".into(),
                "boneyard:oracle".into(),
                Some(50),
            );
            actor.alpha = 0.0;
            actor.damage = source["damage"].as_f64().unwrap();
            actor.lifetime_ticks = 5;
            actor.light_registration = Some(support::registration(&source["lightRegistration"]));
            actor.position = support::point(&source["position"]);
            actor.target_id = Some(7);
            state.next_actor_id = 32;
            state.actors.push(actor);
        }
        for row in case["patches"].as_array().unwrap() {
            apply_native_secondary_target_effect(
                &mut state.target_effects,
                "boneyard:oracle",
                row["id"].as_u64().unwrap() as usize,
                status_support::patch(&row["patch"]),
            );
        }
        let mut order = WorldPainterOrder::default();
        for frame in case["frames"].as_array().unwrap() {
            let tick = frame["tick"].as_u64().unwrap();
            let absent = case["absentFrom"].as_u64().unwrap_or(u64::MAX);
            let mut targets: Vec<_> = fixture["targets"]
                .as_array()
                .unwrap()
                .iter()
                .map(support::target)
                .filter(|target| !(target.id == 7 && tick >= absent))
                .collect();
            if case["reverse"] == true {
                targets.reverse();
            }
            let mut authority = support::authority_from_fixture(declared, &book, &belt, &wizard);
            authority.input.quickbar = None;
            let players = BTreeMap::from([("player".into(), authority)]);
            let output = state.step(
                &NativeSecondaryTickContext {
                    enhanced_effects: case["enhancedEffects"].as_bool(),
                    players: &players,
                    tick,
                },
                &mut World { targets },
                Some(&mut order),
            );
            let actual = json!({"actors":state.actors.iter().map(|actor| support::actor_snapshot(&actor)).collect::<Vec<_>>(),
                "damage":output.damage.iter().map(damage).collect::<Vec<_>>(),
                "effects":state.target_effects.iter().map(support::effect_snapshot).collect::<Vec<_>>(),
                "events":state.events.iter().map(support::event_snapshot).collect::<Vec<_>>(),
                "rng":support::rng_snapshot(&state.rng),"nextActorId":state.next_actor_id,"nextEventId":state.next_event_id});
            native_bits::compare(
                &native_bits::source_value(&actual),
                &native_bits::source_value(&frame["expected"]),
                &format!("{} tick{tick}", case["name"]),
            );
        }
    }
}
