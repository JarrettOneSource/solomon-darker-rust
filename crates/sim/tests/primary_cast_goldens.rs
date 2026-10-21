use serde::Deserialize;
use solomon_sim::spells::casting::{
    NativePlayerWeaponKind, primary_cast_pose, primary_spell_aim_direction,
    primary_spell_emitter_offset,
};
use solomon_sim::{Vec2, profile::WizardElement};

type EmitterSample = (usize, Option<String>, String, f64, [f64; 2]);
type AimSample = ([f64; 2], [f64; 2], f64, [f64; 2]);

#[derive(Deserialize)]
struct Goldens {
    poses: Vec<(String, f64, bool, usize)>,
    emitters: Vec<EmitterSample>,
    aims: Vec<AimSample>,
    facings: [usize; 2],
}

fn element(id: &str) -> WizardElement {
    WizardElement::ALL.into_iter().find(|element| element.as_str() == id).unwrap()
}

#[test]
fn primary_poses_staff_wand_and_bare_emitters_match_the_website() {
    let fixture: Goldens =
        serde_json::from_str(include_str!("fixtures/website/primary-cast-goldens.json")).unwrap();
    for (kind, tick, channel, expected) in fixture.poses {
        assert_eq!(
            primary_cast_pose((tick >= 0.0).then_some(tick), channel, element(&kind)),
            expected
        );
    }
    for (heading, weapon, kind, tick, offset) in fixture.emitters {
        let weapon = weapon.map(|kind| match kind.as_str() {
            "staff" => NativePlayerWeaponKind::Staff,
            "wand" => NativePlayerWeaponKind::Wand,
            other => panic!("unknown weapon {other}"),
        });
        assert_eq!(
            primary_spell_emitter_offset(heading, Some(tick), false, element(&kind), weapon),
            Vec2::new(offset[0], offset[1])
        );
    }
    for (position, aim, scale, expected) in fixture.aims {
        assert_eq!(
            primary_spell_aim_direction(
                Vec2::new(position[0], position[1]),
                Vec2::new(aim[0], aim[1]),
                scale
            ),
            Vec2::new(expected[0], expected[1])
        );
    }
    use solomon_sim::hub::regions::plan_hub_scripted_movement;
    use solomon_sim::hub::{HubWizard, plan_hub_wizard_tick};
    let mut wizard = HubWizard::at(Vec2::new(100.0, 100.0));
    wizard.set_facing(0.0);
    wizard.primary_cast.action_tick = Some(0.0);
    let mut scripted = wizard.clone();
    let plan = plan_hub_wizard_tick(wizard.velocity, Vec2::new(1.0, 0.0), 1.0);
    wizard.commit(
        &plan,
        Vec2::new(wizard.position.x + plan.delta.x, wizard.position.y + plan.delta.y),
    );
    let plan = plan_hub_scripted_movement(scripted.position, Vec2::new(200.0, 100.0), 1.0);
    scripted.commit(
        &plan,
        Vec2::new(scripted.position.x + plan.delta.x, scripted.position.y + plan.delta.y),
    );
    assert_eq!([wizard.heading_index, scripted.heading_index], fixture.facings);
}

#[test]
fn shared_spell_draws_keep_equal_damage_ranges_and_high_tail_percentiles() {
    use solomon_sim::NativeRng;
    use solomon_sim::spells::air_water::{
        draw_native_disintegrate_percentile, draw_native_spell_damage,
    };

    // Sequential Website air-water-spell-actors.ts calls with native seed 7.
    let mut rng = NativeRng::new(7);
    assert_eq!(draw_native_spell_damage(&mut rng, 3.0, 3.0), 3.0);
    assert_eq!(draw_native_spell_damage(&mut rng, 1.2, 2.7), 1.953_180_074_691_772_5);
    assert_eq!(draw_native_spell_damage(&mut rng, 0.0, 0.0), 0.0);
    assert!(!draw_native_disintegrate_percentile(&mut rng, 0.0));
    assert!(!draw_native_disintegrate_percentile(&mut rng, 25.0));
    assert!(draw_native_disintegrate_percentile(&mut rng, 100.0));
    assert_eq!((rng.index_a, rng.index_b), (6, 37));
    assert_eq!(
        &rng.words[..6],
        &[7_170_556, 11_602_193, 18_772_749, 30_374_942, 49_147_691, 79_522_633]
    );
}
