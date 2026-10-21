//! Source `ml-bot-policy/minions.ts`: own and allied Golems in retained actor order.

use std::collections::BTreeSet;

use crate::Vec2;
use crate::float::hypot;
use crate::secondary::golem::NativeGolemPhase;
use crate::secondary::{NativeSecondaryActorKind, NativeSecondarySimulationState};

use super::numbers::{distance_squared, normalized, ratio, unsigned, write};
use super::spec::scales;

#[derive(Clone, Debug)]
pub struct MlBotPolicyMinionObservation {
    pub block_s: Vec<f32>,
    pub own_target_ids: BTreeSet<usize>,
    pub secondary_minion_active: [bool; 8],
}

pub fn observe_ml_bot_policy_minions(
    secondary: &NativeSecondarySimulationState,
    player_id: &str,
    position: Vec2,
    quickbar: &[Option<usize>; 8],
    world_key: &str,
) -> MlBotPolicyMinionObservation {
    let mut golems = secondary
        .actors
        .iter()
        .filter(|actor| {
            actor.kind == NativeSecondaryActorKind::Golem
                && actor.golem.is_some()
                && actor.world_key == world_key
        })
        .collect::<Vec<_>>();
    golems.sort_by(|left, right| {
        (right.owner_id == player_id)
            .cmp(&(left.owner_id == player_id))
            .then_with(|| {
                distance_squared(left.position, position)
                    .total_cmp(&distance_squared(right.position, position))
            })
            .then(left.id.cmp(&right.id))
    });
    let mut block_s = vec![0.0; 4 * 15 + 2];
    for (slot, actor) in golems.iter().take(4).enumerate() {
        let golem = actor.golem.as_ref().expect("the Source minion filter requires a Golem");
        let relative = Vec2::new(actor.position.x - position.x, actor.position.y - position.y);
        let direction = normalized(relative.x, relative.y);
        write(
            &mut block_s,
            slot * 15,
            &[
                1.0,
                f64::from(actor.owner_id == player_id),
                direction.x,
                direction.y,
                unsigned(hypot(relative.x, relative.y), scales::RANGE),
                ratio(golem.current_health, golem.maximum_health),
                unsigned(golem.maximum_health, scales::HP),
                f64::from(golem.iron),
                f64::from(golem.phase == NativeGolemPhase::Assembly),
                f64::from(golem.phase == NativeGolemPhase::Active),
                f64::from(golem.phase == NativeGolemPhase::Attack),
                f64::from(golem.phase == NativeGolemPhase::Provoke),
                f64::from(actor.target_id.is_some()),
                unsigned(golem.reflect_factor, scales::MULTIPLIER),
                unsigned(actor.age_ticks as f64 / scales::TICK_RATE, scales::MINION_AGE_SECONDS),
            ],
        );
    }
    let mut own_count = 0;
    let mut ally_count = 0;
    let mut own_target_ids = BTreeSet::new();
    let mut secondary_minion_active = [false; 8];
    for actor in golems {
        if actor.owner_id != player_id {
            ally_count += 1;
            continue;
        }
        own_count += 1;
        if let Some(id) = actor.target_id {
            own_target_ids.insert(id);
        }
        if let Some(skill_id) = actor.skill_id {
            for (slot, skill) in quickbar.iter().enumerate() {
                if *skill == Some(skill_id) {
                    secondary_minion_active[slot] = true;
                }
            }
        }
    }
    block_s[60] = unsigned(f64::from(own_count), scales::MINION_COUNT) as f32;
    block_s[61] = unsigned(f64::from(ally_count), scales::MINION_COUNT) as f32;
    MlBotPolicyMinionObservation { block_s, own_target_ids, secondary_minion_active }
}
