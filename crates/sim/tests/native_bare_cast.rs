//! Exact retail no-item actions, not regenerated historical Website goldens.
#![cfg(feature = "persistence")]

use solomon_sim::float::fround;
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::primary::{NativePrimarySkillProfile, native_primary_skill_profile};
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};
use solomon_sim::secondary::{NativeSecondaryCastAction, step_native_secondary_cast_action};
use solomon_sim::spells::casting::{NativePlayerWeaponKind, PlayerPrimaryCastState};
use solomon_sim::spells::hand_cast::NativeHandCast1;
use solomon_sim::spells::model::SpellState;
use solomon_sim::spells::{PrimarySpellCastAuthority, PrimarySpellInput, PrimarySpellWorldContext};
use solomon_sim::{HubWizard, NativeRng, Vec2};

fn profile(element: WizardElement, weld: Option<u32>) -> NativePrimarySkillProfile {
    let mut book = NativeSkillBookState::new(element, WizardDiscipline::Arcane);
    if let Some(build) = weld {
        book.grant_weld_build(build);
    }
    native_primary_skill_profile(&book, &NativeOffensiveSpellFactors::default())
}

#[allow(clippy::too_many_arguments)]
fn step(
    spells: &mut SpellState,
    wizard: &mut HubWizard,
    rng: &mut NativeRng,
    profile: &NativePrimarySkillProfile,
    weapon: Option<NativePlayerWeaponKind>,
    tick: u64,
    eligible: bool,
    factor: f64,
    time: f64,
    held: bool,
) {
    spells.step_primary(
        wizard,
        PrimarySpellCastAuthority {
            weapon_kind: weapon,
            alive: true,
            available_mana: 10000.0,
            cast_progress_factor: factor,
            hand_cast_factor: factor,
            action_time_factor: time,
            eligible,
            plane_active: false,
            primary_skill: profile,
        },
        PrimarySpellInput { aim: Some(Vec2::new(700.0, 200.0)), primary: held },
        &PrimarySpellWorldContext {
            tick,
            enhanced_effects: true,
            flame_lash_view_bounds: None,
            player_light_registration: None,
            view_scale: 1.2,
            owner_id: "wizard",
            world_key: "bare-regression",
            targets: &[],
            can_place: &|_, _, _| true,
            can_traverse: &|_, _, _, _, _| true,
            obstruction: &|_, _, _, _| None,
            range_endpoint: &|start, direction, _| {
                Vec2::new(start.x + direction.x * 600.0, start.y + direction.y * 600.0)
            },
            navigation: None,
        },
        rng,
        Some(&mut WorldPainterOrder::default()),
    );
}

#[test]
fn only_accepted_no_item_one_shot_consumes_the_one_born_gameplay_word() {
    for weapon in [None, Some(NativePlayerWeaponKind::Staff), Some(NativePlayerWeaponKind::Wand)] {
        for eligible in [false, true] {
            let mut wizard = HubWizard::at(Vec2::ZERO);
            let mut spells = SpellState::default();
            let mut rng = NativeRng::new(125);
            let mut expected = rng.clone();
            if weapon.is_none() && eligible {
                expected.draw_integer(4);
            }
            let profile = profile(WizardElement::Ether, None);
            step(&mut spells, &mut wizard, &mut rng, &profile, weapon, 0, eligible, 1.0, 1.0, true);
            assert_eq!(rng, expected, "{weapon:?}, eligible={eligible}");
            assert_eq!(wizard.primary_cast.hand_cast.is_some(), weapon.is_none() && eligible);
            step(&mut spells, &mut wizard, &mut rng, &profile, weapon, 1, eligible, 1.0, 0.0, true);
            assert_eq!(rng, expected, "held action draws no additional opening word");
        }
    }
    for element in [WizardElement::Air, WizardElement::Water, WizardElement::Earth] {
        let profile = profile(element, None);
        let run = |weapon| {
            let mut wizard = HubWizard::at(Vec2::ZERO);
            let mut spells = SpellState::default();
            let mut rng = NativeRng::new(125);
            step(&mut spells, &mut wizard, &mut rng, &profile, weapon, 0, true, 1.0, 1.0, true);
            assert!(wizard.primary_cast.hand_cast.is_none());
            step(&mut spells, &mut wizard, &mut rng, &profile, weapon, 1, true, 0.0, 0.0, true);
            if weapon.is_none() {
                assert_eq!(wizard.primary_cast.action_tick, Some(1.0));
                let poses = solomon_sim::player_presentation::player_character_attachment_poses(
                    &wizard.primary_cast,
                    element,
                    None,
                    None,
                );
                assert_eq!(poses.bare, Some(2), "zero-time Constant writes K2");
            }
            rng
        };
        assert_eq!(
            run(None),
            run(Some(NativePlayerWeaponKind::Staff)),
            "{element:?} Constant has no HandCast1 opening draw"
        );
    }
}

#[test]
fn born_bare_rates_marker_and_terminal_tick_follow_the_retail_program() {
    for (element, weld, multiplier) in [
        (WizardElement::Ether, None, 1.0),
        (WizardElement::Fire, None, 0.75),
        (WizardElement::Ether, Some(1000), 0.87_f32 as f64),
        (WizardElement::Ether, Some(1001), 1.0),
        (WizardElement::Ether, Some(1002), 1.3_f32 as f64),
        // grant_weld_build retains native rank-zero Earth Speed Up=50.
        (WizardElement::Ether, Some(1009), 1.5),
    ] {
        let profile = profile(element, weld);
        let rate = fround(fround(0.095_f32 as f64 * 1.5) * multiplier);
        let mut wizard = HubWizard::at(Vec2::ZERO);
        let mut spells = SpellState::default();
        let mut rng = NativeRng::new(73);
        step(&mut spells, &mut wizard, &mut rng, &profile, None, 0, true, 1.5, 1.0, true);
        let mut progress = 0.0;
        let mut emitted = false;
        let mut tick = 1;
        while progress < 4.0 {
            let time = if tick < 4 {
                0.0
            } else if tick < 10 {
                0.5
            } else {
                1.0
            };
            progress = fround(progress + time * rate);
            // Changed Faster Caster cannot rescale an already-born action.
            step(&mut spells, &mut wizard, &mut rng, &profile, None, tick, true, 2.0, time, false);
            let hand = wizard.primary_cast.hand_cast.as_ref().expect("terminal action retained");
            assert_eq!(hand.progress(), progress, "{element:?}/{weld:?} tick{tick}");
            assert_eq!(hand.done(), progress >= 4.0);
            emitted |= progress >= 1.0;
            assert_eq!(wizard.primary_cast.emission_sequence, u64::from(emitted));
            if progress >= 1.0 {
                assert_eq!(hand.pose(), if progress < 2.0 { 1 } else { 2 });
            }
            tick += 1;
        }
        step(&mut spells, &mut wizard, &mut rng, &profile, None, tick, true, 2.0, 1.0, false);
        assert!(wizard.primary_cast.hand_cast.is_none());
        assert!(wizard.primary_cast.action_tick.is_none());
    }
}

#[test]
fn zero_time_updates_pose_but_never_rerolls_opening_and_cast2_retains_its_terminal() {
    for seed in 0..16 {
        let mut rng = NativeRng::new(seed);
        let mut oracle = rng.clone();
        let expected_pose = if oracle.draw_integer(4) == 1 { 3 } else { 0 };
        let mut hand = NativeHandCast1::new(0.095_f32 as f64, 2, &mut rng);
        assert_eq!(hand.pose(), 2, "constructor retains prior K");
        hand.step(0.0);
        assert_eq!(hand.pose(), expected_pose);
        assert_eq!(rng, oracle);
    }
    let mut action = Some(NativeSecondaryCastAction::new(None));
    assert_eq!(action.unwrap().hand.unwrap().pose, 0);
    action = step_native_secondary_cast_action(action, 300.0, 0.0).action;
    assert_eq!(action.unwrap().hand.unwrap().pose, 1);
    assert_eq!(action.unwrap().progress, 0.0);
    let mut progress = 0.0;
    while progress < 6.0 {
        progress = fround(progress + 0.095_f32 as f64 * 0.5);
        action = step_native_secondary_cast_action(action, 300.0, 0.5).action;
        assert_eq!(action.unwrap().progress, progress);
        assert_eq!(action.unwrap().hand.unwrap().done, progress >= 6.0);
    }
    assert_eq!(action.unwrap().hand.unwrap().pose, 3);
    assert!(step_native_secondary_cast_action(action, 0.0, 1.0).action.is_none());
}

#[test]
fn old_actions_default_without_rng_and_new_action_state_round_trips_exactly() {
    let mut cast = PlayerPrimaryCastState::default();
    let mut rng = NativeRng::new(125);
    cast.hand_cast = Some(NativeHandCast1::new(0.095_f32 as f64, 0, &mut rng));
    cast.action_tick = Some(0.0);
    cast.hand_cast.as_mut().unwrap().step(0.5);
    let saved = serde_json::to_value(&cast).unwrap();
    let restored: PlayerPrimaryCastState = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(restored, cast);
    let mut old = saved.clone();
    old.as_object_mut().unwrap().remove("hand_cast");
    let legacy: PlayerPrimaryCastState = serde_json::from_value(old).unwrap();
    assert!(legacy.hand_cast.is_none());
    for (field, value) in [
        ("pose", serde_json::json!(4)),
        ("rate", serde_json::json!(-1.0)),
        ("done", serde_json::json!(true)),
    ] {
        let mut invalid = saved.clone();
        invalid["hand_cast"][field] = value;
        assert!(serde_json::from_value::<PlayerPrimaryCastState>(invalid).is_err());
    }
    let secondary = NativeSecondaryCastAction::new(None);
    let mut invalid = serde_json::to_value(secondary).unwrap();
    invalid["hand"]["pose"] = serde_json::json!(4);
    assert!(serde_json::from_value::<NativeSecondaryCastAction>(invalid).is_err());
}

#[test]
fn native_hand_class_query_rounds_each_store_without_changing_equipped_cadence() {
    let mut player = solomon_sim::player::PlayerState::new(
        solomon_sim::profile::NewGameSelection {
            name: "Native hand rate".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        &mut NativeRng::new(125),
    );
    let modifiers = &mut player.skill_runtime.equipment_modifiers;
    modifiers.cast_speed_multiplier = 1.13_f32 as f64;
    modifiers.cast_speed_flat = 0.17_f32 as f64;
    modifiers.class_cast_speed_multiplier[0] = 1.27_f32 as f64;
    modifiers.class_cast_speed_flat[0] = 0.11_f32 as f64;
    let legacy = ((modifiers.cast_speed_multiplier + modifiers.cast_speed_flat)
        * modifiers.class_cast_speed_multiplier[0]
        + modifiers.class_cast_speed_flat[0])
        .max(0.0);
    let expected = ((1.13_f32 * 1.0 + 0.17_f32) * 1.27_f32 + 0.11_f32).max(0.0);
    let derived = player.derived_stats();
    assert_eq!(derived.hand_cast_factor, expected as f64);
    assert_eq!(derived.cast_progress_factor, legacy);
}

#[test]
fn cast2_staff_marker_zero_and_wand_bare_marker_one_keep_native_endpoints() {
    for weapon in [Some(NativePlayerWeaponKind::Staff), Some(NativePlayerWeaponKind::Wand), None] {
        let mut action = Some(NativeSecondaryCastAction::new(weapon));
        let mut pulses = Vec::new();
        for tick in 1..=15 {
            let step = step_native_secondary_cast_action(action, 0.0, 1.0);
            if step.marker_crossed {
                pulses.push(tick);
            }
            action = step.action;
        }
        assert_eq!(
            pulses,
            if weapon == Some(NativePlayerWeaponKind::Staff) { vec![1] } else { vec![11] }
        );
        let mut exact = NativeSecondaryCastAction::new(weapon);
        exact.progress = if weapon == Some(NativePlayerWeaponKind::Staff) { 0.0 } else { 1.0 };
        if let Some(hand) = &mut exact.hand {
            hand.pose = 3;
        }
        let stalled = step_native_secondary_cast_action(Some(exact), 0.0, 0.0);
        if weapon.is_none() {
            assert!(!stalled.marker_crossed);
        }
        let departed = step_native_secondary_cast_action(Some(exact), 0.0, 1.0);
        assert!(departed.marker_crossed, "native inclusive endpoint {weapon:?}");
    }
    let profile = profile(WizardElement::Ether, None);
    let mut wizard = HubWizard::at(Vec2::ZERO);
    let mut spells = SpellState::default();
    let mut rng = NativeRng::new(125);
    // Exact f32 .5 birth rate lands at1 and the native helper also admits departure.
    let factor = 0.5 / (0.095_f32 as f64);
    for tick in 0..=3 {
        step(
            &mut spells,
            &mut wizard,
            &mut rng,
            &profile,
            None,
            tick,
            true,
            factor,
            1.0,
            tick == 0,
        );
    }
    assert_eq!(wizard.primary_cast.emission_sequence, 2);
}
