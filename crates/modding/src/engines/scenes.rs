//! Nested room ownership and portal admission from `mod-scene-engine.ts` and
//! `mod-portal-engine.ts`. The same scene engine receives every admitted portal.

use std::collections::HashSet;
use std::sync::Arc;

use solomon_sim::modding::LuaValue;
use solomon_sim::modding::content::PreparedModContentCatalog;
use solomon_sim::modding::definition::WebLuaContentKind;
use solomon_sim::modding::runtime::{
    ActiveModScene, ModPortalPolicy, ModSceneCheckpoint, ModSceneStack, PreparedModPortal,
};

use super::MAXIMUM_SAFE_INTEGER;
use crate::definition::values::typed_value;
use crate::json::SourceCollator;

pub struct ModSceneEngine {
    catalog: Arc<PreparedModContentCatalog>,
    state: ModSceneCheckpoint,
    collator: SourceCollator,
}

impl ModSceneEngine {
    pub fn new(catalog: Arc<PreparedModContentCatalog>) -> Self {
        Self { catalog, state: ModSceneCheckpoint::default(), collator: SourceCollator::default() }
    }

    pub fn checkpoint(&self) -> ModSceneCheckpoint {
        ModSceneCheckpoint { scenes: self.project(), ..self.state.clone() }
    }

    pub fn enter(
        &mut self,
        owner_id: &str,
        scene_content_id: &str,
    ) -> Result<ActiveModScene, String> {
        if self.catalog.scene(scene_content_id).is_none() {
            return Err(format!("mod scene is unavailable: {scene_content_id}"));
        }
        let active = self.state.scenes.iter().find(|scene| scene.owner_id == owner_id).cloned();
        let stack_index = self.state.stacks.iter().position(|stack| stack.owner_id == owner_id);
        let mut new_stack = ModSceneStack {
            owner_id: owner_id.into(),
            room_indexes: Vec::new(),
            scene_content_ids: Vec::new(),
        };
        let stack = if let Some(index) = stack_index {
            &mut self.state.stacks[index]
        } else {
            &mut new_stack
        };
        if let Some(active) = &active {
            stack.room_indexes.push(active.room_index);
            stack.scene_content_ids.push(active.scene_content_id.clone())
        }
        if stack.scene_content_ids.len() > 8 {
            return Err("mod scene stack limit reached".into());
        }
        if stack_index.is_none() {
            self.state.stacks.push(new_stack)
        }
        let next = ActiveModScene {
            epoch: self.state.next_epoch,
            owner_id: owner_id.into(),
            parent_content_id: active.map(|active| active.scene_content_id),
            room_index: 0,
            scene_content_id: scene_content_id.into(),
        };
        self.state.next_epoch += 1;
        self.set_scene(next.clone());
        Ok(next)
    }

    pub fn project(&self) -> Vec<ActiveModScene> {
        let mut scenes = self.state.scenes.clone();
        scenes.sort_by(|left, right| self.collator.compare(&left.owner_id, &right.owner_id));
        scenes
    }

    pub fn active(&self, owner_id: &str) -> Option<&ActiveModScene> {
        self.state.scenes.iter().find(|scene| scene.owner_id == owner_id)
    }

    pub fn restore(&mut self, checkpoint: ModSceneCheckpoint) -> Result<(), String> {
        if checkpoint.next_epoch == 0 || checkpoint.next_epoch > MAXIMUM_SAFE_INTEGER {
            return Err("mod scene checkpoint is invalid".into());
        }
        let mut owners = HashSet::new();
        for scene in &checkpoint.scenes {
            if !owners.insert(&scene.owner_id)
                || self
                    .catalog
                    .scene(&scene.scene_content_id)
                    .is_none_or(|definition| scene.room_index >= definition.rooms.len() as u64)
                || scene.epoch == 0
                || scene.epoch > MAXIMUM_SAFE_INTEGER
            {
                return Err("mod scene checkpoint contains an invalid scene".into());
            }
        }
        owners.clear();
        for stack in &checkpoint.stacks {
            if !owners.insert(&stack.owner_id)
                || stack.scene_content_ids.len() > 8
                || stack.scene_content_ids.len() != stack.room_indexes.len()
                || stack.scene_content_ids.iter().zip(&stack.room_indexes).any(
                    |(content_id, room)| {
                        self.catalog
                            .scene(content_id)
                            .is_none_or(|definition| *room >= definition.rooms.len() as u64)
                    },
                )
            {
                return Err("mod scene checkpoint contains an invalid stack".into());
            }
        }
        self.state = checkpoint;
        Ok(())
    }

    pub fn return_scene(&mut self, owner_id: &str) -> Option<ActiveModScene> {
        let stack_index = self.state.stacks.iter().position(|stack| stack.owner_id == owner_id);
        let parent = stack_index.and_then(|index| {
            let stack = &mut self.state.stacks[index];
            stack.scene_content_ids.pop().map(|content_id| {
                (content_id, stack.room_indexes.pop().expect("scene stack has paired room indexes"))
            })
        });
        let Some((content_id, room_index)) = parent else {
            self.state.scenes.retain(|scene| scene.owner_id != owner_id);
            self.state.stacks.retain(|stack| stack.owner_id != owner_id);
            return None;
        };
        let parent_content_id = stack_index
            .and_then(|index| self.state.stacks[index].scene_content_ids.last().cloned());
        let next = ActiveModScene {
            epoch: self.state.next_epoch,
            owner_id: owner_id.into(),
            parent_content_id,
            room_index,
            scene_content_id: content_id,
        };
        self.state.next_epoch += 1;
        self.set_scene(next.clone());
        Some(next)
    }

    pub fn select_room(
        &mut self,
        owner_id: &str,
        room_index: usize,
    ) -> Result<ActiveModScene, String> {
        let index = self
            .state
            .scenes
            .iter()
            .position(|scene| scene.owner_id == owner_id)
            .ok_or("mod scene room is unavailable")?;
        let active = &self.state.scenes[index];
        if self
            .catalog
            .scene(&active.scene_content_id)
            .is_none_or(|definition| room_index >= definition.rooms.len())
        {
            return Err("mod scene room is unavailable".into());
        }
        if active.room_index == room_index as u64 {
            return Ok(active.clone());
        }
        self.state.scenes[index].epoch = self.state.next_epoch;
        self.state.scenes[index].room_index = room_index as u64;
        self.state.next_epoch += 1;
        Ok(self.state.scenes[index].clone())
    }

    fn set_scene(&mut self, scene: ActiveModScene) {
        if let Some(index) = self.state.scenes.iter().position(|row| row.owner_id == scene.owner_id)
        {
            self.state.scenes[index] = scene
        } else {
            self.state.scenes.push(scene)
        }
    }
}

pub struct ModPortalEngine {
    portals: Vec<PreparedModPortal>,
}

impl ModPortalEngine {
    pub fn new(catalog: &PreparedModContentCatalog) -> Result<Self, String> {
        let mut portals = Vec::new();
        for extension in catalog.scene_extensions() {
            for (index, feature) in extension.features.iter().enumerate() {
                if feature.operation != "prefab.portal" {
                    continue;
                }
                let selector =
                    feature.fields.get("selector").and_then(LuaValue::as_object).ok_or_else(
                        || {
                            format!(
                                "{}.features[{index}].selector must be an object",
                                extension.common.key
                            )
                        },
                    )?;
                let destination = feature
                    .fields
                    .get("destination")
                    .filter(|value| {
                        value.as_object().is_some_and(|fields| {
                            fields.get("kind").and_then(LuaValue::as_str)
                                == Some("resolved-content-reference")
                        })
                    })
                    .ok_or_else(|| {
                        format!(
                            "{}.features[{index}].destination must be a resolved reference",
                            extension.common.key
                        )
                    })?;
                let destination = typed_value::<
                    solomon_sim::modding::definition::ResolvedWebLuaContentReference,
                >(destination)?;
                if destination.target_kind != WebLuaContentKind::Scene
                    || catalog.scene(&destination.content_id).is_none()
                {
                    return Err(format!(
                        "{}:{} portal destination is unavailable",
                        extension.common.mod_id, extension.common.key
                    ));
                }
                let policy = match feature.fields.get("policy") {
                    None | Some(LuaValue::Null) => ModPortalPolicy::LeaderConfirms,
                    Some(LuaValue::String(value)) if value == "leader_confirms" => {
                        ModPortalPolicy::LeaderConfirms
                    }
                    Some(LuaValue::String(value)) if value == "any_member" => {
                        ModPortalPolicy::AnyMember
                    }
                    _ => {
                        return Err(format!(
                            "{}:{} portal policy is invalid",
                            extension.common.mod_id, extension.common.key
                        ));
                    }
                };
                let prompt =
                    feature.fields.get("prompt").filter(|value| !matches!(value, LuaValue::Null));
                portals.push(PreparedModPortal {
                    destination,
                    id: format!("{}:{index}", extension.common.content_id),
                    object_kind: portal_text(selector.get("object_kind"), "portal object_kind")?,
                    policy,
                    prompt: prompt.map_or_else(
                        || Ok("Enter".into()),
                        |value| portal_text(Some(value), "portal prompt"),
                    )?,
                    scene: extension.scene.clone(),
                });
            }
        }
        Ok(Self { portals })
    }

    pub fn portals(&self) -> &[PreparedModPortal] {
        &self.portals
    }

    pub fn activate(
        &self,
        scenes: &mut ModSceneEngine,
        input: ModPortalActivation<'_>,
    ) -> Result<ActiveModScene, String> {
        let portal = self
            .portals
            .iter()
            .find(|portal| {
                portal.id == input.portal_id
                    && portal.scene == input.scene
                    && portal.object_kind == input.actor_kind
            })
            .ok_or("mod portal is unavailable")?;
        if portal.policy == ModPortalPolicy::LeaderConfirms && !input.confirmed_by_leader {
            return Err("mod portal requires leader confirmation".into());
        }
        scenes.enter(input.owner_id, &portal.destination.content_id)
    }
}

pub struct ModPortalActivation<'a> {
    pub actor_kind: &'a str,
    pub confirmed_by_leader: bool,
    pub owner_id: &'a str,
    pub portal_id: &'a str,
    pub scene: &'a str,
}

fn portal_text(value: Option<&LuaValue>, field: &str) -> Result<String, String> {
    value
        .and_then(LuaValue::as_str)
        .filter(|value| !value.is_empty() && value.encode_utf16().count() <= 128)
        .map(str::to_owned)
        .ok_or_else(|| format!("{field} is invalid"))
}
