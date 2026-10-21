//! Source `ml-bot-policy/world-state.ts`: pickups, allies and committed action history.

use crate::Vec2;
use crate::combat::PlayerLifeState;
use crate::economy::HubItemKind;
use crate::float::hypot;
use crate::loot::{NativeLootDropKind, NativeOrbKind};
use crate::profile::{WizardDiscipline, WizardElement};
use crate::session::GameSession;

use super::enemies::MlBotPolicyEnemyRow;
use super::numbers::{count_scaled, distance_squared, normalized, ratio, signed, unsigned, write};
use super::observer::{MlBotPolicyControllerKind, MlBotPolicyObservationContext};
use super::spec::scales;

#[derive(Clone, Copy, Debug)]
pub struct MlBotPolicyCommittedAction {
    pub ability_action: usize,
    pub movement: Vec2,
    pub target_action: usize,
    pub target_switched: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MlBotPolicyWorldMemory {
    last_cast_tick: Option<u64>,
    last_enemy_count: Option<usize>,
    last_hp_ratio: Option<f64>,
    last_mana_ratio: Option<f64>,
    last_move_tick: Option<u64>,
    last_target_hp_ratio: Option<f64>,
    last_target_id: Option<usize>,
}

pub struct MlBotPolicyHostContext<'a> {
    pub observation: &'a MlBotPolicyObservationContext<'a>,
    pub memory: &'a MlBotPolicyWorldMemory,
    pub previous_action: Option<MlBotPolicyCommittedAction>,
    pub target_id: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyWorldObservation {
    pub block_g: Vec<f32>,
    pub block_h: Vec<f32>,
    pub block_i: Vec<f32>,
    pub block_j: Vec<f32>,
    pub next_memory: MlBotPolicyWorldMemory,
}

pub fn observe_ml_bot_policy_world_state(
    session: &GameSession,
    player_id: &str,
    enemies: &[MlBotPolicyEnemyRow],
    context: MlBotPolicyHostContext<'_>,
) -> Result<MlBotPolicyWorldObservation, String> {
    let participant = session
        .participant(player_id)
        .ok_or_else(|| format!("ML bot policy world has no player {player_id}"))?;
    let player = &participant.player;
    let position = participant.wizard.position;
    let derived = player.derived_stats();
    let target = context.target_id.and_then(|id| enemies.iter().find(|row| row.id == id));
    let block_g = observe_pickups(session, position);
    let block_i = observe_allies(session, player_id, position, context.observation);
    let block_j = [
        status_time(player.progression.damage_x4_ticks_remaining),
        status_time(player.progression.poison_immunity_ticks_remaining),
        status_time(player.progression.mind_chug_ticks_remaining),
    ]
    .map(|value| value as f32)
    .to_vec();
    let mut block_h = vec![0.0; 43];
    let threats = enemies
        .iter()
        .filter(|enemy| {
            hypot(enemy.position.x - position.x, enemy.position.y - position.y)
                <= scales::THREAT_RADIUS_WORLD
        })
        .collect::<Vec<_>>();
    append_nearest(&mut block_h, 2, enemies.first(), position);
    append_nearest(&mut block_h, 5, threats.first().copied(), position);
    if let Some(threat) = threats.first() {
        let direction = normalized(threat.position.x - position.x, threat.position.y - position.y);
        block_h[8] = -direction.x as f32;
        block_h[9] = -direction.y as f32;
    }
    block_h[0] = unsigned(enemies.len() as f64, scales::ENEMY_COUNT) as f32;
    block_h[1] = unsigned(threats.len() as f64, scales::THREAT_COUNT) as f32;
    if let Some(world) = &session.boneyard {
        let bounds = world.active_bounds();
        let center = Vec2::new(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
        let delta = Vec2::new(center.x - position.x, center.y - position.y);
        let edge_distance = (position.x - bounds.x)
            .min(bounds.x + bounds.width - position.x)
            .min(position.y - bounds.y)
            .min(bounds.y + bounds.height - position.y);
        write(
            &mut block_h,
            10,
            &[
                signed(delta.x, scales::RANGE),
                signed(delta.y, scales::RANGE),
                unsigned(hypot(delta.x, delta.y), scales::RANGE),
                ((position.x - bounds.x) / bounds.width).clamp(-1.0, 1.0),
                ((position.y - bounds.y) / bounds.height).clamp(-1.0, 1.0),
                1.0 - unsigned(edge_distance, scales::EDGE_PRESSURE_RANGE),
            ],
        );
    }
    let element_index = match player.selection.element {
        WizardElement::Fire => 16,
        WizardElement::Water => 17,
        WizardElement::Earth => 18,
        WizardElement::Air => 19,
        WizardElement::Ether => 20,
    };
    block_h[element_index] = 1.0;
    // Source exposes only ordinary discipline flags. An accepted elemental
    // second root leaves all three zero without changing the primary flags.
    block_h[21] = if player.selection.discipline == WizardDiscipline::Mind { 1.0 } else { 0.0 };
    block_h[22] = if player.selection.discipline == WizardDiscipline::Body { 1.0 } else { 0.0 };
    block_h[23] = if player.selection.discipline == WizardDiscipline::Arcane { 1.0 } else { 0.0 };
    let hp_ratio = ratio(player.combat.current_health, player.combat.maximum_health);
    let mana_ratio = ratio(player.combat.current_mana, player.combat.maximum_mana);
    let target_hp_ratio =
        target.map_or(0.0, |target| ratio(target.current_health, target.maximum_health));
    let previous = context.previous_action;
    let last_cast_tick = if previous.is_some_and(|action| (1..=9).contains(&action.ability_action))
    {
        Some(session.ticks)
    } else {
        context.memory.last_cast_tick
    };
    let last_move_tick =
        if previous.is_some_and(|action| action.movement.x != 0.0 || action.movement.y != 0.0) {
            Some(session.ticks)
        } else {
            context.memory.last_move_tick
        };
    write(
        &mut block_h,
        24,
        &[
            context.memory.last_hp_ratio.map_or(0.0, |last| (hp_ratio - last).clamp(-1.0, 1.0)),
            context.memory.last_mana_ratio.map_or(0.0, |last| (mana_ratio - last).clamp(-1.0, 1.0)),
            if target.is_some_and(|target| context.memory.last_target_id == Some(target.id)) {
                context
                    .memory
                    .last_target_hp_ratio
                    .map_or(0.0, |last| (target_hp_ratio - last).clamp(-1.0, 1.0))
            } else {
                0.0
            },
            context.memory.last_enemy_count.map_or(0.0, |last| {
                signed(enemies.len() as f64 - last as f64, scales::ENEMY_COUNT)
            }),
            previous.map_or(0.0, |action| action.movement.x),
            previous.map_or(0.0, |action| action.movement.y),
            f64::from(previous.is_some_and(|action| action.ability_action == 1)),
            f64::from(previous.is_some_and(|action| (2..=9).contains(&action.ability_action))),
            player
                .combat
                .last_damage_tick
                .map_or(1.0, |tick| history_time(session.ticks.saturating_sub(tick))),
            last_cast_tick.map_or(1.0, |tick| history_time(session.ticks.saturating_sub(tick))),
            last_move_tick.map_or(1.0, |tick| history_time(session.ticks.saturating_sub(tick))),
            previous
                .map_or(0.0, |action| unsigned(action.target_action as f64, scales::TARGET_ACTION)),
            f64::from(previous.is_some_and(|action| action.target_switched)),
            f64::from(player.book.effective_ranks[52] > 0),
            f64::from(player.progression.pending_offer.as_ref().is_some_and(|offer| {
                offer.options.iter().any(|option| option.weld_build_id.is_some())
            })),
            signed(derived.offensive_damage_factor, scales::MULTIPLIER),
            signed(derived.offensive_mana_cost_factor, scales::MULTIPLIER),
            signed(derived.cast_progress_factor, scales::MULTIPLIER),
            signed(derived.secondary_recharge_factor, scales::MULTIPLIER),
        ],
    );
    Ok(MlBotPolicyWorldObservation {
        block_g,
        block_h,
        block_i,
        block_j,
        next_memory: MlBotPolicyWorldMemory {
            last_cast_tick,
            last_enemy_count: Some(enemies.len()),
            last_hp_ratio: Some(hp_ratio),
            last_mana_ratio: Some(mana_ratio),
            last_move_tick,
            last_target_hp_ratio: target.map(|_| target_hp_ratio),
            last_target_id: target.map(|target| target.id),
        },
    })
}

fn observe_pickups(session: &GameSession, position: Vec2) -> Vec<f32> {
    let mut block = vec![0.0; 4 * 21 + 1];
    let Some(world) = &session.boneyard else {
        return block;
    };
    let mut actors = world.loot.actors.iter().collect::<Vec<_>>();
    actors.sort_by(|left, right| {
        distance_squared(left.position, position)
            .total_cmp(&distance_squared(right.position, position))
            .then(left.id.cmp(&right.id))
    });
    for (slot, actor) in actors.iter().take(4).enumerate() {
        let relative = Vec2::new(actor.position.x - position.x, actor.position.y - position.y);
        let direction = normalized(relative.x, relative.y);
        let kind = actor.item.as_ref().map(|item| item.kind);
        write(
            &mut block,
            slot * 21,
            &[
                1.0,
                direction.x,
                direction.y,
                unsigned(hypot(relative.x, relative.y), scales::RANGE),
                f64::from(actor.kind == NativeLootDropKind::Gold),
                f64::from(
                    actor.kind == NativeLootDropKind::Orb
                        && actor.orb_kind == Some(NativeOrbKind::Health),
                ),
                f64::from(
                    actor.kind == NativeLootDropKind::Orb
                        && actor.orb_kind == Some(NativeOrbKind::Mana),
                ),
                f64::from(actor.kind == NativeLootDropKind::Sack),
                f64::from(actor.kind == NativeLootDropKind::Bonus),
                f64::from(actor.kind == NativeLootDropKind::Sack && actor.item.is_some()),
                f64::from(kind == Some(HubItemKind::HealthPotion)),
                f64::from(kind == Some(HubItemKind::ManaPotion)),
                f64::from(kind == Some(HubItemKind::WizardChug)),
                f64::from(kind == Some(HubItemKind::Antidote)),
                f64::from(kind == Some(HubItemKind::MindChug)),
                f64::from(kind == Some(HubItemKind::RejuvenationPotion)),
                f64::from(kind == Some(HubItemKind::ModPotion)),
                f64::from(kind == Some(HubItemKind::Equipment)),
                f64::from(kind == Some(HubItemKind::Key)),
                count_scaled(actor.item.as_ref().map_or(0.0, |item| f64::from(item.quantity))),
                count_scaled(actor.amount as f64),
            ],
        );
    }
    block[84] = unsigned(actors.len() as f64, scales::PICKUP_COUNT) as f32;
    block
}

fn observe_allies(
    session: &GameSession,
    player_id: &str,
    position: Vec2,
    context: &MlBotPolicyObservationContext<'_>,
) -> Vec<f32> {
    let mut block = vec![0.0; 4 * 10 + 1];
    let mut allies = session
        .run
        .eligible_player_ids
        .iter()
        .filter(|id| id.as_str() != player_id && session.participant(id).is_some())
        .collect::<Vec<_>>();
    allies.sort_by(|left, right| {
        let left_position = session.wizard(left).expect("eligible ally retained above").position;
        let right_position = session.wizard(right).expect("eligible ally retained above").position;
        distance_squared(left_position, position)
            .total_cmp(&distance_squared(right_position, position))
            .then_with(|| (context.compare_player_ids)(left, right))
    });
    for (slot, id) in allies.iter().take(4).enumerate() {
        let ally = session.participant(id).expect("eligible ally retained above");
        let combat = &ally.player.combat;
        let relative =
            Vec2::new(ally.wizard.position.x - position.x, ally.wizard.position.y - position.y);
        let direction = normalized(relative.x, relative.y);
        let movement =
            context.active_inputs.get(id.as_str()).map_or(Vec2::ZERO, |input| input.movement);
        write(
            &mut block,
            slot * 10,
            &[
                1.0,
                direction.x,
                direction.y,
                unsigned(hypot(relative.x, relative.y), scales::RANGE),
                ratio(combat.current_health, combat.maximum_health),
                ratio(combat.current_mana, combat.maximum_mana),
                f64::from(combat.life_state == PlayerLifeState::Alive),
                f64::from(
                    context.controllers.get(id.as_str()) == Some(&MlBotPolicyControllerKind::Human),
                ),
                movement.x,
                movement.y,
            ],
        );
    }
    block[40] = unsigned(allies.len() as f64, scales::ALLY_COUNT) as f32;
    block
}

fn append_nearest(
    block: &mut [f32],
    offset: usize,
    enemy: Option<&MlBotPolicyEnemyRow>,
    position: Vec2,
) {
    let Some(enemy) = enemy else {
        return;
    };
    let relative = Vec2::new(enemy.position.x - position.x, enemy.position.y - position.y);
    let direction = normalized(relative.x, relative.y);
    write(
        block,
        offset,
        &[direction.x, direction.y, unsigned(hypot(relative.x, relative.y), scales::RANGE)],
    );
}

fn history_time(ticks: u64) -> f64 {
    unsigned(ticks as f64 / scales::TICK_RATE, scales::HISTORY_SECONDS)
}
fn status_time(ticks: u64) -> f64 {
    unsigned(ticks as f64 / scales::TICK_RATE, scales::STATUS_DURATION_SECONDS)
}
