#![cfg(feature = "persistence")]

use solomon_sim::hub::HubWizard;
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::primary::native_primary_skill_profile;
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};
use solomon_sim::spells::model::{PrimarySpellProjectile, PrimarySpellProjectilePhase, SpellState};
use solomon_sim::spells::{
    PrimarySpellCastAuthority, PrimarySpellEpochContext, PrimarySpellInput,
    PrimarySpellTraversalActor, PrimarySpellWorldContext,
};
use solomon_sim::{NativeRng, Vec2};

fn cast(
    element: WizardElement,
    weld: bool,
    id: u64,
    rng: &mut NativeRng,
    order: &mut WorldPainterOrder,
) -> PrimarySpellProjectile {
    let mut book = NativeSkillBookState::new(element, WizardDiscipline::Arcane);
    if weld {
        book.grant_weld_build(1001);
    }
    let profile = native_primary_skill_profile(&book, &NativeOffensiveSpellFactors::default());
    let mut wizard = HubWizard::at(Vec2::new(100.0, 200.0));
    let mut spells = SpellState::default();
    for tick in 0..60 {
        spells.step_primary(
            &mut wizard,
            PrimarySpellCastAuthority {
                weapon_kind: None,
                alive: true,
                available_mana: 1000.0,
                cast_progress_factor: 1.0,
                eligible: true,
                plane_active: false,
                primary_skill: &profile,
            },
            PrimarySpellInput { aim: Some(Vec2::new(700.0, 200.0)), primary: true },
            &PrimarySpellWorldContext {
                tick,
                enhanced_effects: true,
                flame_lash_view_bounds: None,
                player_light_registration: None,
                view_scale: 1.2,
                owner_id: "wizard",
                world_key: "boneyard:ecs",
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
            Some(order),
        );
        if !spells.projectiles.is_empty() {
            let mut projectile = spells.projectiles.remove(0);
            projectile.common_mut().id = id;
            return projectile;
        }
    }
    panic!("learned primary creates a projectile within 60 ticks")
}

fn step(
    spells: &mut SpellState,
    tick: u64,
    retire: Option<u64>,
    rng: &mut NativeRng,
    order: &mut WorldPainterOrder,
) {
    spells.step_primary_players(
        &mut [],
        &PrimarySpellEpochContext {
            tick,
            enhanced_effects: true,
            targets_for_owner: &|_| &[],
            can_place: &|_, _, _| true,
            can_traverse: &|actor, _, _, _, _| match actor {
                PrimarySpellTraversalActor::Projectile(spell) => Some(spell.common().id) != retire,
                PrimarySpellTraversalActor::EtherealBoulder(_) => true,
            },
            obstruction: &|_, _, _, _, _| None,
            navigation: None,
        },
        rng,
        Some(order),
    );
}

#[test]
fn mixed_primary_flight_retirement_birth_and_restore_preserve_order() {
    let mut rng = NativeRng::new(719);
    let mut order = WorldPainterOrder::default();
    let records = [
        cast(WizardElement::Earth, false, 1, &mut rng, &mut order),
        cast(WizardElement::Ether, false, 2, &mut rng, &mut order),
        cast(WizardElement::Fire, false, 3, &mut rng, &mut order),
        cast(WizardElement::Ether, true, 4, &mut rng, &mut order),
    ];
    let mut live = SpellState {
        next_id: 10,
        projectiles: records.clone().into_iter().collect(),
        ..SpellState::default()
    };
    assert_eq!(
        serde_json::to_value(&live.projectiles).unwrap(),
        serde_json::to_value(&records).unwrap()
    );
    for tick in 1..=6 {
        step(&mut live, tick, Some(2), &mut rng, &mut order);
    }
    assert_eq!(
        live.projectiles.iter().map(|spell| spell.common().id).collect::<Vec<_>>(),
        [1, 3, 4]
    );
    let held = live.projectiles.get(0).unwrap().common();
    assert_eq!(held.phase, PrimarySpellProjectilePhase::Held);
    assert_eq!(held.flight_ticks, 0);
    assert_eq!(held.age_ticks, records[0].common().age_ticks + 6);
    let newborn = cast(WizardElement::Ether, false, 5, &mut rng, &mut order);
    live.projectiles.push(newborn);
    let saved = serde_json::to_vec(&live).unwrap();
    let mut restored: SpellState = serde_json::from_slice(&saved).unwrap();
    let mut restored_rng = rng.clone();
    let mut restored_order = order;
    assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
    for tick in 7..=100 {
        step(&mut live, tick, None, &mut rng, &mut order);
        step(&mut restored, tick, None, &mut restored_rng, &mut restored_order);
        assert_eq!(
            live.projectiles.iter().map(|spell| spell.common().id).collect::<Vec<_>>(),
            [1, 3, 4, 5]
        );
        assert_eq!(
            serde_json::to_vec(&restored).unwrap(),
            serde_json::to_vec(&live).unwrap(),
            "tick {tick}"
        );
        assert_eq!(restored_rng, rng);
        assert_eq!(restored_order, order);
    }
}

#[test]
fn mixed_transients_retire_in_place_and_meteor_children_follow_their_parent() {
    use solomon_sim::painter::WorldManagerLane;
    use solomon_sim::spells::fire::NATIVE_FIRE_IMPACT_LIFETIME_TICKS;
    use solomon_sim::spells::model::{
        PrimarySpellFireImpact, PrimarySpellTransient, PrimarySpellTransientBase,
        PrimaryTransientRef,
    };
    use solomon_sim::spells::weld::{
        NativeWeldActorBirth, NativeWeldMeteorSpawnProgram, NativeWeldWorldActor, WeldWorldRef,
        create_native_weld_meteor,
    };

    let mut rng = NativeRng::new(913);
    let mut order = WorldPainterOrder::default();
    let mut impact = |id, age_ticks| {
        let registration = order.register(WorldManagerLane::Transient);
        PrimarySpellTransient::FireImpact(PrimarySpellFireImpact {
            common: PrimarySpellTransientBase {
                id,
                age_ticks,
                owner_id: "wizard".into(),
                world_key: "boneyard:ecs".into(),
                light_registration: Some(registration),
                painter_registrations: vec![registration],
            },
            origin: Vec2::new(100.0, 200.0),
        })
    };
    let first = impact(1, 0);
    let retiring = impact(2, NATIVE_FIRE_IMPACT_LIFETIME_TICKS - 1);
    let last = impact(4, 0);
    let meteor = create_native_weld_meteor(
        NativeWeldActorBirth {
            id: 3,
            tick: 0,
            build_id: 1007,
            owner_id: "wizard",
            world_key: "boneyard:ecs",
            origin: Vec2::new(100.0, 200.0),
            direction: Vec2::new(1.0, 0.0),
            vector: &[0.0; 8],
        },
        NativeWeldMeteorSpawnProgram {
            body_scale: 1.0,
            fall_heading_degrees: 90.0,
            fall_height: 1.0,
            fall_step: 2.0,
            impact_ticks: 30,
            position: Vec2::new(100.0, 200.0),
            private_seed: 19,
        },
        15.0,
        false,
        Some(&mut order),
    );
    let records =
        [first, retiring, PrimarySpellTransient::Weld(NativeWeldWorldActor::Meteor(meteor)), last];
    let mut live = SpellState {
        next_id: 20,
        transients: records.clone().into_iter().collect(),
        ..SpellState::default()
    };
    assert_eq!(
        serde_json::to_value(&live.transients).unwrap(),
        serde_json::to_value(&records).unwrap()
    );
    step(&mut live, 1, None, &mut rng, &mut order);
    let effects: Vec<_> = live.transients.iter().collect();
    assert_eq!(effects[0].common().id, 1);
    assert_eq!(effects[0].common().age_ticks, 1);
    assert_eq!(effects[1].common().id, 3);
    assert!(matches!(effects[2], PrimaryTransientRef::Weld(WeldWorldRef::MeteorFlash(_))));
    assert_eq!(effects.last().unwrap().common().id, 4);
    assert_eq!(effects.last().unwrap().common().age_ticks, 1);
    assert_eq!(
        effects[2..effects.len() - 1].iter().map(|effect| effect.common().id).collect::<Vec<_>>(),
        (20..live.next_id).collect::<Vec<_>>()
    );
    assert!(effects[2..effects.len() - 1].iter().all(|effect| effect.common().age_ticks == 0));
    assert!(effects.iter().all(|effect| !effect.common().painter_registrations.is_empty()));

    let saved = serde_json::to_vec(&live).unwrap();
    let mut restored: SpellState = serde_json::from_slice(&saved).unwrap();
    let mut restored_rng = rng.clone();
    let mut restored_order = order;
    assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
    // Enhanced debris starts at alpha 10 and fades after its Meteor has retired.
    for tick in 2..=410 {
        step(&mut live, tick, None, &mut rng, &mut order);
        step(&mut restored, tick, None, &mut restored_rng, &mut restored_order);
        assert_eq!(serde_json::to_vec(&restored).unwrap(), serde_json::to_vec(&live).unwrap());
        assert_eq!(restored_rng, rng);
        assert_eq!(restored_order, order);
    }
    assert!(live.transients.is_empty());
}
