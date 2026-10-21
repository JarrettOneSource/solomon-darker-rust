//! Stock Inventory and learned Skillbook. Canonical simulation snapshots enter here;
//! typed commands leave through `take_actions`. Browser selection and dragging stay local.

pub(crate) mod chrome;
mod crafting;
mod dialogue;
mod flyby;
mod inventory;
mod inventory_drop;
mod notebox;
pub mod notices;
mod sack_pages;
mod sack_rename;
mod services;
mod skill_selector;
mod skills;
mod viewport;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use solomon_sim::Vec2;
use solomon_sim::audio::{AudioCue, AudioEvent, AudioOptions};
use solomon_sim::belt::{
    NATIVE_BELT_SLOT_COUNT, NativeBeltCommand, PlayerBeltComponent, native_belt_equipment_slots,
    native_belt_owned_item,
};
use solomon_sim::combat::PlayerCombatState;
use solomon_sim::economy::{
    EquipmentSlot, HubEconomyResult, HubEconomyState, HubInventoryAction, HubItemKind, HubTraderId,
    find_inventory_item, inventory_items_at_sack_path, reconcile_inventory_sack_path,
};
use solomon_sim::hub::interaction::{HubInteractionId, HubNpcChatContent, HubNpcSelectorId};
use solomon_sim::hub::interaction::{hub_boast_failure_text, hub_boast_instruction};
use solomon_sim::hud::belt_pull_off::native_belt_pull_off_started;
use solomon_sim::hud::skill_selector::NativeHudSkillSelectorTarget;
use solomon_sim::modding::content::ModBoastSelection;
use solomon_sim::modding::projection::{ModBoastProjectionEntry, ModContentProjection};
use solomon_sim::native_ui_notebox::NativeNoteboxKind;
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::primary::{
    NativePrimarySpellSummary, native_primary_spell_summary_with_stat_book,
};
use solomon_sim::profile::{
    NativeSkillBookState, NewGameSelection, PlayerProgressionState, PlayerStatBookComponent,
    native_skill_category,
};
use solomon_sim::settings::GameControlBindings;
use solomon_sim::tutorial_surface::NativeTutorialSurfaceAction;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    Document, Element, Event, HtmlElement, KeyboardEvent, MouseEvent, PointerEvent, WheelEvent,
};

use crate::hotbar::{HotbarControls, HotbarSelection};
use crate::menu::navigation::MenuNavigation;
use crate::ui::{StockAtlas, append, element};
use notices::PlayerPanelNotice;
use viewport::PanelSection;

/// These are derived by the owning session, alongside the combat and spell snapshots.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InventoryAttributes {
    pub melee_damage_minimum: f64,
    pub melee_damage_maximum: f64,
    pub cast_speed_percent: f64,
    pub walk_speed_percent: f64,
    pub pain_resistance_percent: f64,
    pub magic_resistance_percent: f64,
    pub poison_resistance_percent: f64,
    pub mana_recovery_per_second: f64,
}

/// Physical canvas client pixels, after the panel's actual transform and native scroll.
/// The renderer converts this into its own letterboxed coordinates and applies `clip`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InventoryPreviewPlacement {
    pub center: Vec2,
    pub scale: f64,
    pub clip: solomon_sim::hud::HudRect,
    pub alpha: f64,
}

pub struct PlayerPanelSnapshot<'a> {
    pub selection: &'a NewGameSelection,
    pub book: &'a NativeSkillBookState,
    /// Actual per-player authority, used by the source snapshot's primary summary.
    /// Stock Skills page text intentionally remains the immutable native catalog.
    pub stat_book: &'a PlayerStatBookComponent,
    pub economy: &'a HubEconomyState,
    pub mod_content: Option<&'a ModContentProjection>,
    pub mod_assets: Option<&'a crate::mod_presentation_assets::ModPresentationAssets>,
    pub belt: &'a PlayerBeltComponent,
    /// The session's transient dye owner; the inventory does not persist it.
    pub dye_session_id: Option<&'a str>,
    pub progression: &'a PlayerProgressionState,
    pub resources: &'a PlayerCombatState,
    pub offensive: &'a NativeOffensiveSpellFactors,
    pub attributes: InventoryAttributes,
    pub concentrations: &'a [Option<usize>; 2],
    pub mind_chug_active: bool,
    /// Source Hub interactions permit charm removal; Boneyard surfaces only inspect them.
    pub perk_removal_enabled: bool,
    pub kills: u64,
    pub awesomeness: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerPanelAction {
    OpenInventory,
    OpenSkills,
    Close,
    Surface(NativeTutorialSurfaceAction),
    AcknowledgeCollegeIntroDialogue,
    Inventory(HubInventoryAction),
    Belt(NativeBeltCommand),
    ActivateBeltSlot { slot: usize, pointer: Option<Vec2> },
    SelectPrimary(usize),
    SelectConcentration(usize),
    SelectConcentrationSlot { skill_id: usize, slot: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerPanel {
    Inventory,
    Skills,
    SkillSelector(NativeHudSkillSelectorTarget),
    Dialogue,
    Service(HubTraderId),
}

/// The session owns which NPC can be approached and which archived eulogy is available.
/// The random index only selects cosmetic dialogue; it never advances gameplay RNG.
#[derive(Clone, Copy, Debug)]
pub struct HubDialogueContext {
    pub interaction: HubInteractionId,
    pub story_office: bool,
    pub eulogy_override: Option<u32>,
    pub random_index: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    View(PanelSection),
    Close,
    Inventory,
    Skills,
    Back,
    StatsUp,
    StatsDown,
    Item(u64),
    Cell(usize),
    Equipment(EquipmentSlot),
    Skill(usize),
    Belt(usize),
    NoticeOkay,
    Unforge,
    UnforgeConfirm,
    UnforgeCancel,
    RenameSack,
    DyeSwatch(usize),
    DyeCancel,
    DyeCloth,
    DyeTrim,
    DialogueNext,
    DialogueAccelerate,
    DialogueChoice(usize),
    DialogueSelector(u32),
    SelectorDone,
    SelectorScroll,
    Shop(u64),
    Hagatha(i32),
    Storage(u64),
    ShopCell,
    OwnedPerk(u32),
    Dowse,
    HudSelectSkill(usize),
}

struct DyeSelection {
    session_id: String,
    admitted: bool,
    opened_at: f64,
    closing_at: Option<f64>,
    selected_at: Option<f64>,
    selected_row: Option<usize>,
    path: Vec<u64>,
    swatch_rows: Vec<usize>,
    target_item_id: Option<u64>,
    pending: bool,
}

struct DialogueState {
    context: HubDialogueContext,
    content: HubNpcChatContent,
    started_at: f64,
    accelerated_at: Option<f64>,
    scroll: f64,
    pending_selection:
        Option<(solomon_sim::hub::interaction::HubNpcListSelector, HubNpcSelectorId)>,
    pending_response: Option<(f64, HubNpcChatContent)>,
    college_intro_pending_ack: bool,
}

enum PanelEvent {
    DismissInspection,
    Activate(Target),
    SelectNpc(HubNpcSelectorId),
    Key(PanelKey),
    PointerDown(Target, Vec2, Option<HubNpcSelectorId>, i32),
    PointerMove(Vec2, Option<Target>, Option<HubNpcSelectorId>),
    PointerUp(Vec2, Option<Target>),
    Cancel,
    Wheel(f64),
    Focus(Target),
    FocusNpc(HubNpcSelectorId),
}

struct PanelKey {
    code: String,
    key: String,
    repeat: bool,
    modified: bool,
}

#[derive(Clone, Copy)]
struct Press {
    target: Target,
    inventory_item_id: Option<u64>,
    origin: Vec2,
    dragging: bool,
    pointer_id: i32,
}

#[derive(PartialEq)]
struct PanelRevision {
    economy: u64,
    book: NativeSkillBookState,
    belt: PlayerBeltComponent,
    hotbar_bank: usize,
    progression: u64,
    resources: [f64; 4],
    attributes: InventoryAttributes,
    primary_spell: NativePrimarySpellSummary,
    concentrations: [Option<usize>; 2],
    mind_chug_active: bool,
    kills: u64,
    awesomeness: u64,
    mod_boasts: Vec<ModBoastProjectionEntry>,
    dowsing_rolled: bool,
}

pub struct PlayerPanels {
    document: Document,
    root: HtmlElement,
    viewport: HtmlElement,
    surface: HtmlElement,
    compact_navigation: HtmlElement,
    overlay: HtmlElement,
    stage: HtmlElement,
    content: HtmlElement,
    tooltip: HtmlElement,
    dragger: HtmlElement,
    belt_effects: HtmlElement,
    flyby_layer: HtmlElement,
    inventory_flybys: Vec<flyby::InventoryFlyby>,
    burst_finished: Closure<dyn FnMut(Event)>,
    ui: StockAtlas,
    inventory: StockAtlas,
    skills: StockAtlas,
    library: StockAtlas,
    control_panel: StockAtlas,
    events: Rc<RefCell<Vec<PanelEvent>>>,
    actions: Vec<PlayerPanelAction>,
    audio_events: Vec<AudioEvent>,
    transfer_double_activation: bool,
    listeners: Vec<Closure<dyn FnMut(Event)>>,
    navigation: MenuNavigation,
    controls: GameControlBindings,
    hotbar: HotbarSelection,
    hotbar_controls: HotbarControls,
    panel: Option<PlayerPanel>,
    input_suspended: bool,
    selected: Option<Target>,
    selected_at: f64,
    hovered_skill: Option<usize>,
    target_belt: Option<usize>,
    press: Option<Press>,
    pressed_npc_selector: Option<HubNpcSelectorId>,
    pointer: Vec2,
    last_activation: Option<(Target, f64)>,
    sack_path: Vec<u64>,
    sack_transition: Option<sack_pages::SackPageTransition>,
    sack_page_views: Option<sack_pages::SackPageViews>,
    stats_page: usize,
    notice: Option<PlayerPanelNotice>,
    unforge_item: Option<u64>,
    dye: Option<DyeSelection>,
    rename_sack: Option<sack_rename::SackRenameDialog>,
    dialogue: Option<DialogueState>,
    dialogue_history: BTreeSet<String>,
    hovered_npc_selector: Option<HubNpcSelectorId>,
    npc_notebox: notebox::NpcNotebox,
    boast_instruction: Option<String>,
    boast_failure_sequence: Option<u64>,
    hovered_service: Option<Target>,
    dowsing_reference_item: Option<u64>,
    dowsing_rolled: bool,
    dirty: bool,
    revision: Option<PanelRevision>,
    opened_at: f64,
    closing_at: Option<f64>,
    replace_with: Option<PlayerPanel>,
    compact: bool,
    viewport_size: (i32, i32),
    viewport_panel: Option<PlayerPanel>,
    view_scale: f64,
}

impl PlayerPanels {
    pub async fn attach(document: &Document, hotbar: HotbarSelection) -> Result<Self, String> {
        let root = element(document, "div", "player-panels")?;
        root.set_attribute("hidden", "").map_err(|_| "panel hidden")?;
        let viewport = element(document, "div", "player-panel-viewport")?;
        let surface = element(document, "div", "player-panel-surface")?;
        let compact_navigation = element(document, "nav", "player-panel-navigation")?;
        let overlay = element(document, "div", "player-panel-overlay")?;
        compact_navigation
            .set_attribute("aria-label", "Panel sections")
            .map_err(|_| "panel section label")?;
        let stage = element(document, "div", "player-panel-stage")?;
        stage.set_attribute("role", "dialog").map_err(|_| "panel dialog")?;
        stage.set_attribute("aria-modal", "true").map_err(|_| "panel modal")?;
        let content = element(document, "div", "player-panel-content")?;
        let tooltip = element(document, "div", "player-panel-tooltip")?;
        tooltip.set_attribute("role", "tooltip").map_err(|_| "panel tooltip")?;
        let dragger = element(document, "div", "player-panel-dragger")?;
        let belt_effects = element(document, "div", "player-panel-belt-effects")?;
        let flyby_layer = element(document, "div", "player-panel-flybys")?;
        let belt_rects = solomon_sim::hud::native_hud_control_layout(1600.0, 900.0, Some(1.0)).belt;
        let hotbar_controls = HotbarControls::new(document, hotbar.clone(), Some(&belt_rects))?;
        append(&stage, &content)?;
        append(&stage, &belt_effects)?;
        append(&stage, &flyby_layer)?;
        append(&stage, &tooltip)?;
        append(&stage, &dragger)?;
        append(&surface, &stage)?;
        append(&viewport, &surface)?;
        append(&root, &viewport)?;
        append(&root, &compact_navigation)?;
        append(&root, &overlay)?;
        document
            .body()
            .ok_or("panel document body")?
            .append_child(&root)
            .map_err(|_| "panel attach")?;
        let burst_finished = crate::ui::belt_pull_off::listen_to_burst_completion(&belt_effects)?;
        let mut panels = Self {
            document: document.clone(),
            root,
            viewport,
            surface,
            compact_navigation,
            overlay,
            stage,
            content,
            tooltip,
            dragger,
            belt_effects,
            flyby_layer,
            inventory_flybys: Vec::new(),
            burst_finished,
            ui: StockAtlas::load("UI").await?,
            inventory: StockAtlas::load("Inventory").await?,
            skills: StockAtlas::load("Skills").await?,
            library: StockAtlas::load("Library").await?,
            control_panel: StockAtlas::load("ControlPanel").await?,
            events: Rc::new(RefCell::new(Vec::new())),
            actions: Vec::new(),
            audio_events: Vec::new(),
            transfer_double_activation: false,
            listeners: Vec::new(),
            navigation: MenuNavigation::new(),
            controls: GameControlBindings::default(),
            hotbar,
            hotbar_controls,
            panel: None,
            input_suspended: false,
            selected: None,
            selected_at: 0.0,
            hovered_skill: None,
            target_belt: None,
            press: None,
            pressed_npc_selector: None,
            pointer: Vec2::new(0.0, 0.0),
            last_activation: None,
            sack_path: Vec::new(),
            sack_transition: None,
            sack_page_views: None,
            stats_page: 0,
            notice: None,
            unforge_item: None,
            dye: None,
            rename_sack: None,
            dialogue: None,
            dialogue_history: BTreeSet::new(),
            hovered_npc_selector: None,
            npc_notebox: notebox::NpcNotebox::new(document)?,
            boast_instruction: None,
            boast_failure_sequence: None,
            hovered_service: None,
            dowsing_reference_item: None,
            dowsing_rolled: false,
            dirty: true,
            revision: None,
            opened_at: 0.0,
            closing_at: None,
            replace_with: None,
            compact: false,
            viewport_size: (0, 0),
            viewport_panel: None,
            view_scale: 1.0,
        };
        panels.attach_events()?;
        Ok(panels)
    }

    pub fn show(&mut self, panel: PlayerPanel, now: f64) -> Result<(), String> {
        self.close_sack_rename()?;
        self.clear_inventory_flybys()?;
        self.clear_sack_transition()?;
        let previous = self.panel;
        self.acknowledge_college_dialogue();
        if matches!(panel, PlayerPanel::SkillSelector(_)) {
            self.sound(AudioCue::Click);
        } else if panel != PlayerPanel::Dialogue {
            self.sound(AudioCue::OpenPanel);
        }
        self.panel = Some(panel);
        self.selected = None;
        self.hovered_skill = None;
        self.target_belt = None;
        self.press = None;
        self.belt_effects.set_text_content(None);
        self.pressed_npc_selector = None;
        self.last_activation = None;
        self.sack_path.clear();
        self.notice = None;
        self.unforge_item = None;
        self.close_dye_session();
        self.dialogue = None;
        self.hovered_npc_selector = None;
        self.hovered_service = None;
        self.dowsing_reference_item = None;
        self.stage
            .remove_attribute("data-native-dowsing-reference")
            .map_err(|_| "Dowsing reference reset")?;
        self.stage
            .remove_attribute("data-native-dowsing-rolled")
            .map_err(|_| "Dowsing roll reset")?;
        self.stats_page = 0;
        self.dirty = true;
        self.opened_at = now;
        self.closing_at = None;
        self.replace_with = None;
        self.navigation.release();
        self.root.remove_attribute("hidden").map_err(|_| "show player panel")?;
        self.stage
            .set_attribute(
                "aria-label",
                match panel {
                    PlayerPanel::Inventory => "Inventory",
                    PlayerPanel::Skills => "Skills",
                    PlayerPanel::SkillSelector(target) => target.title(),
                    PlayerPanel::Dialogue => "Conversation",
                    PlayerPanel::Service(trader) => services::service_title(trader),
                },
            )
            .map_err(|_| "panel label")?;
        self.stage
            .set_attribute(
                "data-player-panel",
                match panel {
                    PlayerPanel::Inventory => "inventory",
                    PlayerPanel::Skills => "skills",
                    PlayerPanel::SkillSelector(_) => "skill-selector",
                    PlayerPanel::Dialogue => "dialogue",
                    PlayerPanel::Service(trader) => trader.as_str(),
                },
            )
            .map_err(|_| "panel kind")?;
        self.surface_changed(previous, Some(panel));
        Ok(())
    }

    pub fn hide(&mut self) -> Result<(), String> {
        self.close_sack_rename()?;
        self.clear_inventory_flybys()?;
        self.clear_sack_transition()?;
        self.sack_path.clear();
        self.acknowledge_college_dialogue();
        self.close_dye_session();
        self.audio_events.push(AudioEvent::StopStream(AudioCue::ArchIntro0));
        let previous = self.panel.take();
        self.viewport_panel = None;
        self.belt_effects.set_text_content(None);
        self.press = None;
        self.pressed_npc_selector = None;
        self.root.set_attribute("hidden", "").map_err(|_| "hide player panel")?;
        self.navigation.release();
        self.surface_changed(previous, None);
        Ok(())
    }

    /// A departed scene has no recipient for pending panel actions or dialogue callbacks.
    pub fn reset(&mut self) -> Result<(), String> {
        self.audio_events.clear();
        self.hide()?;
        self.actions.clear();
        self.events.borrow_mut().clear();
        self.dialogue = None;
        self.dialogue_history.clear();
        self.dye = None;
        self.notice = None;
        self.closing_at = None;
        self.replace_with = None;
        self.npc_notebox.reset();
        self.boast_instruction = None;
        self.boast_failure_sequence = None;
        Ok(())
    }

    fn surface_changed(&mut self, previous: Option<PlayerPanel>, next: Option<PlayerPanel>) {
        if previous == next {
            return;
        }
        let closed = match previous {
            Some(PlayerPanel::Inventory) => Some(NativeTutorialSurfaceAction::InventoryClosed),
            Some(PlayerPanel::Skills) => Some(NativeTutorialSurfaceAction::SkillsClosed),
            _ => None,
        };
        let opened = match next {
            Some(PlayerPanel::Inventory) => Some(NativeTutorialSurfaceAction::InventoryOpened),
            Some(PlayerPanel::Skills) => Some(NativeTutorialSurfaceAction::SkillsOpened),
            Some(PlayerPanel::SkillSelector(NativeHudSkillSelectorTarget::Primary)) => {
                Some(NativeTutorialSurfaceAction::PrimarySelectorOpened)
            }
            Some(PlayerPanel::SkillSelector(NativeHudSkillSelectorTarget::ConcentrationA)) => {
                Some(NativeTutorialSurfaceAction::ConcentrationASelectorOpened)
            }
            _ => None,
        };
        for action in [closed, opened].into_iter().flatten() {
            self.actions.push(PlayerPanelAction::Surface(action));
        }
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.panel.is_some()
    }
    #[must_use]
    pub fn panel(&self) -> Option<PlayerPanel> {
        self.panel
    }
    #[must_use]
    pub fn covers_world(&self) -> bool {
        matches!(
            self.panel,
            Some(PlayerPanel::Inventory | PlayerPanel::Skills | PlayerPanel::Service(_))
        )
    }
    /// Keep the actual panel visible beneath chat while its native inert surface owns
    /// no pointer, keyboard or controller input. No canonical game state is changed.
    pub fn set_input_suspended(&mut self, suspended: bool) -> Result<(), String> {
        if self.input_suspended == suspended {
            return Ok(());
        }
        self.root
            .toggle_attribute_with_force("inert", suspended)
            .map_err(|_| "panel input admission")?;
        self.input_suspended = suspended;
        // The modal's closing key must not replay in the panel when it resumes.
        self.events.borrow_mut().retain(|event| !matches!(event, PanelEvent::Key(_)));
        self.navigation.release();
        if suspended {
            self.press = None;
            self.pressed_npc_selector = None;
        }
        Ok(())
    }

    pub fn set_controls(&mut self, controls: GameControlBindings) {
        self.controls = controls;
    }
    pub fn take_actions(&mut self) -> Vec<PlayerPanelAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn take_audio_events(&mut self) -> Vec<AudioEvent> {
        std::mem::take(&mut self.audio_events)
    }
    fn sound(&mut self, cue: AudioCue) {
        self.sound_at_rate(cue, 1.0);
    }
    fn sound_at_rate(&mut self, cue: AudioCue, playback_rate: f64) {
        self.audio_events
            .push(AudioEvent::Play(cue, AudioOptions { playback_rate, ..AudioOptions::default() }));
    }
    pub fn show_notice(&mut self, title: &str, body: &str) {
        self.notice = Some(PlayerPanelNotice::Standard { title: title.into(), body: body.into() });
        self.dirty = true;
    }

    /// Returns whether a source-authored modal was shown. Other rejections use the native
    /// bad-action cue/restoration; callers must not turn the reason code into UI text.
    pub fn show_inventory_rejection(
        &mut self,
        action: &HubInventoryAction,
        reason: solomon_sim::economy::HubEconomyRejection,
    ) -> bool {
        let Some(notice) =
            solomon_sim::player_panels::native_inventory_rejection_notice(action, reason)
        else {
            return false;
        };
        self.show_notice(notice.title, notice.body);
        true
    }

    /// Call once for the actual transaction result, after updating the canonical snapshot.
    /// A selector must not speak its success response merely because a command was queued.
    pub fn inventory_action_feedback(
        &mut self,
        action: &HubInventoryAction,
        result: &HubEconomyResult,
        now: f64,
        mod_content: Option<&ModContentProjection>,
    ) {
        self.inventory_flyby_feedback(action);
        if let Some(dialog) = &mut self.rename_sack {
            match dialog.action_feedback(action, result.accepted) {
                Ok(true) => {
                    if let Err(message) = self.close_sack_rename() {
                        web_sys::console::error_1(&message.into());
                    }
                }
                Ok(false) => {}
                Err(message) => web_sys::console::error_1(&message.into()),
            }
        }
        if self.dye_action_feedback(action, result) {
            self.dirty = true;
            return;
        }
        if result.accepted {
            let selector_response = self.accept_selector_action(action, now, mod_content);
            if !selector_response {
                self.successful_inventory_audio(action, result);
            }
            if matches!(action, HubInventoryAction::ReadSkillBook { .. }) {
                self.close(now, None);
            }
            if let Some(outcome) = &result.unforge_outcome {
                self.show_unforge_result(outcome);
            }
        } else {
            self.sound(AudioCue::BadAction);
            if let Some(dye) = &mut self.dye {
                dye.pending = false;
            }
            if let Some(dialogue) = &mut self.dialogue {
                dialogue.pending_selection = None;
            }
            if let Some(reason) = result.reason {
                self.show_inventory_rejection(action, reason);
            }
        }
        self.dirty = true;
    }

    fn successful_inventory_audio(
        &mut self,
        action: &HubInventoryAction,
        result: &HubEconomyResult,
    ) {
        match action {
            HubInventoryAction::BuyFomentius { .. }
            | HubInventoryAction::BuyHagatha { .. }
            | HubInventoryAction::BuyDowsing { .. } => self.sound(AudioCue::DropCoins),
            HubInventoryAction::Consume { .. } => self.sound(AudioCue::Drink),
            HubInventoryAction::Equip { .. } => self.sound(AudioCue::BackpackOpen),
            HubInventoryAction::Unforge { .. } => {
                let cue = if result.unforge_outcome.as_ref().is_some_and(|outcome| {
                    outcome.kind == solomon_sim::economy::NativeUnforgeOutcomeKind::Fizzle
                }) {
                    AudioCue::Fizzle
                } else {
                    AudioCue::Unforge
                };
                self.sound(cue);
            }
            HubInventoryAction::Transfer { .. } => {
                if std::mem::take(&mut self.transfer_double_activation) {
                    self.sound(AudioCue::BackpackClose);
                } else {
                    self.sound_at_rate(AudioCue::Click, 0.75);
                }
            }
            HubInventoryAction::Dowse { .. } => {
                self.sound(AudioCue::PickSkill);
                for (delay_ms, volume) in [(250.0, 0.25), (500.0, 0.0625), (750.0, 0.015625)] {
                    self.audio_events.push(AudioEvent::PlayAfter {
                        cue: AudioCue::PickSkill,
                        options: AudioOptions { volume, ..AudioOptions::default() },
                        delay_ms,
                    });
                }
            }
            _ => {}
        }
        if let Some(playback_rate) = result.dowsing_pitch {
            self.sound_at_rate(AudioCue::DistortReality, playback_rate);
        }
    }

    pub fn update(
        &mut self,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        // Service close callbacks read the latest accepted prop, including when
        // a roll receipt and Escape arrive in the same presentation frame.
        self.dowsing_rolled = snapshot.economy.dowsing_rolled;
        self.update_dye_state(snapshot, now);
        self.expire_sack_transition(now)?;
        if self.hotbar_controls.update(
            self.input_suspended
                || self.closing_at.is_some()
                || self.rename_sack.is_some()
                || self.inventory_transition_locked(),
        )? {
            self.belt_effects.set_text_content(None);
            // The source remounts belt pointer owners per bank, but retains the
            // selected visible target slot in the surrounding Skillbook.
            if let Some(slot) = self.target_belt {
                self.target_belt = Some(self.hotbar.slot(slot % NATIVE_BELT_SLOT_COUNT));
            }
            if self.press.is_some_and(|press| matches!(press.target, Target::Belt(_))) {
                self.press = None;
            }
            if matches!(self.selected, Some(Target::Belt(_))) {
                self.selected = None;
            }
            self.dirty = true;
        }
        self.boast_instruction = hub_boast_instruction(
            snapshot.economy.npc.boast.selected.as_ref(),
            snapshot.mod_content,
        )
        .map(str::to_owned);
        let sequence = snapshot.economy.npc.boast.failure_sequence;
        if self.boast_failure_sequence.is_some_and(|prior| sequence > prior)
            && let Some(text) =
                hub_boast_failure_text(&snapshot.economy.npc.boast, snapshot.mod_content)
        {
            // A late metadata message can supply the name; Source consumes
            // this sequence only once it can actually present the failure.
            self.boast_failure_sequence = Some(sequence);
            self.sound(AudioCue::BoastFailure);
            self.npc_notebox.show(&self.ui, NativeNoteboxKind::Failure, &text, now)?;
        } else if self.boast_failure_sequence.is_none() {
            self.boast_failure_sequence = Some(sequence);
        }
        // A selected source keeps its ownership. Equipping or consuming an item must
        // not reinterpret its old backpack id as a newly equipped item.
        let selection_exists = match self.selected {
            Some(Target::Item(id)) => find_inventory_item(&snapshot.economy.backpack, id).is_some(),
            Some(Target::Equipment(slot)) => snapshot.economy.equipment.at(slot).is_some(),
            Some(Target::Storage(id)) => {
                find_inventory_item(&snapshot.economy.storage, id).is_some()
            }
            _ => true,
        };
        if !selection_exists {
            self.selected = None;
            self.last_activation = None;
            self.dirty = true;
        }
        if self
            .dowsing_reference_item
            .is_some_and(|id| native_belt_owned_item(snapshot.economy, id).is_none())
        {
            self.dowsing_reference_item = None;
            self.dirty = true;
        }
        let events = std::mem::take(&mut *self.events.borrow_mut());
        if !self.input_suspended {
            for event in events {
                self.apply_event(event, snapshot, now)?;
            }
        }
        self.update_inventory_flybys(now)?;
        self.npc_notebox.update(now, width, height)?;
        if self.panel.is_none() {
            return Ok(());
        }
        self.update_viewport(width, height)?;
        let elapsed_ticks = ((now - self.opened_at) / 10.0).floor().max(0.0);
        let mut progress = if matches!(self.panel, Some(PlayerPanel::SkillSelector(_))) {
            1.0
        } else {
            let reveal_per_tick =
                if self.panel == Some(PlayerPanel::Dialogue) { 0.05 } else { 0.025 };
            (elapsed_ticks * reveal_per_tick).min(1.0)
        };
        if let Some(closing_at) = self.closing_at {
            progress = (1.0 - ((now - closing_at) / 10.0).floor().max(0.0) * 0.025).max(0.0);
            if progress == 0.0 {
                if let Some(panel) = self.replace_with.take() {
                    self.show(panel, now)?;
                } else {
                    self.hide()?;
                    self.actions.push(PlayerPanelAction::Close);
                }
                return Ok(());
            }
        }
        self.content
            .style()
            .set_property("opacity", &progress.to_string())
            .map_err(|_| "panel reveal")?;
        let sack_path = reconcile_inventory_sack_path(&snapshot.economy.backpack, &self.sack_path);
        if sack_path != self.sack_path {
            self.clear_inventory_flybys()?;
            self.clear_sack_transition()?;
            self.sack_path = sack_path;
            self.dirty = true;
        }
        let revision = PanelRevision {
            economy: snapshot.economy.revision,
            book: snapshot.book.clone(),
            belt: *snapshot.belt,
            hotbar_bank: self.hotbar.bank(),
            progression: snapshot.progression.revision,
            resources: [
                snapshot.resources.current_health,
                snapshot.resources.current_mana,
                snapshot.resources.maximum_health,
                snapshot.resources.maximum_mana,
            ],
            attributes: snapshot.attributes,
            primary_spell: native_primary_spell_summary_with_stat_book(
                snapshot.book,
                snapshot.stat_book,
                snapshot.offensive,
            ),
            concentrations: *snapshot.concentrations,
            mind_chug_active: snapshot.mind_chug_active,
            kills: snapshot.kills,
            awesomeness: snapshot.awesomeness,
            mod_boasts: snapshot.mod_content.map_or_else(Vec::new, |mods| mods.boasts.clone()),
            dowsing_rolled: snapshot.economy.dowsing_rolled,
        };
        if self.revision.as_ref() != Some(&revision) {
            self.revision = Some(revision);
            self.dirty = true;
        }
        if self.dirty {
            self.redraw(snapshot)?;
            self.dirty = false;
        }
        self.update_skill_ambient(now)?;
        self.paint_sack_pages(now)?;
        self.update_dye_presentation(now)?;
        self.update_sack_rename(snapshot, width, height)?;
        self.update_dialogue(now)?;
        self.update_tooltip(snapshot, now)?;
        self.update_dragger(snapshot)?;
        let scope = if self.rename_sack.is_some() {
            "sack-rename"
        } else if self.notice.is_some() {
            "panel-notice"
        } else if self.dye.is_some() {
            "fabric-dye"
        } else if self.panel == Some(PlayerPanel::Inventory) {
            "inventory"
        } else {
            "skills"
        };
        let navigation_root = if let Some(dialog) = &self.rename_sack {
            dialog.root.clone()
        } else {
            self.root
                .query_selector(if self.notice.is_some() {
                    ".player-panel-notice"
                } else {
                    ".panel-dye"
                })
                .map_err(|_| "notice scope")?
                .and_then(|element| element.dyn_into::<HtmlElement>().ok())
                .unwrap_or_else(|| self.content.clone())
        };
        if self.input_suspended {
            self.navigation.release();
        } else if self.navigation.update(&navigation_root, &self.document, scope, now)? {
            self.back(now)?;
        }
        Ok(())
    }

    fn redraw(&mut self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        let focus = self
            .document
            .active_element()
            .and_then(|element| element.get_attribute("data-panel-action"));
        self.content.set_text_content(None);
        self.overlay.set_text_content(None);
        self.sack_page_views = None;
        if !self.inventory_owner_active() {
            self.clear_sack_transition()?;
        }
        match self.panel {
            Some(PlayerPanel::Inventory) => self.draw_inventory(snapshot)?,
            Some(PlayerPanel::Skills) => self.draw_skills(snapshot)?,
            Some(PlayerPanel::SkillSelector(target)) => {
                self.draw_hud_skill_selector(target, snapshot)?
            }
            Some(PlayerPanel::Dialogue) => self.draw_dialogue(snapshot)?,
            Some(PlayerPanel::Service(trader)) => {
                self.draw_inventory(snapshot)?;
                self.draw_service(trader, snapshot)?;
            }
            None => {}
        }
        if !matches!(self.panel, Some(PlayerPanel::Dialogue | PlayerPanel::SkillSelector(_))) {
            self.draw_footer(snapshot)?;
        }
        if self.dye.is_some() {
            self.draw_dye(snapshot)?;
        }
        self.draw_sack_rename_open(snapshot)?;
        if let Some(notice) = &self.notice {
            self.draw_notice(notice)?;
        }
        if let Some(focus) = focus
            && let Some(target) = self
                .content
                .query_selector(&format!("[data-panel-action='{focus}']"))
                .map_err(|_| "restore panel focus")?
                .and_then(|element| element.dyn_into::<HtmlElement>().ok())
        {
            target.focus().map_err(|_| "panel focus")?;
        }
        Ok(())
    }

    fn back(&mut self, now: f64) -> Result<(), String> {
        if let Some(dialog) = &self.rename_sack {
            if dialog.can_cancel() {
                self.close_sack_rename()?;
            }
        } else if self.notice.take().is_some() {
            self.unforge_item = None;
            self.dirty = true;
        } else if self.dye.is_some() {
            self.cancel_dye(now);
            self.dirty = true;
        } else if self.panel == Some(PlayerPanel::Dialogue) {
            self.back_dialogue(now)?;
        } else if self.inventory_owner_active() && self.return_from_inventory_sack(now) {
            self.clear_inventory_flybys()?;
        } else {
            self.close(now, None);
        }
        Ok(())
    }

    fn close(&mut self, now: f64, replace_with: Option<PlayerPanel>) {
        if self.closing_at.is_none() {
            self.acknowledge_college_dialogue();
            if !matches!(self.panel, Some(PlayerPanel::SkillSelector(_))) {
                self.sound(AudioCue::OpenPanel);
            }
            self.finish_service();
            self.closing_at = Some(if matches!(self.panel, Some(PlayerPanel::SkillSelector(_))) {
                now - 400.0
            } else {
                now
            });
            self.replace_with = replace_with;
            self.press = None;
            self.pressed_npc_selector = None;
        }
    }

    fn apply_event(
        &mut self,
        event: PanelEvent,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
    ) -> Result<(), String> {
        if self.rename_sack.is_some() {
            return Ok(());
        }
        if let PanelEvent::Key(key) = event {
            if key.repeat {
                return Ok(());
            }
            if self.closing_at.is_some() {
                return Ok(());
            }
            let code = key.code;
            if self.panel.is_none() {
                if code == self.controls.open_inventory {
                    self.actions.push(PlayerPanelAction::OpenInventory);
                }
                if code == self.controls.open_skills {
                    self.actions.push(PlayerPanelAction::OpenSkills);
                }
            } else if code == "Escape" || code == self.controls.open_menu {
                if matches!(self.panel, Some(PlayerPanel::Service(_)))
                    && self.notice.is_none()
                    && self.dye.is_none()
                {
                    // HubInventoryUi gives the menu key a direct service close;
                    // the Inventory key and footer retain Sack Back navigation.
                    self.close(now, None);
                } else {
                    self.back(now)?;
                }
            } else if code == self.controls.open_inventory {
                if matches!(self.panel, Some(PlayerPanel::Service(_))) {
                    self.back(now)?;
                } else {
                    self.close(
                        now,
                        (self.panel != Some(PlayerPanel::Inventory))
                            .then_some(PlayerPanel::Inventory),
                    );
                }
            } else if code == self.controls.open_skills {
                self.close(
                    now,
                    (self.panel != Some(PlayerPanel::Skills)).then_some(PlayerPanel::Skills),
                );
            } else if self.notice.is_none()
                && self.dye.is_none()
                && self.panel == Some(PlayerPanel::Skills)
                && key.key.len() == 1
                && let Ok(slot) = key.key.parse::<usize>()
                && (1..8).contains(&slot)
            {
                self.target_belt = Some(self.hotbar.slot(slot));
                self.dirty = true;
            } else if self.notice.is_none()
                && self.dye.is_none()
                && !self.inventory_transition_locked()
                && !key.modified
                && matches!(self.panel, Some(PlayerPanel::Inventory | PlayerPanel::Service(_)))
                && let Some(visible_slot) =
                    self.controls.belt.iter().position(|binding| binding == &code)
            {
                let slot = self.hotbar.slot(visible_slot);
                if snapshot.belt.entries[slot].is_some() {
                    self.actions.push(PlayerPanelAction::ActivateBeltSlot { slot, pointer: None });
                }
            }
            return Ok(());
        }
        if self.panel.is_none() || self.closing_at.is_some() {
            return Ok(());
        }
        match event {
            PanelEvent::DismissInspection => {
                self.selected = None;
                self.hovered_skill = None;
                self.hovered_service = None;
                if let Some(active) = self.document.active_element()
                    && let Some(active) = active.dyn_ref::<HtmlElement>()
                {
                    active.blur().map_err(|_| "dismiss panel inspection")?;
                }
            }

            PanelEvent::Activate(target) => self.activate(target, snapshot, now, None)?,
            PanelEvent::SelectNpc(id) => self.select_npc(id, snapshot, now)?,
            PanelEvent::PointerDown(target, point, npc_id, pointer_id) => {
                if self.inventory_transition_locked() && inventory_drop::locks_target(target) {
                    return Ok(());
                }
                if self.notice.is_some()
                    && !matches!(
                        target,
                        Target::NoticeOkay | Target::UnforgeConfirm | Target::UnforgeCancel
                    )
                {
                    return Ok(());
                }
                self.pointer = point;
                let inventory_item_id = match target {
                    Target::Item(id) => Some(id),
                    Target::Equipment(slot) => {
                        snapshot.economy.equipment.at(slot).as_ref().map(|item| item.id)
                    }
                    _ => None,
                };
                self.press = Some(Press {
                    target,
                    inventory_item_id,
                    origin: point,
                    dragging: false,
                    pointer_id,
                });
                self.pressed_npc_selector = npc_id;
                if matches!(target, Target::Item(_)) {
                    self.selected = Some(target);
                    self.selected_at = now;
                    self.dirty = true;
                }
            }
            PanelEvent::PointerMove(point, target, npc_id) => {
                let previous_point = self.pointer;
                self.pointer = point;
                let scrolling = self.press.is_some_and(|press| {
                    matches!(press.target, Target::DialogueSelector(_) | Target::SelectorScroll)
                        && ((point.x - press.origin.x).hypot(point.y - press.origin.y) > 10.0
                            || press.dragging)
                });
                if scrolling {
                    self.press.as_mut().expect("captured selector pointer").dragging = true;
                    self.scroll_selector_pixels(previous_point.y - point.y);
                    return Ok(());
                }
                if let Some(press) = self.press
                    && let Target::Belt(slot) = press.target
                    && snapshot.belt.entries[slot].is_some()
                    && self.dye.is_none()
                    && native_belt_pull_off_started(press.origin, point)
                {
                    self.press = None;
                    self.target_belt = None;
                    self.actions.push(PlayerPanelAction::Belt(NativeBeltCommand::BindSkill {
                        skill_id: None,
                        slot,
                    }));
                    self.sound(AudioCue::Poof);
                    self.show_modal_belt_pull_off(slot)?;
                    self.dirty = true;
                    if self.root.has_pointer_capture(press.pointer_id) {
                        self.root
                            .release_pointer_capture(press.pointer_id)
                            .map_err(|_| "modal belt pointer release")?;
                    }
                    return Ok(());
                }
                if let Some(press) = &mut self.press {
                    let distance = (point.x - press.origin.x).hypot(point.y - press.origin.y);
                    let threshold = if matches!(press.target, Target::Belt(_)) {
                        50.0
                    } else if matches!(press.target, Target::Skill(_)) {
                        3.0
                    } else {
                        10.0
                    };
                    if distance > threshold
                        && self.dye.is_none()
                        && !matches!(press.target, Target::Belt(_))
                        && !press.dragging
                    {
                        press.dragging = true;
                        self.dirty = true;
                    }
                } else {
                    if self.hovered_npc_selector != npc_id {
                        self.hovered_npc_selector = npc_id;
                        self.dirty = true;
                    }
                    self.hovered_service = target.filter(|target| {
                        matches!(
                            target,
                            Target::Shop(_)
                                | Target::Hagatha(_)
                                | Target::Storage(_)
                                | Target::OwnedPerk(_)
                        )
                    });
                    let skill = match target {
                        Some(Target::Skill(id)) => Some(id),
                        _ => None,
                    };
                    self.hovered_skill = skill;
                }
            }
            PanelEvent::PointerUp(point, target) => {
                self.pointer = point;
                let npc_id = self.pressed_npc_selector.take();
                if let Some(press) = self.press.take() {
                    if press.dragging {
                        self.drop(press, target, snapshot, now)?;
                    } else if let Some(id) = npc_id {
                        self.select_npc(id, snapshot, now)?;
                    } else {
                        let bounds = self.stage.get_bounding_client_rect();
                        let pointer = Vec2::new(
                            bounds.left() + point.x * bounds.width() / 1600.0,
                            bounds.top() + point.y * bounds.height() / 900.0,
                        );
                        self.activate(press.target, snapshot, now, Some(pointer))?;
                    }
                }
            }
            PanelEvent::Cancel => {
                self.press = None;
                self.pressed_npc_selector = None;
                self.dirty = true;
            }
            PanelEvent::Wheel(delta) => self.scroll_selector(delta),
            PanelEvent::Focus(target) => {
                if self.hovered_npc_selector.take().is_some() {
                    self.dirty = true;
                }
                self.hovered_service = Some(target).filter(|target| {
                    matches!(
                        target,
                        Target::Shop(_)
                            | Target::Hagatha(_)
                            | Target::Storage(_)
                            | Target::OwnedPerk(_)
                    )
                });
            }
            PanelEvent::FocusNpc(id) => {
                self.reveal_selector_focus(&id)?;
                self.hovered_npc_selector = Some(id);
                self.dirty = true;
            }
            PanelEvent::Key(_) => unreachable!(),
        }
        Ok(())
    }

    fn activate(
        &mut self,
        target: Target,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
        pointer: Option<Vec2>,
    ) -> Result<(), String> {
        if self.inventory_transition_locked() && inventory_drop::locks_target(target) {
            return Ok(());
        }
        if let Target::View(section) = target {
            self.show_section(section)?;
            return Ok(());
        }
        if self.notice.is_some()
            && !matches!(
                target,
                Target::NoticeOkay | Target::UnforgeConfirm | Target::UnforgeCancel
            )
        {
            return Ok(());
        }
        self.dirty = true;
        if self.dye.is_some() {
            self.activate_dye(target, snapshot, now);
            return Ok(());
        }
        match target {
            Target::View(_) => unreachable!("section navigation handled above"),
            Target::Close | Target::Back => self.back(now)?,
            Target::Inventory => self.close(now, Some(PlayerPanel::Inventory)),
            Target::Skills => self.close(now, Some(PlayerPanel::Skills)),
            Target::NoticeOkay => {
                self.notice = None;
            }
            Target::Unforge => {
                if let Some(Target::Item(id)) = self.selected {
                    self.begin_unforge(id, snapshot);
                }
            }
            Target::UnforgeConfirm => {
                if let Some(item_id) = self.unforge_item.take() {
                    self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::Unforge {
                        item_id,
                    }));
                }
                self.notice = None;
            }
            Target::UnforgeCancel => {
                self.unforge_item = None;
                self.notice = None;
            }
            Target::RenameSack => self.open_sack_rename(snapshot)?,
            Target::DyeSwatch(_) | Target::DyeCancel | Target::DyeCloth | Target::DyeTrim => {}
            Target::StatsUp => self.stats_page = self.stats_page.saturating_sub(1),
            Target::StatsDown => self.stats_page = (self.stats_page + 1).min(2),
            Target::Belt(slot) => {
                if self.panel != Some(PlayerPanel::Skills) {
                    if snapshot.belt.entries[slot].is_some() {
                        self.actions.push(PlayerPanelAction::ActivateBeltSlot { slot, pointer });
                    }
                    return Ok(());
                }
                self.sound(AudioCue::PickSkill);
                self.target_belt = Some(slot);
                if let Some(Target::Skill(skill_id)) = self.selected {
                    self.actions.push(PlayerPanelAction::Belt(NativeBeltCommand::BindSkill {
                        skill_id: Some(skill_id),
                        slot,
                    }));
                } else if let Some(Target::Item(item_id)) = self.selected {
                    self.actions.push(PlayerPanelAction::Belt(NativeBeltCommand::BindItem {
                        item_id,
                        slot,
                    }));
                }
            }
            Target::Skill(skill_id) => {
                self.sound(AudioCue::PickSkill);
                self.selected = Some(target);
                self.hovered_skill = Some(skill_id);
                if let Some(slot) = self.target_belt.take() {
                    self.actions.push(PlayerPanelAction::Belt(NativeBeltCommand::BindSkill {
                        skill_id: Some(skill_id),
                        slot,
                    }));
                } else if native_skill_category(skill_id) == Some(1) {
                    self.actions.push(PlayerPanelAction::SelectPrimary(skill_id));
                } else if native_skill_category(skill_id) == Some(3) {
                    self.actions.push(PlayerPanelAction::SelectConcentration(skill_id));
                }
            }
            Target::HudSelectSkill(skill_id) => self.select_hud_skill(skill_id, snapshot, now),
            Target::Cell(slot) => {
                if let Some(Target::Item(item_id)) = self.selected {
                    self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::Move {
                        item_id,
                        destination_sack_id: self.sack_path.last().copied(),
                        destination_slot: Some(slot),
                    }));
                } else {
                    self.selected = None;
                }
            }
            Target::Item(item_id) => {
                self.sound(AudioCue::Click);
                self.selected = Some(target);
                self.selected_at = now;
                let double = self
                    .last_activation
                    .is_some_and(|(previous, at)| previous == target && now - at <= 500.0);
                self.last_activation = Some((target, now));
                if double {
                    self.last_activation = None;
                    if let Some(item) = native_belt_owned_item(snapshot.economy, item_id) {
                        match item.kind {
                            HubItemKind::Sack => {
                                self.clear_inventory_flybys()?;
                                self.open_inventory_sack(item_id, snapshot, now);
                            }
                            HubItemKind::SkillBook => {
                                self.actions.push(PlayerPanelAction::Inventory(
                                    HubInventoryAction::ReadSkillBook { item_id },
                                ))
                            }
                            HubItemKind::HealthPotion
                            | HubItemKind::ManaPotion
                            | HubItemKind::RejuvenationPotion
                            | HubItemKind::WizardChug
                            | HubItemKind::MindChug
                            | HubItemKind::ModPotion
                            | HubItemKind::Antidote => {
                                self.actions.push(PlayerPanelAction::Inventory(
                                    HubInventoryAction::Consume { item_id },
                                ))
                            }
                            HubItemKind::Equipment => {
                                // `equipmentSlotsForItem` excludes the third ring until
                                // Disfiguring Curse unlocks it, even when that slot is empty.
                                let slots = native_belt_equipment_slots(
                                    item,
                                    snapshot.economy.owned_perk_selectors.contains(&19),
                                );
                                let slot = slots
                                    .iter()
                                    .copied()
                                    .find(|slot| snapshot.economy.equipment.at(*slot).is_none())
                                    .or_else(|| slots.first().copied());
                                if let Some(slot) = slot {
                                    self.actions.push(PlayerPanelAction::Inventory(
                                        HubInventoryAction::Equip { item_id, slot },
                                    ));
                                }
                            }
                            HubItemKind::Dye => {
                                self.open_dye(item_id, now)?;
                            }
                            HubItemKind::Key | HubItemKind::ModItem => {}
                        }
                    }
                }
            }
            Target::Equipment(slot) => {
                if let Some(Target::Item(item_id)) = self.selected
                    && find_inventory_item(&snapshot.economy.backpack, item_id)
                        .is_some_and(|item| item.equipment_type.is_some())
                {
                    self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::Equip {
                        item_id,
                        slot,
                    }));
                    self.selected = Some(target);
                } else {
                    let double = self
                        .last_activation
                        .is_some_and(|(previous, at)| previous == target && now - at <= 500.0);
                    self.last_activation = Some((target, now));
                    self.selected = Some(target);
                    self.selected_at = now;
                    if double {
                        self.last_activation = None;
                        if matches!(slot, EquipmentSlot::Hat | EquipmentSlot::Robe) {
                            self.show_inventory_rejection(
                                &HubInventoryAction::Unequip {
                                    slot,
                                    destination_sack_id: None,
                                    destination_slot: None,
                                },
                                solomon_sim::economy::HubEconomyRejection::RequiredClothing,
                            );
                        } else {
                            self.actions.push(PlayerPanelAction::Inventory(
                                HubInventoryAction::Unequip {
                                    slot,
                                    destination_sack_id: None,
                                    destination_slot: None,
                                },
                            ));
                        }
                    }
                }
            }
            Target::OwnedPerk(selector) => {
                if snapshot.perk_removal_enabled && selector != 27 {
                    self.sound(AudioCue::Click);
                    self.actions.push(PlayerPanelAction::Inventory(
                        HubInventoryAction::RemoveHagatha { selector },
                    ));
                }
            }
            Target::DialogueNext
            | Target::DialogueAccelerate
            | Target::DialogueChoice(_)
            | Target::DialogueSelector(_)
            | Target::SelectorDone => self.activate_dialogue(target, snapshot, now)?,
            Target::SelectorScroll => {}
            Target::Shop(_)
            | Target::Hagatha(_)
            | Target::Storage(_)
            | Target::ShopCell
            | Target::Dowse => self.activate_service(target, snapshot, now),
        }
        Ok(())
    }

    fn active_items<'a>(
        &self,
        snapshot: &'a PlayerPanelSnapshot<'_>,
    ) -> &'a [solomon_sim::economy::HubInventoryItem] {
        inventory_items_at_sack_path(&snapshot.economy.backpack, &self.sack_path)
            .unwrap_or(&snapshot.economy.backpack)
    }

    fn show_modal_belt_pull_off(&self, slot: usize) -> Result<(), String> {
        let visible_slot = slot % NATIVE_BELT_SLOT_COUNT;
        let rect = solomon_sim::hud::native_hud_control_layout(1600.0, 900.0, Some(1.0)).belt
            [visible_slot];
        let anchor = element(&self.document, "span", "panel-belt-burst-anchor")?;
        anchor
            .set_attribute("data-belt-burst-anchor", "true")
            .map_err(|_| "modal belt burst owner")?;
        anchor.set_attribute("aria-hidden", "true").map_err(|_| "modal belt decoration")?;
        chrome::place(&anchor, rect)?;
        let burst = crate::ui::belt_pull_off::belt_pull_off_burst(&self.document, &self.ui)?;
        append(&anchor, &burst)?;
        append(&self.belt_effects, &anchor)
    }

    fn attach_events(&mut self) -> Result<(), String> {
        let events = self.events.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            // Pointer activation is handled by the captured release. Keyboard and gamepad
            // use the native button click (detail zero), sharing the same command path.
            if event.dyn_ref::<MouseEvent>().is_some_and(|event| event.detail() != 0) {
                return;
            }
            if let Some(id) = npc_selector_from_event(&event) {
                events.borrow_mut().push(PanelEvent::SelectNpc(id));
                return;
            }
            if let Some(target) = target_from_event(&event) {
                events.borrow_mut().push(PanelEvent::Activate(target));
            }
        });
        self.root
            .add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "panel click")?;
        self.listeners.push(click);
        let events = self.events.clone();
        let focus = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            if let Some(id) = npc_selector_from_event(&event) {
                events.borrow_mut().push(PanelEvent::FocusNpc(id));
                return;
            }
            if let Some(target) = target_from_event(&event) {
                events.borrow_mut().push(PanelEvent::Focus(target));
            }
        });
        self.root
            .add_event_listener_with_callback("focusin", focus.as_ref().unchecked_ref())
            .map_err(|_| "panel focus listener")?;
        self.listeners.push(focus);
        let events = self.events.clone();
        let key =
            Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                    return;
                };
                if key.target().and_then(|target| target.dyn_into::<Element>().ok()).is_some_and(
                    |target| matches!(target.tag_name().as_str(), "INPUT" | "TEXTAREA"),
                ) {
                    return;
                }
                events.borrow_mut().push(PanelEvent::Key(PanelKey {
                    code: key.code(),
                    key: key.key(),
                    repeat: key.repeat(),
                    modified: key.alt_key() || key.ctrl_key() || key.meta_key(),
                }));
            });
        self.document
            .add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref())
            .map_err(|_| "panel keyboard")?;
        self.listeners.push(key);
        let events = self.events.clone();
        let wheel = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            if event.target().and_then(|target| target.dyn_into::<Element>().ok()).is_some_and(
                |target| target.closest(".player-panel-tooltip").ok().flatten().is_some(),
            ) {
                return;
            }
            if let Some(wheel) = event.dyn_ref::<WheelEvent>() {
                event.prevent_default();
                events.borrow_mut().push(PanelEvent::Wheel(wheel.delta_y()));
            }
        });
        self.root
            .add_event_listener_with_callback("wheel", wheel.as_ref().unchecked_ref())
            .map_err(|_| "panel wheel")?;
        self.listeners.push(wheel);
        for name in
            ["pointerdown", "pointermove", "pointerup", "pointercancel", "lostpointercapture"]
        {
            let events = self.events.clone();
            let stage = self.stage.clone();
            let root = self.root.clone();
            let document = self.document.clone();
            let listener = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
                    return;
                };
                let rect = stage.get_bounding_client_rect();
                let client = match crate::input::pointer_client_point(pointer) {
                    Ok(point) => point,
                    Err(error) => {
                        web_sys::console::error_1(&error.into());
                        return;
                    }
                };
                let point = Vec2::new(
                    (client.x - rect.left()) * 1600.0 / rect.width(),
                    (client.y - rect.top()) * 900.0 / rect.height(),
                );
                if name == "pointerdown" {
                    if pointer.button() != 0 {
                        return;
                    }
                    if event
                        .target()
                        .and_then(|target| target.dyn_into::<Element>().ok())
                        .is_some_and(|target| {
                            target.closest(".player-panel-tooltip").ok().flatten().is_some()
                        })
                    {
                        return;
                    }
                    if let Some(target) = target_from_event(&event) {
                        let native_scroll = pointer.pointer_type() == "touch"
                            && event
                                .target()
                                .and_then(|target| target.dyn_into::<Element>().ok())
                                .is_some_and(|target| {
                                    target
                                        .closest(".panel-hud-selector-options")
                                        .ok()
                                        .flatten()
                                        .is_some()
                                });
                        if !native_scroll {
                            event.prevent_default();
                            if let Err(error) = root.set_pointer_capture(pointer.pointer_id()) {
                                web_sys::console::error_1(&error);
                            }
                        }
                        events.borrow_mut().push(PanelEvent::PointerDown(
                            target,
                            point,
                            npc_selector_from_event(&event),
                            pointer.pointer_id(),
                        ));
                    } else if root.get_attribute("data-compact").as_deref() == Some("true") {
                        events.borrow_mut().push(PanelEvent::DismissInspection);
                    }
                } else if name == "pointercancel" || name == "lostpointercapture" {
                    events.borrow_mut().push(PanelEvent::Cancel);
                } else {
                    let element = document.element_from_point(client.x as f32, client.y as f32);
                    let target = element.as_ref().and_then(target_from_element);
                    let npc_id = element.as_ref().and_then(npc_selector_from_element);
                    events.borrow_mut().push(if name == "pointerup" {
                        PanelEvent::PointerUp(point, target)
                    } else {
                        PanelEvent::PointerMove(point, target, npc_id)
                    });
                    if name == "pointerup"
                        && root.has_pointer_capture(pointer.pointer_id())
                        && let Err(error) = root.release_pointer_capture(pointer.pointer_id())
                    {
                        web_sys::console::error_1(&error);
                    }
                }
            });
            self.root
                .add_event_listener_with_callback(name, listener.as_ref().unchecked_ref())
                .map_err(|_| "panel pointer listener")?;
            self.listeners.push(listener);
        }
        Ok(())
    }
}

impl Drop for PlayerPanels {
    fn drop(&mut self) {
        if let Err(error) = self.belt_effects.remove_event_listener_with_callback(
            "animationend",
            self.burst_finished.as_ref().unchecked_ref(),
        ) {
            web_sys::console::error_1(&error);
        }
    }
}

fn npc_selector_from_event(event: &Event) -> Option<HubNpcSelectorId> {
    npc_selector_from_element(&event.target()?.dyn_into::<Element>().ok()?)
}

fn npc_selector_from_element(element: &Element) -> Option<HubNpcSelectorId> {
    let row = element.closest("[data-npc-selector]").expect("valid NPC selector row")?;
    let id = row.get_attribute("data-npc-selector").expect("NPC selector identity");
    if let Some(mod_id) = row.get_attribute("data-npc-mod") {
        Some(HubNpcSelectorId::ModBoast(
            ModBoastSelection::new(id, mod_id)
                .expect("selector metadata has an admitted namespaced Boast"),
        ))
    } else {
        Some(HubNpcSelectorId::Native(id.parse().expect("selector has a native numeric ID")))
    }
}

fn target_from_event(event: &Event) -> Option<Target> {
    target_from_element(&event.target()?.dyn_into::<Element>().ok()?)
}

fn target_from_element(element: &Element) -> Option<Target> {
    let action =
        element.closest("[data-panel-action]").ok()??.get_attribute("data-panel-action")?;
    Some(match action.as_str() {
        "close" => Target::Close,
        "inventory" => Target::Inventory,
        "skills" => Target::Skills,
        "back" => Target::Back,
        "stats-up" => Target::StatsUp,
        "stats-down" => Target::StatsDown,
        "notice-okay" => Target::NoticeOkay,
        "unforge" => Target::Unforge,
        "unforge-confirm" => Target::UnforgeConfirm,
        "unforge-cancel" => Target::UnforgeCancel,
        "rename-sack" => Target::RenameSack,
        "dye-cancel" => Target::DyeCancel,
        "dye-cloth" => Target::DyeCloth,
        "dye-trim" => Target::DyeTrim,
        "dialogue-next" => Target::DialogueNext,
        "dialogue-accelerate" => Target::DialogueAccelerate,
        "selector-done" => Target::SelectorDone,
        "selector-scroll" => Target::SelectorScroll,
        "shop-cell" => Target::ShopCell,
        "dowse" => Target::Dowse,
        "view-stats" => Target::View(PanelSection::Stats),
        "view-backpack" => Target::View(PanelSection::Backpack),
        "view-equipment" => Target::View(PanelSection::Equipment),
        "view-shop" => Target::View(PanelSection::Shop),
        "view-previous" => Target::View(PanelSection::Previous),
        "view-next" => Target::View(PanelSection::Next),
        _ => {
            let (kind, value) = action.split_once(':')?;
            match kind {
                "item" => Target::Item(value.parse().ok()?),
                "cell" => Target::Cell(value.parse().ok()?),
                "skill" => Target::Skill(value.parse().ok()?),
                "belt" => Target::Belt(value.parse().ok()?),
                "dye-swatch" => Target::DyeSwatch(value.parse().ok()?),
                "dialogue-choice" => Target::DialogueChoice(value.parse().ok()?),
                "dialogue-selector" => Target::DialogueSelector(value.parse().ok()?),
                "hud-select-skill" => Target::HudSelectSkill(value.parse().ok()?),
                "shop" => Target::Shop(value.parse().ok()?),
                "hagatha" => Target::Hagatha(value.parse().ok()?),
                "storage" => Target::Storage(value.parse().ok()?),
                "owned-perk" => Target::OwnedPerk(value.parse().ok()?),
                "equipment" => Target::Equipment(
                    EquipmentSlot::ALL.into_iter().find(|slot| slot.as_str() == value)?,
                ),
                _ => return None,
            }
        }
    })
}
