//! Stock College HUD in screen space. Geometry and meter rules live in `sim::hud`;
//! Website `GameHud.tsx`, `hub.css`, `SkillQuickbar.tsx` own the presentation reference.

mod belt_pull_off;
mod quickbar;
mod touch;

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::belt::{NATIVE_BELT_SLOT_COUNT, PLAYER_BELT_SLOT_COUNT};
use solomon_sim::hud::{
    HudBeltEntry, HudHealthLayerKind, HudState, clamp_unit, native_health_hud_layers,
    native_health_hud_presentation, native_hud_control_layout, native_mana_hud_presentation,
};
use solomon_sim::mobile_ui::{MobileUiElementId, MobileUiLayoutState};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, Event, HtmlElement};

use crate::hotbar::{HotbarControls, HotbarSelection};
use crate::mobile_layout::apply_mobile_ui_element;
use crate::ui::{StockAtlas, append, element, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudAction {
    Pause,
    Inventory,
    Skills,
    Map,
    Shortcut(usize),
    Belt(usize),
    SkillBinding(usize),
}

/// Website GameHud's `mode`: College shortcuts/help/map are absent from a live run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudMode {
    Hub,
    Run,
}

/// Presentation of canonical Tutorial access; the session still admits every action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudAccess {
    pub combat: bool,
    pub inventory: bool,
    pub quickbar: bool,
    pub skills: bool,
    pub spell: bool,
}

impl HudAccess {
    const UNRESTRICTED: Self =
        Self { combat: true, inventory: true, quickbar: true, skills: true, spell: true };
}

pub struct Hud {
    document: Document,
    root: HtmlElement,
    diagnostics: HtmlElement,
    fps: HtmlElement,
    ping: HtmlElement,
    frame_intervals: u64,
    frame_sample_started_at: Option<f64>,
    meters: HtmlElement,
    selected_skills: HtmlElement,
    belt: HtmlElement,
    hotbar: HotbarSelection,
    hotbar_controls: HotbarControls,
    xp_fill: HtmlElement,
    backpack: HtmlElement,
    tome: HtmlElement,
    ui: StockAtlas,
    skills: StockAtlas,
    inventory: StockAtlas,
    actions: Rc<RefCell<Vec<HudAction>>>,
    _click: Closure<dyn FnMut(Event)>,
    _burst_finished: Closure<dyn FnMut(Event)>,
    controls: Vec<HtmlElement>,
    previous: Option<HudState>,
    viewport: (i32, i32),
    actions_enabled: bool,
    hotbar_enabled: bool,
    ui_scale: f64,
    mode: HudMode,
    controller_belt_slot: Option<usize>,
    tutorial_access: HudAccess,
    touch_layout: Option<touch::TouchBeltLayout>,
    mobile_layout: MobileUiLayoutState,
    mod_belt_icons: [Option<Vec<crate::ui::mod_icon::ModIconLayer>>; 8],
    quickbar: [Option<quickbar::QuickbarSlot>; 8],
}

impl Hud {
    pub async fn attach(document: &Document, hotbar: HotbarSelection) -> Result<Self, String> {
        let ui = StockAtlas::load("UI").await?;
        let skills = StockAtlas::load("Skills").await?;
        let inventory = StockAtlas::load("Inventory").await?;
        let college = StockAtlas::load("College").await?;
        let portraits = StockAtlas::load("LevelPicker").await?;
        let root = element(document, "div", "game-hud")?;
        root.set_attribute("data-mode", "hub").map_err(|_| "HUD mode")?;
        root.set_attribute("aria-label", "Player status").map_err(|_| "HUD label")?;
        let diagnostics = element(document, "div", "hud-diagnostics")?;
        diagnostics
            .set_attribute("aria-label", "Performance")
            .map_err(|_| "HUD diagnostics label")?;
        let fps = element(document, "span", "hud-fps")?;
        fps.set_text_content(Some("-- FPS"));
        fps.set_attribute("aria-label", "Measuring frames per second")
            .map_err(|_| "HUD FPS label")?;
        let ping = element(document, "span", "hud-ping")?;
        ping.set_text_content(Some("-- ms"));
        ping.set_attribute("aria-label", "Measuring network ping").map_err(|_| "HUD ping label")?;
        append(&diagnostics, &fps)?;
        append(&diagnostics, &ping)?;
        append(&root, &diagnostics)?;
        let meters = element(document, "div", "hud-meters")?;
        let selected_skills = element(document, "div", "hud-selected-skills")?;
        let belt = element(document, "div", "hud-belt")?;
        belt.set_attribute("aria-label", "Item belt").map_err(|_| "belt label")?;
        let hotbar_controls = HotbarControls::new(document, hotbar.clone(), None)?;
        let xp = element(document, "div", "hud-xp")?;
        xp.set_attribute("role", "progressbar").map_err(|_| "experience role")?;
        xp.set_attribute("aria-label", "Experience").map_err(|_| "experience label")?;
        xp.set_attribute("aria-valuemin", "0").map_err(|_| "experience minimum")?;
        xp.set_attribute("aria-valuemax", "100").map_err(|_| "experience maximum")?;
        let xp_fill = ui.sprite(document, 81, "hud-xp-fill")?;
        append(&xp, &xp_fill)?;
        append(&xp, &ui.sprite(document, 82, "hud-xp-frame")?)?;
        for layer in [&meters, &selected_skills, &belt, &xp] {
            append(&root, layer)?;
        }
        let backpack = element(document, "button", "hud-backpack hud-button")?;
        let tome = element(document, "button", "hud-tome hud-button")?;
        backpack
            .set_attribute("data-tutorial-anchor", "inventory")
            .map_err(|_| "Tutorial inventory anchor")?;
        tome.set_attribute("data-tutorial-anchor", "skills")
            .map_err(|_| "Tutorial skills anchor")?;
        append(&backpack, &ui.sprite(document, 47, "hud-control-art")?)?;
        append(&tome, &ui.sprite(document, 48, "hud-control-art")?)?;
        let actions = Rc::new(RefCell::new(Vec::new()));
        let clicked = actions.clone();
        // Delegate from one stable root: updating a meter or belt does not retain closures
        // for detached controls, and all buttons share the same native click semantics.
        let click =
            Closure::<dyn FnMut(Event)>::new(move |event: Event| match clicked_action(&event) {
                Ok(Some(action)) => {
                    event.prevent_default();
                    clicked.borrow_mut().push(action);
                }
                Ok(None) => {}
                Err(error) => web_sys::console::error_1(&JsValue::from_str(&error)),
            });
        root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "HUD click listener")?;
        let burst_finished = crate::ui::belt_pull_off::listen_to_burst_completion(&root)?;
        let mut hud = Self {
            document: document.clone(),
            root,
            diagnostics,
            fps,
            ping,
            frame_intervals: 0,
            frame_sample_started_at: None,
            meters,
            selected_skills,
            belt,
            hotbar,
            hotbar_controls,
            xp_fill,
            backpack,
            tome,
            ui,
            skills,
            inventory,
            actions,
            _click: click,
            _burst_finished: burst_finished,
            controls: Vec::new(),
            previous: None,
            mod_belt_icons: std::array::from_fn(|_| None),
            quickbar: std::array::from_fn(|_| None),
            viewport: (0, 0),
            actions_enabled: false,
            hotbar_enabled: false,
            ui_scale: 1.0,
            mode: HudMode::Hub,
            controller_belt_slot: None,
            tutorial_access: HudAccess::UNRESTRICTED,
            touch_layout: None,
            mobile_layout: MobileUiLayoutState::default(),
        };
        let skull = element(document, "button", "hud-menu hud-button")?;
        for (element, id) in [
            (&skull, MobileUiElementId::Pause),
            (&hud.diagnostics, MobileUiElementId::Diagnostics),
            (&hud.meters, MobileUiElementId::Meters),
            (&hud.backpack, MobileUiElementId::Inventory),
            (&hud.tome, MobileUiElementId::Skillbook),
            (&xp, MobileUiElementId::Xp),
        ] {
            apply_mobile_ui_element(element, &hud.mobile_layout, id)?;
        }
        append(&skull, &hud.ui.sprite(document, 42, "hud-menu-art")?)?;
        hud.action_button(&skull, "Menu (Esc)", HudAction::Pause)?;
        let backpack = hud.backpack.clone();
        let tome = hud.tome.clone();
        hud.action_button(&backpack, "Inventory (I)", HudAction::Inventory)?;
        hud.action_button(&tome, "Skills (K)", HudAction::Skills)?;
        let help = element(document, "span", "hud-help")?;
        append(&help, &college.sprite(document, 37, "hud-help-art")?)?;
        help.set_attribute("role", "img").map_err(|_| "Help decoration role")?;
        help.set_attribute("aria-label", "Help").map_err(|_| "Help decoration label")?;
        append(&hud.root, &help)?;
        for (index, (record, name)) in
            [(0, "Provokatus"), (6, "Hagatha"), (4, "Luthacus"), (5, "Fomentius"), (2, "Shlorio")]
                .into_iter()
                .enumerate()
        {
            let button = element(document, "button", "hud-shortcut hud-button")?;
            style(&button, &format!("top:{}px", 55 + index * 95))?;
            append(&button, &portraits.sprite(document, record, "hud-portrait")?)?;
            hud.action_button(&button, name, HudAction::Shortcut(index))?;
        }
        let map = element(document, "button", "hud-map hud-button")?;
        append(&map, &college.sprite(document, 16, "hud-map-art")?)?;
        append(&map, &college.sprite(document, 18, "hud-map-art")?)?;
        hud.action_button(&map, "Enter the Boneyard", HudAction::Map)?;
        let body = document.body().ok_or("page has no body")?;
        append(&body, &hud.root)?;
        Ok(hud)
    }

    /// Enable only once the caller owns every emitted action; idle buttons still paint art.
    pub fn set_actions_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.actions_enabled = enabled;
        for control in &self.controls {
            control
                .toggle_attribute_with_force("disabled", !enabled)
                .map_err(|_| "HUD control availability")?;
        }
        self.previous = None;
        Ok(())
    }

    pub fn take_actions(&self) -> Vec<HudAction> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }

    /// The source hotbar admission is narrower than the other visible HUD
    /// controls. Game projects the same predicate used by the capture shortcut.
    pub fn set_hotbar_enabled(&mut self, enabled: bool) {
        self.hotbar_enabled = enabled;
    }

    pub fn set_tutorial_access(&mut self, access: Option<HudAccess>) -> Result<(), String> {
        let access = access.unwrap_or(HudAccess::UNRESTRICTED);
        if self.tutorial_access == access {
            return Ok(());
        }
        for (name, visible) in [
            ("combat", access.combat),
            ("inventory", access.inventory),
            ("quickbar", access.quickbar),
            ("skills", access.skills),
            ("spell", access.spell),
        ] {
            self.root
                .set_attribute(
                    &format!("data-tutorial-{name}"),
                    if visible { "true" } else { "false" },
                )
                .map_err(|_| "Tutorial HUD access")?;
        }
        self.tutorial_access = access;
        Ok(())
    }

    pub fn set_mode(&mut self, mode: HudMode) -> Result<(), String> {
        if self.mode == mode {
            return Ok(());
        }
        self.mode = mode;
        self.root
            .set_attribute(
                "data-mode",
                match mode {
                    HudMode::Hub => "hub",
                    HudMode::Run => "run",
                },
            )
            .map_err(|_| "HUD mode".into())
    }

    /// Source SkillQuickbar keeps a visible outline even when the controller selects
    /// an empty slot. This is view context supplied by the canonical input sample.
    pub fn set_controller_belt_slot(&mut self, slot: Option<usize>) {
        assert!(
            slot.is_none_or(|slot| slot < 8),
            "controller belt selection is within eight slots"
        );
        if self.controller_belt_slot != slot {
            self.controller_belt_slot = slot;
            self.previous = None;
        }
    }

    /// Static shortcuts can become available as their owning surfaces are connected.
    pub fn set_action_enabled(&self, action: HudAction, enabled: bool) -> Result<(), String> {
        let id = action_id(action);
        for control in &self.controls {
            if control.get_attribute("data-hud-action").as_deref() == Some(&id) {
                control
                    .toggle_attribute_with_force("disabled", !enabled)
                    .map_err(|_| "HUD shortcut availability")?;
            }
        }
        Ok(())
    }

    pub fn set_visible(&self, visible: bool) {
        self.root.set_hidden(!visible);
    }

    pub fn set_ui_scale(&mut self, scale: f64) {
        self.ui_scale = scale;
        self.viewport = (0, 0);
    }

    /// Call once per actual presentation frame, using the rAF timestamp and the
    /// connection's measured ping. `GameHud.FpsCounter` samples one-second intervals;
    /// neither accepted snapshots nor simulation ticks are rendering frame receipts.
    pub fn update_diagnostics(&mut self, now_ms: f64, ping_ms: Option<f64>) -> Result<(), String> {
        if let Some(started_at) = self.frame_sample_started_at {
            self.frame_intervals += 1;
            let elapsed = now_ms - started_at;
            if elapsed >= 1000.0 {
                let fps = (self.frame_intervals as f64 * 1000.0 / elapsed + 0.5).floor();
                self.fps.set_text_content(Some(&format!("{fps} FPS")));
                self.fps
                    .set_attribute("aria-label", &format!("{fps} frames per second"))
                    .map_err(|_| "HUD measured FPS")?;
                self.frame_intervals = 0;
                self.frame_sample_started_at = Some(now_ms);
            }
        } else {
            self.frame_sample_started_at = Some(now_ms);
        }
        let (text, label) = ping_ms.map_or_else(
            || ("-- ms".to_owned(), "Measuring network ping".to_owned()),
            |ping| (format!("{ping} ms"), format!("{ping} milliseconds ping")),
        );
        self.ping.set_text_content(Some(&text));
        self.ping.set_attribute("aria-label", &label).map_err(|_| "HUD measured ping".to_owned())
    }

    pub fn set_mobile_layout(&mut self, state: &MobileUiLayoutState) -> Result<(), String> {
        if self.mobile_layout == *state {
            return Ok(());
        }
        self.mobile_layout = state.clone();
        let nodes = self
            .root
            .query_selector_all("[data-mobile-ui-element]")
            .map_err(|_| "mobile HUD controls")?;
        for index in 0..nodes.length() {
            let Some(node) = nodes.item(index) else { continue };
            let element = node.dyn_into::<HtmlElement>().map_err(|_| "mobile HUD element")?;
            let id = element
                .get_attribute("data-mobile-ui-element")
                .and_then(|id| MobileUiElementId::from_id(&id))
                .ok_or("mobile HUD element identity")?;
            apply_mobile_ui_element(&element, state, id)?;
        }
        self.previous = None;
        Ok(())
    }

    /// Borrow the real belt/economy and verified image owner. This retains only
    /// resolved icon layers, not another inventory or binding store.
    pub fn set_mod_belt_icons(
        &mut self,
        player: Option<(
            &solomon_sim::belt::PlayerBeltComponent,
            &solomon_sim::economy::HubEconomyState,
        )>,
        images: Option<&crate::mod_presentation_assets::ModPresentationAssets>,
    ) -> Result<(), String> {
        let mut icons = std::array::from_fn(|_| None);
        if let Some((belt, economy)) = player {
            for (slot, entry) in belt
                .entries
                .iter()
                .skip(self.hotbar.start())
                .take(NATIVE_BELT_SLOT_COUNT)
                .enumerate()
            {
                if let Some(item) = entry
                    .and_then(|entry| solomon_sim::belt::native_belt_entry_item(entry, economy))
                {
                    icons[slot] = crate::ui::mod_icon::mod_item_icon_layers(images, item)?;
                }
            }
        }
        if self.mod_belt_icons != icons {
            self.mod_belt_icons = icons;
            self.previous = None;
        }
        Ok(())
    }

    pub fn update(&mut self, state: &HudState, width: i32, height: i32) -> Result<(), String> {
        if self.viewport != (width, height) {
            let scale =
                (f64::from(width) / 1600.0).min(f64::from(height) / 900.0).min(1.0) * self.ui_scale;
            let logical_width = f64::from(width) / scale;
            let logical_height = f64::from(height) / scale;
            style(
                &self.root,
                &format!(
                    "width:{logical_width}px;height:{logical_height}px;transform:scale({scale});--hud-scale:{scale}"
                ),
            )?;
            self.root
                .set_attribute("data-hud-input-scale", &scale.to_string())
                .map_err(|_| "HUD pointer scale")?;
            let layout = native_hud_control_layout(logical_width, logical_height, None);
            for (element, rect, id) in [
                (&self.backpack, layout.backpack, MobileUiElementId::Inventory),
                (&self.tome, layout.tome, MobileUiElementId::Skillbook),
            ] {
                style(element, &format!("left:{}px;top:{}px", rect.x, rect.y))?;
                apply_mobile_ui_element(element, &self.mobile_layout, id)?;
            }
            self.viewport = (width, height);
        }
        self.update_touch_layout(width, height)?;
        if self.hotbar_controls.update(!self.actions_enabled || !self.hotbar_enabled)? {
            crate::ui::belt_pull_off::clear_belt_pull_off_bursts(&self.root)?;
            self.previous = None;
        }
        if self.previous.as_ref() == Some(state) {
            return Ok(());
        }
        self.draw_meters(state)?;
        self.draw_selected_skills(state)?;
        self.draw_belt(state)?;
        self.backpack
            .set_attribute("aria-label", &format!("Inventory, {} gold (I)", state.gold))
            .map_err(|_| "inventory label")?;
        self.xp_fill
            .style()
            .set_property(
                "clip-path",
                &format!("inset({}% 0 0)", (1.0 - clamp_unit(state.experience_progress)) * 100.0),
            )
            .map_err(|_| "experience fill")?;
        if let Some(xp) = self.xp_fill.parent_element() {
            xp.set_attribute(
                "aria-valuenow",
                &(clamp_unit(state.experience_progress) * 100.0).to_string(),
            )
            .map_err(|_| "experience value")?;
        }
        self.previous = Some(state.clone());
        Ok(())
    }

    fn action_button(
        &mut self,
        button: &HtmlElement,
        label: &str,
        action: HudAction,
    ) -> Result<(), String> {
        button.set_attribute("type", "button").map_err(|_| "HUD button type")?;
        button.set_attribute("aria-label", label).map_err(|_| "HUD button label")?;
        button.set_attribute("title", label).map_err(|_| "HUD button title")?;
        button
            .toggle_attribute_with_force("disabled", !self.actions_enabled)
            .map_err(|_| "HUD button availability")?;
        button.set_attribute("data-hud-action", &action_id(action)).map_err(|_| "HUD action")?;
        self.controls.push(button.clone());
        append(&self.root, button)
    }

    fn draw_meters(&self, state: &HudState) -> Result<(), String> {
        self.meters.set_text_content(None);
        let health = native_health_hud_presentation(
            state.current_health,
            state.maximum_health,
            state.shield_current,
            state.shield_maximum,
        );
        let mana = native_mana_hud_presentation(
            state.current_mana,
            state.maximum_mana,
            state.reserved_mana,
        );
        for (class, track, core, current, maximum) in [
            (
                "hud-health",
                health.track_width,
                health.core_width,
                state.current_health,
                state.maximum_health,
            ),
            ("hud-mana", mana.track_width, mana.core_width, state.current_mana, state.maximum_mana),
        ] {
            let meter = element(&self.document, "div", &format!("hud-meter {class}"))?;
            if class == "hud-health" {
                meter
                    .set_attribute("data-tutorial-anchor", "health-meter")
                    .map_err(|_| "Tutorial health anchor")?;
            }
            meter.set_attribute("role", "meter").map_err(|_| "meter role")?;
            meter
                .set_attribute("aria-label", if class == "hud-health" { "Health" } else { "Mana" })
                .map_err(|_| "meter label")?;
            for (name, value) in [
                ("aria-valuemin", 0.0),
                ("aria-valuemax", maximum),
                ("aria-valuenow", current.max(0.0)),
            ] {
                meter.set_attribute(name, &value.to_string()).map_err(|_| "meter value")?;
            }
            style(&meter, &format!("width:{track}px"))?;
            append(&meter, &self.ui.strip(&self.document, 70, track, "hud-meter-frame")?)?;
            if class == "hud-health" {
                for layer in native_health_hud_layers(
                    health.fill_progress,
                    health.shield_progress,
                    state.poisoned,
                ) {
                    let fill =
                        self.ui.strip(&self.document, layer.record, core, "hud-meter-fill")?;
                    fill.style()
                        .set_property(
                            "clip-path",
                            &format!("inset(0 {}% 0 0)", (1.0 - layer.progress) * 100.0),
                        )
                        .map_err(|_| "health clipping")?;
                    if layer.kind == HudHealthLayerKind::Shield {
                        fill.style()
                            .set_property("filter", "url(#hud-shield-tint)")
                            .map_err(|_| "shield tint")?;
                    }
                    append(&meter, &fill)?;
                }
            } else {
                let fill = self.ui.strip(&self.document, 40, core, "hud-meter-fill")?;
                fill.style()
                    .set_property(
                        "clip-path",
                        &format!("inset(0 {}% 0 0)", (1.0 - mana.fill_progress) * 100.0),
                    )
                    .map_err(|_| "mana clipping")?;
                append(&meter, &fill)?;
                if mana.reserve_progress > 0.0 {
                    append(
                        &meter,
                        &self.ui.strip(
                            &self.document,
                            41,
                            mana.reserve_width,
                            "hud-mana-reserve",
                        )?,
                    )?;
                }
            }
            append(&self.meters, &meter)?;
        }
        Ok(())
    }

    fn draw_selected_skills(&self, state: &HudState) -> Result<(), String> {
        self.selected_skills.set_text_content(None);
        let records = [
            (Some(state.primary_icon_record), 12),
            (state.concentration_icon_records[1], 20),
            (state.concentration_icon_records[0], 16),
        ];
        let count = records.iter().filter(|(record, _)| record.is_some()).count();
        let mut index = 0;
        for (record, binding) in records {
            let Some(record) = record else { continue };
            let offset = -20.0 * (count as f64 - 1.0) + index as f64 * 40.0;
            let sprite = self.skills.sprite(&self.document, record, "hud-selected-skill")?;
            sprite
                .style()
                .set_property("left", &format!("calc(50% + {offset}px)"))
                .map_err(|_| "selected skill position")?;
            append(&self.selected_skills, &sprite)?;
            let action = element(&self.document, "button", "hud-skill-action hud-button")?;
            action.set_attribute("type", "button").map_err(|_| "skill action type")?;
            action
                .set_attribute(
                    "aria-label",
                    if binding == 12 { "Select primary attack" } else { "Select concentration" },
                )
                .map_err(|_| "skill action label")?;
            action
                .set_attribute("data-hud-action", &action_id(HudAction::SkillBinding(binding)))
                .map_err(|_| "skill action")?;
            if binding == 12 || binding == 16 {
                action
                    .set_attribute(
                        "data-tutorial-anchor",
                        if binding == 12 { "primary-skill" } else { "concentration-a" },
                    )
                    .map_err(|_| "Tutorial selected skill anchor")?;
            }
            action
                .toggle_attribute_with_force("disabled", !self.actions_enabled)
                .map_err(|_| "skill availability")?;
            style(&action, &format!("left:calc(50% + {offset}px - 20px)"))?;
            append(&self.selected_skills, &action)?;
            index += 1;
        }
        Ok(())
    }

    fn draw_belt(&self, state: &HudState) -> Result<(), String> {
        self.belt.set_text_content(None);
        self.belt
            .set_attribute("data-hotbar-bank", &self.hotbar.bank().to_string())
            .map_err(|_| "HUD belt bank")?;
        let health_potion = state
            .belt
            .iter()
            .position(|entry| matches!(entry, Some(HudBeltEntry::HealthPotion { .. })));
        let mana_potion = state
            .belt
            .iter()
            .position(|entry| matches!(entry, Some(HudBeltEntry::ManaPotion { .. })));
        // Empty entries paint no art; a controller selection still needs its source outline.
        for (slot, entry) in state.belt.iter().enumerate() {
            if entry.is_none() && self.controller_belt_slot != Some(slot) {
                continue;
            }
            let button = element(&self.document, "button", "hud-belt-slot hud-button")?;
            button.set_attribute("type", "button").map_err(|_| "belt button type")?;
            button.set_attribute("data-slot", &slot.to_string()).map_err(|_| "belt slot")?;
            button
                .set_attribute("data-belt-index", &self.hotbar.slot(slot).to_string())
                .map_err(|_| "belt authority index")?;
            self.place_touch_belt_slot(&button, slot)?;
            if health_potion == Some(slot) || mana_potion == Some(slot) {
                button
                    .set_attribute(
                        "data-touch-potion",
                        if health_potion == Some(slot) { "health" } else { "mana" },
                    )
                    .map_err(|_| "touch potion alias")?;
            }
            let mobile_id = if health_potion == Some(slot) {
                MobileUiElementId::HealthPotion
            } else if mana_potion == Some(slot) {
                MobileUiElementId::ManaPotion
            } else {
                MobileUiElementId::SLOTS[slot]
            };
            apply_mobile_ui_element(&button, &self.mobile_layout, mobile_id)?;
            if slot == 0 || health_potion == Some(slot) {
                button
                    .set_attribute(
                        "data-tutorial-anchor",
                        if slot == 0 { "secondary-slot" } else { "health-potion" },
                    )
                    .map_err(|_| "Tutorial potion anchor")?;
            }
            button
                .set_attribute(
                    "data-controller-selected",
                    if self.controller_belt_slot == Some(slot) { "true" } else { "false" },
                )
                .map_err(|_| "controller belt outline")?;
            let offset = [-332, -272, -212, -152, 98, 158, 218, 278][slot];
            button
                .style()
                .set_property("--belt-offset", &format!("{offset}px"))
                .map_err(|_| "belt position")?;
            let Some(entry) = entry else {
                button
                    .set_attribute(
                        "aria-label",
                        &format!("Empty belt slot {}, controller selected", slot + 1),
                    )
                    .map_err(|_| "empty belt label")?;
                button.set_attribute("disabled", "").map_err(|_| "empty belt availability")?;
                append(&self.belt, &button)?;
                continue;
            };
            let (sprite, count, label, unavailable) = match *entry {
                HudBeltEntry::Skill { record, unavailable } => (
                    self.skills.sprite(&self.document, record, "hud-belt-skill")?,
                    0,
                    "Secondary skill",
                    unavailable,
                ),
                HudBeltEntry::HealthPotion { count } => (
                    self.inventory.sprite(&self.document, 46, "hud-belt-item")?,
                    count,
                    "Health Potion",
                    count == 0,
                ),
                HudBeltEntry::ManaPotion { count } => (
                    self.inventory.sprite(&self.document, 47, "hud-belt-item")?,
                    count,
                    "Mana Potion",
                    count == 0,
                ),
                HudBeltEntry::Item { record, count } => {
                    let sprite = if let Some(layers) = &self.mod_belt_icons[slot] {
                        let icon = crate::ui::mod_icon::paint_mod_item_icon_layers(
                            &self.document,
                            layers,
                            solomon_sim::Vec2::ZERO,
                        )?;
                        icon.style()
                            .set_property("left", "50%")
                            .map_err(|_| "mod belt icon horizontal anchor")?;
                        icon.style()
                            .set_property("top", "50%")
                            .map_err(|_| "mod belt icon vertical anchor")?;
                        icon
                    } else if let Some(record) = record {
                        self.inventory.sprite(&self.document, record, "hud-belt-item")?
                    } else {
                        element(&self.document, "span", "hud-belt-item")?
                    };
                    (sprite, count, "Item", false)
                }
            };
            let row = self.quickbar[slot].as_ref();
            let combat_disabled = row.is_some_and(|row| row.combat_disabled);
            button
                .set_attribute(
                    "data-belt-cast-eligible",
                    if combat_disabled { "false" } else { "true" },
                )
                .map_err(|_| "belt cast admission")?;
            if combat_disabled {
                button
                    .set_attribute("aria-disabled", "true")
                    .map_err(|_| "Hub belt cast disabled")?;
            }
            let unavailable =
                row.map_or(unavailable, |row| row.combat_disabled || row.insufficient_mana);
            let count = row.and_then(|row| row.quantity).unwrap_or(u64::from(count));
            if unavailable {
                button
                    .set_attribute("data-unavailable", "true")
                    .map_err(|_| "belt availability")?;
            }
            let label = if let Some(row) = row {
                format!(
                    "{}{}",
                    row.label,
                    if self.controller_belt_slot == Some(slot) {
                        ", controller selected"
                    } else {
                        ""
                    }
                )
            } else if matches!(entry, HudBeltEntry::Skill { .. }) {
                format!("{label}, belt slot {}", slot + 1)
            } else {
                format!("{label}, belt slot {}, {count} available", slot + 1)
            };
            button.set_attribute("aria-label", &label).map_err(|_| "belt label")?;
            button
                .toggle_attribute_with_force("disabled", !self.actions_enabled)
                .map_err(|_| "belt button availability")?;
            self.decorate_quickbar_slot(&button, &sprite, slot)?;
            append(&button, &sprite)?;
            if count > 1 {
                let quantity = element(&self.document, "span", "hud-belt-count")?;
                quantity.set_text_content(Some(&count.to_string()));
                append(&button, &quantity)?;
            }
            self.draw_quickbar_binding(&button, slot)?;
            button
                .set_attribute(
                    "data-hud-action",
                    &action_id(HudAction::Belt(self.hotbar.slot(slot))),
                )
                .map_err(|_| "belt action")?;
            append(&self.belt, &button)?;
        }
        append(&self.belt, &self.hotbar_controls.root)?;
        Ok(())
    }
}

impl Drop for Hud {
    fn drop(&mut self) {
        for (name, listener) in [("click", &self._click), ("animationend", &self._burst_finished)] {
            if let Err(error) = self
                .root
                .remove_event_listener_with_callback(name, listener.as_ref().unchecked_ref())
            {
                web_sys::console::error_1(&error);
            }
        }
    }
}

fn action_id(action: HudAction) -> String {
    match action {
        HudAction::Pause => "pause".to_string(),
        HudAction::Inventory => "inventory".to_string(),
        HudAction::Skills => "skills".to_string(),
        HudAction::Map => "map".to_string(),
        HudAction::Shortcut(index) => format!("shortcut:{index}"),
        HudAction::Belt(index) => format!("belt:{index}"),
        HudAction::SkillBinding(binding) => format!("skill:{binding}"),
    }
}

fn clicked_action(event: &Event) -> Result<Option<HudAction>, String> {
    let Some(target) = event.target() else { return Ok(None) };
    let Ok(target) = target.dyn_into::<Element>() else { return Ok(None) };
    let Some(button) = target.closest("[data-hud-action]").map_err(|_| "HUD action selector")?
    else {
        return Ok(None);
    };
    if button.has_attribute("disabled") {
        return Ok(None);
    }
    let Some(id) = button.get_attribute("data-hud-action") else { return Ok(None) };
    let action = match id.as_str() {
        "pause" => HudAction::Pause,
        "inventory" => HudAction::Inventory,
        "skills" => HudAction::Skills,
        "map" => HudAction::Map,
        _ => {
            let Some((kind, index)) = id.split_once(':') else {
                return Err("unknown HUD action".to_string());
            };
            let index = index.parse::<usize>().map_err(|_| "invalid HUD action index")?;
            match kind {
                "shortcut" if index < 5 => HudAction::Shortcut(index),
                "belt" if index < PLAYER_BELT_SLOT_COUNT => HudAction::Belt(index),
                "skill" if [12, 16, 20].contains(&index) => HudAction::SkillBinding(index),
                _ => return Err("unknown HUD action".to_string()),
            }
        }
    };
    Ok(Some(action))
}
