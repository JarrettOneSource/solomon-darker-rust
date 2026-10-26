//! Ground-pickup components with admission-ordered capture and retirement.

use std::fmt;
use std::ops::{Deref, DerefMut};

use bevy_ecs::query::QueryItem;
use bevy_ecs::world::{EntityRef, EntityWorldMut};

use super::{BoneyardLootActor, LootBody, LootReward};
use crate::ecs::OrderedEntities;

pub trait LootActorRead: Deref<Target = LootBody> {
    fn reward(&self) -> &LootReward;

    fn snapshot(&self) -> BoneyardLootActor {
        BoneyardLootActor { body: self.deref().clone(), reward: self.reward().clone() }
    }
}

pub trait LootActorWrite: LootActorRead + DerefMut {
    fn reward_mut(&mut self) -> &mut LootReward;
}

impl Deref for BoneyardLootActor {
    type Target = LootBody;
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DerefMut for BoneyardLootActor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body
    }
}

impl LootActorRead for BoneyardLootActor {
    fn reward(&self) -> &LootReward {
        &self.reward
    }
}

impl LootActorWrite for BoneyardLootActor {
    fn reward_mut(&mut self) -> &mut LootReward {
        &mut self.reward
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LootActorRef<'a> {
    pub body: &'a LootBody,
    pub reward: &'a LootReward,
}

impl Deref for LootActorRef<'_> {
    type Target = LootBody;
    fn deref(&self) -> &Self::Target {
        self.body
    }
}

impl LootActorRead for LootActorRef<'_> {
    fn reward(&self) -> &LootReward {
        self.reward
    }
}

pub struct LootActorMut<'a> {
    pub body: &'a mut LootBody,
    pub reward: &'a mut LootReward,
}

impl Deref for LootActorMut<'_> {
    type Target = LootBody;
    fn deref(&self) -> &Self::Target {
        self.body
    }
}

impl DerefMut for LootActorMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.body
    }
}

impl LootActorRead for LootActorMut<'_> {
    fn reward(&self) -> &LootReward {
        self.reward
    }
}

impl LootActorWrite for LootActorMut<'_> {
    fn reward_mut(&mut self) -> &mut LootReward {
        self.reward
    }
}

type LootQuery = (&'static mut LootBody, &'static mut LootReward);

impl<'a> LootActorMut<'a> {
    fn from_query((body, reward): QueryItem<'a, 'static, LootQuery>) -> Self {
        Self { body: body.into_inner(), reward: reward.into_inner() }
    }
}

#[derive(Default)]
pub struct LootActors {
    entities: OrderedEntities<LootQuery>,
}

impl LootActors {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = LootActorRef<'_>> + ExactSizeIterator + Clone {
        self.entities.iter().map(Self::read)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = LootActorMut<'_>> {
        self.entities.iter_mut().map(LootActorMut::from_query)
    }

    fn read(entity: EntityRef<'_>) -> LootActorRef<'_> {
        LootActorRef {
            body: entity.get::<LootBody>().expect("pickup owns its body"),
            reward: entity.get::<LootReward>().expect("pickup owns its reward"),
        }
    }

    pub fn get(&self, index: usize) -> Option<LootActorRef<'_>> {
        self.entities.get(index).map(Self::read)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<LootActorMut<'_>> {
        self.entities.get_mut(index).map(LootActorMut::from_query)
    }

    pub fn push(&mut self, actor: BoneyardLootActor) {
        self.entities.spawn(|world| world.spawn((actor.body, actor.reward)).id());
    }

    fn take(entity: &mut EntityWorldMut<'_>) -> BoneyardLootActor {
        BoneyardLootActor {
            body: entity.take::<LootBody>().expect("pickup owns its body"),
            reward: entity.take::<LootReward>().expect("pickup owns its reward"),
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&LootActorRef<'_>) -> bool) {
        self.entities.retain(|entity| keep(&Self::read(entity)));
    }

    pub fn retain_mut(&mut self, mut keep: impl FnMut(&mut LootActorMut<'_>) -> bool) {
        self.entities.retain_mut(|components| keep(&mut LootActorMut::from_query(components)));
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }
}

impl FromIterator<BoneyardLootActor> for LootActors {
    fn from_iter<I: IntoIterator<Item = BoneyardLootActor>>(actors: I) -> Self {
        let mut result = Self::default();
        for actor in actors {
            result.push(actor);
        }
        result
    }
}

impl IntoIterator for LootActors {
    type Item = BoneyardLootActor;
    type IntoIter = std::vec::IntoIter<BoneyardLootActor>;
    fn into_iter(self) -> Self::IntoIter {
        self.entities.into_records(Self::take).into_iter()
    }
}

impl Clone for LootActors {
    fn clone(&self) -> Self {
        self.iter().map(|actor| actor.snapshot()).collect()
    }
}

impl fmt::Debug for LootActors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for LootActors {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for LootActors {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for LootActors {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<BoneyardLootActor>::deserialize(deserializer)
            .map(|actors| actors.into_iter().collect())
    }
}

#[cfg(feature = "persistence")]
fn serialize_actor<S: serde::Serializer>(
    actor: &impl LootActorRead,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut row = serializer.serialize_struct("BoneyardLootActor", 27)?;
    row.serialize_field("activation_delay_ticks", &actor.activation_delay_ticks)?;
    row.serialize_field("age_ticks", &actor.age_ticks)?;
    row.serialize_field("alpha", &actor.alpha)?;
    row.serialize_field("amount", &actor.reward().amount)?;
    row.serialize_field("animation_phase", &actor.animation_phase)?;
    row.serialize_field("bonus_kind", &actor.reward().bonus_kind)?;
    row.serialize_field("bounce_height", &actor.bounce_height)?;
    row.serialize_field("bounce_velocity", &actor.bounce_velocity)?;
    row.serialize_field("frame_phase", &actor.frame_phase)?;
    row.serialize_field("id", &actor.id)?;
    row.serialize_field("item", &actor.reward().item)?;
    row.serialize_field("kind", &actor.kind)?;
    row.serialize_field("last_step_tick", &actor.last_step_tick)?;
    row.serialize_field("lifetime_ticks_remaining", &actor.lifetime_ticks_remaining)?;
    row.serialize_field("native_type_id", &actor.reward().native_type_id)?;
    row.serialize_field("orb_kind", &actor.reward().orb_kind)?;
    row.serialize_field("orb_value", &actor.reward().orb_value)?;
    row.serialize_field("painter_registration", &actor.painter_registration)?;
    row.serialize_field("position", &actor.position)?;
    row.serialize_field("rotation_deg", &actor.rotation_deg)?;
    row.serialize_field("scatter_seed", &actor.scatter_seed)?;
    row.serialize_field("scatter_active", &actor.scatter_active)?;
    row.serialize_field("scatter_progress", &actor.scatter_progress)?;
    row.serialize_field("settled_sound_emitted", &actor.settled_sound_emitted)?;
    row.serialize_field("source", &actor.reward().source)?;
    row.serialize_field("spawn_tick", &actor.spawn_tick)?;
    row.serialize_field("tier", &actor.reward().tier)?;
    row.end()
}

#[cfg(feature = "persistence")]
impl serde::Serialize for BoneyardLootActor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_actor(self, serializer)
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for LootActorRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_actor(self, serializer)
    }
}

// Preserve the flat fields and record name for the legacy save decoder. It uses
// typed struct and enum visitors, which serde flatten would bypass.
#[cfg(feature = "persistence")]
#[derive(serde::Deserialize)]
#[serde(rename = "BoneyardLootActor")]
pub(super) struct LootActorFields {
    activation_delay_ticks: i64,
    age_ticks: u64,
    alpha: f64,
    amount: u64,
    animation_phase: f64,
    bonus_kind: Option<crate::loot::NativeBonusKind>,
    bounce_height: f64,
    bounce_velocity: f64,
    frame_phase: f64,
    id: usize,
    item: Option<crate::economy::HubInventoryItem>,
    kind: crate::loot::NativeLootDropKind,
    last_step_tick: Option<u64>,
    lifetime_ticks_remaining: Option<i64>,
    native_type_id: u32,
    orb_kind: Option<crate::loot::NativeOrbKind>,
    orb_value: f64,
    painter_registration: crate::painter::PainterRegistration,
    position: crate::Vec2,
    rotation_deg: f64,
    scatter_seed: u32,
    scatter_active: bool,
    scatter_progress: f64,
    settled_sound_emitted: bool,
    source: crate::loot::NativeLootDropSource,
    spawn_tick: u64,
    tier: u32,
}

#[cfg(feature = "persistence")]
impl From<LootActorFields> for BoneyardLootActor {
    fn from(row: LootActorFields) -> Self {
        Self {
            body: LootBody {
                activation_delay_ticks: row.activation_delay_ticks,
                age_ticks: row.age_ticks,
                alpha: row.alpha,
                animation_phase: row.animation_phase,
                bounce_height: row.bounce_height,
                bounce_velocity: row.bounce_velocity,
                frame_phase: row.frame_phase,
                id: row.id,
                kind: row.kind,
                last_step_tick: row.last_step_tick,
                lifetime_ticks_remaining: row.lifetime_ticks_remaining,
                painter_registration: row.painter_registration,
                position: row.position,
                rotation_deg: row.rotation_deg,
                scatter_seed: row.scatter_seed,
                scatter_active: row.scatter_active,
                scatter_progress: row.scatter_progress,
                settled_sound_emitted: row.settled_sound_emitted,
                spawn_tick: row.spawn_tick,
            },
            reward: LootReward {
                amount: row.amount,
                bonus_kind: row.bonus_kind,
                item: row.item,
                native_type_id: row.native_type_id,
                orb_kind: row.orb_kind,
                orb_value: row.orb_value,
                source: row.source,
                tier: row.tier,
            },
        }
    }
}
