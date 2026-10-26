//! Actual geometry/target callbacks sampled for the source secondary ability epoch.

use crate::boneyard::collision::{
    can_place_boneyard_body, first_boneyard_line_obstruction, resolve_boneyard_movement,
};
use crate::boneyard::projectile_world::{BoneyardProjectileViewport, BoneyardProjectileWorld};
use crate::boneyard::{BoneyardBounds, BoneyardWorld};
use crate::enemies::EnemyToken;
use crate::enemies::boss_spell::BossSpellKind;
use crate::enemies::model::EnemyLifeState;
use crate::enemies::projectiles::EnemyProjectileRead;
use crate::enemies::projectiles::{EnemyProjectileKind, EnemyProjectilePayload};
use crate::enemies::targeting::{boneyard_enemy_actor_flags, boneyard_enemy_collision_radius};
use crate::enemies::world::EnemyProjectileWorldQuery;
use crate::hub::is_traversable_against_segments;
use crate::hub::regions::HubRegion;
use crate::secondary::geometry::*;
use crate::secondary::*;
use crate::spells::boneyard::{
    boneyard_native_secondary_target, boneyard_native_secondary_targets,
};
use crate::{NativeRng, Vec2};

pub(super) struct SessionSecondaryWorld<'a> {
    pub boneyard: Option<&'a mut BoneyardWorld>,
    pub players: Vec<crate::boneyard::world::BoneyardPlayerBody<'a>>,
    pub ether_drain: Option<super::ether_drain::SessionEtherDrainSnapshot>,
    pub regions: std::collections::BTreeMap<String, HubRegion>,
    pub teleport_bodies: Vec<BoneyardNativeTeleportBody>,
    pub viewports: Vec<BoneyardProjectileViewport>,
}

impl SessionSecondaryWorld<'_> {
    pub fn collect_teleport_bodies(
        world: Option<&BoneyardWorld>,
        positions: &[Vec2],
        actors: &crate::secondary::SecondaryActors,
    ) -> Vec<BoneyardNativeTeleportBody> {
        let mut bodies: Vec<_> = positions
            .iter()
            .map(|position| BoneyardNativeTeleportBody { position: *position, radius: 25.0 })
            .collect();
        let Some(world) = world else {
            return bodies;
        };
        for actor in world.enemies.actors.iter() {
            if boneyard_enemy_actor_flags(&actor) != 0 {
                bodies.push(BoneyardNativeTeleportBody {
                    position: actor.position,
                    radius: boneyard_enemy_collision_radius(&actor),
                });
            }
        }
        for maggot in &world.enemies.maggots {
            if maggot.life_state == EnemyLifeState::Alive {
                bodies.push(BoneyardNativeTeleportBody {
                    position: maggot.position,
                    radius: maggot.collision_radius,
                });
            }
        }
        let key = format!("boneyard:{}", world.loaded.run_id);
        for actor in actors.iter() {
            if actor.kind == NativeSecondaryActorKind::Golem && actor.world_key == key {
                bodies.push(BoneyardNativeTeleportBody {
                    position: actor.position,
                    radius: actor.radius,
                });
            }
        }
        bodies
    }

    fn matches_boneyard(&self, key: &str) -> bool {
        self.boneyard
            .as_deref()
            .is_some_and(|world| key == format!("boneyard:{}", world.loaded.run_id))
    }

    fn can_place_for(&self, player_id: &str, position: Vec2, radius: f64) -> bool {
        if let Some(world) = self.boneyard.as_deref() {
            can_place_boneyard_body(
                position,
                world.loaded.scene.bounds,
                &world.dynamic_collision,
                radius,
            )
        } else {
            self.regions.get(player_id).is_some_and(|region| {
                is_traversable_against_segments(position, radius, region.segments())
            })
        }
    }
}

impl NativeSecondaryWorld for SessionSecondaryWorld<'_> {
    fn knockback_area_movement(
        &mut self,
        key: &str,
        id: usize,
        delta: Vec2,
        _source_actor_id: u64,
    ) -> Vec<usize> {
        if !self.matches_boneyard(key) {
            return Vec::new();
        }
        self.boneyard
            .as_deref_mut()
            .expect("matching key has the actual Boneyard")
            .move_knockback_area_target(id, delta, &self.players)
    }
    fn target_exists(&self, key: &str, id: usize) -> bool {
        if !self.matches_boneyard(key) {
            return false;
        }
        let world = self.boneyard.as_deref().expect("matching key has the actual Boneyard");
        world.enemies.actors.iter().any(|actor| actor.id == id)
            || world.enemies.maggots.iter().any(|maggot| maggot.id == id)
    }
    fn ether_drain_targets(
        &self,
        key: &str,
    ) -> Option<Vec<crate::secondary::ether_drain::NativeEtherDrainTarget>> {
        if !self.matches_boneyard(key) {
            return Some(Vec::new());
        }
        Some(
            self.ether_drain
                .as_ref()
                .expect("normal secondary epoch captures actual field targets")
                .targets
                .clone(),
        )
    }
    fn ether_drain_world_animations(
        &self,
        key: &str,
    ) -> Vec<crate::secondary::ether_drain::NativeEtherDrainWorldAnimationTarget> {
        if !self.matches_boneyard(key) {
            return Vec::new();
        }
        self.ether_drain
            .as_ref()
            .expect("normal secondary epoch captures actual world animations")
            .animations
            .clone()
    }
    fn dampen_candidates(&self, key: &str, origin: Vec2) -> NativeSecondaryDampenCandidates {
        if !self.matches_boneyard(key) {
            return NativeSecondaryDampenCandidates::default();
        }
        let world = self.boneyard.as_deref().unwrap();
        let inside = |position: Vec2| {
            (position.x - origin.x).abs() <= 400.0 && (position.y - origin.y).abs() <= 400.0
        };
        let mut result = NativeSecondaryDampenCandidates::default();
        for actor in world.enemies.actors.iter() {
            if actor.life_state != EnemyLifeState::Alive || !inside(actor.position) {
                continue;
            }
            if matches!(
                actor.config.enemy_token,
                EnemyToken::SkeletonMage | EnemyToken::DireFaculty
            ) {
                result.caster_target_ids.push(actor.id);
            }
            if actor.shield_health > 0.0 {
                result.shield_target_ids.push(actor.id);
            }
        }
        result.caster_target_ids.sort_unstable();
        result.shield_target_ids.sort_unstable();
        let mut projectiles: Vec<_> = world
            .enemies
            .projectiles
            .iter()
            .filter(|projectile| {
                let row = projectile.state();
                inside(row.position)
                    && ((row.native_type_id == 0x7eb
                        && projectile.kind() == EnemyProjectileKind::Firebolt
                        && row.payload == EnemyProjectilePayload::Fire)
                        || (row.native_type_id == 0x7ec
                            && projectile.kind() == EnemyProjectileKind::GuidedMissile
                            && matches!(
                                row.payload,
                                EnemyProjectilePayload::Cold | EnemyProjectilePayload::Poison
                            )))
            })
            .collect();
        projectiles.sort_by_key(|projectile| {
            (projectile.state().native_registration_order, projectile.state().id)
        });
        for projectile in projectiles {
            let row = projectile.state();
            result.projectiles.push(NativeSecondaryDampenProjectileCandidate {
                id: row.id,
                position: row.position,
                kind: if row.native_type_id == 0x7eb {
                    NativeSecondaryDampenProjectileKind::Firebolt
                } else {
                    NativeSecondaryDampenProjectileKind::GuidedMissile
                },
                payload: if row.native_type_id == 0x7eb {
                    NativeSecondaryDampenPayload::Fire
                } else if row.payload == EnemyProjectilePayload::Poison {
                    NativeSecondaryDampenPayload::Poison
                } else {
                    NativeSecondaryDampenPayload::Cold
                },
            });
        }
        for spell in &world.enemies.boss_spells {
            if !inside(spell.owner.position) {
                continue;
            }
            let kind = match spell.kind {
                BossSpellKind::SkullMissile { .. } => {
                    NativeSecondaryDampenProjectileKind::SkullMissile
                }
                BossSpellKind::DarkFireball { .. } => {
                    NativeSecondaryDampenProjectileKind::DarkFireball
                }
                _ => continue,
            };
            result.projectiles.push(NativeSecondaryDampenProjectileCandidate {
                id: spell.owner.id,
                position: spell.owner.position,
                kind,
                payload: NativeSecondaryDampenPayload::Dark,
            });
        }
        result
    }
    fn phasing_destination(&self, player_id: &str, origin: Vec2, direction: Vec2) -> Option<Vec2> {
        resolve_native_phasing_destination(origin, direction, |position| {
            self.can_place_for(player_id, position, 25.0)
        })
    }
    fn golem_movement(
        &self,
        player_id: &str,
        key: &str,
        origin: Vec2,
        requested: Vec2,
        radius: f64,
    ) -> Vec2 {
        if let Some(world) = self.boneyard.as_deref() {
            if !self.matches_boneyard(key) {
                return origin;
            }
            resolve_boneyard_movement(
                origin,
                requested,
                world.loaded.scene.bounds,
                &world.dynamic_collision,
                radius,
            )
        } else if self.can_place_for(player_id, requested, radius) {
            requested
        } else {
            origin
        }
    }
    fn golem_placement(
        &self,
        player_id: &str,
        key: &str,
        requested: Vec2,
        rng: &mut NativeRng,
    ) -> Vec2 {
        if self.boneyard.is_some() && !self.matches_boneyard(key) {
            return requested;
        }
        if self.boneyard.is_none()
            && self
                .regions
                .get(player_id)
                .is_none_or(|region| key != format!("hub:{}", region.id()))
        {
            return requested;
        }
        resolve_native_collision_adjusted_position(
            requested,
            NATIVE_GOLEM_PLACEMENT_RADIUS,
            rng,
            |position| self.can_place_for(player_id, position, NATIVE_GOLEM_PLACEMENT_RADIUS),
        )
    }
    fn teleport_destination(&self, _player_id: &str, rng: &mut NativeRng) -> Vec2 {
        let Some(world) = self.boneyard.as_deref() else {
            return Vec2::ZERO;
        };
        resolve_boneyard_native_teleport(
            BoneyardNativeTeleportWorld {
                bodies: &self.teleport_bodies,
                bounds: world.loaded.scene.bounds,
                collision: &world.dynamic_collision,
            },
            rng,
        )
    }
    fn target(&self, key: &str, id: usize) -> Option<NativeSecondaryTarget> {
        if !self.matches_boneyard(key) {
            return None;
        }
        boneyard_native_secondary_target(&self.boneyard.as_deref().unwrap().enemies, id)
    }
    fn targets(&self, key: &str, center: Vec2, radius: f64) -> Vec<NativeSecondaryTarget> {
        if !self.matches_boneyard(key) {
            return Vec::new();
        }
        boneyard_native_secondary_targets(
            &self.boneyard.as_deref().unwrap().enemies,
            center,
            radius,
        )
    }
    fn effect_visible(&self, key: &str, position: Vec2, margin: f64) -> bool {
        // An immediate Inventory cast supplies no projectile-view sampler in Source.
        if self.viewports.is_empty() {
            return false;
        }
        if !self.matches_boneyard(key) {
            return false;
        }
        let world = self.boneyard.as_deref().unwrap();
        let query = BoneyardProjectileWorld::new(
            world.loaded.scene.bounds,
            &world.dynamic_collision,
            &self.viewports,
        );
        !query.blocked(EnemyProjectileWorldQuery::View { projectile_id: 0, position, margin })
    }
    fn golem_foot_placement(
        &self,
        player_id: &str,
        key: &str,
        current: Vec2,
        requested: Vec2,
    ) -> Vec2 {
        self.golem_movement(player_id, key, current, requested, 0.0)
    }
    fn effect_position_blocked(&self, key: &str, position: Vec2) -> bool {
        if let Some(world) = self.boneyard.as_deref() {
            return self.matches_boneyard(key)
                && !can_place_boneyard_body(
                    position,
                    world.loaded.scene.bounds,
                    &world.dynamic_collision,
                    0.0,
                );
        }
        let region = self.regions.values().find(|region| key == format!("hub:{}", region.id()));
        region
            .is_none_or(|region| !is_traversable_against_segments(position, 0.0, region.segments()))
    }
    fn line_obstruction(&self, key: &str, start: Vec2, end: Vec2) -> bool {
        if !self.matches_boneyard(key) {
            return false;
        }
        let world = self.boneyard.as_deref().unwrap();
        first_boneyard_line_obstruction(
            start,
            end,
            world.loaded.scene.bounds,
            &world.dynamic_collision,
            None,
            0,
            &[],
        )
        .is_some()
    }
    fn scenery_targets(
        &self,
        key: &str,
        center: Vec2,
        radius: f64,
    ) -> Vec<NativeSecondarySceneryTarget> {
        if !self.matches_boneyard(key) {
            return Vec::new();
        }
        let world = self.boneyard.as_deref().unwrap();
        let mut targets = Vec::new();
        for (id, object) in world.loaded.scene.objects.iter().enumerate() {
            if !world.retains_earthquake_scenery_target(id) {
                continue;
            }
            if world.cleanup_bounds().is_some_and(|bounds| !contains(bounds, object.position)) {
                continue;
            }
            let x = object.position.x - center.x;
            let y = object.position.y - center.y;
            if x * x + y * y < radius * radius {
                targets.push(NativeSecondarySceneryTarget {
                    id,
                    position: object.position,
                    type_id: object.type_id,
                    secondary_variant: world.earthquake_scenery_variant(id),
                });
            }
        }
        targets
    }
}

fn contains(bounds: BoneyardBounds, position: Vec2) -> bool {
    position.x >= bounds.x
        && position.y >= bounds.y
        && position.x <= bounds.x + bounds.width
        && position.y <= bounds.y + bounds.height
}
