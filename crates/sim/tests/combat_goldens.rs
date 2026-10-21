//! Focused replays of the player combat authority; fixtures are recorded from the Website.
use serde::Deserialize;
use solomon_sim::combat::{PlayerCombatState, PlayerCombatTickOptions, PlayerLifeState};

#[derive(Deserialize)]
struct CombatGoldens {
    scenarios: Vec<CombatScenario>,
}

#[derive(Deserialize)]
struct CombatScenario {
    name: String,
    samples: Vec<CombatSample>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CombatSample {
    action: CombatAction,
    state: [f64; 22],
    flags: [u8; 3],
    debit_accepted: Option<bool>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum CombatAction {
    Damage {
        amount: f64,
        strength: f64,
        record_hit: bool,
    },
    Debit {
        cost: f64,
    },
    RestoreHealth {
        amount: f64,
    },
    RestoreMana {
        amount: f64,
    },
    SetMana {
        amount: f64,
    },
    Poison {
        damage_per_second: f64,
        duration_seconds: f64,
    },
    Cold {
        ticks: u64,
    },
    Dazzle {
        ticks: u64,
    },
    Circle {
        ticks: u64,
    },
    Step {
        ticks: u64,
        health_recovery: Option<f64>,
        mana_recovery: Option<f64>,
        mana_ceiling: Option<f64>,
    },
    Reset,
    Spectate,
    Respawn,
}

fn state_row(state: &PlayerCombatState, tick: u64) -> [f64; 22] {
    [
        state.current_health,
        state.current_mana,
        state.maximum_health,
        state.maximum_mana,
        state.circle_slow_ticks_remaining as f64,
        state.cold_slow_ticks_remaining as f64,
        state.dazzle_ticks_remaining as f64,
        state.death_age_ticks as f64,
        state.death_epoch as f64,
        state.death_tick as f64,
        match state.life_state {
            PlayerLifeState::Alive => 0.0,
            PlayerLifeState::LethalPending => 1.0,
            PlayerLifeState::Dying => 2.0,
            PlayerLifeState::Spectating => 3.0,
        },
        state.last_damage_tick.map_or(-1.0, |tick| tick as f64),
        state.poison_damage_per_tick,
        f64::from(state.poison_before_cold),
        state.poison_ticks_remaining as f64,
        state.hit_feedback.strength,
        state.hit_feedback.tick as f64,
        state.hit_feedback.timer,
        state.movement_scale(),
        state.display_health(),
        state.hit_overlay_alpha(tick as f64 + 0.5),
        f64::from(state.collision_enabled()),
    ]
}

#[test]
fn player_resources_status_order_and_death_replay_the_website() {
    let goldens: CombatGoldens =
        serde_json::from_str(include_str!("fixtures/website/player-combat-goldens.json")).unwrap();
    for scenario in goldens.scenarios {
        let mut state = PlayerCombatState::new();
        let mut tick = 0;
        for (index, sample) in scenario.samples.into_iter().enumerate() {
            let mut flags = [0; 3];
            let mut debit_accepted = None;
            match sample.action {
                CombatAction::Damage { amount, strength, record_hit } => {
                    state.damage(amount, tick, record_hit, strength)
                }
                CombatAction::Debit { cost } => debit_accepted = Some(state.try_debit_mana(cost)),
                CombatAction::RestoreHealth { amount } => state.restore_health(amount),
                CombatAction::RestoreMana { amount } => state.restore_mana(amount),
                CombatAction::SetMana { amount } => state.set_mana(amount),
                CombatAction::Poison { damage_per_second, duration_seconds } => {
                    state.poison(damage_per_second, duration_seconds)
                }
                CombatAction::Cold { ticks } => state.cold_slow(ticks),
                CombatAction::Dazzle { ticks } => state.dazzle(ticks),
                CombatAction::Circle { ticks } => state.circle_slow_ticks_remaining = ticks,
                CombatAction::Reset => state.reset_for_new_run(),
                CombatAction::Spectate => state.set_spectating(),
                CombatAction::Respawn => state.respawn(),
                CombatAction::Step { ticks, health_recovery, mana_recovery, mana_ceiling } => {
                    for _ in 0..ticks {
                        tick += 1;
                        let options = PlayerCombatTickOptions {
                            tick: Some(tick),
                            health_recovery_per_tick: health_recovery.unwrap_or(0.001),
                            mana_recovery_per_tick: mana_recovery.unwrap_or(0.1),
                            mana_ceiling,
                            ..PlayerCombatTickOptions::default()
                        };
                        let result = state.step(options);
                        flags = [
                            u8::from(result.began_death_epoch),
                            u8::from(result.completed_death_presentation),
                            u8::from(result.emitted_death_burst),
                        ];
                    }
                }
            }
            assert_eq!(state_row(&state, tick), sample.state, "{} sample {index}", scenario.name);
            assert_eq!(flags, sample.flags, "{} flags {index}", scenario.name);
            assert_eq!(
                debit_accepted, sample.debit_accepted,
                "{} mana debit {index}",
                scenario.name
            );
        }
    }
}
