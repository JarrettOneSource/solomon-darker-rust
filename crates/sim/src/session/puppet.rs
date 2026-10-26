//! Commit late spell targets before the next enemy pass, from
//! `core-server/enemies/puppet-hits.ts::finalizeBoneyardPuppetQueries`.

use std::collections::HashSet;

use super::*;
use crate::boneyard::world::boneyard_mouth_world_targets;
use crate::boneyard::world_light::BoneyardLightEnvironment;
use crate::enemies::boss_spell::BossSpellKind;
use crate::enemies::projectiles::EnemyProjectile;
use crate::enemies::puppet_hits::{EnemyPuppetHitKind, boneyard_puppet_target};
use crate::secondary::NativeSecondaryActorKind;
use crate::spells::targeting::PrimarySpellQueryLane;

impl GameSession {
    pub(super) fn finalize_puppet_queries(&mut self) {
        let Some(world) = &mut self.boneyard else {
            return;
        };
        world.enemies.assert_painter_registrations_committed();
        let world_key = format!("boneyard:{}", world.loaded.run_id);
        let mut targets = boneyard_mouth_world_targets(
            world,
            BoneyardLightEnvironment {
                primary_spells: Some(&self.spells),
                secondary_abilities: Some(&self.secondary),
            },
        );
        let mut live_bindings = HashSet::new();
        for id in &self.participant_order {
            live_bindings.insert(id.clone());
            world.enemies.bind_native_query_target(id, self.participants[id].wizard.position);
        }
        for actor in self.secondary.actors.iter() {
            if actor.kind != NativeSecondaryActorKind::Golem || actor.world_key != world_key {
                continue;
            }
            let id = format!("golem:{}", actor.id);
            live_bindings.insert(id.clone());
            world.enemies.bind_native_query_target(&id, actor.position);
        }
        world.enemies.bind_world_puppet_targets(&targets);
        for target in &targets {
            if matches!(
                target.hit_kind,
                Some(EnemyPuppetHitKind::Meteor | EnemyPuppetHitKind::Leviathan)
            ) {
                live_bindings.insert(target.target.id.clone());
            }
        }
        // These bindings mutate the same store that owns the spell rows. Collect only
        // the source values needed by the next pass, rather than cloning the store.
        let skulls: Vec<_> = world
            .enemies
            .boss_spells
            .iter()
            .filter(|spell| matches!(spell.kind, BossSpellKind::SkullMissile { .. }))
            .map(|spell| (format!("boss-spell:{}", spell.owner.id), spell.owner.position))
            .collect();
        for (id, position) in skulls {
            live_bindings.insert(id.clone());
            world.enemies.bind_native_query_target(&id, position);
        }
        for projectile in &world.enemies.projectiles {
            let arrow = matches!(projectile, EnemyProjectile::Arrow { .. });
            if !arrow && !matches!(projectile, EnemyProjectile::Firebolt(_)) {
                continue;
            }
            let state = projectile.state();
            targets.push(boneyard_puppet_target(
                format!("projectile:{}", state.id),
                state.position,
                if arrow { 0x80 } else { 0x100 },
                world.enemies.painter_registration_ordinal(state.painter_registration),
                Some(if arrow { EnemyPuppetHitKind::Arrow } else { EnemyPuppetHitKind::Firebolt }),
                PrimarySpellQueryLane::Transient,
            ));
        }
        world.enemies.target_cell_bindings.retain(|id, _| live_bindings.contains(id));
        world.enemies.retain_puppet_hits(&targets);
    }
}
