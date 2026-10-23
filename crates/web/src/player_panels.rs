//! Stock Inventory, learned Skillbook and College interactions. Accepted snapshots
//! enter once; GPU views emit the same typed commands as the source controllers.
pub(crate) mod chrome;
mod crafting;
mod dialogue;
mod flyby;
mod inventory;
mod inventory_drop;
mod model;
mod notebox;
pub mod notices;
mod sack_pages;
mod sack_rename;
mod services;
mod skill_book_feedback;
mod skill_selector;
mod skills;
mod viewport;

use std::cell::RefCell;
use std::collections::BTreeSet;

use crate::gpu_ui::{StockAtlas, UiFrame};
use crate::hotbar::{HotbarControls, HotbarSelection};
use notices::PlayerPanelNotice;
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
use solomon_sim::modding::projection::ModContentProjection;
use solomon_sim::native_ui_notebox::NativeNoteboxKind;
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::{
    NativeSkillBookState, NewGameSelection, PlayerProgressionState, PlayerStatBookComponent,
    native_skill_category,
};
use solomon_sim::settings::GameControlBindings;
use solomon_sim::tutorial_surface::NativeTutorialSurfaceAction;
use viewport::PanelSection;
use web_sys::Document;

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

/// The native UI.62 seal under the live wizard, expressed in CSS canvas pixels.
#[derive(Clone, Copy, Debug)]
pub struct InventoryPreviewUnderlay {
    pub texture_path: &'static str,
    pub source: solomon_sim::hud::HudRect,
    pub destination: solomon_sim::hud::HudRect,
    pub clip: solomon_sim::hud::HudRect,
    pub tint: u32,
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
    SkillBookResultOkay,
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
    PointerDown(Target, Vec2, Option<HubNpcSelectorId>),
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
}

pub struct PlayerPanels {
    document: Document,
    inventory_flybys: Vec<flyby::InventoryFlyby>,
    model: Option<model::PanelModel>,
    mod_icons: std::collections::BTreeMap<u64, Vec<crate::ui::mod_icon::ModIconLayer>>,
    mod_boast_images: std::collections::BTreeMap<String, (String, [u32; 2])>,
    bursts: RefCell<Vec<(usize, crate::ui::belt_pull_off::BeltPullOffBurst)>>,
    scroll: Vec2,
    stage_origin: Vec2,
    alpha: f64,
    now: f64,
    selector_height: f64,
    hits: Vec<chrome::PanelHit>,
    ui: StockAtlas,
    inventory: StockAtlas,
    skills: StockAtlas,
    library: StockAtlas,
    control_panel: StockAtlas,
    events: Vec<PanelEvent>,
    actions: Vec<PlayerPanelAction>,
    audio_events: Vec<AudioEvent>,
    transfer_double_activation: bool,
    controls: GameControlBindings,
    navigation: crate::menu::navigation::MenuNavigation,
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
    pan_pointer: Option<egui::Pos2>,
    last_activation: Option<(Target, f64)>,
    sack_path: Vec<u64>,
    sack_transition: Option<sack_pages::SackPageTransition>,
    stats_page: usize,
    notice: Option<PlayerPanelNotice>,
    skill_book_feedback: skill_book_feedback::SkillBookFeedback,
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
        let belt_rects = solomon_sim::hud::native_hud_control_layout(1600.0, 900.0, Some(1.0)).belt;
        Ok(Self {
            document: document.clone(),
            inventory_flybys: Vec::new(),
            model: None,
            mod_icons: std::collections::BTreeMap::new(),
            mod_boast_images: std::collections::BTreeMap::new(),
            bursts: RefCell::new(Vec::new()),
            scroll: Vec2::ZERO,
            stage_origin: Vec2::ZERO,
            alpha: 0.0,
            now: 0.0,
            selector_height: 0.0,
            hits: Vec::new(),
            ui: StockAtlas::load("UI").await?,
            inventory: StockAtlas::load("Inventory").await?,
            skills: StockAtlas::load("Skills").await?,
            library: StockAtlas::load("Library").await?,
            control_panel: StockAtlas::load("ControlPanel").await?,
            events: Vec::new(),
            actions: Vec::new(),
            audio_events: Vec::new(),
            transfer_double_activation: false,
            controls: GameControlBindings::default(),
            navigation: crate::menu::navigation::MenuNavigation::new(),
            hotbar_controls: HotbarControls::new(document, hotbar.clone(), Some(&belt_rects))?,
            hotbar,
            panel: None,
            input_suspended: false,
            selected: None,
            selected_at: 0.0,
            hovered_skill: None,
            target_belt: None,
            press: None,
            pressed_npc_selector: None,
            pointer: Vec2::ZERO,
            pan_pointer: None,
            last_activation: None,
            sack_path: Vec::new(),
            sack_transition: None,
            stats_page: 0,
            notice: None,
            skill_book_feedback: skill_book_feedback::SkillBookFeedback::new(),
            unforge_item: None,
            dye: None,
            rename_sack: None,
            dialogue: None,
            dialogue_history: BTreeSet::new(),
            hovered_npc_selector: None,
            npc_notebox: notebox::NpcNotebox::new(),
            boast_instruction: None,
            boast_failure_sequence: None,
            hovered_service: None,
            dowsing_reference_item: None,
            dowsing_rolled: false,
            opened_at: 0.0,
            closing_at: None,
            replace_with: None,
            compact: false,
            viewport_size: (0, 0),
            viewport_panel: None,
            view_scale: 1.0,
        })
    }
    pub fn show(&mut self, panel: PlayerPanel, now: f64) -> Result<(), String> {
        self.close_sack_rename();
        self.clear_inventory_flybys()?;
        self.clear_sack_transition()?;
        let previous = self.panel;
        self.acknowledge_college_dialogue();
        if matches!(panel, PlayerPanel::SkillSelector(_)) {
            self.sound(AudioCue::Click);
        } else if panel != PlayerPanel::Dialogue {
            self.sound(AudioCue::OpenPanel);
        }
        self.navigation.release();
        self.panel = Some(panel);
        self.selected = None;
        self.hovered_skill = None;
        self.target_belt = None;
        self.press = None;
        self.bursts.borrow_mut().clear();
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
        self.stats_page = 0;

        self.opened_at = now;
        self.closing_at = None;
        self.replace_with = None;

        self.surface_changed(previous, Some(panel));
        Ok(())
    }

    pub fn hide(&mut self) -> Result<(), String> {
        self.close_sack_rename();
        self.clear_inventory_flybys()?;
        self.clear_sack_transition()?;
        self.sack_path.clear();
        self.acknowledge_college_dialogue();
        self.close_dye_session();
        self.audio_events.push(AudioEvent::StopStream(AudioCue::ArchIntro0));
        self.navigation.release();
        let previous = self.panel.take();
        self.viewport_panel = None;
        self.bursts.borrow_mut().clear();
        self.press = None;
        self.pressed_npc_selector = None;

        self.surface_changed(previous, None);
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), String> {
        self.audio_events.clear();
        self.hide()?;
        self.actions.clear();
        self.events.clear();
        self.dialogue = None;
        self.dialogue_history.clear();
        self.dye = None;
        self.notice = None;
        self.closing_at = None;
        self.replace_with = None;
        self.reset_skill_book_feedback(None);
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

    pub fn is_open(&self) -> bool {
        self.panel.is_some() || self.skill_book_result_open()
    }

    pub fn panel(&self) -> Option<PlayerPanel> {
        self.panel
    }

    pub fn covers_world(&self) -> bool {
        matches!(
            self.panel,
            Some(PlayerPanel::Inventory | PlayerPanel::Skills | PlayerPanel::Service(_))
        )
    }

    pub fn set_input_suspended(&mut self, suspended: bool) -> Result<(), String> {
        if self.input_suspended == suspended {
            return Ok(());
        }
        self.input_suspended = suspended;
        // The modal's closing key must not replay in the panel when it resumes.
        self.events.retain(|event| !matches!(event, PanelEvent::Key(_)));

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
    }

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
                    self.close_sack_rename();
                }
                Ok(false) => {}
                Err(message) => web_sys::console::error_1(&message.into()),
            }
        }
        if self.dye_action_feedback(action, result) {
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

    fn back(&mut self, now: f64) -> Result<(), String> {
        if let Some(dialog) = &self.rename_sack {
            if dialog.can_cancel() {
                self.close_sack_rename();
            }
        } else if self.notice.take().is_some() {
            self.unforge_item = None;
        } else if self.dye.is_some() {
            self.cancel_dye(now);
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
        if self.skill_book_result_open() {
            self.apply_skill_book_result_event(&event);
            return Ok(());
        }
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
            }

            PanelEvent::Activate(target) => self.activate(target, snapshot, now, None)?,
            PanelEvent::SelectNpc(id) => self.select_npc(id, snapshot, now)?,
            PanelEvent::PointerDown(target, point, npc_id) => {
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
                self.press =
                    Some(Press { target, inventory_item_id, origin: point, dragging: false });
                self.pressed_npc_selector = npc_id;
                if matches!(target, Target::Item(_)) {
                    self.selected = Some(target);
                    self.selected_at = now;
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
                    }
                } else {
                    if self.hovered_npc_selector != npc_id {
                        self.hovered_npc_selector = npc_id;
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
                        let pointer = Vec2::new(
                            self.stage_origin.x + point.x * self.view_scale,
                            self.stage_origin.y + point.y * self.view_scale,
                        );
                        self.activate(press.target, snapshot, now, Some(pointer))?;
                    }
                }
            }
            PanelEvent::Cancel => {
                self.press = None;
                self.pressed_npc_selector = None;
            }
            PanelEvent::Wheel(delta) => self.scroll_selector(delta),
            PanelEvent::Focus(target) => {
                self.hovered_skill = if let Target::Skill(id) = target { Some(id) } else { None };
                self.hovered_npc_selector = None;
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
            Target::SkillBookResultOkay => {}
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
    pub fn update(
        &mut self,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        self.now = now;
        self.bursts.borrow_mut().retain(|(_, burst)| !burst.finished(now));
        // Service close callbacks read the latest accepted prop, including when
        // a roll receipt and Escape arrive in the same presentation frame.
        self.dowsing_rolled = snapshot.economy.dowsing_rolled;
        self.update_dye_state(snapshot, now);
        self.expire_sack_transition(now)?;
        if self.hotbar_controls.update(
            self.input_suspended
                || self.skill_book_result_open()
                || self.closing_at.is_some()
                || self.rename_sack.is_some()
                || self.inventory_transition_locked(),
        )? {
            self.bursts.borrow_mut().clear();
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
        }
        if self
            .dowsing_reference_item
            .is_some_and(|id| native_belt_owned_item(snapshot.economy, id).is_none())
        {
            self.dowsing_reference_item = None;
        }
        self.selector_height = self
            .dialogue
            .as_ref()
            .and_then(|dialogue| {
                if let HubNpcChatContent::Selector(selector) = dialogue.content {
                    let rows = solomon_sim::hub::interaction::hub_npc_selector_rows_with_mods(
                        selector,
                        &snapshot.economy.npc,
                        &snapshot.book.advanced_unlocks,
                        snapshot.mod_content,
                    );
                    Some(if rows.is_empty() {
                        0.0
                    } else {
                        50.0 + (rows.len() - 1) as f64 * 90.0 + 85.0
                    })
                } else {
                    None
                }
            })
            .unwrap_or(0.0);
        let events = std::mem::take(&mut self.events);
        if !self.input_suspended {
            for event in events {
                self.apply_event(event, snapshot, now)?;
            }
        }
        self.update_inventory_flybys(now)?;
        self.npc_notebox.update(now);
        if self.panel.is_none() {
            self.viewport_size = (width, height);
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
        self.alpha = progress;
        let sack_path = reconcile_inventory_sack_path(&snapshot.economy.backpack, &self.sack_path);
        if sack_path != self.sack_path {
            self.clear_inventory_flybys()?;
            self.clear_sack_transition()?;
            self.sack_path = sack_path;
        }
        self.model = Some(model::PanelModel::refresh(self.model.take(), snapshot));
        self.refresh_mod_icons(snapshot)?;
        self.update_sack_rename(snapshot, width, height)?;
        self.update_dialogue(now)?;
        Ok(())
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        for key in frame.key_codes() {
            if key.pressed && !self.input_suspended && self.rename_sack.is_none() {
                self.events.push(PanelEvent::Key(PanelKey {
                    code: key.code.clone(),
                    key: key.key.clone(),
                    repeat: key.repeat,
                    modified: key.alt || key.ctrl || key.meta,
                }));
            }
        }
        self.npc_notebox.paint(frame, &self.ui, self.now, self.viewport_size)?;
        let Some(panel) = self.panel else { return self.paint_skill_book_result(frame) };
        let Some(model) = self.model.take() else { return self.paint_skill_book_result(frame) };
        let snapshot = model.snapshot();
        let scope = if self.notice.is_some() {
            "panel-notice"
        } else if self.dye.is_some() {
            "fabric-dye"
        } else {
            match panel {
                PlayerPanel::Inventory => "inventory",
                PlayerPanel::Skills => "skills",
                PlayerPanel::Dialogue => "dialogue",
                PlayerPanel::SkillSelector(_) => "skill-selector",
                PlayerPanel::Service(_) => "service",
            }
        };
        let preferred = if matches!(self.notice, Some(PlayerPanelNotice::UnforgeConfirmation)) {
            Some("panel:unforge-confirm".to_owned())
        } else if self.notice.is_some() {
            Some("panel:notice-okay".to_owned())
        } else if self.dye.is_some() {
            Some("panel:dye-swatch:0".to_owned())
        } else {
            match panel {
                PlayerPanel::Skills => {
                    solomon_sim::player_panels::native_skill_book_page_placements(snapshot.book)
                        .first()
                        .and_then(|page| page.skills.first())
                        .map(|id| format!("panel:skill:{id}"))
                }
                PlayerPanel::Dialogue => Some(
                    if self
                        .dialogue
                        .as_ref()
                        .is_some_and(|d| d.content == HubNpcChatContent::Choices)
                    {
                        "panel:dialogue-choice:0"
                    } else {
                        "panel:dialogue-next"
                    }
                    .to_owned(),
                ),
                PlayerPanel::SkillSelector(target) => {
                    solomon_sim::hud::skill_selector::native_hud_skill_selector_options(
                        snapshot.book,
                        snapshot.concentrations,
                        target,
                    )
                    .first()
                    .map(|option| format!("panel:hud-select-skill:{}", option.skill_id))
                }
                _ => Some("panel:stats-down".to_owned()),
            }
        };
        if !self.input_suspended && self.rename_sack.is_none() && !self.skill_book_result_open() {
            frame.input_scope(scope);
            self.navigation.update(frame.ctx, scope, preferred.as_deref());
        }

        let hits = RefCell::new(Vec::new());
        let events = RefCell::new(Vec::new());
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("player-panels"));
        let transform = egui::emath::TSTransform::new(
            egui::vec2(self.stage_origin.x as f32, self.stage_origin.y as f32),
            self.view_scale as f32,
        );
        frame.ctx.set_transform_layer(layer, transform);
        let ctx = frame.ctx.clone();
        let mut error = None;
        egui::Area::new(layer.id)
            .order(layer.order)
            .fixed_pos(egui::Pos2::ZERO)
            .constrain(false)
            .show(&ctx, |ui| {
                ui.set_min_size(egui::vec2(1675.0, 900.0));
                let canvas = chrome::PanelCanvas::new(
                    frame,
                    ui,
                    &hits,
                    &events,
                    self.alpha as f32,
                    !self.input_suspended
                        && !self.skill_book_result_open()
                        && self.closing_at.is_none()
                        && self.notice.is_none()
                        && self.dye.is_none()
                        && self.rename_sack.is_none(),
                );
                let canvas =
                    if self.dye.is_some() { canvas.namespace("panel:background") } else { canvas };
                let result = (|| {
                    match panel {
                        PlayerPanel::Inventory => self.draw_inventory(&canvas, &snapshot)?,
                        PlayerPanel::Skills => self.draw_skills(&canvas, &snapshot)?,
                        PlayerPanel::SkillSelector(target) => {
                            self.draw_hud_skill_selector(&canvas, target, &snapshot)?
                        }
                        PlayerPanel::Dialogue => self.draw_dialogue(&canvas, &snapshot)?,
                        PlayerPanel::Service(trader) => {
                            self.draw_inventory(&canvas, &snapshot)?;
                            self.draw_service(&canvas, trader, &snapshot)?;
                        }
                    }
                    if !matches!(panel, PlayerPanel::Dialogue | PlayerPanel::SkillSelector(_)) {
                        self.draw_footer(&canvas, &snapshot)?;
                    }
                    self.paint_inventory_flybys(&canvas, &snapshot)?;
                    self.paint_dragger(&canvas, &snapshot)?;
                    if self.notice.is_none() && self.dye.is_none() && self.rename_sack.is_none() {
                        self.paint_tooltip(&canvas, &snapshot, self.now)?;
                        self.draw_sack_rename_open(&canvas, &snapshot)?;
                    }
                    Ok(())
                })();
                if let Err(value) = result {
                    error = Some(value);
                }
                if !matches!(panel, PlayerPanel::Dialogue | PlayerPanel::SkillSelector(_))
                    && self.notice.is_none()
                    && self.dye.is_none()
                    && self.rename_sack.is_none()
                {
                    let belt =
                        solomon_sim::hud::native_hud_control_layout(1600.0, 900.0, Some(1.0)).belt;
                    self.hotbar_controls.paint(frame, ui, &belt, false, egui::vec2(1600.0, 900.0));
                }
            });
        self.hits = hits.into_inner();
        self.events.extend(events.into_inner());
        self.collect_gpu_input(&ctx);
        if self.compact {
            self.paint_compact_navigation(frame)?;
        }
        if self.dye.is_some() {
            self.paint_dye_modal(frame, &snapshot)?;
        }
        if let Some(notice) = self.notice.clone() {
            self.paint_notice_modal(frame, &notice)?;
        }
        self.paint_sack_rename(frame)?;
        self.paint_skill_book_result(frame)?;
        self.model = Some(model);
        error.map_or(Ok(()), Err)
    }
    fn collect_gpu_input(&mut self, ctx: &egui::Context) {
        if self.input_suspended
            || self.rename_sack.is_some()
            || self.notice.is_some()
            || self.dye.is_some()
        {
            return;
        }
        for event in ctx.input(|input| input.events.clone()) {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } => {
                    let point = Vec2::new(
                        (pos.x as f64 - self.stage_origin.x) / self.view_scale,
                        (pos.y as f64 - self.stage_origin.y) / self.view_scale,
                    );
                    let hit = self.hit_at(point);
                    if pressed {
                        if let Some(hit) = hit {
                            self.events.push(PanelEvent::PointerDown(hit.target, point, hit.npc));
                        } else {
                            self.events.push(PanelEvent::DismissInspection);
                            if self.compact
                                && !matches!(
                                    self.panel,
                                    Some(PlayerPanel::Dialogue | PlayerPanel::SkillSelector(_))
                                )
                            {
                                self.pan_pointer = Some(pos);
                            }
                        }
                    } else {
                        self.pan_pointer = None;
                        self.events.push(PanelEvent::PointerUp(point, hit.map(|h| h.target)));
                    }
                }
                egui::Event::PointerMoved(pos) => {
                    if let Some(previous) = self.pan_pointer {
                        self.scroll.x += (previous.x - pos.x) as f64;
                        self.scroll.y += (previous.y - pos.y) as f64;
                        self.pan_pointer = Some(pos);
                        self.clamp_scroll();
                    } else {
                        let point = Vec2::new(
                            (pos.x as f64 - self.stage_origin.x) / self.view_scale,
                            (pos.y as f64 - self.stage_origin.y) / self.view_scale,
                        );
                        let hit = self.hit_at(point);
                        self.events.push(PanelEvent::PointerMove(
                            point,
                            hit.as_ref().map(|h| h.target),
                            hit.and_then(|h| h.npc),
                        ));
                    }
                }
                egui::Event::PointerGone | egui::Event::WindowFocused(false) => {
                    self.pan_pointer = None;
                    self.events.push(PanelEvent::Cancel);
                }
                egui::Event::MouseWheel { delta, .. } => {
                    if self.panel == Some(PlayerPanel::Dialogue) {
                        self.events.push(PanelEvent::Wheel(-delta.y as f64));
                    } else if self.compact
                        && !matches!(self.panel, Some(PlayerPanel::SkillSelector(_)))
                    {
                        self.scroll.x -= delta.x as f64;
                        self.scroll.y -= delta.y as f64;
                        self.clamp_scroll();
                    }
                }
                _ => {}
            }
        }
    }
    fn hit_at(&self, point: Vec2) -> Option<chrome::PanelHit> {
        self.hits
            .iter()
            .rev()
            .find(|hit| {
                hit.enabled && hit.rect.contains(egui::pos2(point.x as f32, point.y as f32))
            })
            .cloned()
    }
    fn show_modal_belt_pull_off(&self, slot: usize) -> Result<(), String> {
        self.bursts
            .borrow_mut()
            .push((slot, crate::ui::belt_pull_off::BeltPullOffBurst::new(self.now)));
        Ok(())
    }
}
fn target_from_action(action: &str) -> Option<Target> {
    Some(match action {
        "close" => Target::Close,
        "inventory" => Target::Inventory,
        "skills" => Target::Skills,
        "back" => Target::Back,
        "stats-up" => Target::StatsUp,
        "stats-down" => Target::StatsDown,
        "notice-okay" => Target::NoticeOkay,
        "skill-book-result-okay" => Target::SkillBookResultOkay,
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
