#![cfg(feature = "persistence")]

use solomon_sim::death_animations::NativeGolemDeathAnimation;
use solomon_sim::secondary::{
    NativeSecondaryActorKind as Kind, NativeSecondaryActorState, NativeSecondarySimulationState,
    SecondaryActorRead,
};

fn burst(lifetime_ticks: u64) -> NativeSecondaryActorState {
    let mut actor = NativeSecondaryActorState::new(
        0,
        Kind::MindblastBurst,
        "wizard".into(),
        "college".into(),
        Some(10),
    );
    actor.lifetime_ticks = lifetime_ticks;
    actor
}

fn ids(state: &NativeSecondarySimulationState) -> Vec<u64> {
    state.actors.iter().map(|actor| actor.id).collect()
}

#[test]
fn retirement_and_birth_preserve_order_through_checkpoint_continuation() {
    let mut live = NativeSecondarySimulationState::new(37);
    live.spawn_actor(burst(1));
    let mut death = NativeSecondaryActorState::new(
        0,
        Kind::GolemDeath,
        "wizard".into(),
        "college".into(),
        Some(45),
    );
    death.golem_death = Some(NativeGolemDeathAnimation {
        fragments: Vec::new(),
        star_rotation_degrees: 15.0,
        star_step_degrees: 3.0,
    });
    live.spawn_actor(death);
    live.spawn_actor(burst(3));

    // A different component layout sits between the retiring burst and its survivor.
    live.step_mindblast_presentation(1);
    assert_eq!(ids(&live), [2, 3]);
    assert_eq!(live.spawn_actor(burst(3)), 4);
    assert_eq!(live.actors.last().unwrap().age_ticks, 0);

    let saved = serde_json::to_value(&live).unwrap();
    let rows = saved["actors"].as_array().unwrap();
    assert_eq!(rows.iter().map(|row| row["id"].as_u64().unwrap()).collect::<Vec<_>>(), [2, 3, 4]);
    assert_eq!(rows[0]["golem_death"]["star_rotation_degrees"], 15.0);
    assert!(rows[1]["golem_death"].is_null());
    assert!(rows.iter().all(|row| row.get("actor").is_none() && row.get("common").is_none()));

    let mut restored: NativeSecondarySimulationState = serde_json::from_value(saved).unwrap();
    for tick in 2..=4 {
        live.step_mindblast_presentation(tick);
        restored.step_mindblast_presentation(tick);
        assert_eq!(restored, live);
        assert_eq!(serde_json::to_vec(&restored).unwrap(), serde_json::to_vec(&live).unwrap());
    }
    assert_eq!(ids(&live), [2]);
    live.remove_owner("wizard");
    assert_eq!(ids(&live), [2], "Golem fragments survive their owner's departure");
    assert!(live.actors.get(0).unwrap().golem_death().is_some());
}

#[test]
fn ability_owner_removal_preserves_other_players_and_shared_target_modifiers() {
    use solomon_sim::secondary::{NativeSecondaryPlayerState, NativeSecondaryTargetEffectState};
    use std::collections::BTreeMap;

    let mut live = NativeSecondarySimulationState::new(718);
    let records: BTreeMap<_, _> = ["zeta", "middle", "alpha"]
        .into_iter()
        .map(|id| {
            let mut player = NativeSecondaryPlayerState {
                global_cooldown_ticks: 40.0,
                ..NativeSecondaryPlayerState::default()
            };
            player.cooldown_ticks_by_skill[45] = 70.0;
            (id.to_owned(), player)
        })
        .collect();
    live.players = records.clone().into_iter().rev().collect();
    assert_eq!(
        serde_json::to_value(&live.players).unwrap(),
        serde_json::to_value(&records).unwrap()
    );
    let mut shared = NativeSecondaryTargetEffectState::new("boneyard".into(), 1);
    shared.cold_slow_ticks = 100;
    shared.cold_slow_factor = 0.5;
    shared.frost_burn_ticks = 10;
    shared.frost_burn_damage_per_tick = 3.0;
    shared.frost_burn_owner_id = Some("middle".into());
    let mut owned = NativeSecondaryTargetEffectState::new("boneyard".into(), 2);
    owned.frost_burn_ticks = 10;
    owned.frost_burn_damage_per_tick = 3.0;
    owned.frost_burn_owner_id = Some("middle".into());
    live.target_effects = [shared, owned].into_iter().collect();
    live.remove_owner("middle");
    assert_eq!(live.players.keys().map(String::as_str).collect::<Vec<_>>(), ["alpha", "zeta"]);
    assert_eq!(live.target_effects.len(), 1);
    assert_eq!(live.target_effects[0].target_id, 1);
    assert_eq!(live.target_effects[0].cold_slow_ticks, 100);
    assert_eq!(live.target_effects[0].frost_burn_owner_id, None);
    assert_eq!(live.target_effects[0].frost_burn_ticks, 0);
    live.players.insert("beta".into(), NativeSecondaryPlayerState::default());

    let saved = serde_json::to_vec(&live).unwrap();
    let mut restored: NativeSecondarySimulationState = serde_json::from_slice(&saved).unwrap();
    assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
    for state in [&mut live, &mut restored] {
        state.cooldown_rejuvenation("alpha");
        assert_eq!(state.players["alpha"].cooldown_ticks_by_skill[45], 0.0);
        assert_eq!(state.players["zeta"].cooldown_ticks_by_skill[45], 70.0);
        state.reset_world();
        assert!(state.target_effects.is_empty());
        assert_eq!(
            state.players.keys().map(String::as_str).collect::<Vec<_>>(),
            ["alpha", "beta", "zeta"]
        );
        assert!(
            state.players.values().all(|player| *player == NativeSecondaryPlayerState::default())
        );
    }
    assert_eq!(restored, live);
}
