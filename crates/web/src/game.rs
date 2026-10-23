//! Browser game owner: assets and UI project the authoritative simulation state.
//! Loading prepares resources before installation; fixed ticks remain in the Rust kernels.

use std::collections::{BTreeMap, BTreeSet};

mod cheats;
mod desktop;
pub(crate) mod editor_test;
mod hosted;
mod hosted_save;
mod native;
mod social;
mod spectator;
use native::{CommandFeedback, NativeGame};
use solomon_protocol::{ClientMessage, GameCommand};

use crate::battle::{BattleFrame, BattlePlayer, BattleScene, BattleViews};
use crate::college_intro::CollegeIntroOverlay;
use crate::fullscreen::{FullscreenControl, FullscreenPlacement};
use crate::game_over::{GameOverAnchor, GameOverOverlay};
use crate::player_panels::{
    HubDialogueContext, InventoryAttributes, PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot,
    PlayerPanels,
};
use crate::storage::SaveStore;
use crate::tutorial::TutorialUi;
use crate::tutorial_scene::{TutorialFrame, TutorialScene};
use crate::{set_status, storage};
use solomon_assets::parse_bundle;
use solomon_assets::save_document;
use solomon_graphics::assets;
use solomon_sim::Vec2;
use solomon_sim::appearance::StarterEquipmentAppearance;
use solomon_sim::audio::MusicScene;
use solomon_sim::audio::combat::{CombatAudioCursor, CombatAudioFrame, CombatAudioPlayer};
use solomon_sim::belt::{NativeBeltEntry, NativeBeltPotionKind, native_belt_potion_projection};
use solomon_sim::boneyard::BONEYARD_CAMERA_SCALE;
use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::boneyard::lighting::NativeBoneyardLightIndex;
use solomon_sim::economy::HubEconomyState;
use solomon_sim::economy::{HubInventoryAction, HubItemKind, HubTraderId};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::hub::depth::HubDepth;
use solomon_sim::hub::interaction::{
    HubInteractionAvailability, HubInteractionId, hub_interaction_at_point,
    hub_interaction_within_range, nearest_hub_interaction,
};
use solomon_sim::hub::painter::courtyard_player_sort_bias;
use solomon_sim::hub::teacher::hub_teacher_draw_plan;
use solomon_sim::hub::{HUB_CAMERA_SCALE, HUB_LOGICAL_VIEW, hub_region_camera_origin_for_view};
use solomon_sim::hud::{HudBeltEntry, HudState};
use solomon_sim::input::{
    GameplayAction, GameplayAim, GameplayInputSample, project_native_stick_aim,
    project_native_world_pointer,
};
use solomon_sim::menu::loading::{MatchLoadingFlow, MatchLoadingStage, MatchLoadingState};
use solomon_sim::painter::overlays::PainterOwnerOverlay;
use solomon_sim::profile::WizardElement;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline};
use solomon_sim::session::{
    GameCheckpoint, GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};
use solomon_sim::settings::GameSettings;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext};

use crate::audio::GameAudio;
use crate::boneyard::PreparedBoneyardView;
use crate::courtyard::{ActorPainters, Camera, CourtyardPass, CourtyardScene};
use crate::gpu_ui::GpuUi;
use crate::hotbar::HotbarSelection;
use crate::hud::{Hud, HudAction, HudMode};
use crate::input::MovementInput;
use crate::menu::{MenuAction, MenuUi};
use crate::npc::NpcView;
use crate::rooms::{PrivateRoomFrame, PrivateRoomScenes};
use crate::shared_gameplay::SharedGameplayUi;
use crate::skill_picker::{SkillPicker, SkillPickerAction};
use crate::social::SocialUi;
use crate::spells::SpellView;
use crate::teacher::{TeacherPass, TeacherView};
use crate::wizard::{WizardPlayerPresentation, WizardView};
use crate::world_overlays::{WorldOverlayFrame, WorldOverlayPlayer, WorldOverlaysView};
use crate::world_screen_feedback::{
    ScreenFeedbackScene, WorldScreenFeedback, WorldScreenFeedbackFrame,
};
use solomon_graphics::Renderer;
use solomon_sim::spells::presentation::SpellDrawQueue;

const LOCAL_PLAYER_ID: &str = "local";
const TICK_MILLISECONDS: f64 = 10.0;
/// Longest stall the loop catches up on before dropping time (a tab that was in the background).
const MAX_CATCH_UP_MILLISECONDS: f64 = 250.0;
/// The source host publishes an autosave every thirty seconds of active simulation.
const AUTOSAVE_TICKS: u64 = 3000;

pub struct Game {
    canvas: HtmlCanvasElement,
    renderer: Renderer,
    gpu_ui: GpuUi,
    ui_ready: bool,
    frame_time: f64,
    scene: CourtyardScene,
    rooms: PrivateRoomScenes,
    wizard_view: WizardView,
    teacher_view: TeacherView,
    spell_view: SpellView,
    world_overlays: WorldOverlaysView,
    world_screen_feedback: WorldScreenFeedback,
    npc_view: NpcView,
    session: GameSession,
    local_player_id: String,
    native: Option<NativeGame>,
    hosted: Option<hosted::HostedGame>,
    editor_test: Option<editor_test::EditorTestPhase>,
    desktop_bridge: Option<crate::desktop::DesktopBridge>,
    desktop_close_callback: Option<wasm_bindgen::closure::Closure<dyn FnMut() -> js_sys::Promise>>,
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
    level_up: BTreeMap<String, solomon_sim::player_presentation::NativeLevelUpPresentationTracker>,
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
    pending_world: Option<LoadedBoneyard>,
    world_loading: bool,
    world_generation: u64,
    save_store: Option<SaveStore>,
    save_revision: u64,
    save_requested: bool,
    save_in_flight: bool,
    last_saved_tick: u64,
    save_error: Option<String>,
    save_listeners: Vec<wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>>,
}

pub struct PendingSave {
    store: SaveStore,
    revision: u64,
    document: String,
}

impl PendingSave {
    pub async fn write(self) -> Result<u64, String> {
        self.store.write(self.document, self.revision).await
    }
}

struct ActiveBattle {
    scene: BattleScene,
    navigation: solomon_sim::boneyard::navigation::NavigationSet,
}

impl Game {
    pub async fn load() -> Result<Self, String> {
        crate::runtime_ui::begin(
            GpuUi::startup_asset_count() + GameAudio::startup_asset_count() + 3,
        )?;
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let playtime = if editor_test::configured(&document)? {
            None
        } else {
            Some(storage::PlaytimeRecorder::start(&window)?)
        };
        let canvas: HtmlCanvasElement = document
            .get_element_by_id("game")
            .ok_or("the page has no #game canvas")?
            .dyn_into()
            .map_err(|_| "#game is not a canvas")?;

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

        let renderer = Renderer::new(webgl2_context(&canvas)?)?;
        let gpu_ui = GpuUi::load(&document, &canvas, &renderer).await?;
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
        let economy = HubEconomyState::new(js_sys::Date::now() as i64, WizardElement::Ether);
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
        let hotbar = HotbarSelection::attach(&window)?;
        let input = MovementInput::attach(&window, hotbar.clone())?;
        input.set_blocked(true)?;
        input.set_primary_casting_enabled(false)?;
        let mut hud = Hud::attach(&document, hotbar.clone()).await?;
        hud.set_actions_enabled(true)?;

        hud.set_action_enabled(HudAction::Pause, true)?;
        let menu = MenuUi::attach(&document).await?;
        let panels = PlayerPanels::attach(&document, hotbar.clone()).await?;
        let skill_picker = SkillPicker::attach(&document).await?;
        set_status("Loading stock audio…");
        let audio = GameAudio::load(&document).await?;
        audio.set_scene(MusicScene::Title)?;
        let battle_views = BattleViews::load(&renderer, &spell_view).await?;
        let world_overlays = WorldOverlaysView::load(&renderer).await?;
        let tutorial_ui = TutorialUi::attach(&document).await?;
        let college_overlay = CollegeIntroOverlay::attach(&document).await?;
        let game_over = GameOverOverlay::attach(&document)?;
        let session = GameSession::new(
            LOCAL_PLAYER_ID.to_owned(),
            NewGameSelection {
                name: "Sirmin".into(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
            GameSessionOptions::default(),
        );
        let mut game = Self {
            canvas,
            renderer,
            gpu_ui,
            ui_ready: false,
            frame_time: 0.0,
            scene,
            rooms,
            wizard_view,
            teacher_view,
            spell_view,
            world_overlays,
            world_screen_feedback: WorldScreenFeedback::default(),
            npc_view,
            session,
            local_player_id: LOCAL_PLAYER_ID.into(),
            native: None,
            hosted: None,
            editor_test: None,
            desktop_bridge: None,
            desktop_close_callback: None,
            desktop_close: None,
            _playtime: playtime,
            mobile_layout_store: crate::mobile_layout::MobileUiLayoutStore::new(
                &window,
                &crate::hosted::WebsiteMount::configured(&window)?
                    .unwrap_or(crate::hosted::WebsiteMount::new("")?),
            )?,
            mod_assets: None,
            mod_content: crate::mod_content_projection::ModContentState::default(),
            mod_minimap: crate::mod_minimap::ModMinimapView::new(&document)?,
            mod_panels: crate::mod_panels::ModPanelsView::new(&document)?,
            mod_skills: crate::mod_skills::ModSkillsUi::new(&document)?,
            mod_skill_quickbar: crate::mod_skill_quickbar::ModSkillQuickbarView::new(&window)?,
            mod_audio: None,
            mod_overlays: crate::mod_world_overlays::ModWorldOverlays::new(&document)?,
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
            match_loading_view: crate::match_loading::MatchLoadingView::attach(&document)?,
            panels,
            skill_picker,
            social: SocialUi::attach(&document).await?,
            cheats: crate::cheats::CheatUi::attach(&document)?,
            pending_cheat_open: false,
            observer_ui: crate::observer::ObserverUi::attach(&document)?,
            spectator_camera: Default::default(),
            spectator_ui: crate::spectator::SpectatorStatusView::attach(&document).await?,
            shared_gameplay: SharedGameplayUi::attach(&document)?,
            fullscreen: FullscreenControl::attach(&document)?,
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
            pending_world: None,
            world_loading: false,
            world_generation: 0,
            save_store: None,
            save_revision: 0,
            save_requested: false,
            save_in_flight: false,
            last_saved_tick: 0,
            save_error: None,
            save_listeners: Vec::new(),
        };
        set_status("");
        game.update_mobile_layout()?;
        match storage::load_settings() {
            Ok(settings) => {
                game.apply_settings(&settings)?;
                game.menu.set_settings(settings)?;
            }
            Err(error) => Self::show_storage_error(&error),
        }
        if editor_test::configured(&document)? {
            game.load_editor_test(&window).await?;
            return Ok(game);
        }
        match storage::read_hall() {
            Ok(entries) => game.hall_entries = entries,
            Err(error) => Self::show_storage_error(&error),
        }
        if let Some(endpoint) = crate::desktop::configured(&window)? {
            game.load_hosted(&window, crate::hosted::WebsiteMount::default(), Some(endpoint))
                .await?;
            return Ok(game);
        }
        if let Some(mount) = crate::hosted::WebsiteMount::configured(&window)? {
            game.load_hosted(&window, mount, None).await?;
            return Ok(game);
        }
        if crate::network::native_host_requested(&window)? {
            game.native = Some(NativeGame::inspect(&window)?);
            game.menu.show_title(false)?;
            set_status("Checking saved game…");
            return Ok(game);
        }
        let mut no_saved_game = false;
        match SaveStore::open().await {
            Ok(store) => {
                let stored = store.read().await;
                game.save_store = Some(store);
                match stored {
                    Ok(Some(record)) => {
                        game.save_revision = record.revision;
                        let restored = match save_document::decode(&record.document) {
                            Ok(checkpoint) => game.restore_checkpoint(checkpoint).await,
                            Err(error) => Err(error),
                        };
                        if let Err(error) = restored {
                            game.save_error = Some(error.clone());
                            Self::show_storage_error(&error);
                        }
                    }
                    Ok(None) => {
                        no_saved_game = true;
                    }
                    Err(error) => {
                        game.save_error = Some(error.clone());
                        Self::show_storage_error(&error);
                    }
                }
            }
            Err(error) => {
                game.save_error = Some(error.clone());
                Self::show_storage_error(&error);
            }
        }
        game.menu.show_title(game.has_game)?;
        if no_saved_game && !storage::tutorial_declined().unwrap_or(true) {
            game.menu.offer_tutorial()?;
        }
        Ok(game)
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
            f64::from(wizard.region.native_id()),
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
        for student in students {
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
        let resolution = web_sys::window().map_or(1.0, |window| window.device_pixel_ratio());
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
        let lights = NativeBoneyardLightIndex::new(Vec2::new(
            bounds.x + bounds.width,
            bounds.y + bounds.height,
        ));
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
            self.world_loading = false;
            self.apply_native_frame(frame, Some(loaded))?;
            self.send_native_message(&ClientMessage::ReadyBoneyard { run_id })?;
            return Ok(());
        }
        self.session.install_boneyard(loaded)?;
        self.reset_world_views()?;
        self.battle = Some(ActiveBattle {
            scene: BattleScene { scenery, lights, weather, compact_masks, compact_frame_count: 0 },
            navigation,
        });
        self.world_loading = false;
        self.accumulator = 0.0;
        self.menu.hide()?;
        self.request_save();
        self.update_combat_audio()?;
        Ok(())
    }

    pub fn take_world_load(&mut self) -> Option<(u64, LoadedBoneyard)> {
        self.pending_world.take().map(|world| (self.world_generation, world))
    }

    pub fn world_load_current(&self, generation: u64) -> bool {
        self.world_generation == generation
    }

    pub fn world_load_failed(&mut self, error: &str) {
        self.world_loading = false;
        self.match_loading = None;
        set_status(error);
    }

    fn reset_world_views(&mut self) -> Result<(), String> {
        self.spectator_camera = Default::default();
        self.spectator_ui.hide();
        if let Some(previous) = self.battle.take() {
            previous.scene.scenery.destroy(&self.renderer);
            previous.scene.weather.destroy(&self.renderer);
            previous.scene.compact_masks.destroy(&self.renderer);
        }
        self.spell_view.reset();
        self.scene.reset_overlays();
        self.battle_views.reset();
        self.world_screen_feedback.reset();
        self.level_up.clear();
        if let Some(mut cursor) = self.combat_audio.take() {
            self.audio.apply_combat(cursor.destroy())?;
        }
        Ok(())
    }

    pub fn frame(&mut self, time: f64) -> Result<(), String> {
        self.frame_time = time;
        if self.editor_test_disposed() {
            return Ok(());
        }
        self.poll_native(time)?;
        self.poll_editor_test()?;
        if self.editor_test_disposed() {
            return Ok(());
        }
        // An embedding page can hide the iframe before its queued cancel message
        // arrives. Keep admission/disposal polling alive, but never lay out a
        // zero-sized canvas. A later visible frame resumes without catch-up time.
        if self.canvas.client_width() <= 0 || self.canvas.client_height() <= 0 {
            self.last_time = Some(time);
            return Ok(());
        }
        self.update_mobile_layout()?;
        self.poll_hosted(time)?;
        self.update_match_loading(time)?;
        self.menu.set_enhanced_effects_authority(self.enhanced_effects_authority())?;
        if self.hosted.as_ref().is_some_and(|hosted| hosted.deployment_update.restarting()) {
            self.input.set_blocked(true)?;
            return self.render();
        }
        self.hud.update_diagnostics(time, self.native.as_ref().and_then(NativeGame::ping_ms))?;
        let last = self.last_time.replace(time).unwrap_or(time);
        if self.is_hosted_observer() {
            return self.frame_hosted_observer(time);
        }
        let skills_were_open = self.panels.panel() == Some(PlayerPanel::Skills);
        let menu_was_open = self.menu.is_blocking();
        self.update_skill_picker(time)?;
        self.update_social(time)?;
        self.update_cheats()?;
        if self.panels.is_open()
            || self.cheats.is_open()
            || self.social.input_active()
            || (!self.menu.is_blocking() && !self.can_pause())
            || self.skill_selection_active()
            || self.tutorial_access().is_some_and(|access| !access.combat)
        {
            self.menu.discard_gameplay_keys();
        }
        self.menu.update(time, self.canvas.client_width(), self.canvas.client_height())?;
        if self.menu.is_create_pending() && self.match_loading.is_none() {
            self.begin_match_loading(MatchLoadingFlow::Hub, MatchLoadingStage::ConnectingTransport);
        }
        for action in self.menu.take_actions() {
            if let Err(error) = self.apply_menu_action(action) {
                self.menu.complete_create_loadout(false)?;
                self.match_loading = None;
                return Err(error);
            }
        }
        for action in self.hud.take_actions() {
            if self.skill_selection_active() {
                continue;
            }
            match action {
                HudAction::Pause if self.can_pause() => self.menu.show_pause()?,
                HudAction::Map if self.session.can_enter_boneyard() => self.queue_world(false)?,
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
                self.canvas.client_width(),
                self.canvas.client_height(),
            )?;
        }
        for action in self.panels.take_actions() {
            self.apply_panel_action(action, time)?;
        }
        if skills_were_open != (self.panels.panel() == Some(PlayerPanel::Skills)) {
            self.refresh_hosted_mod_skills()?;
        }
        self.update_audio()?;
        for action in self.game_over.take_actions() {
            if !self.has_game || self.menu.covers_world() || self.battle.is_none() {
                continue;
            }
            if self.is_editor_test() {
                self.return_to_editor()?;
            } else if self.native.is_some() {
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
        let blocked = self.native_gameplay_held()
            || self.cheats.is_open()
            || self.social.input_active()
            || self.menu.is_blocking()
            || self.panels.is_open()
            || self.skill_selection_active()
            || self.world_loading
            || self.match_loading.is_some()
            || !self.has_game;
        if self.menu.is_blocking() && !menu_was_open {
            self.request_save();
        }
        for slot in self.input.take_belt_unassignments() {
            if !blocked && self.can_edit_books() {
                self.hud.show_belt_pull_off(slot)?;
                self.audio.apply(solomon_sim::audio::AudioEvent::Play(
                    solomon_sim::audio::AudioCue::Poof,
                    solomon_sim::audio::AudioOptions::default(),
                ))?;
                self.apply_panel_action(
                    PlayerPanelAction::Belt(solomon_sim::belt::NativeBeltCommand::BindSkill {
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
        self.hud.set_visible(!self.menu.is_blocking() && !self.panels.covers_world());
        self.input.resize(
            self.canvas.client_width(),
            self.canvas.client_height(),
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
                return self.render();
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
            self.native_gameplay_held() || self.world_loading || !self.has_game,
        )?;
        self.update_spectator_camera(self.input.take_spectator_camera_advances());
        self.input_aim = self.project_input_aim(input.aim);
        self.input_belt_press_count += input.belt_pressed.len() as u64;
        for slot in &input.belt_pressed {
            // Held skills execute during the fixed tick; item activation is a press edge.
            if !matches!(
                self.session.participants[&self.local_player_id].player.belt.entries[*slot],
                Some(NativeBeltEntry::Skill { .. })
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
                return self.render();
            }
        }
        if self.native.is_some() {
            self.native_input_frame(session_input)?;
            self.refresh_hud();
            self.update_overlays(time)?;
            return self.render();
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
        if self.session.ticks.saturating_sub(self.last_saved_tick) >= AUTOSAVE_TICKS {
            self.request_save();
        }
        self.update_overlays(time)?;
        self.render()
    }

    fn update_mobile_layout(&mut self) -> Result<(), String> {
        if self.mobile_layout_store.take_changed() {
            let state = self.mobile_layout_store.read()?;
            self.hud.set_mobile_layout(&state)?;
            self.input.set_mobile_layout(&state)?;
            self.menu.set_mobile_layout_state(&state)?;
        }
        Ok(())
    }

    fn apply_menu_action(&mut self, mut action: MenuAction) -> Result<(), String> {
        if let MenuAction::SettingsChanged(settings) = &mut action
            && !self.request_hosted_settings(settings)?
        {
            return Ok(());
        }
        if self.is_editor_test() && action == MenuAction::Leave {
            return self.return_to_editor();
        }
        if self.hosted_menu_action(&action)? {
            return Ok(());
        }
        if self.native_menu_action(&action)? {
            return Ok(());
        }
        match action {
            MenuAction::Start(selection) => {
                if self.has_game {
                    if !self.session.reset_participant(&self.local_player_id, selection) {
                        return Err(
                            "The current shared run must finish before replacing this wizard"
                                .into(),
                        );
                    }
                } else {
                    self.local_player_id = LOCAL_PLAYER_ID.into();
                    self.session = GameSession::new(
                        self.local_player_id.clone(),
                        selection,
                        GameSessionOptions {
                            enhanced_effects: self.menu.settings().enhanced_effects,
                            ..Default::default()
                        },
                    );
                }
                self.reset_world_views()?;
                self.has_game = true;
                self.session.arm_college_intro(&self.local_player_id);
                self.world_generation += 1;
                self.pending_world = None;
                self.world_loading = false;
                self.accumulator = 0.0;
                if self.save_store.is_some() {
                    self.save_error = None;
                    set_status("");
                }
                self.last_saved_tick = 0;
                self.request_save();
                self.panels.hide()?;
                self.refresh_hud();
                self.update_combat_audio()?;
                self.menu.complete_create_loadout(true)?;
            }
            MenuAction::ConfirmLoadout(selection) => {
                if !self.session.confirm_loadout(&self.local_player_id, selection) {
                    return Err("The College loadout is not ready".into());
                }
                self.accumulator = 0.0;
                self.request_save();
                self.refresh_hud();
                self.menu.complete_create_loadout(true)?;
            }
            MenuAction::StartTutorial => {
                self.reset_world_views()?;
                self.local_player_id = LOCAL_PLAYER_ID.into();
                self.session = GameSession::new(
                    LOCAL_PLAYER_ID.to_owned(),
                    NewGameSelection {
                        name: "Sirmin".into(),
                        element: WizardElement::Ether,
                        discipline: WizardDiscipline::Arcane,
                    },
                    GameSessionOptions {
                        enhanced_effects: self.menu.settings().enhanced_effects,
                        ..Default::default()
                    },
                );
                self.has_game = true;
                self.queue_world(true)?;
            }
            MenuAction::TutorialDeclined => {
                storage::save_tutorial_declined()?;
            }
            MenuAction::Continue | MenuAction::Resume | MenuAction::CancelCreate => {}
            MenuAction::Cheats => self.pending_cheat_open = true,
            MenuAction::Leave => {
                self.request_save();
                self.menu.show_title(self.has_game)?;
            }
            MenuAction::SettingsChanged(settings) => {
                self.commit_settings(*settings)?;
            }
            MenuAction::SetEnhancedEffects { enabled } => {
                if self.enhanced_effects_authority().is_some_and(|authority| !authority.can_change)
                {
                    return Ok(());
                }
                let mut settings = self.menu.settings().clone();
                settings.enhanced_effects = enabled;
                self.commit_settings(settings)?;
                if self.has_game {
                    if self.native.is_some() {
                        self.send_native_message(&solomon_protocol::ClientMessage::Hosted {
                            command: solomon_protocol::hosted::HostedCommand::SetEnhancedEffects {
                                enabled,
                            },
                        })?;
                    } else {
                        self.session.set_enhanced_effects(enabled);
                        self.request_save();
                    }
                }
            }
            MenuAction::SaveMobileLayout(layout) => {
                let result = match layout {
                    Some(layout) => self.mobile_layout_store.set_layout(&layout),
                    None => self.mobile_layout_store.reset(),
                };
                self.menu.complete_mobile_layout_save(result)?;
                self.update_mobile_layout()?;
            }
        }
        Ok(())
    }

    fn skill_selection_active(&self) -> bool {
        self.skill_picker.is_open()
            || self
                .has_game
                .then(|| &self.session.participants[&self.local_player_id].player)
                .is_some_and(|player| player.progression.pending_offer.is_some())
    }

    fn simulation_is_paused(&self) -> bool {
        // The source host ignores gameplay-pause in the Hub. Panels seal the local
        // input there, while the College's students, NPCs and onboarding keep ticking.
        !self.has_game
            || self.world_loading
            || self.menu.covers_world()
            || self.skill_selection_active()
            || (self.session.boneyard.is_some()
                && (self.menu.is_blocking() || self.panels.is_open()))
    }

    fn tutorial_access(&self) -> Option<solomon_sim::boneyard::tutorial::NativeTutorialHudAccess> {
        self.session
            .boneyard
            .as_ref()
            .and_then(|world| world.tutorial.as_ref())
            .map(solomon_sim::boneyard::tutorial::native_tutorial_hud_access)
    }

    fn can_edit_books(&self) -> bool {
        self.has_game
            && !self.cheats.is_open()
            && self.native_modal_available()
            && self.session.can_edit_books(&self.local_player_id)
    }

    /// `MainMenuScene.useHotbarShortcut`: optional books retain the owner's bank
    /// selection while modal, loading and shared readiness barriers prevent cycling.
    fn hotbar_available(&self) -> bool {
        self.has_game
            && self.native_modal_available()
            && !self.cheats.is_open()
            && !self.social.input_active()
            && !self.menu.is_blocking()
            && !self.menu.settings_open()
            && !self.world_loading
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
        if self.is_editor_test() {
            return;
        }
        let Some(world) = &self.session.boneyard else { return };
        // MainMenuScene.recordHallSnapshot excludes the entire Tutorial run,
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
            js_sys::Date::new_0().to_iso_string().into(),
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
        if self.world_loading {
            return Ok(());
        }
        let mut seed = [0; 16];
        web_sys::window()
            .ok_or("no window")?
            .crypto()
            .map_err(|_| "world seed source")?
            .get_random_values_with_u8_array(&mut seed)
            .map_err(|_| "world seed generation")?;
        let loaded = if tutorial {
            solomon_assets::boneyard::load_tutorial(seed)
        } else {
            solomon_assets::boneyard::load_boneyard(seed)
        }?;
        self.world_generation += 1;
        self.pending_world = Some(loaded);
        self.world_loading = true;
        self.begin_match_loading(MatchLoadingFlow::Boneyard, MatchLoadingStage::MaterializingWorld);
        set_status("Loading Boneyard…");
        Ok(())
    }

    fn begin_match_loading(&mut self, flow: MatchLoadingFlow, stage: MatchLoadingStage) {
        if let Some(loading) = &mut self.match_loading
            && loading.flow == flow
        {
            loading.advance(stage);
            return;
        }
        let now = web_sys::window()
            .and_then(|window| window.performance())
            .map_or(0.0, |clock| clock.now());
        self.match_loading = Some(MatchLoadingState::begin(flow, stage, now));
    }

    fn update_match_loading(&mut self, now: f64) -> Result<(), String> {
        if let Some(loading) = &mut self.match_loading {
            let admitted = self.has_game && self.native.as_ref().is_none_or(NativeGame::is_playing);
            let prepared = !self.world_loading
                && !self.menu.is_create_pending()
                && self
                    .hosted
                    .as_ref()
                    .is_none_or(|hosted| !hosted.connecting && !hosted.content_pending);
            let world_ready = loading.flow == MatchLoadingFlow::Hub || self.battle.is_some();
            if admitted && prepared && world_ready {
                let participants_pending = self
                    .native
                    .as_ref()
                    .and_then(NativeGame::host_snapshot)
                    .is_some_and(|host| !host.materializing_player_ids.is_empty());
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
            |intro| {
                intro.phase
                    == solomon_sim::hub::college_intro::NativeCollegeIntroPhase::ArchDialogue
            },
        ) && !self.panels.is_open()
        {
            self.open_hub_dialogue(HubInteractionId::ArchChancellor, now)?;
        }
        Ok(())
    }

    fn update_combat_audio(&mut self) -> Result<(), String> {
        if !self.has_game {
            return Ok(());
        }
        let session = &self.session;
        let world_keys: Vec<_> = session
            .participant_order
            .iter()
            .map(|id| session.world_key(id).expect("ordered participant"))
            .collect();
        let players: Vec<_> = session
            .participant_order
            .iter()
            .zip(&world_keys)
            .map(|(id, world_key)| {
                let participant = &session.participants[id];
                CombatAudioPlayer {
                    id,
                    world_key,
                    position: participant.wizard.position,
                    element: participant.player.selection.element,
                    selected_primary_skill_id: participant.player.book.primary_skill_id as u32,
                    weld_build_id: participant.player.book.weld_build_id,
                    cast: &participant.wizard.primary_cast,
                    footstep_tick: participant.wizard.footstep_tick,
                }
            })
            .collect();
        let staff_contacts: Vec<_> = session
            .spells
            .transients
            .iter()
            .filter_map(|effect| {
                if let solomon_sim::spells::model::PrimarySpellTransient::Staff(
                    solomon_sim::spells::staff::NativePlayerStaffTransient::Contact(event),
                ) = effect
                {
                    Some(event.clone())
                } else {
                    None
                }
            })
            .collect();
        let (camera, viewport) = self.scene_view();
        let center = Vec2::new(
            camera.origin.x + viewport.logical_size.x / (2.0 * camera.scale),
            camera.origin.y + viewport.logical_size.y / (2.0 * camera.scale),
        );
        let width = viewport.logical_size.x / camera.scale;
        let alternate = session.participants[&self.local_player_id].player.combat.life_state
            != solomon_sim::combat::PlayerLifeState::Alive;
        let point_gain = |point| {
            solomon_sim::region_feedback::native_region_point_gain(point, center, width, alternate)
        };
        let hit_gain = |point| {
            solomon_sim::region_feedback::native_region_hit_point_gain(
                point, center, width, alternate,
            )
        };
        let world = session.boneyard.as_ref();
        let frame = CombatAudioFrame {
            tick: session.ticks,
            run_id: world.map(|world| world.loaded.run_id.as_str()),
            listener_id: &self.local_player_id,
            players: &players,
            primary: &session.spells,
            secondary: &session.secondary,
            staff_contacts: &staff_contacts,
            enemies: world.map(|world| &world.enemies),
            enemy_events: world.map_or(&[], |world| world.enemy_events.as_slice()),
            loot_events: world.map_or(&[], |world| world.loot_events.as_slice()),
            encounter: world.and_then(|world| world.encounter.as_ref()),
            point_gain: &point_gain,
            hit_gain: &hit_gain,
        };
        let cursor = self.combat_audio.get_or_insert_with(|| CombatAudioCursor::new(&frame));
        let mut output = cursor.update(&frame);
        if self.native.is_some() {
            // Reliable host audio owns one-shots; snapshots project only continuous voices.
            output.events.clear();
        }
        self.audio.apply_combat(output)
    }

    fn update_overlays(&mut self, now: f64) -> Result<(), String> {
        self.update_shared_gameplay(now)?;
        let college_admission = self.has_game
            && !self.menu.covers_world()
            && self.session.boneyard.is_none()
            && self.session.participants[&self.local_player_id]
                .player
                .economy
                .college_intro_pending;
        let panel_hides_fullscreen = matches!(
            self.panels.panel(),
            Some(PlayerPanel::Skills | PlayerPanel::SkillSelector(_))
        ) || (self.session.boneyard.is_none()
            && self.panels.is_open());
        self.fullscreen.update(
            if college_admission || panel_hides_fullscreen || self.social.input_active() {
                FullscreenPlacement::Hidden
            } else {
                self.menu.fullscreen_placement()
            },
        )?;
        let (camera, layout) = self.scene_view();
        let width = self.canvas.client_width();
        let height = self.canvas.client_height();
        self.hud.set_mode(if self.session.boneyard.is_some() {
            HudMode::Run
        } else {
            HudMode::Hub
        })?;
        self.hud.set_tutorial_access(self.tutorial_access().map(|access| {
            crate::hud::HudAccess {
                combat: access.combat,
                inventory: access.inventory,
                quickbar: access.quickbar,
                skills: access.skills,
                spell: access.spell,
            }
        }))?;
        self.hud.set_action_enabled(HudAction::Pause, self.can_pause())?;
        self.hud.set_action_enabled(
            HudAction::Map,
            self.session.can_enter_boneyard()
                && !self.session.participants[&self.local_player_id]
                    .player
                    .economy
                    .college_intro_pending,
        )?;
        self.hud.set_quickbar_player(
            self.has_game.then(|| self.session.participants[&self.local_player_id].player.as_ref()),
            self.session.secondary.players.get(&self.local_player_id),
            &self.menu.settings().controls,
            self.session.boneyard.is_none(),
        )?;
        self.hud.update(&self.hud_state, width, height)?;
        self.college_overlay.update(
            self.session.participants[&self.local_player_id]
                .college_intro
                .as_ref()
                .filter(|_| !self.menu.covers_world()),
            width,
            height,
        )?;
        let position = self
            .display_session()
            .wizard(&self.local_player_id)
            .expect("local participant")
            .position;
        self.game_over.update(
            (self.has_game && !self.menu.covers_world() && self.battle.is_some())
                .then_some(&self.session.run),
            GameOverAnchor {
                position: Vec2::new(
                    (position.x - camera.origin.x) * camera.scale * layout.display_scale,
                    (position.y - camera.origin.y) * camera.scale * layout.display_scale,
                ),
                zoom: camera.scale * layout.display_scale,
            },
            now,
        )?;
        self.tutorial_scene.update(
            &mut self.tutorial_ui,
            &self.audio,
            self.session.boneyard.as_ref().filter(|_| !self.menu.covers_world()).map(|world| {
                TutorialFrame {
                    world,
                    player: &self.session.participants[&self.local_player_id].player,
                    wizard: self.session.wizard(&self.local_player_id).expect("local participant"),
                    camera: &camera,
                    settings: self.menu.settings(),
                    coarse_pointer: web_sys::window()
                        .is_some_and(|window| window.navigator().max_touch_points() > 0),
                }
            }),
            now,
            width,
            height,
        )
    }

    fn update_audio(&mut self) -> Result<(), String> {
        self.audio.set_volumes(
            self.menu.settings().sound_volume_percent,
            self.menu.settings().music_volume_percent,
        );
        let world_scene = match &self.session.boneyard {
            None => MusicScene::Hub,
            Some(run) if run.encounter.as_ref().is_some_and(|state| state.combat_enabled()) => {
                MusicScene::Combat
            }
            Some(_) => MusicScene::Boneyard,
        };
        self.audio.set_scene(self.menu.music_scene().unwrap_or(world_scene))?;
        for event in self.menu.take_audio_events() {
            self.audio.apply(event)?;
        }
        for event in self.skill_picker.take_audio_events() {
            self.audio.apply(event)?;
        }
        for event in self.panels.take_audio_events() {
            self.audio.apply(event)?;
        }
        self.audio.update()
    }

    fn update_skill_picker(&mut self, now: f64) -> Result<(), String> {
        let was_open = self.skill_picker.is_open();
        // A restored offer waits behind Continue; it must not cover the title screen.
        let player = self
            .has_game
            .then(|| &self.session.participants[&self.local_player_id].player)
            .filter(|_| !self.menu.covers_world());
        self.skill_picker.update(
            player.and_then(|player| player.progression.pending_offer.as_ref()),
            player.is_some_and(|player| player.progression.sorcerors_charm_available),
            now,
            self.canvas.client_width(),
            self.canvas.client_height(),
        )?;
        if self.skill_picker.is_open() && !was_open {
            self.panels.hide()?;
            self.request_save();
        }
        for action in self.skill_picker.take_actions() {
            if self.native.is_some() {
                let command = match action {
                    SkillPickerAction::Choose(choice) => GameCommand::ChooseSkill { choice },
                    SkillPickerAction::Reroll { offer_sequence } => {
                        GameCommand::RerollSkillOffer { offer_sequence }
                    }
                    SkillPickerAction::Save { offer_sequence } => {
                        GameCommand::DeferSkillChoice { offer_sequence }
                    }
                };
                self.send_native_command(command, CommandFeedback::SkillPicker)?;
                continue;
            }
            let accepted = self.has_game
                && match action {
                    SkillPickerAction::Choose(choice) => {
                        self.session.choose_skill(&self.local_player_id, choice)
                    }
                    SkillPickerAction::Reroll { offer_sequence } => {
                        self.session.reroll_skill_offer(&self.local_player_id, offer_sequence)
                    }
                    SkillPickerAction::Save { offer_sequence } => {
                        self.session.defer_skill_choice(&self.local_player_id, offer_sequence)
                    }
                };
            if accepted {
                self.refresh_hud();
                self.request_save();
            } else {
                self.skill_picker.reject_action(now);
            }
        }
        Ok(())
    }

    fn apply_panel_action(&mut self, action: PlayerPanelAction, now: f64) -> Result<(), String> {
        // `BoneyardScene` publishes surface acknowledgements only while a Tutorial
        // exists. Ordinary Inventory/Skill Book use must not send rejected lesson commands.
        if matches!(action, PlayerPanelAction::Surface(_))
            && self.session.boneyard.as_ref().is_none_or(|world| world.tutorial.is_none())
        {
            return Ok(());
        }
        if matches!(
            action,
            PlayerPanelAction::SelectPrimary(_)
                | PlayerPanelAction::SelectConcentration(_)
                | PlayerPanelAction::SelectConcentrationSlot { .. }
                | PlayerPanelAction::Belt(_)
        ) && !self.can_edit_books()
        {
            return Ok(());
        }
        if self.native.is_some() {
            let command = match &action {
                PlayerPanelAction::Surface(action) => {
                    Some(GameCommand::TutorialAction { action: *action })
                }
                PlayerPanelAction::AcknowledgeCollegeIntroDialogue => {
                    Some(GameCommand::AcknowledgeCollegeIntroDialogue)
                }
                PlayerPanelAction::SelectPrimary(skill_id) => {
                    Some(GameCommand::SelectPrimary { skill_id: *skill_id })
                }
                PlayerPanelAction::SelectConcentration(skill_id) => {
                    Some(GameCommand::SelectConcentration { skill_id: *skill_id })
                }
                PlayerPanelAction::SelectConcentrationSlot { skill_id, slot } => {
                    Some(GameCommand::SelectConcentrationSlot {
                        skill_id: *skill_id,
                        slot: match slot {
                            0 => solomon_sim::profile::PlayerConcentrationSlot::A,
                            1 => solomon_sim::profile::PlayerConcentrationSlot::B,
                            _ => return Err("The concentration slot must be A or B".into()),
                        },
                    })
                }
                PlayerPanelAction::Belt(command) => {
                    Some(GameCommand::BindBelt { command: *command })
                }
                PlayerPanelAction::Inventory(action) => {
                    Some(GameCommand::Inventory { action: action.clone() })
                }
                _ => None,
            };
            if let Some(command) = command {
                let feedback = if let PlayerPanelAction::Inventory(action) = action {
                    CommandFeedback::Inventory(action)
                } else {
                    CommandFeedback::None
                };
                self.send_native_command(command, feedback)?;
                return Ok(());
            }
        }
        match action {
            PlayerPanelAction::OpenInventory
                if !self.menu.is_blocking()
                    && !self.skill_selection_active()
                    && self.can_open_panel(PlayerPanel::Inventory) =>
            {
                self.panels.show(PlayerPanel::Inventory, now)?;
            }
            PlayerPanelAction::OpenSkills
                if !self.menu.is_blocking()
                    && !self.skill_selection_active()
                    && self.can_open_panel(PlayerPanel::Skills) =>
            {
                self.panels.show(PlayerPanel::Skills, now)?;
            }
            PlayerPanelAction::OpenInventory
            | PlayerPanelAction::OpenSkills
            | PlayerPanelAction::Close => {}
            PlayerPanelAction::Surface(action) => {
                if self.session.apply_tutorial_action(action) {
                    self.request_save();
                }
            }
            PlayerPanelAction::AcknowledgeCollegeIntroDialogue => {
                if self.session.acknowledge_college_intro_dialogue(&self.local_player_id) {
                    self.request_save();
                }
            }
            PlayerPanelAction::SelectPrimary(id) => {
                if self.has_game {
                    self.session.select_primary(&self.local_player_id, id);
                }
            }
            PlayerPanelAction::SelectConcentration(id) => {
                if self.has_game {
                    self.session.select_concentration(&self.local_player_id, id);
                }
            }
            PlayerPanelAction::SelectConcentrationSlot { skill_id, slot } => {
                use solomon_sim::profile::runtime::PlayerConcentrationSlot;
                let slot = match slot {
                    0 => PlayerConcentrationSlot::A,
                    1 => PlayerConcentrationSlot::B,
                    _ => return Err("The concentration slot must be A or B".into()),
                };
                self.session.select_concentration_slot(&self.local_player_id, skill_id, slot);
            }
            PlayerPanelAction::Belt(command) => {
                if self.has_game {
                    self.session.bind_belt(&self.local_player_id, command);
                }
            }
            PlayerPanelAction::ActivateBeltSlot { slot, pointer } => {
                let bounds = self.canvas.get_bounding_client_rect();
                let pointer = pointer.and_then(|point| {
                    (bounds.width() > 0.0 && bounds.height() > 0.0).then(|| {
                        Vec2::new(
                            (point.x - bounds.left()) / bounds.width(),
                            (point.y - bounds.top()) / bounds.height(),
                        )
                    })
                });
                let aim = self.project_input_aim(Some(self.input.secondary_aim(pointer)));
                return self.activate_belt_slot(slot, aim, now);
            }
            PlayerPanelAction::Inventory(action) => {
                if !self.has_game {
                    return Ok(());
                }
                let applied = self.session.apply_inventory(&self.local_player_id, &action);
                self.panels.inventory_action_feedback(
                    &action,
                    &applied.transaction,
                    now,
                    self.mod_content.current(),
                );
            }
        }
        self.refresh_hud();
        self.request_save();
        Ok(())
    }

    fn apply_settings(&mut self, settings: &GameSettings) -> Result<(), String> {
        self.hud.set_ui_scale(f64::from(settings.ui_scale_percent) / 100.0);
        self.input.set_controls(settings.controls.clone())?;
        self.panels.set_controls(settings.controls.clone());
        self.social.set_controls(settings.controls.clone());
        self.input.set_secondary_at_pointer(settings.cast_secondaries_at_mouse);
        Ok(())
    }

    /// `MainMenuScene.enhancedEffectsAuthority`: the public College changes the
    /// next party run; an admitted private or active game changes its own mode.
    fn enhanced_effects_authority(
        &self,
    ) -> Option<solomon_protocol::hosted::EnhancedEffectsAuthority> {
        use solomon_protocol::hosted::{EnhancedEffectsAuthority, EnhancedEffectsScope};
        if !self.has_game {
            return None;
        }
        let host = self.native.as_ref().and_then(NativeGame::host_snapshot);
        let next_party_run = self.session.boneyard.is_none()
            && self.hosted.as_ref().and_then(|hosted| hosted.welcome.as_ref()).is_some_and(
                |welcome| welcome.session_kind == solomon_sim::social::GameSessionKind::GlobalHub,
            );
        let party = self
            .native
            .as_ref()
            .and_then(|native| native.social.as_ref())
            .and_then(|social| social.party_state.as_ref());
        Some(EnhancedEffectsAuthority {
            enabled: if next_party_run {
                party.map_or(self.session.enhanced_effects, |state| state.party.enhanced_effects)
            } else {
                self.session.enhanced_effects
            },
            can_change: !self.is_hosted_observer()
                && host.is_none_or(|host| {
                    host.authority_player_id.as_deref() == Some(self.local_player_id.as_str())
                        && !host.materializing_player_ids.contains(&self.local_player_id)
                }),
            scope: if next_party_run {
                EnhancedEffectsScope::NextPartyRun
            } else {
                EnhancedEffectsScope::Game
            },
        })
    }

    fn commit_settings(&mut self, settings: GameSettings) -> Result<(), String> {
        self.send_hosted_settings(&settings)?;
        self.apply_settings(&settings)?;
        self.menu.set_settings(settings.clone())?;
        if let Err(error) = storage::save_settings(&settings) {
            Self::show_storage_error(&error);
        }
        Ok(())
    }

    fn checkpoint(&self) -> Option<GameCheckpoint> {
        self.has_game.then(|| self.session.checkpoint())
    }

    async fn restore_checkpoint(&mut self, checkpoint: GameCheckpoint) -> Result<(), String> {
        let loaded = save_document::load_boneyard(&checkpoint)?;
        let battle = if let Some(loaded) = &loaded {
            let prepared = PreparedBoneyardView::load(loaded).await?;
            let navigation = crate::navigation_data::load_navigation(loaded).await?;
            let resolution = web_sys::window().map_or(1.0, |window| window.device_pixel_ratio());
            let compact_masks = crate::compact_masks::CompactMaskView::new(&loaded.scene);
            let bounds = loaded.scene.bounds;
            let lights = NativeBoneyardLightIndex::new(Vec2::new(
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
        self.last_saved_tick = self.session.ticks;
        self.refresh_hud();
        self.update_combat_audio()?;
        Ok(())
    }

    pub fn request_save(&mut self) {
        self.save_requested = self.has_game && self.native.is_none();
    }

    pub fn take_save(&mut self) -> Option<PendingSave> {
        if !self.save_requested || self.save_in_flight || self.save_error.is_some() {
            return None;
        }
        let store = self.save_store.as_ref()?.clone();
        let checkpoint = self.checkpoint()?;
        let document = match save_document::encode(&checkpoint) {
            Ok(document) => document,
            Err(error) => {
                self.save_completed(Err(error));
                return None;
            }
        };
        self.save_requested = false;
        self.save_in_flight = true;
        self.last_saved_tick = self.session.ticks;
        Some(PendingSave { store, revision: self.save_revision, document })
    }

    pub fn save_completed(&mut self, result: Result<u64, String>) {
        self.save_in_flight = false;
        match result {
            Ok(revision) => self.save_revision = revision,
            Err(error) => {
                Self::show_storage_error(&error);
                self.save_error = Some(error);
            }
        }
    }

    fn show_storage_error(error: &str) {
        web_sys::console::warn_1(&JsValue::from_str(error));
        crate::runtime_ui::set_error(error);
    }

    pub fn retain_save_listener(
        &mut self,
        listener: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
    ) {
        self.save_listeners.push(listener);
    }

    fn activate_belt_slot(
        &mut self,
        slot: usize,
        aim: Option<Vec2>,
        now: f64,
    ) -> Result<(), String> {
        if !self.can_edit_books()
            || self.tutorial_access().is_some_and(|access| !access.quickbar)
            || !matches!(self.session.run.phase, GameRunPhase::Hub | GameRunPhase::Active)
        {
            return Ok(());
        }
        let player = &self.session.participants[&self.local_player_id].player;
        let Some(entry) = player.belt.entries.get(slot).copied().flatten() else {
            return Ok(());
        };
        let action = if matches!(entry, NativeBeltEntry::Skill { .. }) {
            None
        } else {
            let Some(item) = solomon_sim::belt::native_belt_entry_item(entry, &player.economy)
            else {
                return Ok(());
            };
            if item.native_type_id == 7001 {
                Some(HubInventoryAction::Consume { item_id: item.id })
            } else if item.kind == HubItemKind::Sack {
                Some(HubInventoryAction::EquipSackContents { item_id: item.id })
            } else {
                solomon_sim::belt::native_belt_equipment_slots(
                    item,
                    player.economy.owned_perk_selectors.contains(&19),
                )
                .first()
                .map(|slot| HubInventoryAction::Equip { item_id: item.id, slot: *slot })
            }
        };
        if self.native.is_some() {
            return self.send_native_command(
                GameCommand::ActivateBeltSlot { slot, aim },
                action.map_or(CommandFeedback::None, CommandFeedback::Inventory),
            );
        }
        let applied = self.session.activate_belt_slot(&self.local_player_id, slot, aim);
        if let Some(action) = action {
            self.panels.inventory_action_feedback(
                &action,
                &applied.transaction,
                now,
                self.mod_content.current(),
            );
        }
        self.refresh_hud();
        self.request_save();
        Ok(())
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
            f64::from(self.canvas.client_width().max(1)),
            f64::from(self.canvas.client_height().max(1)),
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

    fn project_input_aim(&self, aim: Option<GameplayAim>) -> Option<Vec2> {
        let (camera, viewport) = self.scene_view();
        let wizard =
            self.display_session().wizard(&self.local_player_id).expect("local participant");
        match aim {
            None => None,
            Some(GameplayAim::Retain) => self.input_aim,
            Some(GameplayAim::Direction(direction)) => project_native_stick_aim(
                direction,
                wizard.position,
                viewport.logical_size,
                camera.scale,
            ),
            Some(GameplayAim::Facing) => project_native_stick_aim(
                solomon_sim::heading::heading_vector(wizard.heading_index),
                wizard.position,
                viewport.logical_size,
                camera.scale,
            ),
            Some(GameplayAim::Pointer(point)) => {
                self.pointer_to_world(point, &camera, viewport.logical_size)
            }
        }
    }

    fn pointer_to_world(&self, point: Vec2, camera: &Camera, viewport: Vec2) -> Option<Vec2> {
        let width = f64::from(self.canvas.client_width());
        let height = f64::from(self.canvas.client_height());
        project_native_world_pointer(
            Vec2::new(point.x * width, point.y * height),
            solomon_sim::hud::HudRect::new(0.0, 0.0, width, height),
            viewport,
            camera.origin,
            camera.scale,
        )
    }

    fn interact_at_pointer(&mut self, now: f64) -> Result<(), String> {
        // Always drain this input edge; a click in another world cannot open a College NPC later.
        let point = self.input.take_world_click();
        if !self.has_game
            || self.session.boneyard.is_some()
            || self.panels.is_open()
            || self.cheats.is_open()
            || self.social.input_active()
            || self.menu.is_blocking()
            || self.skill_selection_active()
            || self.native_gameplay_held()
            || self.world_loading
            || self.session.participants[&self.local_player_id].wizard.transition.is_some()
        {
            return Ok(());
        }
        let (camera, viewport) = self.scene_view();
        let Some(point) =
            point.and_then(|point| self.pointer_to_world(point, &camera, viewport.logical_size))
        else {
            return Ok(());
        };
        if self.open_hub_player(Some(point)) {
            return Ok(());
        }
        let availability = self.interaction_availability();
        let Some(interaction) = hub_interaction_at_point(
            self.session.participants[&self.local_player_id].wizard.region,
            point,
            availability,
        ) else {
            return Ok(());
        };
        if hub_interaction_within_range(
            interaction,
            self.session.participants[&self.local_player_id].wizard.region,
            self.session.participants[&self.local_player_id].wizard.position,
            availability,
        ) {
            self.open_hub_dialogue(interaction, now)?;
        }
        Ok(())
    }

    fn interaction_availability(&self) -> HubInteractionAvailability {
        HubInteractionAvailability {
            skorcha_position: self
                .session
                .npcs
                .skorcha
                .skorcha
                .as_ref()
                .map(|skorcha| skorcha.position),
            skorcha_variant: self
                .session
                .npcs
                .skorcha
                .skorcha
                .as_ref()
                .map(|skorcha| skorcha.variant),
            story_office: self
                .has_game
                .then(|| &self.session.participants[&self.local_player_id].player)
                .is_some_and(|player| player.economy.college_intro_pending),
        }
    }

    fn open_hub_dialogue(&mut self, interaction: HubInteractionId, now: f64) -> Result<(), String> {
        if !self.has_game {
            return Ok(());
        }
        let player = &self.session.participants[&self.local_player_id].player;
        if self.session.boneyard.is_some()
            || self.session.participants[&self.local_player_id].wizard.transition.is_some()
        {
            return Ok(());
        }
        // HubInventorySurface seeds UI dialogue independently of the gameplay RNG.
        let random_index = if interaction == HubInteractionId::Skorcha {
            self.session.npcs.skorcha.skorcha.as_ref().map_or(0, |skorcha| skorcha.dismissal_index)
                as i64
                - 1
        } else {
            (player.economy.revision + player.economy.npc.boast.failure_sequence) as i64
        };
        let context = HubDialogueContext {
            interaction,
            story_office: player.economy.college_intro_pending,
            eulogy_override: None,
            random_index,
        };
        if interaction == HubInteractionId::ArchChancellor
            && self.session.participants[&self.local_player_id].college_intro.is_some_and(|intro| {
                intro.phase
                    == solomon_sim::hub::college_intro::NativeCollegeIntroPhase::ArchDialogue
            })
        {
            self.panels.show_college_intro_dialogue(context, &player.economy.npc, now)
        } else {
            self.panels.show_dialogue(context, &player.economy.npc, now)
        }
    }

    fn refresh_hud(&mut self) {
        if !self.has_game {
            self.menu.set_mobile_editor_player(None, true);
            self.menu.set_mobile_editor_secondary(None);
            return;
        }
        let player = &self.session.participants[&self.local_player_id].player;
        let secondary = self.session.secondary.players.get(&self.local_player_id);
        self.menu.set_mobile_editor_player(Some(player), self.session.boneyard.is_none());
        self.menu.set_mobile_editor_secondary(secondary);
        let hud = &mut self.hud_state;
        hud.current_health = player.combat.current_health;
        hud.maximum_health = player.combat.maximum_health;
        hud.current_mana = player.combat.current_mana;
        hud.maximum_mana = player.combat.maximum_mana;
        hud.shield_current = secondary.map_or(0.0, |state| state.magic_shield_absorb);
        hud.shield_maximum = secondary.map_or(0.0, |state| state.magic_shield_maximum);
        hud.reserved_mana = secondary.map_or(0.0, |state| state.reserved_mana);
        hud.poisoned = player.combat.poison_ticks_remaining > 0;
        hud.experience_progress = player.progression.experience_progress();
        hud.gold = player.economy.gold.min(u64::from(u32::MAX)) as u32;
        hud.primary_icon_record = player.book.skill_icon_record(player.book.primary_skill_id);
        hud.concentration_icon_records = [
            player.skill_runtime.concentration_skill_id_a,
            player.skill_runtime.concentration_skill_id_b,
        ]
        .map(|id| id.map(|id| player.book.skill_icon_record(id)));
        for (slot, entry) in player
            .belt
            .entries
            .iter()
            .skip(self.hotbar.start())
            .take(solomon_sim::belt::NATIVE_BELT_SLOT_COUNT)
            .enumerate()
        {
            hud.belt[slot] = match entry {
                Some(NativeBeltEntry::Skill { skill_id }) => Some(HudBeltEntry::Skill {
                    record: player.book.skill_icon_record(*skill_id),
                    unavailable: self.session.boneyard.is_none()
                        || !player.combat.can_cast()
                        || secondary.is_some_and(|state| {
                            state.global_cooldown_ticks > 0.0
                                || state.cooldown_ticks_by_skill[*skill_id] > 0.0
                        }),
                }),
                Some(NativeBeltEntry::HealthPotion) => Some(HudBeltEntry::HealthPotion {
                    count: native_belt_potion_projection(
                        &player.economy.backpack,
                        NativeBeltPotionKind::Health,
                    )
                    .count as u32,
                }),
                Some(NativeBeltEntry::ManaPotion) => Some(HudBeltEntry::ManaPotion {
                    count: native_belt_potion_projection(
                        &player.economy.backpack,
                        NativeBeltPotionKind::Mana,
                    )
                    .count as u32,
                }),
                Some(entry @ NativeBeltEntry::Item { .. }) => {
                    solomon_sim::belt::native_belt_entry_item(*entry, &player.economy).map(|item| {
                        HudBeltEntry::Item {
                            record: item.icon_records.first().copied(),
                            count: item.quantity,
                        }
                    })
                }
                None => None,
            };
        }
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

    fn render(&mut self) -> Result<(), String> {
        self.render_world()?;
        self.paint_ui()?;
        if !self.ui_ready && !self.native.as_ref().is_some_and(NativeGame::is_inspecting) {
            let diagnostics = self.gpu_ui.diagnostics();
            if diagnostics.meshes > 0
                && diagnostics.static_images_ready == diagnostics.static_images_total
            {
                crate::runtime_ui::completed("account", "Account and saved game ready", 1)?;
                crate::runtime_ui::completed("ready", "First game frame ready", 1)?;
                crate::runtime_ui::finish()?;
                self.ui_ready = true;
            }
        }
        Ok(())
    }

    fn paint_ui(&mut self) -> Result<(), String> {
        let viewport = [self.canvas.client_width() as f32, self.canvas.client_height() as f32];
        let mut result = Ok(());
        self.gpu_ui.run(&mut self.renderer, viewport, self.frame_time / 1000.0, |frame| {
            result = (|| {
                self.mod_overlays.paint(frame)?;
                self.mod_minimap.paint(frame)?;
                self.hud.paint(frame)?;
                self.input.paint(frame)?;
                self.mod_skill_quickbar.paint(frame)?;
                self.shared_gameplay.paint(frame)?;
                self.panels.paint(frame)?;
                self.mod_skills.paint(frame)?;
                self.mod_panels.paint(frame)?;
                self.tutorial_ui.paint(frame)?;
                self.college_overlay.paint(frame)?;
                self.game_over.paint(frame)?;
                self.skill_picker.paint(frame)?;
                self.social.paint(frame)?;
                self.cheats.paint(frame)?;
                self.observer_ui.paint(frame)?;
                self.spectator_ui.paint(frame)?;
                self.menu.paint(frame)?;
                if let Some(hosted) = &mut self.hosted {
                    hosted.paint_boneyard_library(&self.menu, frame)?;
                }
                self.fullscreen.paint(frame)?;
                if let Some(desktop) = &mut self.desktop_bridge {
                    desktop.paint(frame)?;
                }
                self.match_loading_view.paint(frame)?;
                if let Some(hosted) = &mut self.hosted {
                    hosted.deployment_update.paint(frame)?;
                }
                crate::runtime_ui::paint(frame);
                Ok(())
            })();
        })?;
        result?;
        self.input.set_ui_geometry(self.gpu_ui.diagnostics(), &self.hud.belt_input_geometry());
        Ok(())
    }

    pub(crate) fn present_failure(&mut self, time: f64) -> Result<(), String> {
        let (width, height) = self.fit_canvas();
        let viewport = [self.canvas.client_width() as f32, self.canvas.client_height() as f32];
        self.renderer.begin_frame(width, height, viewport);
        self.renderer.end_frame();
        self.gpu_ui.run(&mut self.renderer, viewport, time / 1000.0, crate::runtime_ui::paint)?;
        if !self.ui_ready {
            crate::runtime_ui::startup_failed(
                crate::runtime_ui::error().as_deref().unwrap_or("Game initialization failed"),
            );
        }
        Ok(())
    }

    pub(crate) fn gpu_ui_snapshot(&self) -> serde_json::Value {
        let ui = self.gpu_ui.diagnostics();
        let widgets: Vec<_> = ui
            .widgets
            .iter()
            .map(|widget| {
                let rect = |bounds: crate::gpu_ui::WidgetBounds| {
                    [bounds.x, bounds.y, bounds.width, bounds.height]
                };
                serde_json::json!({
                    "id": widget.id, "label": widget.label, "role": widget.role,
                    "rect": rect(widget.bounds), "paintRect": rect(widget.paint_bounds),
                    "enabled": widget.enabled, "focused": widget.focused,
                    "value": widget.value, "selected": widget.selected,
                    "modal": widget.modal, "z": widget.z,
                })
            })
            .collect();
        serde_json::json!({
            "schemaVersion": 1, "surface": self.menu.surface_name(), "ready": self.ui_ready,
            "loading": null, "error": ui.error.clone().or_else(crate::runtime_ui::error),
            "canvas": { "width": ui.viewport[0], "height": ui.viewport[1], "pixelsPerPoint": ui.pixels_per_point },
            "capture": ui.capture,
            "inputScope": ui.input_scope, "frame": ui.frame,
            "draw": { "meshes": ui.meshes, "triangles": ui.triangles, "textures": ui.textures },
            "widgets": widgets,
        })
    }

    fn render_world(&mut self) -> Result<(), String> {
        let display = self.native.as_ref().and_then(|native| native.display.as_ref());
        let session = display.map_or(&self.session, |display| display.session.as_ref());
        let presentation_tick = display.map_or(session.ticks as f64, |frame| frame.tick);
        let clocks = display.map(|frame| &frame.clocks);
        self.hud.set_mode(if session.boneyard.is_some() { HudMode::Run } else { HudMode::Hub })?;
        if !self.has_game || self.menu.covers_world() {
            self.spectator_ui.hide();
            self.mod_overlays.reset();
            self.mod_projection = None;
            let (width, height) = self.fit_canvas();
            self.renderer.set_native_world_pixel_centers(false);
            self.renderer.begin_frame(
                width,
                height,
                [self.canvas.client_width() as f32, self.canvas.client_height() as f32],
            );
            self.renderer.end_frame();
            return Ok(());
        }
        let player = &session.participants[&self.local_player_id].player;
        let equipment = crate::wizard::WizardEquipment::prepare(
            &mut self.renderer,
            player.selection.element,
            &player.economy.equipment,
            self.mod_assets.as_mut(),
        )?;
        self.wizard_view.set_equipment(&equipment);
        self.wizard_view.set_material_tint(solomon_sim::presentation::native_player_material_tint(
            0xffffff,
            session.secondary.players.get(&self.local_player_id),
            &player.combat,
        ));
        self.wizard_view.set_player_presentation(WizardPlayerPresentation {
            player,
            secondary: session.secondary.players.get(&self.local_player_id),
            webbed: session
                .boneyard
                .as_ref()
                .and_then(|world| world.enemies.webbed_players.get(&self.local_player_id).copied()),
            overlay_effect_phase: session.participants[&self.local_player_id]
                .lighting
                .overlay_effect_phase,
            // Active Boneyard actions are sampled per participant by BattleViews.
            staff_action_pose: None,
            complex_lighting: self.menu.settings().complex_lighting,
            tick: presentation_tick,
            damage_x4_ticks_remaining: clocks
                .and_then(|clocks| clocks.player_damage_x4.get(&self.local_player_id))
                .copied()
                .unwrap_or(player.progression.damage_x4_ticks_remaining as f64),
        });
        let (camera, layout) = self.scene_view();
        let spectator_status =
            spectator::status(session, &self.local_player_id, &self.spectator_camera);
        self.spectator_ui.update(spectator_status.as_ref(), layout)?;
        let viewport = layout.logical_size;
        let (width, height) = self.fit_canvas();
        self.renderer.set_native_world_pixel_centers(true);
        self.renderer.begin_frame(width, height, [viewport.x as f32, viewport.y as f32]);
        if self.panels.covers_world() {
            self.renderer.set_native_world_pixel_centers(false);
            let client_width = self.canvas.client_width().max(1) as f32;
            let client_height = self.canvas.client_height().max(1) as f32;
            self.renderer.begin_frame(width, height, [client_width, client_height]);
            self.renderer.set_saturation(1.0);
            let ratio_x = f64::from(width) / f64::from(client_width);
            let ratio_y = f64::from(height) / f64::from(client_height);
            let clip = |bounds: solomon_sim::hud::HudRect| {
                [
                    (bounds.x * ratio_x).round() as i32,
                    (f64::from(height) - (bounds.y + bounds.height) * ratio_y).round() as i32,
                    (bounds.width * ratio_x).round() as i32,
                    (bounds.height * ratio_y).round() as i32,
                ]
            };
            if let Some(underlay) = self.panels.inventory_preview_underlay()? {
                let texture = self
                    .gpu_ui
                    .image_texture(underlay.texture_path)
                    .ok_or("The stock Inventory seal is not ready")?;
                let rect = |r: solomon_sim::hud::HudRect| {
                    [r.x as f32, r.y as f32, r.width as f32, r.height as f32]
                };
                self.renderer.draw_clipped(
                    clip(underlay.clip),
                    underlay.alpha as f32,
                    |renderer| {
                        renderer.draw(
                            &texture,
                            solomon_graphics::Quad {
                                source: rect(underlay.source),
                                destination: rect(underlay.destination),
                                tint: [
                                    ((underlay.tint >> 16) & 255) as f32 / 255.0,
                                    ((underlay.tint >> 8) & 255) as f32 / 255.0,
                                    (underlay.tint & 255) as f32 / 255.0,
                                    1.0,
                                ],
                                blend: solomon_graphics::BlendMode::Normal,
                            },
                        );
                    },
                );
            }
            if let Some(preview) = self.panels.inventory_preview_placement() {
                self.renderer.draw_clipped(clip(preview.clip), preview.alpha as f32, |renderer| {
                    self.wizard_view.draw_preview(
                        renderer,
                        &Camera { origin: Vec2::ZERO, scale: preview.scale },
                        Vec2::new(
                            preview.center.x / preview.scale,
                            preview.center.y / preview.scale,
                        ),
                        &self.spell_view,
                    );
                });
            }
            self.renderer.end_frame();
            return Ok(());
        }
        let camera_center = Vec2::new(
            camera.origin.x + viewport.x / (2.0 * camera.scale),
            camera.origin.y + viewport.y / (2.0 * camera.scale),
        );
        let screen_feedback = self.world_screen_feedback.update(WorldScreenFeedbackFrame {
            scene: if session.boneyard.is_some() {
                ScreenFeedbackScene::Boneyard
            } else {
                ScreenFeedbackScene::Hub
            },
            world_key: &session
                .world_key(&self.local_player_id)
                .ok_or("The display has no local world")?,
            tick: presentation_tick,
            stopped_world_tick: solomon_sim::game_run::game_run_world_tick(
                presentation_tick,
                &session.run,
            ),
            flame_epoch: display.and_then(|frame| frame.flame_epoch),
            flame_camera_displacements: display
                .map_or(&[], |frame| frame.flame_camera_displacements.as_slice()),
            screen_flashes: &session.screen_flashes,
            spells: &session.spells,
            secondary: &session.secondary,
            clocks,
            context: solomon_sim::screen_feedback::NativeSecondaryScreenFeedbackContext {
                camera_center,
                visible_world_width: viewport.x / camera.scale,
                local_player_alternate: player.combat.life_state
                    != solomon_sim::combat::PlayerLifeState::Alive,
            },
        });
        let host = self.native.as_ref().and_then(NativeGame::host_snapshot);
        let local_region = session.participants[&self.local_player_id].wizard.region;
        let overlay_players: Vec<_> = session
            .participant_order
            .iter()
            .filter_map(|id| {
                let participant = &session.participants[id];
                if session.boneyard.is_none() && participant.wizard.region != local_region {
                    return None;
                }
                Some(WorldOverlayPlayer {
                    id,
                    name: &participant.player.selection.name,
                    element: participant.player.selection.element,
                    position: participant.wizard.position,
                    current_health: participant.player.combat.current_health,
                    maximum_health: participant.player.combat.maximum_health,
                    activity: if session.boneyard.is_some() {
                        None
                    } else {
                        host.and_then(|host| host.hub_activities.get(id)).copied()
                    },
                })
            })
            .collect();
        let mut overlay_diagnostics = Default::default();
        if let Some(world) = &session.boneyard {
            let camera_focus = self.boneyard_camera_focus();
            // The source owns one level-up view driven by this client's picker.
            self.level_up.retain(|id, _| id == &self.local_player_id);
            let local = &session.participants[&self.local_player_id];
            let level_up = self.level_up.entry(self.local_player_id.clone()).or_default().sample(
                &world.loaded.run_id,
                &local.player,
                session.level_up_barrier.as_ref().map(|barrier| barrier.barrier_id),
                self.last_time.unwrap_or(0.0),
                (local.wizard.position.y - camera.origin.y) * camera.scale,
            );
            let players: Vec<_> = session
                .participant_order
                .iter()
                .map(|id| {
                    let participant = &session.participants[id];
                    let lighting = &participant.lighting;
                    BattlePlayer {
                        id,
                        player: &participant.player,
                        wizard: &participant.wizard,
                        light_registration: lighting.light_registration,
                        // `host/game-snapshot.ts` projects both action and weapon phases.
                        overlay_effect_phase: participant
                            .wizard
                            .primary_cast
                            .weapon_pulse
                            .max(lighting.overlay_effect_phase),
                        drive_active: participant.wizard.primary_cast.owns_facing()
                            || matches!(
                                participant.player.combat.life_state,
                                solomon_sim::combat::PlayerLifeState::Dying
                                    | solomon_sim::combat::PlayerLifeState::Spectating
                            ),
                        materializing: host
                            .is_some_and(|host| host.materializing_player_ids.contains(id)),
                        blindness_ticks_remaining: lighting.blindness_ticks_remaining,
                        level_up: if id == &self.local_player_id {
                            level_up.as_ref()
                        } else {
                            None
                        },
                    }
                })
                .collect();
            self.battle_views.draw(
                &mut self.renderer,
                &mut self.battle.as_mut().expect("prepared active Boneyard").scene,
                &mut self.spell_view,
                &mut self.wizard_view,
                self.mod_assets.as_mut(),
                &BattleFrame {
                    enhanced_effects: session.enhanced_effects,
                    mod_effects: &session.mod_effects,
                    mod_catalog: self
                        .hosted
                        .as_ref()
                        .and_then(|hosted| hosted.welcome.as_ref())
                        .map_or(&[], |welcome| welcome.mod_catalog.as_slice()),
                    world,
                    presentation_tick,
                    stopped_world_tick: solomon_sim::game_run::game_run_world_tick(
                        presentation_tick,
                        &session.run,
                    ),
                    clocks,
                    enemy_presentation: display.and_then(|frame| frame.enemies.as_ref()),
                    level_up_barrier_active: session.level_up_barrier.is_some(),
                    local_player_id: &self.local_player_id,
                    camera_focus_player_id: camera_focus.player_id.as_deref(),
                    scenery_timeline: self.native.as_ref().map(NativeGame::presentation_timeline),
                    players: &players,
                    spells: &session.spells,
                    secondary: &session.secondary,
                    camera: &camera,
                    viewport,
                    settings: self.menu.settings(),
                    screen_feedback,
                    application_time_ms: self.last_time.unwrap_or(0.0),
                    device_resolution: web_sys::window()
                        .map_or(1.0, |window| window.device_pixel_ratio()),
                },
                |renderer, camera| {
                    overlay_diagnostics = self.world_overlays.draw(
                        renderer,
                        &WorldOverlayFrame {
                            world_key: &world.loaded.run_id,
                            players: &overlay_players,
                            local_player_id: &self.local_player_id,
                            camera,
                            viewport,
                            now_ms: self.last_time.unwrap_or(0.0),
                            renderable: true,
                        },
                    );
                },
            )?;
        } else {
            let base_camera = camera;
            let transform =
                solomon_sim::boneyard::presentation::native_enemy_world_feedback_transform(
                    camera_center,
                    base_camera.scale,
                    viewport,
                    session.participants[&self.local_player_id].wizard.position,
                    if self.menu.settings().zoom_effects {
                        screen_feedback.camera_magnitude
                    } else {
                        0.0
                    },
                );
            let displacement = if self.menu.settings().zoom_effects {
                screen_feedback.camera_displacement
            } else {
                Vec2::ZERO
            };
            let camera = Camera {
                origin: Vec2::new(
                    -(transform.position.x + displacement.x) / transform.scale,
                    -(transform.position.y + displacement.y) / transform.scale,
                ),
                scale: transform.scale,
            };
            let hub_players: Vec<_> = session
                .participant_order
                .iter()
                .map(|id| &session.participants[id])
                .filter(|participant| {
                    participant.wizard.region
                        == session.participants[&self.local_player_id].wizard.region
                })
                .collect();
            let mut hub_equipment = Vec::with_capacity(hub_players.len());
            for participant in &hub_players {
                hub_equipment.push(crate::wizard::WizardEquipment::prepare(
                    &mut self.renderer,
                    participant.player.selection.element,
                    &participant.player.economy.equipment,
                    self.mod_assets.as_mut(),
                )?);
            }
            let player_painters: Vec<_> = hub_players
                .iter()
                .map(|participant| {
                    participant.lighting.light_registration.layer(
                        participant.wizard.position.y,
                        if participant.wizard.region.is_private() {
                            0.0
                        } else {
                            courtyard_player_sort_bias(
                                participant.wizard.position,
                                participant.wizard.heading_index as i64,
                            )
                        },
                    )
                })
                .collect();
            if session.participants[&self.local_player_id].wizard.region.is_private() {
                self.rooms.draw(
                    &mut self.renderer,
                    &camera,
                    &PrivateRoomFrame {
                        local_wizard: &session.participants[&self.local_player_id].wizard,
                        player_painters: &player_painters,
                        tick: session.ticks,
                        dowser_frame: session.dowser_clock.frame(),
                    },
                    |renderer, index| {
                        let participant = hub_players[index];
                        prepare_hub_wizard(
                            &mut self.wizard_view,
                            participant,
                            &hub_equipment[index],
                            session.secondary.players.get(&participant.id),
                            self.menu.settings().complex_lighting,
                            presentation_tick,
                            clocks,
                        );
                        self.wizard_view.draw(
                            renderer,
                            &camera,
                            &participant.wizard,
                            presentation_tick,
                            &self.spell_view,
                        );
                    },
                );
            } else {
                let teacher = hub_teacher_draw_plan(
                    session.ambient.teacher_tick,
                    0,
                    session.ambient.teacher_world_release,
                );
                let teacher_painters = TeacherView::painters(&teacher);
                let npc_painters = self.npc_view.painters(&session.npcs);
                self.spell_view.prepare(
                    &session.spells,
                    &format!(
                        "hub:{}",
                        session.participants[&self.local_player_id].wizard.region.id()
                    ),
                    session.ticks,
                    &|_| 1.0,
                    &[],
                    clocks,
                    session.enhanced_effects,
                    Some(solomon_sim::spells::weld::NativeFlameLashBounds {
                        x: base_camera.origin.x,
                        y: base_camera.origin.y,
                        width: viewport.x / base_camera.scale,
                        height: viewport.y / base_camera.scale,
                    }),
                );
                let marker_availability = HubInteractionAvailability {
                    skorcha_position: session
                        .npcs
                        .skorcha
                        .skorcha
                        .as_ref()
                        .map(|skorcha| skorcha.position),
                    skorcha_variant: session
                        .npcs
                        .skorcha
                        .skorcha
                        .as_ref()
                        .map(|skorcha| skorcha.variant),
                    story_office: player.economy.college_intro_pending,
                };
                let mut actors = player_painters;
                let teacher_start = actors.len();
                for (painter, _) in &teacher_painters {
                    actors.push(*painter);
                }
                let npc_start = actors.len();
                for (painter, _) in &npc_painters {
                    actors.push(*painter);
                }
                let spell_start = actors.len();
                let spell_painters: Vec<_> = self
                    .spell_view
                    .painters()
                    .map(|(index, painter)| (index, painter.layer))
                    .collect();
                actors.extend(spell_painters.iter().map(|(_, layer)| *layer));
                let overlays: Vec<_> = self
                    .spell_view
                    .painters()
                    .enumerate()
                    .filter_map(|(offset, (_, painter))| {
                        let owner = painter.overlay_owner_id.as_deref()?;
                        let owner_draw_id =
                            hub_players.iter().position(|participant| participant.id == owner)?;
                        Some(PainterOwnerOverlay {
                            draw_id: spell_start + offset,
                            owner_draw_id,
                            view_id: painter.view_id,
                        })
                    })
                    .collect();
                let live_views: Vec<_> = self.spell_view.live_view_creation_orders().collect();
                self.scene.draw(
                    &mut self.renderer,
                    &camera,
                    &session.ambient,
                    session.participants[&self.local_player_id].wizard.position.y,
                    &ActorPainters {
                        layers: &actors,
                        owner_overlays: &overlays,
                        live_views: &live_views,
                    },
                    |renderer, pass| match pass {
                        CourtyardPass::Actor(index) => {
                            if index < teacher_start {
                                let participant = hub_players[index];
                                prepare_hub_wizard(
                                    &mut self.wizard_view,
                                    participant,
                                    &hub_equipment[index],
                                    session.secondary.players.get(&participant.id),
                                    self.menu.settings().complex_lighting,
                                    presentation_tick,
                                    clocks,
                                );
                                self.wizard_view.draw(
                                    renderer,
                                    &camera,
                                    &participant.wizard,
                                    presentation_tick,
                                    &self.spell_view,
                                );
                            } else if index < npc_start {
                                let pass = teacher_painters[index - teacher_start].1;
                                self.teacher_view.draw(renderer, &camera, &teacher, pass);
                                if matches!(pass, TeacherPass::Actor) {
                                    self.npc_view.draw_courtyard_marker(
                                        renderer,
                                        &camera,
                                        session.participants[&self.local_player_id]
                                            .marker_epoch
                                            .frame(
                                                HubInteractionId::Teacher,
                                                session.ticks,
                                                marker_availability,
                                                None,
                                            ),
                                    );
                                }
                            } else if index < spell_start {
                                let actor = npc_painters[index - npc_start].1;
                                self.npc_view.draw_actor(renderer, &camera, &session.npcs, actor);
                                if let Some(id) = actor.interaction() {
                                    self.npc_view.draw_courtyard_marker(
                                        renderer,
                                        &camera,
                                        session.participants[&self.local_player_id]
                                            .marker_epoch
                                            .frame(id, session.ticks, marker_availability, None),
                                    );
                                }
                            } else {
                                self.spell_view.draw(
                                    renderer,
                                    &camera,
                                    spell_painters[index - spell_start].0,
                                    [1.0; 3],
                                );
                            }
                        }
                        CourtyardPass::Band(HubDepth::TeacherPreWorld) => {
                            self.teacher_view.draw(renderer, &camera, &teacher, TeacherPass::Flare);
                            self.spell_view.draw_queue(renderer, &camera, SpellDrawQueue::PreWorld);
                        }
                        CourtyardPass::Band(HubDepth::TeacherPostWorld) => {
                            self.teacher_view.draw(renderer, &camera, &teacher, TeacherPass::Core);
                            self.spell_view.draw_queue(
                                renderer,
                                &camera,
                                SpellDrawQueue::PostWorld,
                            );
                        }
                        CourtyardPass::Band(HubDepth::UsefulThyngsShadow) => {
                            self.npc_view.draw_tent_shadow(renderer, &camera);
                        }
                        CourtyardPass::Band(_) => {}
                    },
                );
            }
            let nameplate_camera = Camera {
                origin: Vec2::new(
                    -transform.position.x / transform.scale,
                    -transform.position.y / transform.scale,
                ),
                scale: transform.scale,
            };
            overlay_diagnostics = self.world_overlays.draw(
                &mut self.renderer,
                &WorldOverlayFrame {
                    world_key: "hub",
                    players: &overlay_players,
                    local_player_id: &self.local_player_id,
                    camera: &nameplate_camera,
                    viewport,
                    now_ms: self.last_time.unwrap_or(0.0),
                    renderable: !player.economy.college_intro_pending,
                },
            );
            if let Some(overlay) =
                solomon_sim::screen_feedback::present_native_secondary_screen_overlay(
                    screen_feedback.overlay,
                    self.menu.settings().reduced_screen_flashes,
                )
            {
                self.renderer.draw_solid(
                    [0.0, 0.0, viewport.x as f32, viewport.y as f32],
                    [
                        ((overlay.color >> 16) & 255) as f32 / 255.0,
                        ((overlay.color >> 8) & 255) as f32 / 255.0,
                        (overlay.color & 255) as f32 / 255.0,
                        overlay.alpha as f32,
                    ],
                );
            }
        }
        overlay_diagnostics.apply(&self.canvas)?;
        if let Some(transition) = session.participants[&self.local_player_id].wizard.transition {
            self.renderer.draw_solid(
                [0.0, 0.0, viewport.x as f32, viewport.y as f32],
                [0.0, 0.0, 0.0, transition.alpha as f32],
            );
        }
        self.renderer.end_frame();
        self.record_mod_scene_projection()?;
        self.hud.update(
            &self.hud_state,
            self.canvas.client_width(),
            self.canvas.client_height(),
        )?;
        Ok(())
    }

    /// Matches the drawing buffer to the canvas's CSS size and device pixel ratio.
    fn fit_canvas(&self) -> (i32, i32) {
        let ratio = web_sys::window().map_or(1.0, |window| window.device_pixel_ratio());
        let width = (f64::from(self.canvas.client_width()) * ratio).round().max(1.0) as u32;
        let height = (f64::from(self.canvas.client_height()) * ratio).round().max(1.0) as u32;
        if self.canvas.width() != width {
            self.canvas.set_width(width);
        }
        if self.canvas.height() != height {
            self.canvas.set_height(height);
        }
        (width as i32, height as i32)
    }
}

fn webgl2_context(canvas: &HtmlCanvasElement) -> Result<WebGl2RenderingContext, String> {
    let options = js_sys::Object::new();
    for (key, value) in [("alpha", false), ("antialias", false), ("premultipliedAlpha", false)] {
        js_sys::Reflect::set(&options, &JsValue::from_str(key), &JsValue::from_bool(value))
            .map_err(|_| "context options")?;
    }
    canvas
        .get_context_with_context_options("webgl2", &options)
        .map_err(|error| format!("webgl2 context: {error:?}"))?
        .ok_or("this browser has no WebGL2")?
        .dyn_into()
        .map_err(|_| "webgl2 context has the wrong type".to_string())
}

/// One shared view is prepared from each actual Hub participant immediately before drawing.
fn prepare_hub_wizard(
    view: &mut WizardView,
    participant: &solomon_sim::session::SessionParticipant,
    equipment: &crate::wizard::WizardEquipment,
    secondary: Option<&solomon_sim::secondary::NativeSecondaryPlayerState>,
    complex_lighting: bool,
    tick: f64,
    clocks: Option<&crate::presentation::PresentationClocks>,
) {
    let player = &participant.player;
    view.set_equipment(equipment);
    view.set_material_tint(solomon_sim::presentation::native_player_material_tint(
        0xffffff,
        secondary,
        &player.combat,
    ));
    view.set_player_presentation(WizardPlayerPresentation {
        player,
        secondary,
        webbed: None,
        overlay_effect_phase: participant.lighting.overlay_effect_phase,
        staff_action_pose: None,
        complex_lighting,
        tick,
        damage_x4_ticks_remaining: clocks
            .and_then(|clocks| clocks.player_damage_x4.get(&participant.id))
            .copied()
            .unwrap_or(player.progression.damage_x4_ticks_remaining as f64),
    });
}
