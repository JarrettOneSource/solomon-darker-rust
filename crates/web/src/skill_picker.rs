//! Native level-up selection. The session supplies its real offer and accepts typed
//! sequence/id commands; this view retains only the displayed offer during transitions.

use solomon_sim::float;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use solomon_sim::audio::{AudioCue, AudioEvent, AudioOptions};
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{NativeUiFontName as Font, NativeUiTextSpec as TextSpec};
use solomon_sim::player_panels::skill_hover::{
    native_skill_hover_box_layout, skill_picker_detail_lines,
};
use solomon_sim::player_panels::skill_text::{
    native_skill_page_text_height, native_skill_page_wrapped_lines,
};
use solomon_sim::profile::{PlayerSkillChoice, PlayerSkillOffer};
use solomon_sim::skill_picker::{
    skill_picker_card, skill_picker_centers, skill_picker_close, skill_picker_panel,
    skill_picker_reveal,
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, HtmlElement, KeyboardEvent, PointerEvent};

use crate::menu::navigation::MenuNavigation;
use crate::player_panels::chrome::{art, bitmap_text, button, place, tint};
use crate::ui::{StockAtlas, append, element, native_button, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillPickerAction {
    Choose(PlayerSkillChoice),
    Reroll { offer_sequence: u64 },
    Save { offer_sequence: u64 },
}

#[derive(Clone, Copy)]
enum PickerTarget {
    Choice(usize),
    Detail(usize),
    Reroll,
    Save,
    CloseDetail,
}
enum PickerEvent {
    Activate(PickerTarget, bool, u64),
    Hover(Option<usize>),
    Select(usize),
    Key(String),
}

pub struct SkillPicker {
    document: Document,
    root: HtmlElement,
    curtain: HtmlElement,
    stage: HtmlElement,
    ambient: HtmlElement,
    content: HtmlElement,
    detail: HtmlElement,
    ui: StockAtlas,
    skills: StockAtlas,
    displayed: Option<PlayerSkillOffer>,
    special_available: bool,
    selected: usize,
    detail_index: Option<usize>,
    started_at: f64,
    closing: Option<(f64, f64)>,
    waiting_reroll: bool,
    had_interactive: bool,
    actions: Vec<SkillPickerAction>,
    audio_events: Vec<AudioEvent>,
    events: Rc<RefCell<Vec<PickerEvent>>>,
    navigation: MenuNavigation,
    listeners: Vec<Closure<dyn FnMut(Event)>>,
    compact: bool,
    compact_detail: bool,
    viewport: (i32, i32),
    detail_rendered: Cell<Option<(u64, usize, bool)>>,
}

impl SkillPicker {
    pub async fn attach(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "rust-skill-picker")?;
        root.set_hidden(true);
        root.set_attribute("role", "dialog").map_err(|_| "skill picker role")?;
        root.set_attribute("aria-modal", "true").map_err(|_| "skill picker modal")?;
        root.set_attribute("aria-label", "Choose a skill").map_err(|_| "skill picker label")?;
        let curtain = element(document, "div", "picker-curtain")?;
        let stage = element(document, "div", "player-panel-stage picker-stage")?;
        let ambient = element(document, "div", "picker-ambient")?;
        let content = element(document, "div", "picker-content")?;
        let detail = element(document, "div", "player-panel-tooltip picker-detail")?;
        detail.set_attribute("role", "tooltip").map_err(|_| "picker tooltip")?;
        append(&stage, &ambient)?;
        append(&stage, &content)?;
        append(&stage, &detail)?;
        append(&root, &curtain)?;
        append(&root, &stage)?;
        append(&document.body().ok_or("skill picker body")?, &root)?;
        let mut picker = Self {
            document: document.clone(),
            root,
            curtain,
            stage,
            ambient,
            content,
            detail,
            ui: StockAtlas::load("UI").await?,
            skills: StockAtlas::load("Skills").await?,
            displayed: None,
            special_available: false,
            selected: 0,
            detail_index: None,
            started_at: 0.0,
            closing: None,
            waiting_reroll: false,
            had_interactive: false,
            actions: Vec::new(),
            audio_events: Vec::new(),
            events: Rc::new(RefCell::new(Vec::new())),
            navigation: MenuNavigation::new(),
            listeners: Vec::new(),
            compact: false,
            compact_detail: false,
            viewport: (0, 0),
            detail_rendered: Cell::new(None),
        };
        picker.attach_events()?;
        Ok(picker)
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.displayed.is_some()
    }

    pub fn take_actions(&mut self) -> Vec<SkillPickerAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn take_audio_events(&mut self) -> Vec<AudioEvent> {
        std::mem::take(&mut self.audio_events)
    }
    fn sound(&mut self, cue: AudioCue, playback_rate: f64) {
        self.audio_events
            .push(AudioEvent::Play(cue, AudioOptions { playback_rate, ..AudioOptions::default() }));
    }

    /// An admission rejection keeps the current authoritative offer usable.
    pub fn reject_action(&mut self, now: f64) {
        self.closing = None;
        self.waiting_reroll = false;
        self.started_at = now - 400.0;
    }

    pub fn update(
        &mut self,
        offer: Option<&PlayerSkillOffer>,
        special_available: bool,
        now: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        let compact = width < 600;
        let compact_detail = width < 1000;
        if self.viewport != (width, height) {
            self.viewport = (width, height);
            let redraw = self.compact != compact;
            self.compact = compact;
            self.compact_detail = compact_detail;
            self.root
                .set_attribute("data-compact-detail", if compact_detail { "true" } else { "false" })
                .map_err(|_| "picker detail layout")?;
            if compact_detail {
                append(&self.root, &self.detail)?;
            } else {
                append(&self.stage, &self.detail)?;
            }
            if redraw && self.displayed.is_some() {
                self.draw()?;
            }
        }
        let closing_done = self.closing.is_some_and(|(started, direction)| {
            skill_picker_close(now - started, direction).reveal_alpha == 0.0
        });
        if closing_done {
            if offer.map(|offer| offer.sequence)
                == self.displayed.as_ref().map(|offer| offer.sequence)
            {
                // Keep the barrier until the session has committed or rejected the command.
                return Ok(());
            }
            self.displayed = None;
            self.closing = None;
            self.root.set_hidden(true);
        }
        if self.closing.is_none()
            && let Some(offer) = offer
        {
            if self.displayed.as_ref().is_none_or(|displayed| displayed.sequence != offer.sequence)
            {
                let cue = if self.displayed.is_some() {
                    AudioCue::UnlockSkill
                } else {
                    AudioCue::OpenPanel
                };
                self.sound(cue, 1.0);
                self.displayed = Some(offer.clone());
                self.special_available = special_available;
                self.selected = offer.automatic_choice_index.unwrap_or(0);
                self.detail_index = None;
                self.started_at = if self.waiting_reroll { now - 400.0 } else { now };
                self.waiting_reroll = false;
                self.had_interactive = false;
                self.navigation.release();
                self.root.set_hidden(false);
                self.draw()?;
            } else if self.special_available != special_available
                || self.displayed.as_ref() != Some(offer)
            {
                self.displayed = Some(offer.clone());
                self.special_available = special_available;
                self.draw()?;
            }
        }
        if offer.is_none() && self.closing.is_none() && self.displayed.is_some() {
            self.closing = Some((now, -0.75));
        }
        let Some(displayed) = &self.displayed else {
            return Ok(());
        };
        let reveal = self.closing.map_or_else(
            || skill_picker_reveal(now - self.started_at),
            |(started, direction)| skill_picker_close(now - started, direction),
        );
        let interactive = reveal.interactive
            && !self.waiting_reroll
            && displayed.automatic_choice_index.is_none();
        self.stage
            .set_attribute("data-reveal-interactive", if interactive { "true" } else { "false" })
            .map_err(|_| "picker reveal state")?;
        self.curtain
            .style()
            .set_property("opacity", &reveal.curtain_alpha.to_string())
            .map_err(|_| "picker curtain")?;
        self.content
            .style()
            .set_property("opacity", &reveal.panel_alpha.to_string())
            .map_err(|_| "picker panel opacity")?;
        self.ambient
            .style()
            .set_property("opacity", &reveal.ambient_alpha.to_string())
            .map_err(|_| "picker ambient opacity")?;
        let (scale, center_y) = if self.compact {
            let logical_height = if self.special_available { 960.0 } else { 850.0 };
            (
                ((f64::from(width) - 16.0) / 480.0)
                    .min((f64::from(height) - 16.0) / logical_height),
                logical_height / 2.0,
            )
        } else if width < 1000 {
            let logical_width = skill_picker_panel(displayed.options.len()).width
                + if self.special_available { 480.0 } else { 32.0 };
            (
                ((f64::from(width) - 16.0) / logical_width).min((f64::from(height) - 16.0) / 560.0),
                450.0,
            )
        } else {
            ((f64::from(width) / 1600.0).min(f64::from(height) / 900.0), 450.0)
        };
        style(
            &self.stage,
            &format!(
                "left:{}px;top:{}px;transform:scale({scale})",
                (f64::from(width) - 1600.0 * scale) / 2.0,
                f64::from(height) / 2.0 - center_y * scale
            ),
        )?;
        self.animate(now)?;
        if interactive && !self.had_interactive {
            self.focus_choice(0)?;
        }
        self.had_interactive = interactive;
        let events = std::mem::take(&mut *self.events.borrow_mut());
        for event in events {
            if interactive {
                self.apply_event(event, now)?;
            }
        }
        if interactive {
            let _back =
                self.navigation.update(&self.content, &self.document, "skill-picker", now)?;
        }
        if reveal.interactive
            && self.closing.is_none()
            && now - self.started_at >= 1400.0
            && let Some(index) =
                self.displayed.as_ref().and_then(|offer| offer.automatic_choice_index)
        {
            self.choose(index, now);
        }
        self.update_detail()?;
        Ok(())
    }

    fn choose(&mut self, index: usize, now: f64) {
        if self.closing.is_some() {
            return;
        }
        let Some(offer) = &self.displayed else {
            return;
        };
        let Some(option) = offer.options.get(index) else {
            return;
        };
        self.actions.push(SkillPickerAction::Choose(PlayerSkillChoice {
            choice_index: index,
            offer_sequence: offer.sequence,
            skill_id: option.skill_id,
        }));
        self.selected = index;
        self.sound(AudioCue::PickSkill, 1.0);
        self.sound(AudioCue::OpenPanel, 0.75);
        self.detail_index = None;
        self.closing = Some((now, -0.75));
    }

    fn apply_event(&mut self, event: PickerEvent, now: f64) -> Result<(), String> {
        if let PickerEvent::Activate(_, _, sequence) = &event
            && self.displayed.as_ref().is_none_or(|offer| offer.sequence != *sequence)
        {
            return Ok(());
        }
        match event {
            PickerEvent::Activate(PickerTarget::Choice(index), _, _) => self.choose(index, now),
            PickerEvent::Activate(PickerTarget::Detail(index), touch, _) => {
                if touch {
                    self.detail_index = Some(index);
                    self.selected = index;
                } else {
                    self.choose(index, now);
                }
            }
            PickerEvent::Activate(PickerTarget::CloseDetail, _, _) => self.detail_index = None,
            PickerEvent::Activate(PickerTarget::Reroll, _, _) if self.special_available => {
                let sequence = self.displayed.as_ref().expect("open offer").sequence;
                self.actions.push(SkillPickerAction::Reroll { offer_sequence: sequence });
                self.sound(AudioCue::Summon, 0.8);
                self.waiting_reroll = true;
                self.detail_index = None;
            }
            PickerEvent::Activate(PickerTarget::Save, _, _) if self.special_available => {
                let sequence = self.displayed.as_ref().expect("open offer").sequence;
                self.actions.push(SkillPickerAction::Save { offer_sequence: sequence });
                self.sound(AudioCue::Click, 1.0);
                self.sound(AudioCue::OpenPanel, 0.75);
                self.closing = Some((now, -1.0));
                self.detail_index = None;
            }
            PickerEvent::Select(index) => self.selected = index,
            PickerEvent::Hover(index) => {
                self.detail_index = index;
                if let Some(index) = index {
                    self.selected = index;
                }
            }
            PickerEvent::Key(code) => {
                let delta = match code.as_str() {
                    "ArrowLeft" | "ArrowUp" => -1,
                    "ArrowRight" | "ArrowDown" => 1,
                    _ => 0,
                };
                if delta != 0 {
                    let count = self.displayed.as_ref().expect("open offer").options.len();
                    self.selected =
                        (self.selected as isize + delta).rem_euclid(count as isize) as usize;
                    self.detail_index = None;
                    self.focus_choice(self.selected)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn focus_choice(&self, index: usize) -> Result<(), String> {
        let Some(node) = self
            .content
            .query_selector(&format!("[data-picker-choice='{index}']"))
            .map_err(|_| "picker focus choice")?
        else {
            return Ok(());
        };
        node.dyn_into::<HtmlElement>()
            .map_err(|_| "picker focus type")?
            .focus()
            .map_err(|_| "picker focus".into())
    }

    fn draw(&self) -> Result<(), String> {
        let offer = self.displayed.as_ref().expect("open offer");
        self.content.set_text_content(None);
        self.ambient.set_text_content(None);
        self.root
            .set_attribute("data-offer-sequence", &offer.sequence.to_string())
            .map_err(|_| "picker root sequence")?;
        self.stage
            .set_attribute("data-offer-sequence", &offer.sequence.to_string())
            .map_err(|_| "picker offer sequence")?;
        for index in 0..8 {
            let arc = art(
                &self.document,
                &self.ambient,
                &self.ui,
                3,
                [800.0, 450.0],
                [1.9, 1.9],
                index as f64 * 45.0,
            )?;
            arc.set_attribute("data-picker-arc", &index.to_string()).map_err(|_| "picker arc")?;
        }
        for y in [350.0, 550.0] {
            art(&self.document, &self.ambient, &self.ui, 62, [800.0, y], [1.6, 1.6], 0.0)?;
        }
        let panel = if self.compact {
            HudRect::new(580.0, 125.0, 440.0, offer.options.len().div_ceil(2) as f64 * 315.0 + 30.0)
        } else {
            skill_picker_panel(offer.options.len())
        };
        art(
            &self.document,
            &self.content,
            &self.ui,
            37,
            [800.0, if self.compact { 62.0 } else { 215.0 }],
            if self.compact { [0.7, 0.7] } else { [1.0, 1.0] },
            0.0,
        )?;
        art(
            &self.document,
            &self.content,
            &self.ui,
            59,
            [800.0, if self.compact { panel.y + panel.height + 28.0 } else { 655.5 }],
            if self.compact { [0.7, 0.7] } else { [1.0, 1.0] },
            0.0,
        )?;
        let leather = self.ui.tiles(&self.document, 49, panel.width, panel.height)?;
        place(&leather, panel)?;
        append(&self.content, &leather)?;
        for y in [panel.y - 2.0, panel.y + panel.height - 15.0] {
            let chain = self.ui.tiles(&self.document, 10, panel.width - 20.0, 19.0)?;
            place(&chain, HudRect::new(panel.x + 10.0, y, panel.width - 20.0, 19.0))?;
            append(&self.content, &chain)?;
        }
        for x in [panel.x, panel.x + panel.width - 17.0] {
            let chain = self.ui.tiles(&self.document, 79, 21.0, panel.height - 20.0)?;
            place(&chain, HudRect::new(x, panel.y + 10.0, 21.0, panel.height - 20.0))?;
            append(&self.content, &chain)?;
        }
        for (record, x, y) in [
            (107, panel.x + 37.0, panel.y + 37.0),
            (108, panel.x + panel.width - 37.0, panel.y + 37.0),
            (109, panel.x + 37.0, panel.y + panel.height - 37.0),
            (110, panel.x + panel.width - 37.0, panel.y + panel.height - 37.0),
        ] {
            art(&self.document, &self.content, &self.ui, record, [x, y], [1.0, 1.0], 0.0)?;
        }
        let level_line = if self.compact {
            format!("YOU ARE NOW LEVEL {}", offer.level)
        } else {
            format!("Y O U   A R E   N O W   L E V E L   {}", offer.level)
        };
        let level_y = if self.compact { 112.0 } else { 262.5 };
        for degrees in (0..360).step_by(45) {
            let angle = f64::from(degrees) * std::f64::consts::PI / 180.0;
            bitmap_text(
                &self.document,
                &self.content,
                &TextSpec {
                    tint: 0x000000,
                    ..TextSpec::new(
                        Font::Menu,
                        &level_line,
                        800.0 + angle.cos() * 5.0,
                        level_y + angle.sin() * 5.0,
                    )
                },
            )?;
        }
        bitmap_text(
            &self.document,
            &self.content,
            &TextSpec { tint: 0xe0bd58, ..TextSpec::new(Font::Menu, &level_line, 800.0, level_y) },
        )?;
        for (index, option) in offer.options.iter().enumerate() {
            let card = skill_picker_card(option);
            let center = if self.compact {
                if index + 1 == offer.options.len() && index % 2 == 0 {
                    800.0
                } else if index % 2 == 0 {
                    690.0
                } else {
                    910.0
                }
            } else {
                skill_picker_centers(offer.options.len())[index]
            };
            let card_top = if self.compact { 145.0 + (index / 2) as f64 * 315.0 } else { 302.5 };
            let node = button(
                &self.document,
                &self.content,
                "",
                &format!(
                    "{}{}, {}. {}",
                    if option.insight { "Insight. " } else { "" },
                    card.name,
                    card.family_label.trim(),
                    card.quick_description
                ),
                HudRect::new(center - 100.0, card_top, 200.0, 295.0),
            )?;
            node.remove_attribute("data-panel-action").map_err(|_| "picker action owner")?;
            node.set_attribute("data-skill-id", &option.skill_id.to_string())
                .map_err(|_| "picker skill id")?;
            node.set_attribute("data-picker-choice", &index.to_string())
                .map_err(|_| "picker choice")?;
            let frame = self.skills.frame(&self.document, 0, 200.0, 295.0, 30.0)?;
            append(&node, &frame)?;
            if option.insight {
                node.set_attribute("data-picker-insight", "true").map_err(|_| "picker insight")?;
            }
            art(&self.document, &node, &self.skills, 13, [100.0, 80.0], [1.15, 1.15], 0.0)?;
            for (glow_index, color) in card.glow_tints.iter().enumerate() {
                let glow = art(
                    &self.document,
                    &node,
                    &self.skills,
                    164,
                    [100.0, 80.0],
                    [1.15, 1.15],
                    0.0,
                )?;
                tint(&self.document, &glow, *color)?;
                if card.glow_tints.len() == 2 {
                    glow.style()
                        .set_property(
                            "clip-path",
                            if glow_index == 0 {
                                "polygon(0 0,100% 0,0 100%)"
                            } else {
                                "polygon(100% 0,100% 100%,0 100%)"
                            },
                        )
                        .map_err(|_| "weld glow split")?;
                }
            }
            art(
                &self.document,
                &node,
                &self.skills,
                card.frame_record,
                [100.0, 80.0],
                [1.0, 1.0],
                0.0,
            )?;
            let shadow = art(
                &self.document,
                &node,
                &self.skills,
                card.icon_record,
                [104.0, 84.0],
                [1.0, 1.0],
                0.0,
            )?;
            tint(&self.document, &shadow, 0)?;
            art(
                &self.document,
                &node,
                &self.skills,
                card.icon_record,
                [100.0, 80.0],
                [1.0, 1.0],
                0.0,
            )?;
            let name_lines = native_skill_page_wrapped_lines(&card.name);
            let description_lines = native_skill_page_wrapped_lines(card.quick_description);
            let color = if option.insight { 0xd9ba70 } else { card.root_tint };
            for (font, source, baseline) in [
                (Font::Medium, name_lines.join("\n"), 150.0),
                (
                    Font::SkillUppercase,
                    card.family_label.to_owned(),
                    150.0 + native_skill_page_text_height(&name_lines),
                ),
            ] {
                let line_height = (font == Font::Medium).then_some(17.0);
                if !option.insight {
                    bitmap_text(
                        &self.document,
                        &node,
                        &TextSpec {
                            tint: 0x000000,
                            line_height,
                            ..TextSpec::new(font, &source, 101.0, baseline + 1.0)
                        },
                    )?;
                }
                bitmap_text(
                    &self.document,
                    &node,
                    &TextSpec {
                        tint: color,
                        line_height,
                        ..TextSpec::new(font, &source, 100.0, baseline)
                    },
                )?;
            }
            bitmap_text(
                &self.document,
                &node,
                &TextSpec {
                    tint: if option.insight { 0xd9ba70 } else { 0xffffff },
                    line_height: Some(17.0),
                    ..TextSpec::new(
                        Font::Medium,
                        &description_lines.join("\n"),
                        100.0,
                        230.0 - native_skill_page_text_height(&description_lines) / 2.0,
                    )
                },
            )?;
            if option.insight {
                bitmap_text(
                    &self.document,
                    &node,
                    &TextSpec {
                        tint: 0xd9ba70,
                        ..TextSpec::new(Font::Body, "Insight", 100.0, 3.0)
                    },
                )?;
            }
            let info = button(
                &self.document,
                &self.content,
                "",
                &format!("Show details for {}", card.name),
                HudRect::new(center - 43.5, card_top + 36.0, 87.0, 88.0),
            )?;
            info.remove_attribute("data-panel-action").map_err(|_| "picker detail owner")?;
            info.set_attribute("data-picker-detail", &index.to_string())
                .map_err(|_| "picker detail")?;
        }
        if self.special_available {
            for (record, action, x) in [
                (57, "save", 800.0 - panel.width / 2.0 - 140.0),
                (56, "reroll", 800.0 + panel.width / 2.0 + 40.0),
            ] {
                let (left, top, width, art_scale) = if self.compact {
                    (if action == "save" { 590.0 } else { 810.0 }, 835.0, 200.0, 0.75)
                } else {
                    (x, 322.5, 255.0, 1.0)
                };
                let node = button(
                    &self.document,
                    &self.content,
                    "",
                    if action == "save" { "Save Skill" } else { "Roll Again" },
                    HudRect::new(left, top, width, 100.0),
                )?;
                node.remove_attribute("data-panel-action").map_err(|_| "picker special owner")?;
                node.set_attribute("data-picker-action", action).map_err(|_| "picker special")?;
                art(
                    &self.document,
                    &node,
                    &self.ui,
                    record,
                    [width / 2.0, 50.0],
                    [art_scale, art_scale],
                    0.0,
                )?;
            }
        }
        Ok(())
    }

    fn animate(&self, now: f64) -> Result<(), String> {
        let arcs =
            self.ambient.query_selector_all("[data-picker-arc]").map_err(|_| "picker arcs")?;
        for index in 0..arcs.length() {
            let node = arcs
                .item(index)
                .ok_or("picker arc node")?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "picker arc type")?;
            node.style()
                .set_property(
                    "transform",
                    &format!(
                        "translate(-50%,-50%) rotate({}deg) scale(1.9)",
                        f64::from(index) * 45.0
                            + now / 1000.0 * 0.08 * 180.0 / std::f64::consts::PI
                    ),
                )
                .map_err(|_| "picker arc animation")?;
        }
        let choices = self
            .content
            .query_selector_all("[data-picker-choice]")
            .map_err(|_| "picker choices")?;
        for index in 0..choices.length() {
            let node = choices
                .item(index)
                .ok_or("picker choice node")?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "picker choice type")?;
            node.set_attribute(
                "data-selected",
                if index as usize == self.selected { "true" } else { "false" },
            )
            .map_err(|_| "picker selection")?;
            if node.has_attribute("data-picker-insight") {
                node.style()
                    .set_property(
                        "filter",
                        &format!(
                            "brightness({})",
                            0.85 + 0.3
                                * float::sin(
                                    (now - self.started_at) / 10.0 * 2.0 * std::f64::consts::PI
                                        / 180.0
                                )
                                .max(0.0)
                        ),
                    )
                    .map_err(|_| "insight pulse")?;
            }
        }
        Ok(())
    }

    fn update_detail(&self) -> Result<(), String> {
        let key = self.detail_index.map(|index| {
            (self.displayed.as_ref().expect("open offer").sequence, index, self.compact_detail)
        });
        self.detail.set_hidden(key.is_none());
        let Some(index) = self.detail_index else {
            self.detail_rendered.set(None);
            return Ok(());
        };
        let offer = self.displayed.as_ref().expect("open offer");
        let option = &offer.options[index];
        let lines = skill_picker_detail_lines(option);
        let layout = native_skill_hover_box_layout(
            &lines,
            solomon_sim::Vec2::new(skill_picker_centers(offer.options.len())[index], 382.5),
        );
        let repaint = self.detail_rendered.get() != key;
        if repaint {
            crate::ui::skill_hover::paint_skill_hover_box(
                &self.document,
                &self.detail,
                &layout,
                option.skill_id,
            )?;
            self.detail_rendered.set(key);
        }
        if self.compact_detail {
            let scale = ((f64::from(self.viewport.0) - 24.0) / layout.rect.width).min(1.0);
            if repaint {
                let close = native_button(&self.document, "DONE", "close-detail")?;
                close
                    .set_attribute("data-picker-action", "close-detail")
                    .map_err(|_| "close picker detail")?;
                style(
                    &close,
                    "position:absolute;left:0;top:calc(var(--native-hover-height) + 16px);width:100%;height:44px",
                )?;
                append(&self.detail, &close)?;
            }
            style(
                &self.detail,
                &format!(
                    "left:12px;top:auto;bottom:12px;width:{}px;height:{}px;max-width:none;padding:0;border:0;background:none;--native-hover-scale:{scale};--native-hover-height:{}px",
                    layout.rect.width * scale,
                    layout.rect.height * scale + 76.0,
                    layout.rect.height * scale,
                ),
            )?;
        } else {
            style(
                &self.detail,
                &format!(
                    "left:{}px;top:{}px;width:{}px;height:{}px;max-width:none;padding:0;border:0;background:none;--native-hover-scale:1",
                    layout.rect.x, layout.rect.y, layout.rect.width, layout.rect.height,
                ),
            )?;
        }
        Ok(())
    }

    fn attach_events(&mut self) -> Result<(), String> {
        let touch = Rc::new(Cell::new(false));
        let touch_state = touch.clone();
        let pointer = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            if let Some(pointer) = event.dyn_ref::<PointerEvent>() {
                touch_state.set(pointer.pointer_type() == "touch");
            }
        });
        self.root
            .add_event_listener_with_callback("pointerdown", pointer.as_ref().unchecked_ref())
            .map_err(|_| "picker pointer")?;
        self.listeners.push(pointer);
        let events = self.events.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(target) = target(&event) else {
                return;
            };
            let sequence = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| target.closest("[data-offer-sequence]").ok().flatten())
                .and_then(|stage| stage.get_attribute("data-offer-sequence"))
                .and_then(|sequence| sequence.parse().ok());
            if let Some(sequence) = sequence {
                events.borrow_mut().push(PickerEvent::Activate(target, touch.get(), sequence));
            }
        });
        self.root
            .add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "picker click")?;
        self.listeners.push(click);
        for name in ["pointerover", "focusin"] {
            let events = self.events.clone();
            let hover = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                if event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
                    .is_some_and(|target| target.closest(".picker-detail").ok().flatten().is_some())
                {
                    return;
                }
                let target = target(&event);
                if let Some(PickerTarget::Choice(index)) = target {
                    events.borrow_mut().push(PickerEvent::Select(index));
                }
                let detail = match target {
                    Some(PickerTarget::Detail(index)) => Some(index),
                    _ => None,
                };
                events.borrow_mut().push(PickerEvent::Hover(detail));
            });
            self.root
                .add_event_listener_with_callback(name, hover.as_ref().unchecked_ref())
                .map_err(|_| "picker hover")?;
            self.listeners.push(hover);
        }
        let events = self.events.clone();
        let key = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                return;
            };
            if matches!(
                key.code().as_str(),
                "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Escape"
            ) {
                event.prevent_default();
                events.borrow_mut().push(PickerEvent::Key(key.code()));
            }
        });
        self.root
            .add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref())
            .map_err(|_| "picker key")?;
        self.listeners.push(key);
        Ok(())
    }
}

fn target(event: &Event) -> Option<PickerTarget> {
    let node = event
        .target()?
        .dyn_into::<Element>()
        .ok()?
        .closest("[data-picker-choice],[data-picker-detail],[data-picker-action]")
        .ok()??;
    if let Some(index) =
        node.get_attribute("data-picker-choice").and_then(|value| value.parse().ok())
    {
        return Some(PickerTarget::Choice(index));
    }
    if let Some(index) =
        node.get_attribute("data-picker-detail").and_then(|value| value.parse().ok())
    {
        return Some(PickerTarget::Detail(index));
    }
    match node.get_attribute("data-picker-action")?.as_str() {
        "save" => Some(PickerTarget::Save),
        "reroll" => Some(PickerTarget::Reroll),
        "close-detail" => Some(PickerTarget::CloseDetail),
        _ => None,
    }
}
