//! Player projectile components and their ordered admission/retirement boundary.

use super::model::*;
use super::weld::{NativeWeldProjectileState, WeldProjectileProgram};
use crate::ecs::OrderedEntities;
use bevy_ecs::query::QueryItem;
use bevy_ecs::world::{EntityRef, EntityWorldMut};
use std::fmt;
use std::ops::{Deref, DerefMut};

/// The same family view is used by births, ECS queries, contacts and presentation.
#[derive(Debug, PartialEq)]
pub struct ProjectileRef<'a, P> {
    pub common: &'a PrimarySpellProjectileBase,
    pub program: &'a P,
}
impl<P> Copy for ProjectileRef<'_, P> {}
impl<P> Clone for ProjectileRef<'_, P> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<P> Deref for ProjectileRef<'_, P> {
    type Target = P;
    fn deref(&self) -> &P {
        self.program
    }
}
impl<P> ProjectileRef<'_, P> {
    pub fn view(&self) -> ProjectileRef<'_, P> {
        *self
    }
}

pub struct ProjectileMut<'a, P> {
    pub common: &'a mut PrimarySpellProjectileBase,
    pub program: &'a mut P,
}
impl<P> Deref for ProjectileMut<'_, P> {
    type Target = P;
    fn deref(&self) -> &P {
        self.program
    }
}
impl<P> DerefMut for ProjectileMut<'_, P> {
    fn deref_mut(&mut self) -> &mut P {
        self.program
    }
}
impl<P> ProjectileMut<'_, P> {
    pub fn view(&self) -> ProjectileRef<'_, P> {
        ProjectileRef { common: self.common, program: self.program }
    }
}

#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub(crate) enum PrimaryProjectileProgram {
    Weld(WeldProjectileProgram),
    Earth(EarthProjectileProgram),
    Ether(EtherProjectileProgram),
    Fire(FireProjectileProgram),
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Serialize),
    serde(rename = "PrimarySpellProjectile")
)]
pub enum PrimaryProjectileRef<'a> {
    Weld(ProjectileRef<'a, WeldProjectileProgram>),
    Earth(ProjectileRef<'a, EarthProjectileProgram>),
    Ether(ProjectileRef<'a, EtherProjectileProgram>),
    Fire(ProjectileRef<'a, FireProjectileProgram>),
}

pub enum PrimaryProjectilePartsMut<'a> {
    Weld(ProjectileMut<'a, WeldProjectileProgram>),
    Earth(ProjectileMut<'a, EarthProjectileProgram>),
    Ether(ProjectileMut<'a, EtherProjectileProgram>),
    Fire(ProjectileMut<'a, FireProjectileProgram>),
}

pub trait PrimaryProjectileRead {
    fn view(&self) -> PrimaryProjectileRef<'_>;
    fn common(&self) -> &PrimarySpellProjectileBase {
        self.view().common()
    }
    fn snapshot(&self) -> PrimarySpellProjectile {
        self.view().snapshot()
    }
}
impl PrimaryProjectileRead for PrimaryProjectileRef<'_> {
    fn view(&self) -> PrimaryProjectileRef<'_> {
        *self
    }
}
impl PrimaryProjectileRead for PrimarySpellProjectile {
    fn view(&self) -> PrimaryProjectileRef<'_> {
        match self {
            Self::Weld(row) => PrimaryProjectileRef::Weld(row.view()),
            Self::Earth(row) => PrimaryProjectileRef::Earth(row.view()),
            Self::Ether(row) => PrimaryProjectileRef::Ether(row.view()),
            Self::Fire(row) => PrimaryProjectileRef::Fire(row.view()),
        }
    }
}
impl PrimarySpellProjectile {
    pub fn view_mut(&mut self) -> PrimaryProjectilePartsMut<'_> {
        match self {
            Self::Weld(row) => PrimaryProjectilePartsMut::Weld(row.view_mut()),
            Self::Earth(row) => PrimaryProjectilePartsMut::Earth(row.view_mut()),
            Self::Ether(row) => PrimaryProjectilePartsMut::Ether(row.view_mut()),
            Self::Fire(row) => PrimaryProjectilePartsMut::Fire(row.view_mut()),
        }
    }
    fn into_components(self) -> (PrimarySpellProjectileBase, PrimaryProjectileProgram) {
        match self {
            Self::Weld(row) => (row.common, PrimaryProjectileProgram::Weld(row.program)),
            Self::Earth(row) => (row.common, PrimaryProjectileProgram::Earth(row.program)),
            Self::Ether(row) => (row.common, PrimaryProjectileProgram::Ether(row.program)),
            Self::Fire(row) => (row.common, PrimaryProjectileProgram::Fire(row.program)),
        }
    }
}
impl<'a> PrimaryProjectileRef<'a> {
    pub fn common(self) -> &'a PrimarySpellProjectileBase {
        match self {
            Self::Weld(row) => row.common,
            Self::Earth(row) => row.common,
            Self::Ether(row) => row.common,
            Self::Fire(row) => row.common,
        }
    }
    pub fn snapshot(self) -> PrimarySpellProjectile {
        match self {
            Self::Weld(row) => PrimarySpellProjectile::Weld(row.snapshot()),
            Self::Earth(row) => PrimarySpellProjectile::Earth(row.snapshot()),
            Self::Ether(row) => PrimarySpellProjectile::Ether(row.snapshot()),
            Self::Fire(row) => PrimarySpellProjectile::Fire(row.snapshot()),
        }
    }
}
impl PrimaryProjectileRead for PrimaryProjectilePartsMut<'_> {
    fn view(&self) -> PrimaryProjectileRef<'_> {
        match self {
            Self::Weld(row) => PrimaryProjectileRef::Weld(row.view()),
            Self::Earth(row) => PrimaryProjectileRef::Earth(row.view()),
            Self::Ether(row) => PrimaryProjectileRef::Ether(row.view()),
            Self::Fire(row) => PrimaryProjectileRef::Fire(row.view()),
        }
    }
}
impl PrimaryProjectilePartsMut<'_> {
    pub fn common_mut(&mut self) -> &mut PrimarySpellProjectileBase {
        match self {
            Self::Weld(row) => row.common,
            Self::Earth(row) => row.common,
            Self::Ether(row) => row.common,
            Self::Fire(row) => row.common,
        }
    }
}

pub struct PrimaryProjectileMut<'a> {
    common: &'a mut PrimarySpellProjectileBase,
    program: &'a mut PrimaryProjectileProgram,
}
impl PrimaryProjectileRead for PrimaryProjectileMut<'_> {
    fn view(&self) -> PrimaryProjectileRef<'_> {
        read(self.common, self.program)
    }
}
impl PrimaryProjectileMut<'_> {
    pub fn common_mut(&mut self) -> &mut PrimarySpellProjectileBase {
        self.common
    }
    pub fn view_mut(&mut self) -> PrimaryProjectilePartsMut<'_> {
        match self.program {
            PrimaryProjectileProgram::Weld(program) => {
                PrimaryProjectilePartsMut::Weld(ProjectileMut { common: self.common, program })
            }
            PrimaryProjectileProgram::Earth(program) => {
                PrimaryProjectilePartsMut::Earth(ProjectileMut { common: self.common, program })
            }
            PrimaryProjectileProgram::Ether(program) => {
                PrimaryProjectilePartsMut::Ether(ProjectileMut { common: self.common, program })
            }
            PrimaryProjectileProgram::Fire(program) => {
                PrimaryProjectilePartsMut::Fire(ProjectileMut { common: self.common, program })
            }
        }
    }
    pub fn replace(&mut self, record: PrimarySpellProjectile) {
        let (common, program) = record.into_components();
        *self.common = common;
        *self.program = program;
    }
}
fn read<'a>(
    common: &'a PrimarySpellProjectileBase,
    program: &'a PrimaryProjectileProgram,
) -> PrimaryProjectileRef<'a> {
    match program {
        PrimaryProjectileProgram::Weld(program) => {
            PrimaryProjectileRef::Weld(ProjectileRef { common, program })
        }
        PrimaryProjectileProgram::Earth(program) => {
            PrimaryProjectileRef::Earth(ProjectileRef { common, program })
        }
        PrimaryProjectileProgram::Ether(program) => {
            PrimaryProjectileRef::Ether(ProjectileRef { common, program })
        }
        PrimaryProjectileProgram::Fire(program) => {
            PrimaryProjectileRef::Fire(ProjectileRef { common, program })
        }
    }
}

type ProjectileQuery =
    (&'static mut PrimarySpellProjectileBase, &'static mut PrimaryProjectileProgram);
impl<'a> PrimaryProjectileMut<'a> {
    fn from_query((common, program): QueryItem<'a, 'static, ProjectileQuery>) -> Self {
        Self { common: common.into_inner(), program: program.into_inner() }
    }
}

#[derive(Default)]
pub struct PrimaryProjectiles {
    entities: OrderedEntities<ProjectileQuery>,
}
impl PrimaryProjectiles {
    pub fn len(&self) -> usize {
        self.entities.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = PrimaryProjectileRef<'_>> + ExactSizeIterator + Clone {
        self.entities.iter().map(Self::read)
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = PrimaryProjectileMut<'_>> {
        self.entities.iter_mut().map(PrimaryProjectileMut::from_query)
    }
    fn read(entity: EntityRef<'_>) -> PrimaryProjectileRef<'_> {
        read(
            entity.get::<PrimarySpellProjectileBase>().expect("projectile owns flight state"),
            entity.get::<PrimaryProjectileProgram>().expect("projectile owns its program"),
        )
    }
    fn take(entity: &mut EntityWorldMut<'_>) -> PrimarySpellProjectile {
        let common =
            entity.take::<PrimarySpellProjectileBase>().expect("projectile owns flight state");
        let program =
            entity.take::<PrimaryProjectileProgram>().expect("projectile owns its program");
        match program {
            PrimaryProjectileProgram::Weld(program) => {
                PrimarySpellProjectile::Weld(NativeWeldProjectileState { common, program })
            }
            PrimaryProjectileProgram::Earth(program) => {
                PrimarySpellProjectile::Earth(PrimarySpellEarthProjectile { common, program })
            }
            PrimaryProjectileProgram::Ether(program) => {
                PrimarySpellProjectile::Ether(PrimarySpellEtherProjectile { common, program })
            }
            PrimaryProjectileProgram::Fire(program) => {
                PrimarySpellProjectile::Fire(PrimarySpellFireProjectile { common, program })
            }
        }
    }
    pub fn get(&self, index: usize) -> Option<PrimaryProjectileRef<'_>> {
        self.entities.get(index).map(Self::read)
    }
    pub fn get_mut(&mut self, index: usize) -> Option<PrimaryProjectileMut<'_>> {
        self.entities.get_mut(index).map(PrimaryProjectileMut::from_query)
    }
    pub fn push(&mut self, record: PrimarySpellProjectile) {
        self.entities.spawn(|world| world.spawn(record.into_components()).id());
    }
    pub fn remove(&mut self, index: usize) -> PrimarySpellProjectile {
        self.entities.remove(index, Self::take)
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&PrimaryProjectileRef<'_>) -> bool) {
        self.entities.retain(|entity| keep(&Self::read(entity)));
    }
    pub fn retain_mut(&mut self, mut keep: impl FnMut(&mut PrimaryProjectileMut<'_>) -> bool) {
        self.entities
            .retain_mut(|components| keep(&mut PrimaryProjectileMut::from_query(components)));
    }
    pub fn clear(&mut self) {
        self.entities.clear();
    }
}
impl Extend<PrimarySpellProjectile> for PrimaryProjectiles {
    fn extend<I: IntoIterator<Item = PrimarySpellProjectile>>(&mut self, records: I) {
        for record in records {
            self.push(record);
        }
    }
}
impl FromIterator<PrimarySpellProjectile> for PrimaryProjectiles {
    fn from_iter<I: IntoIterator<Item = PrimarySpellProjectile>>(records: I) -> Self {
        let mut result = Self::default();
        result.extend(records);
        result
    }
}
impl Clone for PrimaryProjectiles {
    fn clone(&self) -> Self {
        self.iter().map(|row| row.snapshot()).collect()
    }
}
impl fmt::Debug for PrimaryProjectiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl PartialEq for PrimaryProjectiles {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for PrimaryProjectiles {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}
#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for PrimaryProjectiles {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<PrimarySpellProjectile>::deserialize(deserializer)
            .map(|rows| rows.into_iter().collect())
    }
}

impl Deref for NativeWeldProjectileState {
    type Target = WeldProjectileProgram;
    fn deref(&self) -> &Self::Target {
        &self.program
    }
}
impl DerefMut for NativeWeldProjectileState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.program
    }
}
impl NativeWeldProjectileState {
    pub fn view(&self) -> ProjectileRef<'_, WeldProjectileProgram> {
        ProjectileRef { common: &self.common, program: &self.program }
    }
    pub fn view_mut(&mut self) -> ProjectileMut<'_, WeldProjectileProgram> {
        ProjectileMut { common: &mut self.common, program: &mut self.program }
    }
}
impl ProjectileRef<'_, WeldProjectileProgram> {
    pub fn snapshot(self) -> NativeWeldProjectileState {
        NativeWeldProjectileState { common: self.common.clone(), program: self.program.clone() }
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for NativeWeldProjectileState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.view().serialize(serializer)
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for ProjectileRef<'_, WeldProjectileProgram> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut row = serializer.serialize_struct("NativeWeldProjectileState", 24)?;
        row.serialize_field("common", &self.common)?;
        row.serialize_field("ball_lightning_acceleration", &self.ball_lightning_acceleration)?;
        row.serialize_field(
            "base_presentation_phase_degrees",
            &self.base_presentation_phase_degrees,
        )?;
        row.serialize_field("build_id", &self.build_id)?;
        row.serialize_field("cast_playback_rate", &self.cast_playback_rate)?;
        row.serialize_field("cast_sound_variant", &self.cast_sound_variant)?;
        row.serialize_field("contacts_remaining", &self.contacts_remaining)?;
        row.serialize_field("heading_degrees", &self.heading_degrees)?;
        row.serialize_field("hit_target_ids", &self.hit_target_ids)?;
        row.serialize_field("frost_pulse_aspect", &self.frost_pulse_aspect)?;
        row.serialize_field("frost_presentation_lanes", &self.frost_presentation_lanes)?;
        row.serialize_field("frost_turn_degrees", &self.frost_turn_degrees)?;
        row.serialize_field("ground_spark_native_age_ticks", &self.ground_spark_native_age_ticks)?;
        row.serialize_field(
            "ground_spark_turn_ticks_remaining",
            &self.ground_spark_turn_ticks_remaining,
        )?;
        row.serialize_field("presentation_seed", &self.presentation_seed)?;
        row.serialize_field("projectile_index", &self.projectile_index)?;
        row.serialize_field("reacquires_target", &self.reacquires_target)?;
        row.serialize_field(
            "secondary_presentation_phase_degrees",
            &self.secondary_presentation_phase_degrees,
        )?;
        row.serialize_field("speed", &self.speed)?;
        row.serialize_field("target_id", &self.target_id)?;
        row.serialize_field("turn_accumulator", &self.turn_accumulator)?;
        row.serialize_field("turn_input", &self.turn_input)?;
        row.serialize_field("underpowered", &self.underpowered)?;
        row.serialize_field("vector", &self.vector)?;
        row.end()
    }
}

impl Deref for PrimarySpellEarthProjectile {
    type Target = EarthProjectileProgram;
    fn deref(&self) -> &Self::Target {
        &self.program
    }
}
impl DerefMut for PrimarySpellEarthProjectile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.program
    }
}
impl PrimarySpellEarthProjectile {
    pub fn view(&self) -> ProjectileRef<'_, EarthProjectileProgram> {
        ProjectileRef { common: &self.common, program: &self.program }
    }
    pub fn view_mut(&mut self) -> ProjectileMut<'_, EarthProjectileProgram> {
        ProjectileMut { common: &mut self.common, program: &mut self.program }
    }
}
impl ProjectileRef<'_, EarthProjectileProgram> {
    pub fn snapshot(self) -> PrimarySpellEarthProjectile {
        PrimarySpellEarthProjectile { common: self.common.clone(), program: self.program.clone() }
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for PrimarySpellEarthProjectile {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.view().serialize(serializer)
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for ProjectileRef<'_, EarthProjectileProgram> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut row = serializer.serialize_struct("PrimarySpellEarthProjectile", 8)?;
        row.serialize_field("common", &self.common)?;
        row.serialize_field("assembly_charge", &self.assembly_charge)?;
        row.serialize_field("hit_target_ids", &self.hit_target_ids)?;
        row.serialize_field("maximum_charge", &self.maximum_charge)?;
        row.serialize_field("orientation", &self.orientation)?;
        row.serialize_field("remaining_damage", &self.remaining_damage)?;
        row.serialize_field("shell_charge", &self.shell_charge)?;
        row.serialize_field("toughness", &self.toughness)?;
        row.end()
    }
}

impl Deref for PrimarySpellEtherProjectile {
    type Target = EtherProjectileProgram;
    fn deref(&self) -> &Self::Target {
        &self.program
    }
}
impl DerefMut for PrimarySpellEtherProjectile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.program
    }
}
impl PrimarySpellEtherProjectile {
    pub fn view(&self) -> ProjectileRef<'_, EtherProjectileProgram> {
        ProjectileRef { common: &self.common, program: &self.program }
    }
    pub fn view_mut(&mut self) -> ProjectileMut<'_, EtherProjectileProgram> {
        ProjectileMut { common: &mut self.common, program: &mut self.program }
    }
}
impl ProjectileRef<'_, EtherProjectileProgram> {
    pub fn snapshot(self) -> PrimarySpellEtherProjectile {
        PrimarySpellEtherProjectile { common: self.common.clone(), program: self.program.clone() }
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for PrimarySpellEtherProjectile {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.view().serialize(serializer)
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for ProjectileRef<'_, EtherProjectileProgram> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut row = serializer.serialize_struct("PrimarySpellEtherProjectile", 11)?;
        row.serialize_field("common", &self.common)?;
        row.serialize_field("damage_retention", &self.damage_retention)?;
        row.serialize_field("heading_degrees", &self.heading_degrees)?;
        row.serialize_field("pierces_remaining", &self.pierces_remaining)?;
        row.serialize_field("reacquires_target", &self.reacquires_target)?;
        row.serialize_field("speed", &self.speed)?;
        row.serialize_field("target_id", &self.target_id)?;
        row.serialize_field("turn_input", &self.turn_input)?;
        row.serialize_field("turn_accumulator", &self.turn_accumulator)?;
        row.serialize_field("underpowered", &self.underpowered)?;
        row.serialize_field("visual_scale", &self.visual_scale)?;
        row.end()
    }
}

impl Deref for PrimarySpellFireProjectile {
    type Target = FireProjectileProgram;
    fn deref(&self) -> &Self::Target {
        &self.program
    }
}
impl DerefMut for PrimarySpellFireProjectile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.program
    }
}
impl PrimarySpellFireProjectile {
    pub fn view(&self) -> ProjectileRef<'_, FireProjectileProgram> {
        ProjectileRef { common: &self.common, program: &self.program }
    }
    pub fn view_mut(&mut self) -> ProjectileMut<'_, FireProjectileProgram> {
        ProjectileMut { common: &mut self.common, program: &mut self.program }
    }
}
impl ProjectileRef<'_, FireProjectileProgram> {
    pub fn snapshot(self) -> PrimarySpellFireProjectile {
        PrimarySpellFireProjectile { common: self.common.clone(), program: self.program.clone() }
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for PrimarySpellFireProjectile {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.view().serialize(serializer)
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for ProjectileRef<'_, FireProjectileProgram> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut row = serializer.serialize_struct("PrimarySpellFireProjectile", 9)?;
        row.serialize_field("common", &self.common)?;
        row.serialize_field("burn_damage", &self.burn_damage)?;
        row.serialize_field("ember_damage", &self.ember_damage)?;
        row.serialize_field("ember_fragments", &self.ember_fragments)?;
        row.serialize_field("explode_damage", &self.explode_damage)?;
        row.serialize_field("explode_radius", &self.explode_radius)?;
        row.serialize_field("private_seed", &self.private_seed)?;
        row.serialize_field("spent_ember", &self.spent_ember)?;
        row.serialize_field("underpowered", &self.underpowered)?;
        row.end()
    }
}
