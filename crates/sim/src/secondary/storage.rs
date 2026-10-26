//! Secondary entity ownership and borrowed component views.
//!
//! Admission order is independent of ECS table order: removing an effect must
//! not change which surviving actor consumes the next random number. Saved IDs
//! and painter registrations remain game identities; ECS handles stay private.

use std::fmt;
use std::ops::{Deref, DerefMut};

use bevy_ecs::component::Component;
use bevy_ecs::query::QueryItem;
use bevy_ecs::world::{EntityRef, EntityWorldMut};

use crate::ecs::OrderedEntities;

use super::ether_drain::NativeEtherDrainState;
use super::golem::NativeSecondaryGolemState;
use super::{NativeSecondaryActorKind, NativeSecondaryActorState, SecondaryActor};
use crate::death_animations::NativeGolemDeathAnimation;

/// Shared access for live components, recorded inputs, and presentation plans.
pub trait SecondaryActorRead: Deref<Target = SecondaryActor> {
    fn golem(&self) -> Option<&NativeSecondaryGolemState>;
    fn golem_death(&self) -> Option<&NativeGolemDeathAnimation>;
    fn ether_drain(&self) -> Option<&NativeEtherDrainState>;

    fn view(&self) -> SecondaryActorRef<'_> {
        SecondaryActorRef {
            common: self.deref(),
            golem: self.golem(),
            golem_death: self.golem_death(),
            ether_drain: self.ether_drain(),
        }
    }

    fn snapshot(&self) -> NativeSecondaryActorState {
        NativeSecondaryActorState {
            common: self.deref().clone(),
            golem: self.golem().copied(),
            golem_death: self.golem_death().cloned(),
            ether_drain: self.ether_drain().cloned(),
        }
    }
}

pub trait SecondaryActorWrite: SecondaryActorRead + DerefMut {
    fn golem_mut(&mut self) -> Option<&mut NativeSecondaryGolemState>;
    fn golem_death_mut(&mut self) -> Option<&mut NativeGolemDeathAnimation>;
    fn ether_drain_mut(&mut self) -> &mut Option<NativeEtherDrainState>;
    fn set_ether_drain(&mut self, field: NativeEtherDrainState) {
        *self.ether_drain_mut() = Some(field);
    }
}

impl Deref for NativeSecondaryActorState {
    type Target = SecondaryActor;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

impl DerefMut for NativeSecondaryActorState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.common
    }
}

impl SecondaryActorRead for NativeSecondaryActorState {
    fn golem(&self) -> Option<&NativeSecondaryGolemState> {
        self.golem.as_ref()
    }
    fn golem_death(&self) -> Option<&NativeGolemDeathAnimation> {
        self.golem_death.as_ref()
    }
    fn ether_drain(&self) -> Option<&NativeEtherDrainState> {
        self.ether_drain.as_ref()
    }
}

impl SecondaryActorWrite for NativeSecondaryActorState {
    fn golem_mut(&mut self) -> Option<&mut NativeSecondaryGolemState> {
        self.golem.as_mut()
    }
    fn golem_death_mut(&mut self) -> Option<&mut NativeGolemDeathAnimation> {
        self.golem_death.as_mut()
    }
    fn ether_drain_mut(&mut self) -> &mut Option<NativeEtherDrainState> {
        &mut self.ether_drain
    }
}

impl NativeSecondaryActorState {
    pub fn view_mut(&mut self) -> SecondaryActorMut<'_> {
        SecondaryActorMut {
            common: &mut self.common,
            golem: self.golem.as_mut(),
            golem_death: self.golem_death.as_mut(),
            ether_drain: Some(&mut self.ether_drain),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondaryActorRef<'a> {
    pub common: &'a SecondaryActor,
    golem: Option<&'a NativeSecondaryGolemState>,
    golem_death: Option<&'a NativeGolemDeathAnimation>,
    ether_drain: Option<&'a NativeEtherDrainState>,
}

impl Deref for SecondaryActorRef<'_> {
    type Target = SecondaryActor;
    fn deref(&self) -> &Self::Target {
        self.common
    }
}

impl SecondaryActorRead for SecondaryActorRef<'_> {
    fn golem(&self) -> Option<&NativeSecondaryGolemState> {
        self.golem
    }
    fn golem_death(&self) -> Option<&NativeGolemDeathAnimation> {
        self.golem_death
    }
    fn ether_drain(&self) -> Option<&NativeEtherDrainState> {
        self.ether_drain
    }
}

/// Mutable component references; structural changes belong to the actor store.
pub struct SecondaryActorMut<'a> {
    pub common: &'a mut SecondaryActor,
    pub golem: Option<&'a mut NativeSecondaryGolemState>,
    golem_death: Option<&'a mut NativeGolemDeathAnimation>,
    ether_drain: Option<&'a mut Option<NativeEtherDrainState>>,
}

impl Deref for SecondaryActorMut<'_> {
    type Target = SecondaryActor;
    fn deref(&self) -> &Self::Target {
        self.common
    }
}

impl DerefMut for SecondaryActorMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.common
    }
}

impl SecondaryActorRead for SecondaryActorMut<'_> {
    fn golem(&self) -> Option<&NativeSecondaryGolemState> {
        self.golem.as_deref()
    }
    fn golem_death(&self) -> Option<&NativeGolemDeathAnimation> {
        self.golem_death.as_deref()
    }
    fn ether_drain(&self) -> Option<&NativeEtherDrainState> {
        self.ether_drain.as_deref().and_then(|field| field.as_ref())
    }
}

impl SecondaryActorWrite for SecondaryActorMut<'_> {
    fn golem_mut(&mut self) -> Option<&mut NativeSecondaryGolemState> {
        self.golem.as_deref_mut()
    }
    fn golem_death_mut(&mut self) -> Option<&mut NativeGolemDeathAnimation> {
        self.golem_death.as_deref_mut()
    }
    fn ether_drain_mut(&mut self) -> &mut Option<NativeEtherDrainState> {
        self.ether_drain.as_deref_mut().expect("drain actor owns its field component")
    }
}

// Ether Drain discovers its targets on its first tick. The component exists at
// birth, but its uninitialized state must still serialize as null.
#[derive(Component)]
struct DrainField(Option<NativeEtherDrainState>);

type ActorQuery = (
    &'static mut SecondaryActor,
    Option<&'static mut NativeSecondaryGolemState>,
    Option<&'static mut NativeGolemDeathAnimation>,
    Option<&'static mut DrainField>,
);

impl<'a> SecondaryActorMut<'a> {
    fn from_query(
        (actor, golem, golem_death, ether_drain): QueryItem<'a, 'static, ActorQuery>,
    ) -> Self {
        Self {
            common: actor.into_inner(),
            golem: golem.map(|value| value.into_inner()),
            golem_death: golem_death.map(|value| value.into_inner()),
            ether_drain: ether_drain.map(|value| &mut value.into_inner().0),
        }
    }
}

#[derive(Default)]
pub struct SecondaryActors {
    entities: OrderedEntities<ActorQuery>,
}

impl SecondaryActors {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = SecondaryActorRef<'_>> + ExactSizeIterator + Clone {
        self.entities.iter().map(Self::read)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = SecondaryActorMut<'_>> {
        self.entities.iter_mut().map(SecondaryActorMut::from_query)
    }

    fn read(entity: EntityRef<'_>) -> SecondaryActorRef<'_> {
        SecondaryActorRef {
            common: entity.get::<SecondaryActor>().expect("secondary entity owns its common state"),
            golem: entity.get::<NativeSecondaryGolemState>(),
            golem_death: entity.get::<NativeGolemDeathAnimation>(),
            ether_drain: entity.get::<DrainField>().and_then(|field| field.0.as_ref()),
        }
    }

    pub fn get(&self, index: usize) -> Option<SecondaryActorRef<'_>> {
        self.entities.get(index).map(Self::read)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<SecondaryActorMut<'_>> {
        self.entities.get_mut(index).map(SecondaryActorMut::from_query)
    }

    pub fn last(&self) -> Option<SecondaryActorRef<'_>> {
        self.iter().next_back()
    }

    pub fn push(&mut self, value: NativeSecondaryActorState) {
        self.entities.spawn(|world| {
            let drain = value.common.kind == NativeSecondaryActorKind::EtherDrain
                || value.ether_drain.is_some();
            let mut entity = world.spawn(value.common);
            if let Some(golem) = value.golem {
                entity.insert(golem);
            }
            if let Some(death) = value.golem_death {
                entity.insert(death);
            }
            if drain {
                entity.insert(DrainField(value.ether_drain));
            }
            entity.id()
        });
    }

    pub fn remove(&mut self, index: usize) -> NativeSecondaryActorState {
        self.entities.remove(index, Self::take)
    }

    fn take(entity: &mut EntityWorldMut<'_>) -> NativeSecondaryActorState {
        NativeSecondaryActorState {
            common: entity
                .take::<SecondaryActor>()
                .expect("secondary entity owns its common state"),
            golem: entity.take::<NativeSecondaryGolemState>(),
            golem_death: entity.take::<NativeGolemDeathAnimation>(),
            ether_drain: entity.take::<DrainField>().and_then(|field| field.0),
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&SecondaryActorRef<'_>) -> bool) {
        self.entities.retain(|entity| keep(&Self::read(entity)));
    }

    pub fn retain_mut(&mut self, mut keep: impl FnMut(&mut SecondaryActorMut<'_>) -> bool) {
        self.entities.retain_mut(|components| keep(&mut SecondaryActorMut::from_query(components)));
    }

    pub fn extract_if(
        &mut self,
        mut extract: impl FnMut(&SecondaryActorRef<'_>) -> bool,
    ) -> Vec<NativeSecondaryActorState> {
        self.entities.extract_if(|entity| extract(&Self::read(entity)), Self::take)
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }

    pub fn sort_by_key<K: Ord>(&mut self, mut key: impl FnMut(&SecondaryActorRef<'_>) -> K) {
        self.entities.sort_by_key(|entity| key(&Self::read(entity)));
    }
}

impl Extend<NativeSecondaryActorState> for SecondaryActors {
    fn extend<I: IntoIterator<Item = NativeSecondaryActorState>>(&mut self, values: I) {
        for value in values {
            self.push(value);
        }
    }
}

impl FromIterator<NativeSecondaryActorState> for SecondaryActors {
    fn from_iter<I: IntoIterator<Item = NativeSecondaryActorState>>(values: I) -> Self {
        let mut actors = Self::default();
        actors.extend(values);
        actors
    }
}

impl IntoIterator for SecondaryActors {
    type Item = NativeSecondaryActorState;
    type IntoIter = std::vec::IntoIter<NativeSecondaryActorState>;
    fn into_iter(self) -> Self::IntoIter {
        self.entities.into_records(Self::take).into_iter()
    }
}

impl Clone for SecondaryActors {
    fn clone(&self) -> Self {
        self.iter().map(|actor| actor.snapshot()).collect()
    }
}

impl fmt::Debug for SecondaryActors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for SecondaryActors {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for SecondaryActors {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for SecondaryActors {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<NativeSecondaryActorState>::deserialize(deserializer)
            .map(|actors| actors.into_iter().collect())
    }
}

#[cfg(feature = "persistence")]
fn serialize_actor<S: serde::Serializer>(
    actor: &impl SecondaryActorRead,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut row = serializer.serialize_struct("NativeSecondaryActorState", 33)?;
    row.serialize_field("age_ticks", &actor.age_ticks)?;
    row.serialize_field("alpha", &actor.alpha)?;
    row.serialize_field("damage", &actor.damage)?;
    row.serialize_field("enhanced", &actor.enhanced)?;
    row.serialize_field("endpoint", &actor.endpoint)?;
    row.serialize_field("ether_drain", &actor.ether_drain())?;
    row.serialize_field("frame", &actor.frame)?;
    row.serialize_field("freeze_ticks", &actor.freeze_ticks)?;
    row.serialize_field("golem", &actor.golem())?;
    row.serialize_field("golem_death", &actor.golem_death())?;
    row.serialize_field("hit_target_ids", &actor.hit_target_ids)?;
    row.serialize_field("id", &actor.id)?;
    row.serialize_field("kind", &actor.kind)?;
    row.serialize_field("lifetime_ticks", &actor.lifetime_ticks)?;
    row.serialize_field("light_registration", &actor.light_registration)?;
    row.serialize_field("midpoint", &actor.midpoint)?;
    row.serialize_field("misc_light_append_ordinal", &actor.misc_light_append_ordinal)?;
    row.serialize_field("owner_id", &actor.owner_id)?;
    row.serialize_field("painter_registrations", &actor.painter_registrations)?;
    row.serialize_field("phase", &actor.phase)?;
    row.serialize_field("position", &actor.position)?;
    row.serialize_field("presentation_rng", &actor.presentation_rng)?;
    row.serialize_field("quantity", &actor.quantity)?;
    row.serialize_field("radius", &actor.radius)?;
    row.serialize_field("rank", &actor.rank)?;
    row.serialize_field("rotation_radians", &actor.rotation_radians)?;
    row.serialize_field("scale", &actor.scale)?;
    row.serialize_field("skill_id", &actor.skill_id)?;
    row.serialize_field("slow_factor", &actor.slow_factor)?;
    row.serialize_field("target_id", &actor.target_id)?;
    row.serialize_field("variant", &actor.variant)?;
    row.serialize_field("velocity", &actor.velocity)?;
    row.serialize_field("world_key", &actor.world_key)?;
    row.end()
}

#[cfg(feature = "persistence")]
impl serde::Serialize for NativeSecondaryActorState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_actor(self, serializer)
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for SecondaryActorRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_actor(self, serializer)
    }
}
