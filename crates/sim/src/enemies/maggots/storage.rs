//! Maggot body and emergence/navigation components retain their admission order.

use std::fmt;
use std::ops::{Deref, DerefMut};

use bevy_ecs::query::QueryItem;
use bevy_ecs::world::{EntityRef, EntityWorldMut};

use crate::ecs::OrderedEntities;
use crate::enemies::model::{EnemyMaggotActor, MaggotBody, MaggotController};

pub trait MaggotRead: Deref<Target = MaggotBody> {
    fn controller(&self) -> &MaggotController;

    fn view(&self) -> MaggotRef<'_> {
        MaggotRef { body: self.deref(), controller: self.controller() }
    }

    fn snapshot(&self) -> EnemyMaggotActor {
        EnemyMaggotActor { body: self.deref().clone(), controller: self.controller().clone() }
    }
}

impl Deref for EnemyMaggotActor {
    type Target = MaggotBody;
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DerefMut for EnemyMaggotActor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body
    }
}

impl MaggotRead for EnemyMaggotActor {
    fn controller(&self) -> &MaggotController {
        &self.controller
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaggotRef<'a> {
    pub body: &'a MaggotBody,
    pub controller: &'a MaggotController,
}

impl Deref for MaggotRef<'_> {
    type Target = MaggotBody;
    fn deref(&self) -> &Self::Target {
        self.body
    }
}

impl MaggotRead for MaggotRef<'_> {
    fn controller(&self) -> &MaggotController {
        self.controller
    }
}

pub struct MaggotMut<'a> {
    pub body: &'a mut MaggotBody,
    pub controller: &'a mut MaggotController,
}

impl Deref for MaggotMut<'_> {
    type Target = MaggotBody;
    fn deref(&self) -> &Self::Target {
        self.body
    }
}

impl DerefMut for MaggotMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.body
    }
}

impl MaggotRead for MaggotMut<'_> {
    fn controller(&self) -> &MaggotController {
        self.controller
    }
}

type MaggotQuery = (&'static mut MaggotBody, &'static mut MaggotController);

impl<'a> MaggotMut<'a> {
    fn from_query((body, controller): QueryItem<'a, 'static, MaggotQuery>) -> Self {
        Self { body: body.into_inner(), controller: controller.into_inner() }
    }
}

#[derive(Default)]
pub struct MaggotActors {
    entities: OrderedEntities<MaggotQuery>,
}

impl MaggotActors {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = MaggotRef<'_>> + ExactSizeIterator + Clone {
        self.entities.iter().map(Self::read)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = MaggotMut<'_>> {
        self.entities.iter_mut().map(MaggotMut::from_query)
    }

    fn read(entity: EntityRef<'_>) -> MaggotRef<'_> {
        MaggotRef {
            body: entity.get::<MaggotBody>().expect("maggot owns its body"),
            controller: entity.get::<MaggotController>().expect("maggot owns its controller"),
        }
    }

    pub fn get(&self, index: usize) -> Option<MaggotRef<'_>> {
        self.entities.get(index).map(Self::read)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<MaggotMut<'_>> {
        self.entities.get_mut(index).map(MaggotMut::from_query)
    }

    pub fn remove(&mut self, index: usize) -> EnemyMaggotActor {
        self.entities.remove(index, Self::take)
    }

    pub fn push(&mut self, actor: EnemyMaggotActor) {
        self.entities.spawn(|world| world.spawn((actor.body, actor.controller)).id());
    }

    fn take(entity: &mut EntityWorldMut<'_>) -> EnemyMaggotActor {
        EnemyMaggotActor {
            body: entity.take::<MaggotBody>().expect("maggot owns its body"),
            controller: entity.take::<MaggotController>().expect("maggot owns its controller"),
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&MaggotRef<'_>) -> bool) {
        self.entities.retain(|entity| keep(&Self::read(entity)));
    }

    pub fn retain_mut(&mut self, mut keep: impl FnMut(&mut MaggotMut<'_>) -> bool) {
        self.entities.retain_mut(|components| keep(&mut MaggotMut::from_query(components)));
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }
}

impl FromIterator<EnemyMaggotActor> for MaggotActors {
    fn from_iter<I: IntoIterator<Item = EnemyMaggotActor>>(actors: I) -> Self {
        let mut result = Self::default();
        for actor in actors {
            result.push(actor);
        }
        result
    }
}

impl Clone for MaggotActors {
    fn clone(&self) -> Self {
        self.iter().map(|actor| actor.snapshot()).collect()
    }
}

impl fmt::Debug for MaggotActors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for MaggotActors {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for MaggotActors {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for MaggotActors {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<EnemyMaggotActor>::deserialize(deserializer)
            .map(|actors| actors.into_iter().collect())
    }
}

#[cfg(feature = "persistence")]
fn serialize_maggot<S: serde::Serializer>(
    actor: &impl MaggotRead,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut row = serializer.serialize_struct("EnemyMaggotActor", 48)?;
    row.serialize_field("ether_drain_captured", &actor.ether_drain_captured)?;
    row.serialize_field("hit_feedback", &actor.hit_feedback)?;
    row.serialize_field("blizzard_push_accumulator", &actor.blizzard_push_accumulator)?;
    row.serialize_field("blizzard_push_last_tick", &actor.blizzard_push_last_tick)?;
    row.serialize_field("combat_active", &actor.combat_active)?;
    row.serialize_field("native_tree_query_member", &actor.native_tree_query_member)?;
    row.serialize_field("collision_radius", &actor.collision_radius)?;
    row.serialize_field("current_health", &actor.current_health)?;
    row.serialize_field("death_offsets", &actor.death_offsets)?;
    row.serialize_field("damage", &actor.damage)?;
    row.serialize_field("death_epoch", &actor.death_epoch)?;
    row.serialize_field("death_started_tick", &actor.death_started_tick)?;
    row.serialize_field("death_tick", &actor.death_tick)?;
    row.serialize_field("gait_pose", &actor.gait_pose)?;
    row.serialize_field("heading_deg", &actor.heading_deg)?;
    row.serialize_field("hurricane_contact_cooldown", &actor.hurricane_contact_cooldown)?;
    row.serialize_field("id", &actor.id)?;
    row.serialize_field("emergence_tick", &actor.controller().emergence_tick)?;
    row.serialize_field("emergence_phase", &actor.controller().emergence_phase)?;
    row.serialize_field("launch_trajectory", &actor.controller().launch_trajectory)?;
    row.serialize_field("launch_velocity", &actor.controller().launch_velocity)?;
    row.serialize_field("landing_bounce_velocity", &actor.controller().landing_bounce_velocity)?;
    row.serialize_field("last_attack_tick", &actor.controller().last_attack_tick)?;
    row.serialize_field("last_damaged_by_player_id", &actor.last_damaged_by_player_id)?;
    row.serialize_field("last_damage_tick", &actor.last_damage_tick)?;
    row.serialize_field("last_movement_tick", &actor.controller().last_movement_tick)?;
    row.serialize_field("life_state", &actor.life_state)?;
    row.serialize_field("registration_ordinal", &actor.registration_ordinal)?;
    row.serialize_field("maximum_health", &actor.maximum_health)?;
    row.serialize_field("next_attack_tick", &actor.controller().next_attack_tick)?;
    row.serialize_field("next_movement_tick", &actor.controller().next_movement_tick)?;
    row.serialize_field("next_target_refresh_tick", &actor.controller().next_target_refresh_tick)?;
    row.serialize_field("native_cell_binding_order", &actor.native_cell_binding_order)?;
    row.serialize_field("native_registration_order", &actor.native_registration_order)?;
    row.serialize_field("owner_coffin_actor_id", &actor.owner_coffin_actor_id)?;
    row.serialize_field("path", &actor.controller().path)?;
    row.serialize_field("poison_damage", &actor.poison_damage)?;
    row.serialize_field("poison_duration", &actor.poison_duration)?;
    row.serialize_field("position", &actor.position)?;
    row.serialize_field("movement_phase", &actor.controller().movement_phase)?;
    row.serialize_field("spawn_tick", &actor.spawn_tick)?;
    row.serialize_field("staff_action_factor", &actor.staff_action_factor)?;
    row.serialize_field("staff_movement_factor", &actor.staff_movement_factor)?;
    row.serialize_field("target_player_id", &actor.controller().target_player_id)?;
    row.serialize_field("terminal_emitted", &actor.terminal_emitted)?;
    row.serialize_field("vertical_offset", &actor.vertical_offset)?;
    row.serialize_field("vertical_velocity", &actor.vertical_velocity)?;
    row.serialize_field("visual_scale", &actor.visual_scale)?;
    row.end()
}

#[cfg(feature = "persistence")]
impl serde::Serialize for EnemyMaggotActor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_maggot(self, serializer)
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for MaggotRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_maggot(self, serializer)
    }
}
