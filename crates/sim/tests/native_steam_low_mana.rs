//! Instruction-derived contract: retail 0x00543266 grants normal Steam contacts
//! independently of the low-mana byte.

use solomon_sim::spells::weld::*;
use solomon_sim::{Rng, Vec2};

fn particle(seed: i64, weak: bool) -> (WeldSteamActorState, Rng) {
    let mut rng = Rng::new(seed);
    let actor = spawn_weld_steam_actor(
        SpawnWeldSteamInput {
            birth: WeldActorBirth {
                build_id: 1005,
                direction: Vec2::new(1.0, 0.0),
                id: 1,
                origin: Vec2::ZERO,
                owner_id: "peer",
                tick: 2,
                vector: &[400.0, 20.0, 8.0, 3.0, 12.0, 15.0, 5.0, 2.0],
                world_key: "boneyard:steam",
            },
            damage: if weak { 2.0 } else { 4.0 },
            query_origin: Vec2::ZERO,
            underpowered: weak,
            obstruction_point: &|_, _| None,
        },
        &mut rng,
    )
    .unwrap();
    (actor, rng)
}

#[test]
fn weak_normal_particles_keep_the_ten_tick_contact_clock_and_learned_payload() {
    let (mut actor, _) = particle(0, true);
    assert_eq!(actor.variant, WeldSteamVariant::Normal);
    assert!(actor.contact_enabled, "0x00543266 has no low-mana test");
    assert_eq!(actor.alpha_multiplier, 0.25);
    assert_eq!(actor.contact_damage, 2.0);
    assert_eq!(actor.common.vector, vec![400.0, 20.0, 8.0, 3.0, 12.0, 15.0, 5.0, 2.0]);
    actor.contact_ticks_remaining = 1;
    actor.life = 1.0;
    actor.life_loss = 0.0;
    let mut due = Vec::new();
    for tick in 1..=11 {
        assert!(step_weld_steam_actor(&mut actor));
        if actor.contact_due {
            due.push(tick);
        }
    }
    assert_eq!(due, vec![1, 11]);
    actor.life = 0.5;
    actor.contact_ticks_remaining = 1;
    assert!(step_weld_steam_actor(&mut actor));
    assert!(!actor.contact_due, "contact requires life strictly above .5");
    actor.life = 0.0;
    actor.life_loss = 0.1;
    assert!(!step_weld_steam_actor(&mut actor));
}

#[test]
fn over_is_cosmetic_while_weak_forces_normal_without_changing_constructor_words() {
    let mut over_count = 0;
    for seed in 0..32 {
        let (mut funded, funded_rng) = particle(seed, false);
        let (weak, weak_rng) = particle(seed, true);
        assert_eq!(funded_rng, weak_rng, "seed{seed}: constructor draw cardinality");
        assert_eq!(weak.variant, WeldSteamVariant::Normal);
        assert!(weak.contact_enabled);
        if funded.variant == WeldSteamVariant::Over {
            over_count += 1;
            assert!(!funded.contact_enabled);
            funded.contact_ticks_remaining = 1;
            assert!(step_weld_steam_actor(&mut funded));
            assert!(!funded.contact_due);
        } else {
            assert!(funded.contact_enabled);
            assert_eq!(weak.position, funded.position);
            assert_eq!(weak.life, funded.life);
        }
    }
    assert!(over_count > 0, "the selector matrix must exercise Over");
}

#[cfg(feature = "persistence")]
#[test]
fn weak_contact_permission_and_payload_survive_persistence() {
    let (actor, _) = particle(0, true);
    let restored: WeldSteamActorState =
        serde_json::from_str(&serde_json::to_string(&actor).unwrap()).unwrap();
    assert!(restored.contact_enabled);
    assert_eq!(restored, actor);
}

#[test]
fn steam_cast_rate_scales_funded_damage_and_debit_but_not_weak_strength() {
    use solomon_sim::hub::HubWizard;
    use solomon_sim::profile::primary::{
        PrimarySkillProfile, PrimarySkillProfileBase, WeldPrimarySkillProfile,
    };
    use solomon_sim::profile::weld::{WeldCastKind, WeldPrimaryVector};
    use solomon_sim::spells::casting::PlayerWeaponKind;
    use solomon_sim::spells::model::SpellState;
    use solomon_sim::spells::{
        PrimarySpellCastAuthority, PrimarySpellInput, PrimarySpellWorldContext,
    };

    // Independently selected instruction-derived examples. Damage is already
    // resolved by 0x0065FFF0 before the handler's casting-rate/weak multiplier.
    for weapon_kind in [Some(PlayerWeaponKind::Staff), Some(PlayerWeaponKind::Wand), None] {
        for (rate, boost, mana, damage, spent, weak) in [
            (0.5, 1.0, 100.0, 2.0, 0.1, false),
            (1.0, 1.0, 100.0, 4.0, 0.2, false),
            (2.0, 1.0, 100.0, 8.0, 0.4, false),
            (2.0, 1.0, 0.3, 2.0, 0.3, true),
            (2.0, 1.0, 0.4, 2.0, 0.4, true),
            (0.5, 1.0, 0.15, 2.0, 0.1, false),
            (0.5, 1.0, 0.1, 2.0, 0.1, true),
            (2.0, 3.0, 100.0, 24.0, 0.4, false),
            (2.0, 3.0, 0.0, 6.0, 0.0, true),
        ] {
            let profile = PrimarySkillProfile::Weld(WeldPrimarySkillProfile {
                common: PrimarySkillProfileBase {
                    damage_minimum: 400.0 * boost,
                    damage_maximum: 400.0 * boost,
                    damage_roll_count: 1.0,
                    mana_cost: 20.0,
                    rank: 1,
                    skill_id: 1005,
                },
                build_id: 1005,
                cast_kind: WeldCastKind::Channel,
                damage_factor: boost,
                vector: WeldPrimaryVector {
                    build_id: 1005,
                    cast_kind: WeldCastKind::Channel,
                    values: vec![400.0, 20.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                },
            });
            let mut wizard = HubWizard::at(Vec2::ZERO);
            let mut state = SpellState::default();
            let mut rng = Rng::new(0);
            let mut found = false;
            for tick in 1..=20 {
                let result = state.step_primary(
                    &mut wizard,
                    PrimarySpellCastAuthority {
                        weapon_kind,
                        alive: true,
                        available_mana: mana,
                        cast_progress_factor: rate,
                        hand_cast_factor: 7.0,
                        action_time_factor: 1.0,
                        eligible: true,
                        plane_active: false,
                        primary_skill: &profile,
                    },
                    PrimarySpellInput { aim: Some(Vec2::new(100.0, 0.0)), primary: true },
                    &PrimarySpellWorldContext {
                        enhanced_effects: true,
                        flame_lash_view_bounds: None,
                        player_light_registration: None,
                        tick,
                        view_scale: 1.0,
                        owner_id: "caster",
                        world_key: "boneyard:rate",
                        targets: &[],
                        can_place: &|_, _, _| true,
                        can_traverse: &|_, _, _, _, _| true,
                        obstruction: &|_, _, _, _| None,
                        range_endpoint: &|_, _, _| Vec2::new(100.0, 0.0),
                        navigation: None,
                    },
                    &mut rng,
                    None,
                );
                use solomon_sim::spells::model::PrimarySpellTransient;
                use solomon_sim::spells::weld::WeldWorldActor;
                let particle = state.transients.iter().find_map(|actor| match actor {
                    PrimarySpellTransient::Weld(WeldWorldActor::Steam(actor))
                        if actor.common.birth_tick == tick =>
                    {
                        Some(actor)
                    }
                    _ => None,
                });
                let Some(particle) = particle else {
                    continue;
                };
                let emission = &result.channel_emissions[0];
                assert_eq!(
                    emission.underpowered, weak,
                    "weapon {weapon_kind:?} rate {rate} boost {boost} mana {mana}"
                );
                assert_eq!(emission.damage, damage);
                assert_eq!(emission.mana_cost, 0.2 * rate);
                assert_eq!(result.mana_spent, spent);
                assert_eq!(particle.contact_damage, damage);
                found = true;
                break;
            }
            assert!(found, "held input must reach Steam particle birth");
        }
    }
}
