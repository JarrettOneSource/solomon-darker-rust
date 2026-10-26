//! Enemy component ownership. Controller commits preserve the live entity handle
//! and the admission order observed by targeting, RNG and painter allocation.

use std::fmt;
use std::ops::{Deref, DerefMut};

use bevy_ecs::query::QueryItem;
use bevy_ecs::world::{EntityRef, EntityWorldMut};

use super::model::{EnemyActor, EnemyBody, EnemyBrain};
use crate::ecs::OrderedEntities;

pub trait EnemyActorRead: Deref<Target = EnemyBody> {
    fn brain(&self) -> &EnemyBrain;

    fn view(&self) -> EnemyActorRef<'_> {
        EnemyActorRef { body: self.deref(), brain: self.brain() }
    }

    fn snapshot(&self) -> EnemyActor {
        EnemyActor { body: self.deref().clone(), brain: self.brain().clone() }
    }
}

impl Deref for EnemyActor {
    type Target = EnemyBody;
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DerefMut for EnemyActor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body
    }
}

impl EnemyActorRead for EnemyActor {
    fn brain(&self) -> &EnemyBrain {
        &self.brain
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyActorRef<'a> {
    pub body: &'a EnemyBody,
    pub brain: &'a EnemyBrain,
}

impl Deref for EnemyActorRef<'_> {
    type Target = EnemyBody;
    fn deref(&self) -> &Self::Target {
        self.body
    }
}

impl EnemyActorRead for EnemyActorRef<'_> {
    fn brain(&self) -> &EnemyBrain {
        self.brain
    }
}

pub struct EnemyActorMut<'a> {
    pub body: &'a mut EnemyBody,
    pub brain: &'a mut EnemyBrain,
}

impl Deref for EnemyActorMut<'_> {
    type Target = EnemyBody;
    fn deref(&self) -> &Self::Target {
        self.body
    }
}

impl DerefMut for EnemyActorMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.body
    }
}

impl EnemyActorRead for EnemyActorMut<'_> {
    fn brain(&self) -> &EnemyBrain {
        self.brain
    }
}

type EnemyQuery = (&'static mut EnemyBody, &'static mut EnemyBrain);

impl<'a> EnemyActorMut<'a> {
    fn from_query((body, brain): QueryItem<'a, 'static, EnemyQuery>) -> Self {
        Self { body: body.into_inner(), brain: brain.into_inner() }
    }
}

#[derive(Default)]
pub struct EnemyActors {
    entities: OrderedEntities<EnemyQuery>,
}

impl EnemyActors {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = EnemyActorRef<'_>> + ExactSizeIterator + Clone {
        self.entities.iter().map(Self::read)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = EnemyActorMut<'_>> {
        self.entities.iter_mut().map(EnemyActorMut::from_query)
    }

    fn read(entity: EntityRef<'_>) -> EnemyActorRef<'_> {
        EnemyActorRef {
            body: entity.get::<EnemyBody>().expect("enemy owns its body"),
            brain: entity.get::<EnemyBrain>().expect("enemy owns its brain"),
        }
    }

    pub fn get(&self, index: usize) -> Option<EnemyActorRef<'_>> {
        self.entities.get(index).map(Self::read)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<EnemyActorMut<'_>> {
        self.entities.get_mut(index).map(EnemyActorMut::from_query)
    }

    pub fn replace(&mut self, index: usize, value: EnemyActor) {
        let actor = self.get_mut(index).expect("committed enemy remains live");
        *actor.body = value.body;
        *actor.brain = value.brain;
    }

    pub fn remove(&mut self, index: usize) -> EnemyActor {
        self.entities.remove(index, Self::take)
    }

    pub fn push(&mut self, actor: EnemyActor) {
        self.entities.spawn(|world| world.spawn((actor.body, actor.brain)).id());
    }

    fn take(entity: &mut EntityWorldMut<'_>) -> EnemyActor {
        EnemyActor {
            body: entity.take::<EnemyBody>().expect("enemy owns its body"),
            brain: entity.take::<EnemyBrain>().expect("enemy owns its brain"),
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&EnemyActorRef<'_>) -> bool) {
        self.entities.retain(|entity| keep(&Self::read(entity)));
    }

    pub fn retain_mut(&mut self, mut keep: impl FnMut(&mut EnemyActorMut<'_>) -> bool) {
        self.entities.retain_mut(|components| keep(&mut EnemyActorMut::from_query(components)));
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }
}

impl FromIterator<EnemyActor> for EnemyActors {
    fn from_iter<I: IntoIterator<Item = EnemyActor>>(actors: I) -> Self {
        let mut result = Self::default();
        for actor in actors {
            result.push(actor);
        }
        result
    }
}

impl IntoIterator for EnemyActors {
    type Item = EnemyActor;
    type IntoIter = std::vec::IntoIter<EnemyActor>;
    fn into_iter(self) -> Self::IntoIter {
        self.entities.into_records(Self::take).into_iter()
    }
}

impl Clone for EnemyActors {
    fn clone(&self) -> Self {
        self.iter().map(|actor| actor.snapshot()).collect()
    }
}

impl fmt::Debug for EnemyActors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for EnemyActors {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for EnemyActors {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for EnemyActors {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<EnemyActor>::deserialize(deserializer).map(|actors| actors.into_iter().collect())
    }
}

#[cfg(feature = "persistence")]
fn serialize_actor<S: serde::Serializer>(
    actor: &impl EnemyActorRead,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut row = serializer.serialize_struct("EnemyActor", 48)?;
    row.serialize_field("ether_drain_captured", &actor.ether_drain_captured)?;
    row.serialize_field("hit_feedback", &actor.hit_feedback)?;
    row.serialize_field("hit_reaction_timer", &actor.hit_reaction_timer)?;
    row.serialize_field("blizzard_push_accumulator", &actor.blizzard_push_accumulator)?;
    row.serialize_field("blizzard_push_last_tick", &actor.blizzard_push_last_tick)?;
    row.serialize_field("body_gait_phase", &actor.body_gait_phase)?;
    row.serialize_field("body_pose", &actor.body_pose)?;
    row.serialize_field("brain", &actor.brain())?;
    row.serialize_field("config", &actor.config)?;
    row.serialize_field("current_health", &actor.current_health)?;
    row.serialize_field("death_epoch", &actor.death_epoch)?;
    row.serialize_field("death_presentation_started", &actor.death_presentation_started)?;
    row.serialize_field("death_started_tick", &actor.death_started_tick)?;
    row.serialize_field("death_tick", &actor.death_tick)?;
    row.serialize_field("gait_pose", &actor.gait_pose)?;
    row.serialize_field("head_facing_offset", &actor.head_facing_offset)?;
    row.serialize_field("heading_deg", &actor.heading_deg)?;
    row.serialize_field("hurricane_contact_cooldown", &actor.hurricane_contact_cooldown)?;
    row.serialize_field("id", &actor.id)?;
    row.serialize_field("last_damaged_by_player_id", &actor.last_damaged_by_player_id)?;
    row.serialize_field("last_damage_tick", &actor.last_damage_tick)?;
    row.serialize_field("last_movement_tick", &actor.last_movement_tick)?;
    row.serialize_field("lethal_magic_damage", &actor.lethal_magic_damage)?;
    row.serialize_field("life_state", &actor.life_state)?;
    row.serialize_field("registration_ordinal", &actor.registration_ordinal)?;
    row.serialize_field("lighting", &actor.lighting)?;
    row.serialize_field("loot_seed", &actor.loot_seed)?;
    row.serialize_field("next_movement_tick", &actor.next_movement_tick)?;
    row.serialize_field("next_target_refresh_tick", &actor.next_target_refresh_tick)?;
    row.serialize_field("native_cell_binding_order", &actor.native_cell_binding_order)?;
    row.serialize_field("native_registration_order", &actor.native_registration_order)?;
    row.serialize_field("path", &actor.path)?;
    row.serialize_field("position", &actor.position)?;
    row.serialize_field("reward_granted", &actor.reward_granted)?;
    row.serialize_field("rest_body_pose", &actor.rest_body_pose)?;
    row.serialize_field("shield_health", &actor.shield_health)?;
    row.serialize_field("shadow_lateral_offset", &actor.shadow_lateral_offset)?;
    row.serialize_field("shield_maximum_health", &actor.shield_maximum_health)?;
    row.serialize_field("shield_pulse", &actor.shield_pulse)?;
    row.serialize_field("shield_sound_cooldown_ticks", &actor.shield_sound_cooldown_ticks)?;
    row.serialize_field("source_spawn_intent_id", &actor.source_spawn_intent_id)?;
    row.serialize_field("spawn_tick", &actor.spawn_tick)?;
    row.serialize_field("staff_action_factor", &actor.staff_action_factor)?;
    row.serialize_field("staff_movement_factor", &actor.staff_movement_factor)?;
    row.serialize_field("stride_phase_deg", &actor.stride_phase_deg)?;
    row.serialize_field("target_player_id", &actor.target_player_id)?;
    row.serialize_field("terminal_emitted", &actor.terminal_emitted)?;
    row.serialize_field("wave_ordinal", &actor.wave_ordinal)?;
    row.end()
}

#[cfg(feature = "persistence")]
impl serde::Serialize for EnemyActor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_actor(self, serializer)
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for EnemyActorRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_actor(self, serializer)
    }
}

// Preserve the flat fields and record name for the legacy save decoder. It uses
// typed struct and enum visitors, which serde flatten would bypass.
#[cfg(feature = "persistence")]
#[derive(serde::Deserialize)]
#[serde(rename = "EnemyActor")]
pub(super) struct EnemyActorFields {
    ether_drain_captured: bool,
    hit_feedback: crate::combat::PuppetHitState,
    /// Native +0x80 controls movement independently of visible feedback +0x78.
    hit_reaction_timer: f64,
    blizzard_push_accumulator: f64,
    blizzard_push_last_tick: Option<u64>,
    body_gait_phase: f64,
    body_pose: f64,
    brain: EnemyBrain,
    config: super::config::EnemyConfig,
    current_health: f64,
    death_epoch: Option<u64>,
    death_presentation_started: bool,
    death_started_tick: Option<u64>,
    death_tick: u64,
    gait_pose: f64,
    head_facing_offset: i8,
    heading_deg: f64,
    hurricane_contact_cooldown: f64,
    id: usize,
    last_damaged_by_player_id: Option<String>,
    last_damage_tick: Option<u64>,
    last_movement_tick: Option<u64>,
    lethal_magic_damage: bool,
    life_state: super::model::EnemyLifeState,
    registration_ordinal: usize,
    lighting: super::model::EnemyLighting,
    loot_seed: u32,
    next_movement_tick: u64,
    next_target_refresh_tick: u64,
    native_cell_binding_order: usize,
    native_registration_order: usize,
    path: super::path::NativeEnemyPathState,
    position: crate::Vec2,
    reward_granted: bool,
    rest_body_pose: f64,
    shield_health: f64,
    shadow_lateral_offset: f64,
    shield_maximum_health: f64,
    shield_pulse: f64,
    shield_sound_cooldown_ticks: u64,
    source_spawn_intent_id: usize,
    spawn_tick: u64,
    staff_action_factor: f64,
    staff_movement_factor: f64,
    stride_phase_deg: f64,
    target_player_id: Option<String>,
    terminal_emitted: bool,
    wave_ordinal: u64,
}

#[cfg(feature = "persistence")]
impl From<EnemyActorFields> for EnemyActor {
    fn from(row: EnemyActorFields) -> Self {
        Self {
            body: EnemyBody {
                ether_drain_captured: row.ether_drain_captured,
                hit_feedback: row.hit_feedback,
                hit_reaction_timer: row.hit_reaction_timer,
                blizzard_push_accumulator: row.blizzard_push_accumulator,
                blizzard_push_last_tick: row.blizzard_push_last_tick,
                body_gait_phase: row.body_gait_phase,
                body_pose: row.body_pose,
                config: row.config,
                current_health: row.current_health,
                death_epoch: row.death_epoch,
                death_presentation_started: row.death_presentation_started,
                death_started_tick: row.death_started_tick,
                death_tick: row.death_tick,
                gait_pose: row.gait_pose,
                head_facing_offset: row.head_facing_offset,
                heading_deg: row.heading_deg,
                hurricane_contact_cooldown: row.hurricane_contact_cooldown,
                id: row.id,
                last_damaged_by_player_id: row.last_damaged_by_player_id,
                last_damage_tick: row.last_damage_tick,
                last_movement_tick: row.last_movement_tick,
                lethal_magic_damage: row.lethal_magic_damage,
                life_state: row.life_state,
                registration_ordinal: row.registration_ordinal,
                lighting: row.lighting,
                loot_seed: row.loot_seed,
                next_movement_tick: row.next_movement_tick,
                next_target_refresh_tick: row.next_target_refresh_tick,
                native_cell_binding_order: row.native_cell_binding_order,
                native_registration_order: row.native_registration_order,
                path: row.path,
                position: row.position,
                reward_granted: row.reward_granted,
                rest_body_pose: row.rest_body_pose,
                shield_health: row.shield_health,
                shadow_lateral_offset: row.shadow_lateral_offset,
                shield_maximum_health: row.shield_maximum_health,
                shield_pulse: row.shield_pulse,
                shield_sound_cooldown_ticks: row.shield_sound_cooldown_ticks,
                source_spawn_intent_id: row.source_spawn_intent_id,
                spawn_tick: row.spawn_tick,
                staff_action_factor: row.staff_action_factor,
                staff_movement_factor: row.staff_movement_factor,
                stride_phase_deg: row.stride_phase_deg,
                target_player_id: row.target_player_id,
                terminal_emitted: row.terminal_emitted,
                wave_ordinal: row.wave_ordinal,
            },
            brain: row.brain,
        }
    }
}
