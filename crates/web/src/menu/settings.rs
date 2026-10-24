//! GPU `GameSettingsDialog` pages. Draft preferences leave through MenuUi;
//! storage, account admission and simulation authority retain their existing owners.
mod fullscreen;
mod mobile;
mod mobile_sharing;
mod transfer;

use super::widgets;
use crate::gpu_ui::{StockAtlas, UiFrame};
use crate::mobile_editor::{MobileEditorArt, MobileEditorPlayerPreview, MobileUiEditor};
use egui::{Color32, Pos2, Rect, Ui, Vec2};
use fullscreen::SettingsFullscreen;
use solomon_protocol::hosted::{EnhancedEffectsAuthority, EnhancedEffectsScope};
use solomon_sim::hud::quickbar::NativeSkillQuickbarCooldownPresentation;
use solomon_sim::mobile_ui::{MobileUiLayout, MobileUiLayoutState};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use solomon_sim::settings::{GameControlAction, GameSettings, game_binding_label};
use std::rc::Rc;
use web_sys::Document;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsContext {
    Title,
    Gameplay,
    DarkCloud,
}

impl SettingsContext {
    fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Gameplay => "gameplay",
            Self::DarkCloud => "dark-cloud",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Root,
    Cloud,
    Controls,
    Performance,
    MobileUi,
    SaveTransfer,
}

impl Page {
    fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Cloud => "cloud",
            Self::Controls => "controls",
            Self::Performance => "performance",
            Self::MobileUi => "mobile-ui",
            Self::SaveTransfer => "save-transfer",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Root => "GAME SETTINGS",
            Self::Cloud => "DARK CLOUD SETTINGS",
            Self::Controls => "CUSTOMIZE KEYBOARD",
            Self::Performance => "TWEAK PERFORMANCE",
            Self::MobileUi => "MOBILE UI EDITOR",
            Self::SaveTransfer => "SAVE TRANSFER",
        }
    }
}

#[derive(Clone, Copy)]
enum NumericSetting {
    Sound,
    Music,
    Fov,
    UiScale,
    LightQuality,
}

impl NumericSetting {
    fn definition(self) -> (&'static str, &'static str, u16, u16) {
        match self {
            Self::Sound => ("sound", "SOUND VOL:", 0, 100),
            Self::Music => ("music", "MUSIC VOL:", 0, 100),
            Self::Fov => ("fov", "CAMERA FOV", 75, 125),
            Self::UiScale => ("ui-scale", "UI SCALE", 75, 150),
            Self::LightQuality => ("light-quality", "LIGHT QUALITY", 24, 100),
        }
    }

    fn value(self, settings: &GameSettings) -> u16 {
        match self {
            Self::Sound => settings.sound_volume_percent,
            Self::Music => settings.music_volume_percent,
            Self::Fov => settings.camera_fov_percent,
            Self::UiScale => settings.ui_scale_percent,
            Self::LightQuality => settings.light_quality_percent,
        }
    }

    fn set(self, settings: &mut GameSettings, value: u16) {
        match self {
            Self::Sound => settings.sound_volume_percent = value,
            Self::Music => settings.music_volume_percent = value,
            Self::Fov => settings.camera_fov_percent = value,
            Self::UiScale => settings.ui_scale_percent = value,
            Self::LightQuality => settings.light_quality_percent = value,
        }
    }
}

#[derive(Clone, Copy)]
enum ToggleSetting {
    Online,
    Activity,
    GlobalChat,
    SharedHub,
    SubmitRuns,
    Cheats,
    Lighting,
    Shadows,
    MultipleShadows,
    AimAtMouse,
    ReducedFlashes,
    CameraShake,
    ReverseHotbar,
    EnhancedEffects,
}

impl ToggleSetting {
    fn definition(self) -> (&'static str, &'static str, bool) {
        match self {
            Self::Online => ("online", "ENABLE ONLINE FEATURES", false),
            Self::Activity => ("activity", "ENABLE ACTIVITY MESSAGES", true),
            Self::GlobalChat => ("global-chat", "ENABLE GLOBAL CHAT", true),
            Self::SharedHub => ("shared-hub", "ENABLE SHARED HUB", true),
            Self::SubmitRuns => ("submit-runs", "SUBMIT RUNS TO SERVER", true),
            Self::Cheats => ("cheats", "ENABLE CHEATS", false),
            Self::Lighting => ("lighting", "COMPLEX LIGHTING", false),
            Self::Shadows => ("shadows", "COMPLEX SHADOWS", false),
            Self::MultipleShadows => ("multiple-shadows", "MULTIPLE SHADOWS", false),
            Self::AimAtMouse => ("aim-at-mouse", "CAST SECONDARY SPELLS AT MOUSE", false),
            Self::ReducedFlashes => ("reduced-flashes", "REDUCED SCREEN FLASHES", false),
            Self::CameraShake => ("zoom-effects", "CAMERA SHAKE", false),
            Self::ReverseHotbar => ("reverse-hotbar", "REVERSE HOTBAR CYCLING", false),
            Self::EnhancedEffects => ("enhanced-effects", "ENHANCED EFFECTS", false),
        }
    }

    fn stored(self, settings: &GameSettings) -> bool {
        match self {
            Self::Online => settings.enable_online_features,
            Self::Activity => settings.enable_activity_messages,
            Self::GlobalChat => settings.enable_global_chat,
            Self::SharedHub => settings.enable_shared_hub,
            Self::SubmitRuns => settings.submit_runs_to_server,
            Self::Cheats => settings.enable_cheats,
            Self::Lighting => settings.complex_lighting,
            Self::Shadows => settings.complex_shadows,
            Self::MultipleShadows => settings.multiple_shadows,
            Self::AimAtMouse => settings.cast_secondaries_at_mouse,
            Self::ReducedFlashes => settings.reduced_screen_flashes,
            Self::CameraShake => settings.zoom_effects,
            Self::ReverseHotbar => settings.reverse_hotbar_cycle,
            Self::EnhancedEffects => settings.enhanced_effects,
        }
    }

    fn disabled(self, settings: &GameSettings) -> bool {
        match self {
            Self::Activity => !settings.enable_online_features || !settings.enable_global_chat,
            Self::GlobalChat | Self::SharedHub | Self::SubmitRuns => {
                !settings.enable_online_features
            }
            _ => false,
        }
    }

    fn toggle(self, settings: &mut GameSettings) {
        let value = !self.stored(settings);
        match self {
            Self::Online => settings.enable_online_features = value,
            Self::Activity => settings.enable_activity_messages = value,
            Self::GlobalChat => settings.enable_global_chat = value,
            Self::SharedHub => settings.enable_shared_hub = value,
            Self::SubmitRuns => settings.submit_runs_to_server = value,
            Self::Cheats => settings.enable_cheats = value,
            Self::Lighting => settings.complex_lighting = value,
            Self::Shadows => settings.complex_shadows = value,
            Self::MultipleShadows => settings.multiple_shadows = value,
            Self::AimAtMouse => settings.cast_secondaries_at_mouse = value,
            Self::ReducedFlashes => settings.reduced_screen_flashes = value,
            Self::CameraShake => settings.zoom_effects = value,
            Self::ReverseHotbar => settings.reverse_hotbar_cycle = value,
            Self::EnhancedEffects => settings.enhanced_effects = value,
        }
    }
}

pub(super) struct SettingsView {
    page: Page,
    context: SettingsContext,
    account_username: Option<String>,
    enhanced_effects_authority: Option<EnhancedEffectsAuthority>,
    pending_binding: Option<GameControlAction>,
    settings: GameSettings,
    fullscreen: Option<SettingsFullscreen>,
    ui_atlas: StockAtlas,
    control_panel: StockAtlas,
    document: Document,
    mobile_art: Rc<MobileEditorArt>,
    mobile_state: MobileUiLayoutState,
    mobile_preview: Option<MobileEditorPlayerPreview>,
    mobile_cooldowns: [NativeSkillQuickbarCooldownPresentation; 83],
    mobile_editor: Option<MobileUiEditor>,
    mobile_fullscreen: bool,
    mobile_save: Option<Option<MobileUiLayout>>,
    mobile_save_in_flight: bool,
    mobile_sharing: Option<mobile_sharing::SettingsLayoutPublication>,
    save_can_export: bool,
    save_transfer_available: bool,
    save_transfer: Option<transfer::SaveTransferView>,
    save_request_sequence: Rc<std::cell::Cell<u64>>,
    viewport: Vec2,
    changed: bool,
    scroll_reset: bool,
}
impl SettingsView {
    pub async fn new(document: &Document) -> Result<Self, String> {
        Ok(Self {
            page: Page::Root,
            context: SettingsContext::Title,
            account_username: None,
            enhanced_effects_authority: None,
            pending_binding: None,
            settings: GameSettings::default(),
            fullscreen: SettingsFullscreen::new(document)?,
            ui_atlas: StockAtlas::load("UI").await?,
            control_panel: StockAtlas::load("ControlPanel").await?,
            document: document.clone(),
            mobile_art: Rc::new(MobileEditorArt::load().await?),
            mobile_state: MobileUiLayoutState::default(),
            mobile_preview: None,
            mobile_cooldowns: [NativeSkillQuickbarCooldownPresentation::default(); 83],
            mobile_editor: None,
            mobile_fullscreen: false,
            mobile_save: None,
            mobile_save_in_flight: false,
            mobile_sharing: None,
            save_can_export: false,
            save_transfer_available: false,
            save_transfer: None,
            save_request_sequence: Rc::new(std::cell::Cell::new(0)),
            viewport: Vec2::new(1600.0, 900.0),
            changed: false,
            scroll_reset: true,
        })
    }
    pub fn resize(&mut self, width: i32, height: i32) {
        self.viewport = Vec2::new(width as f32, height as f32);
    }
    pub fn open(&mut self, context: SettingsContext) -> Result<(), String> {
        self.context = context;
        self.page = Page::Root;
        self.build_page()
    }
    pub fn set_account_username(&mut self, username: Option<&str>) -> Result<(), String> {
        self.account_username = username.map(str::to_owned);
        if let Some(sharing) = &mut self.mobile_sharing {
            sharing.set_admission(username.is_some(), self.mobile_state.customized)?;
        }
        Ok(())
    }
    pub fn set_enhanced_effects_authority(
        &mut self,
        authority: Option<EnhancedEffectsAuthority>,
    ) -> Result<(), String> {
        self.enhanced_effects_authority = authority;
        Ok(())
    }
    pub fn requested_enhanced_effects(&self) -> Option<bool> {
        match self.enhanced_effects_authority {
            Some(a) if !a.can_change => None,
            Some(a) => Some(!a.enabled),
            None => Some(!self.settings.enhanced_effects),
        }
    }
    pub fn close(&mut self) -> Result<(), String> {
        self.clear_mobile_editor()?;
        self.mobile_sharing = None;
        self.save_transfer = None;
        self.pending_binding = None;
        Ok(())
    }
    pub fn update(&mut self) -> Result<(), String> {
        self.update_mobile_editor()?;
        self.update_save_transfer()?;
        if let Some(s) = &mut self.mobile_sharing {
            s.update()?;
        }
        Ok(())
    }
    pub fn set(&mut self, settings: &GameSettings) -> Result<(), String> {
        self.settings = settings.clone();
        self.pending_binding = None;
        Ok(())
    }
    pub fn read(&mut self) -> Result<GameSettings, String> {
        Ok(self.settings.clone())
    }
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }
    pub fn activate(&mut self, action: &str) -> Result<bool, String> {
        let page = match action {
            "settings-page:cloud" => Some(Page::Cloud),
            "settings-page:controls" => Some(Page::Controls),
            "settings-page:performance" => Some(Page::Performance),
            "settings-page:mobile-ui" => Some(Page::MobileUi),
            "settings-page:save-transfer"
                if self.context == SettingsContext::Title && self.save_transfer_available =>
            {
                Some(Page::SaveTransfer)
            }
            _ => None,
        };
        if let Some(page) = page {
            self.page = page;
            self.build_page()?;
            return Ok(true);
        }
        if let Some(id) = action.strip_prefix("bind:") {
            self.begin_binding(id)?;
            return Ok(true);
        }
        if let Some(id) = action.strip_prefix("settings-toggle:") {
            let setting = ToggleSetting::ALL.into_iter().find(|s| s.definition().0 == id);
            if let Some(setting) = setting {
                if !setting.disabled(&self.settings) {
                    setting.toggle(&mut self.settings);
                    self.changed = true;
                }
                return Ok(true);
            }
        }
        if action == "settings-fullscreen" {
            if let Some(fullscreen) = &mut self.fullscreen {
                fullscreen.activate()?;
            }
            return Ok(true);
        }
        if let Some(view) = &mut self.mobile_sharing
            && view.activate(action)?
        {
            return Ok(true);
        }
        if let Some(view) = &mut self.save_transfer
            && view.activate(action)?
        {
            return Ok(true);
        }
        Ok(false)
    }
    pub fn back(&mut self) -> Result<bool, String> {
        if self.pending_binding.take().is_some() {
            return Ok(true);
        }
        if self.page == Page::MobileUi {
            self.request_mobile_save()?;
            return Ok(true);
        }
        if self.page != Page::Root {
            self.page = Page::Root;
            self.build_page()?;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn begin_binding(&mut self, id: &str) -> Result<(), String> {
        if let Some(action) = GameControlAction::ALL.into_iter().find(|a| binding_id(*a) == id) {
            self.pending_binding =
                if self.pending_binding == Some(action) { None } else { Some(action) };
        }
        Ok(())
    }
    pub fn capture_binding(&mut self, code: &str) -> Result<bool, String> {
        let Some(action) = self.pending_binding.take() else { return Ok(false) };
        let changed = self.settings.controls.rebind(action, code);
        self.changed |= changed;
        Ok(changed)
    }
    pub fn awaiting_binding(&self) -> bool {
        self.pending_binding.is_some()
    }
    pub fn awaiting_mouse_binding(&self) -> bool {
        matches!(self.pending_binding, Some(GameControlAction::Belt(_)))
    }
    fn build_page(&mut self) -> Result<(), String> {
        self.clear_mobile_editor()?;
        self.mobile_sharing = None;
        self.save_transfer = None;
        self.pending_binding = None;
        self.scroll_reset = true;
        match self.page {
            Page::MobileUi => self.build_mobile_editor()?,
            Page::Cloud => {
                self.mobile_sharing = Some(mobile_sharing::SettingsLayoutPublication::new(
                    self.account_username.is_some(),
                    self.mobile_state.customized,
                ))
            }
            Page::SaveTransfer => {
                self.save_transfer = Some(transfer::SaveTransferView::new(
                    &self.document,
                    self.save_can_export,
                    self.save_request_sequence.clone(),
                )?)
            }
            _ => {}
        }
        Ok(())
    }
    pub fn default_focus(&self) -> Option<String> {
        match self.page {
            Page::Root => Some("settings-range:sound".to_owned()),
            Page::Cloud => Some("settings-toggle:online".to_owned()),
            Page::Controls => Some("settings-toggle:reverse-hotbar".to_owned()),
            Page::Performance => Some("settings-toggle:lighting".to_owned()),
            Page::SaveTransfer => Some("save-import".to_owned()),
            Page::MobileUi => None,
        }
    }
    pub(super) fn mobile_editor_active(&self) -> bool {
        self.page == Page::MobileUi && self.mobile_editor.is_some()
    }
    pub fn navigation_scope(&self) -> String {
        format!("settings-{}-{}", self.context.as_str(), self.page.as_str())
    }
    pub fn paint(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        view: Rect,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        widgets::scrollbar_style(ui, 0x9f8547, 0x17130e, false);
        if self.page == Page::MobileUi && self.mobile_fullscreen {
            if let Some(editor) = &mut self.mobile_editor {
                editor.paint_in(frame, view, ui)?;
                if editor.take_save_request() {
                    actions.push("settings-back".to_owned());
                }
            }
            return Ok(());
        }
        ui.painter().rect_filled(view, 0.0, egui::Color32::from_black_alpha(209));
        let pad = (view.width().min(view.height()) * 0.03).clamp(8.0, 24.0);
        let panel = Rect::from_center_size(
            view.center(),
            Vec2::new(
                600.0_f32.min(view.width() - pad * 2.0),
                700.0_f32.min(view.height() - pad * 2.0),
            ),
        );
        paint_panel_background(frame, ui, panel)?;
        frame.record_rect("settings-panel", self.page.title(), "dialog", panel, ui.is_enabled());
        let compact = view.width() < 700.0 || view.height() < 540.0;
        let band = if compact { 54.0 } else { 70.0 };
        let header = Rect::from_min_size(panel.min, Vec2::new(panel.width(), band));
        widgets::gradient(ui, header, widgets::color(0x65532b), widgets::color(0x3c321d));
        widgets::native(
            frame,
            ui,
            self.page.title(),
            NativeUiFontName::ControlPanel,
            header.center() + Vec2::new(0.0, 10.0),
            2.0,
            0xd9bd72,
            NativeUiTextAlign::Center,
        );
        let content = Rect::from_min_max(
            panel.min + Vec2::new(if compact { 14.0 } else { 20.0 }, band + 12.0),
            panel.max - Vec2::new(if compact { 14.0 } else { 20.0 }, band),
        );
        frame.record_rect(
            "settings-content",
            "Settings preferences",
            "region",
            content,
            ui.is_enabled(),
        );
        if self.page == Page::MobileUi {
            if let Some(editor) = &mut self.mobile_editor {
                editor.paint_in(frame, content, ui)?;
                if editor.take_save_request() {
                    actions.push("settings-back".to_owned());
                }
            }
        } else {
            let mut child =
                ui.new_child(egui::UiBuilder::new().id_salt("settings-content").max_rect(content));
            child.set_clip_rect(content);
            child.spacing_mut().item_spacing = Vec2::ZERO;
            let mut scroll = egui::ScrollArea::vertical()
                .id_salt(self.navigation_scope())
                .auto_shrink([false, false]);
            if self.scroll_reset {
                scroll = scroll.vertical_scroll_offset(0.0);
                self.scroll_reset = false;
            }
            let mut result = Ok(());
            scroll.show(&mut child, |ui| {
                result = self.paint_page(frame, ui, content.width(), actions);
            });
            std::mem::replace(&mut result, Ok(()))?;
        }
        let footer = Rect::from_center_size(
            Pos2::new(panel.center().x, panel.bottom() - band / 2.0 - 2.0),
            Vec2::new(300.0_f32.min(panel.width() - 40.0), 41.0),
        );
        let label = match self.page {
            Page::Root => "DONE",
            Page::MobileUi => "SAVE",
            _ => "BACK",
        };
        if widgets::stone_button(
            frame,
            ui,
            &self.ui_atlas,
            "settings-back",
            label,
            footer,
            !self.mobile_save_in_flight,
        )?
        .clicked()
            && !self.mobile_save_in_flight
        {
            actions.push("settings-back".to_owned());
        }
        ui.painter().rect_stroke(
            panel,
            0.0,
            egui::Stroke::new(3.0, widgets::color(0x9f8547)),
            egui::StrokeKind::Inside,
        );
        for (x, y, flip_x, flip_y) in [
            (panel.left() - 13.0, panel.top() - 13.0, false, false),
            (panel.right() - 67.0, panel.top() - 13.0, true, false),
            (panel.left() - 13.0, panel.bottom() - 70.0, false, true),
            (panel.right() - 67.0, panel.bottom() - 70.0, true, true),
        ] {
            let record = self.ui_atlas.record(17)?;
            let size = Vec2::new(record.width as f32, record.height as f32)
                * if compact { 0.72 } else { 1.0 };
            let dest = Rect::from_min_size(egui::pos2(x, y), size);
            let atlas_size = frame.image_size(self.ui_atlas.path()).ok_or("UI atlas not ready")?;
            let uv = Rect::from_min_max(
                egui::pos2(record.x as f32 / atlas_size.x, record.y as f32 / atlas_size.y),
                egui::pos2(
                    ((record.x + record.width) as f32) / atlas_size.x,
                    ((record.y + record.height) as f32) / atlas_size.y,
                ),
            );
            frame.image_quad(
                ui.painter(),
                self.ui_atlas.path(),
                [
                    if flip_x {
                        if flip_y { dest.right_bottom() } else { dest.right_top() }
                    } else if flip_y {
                        dest.left_bottom()
                    } else {
                        dest.left_top()
                    },
                    if flip_x {
                        if flip_y { dest.left_bottom() } else { dest.left_top() }
                    } else if flip_y {
                        dest.right_bottom()
                    } else {
                        dest.right_top()
                    },
                    if flip_x {
                        if flip_y { dest.left_top() } else { dest.left_bottom() }
                    } else if flip_y {
                        dest.right_top()
                    } else {
                        dest.right_bottom()
                    },
                    if flip_x {
                        if flip_y { dest.right_top() } else { dest.right_bottom() }
                    } else if flip_y {
                        dest.left_top()
                    } else {
                        dest.left_bottom()
                    },
                ],
                uv,
                Color32::WHITE,
            );
        }
        if !compact {
            self.ui_atlas.paint(
                frame,
                ui.painter(),
                18,
                panel.min + Vec2::new(-101.0, 216.0),
                1.0,
                Color32::WHITE,
            )?;
            self.ui_atlas.paint_centered(
                frame,
                ui.painter(),
                18,
                egui::pos2(panel.right() + 67.5, panel.top() + 347.0),
                1.0,
                0.0,
                Color32::WHITE,
            )?;
        }
        Ok(())
    }
    fn paint_page(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        width: f32,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        match self.page {
            Page::Root => {
                group_heading(frame, ui, "SOUND AND MUSIC", width);
                self.range(frame, ui, NumericSetting::Sound, width)?;
                self.range(frame, ui, NumericSetting::Music, width)?;
                group(frame, ui, "VIDEO SETTINGS", width);
                if let Some(fullscreen) = &mut self.fullscreen {
                    fullscreen.paint(frame, ui, &self.control_panel, width, actions)?;
                }
                self.range(frame, ui, NumericSetting::Fov, width)?;
                self.range(frame, ui, NumericSetting::UiScale, width)?;
                for (heading, label, id) in [
                    ("DARK CLOUD SETTINGS", "ONLINE AND ACCOUNT", "settings-page:cloud"),
                    ("CONTROLS", "CUSTOMIZE MOBILE UI", "settings-page:mobile-ui"),
                    ("", "CUSTOMIZE KEYBOARD", "settings-page:controls"),
                    ("PERFORMANCE", "TWEAK GAME", "settings-page:performance"),
                ] {
                    if !heading.is_empty() {
                        group(frame, ui, heading, width);
                    }
                    self.action_row(frame, ui, label, id, width, true, actions)?;
                }
            }
            Page::Cloud => {
                group_heading(frame, ui, "DARK ACCOUNT", width);
                let text = format!(
                    "ACCOUNT: {}",
                    self.account_username.as_deref().unwrap_or("GUEST").to_uppercase()
                );
                self.static_row(frame, ui, &text, width)?;
                group(frame, ui, "ONLINE FEATURES", width);
                for setting in [
                    ToggleSetting::Online,
                    ToggleSetting::Activity,
                    ToggleSetting::GlobalChat,
                    ToggleSetting::SharedHub,
                    ToggleSetting::SubmitRuns,
                ] {
                    self.toggle(frame, ui, setting, width, actions)?;
                }
                group(frame, ui, "MOBILE INTERFACE", width);
                if let Some(mut sharing) = self.mobile_sharing.take() {
                    sharing.paint(self, frame, ui, width, actions)?;
                    self.mobile_sharing = Some(sharing);
                }
                if self.context == SettingsContext::Title && self.save_transfer_available {
                    group(frame, ui, "SAVE TRANSFER", width);
                    self.action_row(
                        frame,
                        ui,
                        "STOCK / BROWSER SAVE",
                        "settings-page:save-transfer",
                        width,
                        true,
                        actions,
                    )?;
                }
                group(frame, ui, "DEVELOPER", width);
                self.toggle(frame, ui, ToggleSetting::Cheats, width, actions)?;
                if self.settings.enable_cheats {
                    flow(
                        ui,
                        &format!(
                            "Debug menu: {}
DevTools: solomonDark.lua.help()",
                            game_binding_label(&self.settings.controls.open_cheats)
                        ),
                        width,
                        12.0,
                        widgets::color(0xb3a477),
                    );
                }
                if self.context != SettingsContext::Title {
                    flow(
                        ui,
                        "Display size follows the browser. Resolution is available automatically.",
                        width,
                        12.0,
                        widgets::color(0xb3a477),
                    );
                }
            }
            Page::Controls => {
                group_heading(frame, ui, "HOTBAR CYCLING", width);
                self.toggle(frame, ui, ToggleSetting::ReverseHotbar, width, actions)?;
                for (title, actions_group) in
                    [("WIZARD CONTROLS", 0), ("STATS AND STUFF", 1), ("BELT HOTKEYS", 2)]
                {
                    group(frame, ui, title, width);
                    for action in GameControlAction::ALL {
                        let group = match action {
                            GameControlAction::MoveUp
                            | GameControlAction::MoveDown
                            | GameControlAction::MoveLeft
                            | GameControlAction::MoveRight => 0,
                            GameControlAction::Belt(_) | GameControlAction::CycleHotbar => 2,
                            _ => 1,
                        };
                        if group == actions_group {
                            self.binding(frame, ui, action, width, actions)?;
                        }
                    }
                }
            }
            Page::Performance => {
                group_heading(frame, ui, "LIGHTING", width);
                for setting in [
                    ToggleSetting::Lighting,
                    ToggleSetting::Shadows,
                    ToggleSetting::MultipleShadows,
                ] {
                    self.toggle(frame, ui, setting, width, actions)?;
                }
                self.range(frame, ui, NumericSetting::LightQuality, width)?;
                group(frame, ui, "PLAY STYLE", width);
                self.toggle(frame, ui, ToggleSetting::AimAtMouse, width, actions)?;
                group(frame, ui, "SPECIAL EFFECTS", width);
                for setting in [
                    ToggleSetting::ReducedFlashes,
                    ToggleSetting::CameraShake,
                    ToggleSetting::EnhancedEffects,
                ] {
                    self.toggle(frame, ui, setting, width, actions)?;
                }
                if let Some(authority) = self.enhanced_effects_authority {
                    let mut text = match authority.scope {
                        EnhancedEffectsScope::NextPartyRun => {
                            "Applies to this party’s next game. The shared College is unchanged."
                                .to_owned()
                        }
                        EnhancedEffectsScope::Game => {
                            "Shared game setting. Existing effects keep their native birth state."
                                .to_owned()
                        }
                    };
                    if !authority.can_change {
                        text.push_str(" Only the game owner can change it.");
                    }
                    flow(ui, &text, width, 12.0, widgets::color(0xb3a477));
                }
            }
            Page::SaveTransfer => {
                if let Some(transfer) = self.save_transfer.take() {
                    transfer.paint(self, frame, ui, width, actions)?;
                    self.save_transfer = Some(transfer);
                }
            }
            Page::MobileUi => {}
        }
        Ok(())
    }
    fn row(&self, ui: &mut Ui, width: f32) -> Rect {
        ui.allocate_space(Vec2::new(width, 44.0)).1
    }
    fn static_row(
        &self,
        frame: &UiFrame<'_>,
        ui: &mut Ui,
        label: &str,
        width: f32,
    ) -> Result<(), String> {
        let rect = self.row(ui, width);
        self.control_panel.paint_raw(frame, ui.painter(), 3, rect, Color32::WHITE)?;
        widgets::native(
            frame,
            ui,
            label,
            NativeUiFontName::ControlPanel,
            rect.min + Vec2::new(10.0, 26.0),
            1.0,
            0xdbc174,
            NativeUiTextAlign::Left,
        );
        Ok(())
    }
    // This native row binds an action and enabled policy to authored text and geometry.
    #[allow(clippy::too_many_arguments)]
    fn action_row(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        label: &str,
        id: &str,
        width: f32,
        enabled: bool,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let rect = self.row(ui, width);
        let response = widgets::interact_unstyled(frame, ui, id, label, "button", rect, enabled);
        let highlight = enabled && (response.hovered() || frame.focus_visible(&response));
        let saturation = frame.with_saturation(if enabled { 1.0 } else { 0.35 });
        let paint = saturation.with_brightness(if !enabled {
            0.62
        } else if highlight {
            1.16
        } else {
            1.0
        });
        self.control_panel.paint_raw(&paint, ui.painter(), 3, rect, Color32::WHITE)?;
        widgets::native(
            &paint,
            ui,
            label,
            NativeUiFontName::ControlPanel,
            rect.min + Vec2::new(10.0, 26.0),
            1.0,
            0xdbc174,
            NativeUiTextAlign::Left,
        );
        self.control_panel.paint_centered(
            &paint,
            ui.painter(),
            0,
            egui::pos2(rect.right() - 20.0, rect.center().y),
            1.0,
            0.0,
            Color32::WHITE,
        )?;
        if highlight {
            row_outline(&paint, ui, rect);
        }
        if response.clicked() && enabled {
            actions.push(id.to_owned());
        }
        Ok(())
    }
    fn toggle(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        setting: ToggleSetting,
        width: f32,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let (id, label, nested) = setting.definition();
        let rect = self.row(ui, width);
        let (disabled, checked) = if matches!(setting, ToggleSetting::EnhancedEffects) {
            self.enhanced_effects_authority
                .map_or((false, self.settings.enhanced_effects), |a| (!a.can_change, a.enabled))
        } else {
            let disabled = setting.disabled(&self.settings);
            (disabled, !disabled && setting.stored(&self.settings))
        };
        let id = format!("settings-toggle:{id}");
        let response =
            widgets::interact_unstyled(frame, ui, &id, label, "checkbox", rect, !disabled);
        frame.record_value(&id, if checked { "true" } else { "false" }, Some(checked));
        let highlight = !disabled && (response.hovered() || frame.focus_visible(&response));
        let saturation = frame.with_saturation(if disabled { 0.35 } else { 1.0 });
        let paint = saturation.with_brightness(if disabled {
            0.62
        } else if highlight {
            1.16
        } else {
            1.0
        });
        self.control_panel.paint_raw(&paint, ui.painter(), 3, rect, Color32::WHITE)?;
        widgets::native(
            &paint,
            ui,
            label,
            NativeUiFontName::ControlPanel,
            rect.min + Vec2::new(if nested { 30.0 } else { 10.0 }, 26.0),
            1.0,
            0xdbc174,
            NativeUiTextAlign::Left,
        );
        toggle_art(
            &paint,
            ui,
            &self.control_panel,
            rect,
            checked,
            widgets::alpha(0xffffff, if disabled { 0.58 } else { 1.0 }),
        )?;
        if highlight {
            row_outline(&paint, ui, rect);
        }
        if response.clicked() && !disabled {
            actions.push(id);
        }
        Ok(())
    }
    fn range(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        setting: NumericSetting,
        width: f32,
    ) -> Result<(), String> {
        let (id, label, min, max) = setting.definition();
        let rect = self.row(ui, width);
        let track = Rect::from_min_max(
            rect.min + Vec2::new(if width < 500.0 { 144.0 } else { 156.0 }, 7.5),
            rect.max - Vec2::new(68.0, 7.5),
        );
        let id = format!("settings-range:{id}");
        let response = ui.interact(track, egui::Id::unique(&id), egui::Sense::click_and_drag());
        widgets::native_control_focus(&response);
        frame.record(&id, label, "slider", &response);
        let paint = frame.with_brightness(if response.has_focus() { 1.16 } else { 1.0 });
        self.control_panel.paint_raw(&paint, ui.painter(), 3, rect, Color32::WHITE)?;
        widgets::native(
            &paint,
            ui,
            label,
            NativeUiFontName::ControlPanel,
            rect.min + Vec2::new(10.0, 26.0),
            1.0,
            0xdbc174,
            NativeUiTextAlign::Left,
        );
        self.control_panel.paint_strip(&paint, ui.painter(), 4, track, Color32::WHITE)?;
        let mut value = setting.value(&self.settings);
        let before = value;
        if (response.dragged() || response.clicked())
            && let Some(pos) = response.interact_pointer_pos()
        {
            let fraction =
                ((pos.x - track.left() - 32.0) / (track.width() - 64.0).max(1.0)).clamp(0.0, 1.0);
            value = (f32::from(min) + fraction * f32::from(max - min)).round() as u16;
        }
        if response.has_focus() {
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter { horizontal_arrows: true, ..Default::default() },
                )
            });
            for (key, delta) in [
                (egui::Key::ArrowLeft, -1_i32),
                (egui::Key::ArrowRight, 1),
                (egui::Key::PageDown, -10),
                (egui::Key::PageUp, 10),
            ] {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                    value = (i32::from(value) + delta).clamp(i32::from(min), i32::from(max)) as u16;
                }
            }
        }
        if response.has_focus() {
            for (key, next) in [(egui::Key::Home, min), (egui::Key::End, max)] {
                if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                    value = next;
                }
            }
            for (key, code, delta) in
                [(egui::Key::ArrowUp, "ArrowUp", 1_i32), (egui::Key::ArrowDown, "ArrowDown", -1)]
            {
                if frame.key_codes().iter().any(|event| event.pressed && event.code == code)
                    && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key))
                {
                    value = (i32::from(value) + delta).clamp(i32::from(min), i32::from(max)) as u16;
                    ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
                }
            }
        }
        if response.has_focus() {
            row_outline(&paint, ui, rect);
        }
        if value != before {
            setting.set(&mut self.settings, value);
            self.changed = true;
        }
        frame.record_value(&id, &value.to_string(), None);
        let fraction = f32::from(value - min) / f32::from(max - min);
        let center = egui::pos2(
            track.left() + 32.0 + fraction * (track.width() - 64.0).max(0.0),
            track.center().y,
        );
        self.control_panel.paint_centered(
            &paint,
            ui.painter(),
            18,
            center,
            1.0,
            0.0,
            Color32::WHITE,
        )?;
        widgets::native(
            &paint,
            ui,
            &format!("{value}%"),
            NativeUiFontName::ControlPanel,
            egui::pos2(rect.right() - 10.0, rect.top() + 26.0),
            1.0,
            0xf0d996,
            NativeUiTextAlign::Right,
        );
        Ok(())
    }
    fn binding(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        action: GameControlAction,
        width: f32,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let rect = self.row(ui, width);
        let label = binding_label(action);
        let id = format!("bind:{}", binding_id(action));
        let response = widgets::interact_unstyled(frame, ui, &id, &label, "button", rect, true);
        let highlight = response.hovered() || frame.focus_visible(&response);
        let paint = frame.with_brightness(if highlight { 1.16 } else { 1.0 });
        self.control_panel.paint_raw(&paint, ui.painter(), 3, rect, Color32::WHITE)?;
        widgets::native(
            &paint,
            ui,
            &label,
            NativeUiFontName::ControlPanel,
            rect.min + Vec2::new(10.0, 26.0),
            1.0,
            0xdbc174,
            NativeUiTextAlign::Left,
        );
        let plate = Rect::from_min_size(
            egui::pos2(rect.right() - 210.0, rect.center().y - 15.0),
            Vec2::new(200.0, 30.0),
        );
        let listening = self.pending_binding == Some(action);
        let alpha = binding_alpha(ui, response.id, listening);
        self.control_panel.paint_raw(
            &paint,
            ui.painter(),
            5,
            plate,
            widgets::alpha(0xffffff, f64::from(alpha)),
        )?;
        frame.record_value(&id, self.settings.controls.code(action), Some(listening));
        let value = if listening {
            "PRESS A KEY".to_owned()
        } else {
            game_binding_label(self.settings.controls.code(action))
        };
        let position = plate.center() + Vec2::new(0.0, 4.0);
        crate::gpu_ui::native_text(
            &paint,
            ui.painter(),
            &NativeUiTextSpec {
                align: NativeUiTextAlign::Center,
                alpha: f64::from(alpha),
                tint: if listening { 0xf7e1a0 } else { 0xd9bd72 },
                ..NativeUiTextSpec::new(
                    NativeUiFontName::ControlPanel,
                    &value,
                    f64::from(position.x),
                    f64::from(position.y),
                )
            },
        );
        if highlight {
            row_outline(&paint, ui, rect);
        }
        if response.clicked() {
            actions.push(id);
        }
        Ok(())
    }
}
impl ToggleSetting {
    const ALL: [Self; 14] = [
        Self::Online,
        Self::Activity,
        Self::GlobalChat,
        Self::SharedHub,
        Self::SubmitRuns,
        Self::Cheats,
        Self::Lighting,
        Self::Shadows,
        Self::MultipleShadows,
        Self::AimAtMouse,
        Self::ReducedFlashes,
        Self::CameraShake,
        Self::ReverseHotbar,
        Self::EnhancedEffects,
    ];
}
fn paint_panel_background(frame: &UiFrame<'_>, ui: &Ui, panel: Rect) -> Result<(), String> {
    ui.painter().add(
        egui::Shadow { offset: [0, 14], blur: 60, spread: 0, color: Color32::BLACK }
            .as_shape(panel, 0.0),
    );
    let path = "assets/stock/settings-leather.png";
    let size = frame.image_size(path).ok_or("Settings leather is not ready")?;
    StockAtlas::from_image(path, size)?.paint_tiles(
        frame,
        ui.painter(),
        0,
        panel,
        1.0,
        Color32::WHITE,
    )?;
    let painter = ui.painter().with_clip_rect(panel.intersect(ui.clip_rect()));
    let center = panel.center_top();
    let radius = Vec2::new(panel.width() / 2.0, panel.height()).length() * 0.36;
    widgets::radial_gradient(
        ui,
        panel,
        center,
        radius,
        widgets::alpha(0x5f5230, 0.26),
        Color32::TRANSPARENT,
    );
    for outside in [
        Rect::from_min_max(
            panel.min - Vec2::splat(40.0),
            egui::pos2(panel.left(), panel.bottom() + 40.0),
        ),
        Rect::from_min_max(
            egui::pos2(panel.right(), panel.top() - 40.0),
            panel.max + Vec2::splat(40.0),
        ),
        Rect::from_min_max(
            panel.min - Vec2::splat(40.0),
            egui::pos2(panel.right() + 40.0, panel.top()),
        ),
        Rect::from_min_max(
            egui::pos2(panel.left() - 40.0, panel.bottom()),
            panel.max + Vec2::splat(40.0),
        ),
    ] {
        painter.add(
            egui::epaint::RectShape::filled(outside, 0.0, widgets::alpha(0xb9974e, 0.12))
                .with_blur_width(40.0),
        );
    }
    Ok(())
}
fn row_outline(frame: &UiFrame<'_>, ui: &Ui, rect: Rect) {
    ui.painter().rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, frame.filtered_color(widgets::alpha(0xdbc594, 0.8))),
        egui::StrokeKind::Inside,
    );
}

fn binding_alpha(ui: &Ui, id: egui::Id, listening: bool) -> f32 {
    let animation = id.with("listening-opacity");
    if !listening {
        ui.data_mut(|data| data.remove::<f64>(animation));
        return 1.0;
    }
    let now = ui.input(|input| input.time);
    let started = ui.data_mut(|data| *data.get_temp_mut_or_insert_with(animation, || now));
    // Source's .8s steps(2,end) cycle has 0/50/100% keyframes at 1/.45/1.
    let step = ((now - started).rem_euclid(0.8) / 0.2).floor() as usize;
    [1.0, 0.725, 0.45, 0.725][step.min(3)]
}

fn toggle_art(
    frame: &UiFrame<'_>,
    ui: &Ui,
    atlas: &StockAtlas,
    rect: Rect,
    checked: bool,
    tint: Color32,
) -> Result<(), String> {
    let index = if checked { 9 } else { 8 };
    let record = atlas.record(index)?;
    atlas.paint_centered(
        frame,
        ui.painter(),
        index,
        egui::pos2(rect.right() - 10.0 - record.logical_width as f32 / 2.0, rect.center().y),
        1.0,
        0.0,
        tint,
    )
}
fn group(frame: &UiFrame<'_>, ui: &mut Ui, title: &str, width: f32) {
    ui.add_space(12.0);
    group_heading(frame, ui, title, width);
}
fn group_heading(frame: &UiFrame<'_>, ui: &mut Ui, title: &str, width: f32) {
    let (_, r) = ui.allocate_space(Vec2::new(width, 23.0));
    widgets::native(
        frame,
        ui,
        title,
        NativeUiFontName::ControlPanel,
        r.min + Vec2::new(8.0, 14.0),
        1.0,
        0xa99258,
        NativeUiTextAlign::Left,
    );
}
fn flow(ui: &mut Ui, text: &str, width: f32, size: f32, tint: Color32) -> Rect {
    let font = egui::FontId::new(size, egui::FontFamily::Name("Cinzel".into()));
    let galley = ui.painter().layout(text.to_owned(), font, tint, width - 16.0);
    let size = galley.size();
    let (_, r) = ui.allocate_space(Vec2::new(width, size.y + 16.0));
    let origin = r.min + Vec2::splat(8.0);
    ui.painter().galley(origin, galley, tint);
    Rect::from_min_size(origin, size)
}
fn binding_label(action: GameControlAction) -> String {
    match action {
        GameControlAction::MoveUp => "MOVE UP".to_owned(),
        GameControlAction::MoveDown => "MOVE DOWN".to_owned(),
        GameControlAction::MoveLeft => "MOVE LEFT".to_owned(),
        GameControlAction::MoveRight => "MOVE RIGHT".to_owned(),
        GameControlAction::OpenMenu => "OPEN MENU".to_owned(),
        GameControlAction::OpenInventory => "OPEN INVENTORY".to_owned(),
        GameControlAction::OpenSkills => "OPEN SKILLS".to_owned(),
        GameControlAction::OpenChat => "OPEN CHAT".to_owned(),
        GameControlAction::OpenCheats => "OPEN CHEATS".to_owned(),
        GameControlAction::CycleHotbar => "CYCLE HOTBAR".to_owned(),
        GameControlAction::Belt(slot) => format!("BELT SLOT {}", slot + 1),
    }
}
fn binding_id(action: GameControlAction) -> String {
    match action {
        GameControlAction::MoveUp => "move-up".to_owned(),
        GameControlAction::MoveDown => "move-down".to_owned(),
        GameControlAction::MoveLeft => "move-left".to_owned(),
        GameControlAction::MoveRight => "move-right".to_owned(),
        GameControlAction::OpenMenu => "menu".to_owned(),
        GameControlAction::OpenInventory => "inventory".to_owned(),
        GameControlAction::OpenSkills => "skills".to_owned(),
        GameControlAction::OpenChat => "chat".to_owned(),
        GameControlAction::OpenCheats => "cheats".to_owned(),
        GameControlAction::CycleHotbar => "cycle-hotbar".to_owned(),
        GameControlAction::Belt(slot) => format!("belt-{slot}"),
    }
}
