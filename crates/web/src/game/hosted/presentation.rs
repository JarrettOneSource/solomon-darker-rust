//! Session ownership of authored content and its presentation resources.
//! Accepted snapshots wait for verified textures and audio before their renderer-ready ACK.

use super::*;
use crate::mod_audio::{ModAudioDriver, ModAudioSources};
use crate::mod_content::ModContentCache;
use crate::mod_panels::{ModPanelsPlayer, ModPanelsSceneObject, ModPanelsSnapshot, ModPanelsWorld};
use crate::mod_presentation_assets::PreparedModPresentationAssets;
use crate::mod_world_overlays::{
    ModSceneOverlayFrame, ModScenePartyMember, ModWorldOverlayFrame, ModWorldProjection,
};
use solomon_sim::enemies::model::{EnemyBrain, SpiderPhase};
use solomon_sim::hub::regions::HubRegion;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::presentation::ModWorldLocation;
use solomon_sim::modding::skill_quickbar::ModSkillQuickbarPlayer;
use solomon_sim::modding::ui::{ModMinimapEnemy, ModMinimapPlayer, ModMinimapSnapshot};

pub(super) struct PreparedHostedPresentation {
    images: PreparedModPresentationAssets,
    audio: ModAudioSources,
}

impl PreparedHostedPresentation {
    pub async fn load(
        api: &WebsiteApi,
        assets: &[GameModAsset],
        progress: impl FnMut(crate::mod_content::GameContentDownloadProgress<'_>),
    ) -> Result<Self, String> {
        let cache = ModContentCache::open(api.clone()).await.map_err(|error| error.to_string())?;
        cache.prefetch(assets, progress).await.map_err(|error| error.to_string())?;
        let images = PreparedModPresentationAssets::load(&cache, assets)
            .await
            .map_err(|error| error.to_string())?;
        let audio =
            ModAudioSources::load(&cache, assets).await.map_err(|error| error.to_string())?;
        Ok(Self { images, audio })
    }
}

impl Game {
    pub(super) fn complete_hosted_presentation(
        &mut self,
        result: Result<PreparedHostedPresentation, String>,
    ) -> Result<(), String> {
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => return self.native_disconnected(&error),
        };
        self.mod_assets = match prepared.images.upload(&mut self.renderer) {
            Ok(images) => Some(images),
            Err(error) => return self.native_disconnected(&error),
        };
        for (path, texture) in self.mod_assets.as_ref().unwrap().ui_images() {
            self.gpu_ui.register_image(path, texture);
        }
        self.mod_audio = Some(ModAudioDriver::new(self.audio.assets(), prepared.audio));
        let hosted = self.hosted.as_mut().unwrap();
        hosted.content_pending = false;
        hosted.connecting = false;
        if let Some(frame) = hosted.pending_content_frame.take() {
            self.accept_native_snapshot(frame)?;
        }
        self.update_hosted_presentation(true)
    }

    pub(in crate::game) fn reset_hosted_presentation(&mut self) -> Result<(), String> {
        self.hud.set_mod_belt_icons(None, None)?;
        self.mod_content.reset();
        self.mod_minimap.reset();
        self.mod_panels.reset();
        self.mod_skills.reset();
        self.mod_skill_quickbar.reset()?;
        self.mod_overlays.reset();
        self.mod_audio = None;
        self.mod_projection = None;
        self.mod_viewport_size = None;
        self.battle_views.reset();
        self.world_screen_feedback.reset();
        // Release the wizard's shared custom frame banks before destroying their textures.
        let stock = crate::wizard::WizardEquipment::prepare(
            &mut self.renderer,
            WizardElement::Ether,
            &solomon_sim::economy::HubEquipmentState::default(),
            None,
        )?;
        self.wizard_view.set_equipment(&stock);
        if let Some(mut assets) = self.mod_assets.take() {
            for (path, _) in assets.ui_images() {
                self.gpu_ui.forget_image(path);
            }
            assets.destroy(&mut self.renderer);
        }
        if let Some(hosted) = &mut self.hosted {
            hosted.content_pending = false;
            hosted.pending_content_frame = None;
            hosted.welcome = None;
            hosted.runtime = None;
            hosted.mod_request_sequence = 0;
        }
        Ok(())
    }

    pub(in crate::game) fn restore_hosted_saved_selection(&mut self) {
        if let Some(hosted) = &self.hosted {
            self.native.as_mut().unwrap().set_saved_selection(
                hosted
                    .profile
                    .as_ref()
                    .and_then(|profile| profile.continuation.as_ref())
                    .map(|save| save.summary.character.clone()),
            );
        }
    }

    pub(in crate::game) fn update_hosted_presentation(
        &mut self,
        runtime_updated: bool,
    ) -> Result<(), String> {
        let Some(hosted) = &self.hosted else { return Ok(()) };
        let Some(welcome) = &hosted.welcome else { return Ok(()) };
        if hosted.content_pending {
            return Ok(());
        }
        let player = &self
            .session
            .participants
            .get(&self.local_player_id)
            .expect("participant exists")
            .player;
        self.hud
            .set_mod_belt_icons(Some((&player.belt, &player.economy)), self.mod_assets.as_ref())?;
        if let Some(audio) = &mut self.mod_audio {
            audio.update(
                hosted.runtime.as_ref(),
                &welcome.mod_assets,
                self.session.ticks,
                runtime_updated,
            )?;
        }
        self.refresh_mod_overlays()?;
        self.refresh_mod_gameplay_hud()?;
        self.refresh_hosted_mod_skills()
    }

    fn hosted_gameplay_hud_visible(&self) -> bool {
        if !self.has_game || self.is_hosted_observer() {
            return false;
        }
        let local =
            &self.session.participants.get(&self.local_player_id).expect("participant exists");
        self.session.boneyard.is_some()
            || !solomon_sim::hub::college_intro::hub_college_admission_pre_loadout(
                local.wizard,
                local.player.economy.college_intro_pending,
            )
    }

    pub(in crate::game) fn refresh_hosted_mod_skills(&mut self) -> Result<(), String> {
        if !self.hosted_gameplay_hud_visible() {
            self.mod_skills.hide();
            return Ok(());
        }
        let Some(hosted) = &self.hosted else { return Ok(()) };
        let Some(welcome) = &hosted.welcome else { return Ok(()) };
        if hosted.content_pending {
            return Ok(());
        }
        self.mod_skills.update(&crate::mod_skills::ModSkillsSnapshot {
            runtime: hosted.runtime.as_ref(),
            player_id: &self.local_player_id,
            skills_open: self.panels.panel() == Some(PlayerPanel::Skills),
            native_offer_sequence: self
                .session
                .participants
                .get(&self.local_player_id)
                .expect("participant exists")
                .player
                .progression
                .pending_offer
                .as_ref()
                .map(|offer| offer.sequence),
            assets: &welcome.mod_assets,
            images: self.mod_assets.as_ref(),
        })
    }

    fn refresh_mod_gameplay_hud(&mut self) -> Result<(), String> {
        if !self.hosted_gameplay_hud_visible() {
            self.mod_minimap.hide();
            self.mod_panels.hide();
            self.mod_skill_quickbar.reset()?;
            return Ok(());
        }
        let local =
            &self.session.participants.get(&self.local_player_id).expect("participant exists");
        let players: Vec<_> = self
            .session
            .participant_order
            .iter()
            .map(|id| ModMinimapPlayer {
                id,
                position: self
                    .session
                    .participants
                    .get(id)
                    .expect("participant exists")
                    .wizard
                    .position,
            })
            .collect();
        let enemies: Vec<_> = self
            .session
            .boneyard
            .iter()
            .flat_map(|world| world.enemies.actors.iter())
            // Source projectBoneyardEnemies omits the captured spider from snapshots.
            .filter(|actor| {
                !matches!(&actor.brain, EnemyBrain::Spider(brain)
                if brain.phase == SpiderPhase::Captured)
            })
            .map(|actor| ModMinimapEnemy { id: actor.id as f64, position: actor.position })
            .collect();
        let runtime = self.hosted.as_ref().and_then(|hosted| hosted.runtime.as_ref());
        let hosted = self.hosted.as_ref().expect("hosted presentation owns this update");
        let welcome = hosted.welcome.as_ref().expect("admitted presentation owns its assets");
        self.mod_skill_quickbar.update(
            hosted.generation,
            runtime,
            Some(ModSkillQuickbarPlayer {
                position: local.wizard.position,
                heading_index: local.wizard.heading_index,
            }),
            &welcome.mod_assets,
            self.mod_assets.as_ref(),
        )?;
        let scene_objects: Vec<_> = self
            .session
            .boneyard
            .iter()
            .flat_map(|world| {
                world.loaded.scene.objects.iter().map(|object| ModPanelsSceneObject {
                    type_id: object.type_id,
                    position: object.position,
                })
            })
            .collect();
        self.mod_panels.update(&ModPanelsSnapshot {
            runtime,
            player: Some(ModPanelsPlayer {
                position: local.wizard.position,
                backpack: &local.player.economy.backpack,
            }),
            world: if self.session.boneyard.is_some() {
                ModPanelsWorld::Boneyard
            } else {
                ModPanelsWorld::Hub { region: Some(local.wizard.region) }
            },
            scene_objects: &scene_objects,
        })?;
        self.mod_minimap.update(
            &ModMinimapSnapshot {
                players: &players,
                enemies: &enemies,
                in_boneyard: self.session.boneyard.is_some(),
            },
            self.mod_content.current(),
            &self.local_player_id,
            runtime,
        )
    }

    fn refresh_mod_overlays(&mut self) -> Result<(), String> {
        let Some(hosted) = &self.hosted else { return Ok(()) };
        let (Some(welcome), Some(images)) = (&hosted.welcome, &self.mod_assets) else {
            return Ok(());
        };
        let projection = self.mod_projection.map(|mut projection| {
            projection.local_player_position = self
                .session
                .participants
                .get(&self.local_player_id)
                .expect("participant exists")
                .wizard
                .position;
            projection
        });
        self.mod_overlays.refresh(
            &ModWorldOverlayFrame {
                runtime: hosted.runtime.as_ref(),
                assets: &welcome.mod_assets,
                projection,
            },
            images,
        )?;
        let party: Vec<_> = self
            .session
            .participant_order
            .iter()
            .map(|id| ModScenePartyMember {
                id,
                display_name: &self
                    .session
                    .participants
                    .get(id)
                    .expect("participant exists")
                    .player
                    .selection
                    .name,
            })
            .collect();
        let is_host = !welcome.observer
            && self
                .native
                .as_ref()
                .and_then(NativeGame::host_snapshot)
                .and_then(|host| host.authority_player_id.as_deref())
                == Some(self.local_player_id.as_str());
        self.mod_overlays.scene.refresh(&ModSceneOverlayFrame {
            runtime: hosted.runtime.as_ref(),
            owner_id: self.session.run.run_id.as_deref().unwrap_or(&self.local_player_id),
            is_host,
            party: &party,
        })
    }

    /// Overlays use the most recently rendered scene metrics. Accepted notifications
    /// and viewport changes refresh their keyed elements.
    pub(in crate::game) fn record_mod_scene_projection(&mut self) -> Result<(), String> {
        if self.hosted.is_none() || self.mod_assets.is_none() {
            return Ok(());
        }
        let session = self.display_session();
        let wizard =
            &session.participants.get(&self.local_player_id).expect("participant exists").wizard;
        let location = if session.boneyard.is_some() {
            ModWorldLocation::Boneyard
        } else if wizard.region == HubRegion::Courtyard {
            ModWorldLocation::HubCourtyard
        } else {
            ModWorldLocation::HubPrivate
        };
        let (camera, layout) = self.scene_view();
        let [left, top] = self.surface.origin();
        let zoom = camera.scale * layout.display_scale;
        let projection = ModWorldProjection {
            location,
            screen_origin: Vec2::new(
                left + (wizard.position.x - camera.origin.x) * zoom,
                top + (wizard.position.y - camera.origin.y) * zoom,
            ),
            local_player_position: self
                .session
                .participants
                .get(&self.local_player_id)
                .expect("participant exists")
                .wizard
                .position,
            zoom,
        };
        let size = (self.surface.logical_width(), self.surface.logical_height());
        let refresh = self.mod_projection.is_none_or(|last| last.location != location)
            || self.mod_viewport_size != Some(size);
        self.mod_projection = Some(projection);
        self.mod_viewport_size = Some(size);
        if refresh {
            self.refresh_mod_overlays()?;
        }
        Ok(())
    }
}
