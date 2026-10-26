//! Enemy target eligibility and selection from Website `enemies/targeting.ts`.

use super::model::EnemyTarget;
use super::model::{CoffinPhase, EnemyActor, EnemyBrain, EnemyLifeState};
use super::path::native_enemy_target_refresh_ticks;
use super::world::EnemyStepContext;
use super::world::EnemyWorld;
use crate::Vec2;
use crate::enemies::model::EnemyActorRead;
use crate::enemies::projectiles::EnemyProjectileRead;
use crate::float::hypot;
use crate::heading::heading_degrees_from_vector;
use crate::spells::targeting::{
    PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
    native_primary_cell_coordinate,
};

#[must_use]
pub fn target_by_id<'a>(id: Option<&str>, players: &'a [EnemyTarget]) -> Option<&'a EnemyTarget> {
    id.and_then(|id| players.iter().find(|player| player.id == id))
}

pub fn refresh_target(actor: &mut EnemyActor, context: &EnemyStepContext<'_>) {
    if let Some(current) = target_by_id(actor.target_player_id.as_deref(), context.players)
        && target_eligible(current)
        && context.tick < actor.next_target_refresh_tick
    {
        return;
    }
    let next =
        nearest_eligible_target(actor.position, context.players).map(|target| target.id.clone());
    if next != actor.target_player_id {
        actor.path.clear_route();
    }
    actor.next_target_refresh_tick =
        context.tick + native_enemy_target_refresh_ticks(actor.config.pathfinding_mode);
    actor.target_player_id = next;
}

#[must_use]
pub fn target_eligible(target: &EnemyTarget) -> bool {
    target.alive && target.connected && target.eligible
}

#[must_use]
pub fn target_player_within_attack_reach(
    actor: &impl EnemyActorRead,
    id: Option<&str>,
    players: &[EnemyTarget],
    center_reach: f64,
) -> bool {
    let Some(target) = target_by_id(id, players) else {
        return false;
    };
    target_eligible(target)
        && hypot(target.position.x - actor.position.x, target.position.y - actor.position.y)
            <= center_reach.max(actor.config.collision_radius + target.collision_radius + 0.1)
}

#[must_use]
pub fn target_within_attack_reach(
    actor: &impl EnemyActorRead,
    players: &[EnemyTarget],
    center_reach: f64,
) -> bool {
    target_player_within_attack_reach(
        actor,
        actor.target_player_id.as_deref(),
        players,
        center_reach,
    )
}

pub fn track_enemy_action_heading(actor: &mut EnemyActor, players: &[EnemyTarget]) {
    if let Some(target) = target_by_id(actor.target_player_id.as_deref(), players)
        && target_eligible(target)
    {
        actor.heading_deg = heading_degrees_from_vector(
            target.position.x - actor.position.x,
            target.position.y - actor.position.y,
        );
    }
}

#[must_use]
pub fn enemy_navigation_clearance(actor: &impl EnemyActorRead) -> f64 {
    if actor.config.enemy_token == super::EnemyToken::Demon { 50.0 } else { 25.0 }
}

#[must_use]
pub fn target_distance(actor: &impl EnemyActorRead, players: &[EnemyTarget]) -> f64 {
    target_by_id(actor.target_player_id.as_deref(), players).map_or(f64::INFINITY, |target| {
        hypot(target.position.x - actor.position.x, target.position.y - actor.position.y)
    })
}

#[must_use]
pub fn enemy_target_line_of_sight_is_clear(
    actor: &impl EnemyActorRead,
    context: &EnemyStepContext<'_>,
) -> bool {
    let Some(target) = target_by_id(actor.target_player_id.as_deref(), context.players) else {
        return false;
    };
    target_eligible(target)
        && context.geometry.direct_path_clear(
            actor.position,
            target.position,
            enemy_navigation_clearance(actor),
            actor.config.collision_radius,
        )
}

#[must_use]
pub fn nearest_eligible_target(position: Vec2, players: &[EnemyTarget]) -> Option<&EnemyTarget> {
    let mut selected = None;
    let mut selected_distance = f64::INFINITY;
    for player in players {
        if !(player.alive && player.connected && player.eligible) {
            continue;
        }
        let dx = player.position.x - position.x;
        let dy = player.position.y - position.y;
        let distance = dx * dx + dy * dy;
        if distance < selected_distance {
            selected = Some(player);
            selected_distance = distance;
        }
    }
    selected
}

/// `boneyardEnemyActorFlags`: hidden Coffins reserve their row but do not enter hostile
/// queries. Every dying family has already left the hostile lane.
#[must_use]
pub fn boneyard_enemy_actor_flags(actor: &impl EnemyActorRead) -> u32 {
    if actor.life_state != EnemyLifeState::Alive {
        return 0;
    }
    if matches!(actor.brain(),EnemyBrain::Coffin(brain) if matches!(brain.phase,CoffinPhase::Hidden|CoffinPhase::Death))
    {
        return 0;
    }
    0x2
}

/// `boneyardEnemyCollisionRadius`: Spider attachment and Portal materialization use
/// their active body radius, rather than the constructor's persistent config radius.
#[must_use]
pub fn boneyard_enemy_collision_radius(actor: &impl EnemyActorRead) -> f64 {
    match actor.brain() {
        EnemyBrain::Spider(brain) => {
            if brain.attached {
                5.0
            } else {
                15.0
            }
        }
        EnemyBrain::Portal(brain) => {
            if brain.age_ticks < 10 {
                45.0
            } else {
                5.0
            }
        }
        _ => actor.config.collision_radius,
    }
}

impl EnemyWorld {
    /// Arrow and Silk rows from `spell-combat/targets.ts::primaryWaterTargetRows`.
    /// Append these after `spell_targets()` to retain the source's query lane order.
    #[must_use]
    pub fn projectile_force_targets(&self) -> Vec<PrimarySpellTarget> {
        let mut targets = Vec::new();
        for projectile in self.projectiles.iter() {
            if projectile.kind() != super::projectiles::EnemyProjectileKind::Arrow {
                continue;
            }
            let state = projectile.state();
            targets.push(PrimarySpellTarget {
                active: state.age_ticks < state.lifetime_ticks,
                actor_flags: 0x80,
                attachment: Vec2::ZERO,
                body_radius: state.contact_radius,
                cell_binding_order: state.native_cell_binding_order,
                heading_degrees: None,
                grid_position: None,
                id: format!("projectile:{}", state.id),
                kind: PrimarySpellTargetKind::Projectile,
                native_priority: 0,
                pending_remove: false,
                position: state.position,
                query_lane: PrimarySpellQueryLane::Transient,
                registration_order: self.painter_registration_ordinal(state.painter_registration),
            });
        }
        for silk in &self.silks {
            targets.push(PrimarySpellTarget {
                active: true,
                actor_flags: 0x1000,
                attachment: Vec2::ZERO,
                body_radius: 15.0,
                cell_binding_order: silk.native_cell_binding_order,
                heading_degrees: None,
                grid_position: None,
                id: format!("silk:{}", silk.id),
                kind: PrimarySpellTargetKind::Projectile,
                native_priority: 0,
                pending_remove: false,
                position: silk.state.position,
                query_lane: PrimarySpellQueryLane::Grid,
                registration_order: silk.native_registration_order,
            });
        }
        targets
    }

    /// `spell-combat/targets.ts::primaryTargetRows`, in actor order followed by Maggots.
    /// Cell binding order is retained even when registration order differs after motion.
    #[must_use]
    pub fn spell_targets(&self) -> Vec<PrimarySpellTarget> {
        let mut targets = Vec::with_capacity(self.actors.len() + self.maggots.len());
        for actor in self.actors.iter() {
            targets.push(PrimarySpellTarget {
                active: actor.life_state == EnemyLifeState::Alive,
                actor_flags: boneyard_enemy_actor_flags(&actor),
                attachment: Vec2::ZERO,
                body_radius: boneyard_enemy_collision_radius(&actor),
                cell_binding_order: actor.native_cell_binding_order,
                heading_degrees: Some(actor.heading_deg),
                grid_position: None,
                id: format!("enemy:{}", actor.id),
                kind: PrimarySpellTargetKind::Enemy,
                native_priority: 0,
                pending_remove: false,
                position: actor.position,
                query_lane: PrimarySpellQueryLane::Grid,
                registration_order: actor.native_registration_order,
            });
        }
        for maggot in self.maggots.iter() {
            targets.push(PrimarySpellTarget {
                active: maggot.life_state == EnemyLifeState::Alive && maggot.combat_active,
                actor_flags: 0x2,
                attachment: Vec2::ZERO,
                body_radius: maggot.collision_radius,
                cell_binding_order: maggot.native_cell_binding_order,
                heading_degrees: Some(maggot.heading_deg),
                grid_position: None,
                id: format!("enemy:{}", maggot.id),
                kind: PrimarySpellTargetKind::Enemy,
                native_priority: 0,
                pending_remove: false,
                position: maggot.position,
                query_lane: PrimarySpellQueryLane::Grid,
                registration_order: maggot.native_registration_order,
            });
        }
        targets
    }

    /// `positionBoneyardEnemy`: the caller owns collision resolution; this accepts the
    /// final spell impulse and rebinds only when the native hundred-unit cell changes.
    pub fn position_enemy(&mut self, actor_id: usize, position: Vec2) -> bool {
        assert!(
            position.x.is_finite() && position.y.is_finite(),
            "enemy spell-impulse position must be finite"
        );
        if let Some(mut actor) = self.actors.iter_mut().find(|actor| actor.id == actor_id) {
            if actor.life_state != EnemyLifeState::Alive {
                return false;
            }
            if primary_cell_changed(actor.position, position) {
                actor.native_cell_binding_order = self.next_native_cell_binding_order;
                self.next_native_cell_binding_order += 1;
            }
            actor.position = position;
            return true;
        }
        let Some(mut maggot) = self.maggots.iter_mut().find(|maggot| maggot.id == actor_id) else {
            return false;
        };
        if maggot.life_state != EnemyLifeState::Alive || !maggot.combat_active {
            return false;
        }
        if primary_cell_changed(maggot.position, position) {
            maggot.native_cell_binding_order = self.next_native_cell_binding_order;
            self.next_native_cell_binding_order += 1;
        }
        maggot.position = position;
        maggot.native_tree_query_member = true;
        true
    }
}

pub(super) fn primary_cell_changed(before: Vec2, after: Vec2) -> bool {
    native_primary_cell_coordinate(before.x) != native_primary_cell_coordinate(after.x)
        || native_primary_cell_coordinate(before.y) != native_primary_cell_coordinate(after.y)
}
