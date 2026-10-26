//! Keyed participant components. Sorted lookup order and gameplay admission order
//! are distinct: the session supplies its explicit order to movement and tick phases.

use std::fmt;

use bevy_ecs::component::Component;
use bevy_ecs::query::QueryItem;
use bevy_ecs::world::EntityRef;

use super::*;
use crate::ecs::KeyedEntities;

#[derive(Component)]
struct ParticipantIdentity {
    id: String,
    entity_id: u64,
}

#[derive(Component)]
struct ParticipantCharacter {
    player: Box<PlayerState>,
}

#[derive(Component)]
struct ParticipantHubState {
    college_intro: Option<NativeCollegeIntroState>,
    marker_epoch: HubNpcMarkerEpoch,
    tutorial_profile_economy: Option<Box<HubEconomyState>>,
}

#[derive(Component)]
struct ParticipantMotion {
    wizard: HubWizard,
    world_aim: Option<Vec2>,
}

/// Borrowed fields preserve the participant contract without copying its profile.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(rename = "SessionParticipant"))]
pub struct ParticipantRef<'a> {
    pub id: &'a String,
    pub entity_id: u64,
    pub player: &'a PlayerState,
    pub wizard: &'a HubWizard,
    pub lighting: &'a PlayerLightingState,
    pub college_intro: Option<NativeCollegeIntroState>,
    pub marker_epoch: &'a HubNpcMarkerEpoch,
    pub score: &'a NativeHallOfFameRunState,
    pub screen_feedback: &'a NativeSecondaryScreenFeedbackPresentation,
    pub world_aim: Option<Vec2>,
    pub tutorial_profile_economy: &'a Option<Box<HubEconomyState>>,
}

pub struct ParticipantMut<'a> {
    pub id: &'a mut String,
    pub entity_id: &'a mut u64,
    pub player: &'a mut PlayerState,
    pub wizard: &'a mut HubWizard,
    pub lighting: &'a mut PlayerLightingState,
    pub college_intro: &'a mut Option<NativeCollegeIntroState>,
    pub marker_epoch: &'a mut HubNpcMarkerEpoch,
    pub score: &'a mut NativeHallOfFameRunState,
    pub screen_feedback: &'a mut NativeSecondaryScreenFeedbackPresentation,
    pub world_aim: &'a mut Option<Vec2>,
    pub tutorial_profile_economy: &'a mut Option<Box<HubEconomyState>>,
}

pub trait ParticipantRead {
    fn view(&self) -> ParticipantRef<'_>;
}

impl ParticipantRead for SessionParticipant {
    fn view(&self) -> ParticipantRef<'_> {
        ParticipantRef {
            id: &self.id,
            entity_id: self.entity_id,
            player: &self.player,
            wizard: &self.wizard,
            lighting: &self.lighting,
            college_intro: self.college_intro,
            marker_epoch: &self.marker_epoch,
            score: &self.score,
            screen_feedback: &self.screen_feedback,
            world_aim: self.world_aim,
            tutorial_profile_economy: &self.tutorial_profile_economy,
        }
    }
}

impl ParticipantRead for ParticipantRef<'_> {
    fn view(&self) -> ParticipantRef<'_> {
        *self
    }
}

impl ParticipantRead for ParticipantMut<'_> {
    fn view(&self) -> ParticipantRef<'_> {
        ParticipantRef {
            id: self.id,
            entity_id: *self.entity_id,
            player: self.player,
            wizard: self.wizard,
            lighting: self.lighting,
            college_intro: *self.college_intro,
            marker_epoch: self.marker_epoch,
            score: self.score,
            screen_feedback: self.screen_feedback,
            world_aim: *self.world_aim,
            tutorial_profile_economy: self.tutorial_profile_economy,
        }
    }
}

impl SessionParticipant {
    pub fn view_mut(&mut self) -> ParticipantMut<'_> {
        ParticipantMut {
            id: &mut self.id,
            entity_id: &mut self.entity_id,
            player: &mut self.player,
            wizard: &mut self.wizard,
            lighting: &mut self.lighting,
            college_intro: &mut self.college_intro,
            marker_epoch: &mut self.marker_epoch,
            score: &mut self.score,
            screen_feedback: &mut self.screen_feedback,
            world_aim: &mut self.world_aim,
            tutorial_profile_economy: &mut self.tutorial_profile_economy,
        }
    }
}

impl ParticipantRef<'_> {
    pub fn snapshot(self) -> SessionParticipant {
        SessionParticipant {
            id: self.id.clone(),
            entity_id: self.entity_id,
            player: Box::new(self.player.clone()),
            wizard: self.wizard.clone(),
            lighting: self.lighting.clone(),
            college_intro: self.college_intro,
            marker_epoch: *self.marker_epoch,
            score: self.score.clone(),
            screen_feedback: self.screen_feedback.clone(),
            world_aim: self.world_aim,
            tutorial_profile_economy: self.tutorial_profile_economy.clone(),
        }
    }

    pub(in crate::session) fn selected_primary_id(self) -> u32 {
        if self.player.book.primary_skill_id == 52 {
            self.player.book.weld_build_id.expect("selected Weld has its build")
        } else {
            self.player.book.primary_skill_id as u32
        }
    }
}

type ParticipantQuery = (
    &'static mut ParticipantIdentity,
    &'static mut ParticipantCharacter,
    &'static mut ParticipantMotion,
    &'static mut PlayerLightingState,
    &'static mut ParticipantHubState,
    &'static mut NativeHallOfFameRunState,
    &'static mut NativeSecondaryScreenFeedbackPresentation,
);

impl<'a> ParticipantMut<'a> {
    pub(in crate::session) fn selected_primary_id(&self) -> u32 {
        self.view().selected_primary_id()
    }

    pub(in crate::session) fn synchronize_primary_selection(&mut self, previous: u32) {
        let selected = self.selected_primary_id();
        if selected != previous {
            self.wizard.primary_cast.reset_for_selection(selected);
        }
    }

    /// Replace a completed transaction without retiring its live entity.
    pub fn replace(&mut self, record: SessionParticipant) -> SessionParticipant {
        assert_eq!(*self.id, record.id, "participant identity is unchanged by a transaction");
        let mut player = record.player;
        std::mem::swap(self.player, &mut player);
        SessionParticipant {
            id: std::mem::replace(self.id, record.id),
            entity_id: std::mem::replace(self.entity_id, record.entity_id),
            player,
            wizard: std::mem::replace(self.wizard, record.wizard),
            lighting: std::mem::replace(self.lighting, record.lighting),
            college_intro: std::mem::replace(self.college_intro, record.college_intro),
            marker_epoch: std::mem::replace(self.marker_epoch, record.marker_epoch),
            score: std::mem::replace(self.score, record.score),
            screen_feedback: std::mem::replace(self.screen_feedback, record.screen_feedback),
            world_aim: std::mem::replace(self.world_aim, record.world_aim),
            tutorial_profile_economy: std::mem::replace(
                self.tutorial_profile_economy,
                record.tutorial_profile_economy,
            ),
        }
    }

    fn from_query(components: QueryItem<'a, 'static, ParticipantQuery>) -> Self {
        let (identity, character, motion, lighting, hub, score, screen_feedback) = components;
        let identity = identity.into_inner();
        let character = character.into_inner();
        let motion = motion.into_inner();
        let lighting = lighting.into_inner();
        let hub = hub.into_inner();
        let score = score.into_inner();
        let screen_feedback = screen_feedback.into_inner();
        Self {
            id: &mut identity.id,
            entity_id: &mut identity.entity_id,
            player: &mut character.player,
            wizard: &mut motion.wizard,
            lighting,
            college_intro: &mut hub.college_intro,
            marker_epoch: &mut hub.marker_epoch,
            score,
            screen_feedback,
            world_aim: &mut motion.world_aim,
            tutorial_profile_economy: &mut hub.tutorial_profile_economy,
        }
    }
}

#[derive(Default)]
pub struct Participants {
    entities: KeyedEntities<ParticipantQuery>,
}

impl Participants {
    pub fn len(&self) -> usize {
        self.entities.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
    pub fn contains_key(&self, id: &str) -> bool {
        self.entities.contains_key(id)
    }
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &String> + ExactSizeIterator + Clone {
        self.entities.keys()
    }
    pub fn get(&self, id: &str) -> Option<ParticipantRef<'_>> {
        self.entities.get(id).map(Self::read)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<ParticipantMut<'_>> {
        self.entities.get_mut(id).map(ParticipantMut::from_query)
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&String, ParticipantRef<'_>)> + ExactSizeIterator + Clone
    {
        self.entities.iter().map(|(id, entity)| (id, Self::read(entity)))
    }
    pub fn values(
        &self,
    ) -> impl DoubleEndedIterator<Item = ParticipantRef<'_>> + ExactSizeIterator + Clone {
        self.iter().map(|(_, participant)| participant)
    }
    pub fn values_mut(&mut self) -> impl Iterator<Item = ParticipantMut<'_>> {
        self.entities.iter_mut().map(|(_, participant)| ParticipantMut::from_query(participant))
    }
    fn read(entity: EntityRef<'_>) -> ParticipantRef<'_> {
        let identity =
            entity.get::<ParticipantIdentity>().expect("participant retains its identity");
        let character =
            entity.get::<ParticipantCharacter>().expect("participant retains its character");
        let motion = entity.get::<ParticipantMotion>().expect("participant retains its motion");
        let lighting =
            entity.get::<PlayerLightingState>().expect("participant retains its lighting");
        let hub = entity.get::<ParticipantHubState>().expect("participant retains its hub");
        let score =
            entity.get::<NativeHallOfFameRunState>().expect("participant retains its score");
        let screen_feedback = entity
            .get::<NativeSecondaryScreenFeedbackPresentation>()
            .expect("participant retains its screen feedback");
        ParticipantRef {
            id: &identity.id,
            entity_id: identity.entity_id,
            player: &character.player,
            wizard: &motion.wizard,
            lighting,
            college_intro: hub.college_intro,
            marker_epoch: &hub.marker_epoch,
            score,
            screen_feedback,
            world_aim: motion.world_aim,
            tutorial_profile_economy: &hub.tutorial_profile_economy,
        }
    }

    pub fn insert(&mut self, id: String, record: SessionParticipant) -> Option<SessionParticipant> {
        assert_eq!(id, record.id, "participant key matches its identity");
        if let Some(mut participant) = self.get_mut(&id) {
            return Some(participant.replace(record));
        }
        self.entities.spawn(id, |world| {
            world
                .spawn((
                    ParticipantIdentity { id: record.id, entity_id: record.entity_id },
                    ParticipantCharacter { player: record.player },
                    ParticipantMotion { wizard: record.wizard, world_aim: record.world_aim },
                    record.lighting,
                    ParticipantHubState {
                        college_intro: record.college_intro,
                        marker_epoch: record.marker_epoch,
                        tutorial_profile_economy: record.tutorial_profile_economy,
                    },
                    record.score,
                    record.screen_feedback,
                ))
                .id()
        });
        None
    }

    pub fn remove(&mut self, id: &str) -> Option<SessionParticipant> {
        self.entities.remove(id, |entity| {
            let (identity, character, motion, lighting, hub, score, screen_feedback) = entity
                .take::<(
                    ParticipantIdentity,
                    ParticipantCharacter,
                    ParticipantMotion,
                    PlayerLightingState,
                    ParticipantHubState,
                    NativeHallOfFameRunState,
                    NativeSecondaryScreenFeedbackPresentation,
                )>()
                .expect("participant retains its components");
            SessionParticipant {
                id: identity.id,
                entity_id: identity.entity_id,
                player: character.player,
                wizard: motion.wizard,
                lighting,
                college_intro: hub.college_intro,
                marker_epoch: hub.marker_epoch,
                score,
                screen_feedback,
                world_aim: motion.world_aim,
                tutorial_profile_economy: hub.tutorial_profile_economy,
            }
        })
    }

    pub fn records(&self) -> std::collections::BTreeMap<String, SessionParticipant> {
        self.iter().map(|(id, participant)| (id.clone(), participant.snapshot())).collect()
    }
}

impl FromIterator<(String, SessionParticipant)> for Participants {
    fn from_iter<I: IntoIterator<Item = (String, SessionParticipant)>>(records: I) -> Self {
        let mut result = Self::default();
        for (id, record) in records {
            result.insert(id, record);
        }
        result
    }
}
impl Clone for Participants {
    fn clone(&self) -> Self {
        self.iter().map(|(id, participant)| (id.clone(), participant.snapshot())).collect()
    }
}
impl fmt::Debug for Participants {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for Participants {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.iter())
    }
}
