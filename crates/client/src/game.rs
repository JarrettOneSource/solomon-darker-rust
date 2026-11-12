//! Shared game owner: assets and UI project the authoritative simulation state.
//! Loading prepares resources before installation; fixed ticks remain in the simulation kernels.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

mod actions;
#[cfg(target_arch = "wasm32")]
mod browser;
mod cheats;
mod desktop;
#[cfg(target_arch = "wasm32")]
pub(crate) mod editor_test;
mod hosted;
mod hosted_save;
mod local_save;
mod native;
mod overlays;
mod settings;
mod social;
mod sound;
mod spectator;
pub(crate) mod tasks;
mod ui_layers;
mod world_input;
mod world_load;
mod world_render;
use local_save::LocalSave;
pub use local_save::PendingSave;
use native::{CommandFeedback, NativeGame};
use solomon_protocol::{ClientMessage, GameCommand};
use world_load::WorldLoad;

use crate::battle::{BattleScene, BattleViews};
use crate::college_intro::CollegeIntroOverlay;
use crate::fullscreen::FullscreenControl;
use crate::game_over::GameOverOverlay;
use crate::player_panels::{
    InventoryAttributes, PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels,
};
use crate::storage::SaveStore;
use crate::tutorial::TutorialUi;
use crate::tutorial_scene::TutorialScene;
use crate::{set_status, storage};
use solomon_assets::parse_bundle;
use solomon_assets::save_document;
use solomon_graphics::assets;
use solomon_sim::Vec2;
use solomon_sim::appearance::StarterEquipmentAppearance;
use solomon_sim::audio::MusicScene;
use solomon_sim::audio::combat::CombatAudioCursor;
use solomon_sim::belt::BeltEntry;
use solomon_sim::boneyard::BONEYARD_CAMERA_SCALE;
use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::boneyard::lighting::BoneyardLightIndex;
use solomon_sim::economy::HubEconomyState;
use solomon_sim::economy::{HubInventoryAction, HubTraderId};
use solomon_sim::game_run::GameRunPhase;
#[cfg(target_arch = "wasm32")]
use solomon_sim::hub::HUB_LOGICAL_VIEW;
use solomon_sim::hub::interaction::{HubInteractionId, nearest_hub_interaction};
use solomon_sim::hub::{HUB_CAMERA_SCALE, hub_region_camera_origin_for_view};
use solomon_sim::hud::HudState;
use solomon_sim::input::{GameplayAction, GameplayAim, GameplayInputSample};
use solomon_sim::menu::loading::{MatchLoadingFlow, MatchLoadingStage, MatchLoadingState};
use solomon_sim::profile::WizardElement;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline};
use solomon_sim::session::{
    GameCheckpoint, GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};

use crate::audio::GameAudio;
use crate::boneyard::PreparedBoneyardView;
use crate::courtyard::{Camera, CourtyardScene};
use crate::gpu_ui::{GpuUi, UiFrame};
use crate::hotbar::HotbarSelection;
use crate::hud::{Hud, HudAction};
use crate::input::MovementInput;
use crate::menu::{MenuAction, MenuUi};
use crate::npc::NpcView;
use crate::rooms::PrivateRoomScenes;
use crate::shared_gameplay::SharedGameplayUi;
use crate::skill_picker::SkillPicker;
use crate::social::SocialUi;
use crate::spells::SpellView;
use crate::teacher::TeacherView;
use crate::wizard::WizardView;
use crate::world_overlays::WorldOverlaysView;
use crate::world_screen_feedback::WorldScreenFeedback;
use solomon_graphics::Renderer;

const LOCAL_PLAYER_ID: &str = "local";
const TICK_MILLISECONDS: f64 = 10.0;
/// Longest stall the loop catches up on before dropping time (a tab that was in the background).
const MAX_CATCH_UP_MILLISECONDS: f64 = 250.0;

/// Game-owned metadata from the last submitted UI frame. Input dispatch can advance
/// the live menu between draws without changing the browser's rendered observation.
struct GameUiSnapshot {
    surface: &'static str,
    ready: bool,
    selection: Option<NewGameSelection>,
    error: Option<String>,
    borrowed_image_paths: Vec<String>,
}

/// The running game: the accepted session and local player, every presentation view and UI
/// surface, audio, input, saving, and the native or hosted connection.
pub struct Game {
    surface: crate::platform::Surface,
    renderer: Renderer,
    gpu_ui: GpuUi,
    ui_ready: bool,
    ui_snapshot: Option<GameUiSnapshot>,
    ui_top_layers: Vec<egui::LayerId>,
    scene: CourtyardScene,
    rooms: PrivateRoomScenes,
    wizard_view: WizardView,
    teacher_view: TeacherView,
    spell_view: SpellView,
    world_overlays: WorldOverlaysView,
    world_overlay_diagnostics: crate::world_overlays::WorldOverlayDiagnostics,
    notifications: crate::loot::WorldNotifications,
    world_screen_feedback: WorldScreenFeedback,
    npc_view: NpcView,
    session: GameSession,
    local_player_id: String,
    native: Option<NativeGame>,
    hosted: Option<hosted::HostedGame>,
    #[cfg(target_arch = "wasm32")]
    editor_test: Option<editor_test::EditorTestPhase>,
    desktop_ui: Option<crate::desktop::DesktopUi>,
    desktop_close: Option<desktop::DesktopClose>,
    _playtime: Option<storage::PlaytimeRecorder>,
    mobile_layout_store: crate::mobile_layout::MobileUiLayoutStore,
    mod_assets: Option<crate::mod_presentation_assets::ModPresentationAssets>,
    mod_content: crate::mod_content_projection::ModContentState,
    mod_minimap: crate::mod_minimap::ModMinimapView,
    mod_panels: crate::mod_panels::ModPanelsView,
    mod_skills: crate::mod_skills::ModSkillsUi,
    mod_skill_quickbar: crate::mod_skill_quickbar::ModSkillQuickbarView,
    mod_audio: Option<crate::mod_audio::ModAudioDriver>,
    mod_overlays: crate::mod_world_overlays::ModWorldOverlays,
    mod_projection: Option<crate::mod_world_overlays::ModWorldProjection>,
    mod_viewport_size: Option<(i32, i32)>,
    input_aim: Option<Vec2>,
    has_game: bool,
    battle: Option<ActiveBattle>,
    battle_views: BattleViews,
    input: MovementInput,
    hotbar: HotbarSelection,
    hud: Hud,
    hud_state: HudState,
    menu: MenuUi,
    match_loading: Option<MatchLoadingState>,
    match_loading_view: crate::match_loading::MatchLoadingView,
    panels: PlayerPanels,
    book_feedback_scope: Option<(String, Option<String>)>,
    skill_picker: SkillPicker,
    social: SocialUi,
    cheats: crate::cheats::CheatUi,
    pending_cheat_open: bool,
    observer_ui: crate::observer::ObserverUi,
    spectator_camera: solomon_sim::boneyard::spectator::BoneyardSpectatorCameraState,
    spectator_ui: crate::spectator::SpectatorStatusView,
    shared_gameplay: SharedGameplayUi,
    fullscreen: FullscreenControl,
    audio: GameAudio,
    combat_audio: Option<CombatAudioCursor>,
    level_up: BTreeMap<String, solomon_sim::player_presentation::LevelUpPresentationTracker>,
    hall_entries: Vec<solomon_sim::hall_of_fame::HallOfFameEntry>,
    archived_hall_run: Option<String>,
    tutorial_ui: TutorialUi,
    tutorial_scene: TutorialScene,
    college_overlay: CollegeIntroOverlay,
    game_over: GameOverOverlay,
    last_input: Option<GameplayInputSample>,
    input_belt_press_count: u64,
    accumulator: f64,
    last_time: Option<f64>,
    world_load: WorldLoad,
    local_save: LocalSave,
    #[cfg(target_arch = "wasm32")]
    save_listeners: Vec<wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>>,
}

struct ActiveBattle {
    scene: BattleScene,
    navigation: solomon_sim::boneyard::navigation::NavigationSet,
}

impl Game {
    /// Allocate the retained resource future before polling its child loaders.
    fn load_resources(
        surface: crate::platform::Surface,
        renderer: Renderer,
        playtime: Option<storage::PlaytimeRecorder>,
    ) -> impl std::future::Future<Output = Result<Self, String>> {
        Box::pin(async move {
            crate::runtime_ui::begin(
                GpuUi::startup_resources().into_iter().chain(GameAudio::startup_resources()),
            )?;
            set_status("Loading stock assets…");
            let college = parse_bundle(&assets::fetch_bytes("assets/stock/College.bundle").await?)
                .map_err(|error| format!("College.bundle: {error}"))?;
            let clothes = parse_bundle(&assets::fetch_bytes("assets/stock/Clothes.bundle").await?)
                .map_err(|error| format!("Clothes.bundle: {error}"))?;
            let college_image = assets::load_image("assets/stock/College.png").await?;
            let clothes_image = assets::load_image("assets/stock/Clothes.png").await?;
            let shadow_image = assets::load_image("assets/stock/player-shadow.png").await?;

            let attachment_back =
                assets::load_image("assets/stock/player-character-unselected-attachment-back.png")
                    .await?;
            let attachment_front =
                assets::load_image("assets/stock/player-character-unselected-attachment-front.png")
                    .await?;
            let robe_attachment =
                assets::load_image("assets/stock/player-character-unselected-robe-attachment.png")
                    .await?;

            let gpu_ui = GpuUi::load(&surface, &renderer).await?;
            let teacher_view = TeacherView::load(&renderer).await?;
            let spell_view = SpellView::load(&renderer).await?;
            let college_texture = renderer.upload(&college_image)?;
            let clothes_texture = renderer.upload(&clothes_image)?;
            let shadow_texture = renderer.upload(&shadow_image)?;
            let npc_view = NpcView::new(
                college.clone(),
                clothes.clone(),
                college_texture.clone(),
                clothes_texture.clone(),
                shadow_texture.clone(),
            )?;
            let scene = CourtyardScene::new(college, college_texture)?;
            let rooms = PrivateRoomScenes::load(&renderer).await?;
            let economy =
                HubEconomyState::new(crate::platform::epoch_ms() as i64, WizardElement::Ether);
            let [primary_tint, secondary_tint] = economy
                .equipment
                .hat
                .as_ref()
                .and_then(|hat| hat.clothing_tints())
                .expect("the fresh wizard owns a tinted starter hat");
            let appearance = StarterEquipmentAppearance { primary_tint, secondary_tint };
            let wizard_view = WizardView::new(
                clothes,
                clothes_texture,
                shadow_texture,
                appearance,
                renderer.upload(&attachment_back)?,
                renderer.upload(&attachment_front)?,
                renderer.upload(&robe_attachment)?,
                &renderer,
            )?;
            let hotbar = HotbarSelection::attach(&surface)?;
            let input = MovementInput::attach(&surface, hotbar.clone())?;
            input.set_blocked(true)?;
            input.set_primary_casting_enabled(false)?;
            let mut hud = Hud::attach(hotbar.clone()).await?;
            hud.set_actions_enabled(true)?;

            hud.set_action_enabled(HudAction::Pause, true)?;
            let menu = MenuUi::attach(&surface).await?;
            let panels = PlayerPanels::attach(hotbar.clone()).await?;
            let skill_picker = SkillPicker::attach().await?;
            set_status("Loading stock audio…");
            let audio = GameAudio::load(&surface).await?;
            audio.set_scene(MusicScene::Title)?;
            let battle_views = BattleViews::load(&renderer, &spell_view).await?;
            let world_overlays = WorldOverlaysView::load(&renderer).await?;
            let tutorial_ui = TutorialUi::attach().await?;
            let college_overlay = CollegeIntroOverlay::attach().await?;
            let game_over = GameOverOverlay::attach()?;
            let session = GameSession::new(
                LOCAL_PLAYER_ID.to_owned(),
                NewGameSelection {
                    name: "Sirmin".into(),
                    element: WizardElement::Ether,
                    discipline: WizardDiscipline::Arcane,
                },
                GameSessionOptions::default(),
            );
            let game = Self {
                surface: surface.clone(),
                renderer,
                gpu_ui,
                ui_ready: false,
                ui_snapshot: None,
                ui_top_layers: Vec::new(),
                scene,
                rooms,
                wizard_view,
                teacher_view,
                spell_view,
                world_overlays,
                world_overlay_diagnostics: Default::default(),
                notifications: crate::loot::WorldNotifications::default(),
                world_screen_feedback: WorldScreenFeedback::default(),
                npc_view,
                session,
                local_player_id: LOCAL_PLAYER_ID.into(),
                native: None,
                hosted: None,
                #[cfg(target_arch = "wasm32")]
                editor_test: None,
                desktop_ui: None,
                desktop_close: None,
                _playtime: playtime,
                mobile_layout_store: crate::mobile_layout::MobileUiLayoutStore::new(&surface)?,
                mod_assets: None,
                mod_content: crate::mod_content_projection::ModContentState::default(),
                mod_minimap: crate::mod_minimap::ModMinimapView::new()?,
                mod_panels: crate::mod_panels::ModPanelsView::new()?,
                mod_skills: crate::mod_skills::ModSkillsUi::new()?,
                mod_skill_quickbar: crate::mod_skill_quickbar::ModSkillQuickbarView::new(&surface)?,
                mod_audio: None,
                mod_overlays: crate::mod_world_overlays::ModWorldOverlays::new()?,
                mod_projection: None,
                mod_viewport_size: None,
                input_aim: None,
                has_game: false,
                battle: None,
                battle_views,
                input,
                hotbar,
                hud,
                menu,
                match_loading: None,
                match_loading_view: crate::match_loading::MatchLoadingView::attach()?,
                panels,
                book_feedback_scope: None,
                skill_picker,
                social: SocialUi::attach(&surface).await?,
                cheats: crate::cheats::CheatUi::attach(&surface)?,
                pending_cheat_open: false,
                observer_ui: crate::observer::ObserverUi::attach()?,
                spectator_camera: Default::default(),
                spectator_ui: crate::spectator::SpectatorStatusView::attach().await?,
                shared_gameplay: SharedGameplayUi::attach()?,
                fullscreen: FullscreenControl::attach(&surface)?,
                audio,
                combat_audio: None,
                level_up: Default::default(),
                hall_entries: Vec::new(),
                archived_hall_run: None,
                tutorial_ui,
                tutorial_scene: TutorialScene::default(),
                college_overlay,
                game_over,
                last_input: None,
                input_belt_press_count: 0,
                hud_state: HudState::new_game(),
                accumulator: 0.0,
                last_time: None,
                world_load: WorldLoad::default(),
                local_save: LocalSave::default(),
                #[cfg(target_arch = "wasm32")]
                save_listeners: Vec::new(),
            };
            Ok(game)
        })
    }

    pub fn wizard_snapshot(&self) -> Vec<f64> {
        let wizard = self.session.wizard(&self.local_player_id).expect("local participant");
        vec![
            wizard.position.x,
            wizard.position.y,
            wizard.heading_index as f64,
            wizard.gait_degrees,
            wizard.walk_cycle_primary,
            self.session.ticks as f64,
            f64::from(wizard.region.index()),
            wizard.transition.map_or(-1.0, |transition| match transition.phase {
                solomon_sim::hub::regions::HubTransitionPhase::CollegeIntro => 0.0,
                solomon_sim::hub::regions::HubTransitionPhase::CollegeLoadout => 1.0,
                solomon_sim::hub::regions::HubTransitionPhase::Outgoing => 2.0,
                solomon_sim::hub::regions::HubTransitionPhase::Incoming => 3.0,
            }),
            wizard.transition.map_or(0.0, |transition| transition.alpha),
        ]
    }

    pub fn npc_snapshot(&self) -> Vec<f64> {
        let students = &self.session.npcs.students.students;
        let mut snapshot = Vec::with_capacity(1 + students.len() * 3);
        snapshot.push(students.len() as f64);
        for student in students.iter() {
            snapshot.extend([student.id as f64, student.position.x, student.position.y]);
        }
        snapshot
    }

    pub fn install_boneyard(
        &mut self,
        loaded: LoadedBoneyard,
        prepared: PreparedBoneyardView,
        navigation: solomon_sim::boneyard::navigation::NavigationSet,
    ) -> Result<(), String> {
        if !self.has_game && self.native.is_none() {
            return Err("Start a wizard before entering the Boneyard".into());
        }
        if self.native.is_none() && !self.session.can_enter_boneyard() {
            return Err("The current run must finish before starting another".into());
        }
        let resolution = self.surface.scale_factor();
        let pending_snapshot = self
            .native
            .as_ref()
            .and_then(|native| native.pending_snapshot.as_ref())
            .map(|received| &received.frame.snapshot);
        let initial_tick = pending_snapshot.map_or(self.session.ticks, |snapshot| snapshot.ticks);
        let initial_run = pending_snapshot.map_or(&self.session.run, |snapshot| &snapshot.run);
        let scenery = prepared.upload(
            &self.renderer,
            self.scene_view().1.logical_size,
            resolution,
            initial_tick,
            solomon_sim::game_run::game_run_world_tick(initial_tick as f64, initial_run) as u64,
        )?;
        let compact_masks = crate::compact_masks::CompactMaskView::new(&loaded.scene);
        let bounds = loaded.scene.bounds;
        let lights =
            BoneyardLightIndex::new(Vec2::new(bounds.x + bounds.width, bounds.y + bounds.height));
        let weather = crate::boneyard_weather::BoneyardWeatherView::new(
            &self.renderer,
            initial_tick,
            loaded.scene.environment_mode,
            true,
        )?;
        if self.native.is_some() {
            let frame = self
                .native
                .as_mut()
                .unwrap()
                .pending_snapshot
                .take()
                .ok_or("The game host no longer has a pending Boneyard")?;
            if frame.frame.snapshot.boneyard.as_ref().map(|world| world.run_id.as_str())
                != Some(loaded.run_id.as_str())
            {
                return Err("The prepared Boneyard differs from the host snapshot".into());
            }
            let run_id = loaded.run_id.clone();
            self.reset_world_views()?;
            self.battle = Some(ActiveBattle {
                scene: BattleScene {
                    scenery,
                    lights,
                    weather,
                    compact_masks,
                    compact_frame_count: 0,
                },
                navigation,
            });
            self.world_load.finish();
            self.apply_native_frame(frame, Some(loaded))?;
            // An observer loads the world without claiming
            // participant readiness or changing that participant's pause barrier.
            if !self.is_hosted_observer() {
                self.send_native_message(&ClientMessage::ReadyBoneyard { run_id })?;
            }
            return Ok(());
        }
        self.session.install_boneyard(loaded)?;
        self.reset_world_views()?;
        self.battle = Some(ActiveBattle {
            scene: BattleScene { scenery, lights, weather, compact_masks, compact_frame_count: 0 },
            navigation,
        });
        self.world_load.finish();
        self.accumulator = 0.0;
        self.menu.hide()?;
        self.request_save();
        self.update_combat_audio()?;
        Ok(())
    }

    pub fn take_world_load(&mut self) -> Option<(u64, LoadedBoneyard)> {
        self.world_load.take_queued()
    }

    pub fn world_load_current(&self, generation: u64) -> bool {
        self.world_load.is_current(generation)
    }

    pub fn world_load_failed(&mut self, error: &str) {
        self.world_load.finish();
        self.match_loading = None;
        set_status(error);
    }

    fn reset_world_views(&mut self) -> Result<(), String> {
        self.notifications.reset();
        self.book_feedback_scope = None;
        self.panels.reset_skill_book_feedback(None);
        self.spectator_camera = Default::default();
        self.spectator_ui.hide();
        if let Some(previous) = self.battle.take() {
            previous.scene.scenery.destroy(&self.renderer);
            previous.scene.weather.destroy(&self.renderer);
            previous.scene.compact_masks.destroy(&self.renderer);
        }
        self.spell_view.reset();
        self.scene.reset_overlays();
        self.battle_views.reset(&mut self.renderer);
        self.world_screen_feedback.reset();
        self.level_up.clear();
        if let Some(mut cursor) = self.combat_audio.take() {
            self.audio.apply_combat(cursor.destroy())?;
        }
        Ok(())
    }

    /// Platform clipboard and editing defaults observe the preceding UI action's
    /// committed focus. Simulation, transport polling and drawing advance once per frame.
    fn dispatch_ui_input(&mut self, time: f64) -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        if self.editor_test_disposed() {
            return Ok(());
        }
        if self.surface.logical_width() <= 0 || self.surface.logical_height() <= 0 {
            return Ok(());
        }
        let Some(mut run) = self.gpu_ui.begin_native_dispatch(time / 1000.0)? else {
            return Ok(());
        };
        let result = (|| {
            loop {
                let menu_was_open = self.menu.is_blocking();
                let more = {
                    let (gpu_ui, paint) = self.ui_painter()?;
                    gpu_ui.paint_next(&mut run, paint)?
                };
                if !more {
                    break;
                }
                self.commit_ui(time, menu_was_open)?;
                #[cfg(target_arch = "wasm32")]
                if self.editor_test_disposed() {
                    return Ok(());
                }
            }
            let (gpu_ui, paint) = self.ui_painter()?;
            gpu_ui.paint_layout(&mut run, paint)
        })();
        self.gpu_ui.finish_native_dispatch(run, result)?;
        self.input
            .set_ui_geometry(self.gpu_ui.input_diagnostics(), &self.hud.belt_input_geometry());
        Ok(())
    }

    /// Start asynchronous work selected by the previous frame without borrowing
    /// the game across a file, asset or network await.
    pub fn run_pending_work(game: &Rc<RefCell<Self>>) {
        crate::load_pending_world(game);
        crate::write_pending_save(game);
        crate::run_hosted_work(game);
    }

    pub fn frame(&mut self, time: f64) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(playtime) = &mut self._playtime
            && let Err(error) = playtime.tick()
        {
            crate::platform::error(&error);
        }
        #[cfg(target_arch = "wasm32")]
        if self.editor_test_disposed() {
            return Ok(());
        }
        self.poll_native(time)?;
        #[cfg(target_arch = "wasm32")]
        self.poll_editor_test()?;
        #[cfg(target_arch = "wasm32")]
        if self.editor_test_disposed() {
            return Ok(());
        }
        // An embedding page can hide the iframe before its queued cancel message
        // arrives. Keep admission/disposal polling alive, but never lay out a
        // zero-sized canvas. A later visible frame resumes without catch-up time.
        if self.surface.logical_width() <= 0 || self.surface.logical_height() <= 0 {
            self.last_time = Some(time);
            return Ok(());
        }
        self.update_mobile_layout()?;
        self.poll_hosted(time)?;
        self.update_match_loading(time)?;
        self.menu.set_enhanced_effects_authority(self.enhanced_effects_authority())?;
        self.hud.update_diagnostics(time, self.native.as_ref().and_then(NativeGame::ping_ms))?;
        let last = self.last_time.replace(time).unwrap_or(time);
        self.commit_ui(time, self.menu.is_blocking())?;
        #[cfg(target_arch = "wasm32")]
        if self.editor_test_disposed() {
            return Ok(());
        }
        self.fit_canvas();
        let viewport = [self.surface.logical_width() as f32, self.surface.logical_height() as f32];
        let mut run = self.gpu_ui.begin_frame(&mut self.renderer, viewport, time / 1000.0)?;
        if self.menu.is_blocking() || !self.has_game {
            run.defer_idle_layout();
        }
        let result = (|| {
            loop {
                let menu_was_open = self.menu.is_blocking();
                let more = {
                    let (gpu_ui, paint) = self.ui_painter()?;
                    gpu_ui.paint_next(&mut run, paint)?
                };
                if !more {
                    break;
                }
                self.commit_ui(time, menu_was_open)?;
                #[cfg(target_arch = "wasm32")]
                if self.editor_test_disposed() {
                    return Ok(());
                }
            }
            self.input
                .set_ui_geometry(self.gpu_ui.input_diagnostics(), &self.hud.belt_input_geometry());
            if !self.deployment_restarting() {
                if self.is_hosted_observer() {
                    self.frame_hosted_observer(time)?;
                } else {
                    self.frame_gameplay(time, last)?;
                }
                self.update_audio()?;
            }
            self.render_world_and_notifications()?;
            let (gpu_ui, paint) = self.ui_painter()?;
            gpu_ui.paint_layout(&mut run, paint)
        })();
        // Managed textures can be allocated before a later controller/action fails.
        // Always finish the transaction, including its deferred image retirements.
        self.gpu_ui.finish_frame(&mut self.renderer, run, result)?;
        #[cfg(target_arch = "wasm32")]
        if self.editor_test_disposed() {
            return Ok(());
        }
        self.input
            .set_ui_geometry(self.gpu_ui.input_diagnostics(), &self.hud.belt_input_geometry());
        self.finish_ready_frame()
    }

    fn deployment_restarting(&self) -> bool {
        self.hosted.as_ref().is_some_and(|hosted| hosted.deployment_update.restarting())
    }

    fn commit_ui(&mut self, time: f64, menu_was_open: bool) -> Result<(), String> {
        if self.deployment_restarting() {
            return self.input.set_blocked(true);
        }
        if self.is_hosted_observer() {
            return self.commit_hosted_observer_ui(time);
        }
        let skills_were_open = self.panels.panel() == Some(PlayerPanel::Skills);
        self.update_skill_picker(time)?;
        self.update_social(time)?;
        self.update_cheats()?;
        self.menu.update(time, self.surface.logical_width(), self.surface.logical_height())?;
        if self.menu.is_create_pending() && self.match_loading.is_none() {
            self.begin_match_loading(MatchLoadingFlow::Hub, MatchLoadingStage::ConnectingTransport);
        }
        for action in self.menu.take_actions() {
            if let Err(error) = self.apply_menu_action(action) {
                self.menu.complete_create_loadout(false)?;
                self.match_loading = None;
                return Err(error);
            }
            #[cfg(target_arch = "wasm32")]
            if self.editor_test_disposed() {
                return Ok(());
            }
        }
        for action in self.hud.take_actions() {
            if self.skill_selection_active() {
                continue;
            }
            match action {
                HudAction::Pause if self.can_pause() => self.menu.show_pause()?,
                HudAction::Map if self.can_start_boneyard() => self.queue_world(false)?,
                HudAction::Inventory if self.can_open_panel(PlayerPanel::Inventory) => {
                    self.panels.show(PlayerPanel::Inventory, time)?
                }
                HudAction::Skills if self.can_open_panel(PlayerPanel::Skills) => {
                    self.panels.show(PlayerPanel::Skills, time)?
                }
                HudAction::SkillBinding(binding)
                    if self.can_edit_books()
                        && self.tutorial_access().is_none_or(|access| access.spell) =>
                {
                    self.panels.show_skill_selector(binding, time)?
                }
                HudAction::Shortcut(index) if self.session.boneyard.is_none() => {
                    if index == 0 {
                        self.open_hub_dialogue(HubInteractionId::Annalist, time)?;
                    } else {
                        let trader = match index {
                            1 => HubTraderId::Hagatha,
                            2 => HubTraderId::Luthacus,
                            3 => HubTraderId::Fomentius,
                            4 => HubTraderId::Shlorio,
                            _ => continue,
                        };
                        self.panels.show_service(trader, time)?;
                    }
                }
                // Belt press/release belongs to the single sampled gameplay input path.
                _ => {}
            }
        }
        if self.has_game {
            let player = &self.session.participants[&self.local_player_id].player;
            let derived = player.derived_stats();
            let offensive = derived.offensive_factors(&player.skill_runtime);
            let concentrations = [
                player.skill_runtime.concentration_skill_id_a,
                player.skill_runtime.concentration_skill_id_b,
            ];
            self.panels.update(
                &PlayerPanelSnapshot {
                    selection: &player.selection,
                    book: &player.book,
                    stat_book: &player.stat_book,
                    economy: &player.economy,
                    dye_session_id: self
                        .session
                        .inventory_dye_session(&self.local_player_id)
                        .map(|session| session.id.as_str()),
                    mod_content: self.mod_content.current(),
                    mod_assets: self.mod_assets.as_ref(),
                    belt: &player.belt,
                    progression: &player.progression,
                    resources: &player.combat,
                    offensive: &offensive,
                    attributes: InventoryAttributes {
                        cast_speed_percent: derived.cast_progress_factor * 100.0,
                        walk_speed_percent: derived.movement_factor * 100.0,
                        pain_resistance_percent: derived.damage_resistance * 100.0,
                        magic_resistance_percent: derived.magic_resistance * 100.0,
                        poison_resistance_percent: derived.poison_resistance * 100.0,
                        mana_recovery_per_second: derived.mana_recovery_per_second,
                        melee_damage_minimum: derived.melee_damage_minimum,
                        melee_damage_maximum: derived.melee_damage_maximum,
                    },
                    concentrations: &concentrations,
                    mind_chug_active: player.progression.mind_chug_ticks_remaining > 0,
                    perk_removal_enabled: self.session.boneyard.is_none(),
                    kills: self.session.participants[&self.local_player_id].score.monsters_killed,
                    awesomeness: self.session.participants[&self.local_player_id].score.awesomeness,
                },
                time,
                self.surface.logical_width(),
                self.surface.logical_height(),
            )?;
        }
        for action in self.panels.take_actions() {
            self.apply_panel_action(action, time)?;
        }
        if skills_were_open != (self.panels.panel() == Some(PlayerPanel::Skills)) {
            self.refresh_hosted_mod_skills()?;
        }
        for action in self.game_over.take_actions() {
            if !self.has_game || self.menu.covers_world() || self.battle.is_none() {
                continue;
            }
            #[cfg(target_arch = "wasm32")]
            if self.is_editor_test() {
                return self.return_to_editor();
            }
            if self.native.is_some() {
                self.send_native_command(
                    GameCommand::ContinueGameOver {
                        run_id: action.run_id,
                        event_id: action.event_id,
                    },
                    CommandFeedback::None,
                )?;
            } else if self.session.continue_game_over(&action.run_id, action.event_id) {
                self.request_save();
            }
        }
        self.advance_session_ui(time)?;
        self.archive_completed_run();
        self.menu.set_hall_snapshot(crate::menu::HallSnapshot {
            entries: &self.hall_entries,
            current_run_id: self.session.run.run_id.as_deref(),
        });
        self.update_overlays(time)?;
        self.update_match_loading(time)?;
        let hotbar_enabled = self.hotbar_available();
        self.hotbar.set_shortcut(self.menu.settings(), hotbar_enabled);
        self.hud.set_hotbar_enabled(hotbar_enabled);
        if self.menu.is_blocking() && !menu_was_open {
            self.request_save();
        }
        self.hud.set_visible(!self.menu.is_blocking() && !self.panels.covers_world());
        Ok(())
    }

    fn frame_gameplay(&mut self, time: f64, last: f64) -> Result<(), String> {
        let blocked = self.native_gameplay_held()
            || self.cheats.is_open()
            || self.social.input_active()
            || self.menu.is_blocking()
            || self.panels.is_open()
            || self.skill_selection_active()
            || self.world_load.is_loading()
            || self.match_loading.is_some()
            || !self.has_game;
        for slot in self.input.take_belt_unassignments() {
            if !blocked && self.can_edit_books() {
                self.hud.show_belt_pull_off(slot)?;
                self.audio.apply(solomon_sim::audio::AudioEvent::Play(
                    solomon_sim::audio::AudioCue::Poof,
                    solomon_sim::audio::AudioOptions::default(),
                ))?;
                self.apply_panel_action(
                    PlayerPanelAction::Belt(solomon_sim::belt::BeltCommand::BindSkill {
                        skill_id: None,
                        slot,
                    }),
                    time,
                )?;
            }
        }
        self.input.set_blocked(blocked)?;
        self.input.set_primary_casting_enabled(self.session.boneyard.is_some())?;
        self.configure_spectator_input();
        self.input.resize(
            self.surface.logical_width(),
            self.surface.logical_height(),
            self.menu.settings().ui_scale_percent,
        )?;
        let input = self.input.sample()?;
        self.hud
            .set_controller_belt_slot(input.controller_present.then_some(input.selected_belt_slot));
        if blocked {
            self.input_aim = None;
            if self.native.is_none() && self.simulation_is_paused() {
                self.last_input = Some(input);
                self.accumulator = 0.0;
                return Ok(());
            }
        }
        self.sample_native_presentation(
            time,
            GameSessionInput {
                aim: self.input_aim,
                movement: input.movement,
                primary: input.primary,
                quickbar: input.quickbar,
                viewport: self.scene_view().1.logical_size,
            },
            self.native_gameplay_held() || self.world_load.is_loading() || !self.has_game,
        )?;
        self.update_spectator_camera(self.input.take_spectator_camera_advances());
        self.input_aim = self.project_input_aim(input.aim);
        self.input_belt_press_count += input.belt_pressed.len() as u64;
        for slot in &input.belt_pressed {
            // Held skills execute during the fixed tick; item activation is a press edge.
            if !matches!(
                self.session.participants[&self.local_player_id].player.belt.entries[*slot],
                Some(BeltEntry::Skill { .. })
            ) {
                self.activate_belt_slot(*slot, None, time)?;
            }
        }
        for action in &input.actions {
            match action {
                GameplayAction::Chat => self.social.open_chat(time),
                GameplayAction::Pause if self.can_pause() => self.menu.show_pause()?,
                GameplayAction::Inventory if self.can_open_panel(PlayerPanel::Inventory) => {
                    self.panels.show(PlayerPanel::Inventory, time)?
                }
                GameplayAction::Skills if self.can_open_panel(PlayerPanel::Skills) => {
                    self.panels.show(PlayerPanel::Skills, time)?
                }
                GameplayAction::Interact if self.session.boneyard.is_some() => {
                    if self.native.is_some() {
                        self.send_native_command(
                            GameCommand::InteractGoodie,
                            CommandFeedback::None,
                        )?;
                    } else if self
                        .session
                        .interact_goodie(&self.local_player_id)
                        .transaction
                        .accepted
                    {
                        self.request_save();
                    }
                }
                GameplayAction::Interact if self.session.boneyard.is_none() => {
                    if !self.open_hub_player(None)
                        && let Some(interaction) = nearest_hub_interaction(
                            self.session.participants[&self.local_player_id].wizard.region,
                            self.session.participants[&self.local_player_id].wizard.position,
                            self.interaction_availability(),
                        )
                    {
                        self.open_hub_dialogue(interaction, time)?;
                    }
                }
                _ => {}
            }
        }
        self.interact_at_pointer(time)?;
        let mut session_input = GameSessionInput {
            aim: self.input_aim,
            movement: input.movement,
            primary: input.primary,
            quickbar: input.quickbar,
            viewport: self.scene_view().1.logical_size,
        };
        self.last_input = Some(input);
        if self.menu.is_blocking()
            || self.cheats.is_open()
            || self.panels.is_open()
            || self.skill_selection_active()
            || self.social.input_active()
        {
            if self.menu.is_blocking() {
                self.request_save();
            }
            self.input.set_blocked(true)?;
            session_input = GameSessionInput {
                viewport: session_input.viewport,
                ..GameSessionInput::default()
            };
            if self.native.is_none() && self.simulation_is_paused() {
                self.accumulator = 0.0;
                return Ok(());
            }
        }
        if self.native.is_some() {
            self.native_input_frame(session_input)?;
            self.refresh_hud();
            self.update_overlays(time)?;
            return Ok(());
        }
        let inputs = BTreeMap::from([(self.local_player_id.clone(), session_input)]);
        let college_ready = BTreeSet::from([self.local_player_id.clone()]);
        self.accumulator = (self.accumulator + (time - last)).min(MAX_CATCH_UP_MILLISECONDS);
        while self.accumulator >= TICK_MILLISECONDS {
            let result = self.session.step(
                &inputs,
                GameSessionTickContext {
                    navigation: self.battle.as_ref().map(|battle| &battle.navigation),
                    college_intro_ready_player_ids: Some(&college_ready),
                    enemy_spawn_intents: &[],
                },
            );
            self.accumulator -= TICK_MILLISECONDS;
            if !result.advanced {
                self.accumulator = 0.0;
                break;
            }
            self.advance_local_scenery();
            if let Some(world) = &self.session.boneyard {
                self.battle_views.consume_events(&world.enemies.events);
                self.world_screen_feedback
                    .consume_enemy_events(&world.loaded.run_id, &world.enemies.events);
            } else if self.battle.is_some() {
                self.reset_world_views()?;
                self.request_save();
            }
            for event in result.audio_events {
                if matches!(
                    event,
                    solomon_sim::audio::AudioEvent::Play(
                        solomon_sim::audio::AudioCue::SolomonLaughBig,
                        _
                    )
                ) {
                    for cue in solomon_sim::audio::combat::BONEYARD_SOLOMON_DIALOGUE_CUES {
                        self.audio.apply(solomon_sim::audio::AudioEvent::StopStream(cue))?;
                    }
                }
                self.audio.apply(event)?;
            }
            self.update_combat_audio()?;
            self.advance_session_ui(time)?;
        }
        self.refresh_hud();
        if self.local_save.autosave_due(self.session.ticks) {
            self.request_save();
        }
        self.update_overlays(time)?;
        Ok(())
    }

    fn update_mobile_layout(&mut self) -> Result<(), String> {
        if self.mobile_layout_store.take_changed()? {
            let state = self.mobile_layout_store.read()?;
            self.hud.set_mobile_layout(&state)?;
            self.input.set_mobile_layout(&state)?;
            self.menu.set_mobile_layout_state(&state)?;
        }
        Ok(())
    }

    fn skill_selection_active(&self) -> bool {
        self.skill_picker.is_open()
            || self
                .has_game
                .then(|| self.session.participants[&self.local_player_id].player.as_ref())
                .is_some_and(|player| player.progression.pending_offer.is_some())
    }

    fn simulation_is_paused(&self) -> bool {
        // Gameplay pause applies only in a Boneyard. In the Hub, panels seal the local
        // input, while the College's students, NPCs and onboarding keep ticking.
        !self.has_game
            || self.world_load.is_loading()
            || self.menu.covers_world()
            || self.skill_selection_active()
            || (self.session.boneyard.is_some()
                && (self.menu.is_blocking() || self.panels.is_open()))
    }

    fn tutorial_access(&self) -> Option<solomon_sim::boneyard::tutorial::TutorialHudAccess> {
        self.session
            .boneyard
            .as_ref()
            .and_then(|world| world.tutorial.as_ref())
            .map(solomon_sim::boneyard::tutorial::tutorial_hud_access)
    }

    fn can_edit_books(&self) -> bool {
        self.has_game
            && !self.cheats.is_open()
            && self.native_modal_available()
            && self.session.can_edit_books(&self.local_player_id)
    }

    /// Whether the hotbar shortcut may cycle banks. Optional books retain the owner's bank
    /// selection while modal, loading and shared readiness barriers prevent cycling.
    fn hotbar_available(&self) -> bool {
        self.has_game
            && self.native_modal_available()
            && !self.cheats.is_open()
            && !self.social.input_active()
            && !self.menu.is_blocking()
            && !self.menu.settings_open()
            && !self.world_load.is_loading()
            && self.match_loading.is_none()
            && !self.skill_selection_active()
            && self.session.level_up_barrier.is_none()
            && !matches!(self.panels.panel(), Some(PlayerPanel::SkillSelector(_)))
            && self
                .session
                .boneyard
                .as_ref()
                .and_then(|world| world.tutorial.as_ref())
                .is_none_or(|tutorial| !tutorial.intro_active)
            && self
                .native
                .as_ref()
                .and_then(NativeGame::host_snapshot)
                .and_then(|host| host.pause.as_ref())
                .is_none_or(|pause| {
                    pause.owner_player_id == self.local_player_id
                        && pause.source != solomon_protocol::PauseSource::PauseMenu
                })
    }

    fn archive_completed_run(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if self.is_editor_test() {
            return;
        }
        let Some(world) = &self.session.boneyard else {
            return;
        };
        // The Hall of Fame excludes the entire Tutorial run,
        // including its archived death portrait before the College return.
        if world.tutorial.is_some() {
            return;
        }
        if self.archived_hall_run.as_deref() == Some(&world.loaded.run_id)
            || self.hall_entries.iter().any(|entry| entry.run_id == world.loaded.run_id)
        {
            return;
        }
        let Some(entry) = solomon_sim::hall_of_fame::completed_hall_of_fame_entry(
            &world.loaded.run_id,
            self.session.run.phase,
            &self.session.participants[&self.local_player_id].score,
            &self.session.participants[&self.local_player_id].player,
            world.waves.as_ref().map_or(0, |waves| waves.state.wave_ordinal),
            crate::platform::iso_now(),
        ) else {
            return;
        };
        self.archived_hall_run = Some(entry.run_id.clone());
        match storage::record_hall(entry) {
            Ok(entries) => self.hall_entries = entries,
            Err(error) => Self::show_storage_error(&error),
        }
    }

    fn can_pause(&self) -> bool {
        self.has_game
            && !self.cheats.is_open()
            && self.native_modal_available()
            && matches!(self.session.run.phase, GameRunPhase::Hub | GameRunPhase::Active)
            && self.tutorial_access().is_none_or(|access| access.combat)
    }

    fn can_open_panel(&self, panel: PlayerPanel) -> bool {
        self.has_game
            && !self.cheats.is_open()
            && self.native_modal_available()
            && matches!(self.session.run.phase, GameRunPhase::Hub | GameRunPhase::Active)
            && self.tutorial_access().is_none_or(|access| match panel {
                PlayerPanel::Inventory => access.inventory,
                PlayerPanel::Skills => access.skills,
                PlayerPanel::SkillSelector(_) => access.spell,
                PlayerPanel::Dialogue | PlayerPanel::Service(_) => false,
            })
    }

    fn queue_world(&mut self, tutorial: bool) -> Result<(), String> {
        if self.native.is_some() {
            return self.native_start_run(tutorial);
        }
        if self.world_load.is_loading() {
            return Ok(());
        }
        let mut seed = [0; 16];
        crate::platform::random_bytes(&mut seed)?;
        let loaded = if tutorial {
            solomon_assets::boneyard::load_tutorial(seed)
        } else {
            solomon_assets::boneyard::load_boneyard(seed)
        }?;
        self.world_load.start(loaded);
        self.begin_match_loading(MatchLoadingFlow::Boneyard, MatchLoadingStage::MaterializingWorld);
        set_status("Loading Boneyard…");
        Ok(())
    }

    fn can_start_boneyard(&self) -> bool {
        // Only the host may start a Boneyard. A follower's ignored request
        // must never create a loading screen that no accepted world can complete.
        self.session.can_enter_boneyard()
            && !self.session.participants[&self.local_player_id]
                .player
                .economy
                .college_intro_pending
            && self.native.as_ref().is_none_or(|native| {
                native.host_snapshot().is_some_and(|host| {
                    host.authority_player_id.as_deref() == Some(self.local_player_id.as_str())
                })
            })
    }

    fn begin_match_loading(&mut self, flow: MatchLoadingFlow, stage: MatchLoadingStage) {
        if let Some(loading) = &mut self.match_loading
            && loading.flow == flow
        {
            loading.advance(stage);
            return;
        }
        let now = crate::platform::monotonic_ms();
        self.match_loading = Some(MatchLoadingState::begin(flow, stage, now));
    }

    fn update_match_loading(&mut self, now: f64) -> Result<(), String> {
        if let Some(loading) = &mut self.match_loading {
            let admitted = self.has_game && self.native.as_ref().is_none_or(NativeGame::is_playing);
            let prepared = !self.world_load.is_loading()
                && !self.menu.is_create_pending()
                && self
                    .hosted
                    .as_ref()
                    .is_none_or(|hosted| !hosted.connecting && !hosted.content_pending);
            let world_ready = loading.flow == MatchLoadingFlow::Hub || self.battle.is_some();
            if admitted && prepared && world_ready {
                // A wizard who rejoins with a level-up pending stays staged, outside the
                // world, until it chooses. Its own staging must not keep the loading
                // screen over the picker that ends it.
                let participants_pending =
                    self.native.as_ref().and_then(NativeGame::host_snapshot).is_some_and(|host| {
                        host.materializing_player_ids.iter().any(|id| *id != self.local_player_id)
                    });
                if participants_pending {
                    loading.advance(MatchLoadingStage::WaitingForParticipants);
                } else {
                    self.match_loading = None;
                }
            }
        }
        self.match_loading_view.update(self.match_loading.as_ref(), now)
    }

    fn college_loadout_pending(&self) -> bool {
        self.session.participants[&self.local_player_id].wizard.transition.is_some_and(
            |transition| {
                transition.phase == solomon_sim::hub::regions::HubTransitionPhase::CollegeLoadout
            },
        )
    }

    fn advance_session_ui(&mut self, now: f64) -> Result<(), String> {
        self.observe_skill_book_feedback(now)?;
        if !self.has_game || self.native_ui_waiting() {
            return Ok(());
        }
        if self.session.run.phase == GameRunPhase::Loadout || self.college_loadout_pending() {
            if !self.menu.is_blocking() {
                self.menu.show_loadout(
                    &self.session.participants[&self.local_player_id].player.selection.name,
                )?;
            }
        } else if self.session.participants[&self.local_player_id].college_intro.is_some_and(
            |intro| intro.phase == solomon_sim::hub::college_intro::CollegeIntroPhase::ArchDialogue,
        ) && !self.panels.is_open()
        {
            self.open_hub_dialogue(HubInteractionId::ArchChancellor, now)?;
        }
        Ok(())
    }

    fn observe_skill_book_feedback(&mut self, now: f64) -> Result<(), String> {
        if !self.has_game
            || self.world_load.is_loading()
            || self.is_hosted_observer()
            || matches!(self.session.run.phase, GameRunPhase::Loadout | GameRunPhase::GameOver)
        {
            if self.book_feedback_scope.take().is_some() {
                self.panels.reset_skill_book_feedback(None);
            }
            return Ok(());
        }
        let run = self.session.boneyard.as_ref().map(|world| world.loaded.run_id.as_str());
        let Some(participant) = self.session.participants.get(&self.local_player_id) else {
            self.book_feedback_scope = None;
            self.panels.reset_skill_book_feedback(None);
            return Ok(());
        };
        let feedback = participant.player.economy.action_feedback.as_ref();
        if self.book_feedback_scope.as_ref().is_none_or(|(player, previous_run)| {
            player != &self.local_player_id || previous_run.as_deref() != run
        }) {
            self.panels.reset_skill_book_feedback(feedback);
            self.book_feedback_scope = Some((self.local_player_id.clone(), run.map(str::to_owned)));
        } else {
            self.panels.observe_skill_book_feedback(feedback, now)?;
        }
        Ok(())
    }

    fn checkpoint(&self) -> Option<GameCheckpoint> {
        self.has_game.then(|| self.session.checkpoint())
    }

    #[cfg(target_arch = "wasm32")]
    async fn restore_checkpoint(&mut self, checkpoint: GameCheckpoint) -> Result<(), String> {
        let loaded = save_document::load_boneyard(&checkpoint)?;
        let battle = if let Some(loaded) = &loaded {
            let prepared = PreparedBoneyardView::load(loaded).await?;
            let navigation = crate::navigation_data::load_navigation(loaded).await?;
            let resolution = self.surface.scale_factor();
            let compact_masks = crate::compact_masks::CompactMaskView::new(&loaded.scene);
            let bounds = loaded.scene.bounds;
            let lights = BoneyardLightIndex::new(Vec2::new(
                bounds.x + bounds.width,
                bounds.y + bounds.height,
            ));
            let scenery = prepared.upload(
                &self.renderer,
                HUB_LOGICAL_VIEW,
                resolution,
                checkpoint.ticks,
                solomon_sim::game_run::game_run_world_tick(checkpoint.ticks as f64, &checkpoint.run)
                    as u64,
            )?;
            let weather = crate::boneyard_weather::BoneyardWeatherView::new(
                &self.renderer,
                checkpoint.ticks,
                loaded.scene.environment_mode,
                true,
            )?;
            Some(ActiveBattle {
                scene: BattleScene {
                    scenery,
                    lights,
                    weather,
                    compact_masks,
                    compact_frame_count: 0,
                },
                navigation,
            })
        } else {
            None
        };
        let local_player_id =
            checkpoint.participant_order.first().cloned().ok_or("The save has no wizard")?;
        let session = GameSession::restore(checkpoint, loaded)?;
        self.reset_world_views()?;
        self.session = session;
        self.world_overlays.reset();
        self.local_player_id = local_player_id;
        self.battle = battle;
        self.has_game = true;
        self.local_save.restored_at(self.session.ticks);
        self.refresh_hud();
        self.update_combat_audio()?;
        Ok(())
    }

    /// Offline games save locally; games on a native authority are saved by the host.
    pub fn request_save(&mut self) {
        self.local_save.request(self.has_game && self.native.is_none());
    }

    pub fn take_save(&mut self) -> Option<PendingSave> {
        let store = self.local_save.ready_store()?;
        let checkpoint = self.checkpoint()?;
        let document = match save_document::encode(&checkpoint) {
            Ok(document) => document,
            Err(error) => {
                self.save_completed(Err(error));
                return None;
            }
        };
        Some(self.local_save.begin(store, document, self.session.ticks))
    }

    pub fn save_completed(&mut self, result: Result<u64, String>) {
        if let Some(error) = self.local_save.finish(result) {
            Self::show_storage_error(&error);
        }
    }

    fn show_storage_error(error: &str) {
        crate::platform::warn(error);
        crate::runtime_ui::set_error(error);
    }

    fn display_session(&self) -> &GameSession {
        self.native
            .as_ref()
            .and_then(|native| native.display.as_ref())
            .map_or(&self.session, |display| display.session.as_ref())
    }

    fn scene_view(&self) -> (Camera, solomon_sim::viewport::GameViewport) {
        let session = self.display_session();
        let size = Vec2::new(
            f64::from(self.surface.logical_width().max(1)),
            f64::from(self.surface.logical_height().max(1)),
        );
        let native_zoom =
            if session.boneyard.is_some() { BONEYARD_CAMERA_SCALE } else { HUB_CAMERA_SCALE };
        let zoom = solomon_sim::settings::camera_zoom_for_fov(
            native_zoom,
            self.menu.settings().camera_fov_percent,
        );
        let world_size = session.boneyard.as_ref().map_or_else(
            || session.participants[&self.local_player_id].wizard.region.size(),
            |run| Vec2::new(run.loaded.scene.bounds.width, run.loaded.scene.bounds.height),
        );
        let viewport = solomon_sim::viewport::bounded_game_viewport(size, world_size, zoom);
        let scale = zoom * viewport.world_zoom;
        let origin = session.boneyard.as_ref().map_or_else(
            || {
                hub_region_camera_origin_for_view(
                    session.participants[&self.local_player_id].wizard.region,
                    session.participants[&self.local_player_id].wizard.position,
                    viewport.logical_size,
                    scale,
                )
            },
            |run| {
                run.camera_origin(
                    self.boneyard_camera_focus().position,
                    viewport.logical_size,
                    scale,
                )
            },
        );
        (Camera { origin, scale }, viewport)
    }

    pub fn gameplay_input_snapshot(&self) -> Vec<f64> {
        let Some(input) = &self.last_input else {
            return Vec::new();
        };
        let (kind, point) = match input.aim {
            None => (0.0, Vec2::ZERO),
            Some(GameplayAim::Pointer(point)) => (1.0, point),
            Some(GameplayAim::Direction(point)) => (2.0, point),
            Some(GameplayAim::Facing) => (3.0, Vec2::ZERO),
            Some(GameplayAim::Retain) => (4.0, Vec2::ZERO),
        };
        vec![
            input.movement.x,
            input.movement.y,
            f64::from(input.primary),
            input.quickbar.map_or(-1.0, |slot| slot as f64),
            kind,
            point.x,
            point.y,
            input.selected_belt_slot as f64,
            self.input_belt_press_count as f64,
            self.input_aim.map_or(f64::NAN, |point| point.x),
            self.input_aim.map_or(f64::NAN, |point| point.y),
        ]
    }

    fn render_world_and_notifications(&mut self) -> Result<(), String> {
        if self.has_game {
            let combat_hud = self.tutorial_access().is_none_or(|access| access.combat);
            self.notifications.update(&self.session, &self.local_player_id, combat_hud)?;
        } else {
            self.notifications.reset();
        }
        self.render_world()
    }

    fn finish_ready_frame(&mut self) -> Result<(), String> {
        if !self.ui_ready && !self.native.as_ref().is_some_and(NativeGame::is_inspecting) {
            let diagnostics = self.gpu_ui.diagnostics();
            if diagnostics.meshes > 0
                && diagnostics.static_images_ready == diagnostics.static_images_total
            {
                crate::runtime_ui::completed(crate::runtime_ui::ACCOUNT)?;
                crate::runtime_ui::completed(crate::runtime_ui::FIRST_FRAME)?;
                crate::runtime_ui::finish()?;
                self.ui_ready = true;
            }
        }
        self.capture_ui_snapshot();
        Ok(())
    }

    #[allow(clippy::type_complexity)] // The closure borrows the actual UI owners without allocation.
    fn ui_painter(
        &mut self,
    ) -> Result<(&mut GpuUi, impl FnMut(&mut UiFrame<'_>) -> Result<(), String> + '_), String> {
        let gameplay_keys_enabled = !(self.panels.is_open()
            || self.cheats.is_open()
            || self.social.input_active()
            || (!self.menu.is_blocking() && !self.can_pause())
            || self.skill_selection_active()
            || self.tutorial_access().is_some_and(|access| !access.combat));
        self.menu.set_gameplay_keys_enabled(gameplay_keys_enabled);
        self.configure_ui_layers()?;
        let Self {
            gpu_ui,
            mod_overlays,
            mod_minimap,
            hud,
            input,
            has_game,
            menu,
            panels,
            notifications,
            mod_skill_quickbar,
            shared_gameplay,
            mod_skills,
            skill_picker,
            mod_panels,
            tutorial_ui,
            college_overlay,
            game_over,
            social,
            cheats,
            observer_ui,
            spectator_ui,
            hosted,
            fullscreen,
            desktop_ui,
            match_loading_view,
            ..
        } = self;
        Ok((gpu_ui, move |frame: &mut UiFrame<'_>| {
            mod_overlays.paint(frame)?;
            mod_minimap.paint(frame)?;
            hud.paint(frame)?;
            input.paint(frame)?;
            if *has_game && !menu.covers_world() && !panels.covers_world() {
                notifications.paint(frame, f64::from(menu.settings().ui_scale_percent) / 100.0)?;
            }
            mod_skill_quickbar.paint(frame)?;
            shared_gameplay.paint(frame)?;
            panels.paint(frame)?;
            mod_skills.paint(frame)?;
            mod_panels.paint(frame)?;
            tutorial_ui.paint(frame)?;
            college_overlay.paint(frame)?;
            game_over.paint(frame)?;
            skill_picker.paint(frame)?;
            social.paint(frame)?;
            cheats.paint(frame)?;
            observer_ui.paint(frame)?;
            spectator_ui.paint(frame)?;
            menu.paint(frame)?;
            #[cfg(target_arch = "wasm32")]
            if let Some(hosted) = hosted {
                hosted.paint_boneyard_library(menu, frame)?;
            }
            fullscreen.paint(frame)?;
            if let Some(desktop) = desktop_ui {
                desktop.paint(frame, menu.desktop_controls_visible())?;
            }
            match_loading_view.paint(frame)?;
            if let Some(hosted) = hosted {
                hosted.deployment_update.paint(frame)?;
            }
            crate::runtime_ui::paint(frame);
            Ok(())
        }))
    }

    pub fn report_failure(&mut self, error: &str) -> Result<(), String> {
        crate::runtime_ui::set_error(error);
        self.input.set_blocked(true)?;
        if let Some(native) = &mut self.native {
            native.release_input(self.session.ticks)?;
        }
        Ok(())
    }

    pub fn present_failure(&mut self, time: f64) -> Result<(), String> {
        let (width, height) = self.fit_canvas();
        let viewport = [self.surface.logical_width() as f32, self.surface.logical_height() as f32];
        self.renderer.begin_frame(width, height, viewport);
        self.renderer.end_frame();
        self.gpu_ui.run(&mut self.renderer, viewport, time / 1000.0, |frame| {
            crate::runtime_ui::paint_failure(frame);
            if let Some(desktop) = &mut self.desktop_ui {
                desktop.paint(frame, true)?;
            }
            Ok(())
        })?;
        if !self.ui_ready {
            crate::runtime_ui::startup_failed(
                crate::runtime_ui::error().as_deref().unwrap_or("Game initialization failed"),
            );
        }
        self.capture_ui_snapshot();
        Ok(())
    }

    fn capture_ui_snapshot(&mut self) {
        self.ui_snapshot = Some(GameUiSnapshot {
            surface: self.menu.surface_name(),
            ready: self.ui_ready,
            selection: self
                .has_game
                .then(|| {
                    self.session
                        .participants
                        .get(&self.local_player_id)
                        .map(|participant| participant.player.selection.clone())
                })
                .flatten(),
            error: self.gpu_ui.diagnostics().error.clone().or_else(crate::runtime_ui::error),
            borrowed_image_paths: self.gpu_ui.borrowed_image_paths(),
        });
    }

    pub fn gpu_ui_snapshot(&self) -> serde_json::Value {
        let Some(snapshot) = &self.ui_snapshot else {
            return serde_json::Value::Null;
        };
        let mut value =
            self.gpu_ui.snapshot(snapshot.surface, snapshot.ready, snapshot.error.as_deref());
        value["worldOverlays"] = serde_json::json!(self.world_overlay_diagnostics);
        value["acceptedSelection"] = serde_json::json!(snapshot.selection);
        value["draw"]["borrowedImagePaths"] = serde_json::json!(snapshot.borrowed_image_paths);
        value
    }

    fn fit_canvas(&self) -> (i32, i32) {
        self.surface.fit_buffer()
    }
}
