//! Contact-owned Cocoon construction and removal from `enemies/construction.ts` and
//! `enemies/cocoon.ts`. Contact births intentionally use the enemy's standalone entropy.
use super::config::EnemyArenaScalars;
use super::model::*;
use super::path::EnemyNavigation;
use super::world::{
    EnemyGeometry, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemySpawnContext, EnemyWorld,
};
use super::{EnemySpawnIntent, EnemyToken};
use crate::Vec2;
use crate::painter::WorldPainterOrder;

impl EnemyWorld {
    /// The source contact caller ignores these returned spawn events. Keep its current
    /// tick queues intact while retaining the actor, RNG draws and consumed event IDs.
    pub fn add_native_cocoon(
        &mut self,
        owner: &str,
        position: Vec2,
        players: &[EnemyTarget],
        tick: u64,
        painter_order: Option<&mut WorldPainterOrder>,
    ) -> Vec<EnemyEvent> {
        let events_start = self.events.len();
        let spawned_start = self.spawned_actor_ids.len();
        let mut intent = EnemySpawnIntent::new(
            EnemyToken::Cocoon,
            Vec2::new(position.x, position.y + 1.0),
            self.next_synthetic_spawn_intent_id,
            tick,
            0,
        );
        self.next_synthetic_spawn_intent_id += 1;
        intent.cocoon_target_player_id = Some(owner.to_owned());
        self.spawn(
            &[intent],
            EnemySpawnContext {
                players,
                tick,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &CocoonBirth,
            painter_order,
        );
        self.spawned_actor_ids.truncate(spawned_start);
        if let Some(tick) = self.last_step_tick {
            self.step_pre_world_effect_births(tick);
        }
        if self.featured_boss_id.is_some_and(|id| !self.actors.iter().any(|actor| actor.id == id)) {
            self.featured_boss_id = None;
        }
        self.events.split_off(events_start)
    }

    pub fn remove_cocoon_owner(&mut self, owner: &str) -> bool {
        let removed_web = self.webbed_players.remove(owner).is_some();
        let before = self.actors.len();
        self.actors.retain(|actor|!matches!(&actor.brain,EnemyBrain::Cocoon(brain) if brain.owner_player_id.as_deref()==Some(owner)));
        removed_web || before != self.actors.len()
    }
}

/// `addNativeCocoon` supplies only a root-preserving movement callback. Other geometry
/// callbacks do not belong to this birth path and must never silently become world rules.
struct CocoonBirth;
impl EnemyNavigation for CocoonBirth {
    fn direct_path_clear(&self, _start: Vec2, _end: Vec2, _clearance: f64, _radius: f64) -> bool {
        unreachable!("Cocoon birth does not query navigation")
    }
    fn find_route(
        &self,
        _start: Vec2,
        _end: Vec2,
        _clearance: f64,
        _radius: f64,
    ) -> Option<Vec<Vec2>> {
        unreachable!("Cocoon birth does not query navigation")
    }
}
impl EnemyGeometry for CocoonBirth {
    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2 {
        request.position
    }
    fn line_obstruction(&self, _start: Vec2, _end: Vec2, _mask: u32) -> Option<Vec2> {
        unreachable!("Cocoon birth does not query world lines")
    }
    fn projectile_world_blocked(&self, _query: EnemyProjectileWorldQuery) -> bool {
        unreachable!("Cocoon birth does not query projectiles")
    }
}
