//! Current accepted damage owns rescue and lethal rejection; the base tick preserves it.

#[path = "support/player_combat.rs"]
#[allow(dead_code)] // Shared player setup also serves the complete resource/skill fixtures.
mod support;

use serde_json::{Value, json};
use solomon_sim::NativeRng;
use solomon_sim::combat::{PlayerCombatState, PlayerCombatTickOptions, PlayerLifeState};
use solomon_sim::player::PlayerState;
use solomon_sim::session::player_combat::{PlayerCombatContext, step_player_combat};

fn life(state: PlayerLifeState) -> &'static str {
    match state {
        PlayerLifeState::Alive => "alive",
        PlayerLifeState::LethalPending => "lethal-pending",
        PlayerLifeState::Dying => "dying",
        PlayerLifeState::Spectating => "spectating",
    }
}

fn snapshot(player: &PlayerState) -> Value {
    let p = &player.combat;
    let hagatha = player.progression.hagatha_runtime;
    let potions: Vec<_> = player
        .economy
        .backpack
        .iter()
        .filter(|item| item.native_type_id == 7001)
        .map(|item| (item.native_subtype, item.quantity))
        .collect();
    json!({"health":p.current_health,"mana":p.current_mana,"life":life(p.life_state),
        "death":[p.death_age_ticks,p.death_epoch,p.death_tick],"corpseConsumed":p.corpse_consumed,
        "collision":p.collision_enabled(),"protection":[p.rescue_protection.fraction,
            p.rescue_protection.next_particle_id as f64,p.rescue_protection.particles.len() as f64],
        "hagatha":{"cheatDeathCharges":hagatha.cheat_death_charges,"reverieActive":hagatha.reverie_active,
            "serendipityActive":hagatha.serendipity_active},"potions":potions})
}

#[test]
fn current_damage_receiver_preserves_blindness_rescue_charm_and_corpse_boundaries() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-player-damage-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let spec = &case["spec"];
        let setup = json!({"perks":spec["perks"],"removePotions":spec["removePotions"],
            "combat":{"currentHealth":50,"currentMana":10}});
        let (mut player, wizard, primary) = support::configured_player(&setup);
        player.combat.rescue_protection.fraction = spec["fraction"].as_f64().unwrap();
        let blindness = spec["blindness"].as_f64().unwrap();
        let flags = player.damage_with_result(
            spec["amount"].as_f64().unwrap(),
            201,
            spec["scaled"].as_bool().unwrap(),
            true,
            0.75,
            blindness,
        );
        support::assert_snapshot(
            &json!({"autoHealthPotionUsed":flags.auto_health_potion_used,
            "cheatDeathTriggered":flags.cheat_death_triggered,"hagathaCharmsLost":flags.hagatha_charms_lost}),
            &case["flags"],
            &format!("receiver{index} flags"),
        );
        let mut rng = NativeRng::new(917);
        for frame in case["frames"].as_array().unwrap() {
            let tick = frame["tick"].as_u64().unwrap();
            if tick > 201 {
                step_player_combat(PlayerCombatContext {
                    player_id: "local",
                    player: &mut player,
                    wizard: &wizard,
                    primary_cast: &primary,
                    acting_staff: false,
                    blindness_ticks_remaining: blindness,
                    tick,
                    mana_ceiling: None,
                    rng: &mut rng,
                });
            }
            support::assert_snapshot(
                &snapshot(&player),
                &frame["expected"],
                &format!("receiver{index} tick{tick}"),
            );
        }
    }
    for case in fixture["core"].as_array().unwrap() {
        let mut state = PlayerCombatState::new();
        state.corpse_consumed = case["corpseConsumed"].as_bool().unwrap();
        state.current_health = -50.0;
        state.death_age_ticks = 264;
        state.death_epoch = 2;
        state.death_tick = 158;
        state.rescue_protection.fraction = 0.75;
        let action = case["action"].as_str().unwrap();
        state.life_state =
            if action == "step" { PlayerLifeState::Dying } else { PlayerLifeState::Spectating };
        let burst = match action {
            "step" => {
                state
                    .step(PlayerCombatTickOptions {
                        tick: Some(300),
                        ..PlayerCombatTickOptions::default()
                    })
                    .emitted_death_burst
            }
            "respawn" => {
                state.respawn();
                false
            }
            "reset" => {
                state.reset_for_new_run();
                false
            }
            _ => panic!("unknown Source core action"),
        };
        support::assert_snapshot(
            &json!({"health":state.current_health,"life":life(state.life_state),
            "death":[state.death_age_ticks,state.death_epoch,state.death_tick],"corpseConsumed":state.corpse_consumed,
            "fraction":state.rescue_protection.fraction,"collision":state.collision_enabled(),"burst":burst}),
            &case["expected"],
            action,
        );
    }
}
