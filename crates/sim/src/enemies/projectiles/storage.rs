//! Projectile flight and collision state share an entity with its motion program.
//! The owned enum is the construction/save format; ticking borrows the components.

use std::fmt;

use bevy_ecs::component::Component;
use bevy_ecs::query::QueryItem;
use bevy_ecs::world::{EntityRef, EntityWorldMut};

use super::{EnemyProjectile, EnemyProjectileKind, EnemyProjectileState};
use crate::Vec2;
use crate::ecs::OrderedEntities;

#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub enum EnemyProjectileMotion {
    Arrow { velocity: Vec2 },
    DemonBomb,
    Firebolt,
    GuidedMissile,
    PoisonPool,
}

impl EnemyProjectileMotion {
    pub const fn kind(self) -> EnemyProjectileKind {
        match self {
            Self::Arrow { .. } => EnemyProjectileKind::Arrow,
            Self::DemonBomb => EnemyProjectileKind::DemonBomb,
            Self::Firebolt => EnemyProjectileKind::Firebolt,
            Self::GuidedMissile => EnemyProjectileKind::GuidedMissile,
            Self::PoisonPool => EnemyProjectileKind::PoisonPool,
        }
    }

    fn into_record(self, state: EnemyProjectileState) -> EnemyProjectile {
        match self {
            Self::Arrow { velocity } => EnemyProjectile::Arrow { state, velocity },
            Self::DemonBomb => EnemyProjectile::DemonBomb(state),
            Self::Firebolt => EnemyProjectile::Firebolt(state),
            Self::GuidedMissile => EnemyProjectile::GuidedMissile(state),
            Self::PoisonPool => EnemyProjectile::PoisonPool(state),
        }
    }
}

pub trait EnemyProjectileRead {
    fn state(&self) -> &EnemyProjectileState;
    fn motion(&self) -> EnemyProjectileMotion;

    fn kind(&self) -> EnemyProjectileKind {
        self.motion().kind()
    }

    fn snapshot(&self) -> EnemyProjectile {
        self.motion().into_record(self.state().clone())
    }
}

impl EnemyProjectileRead for EnemyProjectile {
    fn state(&self) -> &EnemyProjectileState {
        self.state()
    }

    fn motion(&self) -> EnemyProjectileMotion {
        match self {
            Self::Arrow { velocity, .. } => EnemyProjectileMotion::Arrow { velocity: *velocity },
            Self::DemonBomb(_) => EnemyProjectileMotion::DemonBomb,
            Self::Firebolt(_) => EnemyProjectileMotion::Firebolt,
            Self::GuidedMissile(_) => EnemyProjectileMotion::GuidedMissile,
            Self::PoisonPool(_) => EnemyProjectileMotion::PoisonPool,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyProjectileRef<'a> {
    pub state: &'a EnemyProjectileState,
    pub motion: EnemyProjectileMotion,
}

impl EnemyProjectileRead for EnemyProjectileRef<'_> {
    fn state(&self) -> &EnemyProjectileState {
        self.state
    }
    fn motion(&self) -> EnemyProjectileMotion {
        self.motion
    }
}

pub struct EnemyProjectileMut<'a> {
    pub state: &'a mut EnemyProjectileState,
    pub motion: &'a mut EnemyProjectileMotion,
}

impl EnemyProjectileRead for EnemyProjectileMut<'_> {
    fn state(&self) -> &EnemyProjectileState {
        self.state
    }
    fn motion(&self) -> EnemyProjectileMotion {
        *self.motion
    }
}

type ProjectileQuery = (&'static mut EnemyProjectileState, &'static mut EnemyProjectileMotion);

impl<'a> EnemyProjectileMut<'a> {
    fn from_query((state, motion): QueryItem<'a, 'static, ProjectileQuery>) -> Self {
        Self { state: state.into_inner(), motion: motion.into_inner() }
    }

    pub fn state_mut(&mut self) -> &mut EnemyProjectileState {
        self.state
    }
}

#[derive(Default)]
pub struct EnemyProjectiles {
    entities: OrderedEntities<ProjectileQuery>,
}

impl EnemyProjectiles {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = EnemyProjectileRef<'_>> + ExactSizeIterator + Clone {
        self.entities.iter().map(Self::read)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = EnemyProjectileMut<'_>> {
        self.entities.iter_mut().map(EnemyProjectileMut::from_query)
    }

    fn read(entity: EntityRef<'_>) -> EnemyProjectileRef<'_> {
        EnemyProjectileRef {
            state: entity.get::<EnemyProjectileState>().expect("projectile owns its flight state"),
            motion: *entity.get::<EnemyProjectileMotion>().expect("projectile owns its motion"),
        }
    }

    pub fn get(&self, index: usize) -> Option<EnemyProjectileRef<'_>> {
        self.entities.get(index).map(Self::read)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<EnemyProjectileMut<'_>> {
        self.entities.get_mut(index).map(EnemyProjectileMut::from_query)
    }

    pub fn push(&mut self, projectile: EnemyProjectile) {
        let motion = projectile.motion();
        let state = match projectile {
            EnemyProjectile::Arrow { state, .. }
            | EnemyProjectile::DemonBomb(state)
            | EnemyProjectile::Firebolt(state)
            | EnemyProjectile::GuidedMissile(state)
            | EnemyProjectile::PoisonPool(state) => state,
        };
        self.entities.spawn(|world| world.spawn((state, motion)).id());
    }

    fn take(entity: &mut EntityWorldMut<'_>) -> EnemyProjectile {
        let state =
            entity.take::<EnemyProjectileState>().expect("projectile owns its flight state");
        let motion = entity.take::<EnemyProjectileMotion>().expect("projectile owns its motion");
        motion.into_record(state)
    }

    pub fn remove(&mut self, index: usize) -> EnemyProjectile {
        self.entities.remove(index, Self::take)
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&EnemyProjectileRef<'_>) -> bool) {
        self.entities.retain(|entity| keep(&Self::read(entity)));
    }

    pub fn retain_mut(&mut self, mut keep: impl FnMut(&mut EnemyProjectileMut<'_>) -> bool) {
        self.entities
            .retain_mut(|components| keep(&mut EnemyProjectileMut::from_query(components)));
    }
}

impl FromIterator<EnemyProjectile> for EnemyProjectiles {
    fn from_iter<I: IntoIterator<Item = EnemyProjectile>>(projectiles: I) -> Self {
        let mut result = Self::default();
        for projectile in projectiles {
            result.push(projectile);
        }
        result
    }
}

impl Clone for EnemyProjectiles {
    fn clone(&self) -> Self {
        self.iter().map(|projectile| projectile.snapshot()).collect()
    }
}

impl fmt::Debug for EnemyProjectiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for EnemyProjectiles {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for EnemyProjectileRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStructVariant;
        match self.motion {
            EnemyProjectileMotion::Arrow { velocity } => {
                let mut row =
                    serializer.serialize_struct_variant("EnemyProjectile", 0, "Arrow", 2)?;
                row.serialize_field("state", self.state)?;
                row.serialize_field("velocity", &velocity)?;
                row.end()
            }
            EnemyProjectileMotion::DemonBomb => {
                serializer.serialize_newtype_variant("EnemyProjectile", 1, "DemonBomb", self.state)
            }
            EnemyProjectileMotion::Firebolt => {
                serializer.serialize_newtype_variant("EnemyProjectile", 2, "Firebolt", self.state)
            }
            EnemyProjectileMotion::GuidedMissile => serializer.serialize_newtype_variant(
                "EnemyProjectile",
                3,
                "GuidedMissile",
                self.state,
            ),
            EnemyProjectileMotion::PoisonPool => {
                serializer.serialize_newtype_variant("EnemyProjectile", 4, "PoisonPool", self.state)
            }
        }
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for EnemyProjectiles {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for EnemyProjectiles {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<EnemyProjectile>::deserialize(deserializer)
            .map(|projectiles| projectiles.into_iter().collect())
    }
}
