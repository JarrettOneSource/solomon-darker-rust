//! Component ownership keyed by persistent player IDs. Key order remains the
//! deterministic iteration order; replacing a value retains its ECS entity.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Index, IndexMut};

use bevy_ecs::component::{Component, Mutable};
use bevy_ecs::entity::{Entity, EntityIndexSet};
use bevy_ecs::query::{IterQueryData, QueryItem};
use bevy_ecs::world::{EntityRef, EntityWorldMut, World};

use super::ComponentWorld;

pub(crate) struct KeyedEntities<Q: IterQueryData> {
    storage: Option<Box<ComponentWorld<Q>>>,
    entries: BTreeMap<String, Entity>,
    order: EntityIndexSet,
}

impl<Q: IterQueryData> Default for KeyedEntities<Q> {
    fn default() -> Self {
        Self { storage: None, entries: BTreeMap::new(), order: EntityIndexSet::new() }
    }
}

impl<Q: IterQueryData> KeyedEntities<Q> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &String> + ExactSizeIterator + Clone {
        self.entries.keys()
    }

    fn read(&self, entity: Entity) -> EntityRef<'_> {
        self.storage.as_ref().expect("live entity has component storage").world.entity(entity)
    }

    pub fn get(&self, key: &str) -> Option<EntityRef<'_>> {
        self.entries.get(key).map(|entity| self.read(*entity))
    }

    pub fn get_mut(&mut self, key: &str) -> Option<QueryItem<'_, '_, Q>> {
        let entity = *self.entries.get(key)?;
        let storage = self.storage.as_mut().expect("live entity has component storage");
        Some(
            storage
                .query
                .get_mut(&mut storage.world, entity)
                .expect("entity retains its components"),
        )
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&String, EntityRef<'_>)> + ExactSizeIterator + Clone {
        self.entries.iter().map(|(key, entity)| (key, self.read(*entity)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, QueryItem<'_, '_, Q>)> {
        let entries = &self.entries;
        let order = &self.order;
        self.storage.iter_mut().flat_map(move |storage| {
            entries.keys().zip(
                storage
                    .query
                    .iter_many_unique_mut(&mut storage.world, order.iter())
                    .map(|item| item.expect("entity retains its components")),
            )
        })
    }

    pub fn spawn(&mut self, key: String, insert: impl FnOnce(&mut World) -> Entity) {
        assert!(!self.entries.contains_key(&key), "entity keys are unique");
        let storage = self.storage.get_or_insert_with(|| Box::new(ComponentWorld::new()));
        self.entries.insert(key, insert(&mut storage.world));
        self.order = self.entries.values().copied().collect();
    }

    pub fn remove<T>(
        &mut self,
        key: &str,
        take: impl FnOnce(&mut EntityWorldMut<'_>) -> T,
    ) -> Option<T> {
        let entity = self.entries.remove(key)?;
        self.order.shift_remove(&entity);
        let storage = self.storage.as_mut().expect("live entity has component storage");
        let mut entity = storage.world.entity_mut(entity);
        let value = take(&mut entity);
        entity.despawn();
        Some(value)
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&str, EntityRef<'_>) -> bool) {
        let Some(storage) = &mut self.storage else { return };
        self.entries.retain(|key, entity| {
            let retained = keep(key, storage.world.entity(*entity));
            if !retained {
                storage.world.despawn(*entity);
            }
            retained
        });
        self.order.retain(|entity| storage.world.entities().contains(*entity));
    }
}

/// A keyed population whose state has one cohesive component, such as a player's
/// secondary ability clocks and mana reserve. Multi-component actors use KeyedEntities.
pub struct KeyedComponents<T: Component<Mutability = Mutable>> {
    entities: KeyedEntities<&'static mut T>,
}

impl<T: Component<Mutability = Mutable>> Default for KeyedComponents<T> {
    fn default() -> Self {
        Self { entities: KeyedEntities::default() }
    }
}

impl<T: Component<Mutability = Mutable>> KeyedComponents<T> {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entities.contains_key(key)
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &String> + ExactSizeIterator + Clone {
        self.entities.keys()
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        self.entities
            .get(key)
            .map(|entity| entity.get::<T>().expect("entity retains its component"))
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut T> {
        self.entities.get_mut(key).map(|value| value.into_inner())
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&String, &T)> + ExactSizeIterator + Clone {
        self.entities
            .iter()
            .map(|(key, entity)| (key, entity.get::<T>().expect("entity retains its component")))
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator + Clone {
        self.iter().map(|(_, component)| component)
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.entities.iter_mut().map(|(_, component)| component.into_inner())
    }

    pub fn insert(&mut self, key: String, component: T) -> Option<T> {
        if let Some(current) = self.get_mut(&key) {
            Some(std::mem::replace(current, component))
        } else {
            self.entities.spawn(key, |world| world.spawn(component).id());
            None
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<T> {
        self.entities
            .remove(key, |entity| entity.take::<T>().expect("entity retains its component"))
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&str, &T) -> bool) {
        self.entities.retain(|key, entity| {
            keep(key, entity.get::<T>().expect("entity retains its component"))
        });
    }
}

impl<T: Component<Mutability = Mutable>, K: AsRef<str> + ?Sized> Index<&K> for KeyedComponents<T> {
    type Output = T;
    fn index(&self, key: &K) -> &Self::Output {
        self.get(key.as_ref()).expect("entity key exists")
    }
}

impl<T: Component<Mutability = Mutable>, K: AsRef<str> + ?Sized> IndexMut<&K>
    for KeyedComponents<T>
{
    fn index_mut(&mut self, key: &K) -> &mut Self::Output {
        self.get_mut(key.as_ref()).expect("entity key exists")
    }
}

impl<T: Component<Mutability = Mutable>> FromIterator<(String, T)> for KeyedComponents<T> {
    fn from_iter<I: IntoIterator<Item = (String, T)>>(records: I) -> Self {
        let mut result = Self::default();
        for (key, record) in records {
            result.insert(key, record);
        }
        result
    }
}

impl<T: Component<Mutability = Mutable> + Clone> Clone for KeyedComponents<T> {
    fn clone(&self) -> Self {
        self.iter().map(|(key, value)| (key.clone(), value.clone())).collect()
    }
}

impl<T: Component<Mutability = Mutable> + fmt::Debug> fmt::Debug for KeyedComponents<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<T: Component<Mutability = Mutable> + PartialEq> PartialEq for KeyedComponents<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

#[cfg(feature = "persistence")]
impl<T: Component<Mutability = Mutable> + serde::Serialize> serde::Serialize
    for KeyedComponents<T>
{
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.iter())
    }
}

#[cfg(feature = "persistence")]
impl<'de, T: Component<Mutability = Mutable> + serde::Deserialize<'de>> serde::Deserialize<'de>
    for KeyedComponents<T>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BTreeMap::<String, T>::deserialize(deserializer)
            .map(|records| records.into_iter().collect())
    }
}
