//! Independent retail contracts recovered from SolomonDark.exe 03a83456 (ledger123,
//! Report131): row9/10, constructors5FA270/5FA670, release5FA6D0,
//! acquisition5F28A0 and movement/contact-before-guidance621450.
use solomon_sim::float::fround;
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::profile::WeldBuildId;
use solomon_sim::spells::targeting::{
    PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
};
use solomon_sim::spells::weld::*;
use solomon_sim::{Rng, Vec2};

fn target(id: &str, x: f64, y: f64, order: usize) -> PrimarySpellTarget {
    PrimarySpellTarget {
        active: true,
        actor_flags: 2,
        attachment: Vec2::ZERO,
        body_radius: 10.0,
        cell_binding_order: order,
        heading_degrees: None,
        grid_position: None,
        id: id.into(),
        kind: PrimarySpellTargetKind::Enemy,
        priority: 0,
        pending_remove: false,
        position: Vec2::new(x, y),
        query_lane: PrimarySpellQueryLane::Grid,
        registration_order: order,
    }
}
fn birth(quantity: u32, smart: f64) -> WeldPersistentActorState {
    create_weld_persistent_actor(
        WeldActorBirth {
            build_id: WeldBuildId::EtherealBoulder,
            direction: Vec2::new(1.0, 0.0),
            id: 100,
            origin: Vec2::ZERO,
            owner_id: "hero",
            tick: 0,
            vector: &[12.0, 3.0, f64::from(quantity), smart, 2.5, 1.0],
            world_key: "boneyard:1",
        },
        None,
    )
}
fn release(quantity: u32, smart: f64, targets: &[PrimarySpellTarget]) -> WeldPersistentRelease {
    let mut rng = Rng::new(17);
    let mut painters = WorldPainterOrder { next_actor_ordinal: 101, next_transient_ordinal: 0 };
    let result = release_weld_persistent_actor(
        birth(quantity, smart),
        101,
        1,
        &mut rng,
        Some(&mut painters),
        targets,
    );
    assert_eq!(rng, Rng::new(17), "release/acquisition consumes no RNG");
    assert_eq!(painters.next_actor_ordinal, 100 + result.actors.len());
    result
}
fn boulder(actor: &WeldPersistentActorState) -> &WeldEtherealBoulderState {
    let WeldPersistentActorState::EtherealBoulder(actor) = actor else { panic!("EBoulder") };
    actor
}
fn boulder_mut(actor: &mut WeldPersistentActorState) -> &mut WeldEtherealBoulderState {
    let WeldPersistentActorState::EtherealBoulder(actor) = actor else { panic!("EBoulder") };
    actor
}
fn advance(actor: &mut WeldPersistentActorState, targets: &[PrimarySpellTarget]) {
    assert!(step_weld_persistent_actor(actor, &|_, _, _| true).0);
    finish_weld_ethereal_boulder_guidance(boulder_mut(actor), targets);
}
#[test]
fn all_authored_quantity_and_smart_rows_preserve_native_split_identity_pool_and_motion() {
    for quantity in 1..=14 {
        for addition in [0, 10, 25, 40, 55, 100, 110, 120, 130, 140, 150] {
            let smart = fround(1.0 + f64::from(addition) / 100.0);
            let result = release(quantity, smart, &[target("near", 100.0, -100.0, 0)]);
            assert_eq!(result.actors.len(), quantity.min(4) as usize);
            assert_eq!(result.next_id, 100 + result.actors.len() as u64);
            let mut lights = std::collections::BTreeSet::new();
            for (index, actor) in result.actors.iter().enumerate() {
                let actor = boulder(actor);
                assert_eq!(actor.common.owner.id, 100 + index as u64);
                lights.insert(actor.common.owner.light_registration.unwrap().registration_ordinal);
                let factor = if index == 0 || quantity == 2 {
                    1.0
                } else if index == 3 {
                    0.9_f32 as f64
                } else {
                    0.95_f32 as f64
                };
                assert_eq!(actor.flight_speed, fround(2.25 * factor));
                assert_eq!(actor.turn_input, if index == 0 { smart } else { factor });
                assert_eq!(
                    actor.tracking_ticks_remaining,
                    if addition == 0 { 250 } else { (smart * 1000.0 + 250.0).trunc() as u64 }
                );
                assert_eq!(actor.reacquires_target, addition > 0);
                assert_eq!(actor.target_id.as_deref(), Some("near"));
                assert_eq!(actor.turn_accumulator, 0.01_f32 as f64);
                assert!(actor.hit_target_ids.is_empty());
                assert!((actor.remaining_damage - 0.3888).abs() < 1e-6);
            }
            assert_eq!(lights.len(), result.actors.len(), "independent light roots");
        }
    }
    let result = release(2, 1.0, &[target("north", 0.0, -31.0, 0), target("south", 0.0, 31.0, 1)]);
    assert_eq!(boulder(&result.actors[0]).target_id.as_deref(), Some("north"));
    assert_eq!(boulder(&result.actors[1]).target_id.as_deref(), Some("south"));
    assert_eq!(boulder(&result.actors[1]).heading_degrees, 100.0);
}
#[test]
fn native_guidance_moves_before_turning_and_reacquires_only_a_missing_nonnull_handle() {
    let targets = [target("near", 100.0, -100.0, 0)];
    let mut actor = release(1, 1.5, &targets).actors.remove(0);
    advance(&mut actor, &targets);
    let rock = boulder_mut(&mut actor);
    assert_eq!(rock.common.origin.x, 2.25);
    assert!(rock.common.origin.y.abs() < 1e-12);
    assert_eq!(rock.heading_degrees, fround(90.0 - 1.5 * (0.01_f32 as f64) * (0.7_f32 as f64)));
    assert_eq!(rock.turn_accumulator, fround((0.01_f32 as f64) + (0.05_f32 as f64)));
    assert_eq!(rock.tracking_ticks_remaining, 1749);
    rock.target_id = Some("removed".into());
    let heading = rock.heading_degrees;
    finish_weld_ethereal_boulder_guidance(rock, &targets);
    assert_eq!(rock.target_id.as_deref(), Some("near"));
    assert_eq!(rock.heading_degrees, heading, "no same-tick steering on replacement");
    let mut inactive = targets[0].clone();
    inactive.active = false;
    finish_weld_ethereal_boulder_guidance(rock, &[inactive]);
    assert_eq!(rock.target_id, None);
    assert!(rock.heading_degrees < heading, "inactive resolved target supplies final sample");
    let heading = rock.heading_degrees;
    finish_weld_ethereal_boulder_guidance(rock, &targets);
    assert_eq!(rock.heading_degrees, heading, "null target never polls");
    rock.target_id = Some("removed".into());
    rock.hit_target_ids.push("near".into());
    finish_weld_ethereal_boulder_guidance(
        rock,
        &[targets[0].clone(), target("farther", 200.0, -200.0, 1)],
    );
    assert_eq!(rock.target_id, None);
    assert!(!rock.reacquires_target, "visited closest rejects without fallback");
    let empty = release(1, 1.5, &[]);
    assert!(!boulder(&empty.actors[0]).reacquires_target);
}
#[test]
fn native_steering_window_is_not_actor_lifetime_and_terrain_still_retires_the_moved_rock() {
    let mut actor = release(1, 1.0, &[]).actors.remove(0);
    boulder_mut(&mut actor).tracking_ticks_remaining = 1;
    for _ in 0..1300 {
        advance(&mut actor, &[target("late", 100.0, -100.0, 0)]);
    }
    assert_eq!(boulder(&actor).tracking_ticks_remaining, 0);
    assert_eq!(boulder(&actor).heading_degrees, 90.0);
    assert_eq!(boulder(&actor).common.origin.x, 2925.0);
    let (alive, terminal) = step_weld_persistent_actor(&mut actor, &|_, _, _| false);
    assert!(!alive);
    assert_eq!(terminal.unwrap().common.origin.x, 2927.25);
}
#[test]
fn weak_release_uses_native_single_rock_policy_and_turn_accumulator_caps_at_ten() {
    let mut actor = birth(4, 2.5);
    let mut rng = Rng::new(7);
    update_weld_persistent_actor(
        &mut actor,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        WeldPersistentUpdateOptions { underpowered: true, ..Default::default() },
        &mut rng,
    );
    let weak = boulder(&actor);
    assert_eq!(weak.quantity, 1);
    assert_eq!(weak.turn_input, 1.0);
    assert_eq!(weak.tracking_ticks_remaining, 150);
    assert!(!weak.reacquires_target);
    let mut released = release_weld_persistent_actor(
        actor,
        101,
        2,
        &mut rng,
        None,
        &[target("near", 100.0, -100.0, 0)],
    )
    .actors
    .remove(0);
    let rock = boulder_mut(&mut released);
    rock.flight_ticks = 1;
    rock.turn_accumulator = 9.999;
    finish_weld_ethereal_boulder_guidance(rock, &[target("near", 100.0, -100.0, 0)]);
    assert_eq!(rock.turn_accumulator, 10.0);
}
