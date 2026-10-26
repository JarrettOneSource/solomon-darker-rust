//! Entity ownership for domains whose admission order is observable gameplay.
//!
//! A query supplies each domain's components. The order contains only ECS handles;
//! game IDs and checkpoint fields remain with the domain. Removing an entity never
//! reorders survivors, even when Bevy moves its table row.

mod components;
pub use components::{ComponentEntities, ComponentIter};

use bevy_ecs::entity::{Entity, EntityHashSet, EntityIndexSet};
use bevy_ecs::query::{IterQueryData, QueryItem, QueryState};
use bevy_ecs::world::{EntityRef, EntityWorldMut, World};

pub(crate) struct OrderedEntities<Q: IterQueryData> {
    // Sessions embed several domains; their World/query allocations belong on the heap.
    storage: Option<Box<ComponentWorld<Q>>>,
    order: EntityIndexSet,
}

struct ComponentWorld<Q: IterQueryData> {
    world: World,
    query: QueryState<Q>,
}

impl<Q: IterQueryData> Default for OrderedEntities<Q> {
    fn default() -> Self {
        // Empty domains and temporary birth collectors do not need a World.
        Self { storage: None, order: EntityIndexSet::new() }
    }
}

impl<Q: IterQueryData> OrderedEntities<Q> {
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = EntityRef<'_>> + ExactSizeIterator + Clone {
        self.order.iter().map(|entity| self.read(*entity))
    }

    fn read(&self, entity: Entity) -> EntityRef<'_> {
        self.storage.as_ref().expect("live entity has component storage").world.entity(entity)
    }

    pub fn get(&self, index: usize) -> Option<EntityRef<'_>> {
        self.order.get_index(index).map(|entity| self.read(*entity))
    }

    pub fn get_mut(&mut self, index: usize) -> Option<QueryItem<'_, '_, Q>> {
        let entity = *self.order.get_index(index)?;
        let storage = self.storage.as_mut().expect("live entity has component storage");
        Some(
            storage
                .query
                .get_mut(&mut storage.world, entity)
                .expect("entity retains its components"),
        )
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = QueryItem<'_, '_, Q>> {
        self.storage.iter_mut().flat_map(|storage| {
            storage
                .query
                .iter_many_unique_mut(&mut storage.world, self.order.iter())
                .map(|item| item.expect("entity retains its components"))
        })
    }

    /// The domain inserts its complete component set before exposing the entity.
    pub fn spawn(&mut self, insert: impl FnOnce(&mut World) -> Entity) {
        let storage = self.storage.get_or_insert_with(|| {
            let mut world = World::new();
            let query = world.query::<Q>();
            Box::new(ComponentWorld { world, query })
        });
        self.order.insert(insert(&mut storage.world));
    }

    pub fn remove<T>(
        &mut self,
        index: usize,
        take: impl FnOnce(&mut EntityWorldMut<'_>) -> T,
    ) -> T {
        let entity = self.order.shift_remove_index(index).expect("entity index is in bounds");
        let storage = self.storage.as_mut().expect("live entity has component storage");
        let mut entity = storage.world.entity_mut(entity);
        let value = take(&mut entity);
        entity.despawn();
        value
    }

    pub fn retain(&mut self, mut keep: impl FnMut(EntityRef<'_>) -> bool) {
        let retired: EntityHashSet =
            self.order.iter().copied().filter(|entity| !keep(self.read(*entity))).collect();
        if let Some(storage) = &mut self.storage {
            for entity in &retired {
                storage.world.despawn(*entity);
            }
        }
        self.order.retain(|entity| !retired.contains(entity));
    }

    pub fn retain_mut(&mut self, mut keep: impl FnMut(QueryItem<'_, '_, Q>) -> bool) {
        let Some(storage) = &mut self.storage else { return };
        let mut retired = EntityHashSet::default();
        let actors = storage.query.iter_many_unique_mut(&mut storage.world, self.order.iter());
        for (entity, item) in self.order.iter().copied().zip(actors) {
            if !keep(item.expect("entity retains its components")) {
                retired.insert(entity);
            }
        }
        for entity in &retired {
            storage.world.despawn(*entity);
        }
        self.order.retain(|entity| !retired.contains(entity));
    }

    /// Update the original population once. Children enter the order immediately
    /// before their retained parent, but receive their first update on the next pass.
    pub fn advance_with_births<T>(
        &mut self,
        mut step: impl FnMut(QueryItem<'_, '_, Q>) -> (bool, Vec<T>),
        mut spawn: impl FnMut(&mut World, T) -> Entity,
    ) {
        let Some(storage) = &mut self.storage else { return };
        let population = std::mem::take(&mut self.order);
        self.order.reserve(population.len());
        for entity in population {
            let components = storage
                .query
                .get_mut(&mut storage.world, entity)
                .expect("entity retains its components");
            let (retained, births) = step(components);
            for birth in births {
                self.order.insert(spawn(&mut storage.world, birth));
            }
            if retained {
                self.order.insert(entity);
            } else {
                storage.world.despawn(entity);
            }
        }
    }

    pub fn extract_if<T>(
        &mut self,
        mut extract: impl FnMut(EntityRef<'_>) -> bool,
        mut take: impl FnMut(&mut EntityWorldMut<'_>) -> T,
    ) -> Vec<T> {
        let removed: EntityIndexSet =
            self.order.iter().copied().filter(|entity| extract(self.read(*entity))).collect();
        self.order.retain(|entity| !removed.contains(entity));
        removed
            .into_iter()
            .map(|entity| {
                let storage = self.storage.as_mut().expect("live entity has component storage");
                let mut entity = storage.world.entity_mut(entity);
                let value = take(&mut entity);
                entity.despawn();
                value
            })
            .collect()
    }

    pub fn clear(&mut self) {
        if let Some(storage) = &mut self.storage {
            storage.world.clear_entities();
        }
        self.order.clear();
    }

    pub fn sort_by_key<K: Ord>(&mut self, mut key: impl FnMut(EntityRef<'_>) -> K) {
        let mut ordered: Vec<_> =
            self.order.iter().map(|entity| (key(self.read(*entity)), *entity)).collect();
        ordered.sort_by(|left, right| left.0.cmp(&right.0));
        self.order = ordered.into_iter().map(|(_, entity)| entity).collect();
    }

    pub fn into_records<T>(mut self, mut take: impl FnMut(&mut EntityWorldMut<'_>) -> T) -> Vec<T> {
        self.order
            .into_iter()
            .map(|entity| {
                let storage = self.storage.as_mut().expect("live entity has component storage");
                take(&mut storage.world.entity_mut(entity))
            })
            .collect()
    }
}
