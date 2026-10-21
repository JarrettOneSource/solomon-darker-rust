//! Current EtherDrain pressure, membership and private manager against raw Source f64 bits.
#![cfg(feature = "persistence")]

#[path = "support/native_bits.rs"]
mod native_bits;
use native_bits::{compare, input};

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_sim::secondary::ether_drain::*;
use solomon_sim::{NativeRng, Vec2};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/current-ether-drain-goldens.json")).unwrap()
}

#[derive(Deserialize)]
struct Field {
    alpha: f64,
    damage: f64,
    id: u64,
    owner_id: String,
    position: Vec2,
}
impl Field {
    fn borrowed(&self) -> NativeEtherDrainField<'_> {
        NativeEtherDrainField {
            alpha: self.alpha,
            damage: self.damage,
            id: self.id,
            owner_id: &self.owner_id,
            position: self.position,
        }
    }
}

#[test]
fn current_pressure_thresholds_delays_family_masks_and_grounded_animation_forces_are_exact() {
    let fixture = fixture();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for (index, row) in fixture["contacts"].as_array().unwrap().iter().enumerate() {
        let field: Field = input(&row["field"]);
        let target = input(&row["target"]);
        compare(
            &serde_json::to_value(native_ether_drain_contact(field.borrowed(), &target)).unwrap(),
            &row["result"],
            &format!("contact {index}"),
        );
    }
    for (index, row) in fixture["animationContacts"].as_array().unwrap().iter().enumerate() {
        let field: Field = input(&row["field"]);
        compare(
            &serde_json::to_value(native_ether_drain_world_animation_contact(
                field.borrowed(),
                input(&row["target"]),
            ))
            .unwrap(),
            &row["result"],
            &format!("animation contact {index}"),
        );
    }
    for row in fixture["families"].as_array().unwrap() {
        let family = row["family"].as_str().unwrap();
        compare(&json!(native_ether_drain_enemy_force_factor(family)), &row["force"], family);
        assert_eq!(
            native_ether_drain_captures_family(family),
            row["captures"].as_bool().unwrap(),
            "{family}"
        );
    }
}

#[test]
fn loose_drop_notifications_and_reused_world_ids_preserve_source_membership_order() {
    let fixture = fixture();
    for (index, row) in fixture["references"].as_array().unwrap().iter().enumerate() {
        let source: Option<NativeEtherDrainState> = input(&row["source"]);
        let (refreshed, state) = refresh_native_ether_drain_targets(
            source.as_ref(),
            &input::<Vec<_>>(&row["targets"]),
            input(&row["center"]),
            row["refresh"].as_bool().unwrap(),
        );
        compare(
            &json!({"refreshed":refreshed,"state":state}),
            &row["result"],
            &format!("targets {index}"),
        );
    }
    for (index, row) in fixture["worldReferences"].as_array().unwrap().iter().enumerate() {
        let mut state: NativeEtherDrainState = input(&row["source"]);
        refresh_native_ether_drain_world_animations(
            &mut state,
            &input::<Vec<_>>(&row["targets"]),
            input(&row["center"]),
            row["initialized"].as_bool().unwrap(),
        );
        compare(
            &serde_json::to_value(state).unwrap(),
            &row["result"],
            &format!("world targets {index}"),
        );
    }
}

#[test]
fn private_debris_capture_expiry_and_leaf_centroids_preserve_every_source_draw_and_float() {
    let fixture = fixture();
    for row in fixture["leaves"].as_array().unwrap() {
        compare(
            &serde_json::to_value(native_ether_drain_leaf_center(&input::<Vec<_>>(
                &row["polygon"],
            )))
            .unwrap(),
            &row["result"],
            "leaf centroid",
        );
    }
    let mut initial = Vec::new();
    for (index, row) in fixture["debris"].as_array().unwrap().iter().enumerate() {
        let animation = create_native_ether_drain_debris(
            input(&row["center"]),
            input(&row["position"]),
            97.25,
            13.125,
            2,
        );
        compare(
            &serde_json::to_value(&animation).unwrap(),
            &row["result"],
            &format!("debris {index}"),
        );
        initial.push(animation);
    }
    initial.push(NativeEtherDrainAnimation::Captured {
        alpha: solomon_sim::float::fround(1.25 - (0.2_f32 as f64)),
        atlas: NativeEtherDrainAtlas::Demon,
        entry: 80,
        tint: 0xffffff,
        body_y_offset: 0,
    });
    for case in fixture["animationSteps"].as_array().unwrap() {
        let seed = case["seed"].as_i64().unwrap();
        let mut rng = NativeRng::new(seed);
        let mut animations = initial.clone();
        for frame in case["frames"].as_array().unwrap() {
            let stepped = step_native_ether_drain_animations(&animations, Vec2::ZERO, &mut rng);
            compare(
                &json!({"animations":stepped.animations,"pulses":stepped.pulses,
                "rng":{"index_a":rng.index_a,"index_b":rng.index_b,"words":&rng.words[..]}}),
                &frame["result"],
                &format!("seed{seed} tick{}", frame["tick"]),
            );
            animations = stepped.animations;
        }
    }
}
