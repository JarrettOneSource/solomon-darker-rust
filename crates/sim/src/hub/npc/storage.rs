//! Student bodies and routes retain their admission order through contact commits.

use std::fmt;
use std::ops::{Deref, DerefMut};

use bevy_ecs::query::QueryItem;
use bevy_ecs::world::EntityRef;

use super::{HubStudentState, StudentBody, StudentRoute};
use crate::ecs::OrderedEntities;

pub trait StudentRead: Deref<Target = StudentBody> {
    fn controller(&self) -> &StudentRoute;

    fn view(&self) -> StudentRef<'_> {
        StudentRef { body: self.deref(), controller: self.controller() }
    }

    fn snapshot(&self) -> HubStudentState {
        HubStudentState { body: self.deref().clone(), controller: self.controller().clone() }
    }
}

impl Deref for HubStudentState {
    type Target = StudentBody;
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DerefMut for HubStudentState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body
    }
}

impl StudentRead for HubStudentState {
    fn controller(&self) -> &StudentRoute {
        &self.controller
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StudentRef<'a> {
    pub body: &'a StudentBody,
    pub controller: &'a StudentRoute,
}

impl Deref for StudentRef<'_> {
    type Target = StudentBody;
    fn deref(&self) -> &Self::Target {
        self.body
    }
}

impl StudentRead for StudentRef<'_> {
    fn controller(&self) -> &StudentRoute {
        self.controller
    }
}

pub struct StudentMut<'a> {
    pub body: &'a mut StudentBody,
    pub controller: &'a mut StudentRoute,
}

impl Deref for StudentMut<'_> {
    type Target = StudentBody;
    fn deref(&self) -> &Self::Target {
        self.body
    }
}

impl DerefMut for StudentMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.body
    }
}

impl StudentRead for StudentMut<'_> {
    fn controller(&self) -> &StudentRoute {
        self.controller
    }
}

type StudentQuery = (&'static mut StudentBody, &'static mut StudentRoute);

impl<'a> StudentMut<'a> {
    pub fn replace(&mut self, record: HubStudentState) {
        *self.body = record.body;
        *self.controller = record.controller;
    }

    fn from_query((body, controller): QueryItem<'a, 'static, StudentQuery>) -> Self {
        Self { body: body.into_inner(), controller: controller.into_inner() }
    }
}

#[derive(Default)]
pub struct StudentActors {
    entities: OrderedEntities<StudentQuery>,
}

impl StudentActors {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = StudentRef<'_>> + ExactSizeIterator + Clone {
        self.entities.iter().map(Self::read)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = StudentMut<'_>> {
        self.entities.iter_mut().map(StudentMut::from_query)
    }

    fn read(entity: EntityRef<'_>) -> StudentRef<'_> {
        StudentRef {
            body: entity.get::<StudentBody>().expect("student owns its body"),
            controller: entity.get::<StudentRoute>().expect("student owns its controller"),
        }
    }

    pub fn get(&self, index: usize) -> Option<StudentRef<'_>> {
        self.entities.get(index).map(Self::read)
    }

    pub fn push(&mut self, actor: HubStudentState) {
        self.entities.spawn(|world| world.spawn((actor.body, actor.controller)).id());
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&StudentRef<'_>) -> bool) {
        self.entities.retain(|entity| keep(&Self::read(entity)));
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }
}

impl FromIterator<HubStudentState> for StudentActors {
    fn from_iter<I: IntoIterator<Item = HubStudentState>>(actors: I) -> Self {
        let mut result = Self::default();
        for actor in actors {
            result.push(actor);
        }
        result
    }
}

impl Clone for StudentActors {
    fn clone(&self) -> Self {
        self.iter().map(|actor| actor.snapshot()).collect()
    }
}

impl fmt::Debug for StudentActors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for StudentActors {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for StudentActors {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for StudentActors {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<HubStudentState>::deserialize(deserializer).map(|actors| actors.into_iter().collect())
    }
}

#[cfg(feature = "persistence")]
fn serialize_student<S: serde::Serializer>(
    actor: &impl StudentRead,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut row = serializer.serialize_struct("HubStudentState", 21)?;
    row.serialize_field("current_speed", &actor.current_speed)?;
    row.serialize_field("desired_speed", &actor.controller().desired_speed)?;
    row.serialize_field("frame_phase", &actor.frame_phase)?;
    row.serialize_field("gait_degrees", &actor.gait_degrees)?;
    row.serialize_field("heading", &actor.heading)?;
    row.serialize_field("heading_index", &actor.heading_index)?;
    row.serialize_field("id", &actor.id)?;
    row.serialize_field("path_cursor", &actor.controller().path_cursor)?;
    row.serialize_field("path_id", &actor.controller().path_id)?;
    row.serialize_field("path_step", &actor.controller().path_step)?;
    row.serialize_field("registration_ordinal", &actor.registration_ordinal)?;
    row.serialize_field("position", &actor.position)?;
    row.serialize_field("profile", &actor.profile)?;
    row.serialize_field("props", &actor.props)?;
    row.serialize_field("reading", &actor.reading)?;
    row.serialize_field("retired", &actor.retired)?;
    row.serialize_field("rng_state", &actor.controller().rng_state)?;
    row.serialize_field("scale", &actor.scale)?;
    row.serialize_field("static_collision_enabled", &actor.static_collision_enabled)?;
    row.serialize_field("tick", &actor.controller().tick)?;
    row.serialize_field("wander", &actor.controller().wander)?;
    row.end()
}
#[cfg(feature = "persistence")]
impl serde::Serialize for HubStudentState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_student(self, serializer)
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for StudentRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_student(self, serializer)
    }
}
