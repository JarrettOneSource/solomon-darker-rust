//! Actual accepted host rows from `prepared-mod-host.ts::project/runtimeProjection`.
//! These functions only borrow content and live engines; no view owns authority.

use serde_json::{Value, json};
use solomon_sim::modding::assets::{
    ModSpriteFrame, PreparedModAsset, PreparedModAudioAsset, PreparedModSpriteAsset,
};
use solomon_sim::modding::content::{PreparedModContentDefinition, PreparedModContentEntry};
use solomon_sim::modding::definition::WebLuaAssetKind;

use crate::json::{SourceCollator, lua_value};

use super::*;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PreparedModRuntimeProjection {
    pub projection: LuaObject,
    pub revision: u64,
}

impl PreparedModHost {
    pub fn projection_revision(&self) -> Result<u64, String> {
        self.require_open()?;
        Ok(self.runtime.statuses.revision()
            + self.runtime.powerups.revision()
            + self.runtime.skills.revision()
            + self.runtime.semantic_state.revision()
            + self.runtime.spell_effects.revision())
    }

    pub fn project(&self) -> Result<LuaObject, String> {
        self.require_open()?;
        let collator = SourceCollator::default();
        let mut content = Vec::new();
        for definition in self.runtime.content.all() {
            let entry = definition.common();
            let mut art: Vec<_> = entry.art.iter().collect();
            art.sort_by(|(left, _), (right, _)| collator.compare(left, right));
            let art: Vec<_> =
                art.iter().map(|(slot, art)| json!({"path":art.path,"slot":slot})).collect();
            let presentation = if matches!(definition, PreparedModContentDefinition::Ui(_)) {
                entry
                    .fields
                    .get("view")
                    .and_then(LuaValue::as_object)
                    .and_then(|view| view.get("operation"))
                    .and_then(LuaValue::as_str)
            } else {
                None
            };
            content.push(json!({
                "art":art,"contentId":entry.content_id,"contentKind":definition.kind().as_str(),
                "description":entry.description,"key":entry.key,"modId":entry.mod_id,
                "name":entry.name,"presentation":presentation,
            }));
        }
        let mut boasts = Vec::new();
        for definition in self.runtime.content.boasts() {
            boasts.push(json!({
                "contentId":definition.common.content_id,"failureProducers":definition.failure_producers,
                "icon":definition.icon,"instruction":definition.instruction,"modId":definition.common.mod_id,
                "name":definition.label,"randomSkillChoices":definition.random_skill_choices,
                "response":definition.response,"scoreMultiplier":definition.score_multiplier,
                "statement":definition.statement,"successWave":definition.success_wave,
            }));
        }
        let mut powerups = Vec::new();
        for powerup in self.runtime.powerups.project() {
            powerups.push(json!({"contentId":powerup.content_id,"id":powerup.id,"spawnedTick":powerup.spawned_tick,"x":powerup.x,"y":powerup.y}));
        }
        let mut statuses = Vec::new();
        for status in self.runtime.statuses.project(None) {
            statuses.push(json!({
                "contentId":status.content_id,"expiresTick":status.expires_tick,"instanceId":status.instance_id,
                "startedTick":status.started_tick,"targetId":status.target_id,
            }));
        }
        row(json!({"boasts":boasts,"content":content,"manifestSha256":self.manifest_sha256,
            "powerups":powerups,"revision":self.projection_revision()?,"statuses":statuses}))
    }

    pub fn runtime_projection(
        &self,
        viewer_id: Option<&str>,
        game: &GameSession,
    ) -> Result<PreparedModRuntimeProjection, String> {
        self.require_open()?;
        let checkpoint = self.checkpoint()?;
        let mut projection = LuaObject::new();
        let active = if let Some(id) = &self.active_boneyard_content_id {
            let definition =
                self.runtime.content.boneyard(id).expect("active Boneyard was admitted");
            object_value([
                ("anchors", LuaValue::Object(definition.anchors.clone())),
                ("content_id", LuaValue::String(id.clone())),
                ("environment", LuaValue::Object(definition.environment.clone())),
            ])
        } else {
            LuaValue::Null
        };
        projection.insert("active_boneyard".into(), active);
        projection.insert("audio_loops".into(), lua_value(&self.audio_loops(&checkpoint)?)?);
        let mut enemies = Vec::new();
        for enemy in &checkpoint.enemies.enemies {
            enemies.push(self.enemy_row(enemy, game.ticks)?);
        }
        projection.insert("enemies".into(), lua_value(&enemies)?);
        let portals: Vec<_> = self.runtime.portals.portals().iter().map(|portal| json!({
            "id":portal.id,"policy":portal.policy,"prompt":portal.prompt,"scene":portal.scene,
        })).collect();
        projection.insert("portals".into(), lua_value(&portals)?);
        let mut powerups = Vec::new();
        for powerup in &checkpoint.powerups.instances {
            powerups.push(self.powerup_row(powerup, game.ticks)?);
        }
        projection.insert("powerup_actors".into(), lua_value(&powerups)?);
        let mut collections = Vec::new();
        for event in self.runtime.powerups.project_collections() {
            collections.push(self.collection_row(event)?);
        }
        projection.insert("powerup_events".into(), lua_value(&collections)?);
        projection.insert("presentation_events".into(), lua_value(&self.presentation_audio()?)?);
        let mut scenes = Vec::new();
        for scene in &checkpoint.scenes.scenes {
            let definition = self
                .runtime
                .content
                .scene(&scene.scene_content_id)
                .expect("active scene references accepted content");
            let mut rooms = Vec::new();
            for reference in &definition.rooms {
                let room = self
                    .runtime
                    .content
                    .room(&reference.content_id)
                    .expect("accepted scene references an accepted room");
                rooms.push(json!({"content_id":room.common.content_id,"description":room.common.description,
                    "geometry":room.geometry,"name":room.common.name,
                    "props":room.common.fields.get("props").cloned().unwrap_or_else(||LuaValue::Array(Vec::new()))}));
            }
            scenes.push(json!({"epoch":scene.epoch,"mod_id":definition.common.mod_id,"owner_id":scene.owner_id,
                "parent_content_id":scene.parent_content_id,"room_index":scene.room_index,"rooms":rooms,"scene_content_id":scene.scene_content_id}));
        }
        projection.insert("scenes".into(), lua_value(&scenes)?);
        let state: Vec<_> = self.runtime.semantic_state.project(viewer_id).iter().map(|value| json!({
            "key":value.key,"mod_id":value.mod_id,"scope_id":value.scope.id,"scope_kind":value.scope.kind,"value":value.value,
        })).collect();
        projection.insert("state_values".into(), lua_value(&state)?);
        let mut shops = Vec::new();
        for entry in self.runtime.content.all() {
            let PreparedModContentDefinition::Shop(shop) = entry else { continue };
            let services: Vec<_> = shop.services.iter().map(|service| json!({"pool_content_id":service.pool.content_id,"price":service.price,"type":"reforge"})).collect();
            let stock: Vec<_> = shop.stock.iter().map(|stock| json!({"item_content_id":stock.item.content_id,"price":stock.price,"quantity":stock.quantity})).collect();
            shops.push(json!({"content_id":shop.common.content_id,
                "icon_path":shop.common.art.get("npc").or_else(||shop.common.art.get("icon")).map(|art|&art.path),
                "mod_id":shop.common.mod_id,"mount":shop.mount,"name":shop.common.name,"npc":shop.npc,
                "restock_ms":shop.restock_ms,"services":services,"stock":stock}));
        }
        projection.insert("shops".into(), lua_value(&shops)?);
        let stock = if viewer_id.is_some() {
            self.runtime.shops.project(viewer_id)
        } else {
            checkpoint.shops.stock.clone()
        };
        let stock: Vec<_> = stock.iter().map(|stock| json!({"player_id":stock.player_id,"remaining":stock.remaining,
            "restock_tick":stock.restock_tick,"row":stock.row,"shop_content_id":stock.shop_content_id})).collect();
        projection.insert("shop_stock".into(), lua_value(&stock)?);
        let skills: Vec<_> = self.runtime.content.skills().map(|skill| json!({
            "content_id":skill.common.content_id,"description":skill.common.description,
            "icon_path":skill.common.art.get("icon").map(|art|&art.path),"maximum_rank":skill.maximum_rank,
            "minimum_level":skill.minimum_level,"mod_id":skill.common.mod_id,"name":skill.common.name,
            "parent_content_id":skill.parent.as_ref().map(|parent|&parent.content_id),
        })).collect();
        projection.insert("skills".into(), lua_value(&skills)?);
        let ranks = if viewer_id.is_some() {
            self.runtime.skills.project(viewer_id)
        } else {
            checkpoint.skills.ranks.clone()
        };
        let ranks: Vec<_> = ranks.iter().map(|rank| json!({"content_id":rank.content_id,"player_id":rank.player_id,"rank":rank.rank})).collect();
        projection.insert("skill_ranks".into(), lua_value(&ranks)?);
        let offers = if viewer_id.is_some() {
            self.runtime.skills.offers(viewer_id)
        } else {
            checkpoint.skills.offers.clone()
        };
        let offers: Vec<_> = offers.iter().map(|offer| json!({"content_ids":offer.content_ids,"player_id":offer.player_id,"sequence":offer.sequence})).collect();
        projection.insert("skill_offers".into(), lua_value(&offers)?);
        let mut spells = Vec::new();
        for spell in self.runtime.content.spells() {
            if let Some(viewer) = viewer_id
                && !self.runtime.skills.spell_unlocked(viewer, &spell.common.content_id)?
            {
                continue;
            }
            spells.push(json!({"content_id":spell.common.content_id,"icon_path":spell.common.art.get("icon").map(|art|&art.path),
                "mana":spell.mana,"mod_id":spell.common.mod_id,"name":spell.common.name,"slot":spell.slot}));
        }
        projection.insert("spells".into(), lua_value(&spells)?);
        let bindings = if viewer_id.is_some() {
            self.runtime.skills.project_bindings(viewer_id)
        } else {
            checkpoint.skills.bindings.clone()
        };
        let bindings: Vec<_> = bindings.iter().map(|binding| json!({"content_id":binding.content_id,"player_id":binding.player_id,"slot":binding.slot})).collect();
        projection.insert("mod_quickbar".into(), lua_value(&bindings)?);
        let cooldowns = if viewer_id.is_some() {
            self.runtime.spells.project(viewer_id)
        } else {
            checkpoint.spells.cooldowns.clone()
        };
        let cooldowns: Vec<_> = cooldowns.iter().map(|cooldown| json!({"content_id":cooldown.content_id,"player_id":cooldown.player_id,"ready_tick":cooldown.ready_tick})).collect();
        projection.insert("spell_cooldowns".into(), lua_value(&cooldowns)?);
        let mut effects = Vec::new();
        for effect in &checkpoint.spell_effects.effects {
            effects.push(self.spell_effect_row(effect, game.ticks)?);
        }
        projection.insert("spell_effects".into(), lua_value(&effects)?);
        let mut surfaces = Vec::new();
        for definition in self.runtime.content.uis() {
            if self.ui_visible(definition, viewer_id, game) {
                surfaces.push(json!({"accessible_name":definition.accessible_name,"actions":definition.actions,
                    "bindings":self.ui_bindings(definition, viewer_id),"content_id":definition.common.content_id,
                    "mount":definition.mount,"view":{"fields":definition.view.fields,"operation":definition.view.operation}}));
            }
        }
        projection.insert("ui_surfaces".into(), lua_value(&surfaces)?);
        Ok(PreparedModRuntimeProjection {
            projection,
            revision: checkpoint.enemies.revision
                + checkpoint.scenes.next_epoch
                + checkpoint.semantic_state.revision
                + checkpoint.skills.revision
                + checkpoint.spells.revision
                + checkpoint.spell_effects.revision
                + checkpoint.shops.revision,
        })
    }

    fn image<'a>(
        &'a self,
        entry: &PreparedModContentEntry,
        slots: &[&str],
        error: &str,
    ) -> Result<&'a PreparedModSpriteAsset, String> {
        let binding =
            slots.iter().find_map(|slot| entry.art.get(*slot)).ok_or_else(|| error.to_owned())?;
        match self.runtime.assets.get(&entry.mod_id, &binding.key) {
            Some(PreparedModAsset::Image(image)) => Ok(image),
            _ => Err(error.into()),
        }
    }
    fn sound<'a>(
        &'a self,
        entry: &PreparedModContentEntry,
        slots: &[&str],
        error: &str,
    ) -> Result<Option<&'a PreparedModAudioAsset>, String> {
        let Some(binding) = slots.iter().find_map(|slot| entry.art.get(*slot)) else {
            return Ok(None);
        };
        match self.runtime.assets.get(&entry.mod_id, &binding.key) {
            Some(PreparedModAsset::Audio(sound)) => Ok(Some(sound)),
            Some(_) => Err(error.into()),
            None => Ok(None),
        }
    }
    fn frame(
        &self,
        image: &PreparedModSpriteAsset,
        names: &[&str],
        ordinal: usize,
        clamp: bool,
        heading: usize,
    ) -> ModSpriteFrame {
        let animation = names
            .iter()
            .find_map(|name| image.animations.get(*name).map(Vec::as_slice))
            .or_else(|| self.runtime.assets.first_animation(image));
        let length = animation.map_or(image.frames.len(), <[usize]>::len);
        let offset =
            if clamp { ordinal.min(length.saturating_sub(1)) } else { ordinal % length.max(1) };
        let base =
            animation.map_or(offset, |animation| animation.get(offset).copied().unwrap_or(0));
        image.frames.get(base + heading).copied().unwrap_or(image.frames[0])
    }
    fn enemy_row(&self, enemy: &ActiveModEnemy, tick: u64) -> Result<Value, String> {
        let definition = self
            .runtime
            .content
            .enemy(&enemy.content_id)
            .ok_or("active mod enemy references unavailable content")?;
        let image = self.image(&definition.common, &["atlas"], "mod enemy image is unavailable")?;
        let dying = enemy.life_state == ModEnemyLifeState::Dying;
        let attacking =
            enemy.last_attack_tick.is_some_and(|last| tick as f64 - (last as f64) < 20.0);
        let (names, started): (&[&str], u64) = if dying {
            (&["death"], enemy.death_tick.unwrap())
        } else if attacking {
            (&["attack", "punch"], enemy.last_attack_tick.unwrap())
        } else if enemy.moving {
            (&["move", "run", "walk"], enemy.spawned_tick)
        } else {
            (&["idle"], enemy.spawned_tick)
        };
        let ordinal = ((tick as f64 - started as f64).max(0.0) / 8.0).floor() as usize;
        let heading = image.heading_count.map_or(0, |headings| {
            ((enemy.heading_index as f64 * f64::from(headings) / 16.0).floor() as usize)
                % headings as usize
        });
        let frame = self.frame(image, names, ordinal, dying, heading);
        let sound = if dying {
            self.sound(
                &definition.common,
                &["death_sound", "sound"],
                "mod enemy sound is unavailable",
            )?
        } else if enemy.last_attack_tick.is_some() {
            self.sound(
                &definition.common,
                &["attack_sound", "sound"],
                "mod enemy sound is unavailable",
            )?
        } else {
            None
        };
        Ok(
            json!({"content_id":enemy.content_id,"current_health":enemy.current_health,"death_tick":enemy.death_tick,
            "frame_height":frame.height,"frame_width":frame.width,"frame_x":frame.x,"frame_y":frame.y,
            "heading_index":enemy.heading_index,"id":enemy.id,"image_height":image.height,"image_path":image.identity.path,
            "image_width":image.width,"last_attack_tick":enemy.last_attack_tick,"life_state":enemy.life_state,
            "light_color":0x9c7ad9,"light_radius":72.0*definition.scale,"maximum_health":enemy.maximum_health,
            "mod_id":definition.common.mod_id,"moving":enemy.moving,"name":definition.common.name,"scale":definition.scale,
            "sound_event_tick":if dying{enemy.death_tick}else{enemy.last_attack_tick},
            "sound_path":sound.map(|sound|&sound.identity.path),"sound_volume":sound.map_or(0.0,|sound|sound.volume),
            "target_player_id":enemy.target_player_id,"x":enemy.x,"y":enemy.y}),
        )
    }
    fn powerup_row(&self, powerup: &ActiveModPowerup, tick: u64) -> Result<Value, String> {
        let definition = self
            .runtime
            .content
            .powerup(&powerup.content_id)
            .ok_or("active mod powerup references unavailable content")?;
        let image =
            self.image(&definition.common, &["world"], "mod powerup image is unavailable")?;
        let ordinal = ((tick as f64 - powerup.spawned_tick as f64).max(0.0) / 8.0).floor() as usize;
        let frame = self.frame(image, &["active", "idle"], ordinal, false, 0);
        Ok(
            json!({"content_id":powerup.content_id,"frame_height":frame.height,"frame_width":frame.width,"frame_x":frame.x,"frame_y":frame.y,
            "id":powerup.id,"image_height":image.height,"image_path":image.identity.path,"image_width":image.width,
            "mod_id":definition.common.mod_id,"name":definition.common.name,"spawned_tick":powerup.spawned_tick,"x":powerup.x,"y":powerup.y}),
        )
    }
    fn collection_row(&self, event: &ModPowerupCollectionEvent) -> Result<Value, String> {
        let definition = self
            .runtime
            .content
            .powerup(&event.content_id)
            .ok_or("mod powerup collection references unavailable content")?;
        let sound =
            self.sound(&definition.common, &["sound"], "mod powerup sound is unavailable")?;
        Ok(
            json!({"content_id":event.content_id,"id":event.id,"mod_id":definition.common.mod_id,"player_id":event.player_id,
            "sound_path":sound.map(|sound|&sound.identity.path),"sound_volume":sound.map_or(0.0,|sound|sound.volume),"tick":event.tick,"x":event.x,"y":event.y}),
        )
    }
    fn spell_effect_row(&self, effect: &ActiveModSpellEffect, tick: u64) -> Result<Value, String> {
        let spell = self
            .runtime
            .content
            .spell(&effect.content_id)
            .ok_or("active mod spell effect references unavailable content")?;
        let image = self.image(
            &spell.common,
            &["effect", "icon"],
            "mod spell effect image is unavailable",
        )?;
        let kind = match effect.kind {
            ModSpellEffectKind::Area => "area",
            ModSpellEffectKind::Channel => "channel",
            ModSpellEffectKind::Projectile => "projectile",
        };
        let names = [kind, "active", "idle"];
        let ordinal = ((tick as f64 - effect.started_tick as f64).max(0.0) / 8.0).floor() as usize;
        let frame = self.frame(image, &names, ordinal, false, 0);
        let sound =
            self.sound(&spell.common, &["sound"], "mod spell effect sound is unavailable")?;
        Ok(
            json!({"content_id":effect.content_id,"expires_tick":effect.expires_tick,"frame_height":frame.height,"frame_width":frame.width,
            "frame_x":frame.x,"frame_y":frame.y,"id":effect.id,"image_height":image.height,"image_path":image.identity.path,"image_width":image.width,
            "kind":effect.kind,"mod_id":effect.mod_id,"owner_player_id":effect.owner_player_id,"radius":effect.radius,
            "sound_path":sound.map(|sound|&sound.identity.path),"sound_volume":sound.map_or(0.0,|sound|sound.volume),
            "started_tick":effect.started_tick,"target_x":effect.target_x,"target_y":effect.target_y,"x":effect.x,"y":effect.y}),
        )
    }
    fn audio_loops(&self, checkpoint: &PreparedModHostCheckpoint) -> Result<Vec<Value>, String> {
        let mut owners = Vec::new();
        if let Some(id) = &self.active_boneyard_content_id {
            let definition = self
                .runtime
                .content
                .boneyard(id)
                .expect("active Boneyard references accepted content");
            owners.push((&definition.common, format!("boneyard:{id}")));
        }
        for scene in &checkpoint.scenes.scenes {
            let definition = self
                .runtime
                .content
                .scene(&scene.scene_content_id)
                .expect("active scene references accepted content");
            // Restored scene indexes retain Source's safe-integer width. Only
            // an index admitted against this exact definition can be sampled.
            let reference = usize::try_from(scene.room_index)
                .ok()
                .and_then(|index| definition.rooms.get(index))
                .ok_or("mod scene room is unavailable")?;
            let room = self
                .runtime
                .content
                .room(&reference.content_id)
                .expect("accepted scene references an accepted room");
            owners.push((&definition.common, format!("scene:{}", scene.owner_id)));
            owners.push((&room.common, format!("room:{}:{}", scene.owner_id, scene.room_index)));
        }
        for status in &checkpoint.statuses.instances {
            let definition = self
                .runtime
                .content
                .status(&status.content_id)
                .expect("active status references accepted content");
            owners.push((&definition.common, format!("status:{}", status.instance_id)));
        }
        let mut loops = Vec::new();
        for (entry, owner) in owners {
            for slot in ["music", "ambience", "loop"] {
                let Some(binding) = entry.art.get(slot) else { continue };
                let Some(PreparedModAsset::Audio(audio)) =
                    self.runtime.assets.get(&entry.mod_id, &binding.key)
                else {
                    return Err("mod audio loop is unavailable".into());
                };
                loops.push(json!({"bus":audio.bus,"loop":true,"owner":format!("{owner}:{slot}"),"path":audio.identity.path,"mod_id":entry.mod_id,"volume":audio.volume}));
            }
        }
        Ok(loops)
    }
    fn presentation_audio(&self) -> Result<Vec<Value>, String> {
        let mut events = Vec::new();
        for event in &self.presentation {
            let Some(sound) = event.fields.get("sound").and_then(LuaValue::as_object) else {
                continue;
            };
            if sound.get("kind").and_then(LuaValue::as_str) != Some("asset-reference") {
                continue;
            }
            let Some(key) = sound.get("key").and_then(LuaValue::as_str) else { continue };
            let Some(PreparedModAsset::Audio(sound)) = self.runtime.assets.get(&event.mod_id, key)
            else {
                return Err("mod presentation sound is unavailable".into());
            };
            if sound.asset_kind != WebLuaAssetKind::Sound {
                return Err("mod presentation sound is unavailable".into());
            }
            events.push(json!({"owner":format!("{}:presentation:{}",event.mod_id,event.sequence),"path":sound.identity.path,
                "mod_id":event.mod_id,"sequence":event.sequence,"tick":event.tick,"volume":sound.volume}));
        }
        Ok(events)
    }
}

fn row(value: Value) -> Result<LuaObject, String> {
    match lua_value(&value)? {
        LuaValue::Object(fields) => Ok(fields),
        _ => unreachable!("projection rows are records"),
    }
}
