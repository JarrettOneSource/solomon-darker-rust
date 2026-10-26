//! One optional actor with a cohesive component and an explicit birth/retirement owner.

use bevy_ecs::component::{Component, Mutable};

use super::ComponentEntities;

pub struct OptionalEntity<T: Component<Mutability = Mutable>> {
    entities: ComponentEntities<T>,
}

impl<T: Component<Mutability = Mutable>> Default for OptionalEntity<T> {
    fn default() -> Self {
        Self { entities: ComponentEntities::default() }
    }
}

impl<T: Component<Mutability = Mutable>> OptionalEntity<T> {
    pub fn as_ref(&self) -> Option<&T> {
        self.entities.get(0)
    }

    pub fn as_mut(&mut self) -> Option<&mut T> {
        self.entities.get_mut(0)
    }

    pub fn is_some(&self) -> bool {
        !self.entities.is_empty()
    }

    pub fn set(&mut self, value: Option<T>) {
        match (self.as_mut(), value) {
            (Some(current), Some(value)) => *current = value,
            (None, Some(value)) => self.entities.push(value),
            (_, None) => self.entities.clear(),
        }
    }
}

impl<T: Component<Mutability = Mutable>> From<Option<T>> for OptionalEntity<T> {
    fn from(value: Option<T>) -> Self {
        let mut result = Self::default();
        result.set(value);
        result
    }
}

impl<T: Component<Mutability = Mutable> + Clone> Clone for OptionalEntity<T> {
    fn clone(&self) -> Self {
        self.as_ref().cloned().into()
    }
}

impl<T: Component<Mutability = Mutable> + std::fmt::Debug> std::fmt::Debug for OptionalEntity<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}

#[cfg(feature = "persistence")]
impl<T: Component<Mutability = Mutable> + serde::Serialize> serde::Serialize for OptionalEntity<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_ref().serialize(serializer)
    }
}

#[cfg(feature = "persistence")]
impl<'de, T: Component<Mutability = Mutable> + serde::Deserialize<'de>> serde::Deserialize<'de>
    for OptionalEntity<T>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(Self::from)
    }
}
