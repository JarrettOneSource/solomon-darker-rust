//! Ordered ownership for entities described by one cohesive component, such as
//! fountain particles or scenery interaction state.

use std::fmt;
use std::ops::{Index, IndexMut};

use bevy_ecs::component::{Component, Mutable};

use super::OrderedEntities;

pub struct ComponentEntities<T: Component<Mutability = Mutable>> {
    entities: OrderedEntities<&'static mut T>,
}

impl<T: Component<Mutability = Mutable>> Default for ComponentEntities<T> {
    fn default() -> Self {
        Self { entities: OrderedEntities::default() }
    }
}

impl<T: Component<Mutability = Mutable>> ComponentEntities<T> {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.entities
            .get(index)
            .map(|entity| entity.get::<T>().expect("entity retains its component"))
    }

    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.entities.get_mut(index).map(|value| value.into_inner())
    }

    pub fn last_mut(&mut self) -> Option<&mut T> {
        self.len().checked_sub(1).and_then(|index| self.get_mut(index))
    }

    pub fn iter(&self) -> ComponentIter<'_, T> {
        ComponentIter { entities: self, front: 0, back: self.len() }
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.entities.iter_mut().map(|value| value.into_inner())
    }

    pub fn push(&mut self, component: T) {
        self.entities.spawn(|world| world.spawn(component).id());
    }

    pub fn remove(&mut self, index: usize) -> T {
        self.entities
            .remove(index, |entity| entity.take::<T>().expect("entity retains its component"))
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        self.entities
            .retain(|entity| keep(entity.get::<T>().expect("entity retains its component")));
    }

    pub fn retain_mut(&mut self, mut keep: impl FnMut(&mut T) -> bool) {
        self.entities.retain_mut(|mut component| keep(&mut component));
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }
}

pub struct ComponentIter<'a, T: Component<Mutability = Mutable>> {
    entities: &'a ComponentEntities<T>,
    front: usize,
    back: usize,
}

impl<T: Component<Mutability = Mutable>> Clone for ComponentIter<'_, T> {
    fn clone(&self) -> Self {
        Self { entities: self.entities, front: self.front, back: self.back }
    }
}

impl<'a, T: Component<Mutability = Mutable>> Iterator for ComponentIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let value = self.entities.get(self.front);
        self.front += 1;
        value
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.back - self.front;
        (len, Some(len))
    }
}

impl<T: Component<Mutability = Mutable>> DoubleEndedIterator for ComponentIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        self.entities.get(self.back)
    }
}

impl<T: Component<Mutability = Mutable>> ExactSizeIterator for ComponentIter<'_, T> {}

impl<T: Component<Mutability = Mutable>> Index<usize> for ComponentEntities<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("entity index is in bounds")
    }
}

impl<T: Component<Mutability = Mutable>> IndexMut<usize> for ComponentEntities<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("entity index is in bounds")
    }
}

impl<'a, T: Component<Mutability = Mutable>> IntoIterator for &'a ComponentEntities<T> {
    type Item = &'a T;
    type IntoIter = ComponentIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: Component<Mutability = Mutable>> IntoIterator for ComponentEntities<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.entities
            .into_records(|entity| entity.take::<T>().expect("entity retains its component"))
            .into_iter()
    }
}

impl<T: Component<Mutability = Mutable>> Extend<T> for ComponentEntities<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, values: I) {
        for value in values {
            self.push(value);
        }
    }
}

impl<T: Component<Mutability = Mutable>> FromIterator<T> for ComponentEntities<T> {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        let mut result = Self::default();
        result.extend(values);
        result
    }
}

impl<T: Component<Mutability = Mutable> + Clone> Clone for ComponentEntities<T> {
    fn clone(&self) -> Self {
        self.iter().cloned().collect()
    }
}

impl<T: Component<Mutability = Mutable> + fmt::Debug> fmt::Debug for ComponentEntities<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Component<Mutability = Mutable> + PartialEq> PartialEq for ComponentEntities<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

#[cfg(feature = "persistence")]
impl<T: Component<Mutability = Mutable> + serde::Serialize> serde::Serialize
    for ComponentEntities<T>
{
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "persistence")]
impl<'de, T: Component<Mutability = Mutable> + serde::Deserialize<'de>> serde::Deserialize<'de>
    for ComponentEntities<T>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<T>::deserialize(deserializer).map(|values| values.into_iter().collect())
    }
}
