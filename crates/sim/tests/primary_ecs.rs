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
