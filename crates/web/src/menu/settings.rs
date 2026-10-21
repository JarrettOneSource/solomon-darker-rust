//! Source `GameSettingsDialog.tsx` pages. Preferences leave through MenuUi;
//! this view retains only page navigation, form values and binding capture.

mod fullscreen;
mod mobile;
mod mobile_sharing;
mod transfer;

use solomon_protocol::hosted::{EnhancedEffectsAuthority, EnhancedEffectsScope};
use std::rc::Rc;

use crate::mobile_editor::{MobileEditorArt, MobileEditorPlayerPreview, MobileUiEditor};
use solomon_sim::hud::quickbar::NativeSkillQuickbarCooldownPresentation;
use solomon_sim::mobile_ui::{MobileUiLayout, MobileUiLayoutState};
use solomon_sim::native_ui_text::NativeUiTextAlign;
use solomon_sim::settings::{GameControlAction, GameSettings, game_binding_label};
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlElement, HtmlInputElement};

use crate::ui::settings::{NativeSettingsArt, settings_text};
use crate::ui::{append, element};
use fullscreen::SettingsFullscreen;

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

struct NumericRow {
    setting: NumericSetting,
    input: HtmlInputElement,
    output: HtmlElement,
    track: HtmlElement,
    track_width: f64,
}

struct ToggleRow {
    setting: ToggleSetting,
    button: HtmlElement,
    art: HtmlElement,
}

struct BindingRow {
    action: GameControlAction,
    label: String,
    button: HtmlElement,
    value: HtmlElement,
}

pub struct SettingsView {
    pub root: HtmlElement,
    panel: HtmlElement,
    title: HtmlElement,
    content: HtmlElement,
    footer: HtmlElement,
    page: Page,
    context: SettingsContext,
    account_username: Option<String>,
    account_label: Option<HtmlElement>,
    console_help: Option<HtmlElement>,
    enhanced_effects_authority: Option<EnhancedEffectsAuthority>,
    enhanced_effects_note: Option<HtmlElement>,
    numeric: Vec<NumericRow>,
    toggles: Vec<ToggleRow>,
    bindings: Vec<BindingRow>,
    pending_binding: Option<GameControlAction>,
    settings: GameSettings,
    fullscreen: Option<SettingsFullscreen>,
    art: Rc<NativeSettingsArt>,
    document: Document,
    mobile_art: Rc<MobileEditorArt>,
    mobile_state: MobileUiLayoutState,
    mobile_preview: Option<MobileEditorPlayerPreview>,
    mobile_cooldowns: [NativeSkillQuickbarCooldownPresentation; 83],
    mobile_editor: Option<MobileUiEditor>,
    mobile_save: Option<Option<MobileUiLayout>>,
    mobile_save_in_flight: bool,
    mobile_sharing: Option<mobile_sharing::SettingsLayoutPublication>,
    save_can_export: bool,
    save_transfer_available: bool,
    save_transfer: Option<transfer::SaveTransferView>,
    save_request_sequence: Rc<std::cell::Cell<u64>>,
}

impl SettingsView {
    pub async fn new(document: &Document) -> Result<Self, String> {
        let art = Rc::new(NativeSettingsArt::load().await?);
        let mobile_art = Rc::new(MobileEditorArt::load().await?);
        let root = element(document, "div", "menu-screen game-settings-backdrop")?;
        root.set_hidden(true);
        root.set_attribute("role", "presentation").map_err(|_| "settings backdrop role")?;
        root.set_attribute("data-native-ui-settings", "").map_err(|_| "settings hook")?;
        let panel = element(document, "section", "game-settings-dialog settings-panel")?;
        panel.set_attribute("role", "dialog").map_err(|_| "settings role")?;
        panel.set_attribute("aria-modal", "true").map_err(|_| "settings modal")?;
        append(&panel, &art.panel(document)?)?;
        let header = element(document, "header", "game-settings-header")?;
        let title = element(document, "h2", "")?;
        append(&header, &title)?;
        append(&panel, &header)?;
        let content = element(document, "div", "game-settings-content")?;
        append(&panel, &content)?;
        let footer = art.stone_button(document, "DONE", "settings-back")?;
        footer.set_class_name("native-stone-button game-settings-close");
        footer.set_attribute("data-game-back", "true").map_err(|_| "settings back hook")?;
        append(&panel, &footer)?;
        append(&root, &panel)?;
        Ok(Self {
            root,
            panel,
            title,
            content,
            footer,
            page: Page::Root,
            context: SettingsContext::Title,
            account_username: None,
            account_label: None,
            console_help: None,
            enhanced_effects_authority: None,
            enhanced_effects_note: None,
            numeric: Vec::new(),
            toggles: Vec::new(),
            bindings: Vec::new(),
            pending_binding: None,
            settings: GameSettings::default(),
            fullscreen: None,
            art,
            document: document.clone(),
            mobile_art,
            mobile_state: MobileUiLayoutState::default(),
            mobile_preview: None,
            mobile_cooldowns: [NativeSkillQuickbarCooldownPresentation::default(); 83],
            mobile_editor: None,
            mobile_save: None,
            mobile_save_in_flight: false,
            mobile_sharing: None,
            save_can_export: false,
            save_transfer_available: false,
            save_transfer: None,
            save_request_sequence: Rc::new(std::cell::Cell::new(0)),
        })
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
        self.paint_account()
    }

    pub fn set_enhanced_effects_authority(
        &mut self,
        authority: Option<EnhancedEffectsAuthority>,
    ) -> Result<(), String> {
        if self.enhanced_effects_authority != authority {
            self.enhanced_effects_authority = authority;
            self.paint_values()?;
        }
        Ok(())
    }

    pub fn requested_enhanced_effects(&self) -> Option<bool> {
        match self.enhanced_effects_authority {
            Some(authority) if !authority.can_change => None,
            Some(authority) => Some(!authority.enabled),
            None => Some(!self.settings.enhanced_effects),
        }
    }

    pub fn close(&mut self) -> Result<(), String> {
        self.clear_mobile_editor()?;
        self.mobile_sharing = None;
        self.save_transfer = None;
        self.pending_binding = None;
        self.fullscreen = None;
        self.paint_bindings()
    }

    /// Hidden pages have zero width. Repaint a repeated track only when its actual
    /// containing width changes, including the first visible frame after opening.
    pub fn update(&mut self) -> Result<(), String> {
        self.update_mobile_editor()?;
        self.update_save_transfer()?;
        if let Some(sharing) = &mut self.mobile_sharing {
            sharing.update()?;
        }
        for row in &mut self.numeric {
            let width = row.track.get_bounding_client_rect().width();
            if width == row.track_width {
                continue;
            }
            row.track_width = width;
            row.track.set_text_content(None);
            append(
                &row.track,
                &self.art.control_panel.strip(
                    &self.document,
                    4,
                    width,
                    "game-settings-native-range-track",
                )?,
            )?;
        }
        Ok(())
    }

    pub fn set(&mut self, settings: &GameSettings) -> Result<(), String> {
        self.settings = settings.clone();
        self.pending_binding = None;
        self.paint_values()
    }

    pub fn read(&mut self) -> Result<GameSettings, String> {
        let previous = self.settings.clone();
        for row in &self.numeric {
            let value = row.input.value().parse().map_err(|_| "invalid settings range value")?;
            row.setting.set(&mut self.settings, value);
        }
        if self.settings != previous {
            self.paint_values()?;
        }
        Ok(self.settings.clone())
    }

    pub fn activate(&mut self, action: &str) -> Result<bool, String> {
        let page = match action {
            "settings-page:cloud" => Some(Page::Cloud),
            "settings-page:controls" => Some(Page::Controls),
            "settings-page:performance" => Some(Page::Performance),
            "settings-page:mobile-ui" => Some(Page::MobileUi),
            "settings-page:save-transfer" if self.context == SettingsContext::Title => {
                Some(Page::SaveTransfer)
            }
            _ => None,
        };
        if let Some(page) = page {
            self.read()?;
            self.page = page;
            self.build_page()?;
            return Ok(true);
        }
        if let Some(id) = action.strip_prefix("bind:") {
            self.begin_binding(id)?;
            return Ok(true);
        }
        if let Some(id) = action.strip_prefix("settings-toggle:") {
            let Some(row) = self.toggles.iter().find(|row| row.setting.definition().0 == id) else {
                return Ok(false);
            };
            if !row.setting.disabled(&self.settings) {
                row.setting.toggle(&mut self.settings);
                self.paint_values()?;
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Footer Back cancels a capture first. Escape is captured as a binding by
    /// MenuUi before reaching this method, as in the source dialog's key handler.
    pub fn back(&mut self) -> Result<bool, String> {
        if self.pending_binding.take().is_some() {
            self.paint_bindings()?;
            return Ok(true);
        }
        if self.page == Page::MobileUi {
            self.request_mobile_save()?;
            return Ok(true);
        }
        if self.page != Page::Root {
            self.read()?;
            self.page = Page::Root;
            self.build_page()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn begin_binding(&mut self, id: &str) -> Result<(), String> {
        let Some(selected) = self.bindings.iter().find(|row| binding_id(row.action) == id) else {
            return Ok(());
        };
        self.pending_binding = if self.pending_binding == Some(selected.action) {
            None
        } else {
            Some(selected.action)
        };
        self.paint_bindings()
    }

    pub fn capture_binding(&mut self, code: &str) -> Result<bool, String> {
        let Some(action) = self.pending_binding.take() else {
            return Ok(false);
        };
        let changed = self.settings.controls.rebind(action, code);
        self.paint_bindings()?;
        Ok(changed)
    }

    pub fn awaiting_binding(&self) -> bool {
        self.pending_binding.is_some()
    }

    pub fn awaiting_mouse_binding(&self) -> bool {
        matches!(self.pending_binding, Some(GameControlAction::Belt(_)))
    }

    fn build_page(&mut self) -> Result<(), String> {
        // Release document fullscreen listeners before discarding the page's nodes.
        self.fullscreen = None;
        self.clear_mobile_editor()?;
        self.save_transfer = None;
        self.mobile_sharing = None;
        self.content
            .class_list()
            .remove_1("native-save-transfer-settings")
            .map_err(|_| "save content owner")?;
        self.content.set_text_content(None);
        self.content.set_scroll_top(0);
        self.numeric.clear();
        self.toggles.clear();
        self.bindings.clear();
        self.account_label = None;
        self.console_help = None;
        self.enhanced_effects_note = None;
        self.pending_binding = None;
        self.panel
            .set_attribute("data-settings-page", self.page.as_str())
            .map_err(|_| "settings page hook")?;
        self.panel
            .set_attribute("data-settings-context", self.context.as_str())
            .map_err(|_| "settings context hook")?;
        self.panel.set_attribute("aria-label", self.page.title()).map_err(|_| "settings title")?;
        self.title.set_text_content(None);
        append(
            &self.title,
            &settings_text(&self.document, self.page.title(), 2.0, 0xd9bd72, None)?,
        )?;
        self.art.set_stone_label(
            &self.document,
            &self.footer,
            match self.page {
                Page::Root => "DONE",
                Page::MobileUi => "SAVE",
                _ => "BACK",
            },
        )?;
        match self.page {
            Page::Root => self.build_root()?,
            Page::Cloud => self.build_cloud()?,
            Page::Controls => self.build_controls()?,
            Page::Performance => self.build_performance()?,
            Page::MobileUi => self.build_mobile_editor()?,
            Page::SaveTransfer => self.build_save_transfer()?,
        }
        self.paint_values()?;
        self.focus_default()
    }

    fn build_root(&mut self) -> Result<(), String> {
        let group = self.group("SOUND AND MUSIC")?;
        self.range(&group, NumericSetting::Sound, true)?;
        self.range(&group, NumericSetting::Music, false)?;
        let group = self.group("VIDEO SETTINGS")?;
        self.fullscreen = SettingsFullscreen::new(&self.document, self.art.clone())?;
        if let Some(fullscreen) = &self.fullscreen {
            append(&group, &fullscreen.root)?;
        }
        self.range(&group, NumericSetting::Fov, false)?;
        self.range(&group, NumericSetting::UiScale, false)?;
        let group = self.group("DARK CLOUD SETTINGS")?;
        self.action(&group, "ONLINE AND ACCOUNT", "settings-page:cloud")?;
        let group = self.group("CONTROLS")?;
        self.action(&group, "CUSTOMIZE MOBILE UI", "settings-page:mobile-ui")?;
        self.action(&group, "CUSTOMIZE KEYBOARD", "settings-page:controls")?;
        let group = self.group("PERFORMANCE")?;
        self.action(&group, "TWEAK GAME", "settings-page:performance")
    }

    fn build_cloud(&mut self) -> Result<(), String> {
        let group = self.group("DARK ACCOUNT")?;
        self.account_label = Some(self.static_row(&group, "", None)?);
        let group = self.group("ONLINE FEATURES")?;
        for setting in [
            ToggleSetting::Online,
            ToggleSetting::Activity,
            ToggleSetting::GlobalChat,
            ToggleSetting::SharedHub,
            ToggleSetting::SubmitRuns,
        ] {
            self.toggle(&group, setting)?;
        }
        let group = self.group("MOBILE INTERFACE")?;
        let sharing = mobile_sharing::SettingsLayoutPublication::new(
            &self.document,
            self.art.clone(),
            self.account_username.is_some(),
            self.mobile_state.customized,
        )?;
        append(&group, &sharing.root)?;
        self.mobile_sharing = Some(sharing);
        self.add_save_transfer_route()?;
        let group = self.group("DEVELOPER")?;
        self.toggle(&group, ToggleSetting::Cheats)?;
        let help = element(&self.document, "p", "game-settings-console-help")?;
        help.set_attribute("role", "status").map_err(|_| "console help role")?;
        append(&group, &help)?;
        self.console_help = Some(help);
        if self.context != SettingsContext::Title {
            let note = element(&self.document, "p", "game-settings-context-note")?;
            note.set_text_content(Some(
                "Display size follows the browser. Resolution is available automatically.",
            ));
            append(&self.content, &note)?;
        }
        Ok(())
    }

    fn build_controls(&mut self) -> Result<(), String> {
        let cycling = self.group("HOTBAR CYCLING")?;
        self.toggle(&cycling, ToggleSetting::ReverseHotbar)?;
        let movement = self.group("WIZARD CONTROLS")?;
        let surfaces = self.group("STATS AND STUFF")?;
        let belt = self.group("BELT HOTKEYS")?;
        for action in GameControlAction::ALL {
            let (group, label) = match action {
                GameControlAction::MoveUp => (&movement, "MOVE UP".to_owned()),
                GameControlAction::MoveDown => (&movement, "MOVE DOWN".to_owned()),
                GameControlAction::MoveLeft => (&movement, "MOVE LEFT".to_owned()),
                GameControlAction::MoveRight => (&movement, "MOVE RIGHT".to_owned()),
                GameControlAction::OpenMenu => (&surfaces, "OPEN MENU".to_owned()),
                GameControlAction::OpenInventory => (&surfaces, "OPEN INVENTORY".to_owned()),
                GameControlAction::OpenSkills => (&surfaces, "OPEN SKILLS".to_owned()),
                GameControlAction::OpenChat => (&surfaces, "OPEN CHAT".to_owned()),
                GameControlAction::OpenCheats => (&surfaces, "OPEN CHEATS".to_owned()),
                GameControlAction::CycleHotbar => (&belt, "CYCLE HOTBAR".to_owned()),
                GameControlAction::Belt(slot) => (&belt, format!("BELT SLOT {}", slot + 1)),
            };
            let button =
                self.button("game-settings-binding", &format!("bind:{}", binding_id(action)))?;
            button
                .set_attribute("data-binding-action", action.as_str())
                .map_err(|_| "binding id")?;
            self.label(&button, &label)?;
            let value = element(&self.document, "strong", "")?;
            append(&button, &value)?;
            append(group, &button)?;
            self.bindings.push(BindingRow { action, label, button, value });
        }
        Ok(())
    }

    fn build_performance(&mut self) -> Result<(), String> {
        let group = self.group("LIGHTING")?;
        self.toggle(&group, ToggleSetting::Lighting)?;
        self.toggle(&group, ToggleSetting::Shadows)?;
        self.toggle(&group, ToggleSetting::MultipleShadows)?;
        self.range(&group, NumericSetting::LightQuality, false)?;
        let group = self.group("PLAY STYLE")?;
        self.toggle(&group, ToggleSetting::AimAtMouse)?;
        let group = self.group("SPECIAL EFFECTS")?;
        self.toggle(&group, ToggleSetting::ReducedFlashes)?;
        self.toggle(&group, ToggleSetting::CameraShake)?;
        self.toggle(&group, ToggleSetting::EnhancedEffects)?;
        let note = element(&self.document, "p", "game-settings-context-note")?;
        note.set_attribute("role", "status").map_err(|_| "effects authority note")?;
        append(&group, &note)?;
        self.enhanced_effects_note = Some(note);
        Ok(())
    }

    fn paint_values(&mut self) -> Result<(), String> {
        self.root
            .set_attribute("data-settings-menu-binding", &self.settings.controls.open_menu)
            .map_err(|_| "settings menu key")?;
        for row in &self.numeric {
            let value = row.setting.value(&self.settings);
            row.input.set_value(&value.to_string());
            row.input
                .set_attribute("aria-valuetext", &format!("{value}%"))
                .map_err(|_| "range spoken value")?;
            row.output.set_text_content(None);
            append(
                &row.output,
                &settings_text(
                    &self.document,
                    &format!("{value}%"),
                    1.0,
                    0xf0d996,
                    Some((NativeUiTextAlign::Right, 48.0)),
                )?,
            )?;
        }
        for row in &self.toggles {
            let (disabled, checked) = if matches!(row.setting, ToggleSetting::EnhancedEffects) {
                self.enhanced_effects_authority
                    .map_or((false, self.settings.enhanced_effects), |authority| {
                        (!authority.can_change, authority.enabled)
                    })
            } else {
                let disabled = row.setting.disabled(&self.settings);
                (disabled, !disabled && row.setting.stored(&self.settings))
            };
            row.button
                .toggle_attribute_with_force("disabled", disabled)
                .map_err(|_| "toggle admission")?;
            row.button
                .set_attribute("aria-pressed", if checked { "true" } else { "false" })
                .map_err(|_| "toggle state")?;
            row.art.set_text_content(None);
            append(&row.art, &self.art.toggle(&self.document, checked)?)?;
        }
        if let Some(note) = &self.enhanced_effects_note {
            note.set_hidden(self.enhanced_effects_authority.is_none());
            note.set_text_content(None);
            if let Some(authority) = self.enhanced_effects_authority {
                let mut copy = match authority.scope {
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
                    copy.push_str(" Only the game owner can change it.");
                }
                append_text(&self.document, note, "span", &copy)?;
            }
        }
        self.paint_bindings()?;
        self.paint_account()?;
        if let Some(help) = &self.console_help {
            help.set_hidden(!self.settings.enable_cheats);
            help.set_text_content(None);
            append_text(&self.document, help, "span", "Debug menu: ")?;
            append_text(
                &self.document,
                help,
                "strong",
                &game_binding_label(&self.settings.controls.open_cheats),
            )?;
            append(help, &element(&self.document, "br", "")?)?;
            append_text(&self.document, help, "span", "DevTools: ")?;
            append_text(&self.document, help, "code", "solomonDark.lua.help()")?;
        }
        Ok(())
    }

    fn paint_bindings(&self) -> Result<(), String> {
        for row in &self.bindings {
            let code = self.settings.controls.code(row.action);
            let listening = self.pending_binding == Some(row.action);
            let label = if listening { "PRESS A KEY".to_owned() } else { game_binding_label(code) };
            row.button.set_attribute("data-binding-code", code).map_err(|_| "binding code")?;
            row.button
                .set_attribute("aria-label", &format!("{}, {label}", row.label))
                .map_err(|_| "binding label")?;
            row.button
                .set_attribute("aria-pressed", if listening { "true" } else { "false" })
                .map_err(|_| "binding listening state")?;
            row.button
                .toggle_attribute_with_force("data-awaiting-binding", listening)
                .map_err(|_| "binding capture state")?;
            row.value.set_text_content(None);
            self.art.binding_plate(&self.document, &row.value)?;
            append(
                &row.value,
                &settings_text(
                    &self.document,
                    &label,
                    1.0,
                    0xd9bd72,
                    Some((NativeUiTextAlign::Center, 200.0)),
                )?,
            )?;
        }
        Ok(())
    }

    fn paint_account(&self) -> Result<(), String> {
        if let Some(label) = &self.account_label {
            let text = format!(
                "ACCOUNT: {}",
                self.account_username.as_deref().unwrap_or("GUEST").to_uppercase()
            );
            label.set_text_content(None);
            label.set_attribute("aria-label", &text).map_err(|_| "settings account label")?;
            append(label, &settings_text(&self.document, &text, 1.0, 0xdbc174, None)?)?;
        }
        Ok(())
    }

    fn group(&self, title: &str) -> Result<HtmlElement, String> {
        let group = element(&self.document, "section", "game-settings-group")?;
        group.set_attribute("aria-label", title).map_err(|_| "settings group label")?;
        let heading = element(&self.document, "h3", "")?;
        heading.set_attribute("aria-label", title).map_err(|_| "settings heading label")?;
        append(&heading, &settings_text(&self.document, title, 1.0, 0xa99258, None)?)?;
        append(&group, &heading)?;
        let content = element(&self.document, "div", "")?;
        append(&group, &content)?;
        append(&self.content, &group)?;
        Ok(content)
    }

    fn label(&self, parent: &HtmlElement, text: &str) -> Result<HtmlElement, String> {
        let label = element(&self.document, "span", "game-settings-native-label")?;
        append(&label, &settings_text(&self.document, text, 1.0, 0xdbc174, None)?)?;
        append(parent, &label)?;
        Ok(label)
    }

    fn button(&self, class: &str, action: &str) -> Result<HtmlElement, String> {
        let button = element(&self.document, "button", class)?;
        button.set_attribute("type", "button").map_err(|_| "settings button type")?;
        button.set_attribute("data-menu-action", action).map_err(|_| "settings action")?;
        self.art.row(&self.document, &button)?;
        Ok(button)
    }

    fn action(&self, parent: &HtmlElement, label: &str, action: &str) -> Result<(), String> {
        let button = self.button("game-settings-action", action)?;
        button.set_attribute("aria-label", label).map_err(|_| "settings action label")?;
        self.label(&button, label)?;
        self.art.arrow(&self.document, &button)?;
        append(parent, &button)
    }

    fn static_row(
        &self,
        parent: &HtmlElement,
        label: &str,
        detail: Option<&str>,
    ) -> Result<HtmlElement, String> {
        let row = element(&self.document, "p", "game-settings-fixed-policy")?;
        self.art.row(&self.document, &row)?;
        let text = self.label(&row, label)?;
        text.set_attribute("aria-label", label).map_err(|_| "settings static label")?;
        if let Some(detail) = detail {
            append_text(&self.document, &row, "small", detail)?;
        }
        append(parent, &row)?;
        Ok(text)
    }

    fn toggle(&mut self, parent: &HtmlElement, setting: ToggleSetting) -> Result<(), String> {
        let (id, label, nested) = setting.definition();
        let button =
            self.button("game-settings-native-toggle-row", &format!("settings-toggle:{id}"))?;
        button.set_attribute("aria-label", label).map_err(|_| "settings toggle label")?;
        button.set_attribute("data-setting", id).map_err(|_| "settings toggle id")?;
        if nested {
            button.set_attribute("data-settings-nested", "true").map_err(|_| "nested toggle")?;
        }
        if matches!(setting, ToggleSetting::Online) {
            button.set_attribute("data-game-default-focus", "true").map_err(|_| "online focus")?;
        }
        self.label(&button, label)?;
        let art = element(&self.document, "span", "")?;
        append(&button, &art)?;
        append(parent, &button)?;
        self.toggles.push(ToggleRow { setting, button, art });
        Ok(())
    }

    fn range(
        &mut self,
        parent: &HtmlElement,
        setting: NumericSetting,
        auto_focus: bool,
    ) -> Result<(), String> {
        let (id, label, minimum, maximum) = setting.definition();
        let row = element(&self.document, "label", "game-settings-range")?;
        self.art.row(&self.document, &row)?;
        self.label(&row, label)?;
        let control = element(&self.document, "div", "game-settings-range-control")?;
        let track = element(&self.document, "span", "game-settings-range-track-host")?;
        append(&control, &track)?;
        let input: HtmlInputElement =
            element(&self.document, "input", "")?.dyn_into().map_err(|_| "settings range input")?;
        input.set_type("range");
        input.set_min(&minimum.to_string());
        input.set_max(&maximum.to_string());
        input.set_step("1");
        input.set_attribute("data-setting", id).map_err(|_| "settings range id")?;
        input.set_attribute("aria-label", label).map_err(|_| "settings range label")?;
        if auto_focus {
            input.set_attribute("data-game-default-focus", "true").map_err(|_| "range focus")?;
        }
        control.append_child(&input).map_err(|_| "range input attachment")?;
        append(&row, &control)?;
        let output = element(&self.document, "output", "")?;
        append(&row, &output)?;
        append(parent, &row)?;
        self.numeric.push(NumericRow { setting, input, output, track, track_width: -1.0 });
        Ok(())
    }

    fn focus_default(&self) -> Result<(), String> {
        if self.root.hidden() {
            return Ok(());
        }
        if self.page == Page::MobileUi {
            return Ok(());
        }
        let selector =
            "[data-game-default-focus='true'], button:not([disabled]), input:not([disabled])";
        if let Some(target) =
            self.content.query_selector(selector).map_err(|_| "settings focus lookup")?
        {
            target
                .dyn_into::<HtmlElement>()
                .map_err(|_| "settings focus target")?
                .focus()
                .map_err(|_| "settings page focus")?;
        }
        Ok(())
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

fn append_text(
    document: &Document,
    parent: &HtmlElement,
    tag: &str,
    text: &str,
) -> Result<(), String> {
    let node = element(document, tag, "")?;
    node.set_text_content(Some(text));
    append(parent, &node)
}
