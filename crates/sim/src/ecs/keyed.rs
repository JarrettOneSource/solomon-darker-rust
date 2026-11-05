//! Component ownership keyed by persistent player IDs. Key order remains the
//! deterministic iteration order; replacing a value retains its ECS entity.

use std::collections::BTreeMap;

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
}
