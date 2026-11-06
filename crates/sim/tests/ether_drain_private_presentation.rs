//! Independent retail EtherDrain private-manager cases, ledger 083, Report 129.
//! Expected art/geometry comes from Anim_Sucked 0x00455370 and parent draw 0x005EE120.

use solomon_sim::secondary::ether_drain::{
    EtherDrainAnimation, EtherDrainAtlas, EtherDrainState, step_ether_drain_animations,
};
use solomon_sim::secondary::presentation::{SecondaryBlend, secondary_presentation_plan};
use solomon_sim::secondary::{SecondaryActorKind, SecondaryActorState};
use solomon_sim::{Rng, Vec2};

fn field(animations: Vec<EtherDrainAnimation>) -> SecondaryActorState {
    let mut actor = SecondaryActorState::new(
        1,
        SecondaryActorKind::EtherDrain,
        "caster".into(),
        "boneyard:private".into(),
        Some(74),
    );
    actor.position = Vec2::new(10.0, 20.0);
    actor.alpha = 0.25;
    actor.scale = 0.4;
    actor.ether_drain = Some(EtherDrainState { animations, ..Default::default() });
    actor
}

#[test]
fn every_retail_capture_bank_draws_its_retained_image_between_the_native_rims() {
    let mut cases = Vec::new();
    for base in [1477, 1495, 1513, 1531, 1549, 1567] {
        cases.extend((base..base + 18).map(|entry| (EtherDrainAtlas::BadGuys, entry, 23)));
    }
    for base in [2293, 2311, 2329] {
        cases.extend((base..base + 18).map(|entry| (EtherDrainAtlas::BadGuys, entry, 0)));
    }
    cases.extend((80..98).map(|entry| (EtherDrainAtlas::Demon, entry, 0)));
    assert_eq!(cases.len(), 180);
    for (atlas, entry, body_y_offset) in cases {
        for enhanced in [false, true] {
            let mut actor = field(vec![EtherDrainAnimation::Captured {
                alpha: 0.8,
                atlas,
                entry,
                tint: 0x123456,
                body_y_offset,
            }]);
            actor.enhanced = enhanced;
            let plan = secondary_presentation_plan(&actor, 42.0, 1.0);
            let private: Vec<_> = plan
                .draws
                .iter()
                .filter(|draw| {
                    matches!(
                        draw.role.as_str(),
                        "ether-drain-capture-back-rim"
                            | "ether-drain-captured-image"
                            | "ether-drain-capture-front-rim"
                    )
                })
                .collect();
            assert_eq!(private.len(), 3, "{atlas:?} {entry} enhanced={enhanced}");
            assert_eq!(private[0].entry, 9);
            assert_eq!(private[0].offset, Vec2::ZERO);
            assert_eq!(private[0].clip, None);
            assert_eq!(private[2].entry, 8);
            assert_eq!(private[2].offset, Vec2::new(0.0, -1.0));
            assert_eq!(private[2].clip, None);
            let image = private[1];
            assert_eq!(image.role, "ether-drain-captured-image");
            assert_eq!(image.entry, entry);
            assert_eq!(
                image.atlas.as_str(),
                if atlas == EtherDrainAtlas::Demon { "Demon" } else { "BadGuys" }
            );
            assert_eq!(image.tint, 0x123456);
            assert_eq!(image.clip, Some([-100.0, -110.0, 200.0, 110.0]));
            let expected_y = if body_y_offset == 23 { 20.0 } else { -3.0 };
            assert!((image.offset.y - expected_y).abs() < 1e-12);
            for draw in private {
                assert_eq!(draw.alpha, 0.25, "field opacity, not capture timer opacity");
                assert_eq!(draw.scale_x, 1.0);
                assert_eq!(draw.scale_y, 1.0);
                assert_eq!(draw.blend, SecondaryBlend::Normal);
            }
        }
    }
}

#[test]
fn all_private_debris_variants_use_live_world_positions_and_independent_rotation() {
    for variant in 0..3 {
        let actor = field(vec![EtherDrainAnimation::Debris {
            position: Vec2::new(30.0, 40.0),
            direction: Vec2::new(1.0, 0.0),
            remaining_distance: 30.0,
            speed: 1.0,
            oscillation_degrees: 0.0,
            rotation_degrees: 90.0,
            variant,
        }]);
        let plan = secondary_presentation_plan(&actor, 42.0, 1.0);
        let draws: Vec<_> = plan
            .draws
            .iter()
            .filter(|draw| draw.role.starts_with("ether-drain-private-debris-"))
            .collect();
        assert_eq!(draws.len(), 1, "private debris {variant}");
        let draw = draws[0];
        assert_eq!(draw.atlas.as_str(), "DeadHawg");
        assert_eq!(draw.entry, 177 + variant as usize);
        assert_eq!(draw.offset, Vec2::new(20.0, 20.0));
        assert_eq!(draw.rotation_radians, std::f64::consts::FRAC_PI_2);
        assert_eq!(draw.alpha, 0.25);
        assert_eq!(draw.scale_x, 1.0);
        assert_eq!(draw.scale_y, 1.0);
        assert_eq!(draw.clip, None);
    }
}

#[test]
fn capture_rims_begin_strictly_below_one_and_retire_with_the_private_image() {
    for (alpha, rims) in [(1.25, 0), (1.0, 0), (0.8, 2), (0.05, 2)] {
        let mut actor = field(vec![EtherDrainAnimation::Captured {
            alpha,
            atlas: EtherDrainAtlas::Demon,
            entry: 80,
            tint: 0xffffff,
            body_y_offset: 0,
        }]);
        let before = actor.clone();
        let plan = secondary_presentation_plan(&actor, 42.0, 1.0);
        assert_eq!(actor, before, "drawing must not advance private animation or gameplay RNG");
        assert_eq!(plan.draws.iter().filter(|draw| draw.role.ends_with("-rim")).count(), rims);
        assert_eq!(
            plan.draws.iter().filter(|draw| draw.role == "ether-drain-captured-image").count(),
            1
        );
        let image =
            plan.draws.iter().find(|draw| draw.role == "ether-drain-captured-image").unwrap();
        assert_eq!(image.clip.is_some(), alpha < 1.0);
        let mut rng = Rng::new(17);
        let mut retired = false;
        for _ in 0..7 {
            let step = step_ether_drain_animations(
                &actor.ether_drain.as_ref().unwrap().animations,
                actor.position,
                &mut rng,
            );
            actor.ether_drain.as_mut().unwrap().animations = step.animations;
            if !step.pulses.is_empty() {
                assert_eq!(step.pulses, [1.5]);
                retired = true;
                break;
            }
        }
        assert!(retired);
        let plan = secondary_presentation_plan(&actor, 49.0, 1.0);
        assert!(
            !plan.draws.iter().any(
                |draw| draw.role.ends_with("-rim") || draw.role == "ether-drain-captured-image"
            )
        );
        assert!(plan.draws.iter().all(|draw| draw.clip.is_none()));
    }
}
