//! Native earned skill selection. The accepted offer sequence and simulation
//! command admission remain canonical; transitions retain their displayed offer.
use crate::gpu_ui::{StockAtlas, UiFrame, native_text};
use crate::player_panels::chrome::{color, rect};
use solomon_sim::audio::{AudioCue, AudioEvent, AudioOptions};
use solomon_sim::float;
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
    SkillPickerReveal, skill_picker_card, skill_picker_centers, skill_picker_close,
    skill_picker_panel, skill_picker_reveal,
};
use web_sys::Document;
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
}
enum PickerEvent {
    Activate(PickerTarget, bool, u64),
    Hover(Option<usize>),
    Select(usize),
    Key(String),
}

pub struct SkillPicker {
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
    compact: bool,
    compact_detail: bool,
    viewport: (i32, i32),
    now: f64,
    focus_index: Option<usize>,
    touch: bool,
}
impl SkillPicker {
    pub async fn attach(_document: &Document) -> Result<Self, String> {
        Ok(Self {
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
            compact: false,
            compact_detail: false,
            viewport: (0, 0),
            now: 0.0,
            focus_index: None,
            touch: false,
        })
    }
    pub fn is_open(&self) -> bool {
        self.displayed.is_some()
    }
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("skill-picker"))
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
    pub fn reject_action(&mut self, now: f64) {
        self.closing = None;
        self.waiting_reroll = false;
        self.started_at = now - 400.0;
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
                    self.focus_index = Some(self.selected);
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub fn update(
        &mut self,
        offer: Option<&PlayerSkillOffer>,
        special_available: bool,
        now: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        self.now = now;
        self.viewport = (width, height);
        self.compact = width < 600;
        self.compact_detail = width < 1000;
        if self.closing.is_some_and(|(started, direction)| {
            skill_picker_close(now - started, direction).reveal_alpha == 0.0
        }) {
            if offer.map(|offer| offer.sequence)
                == self.displayed.as_ref().map(|offer| offer.sequence)
            {
                return Ok(());
            }
            self.displayed = None;
            self.closing = None;
        }
        if self.closing.is_none()
            && let Some(offer) = offer
        {
            if self.displayed.as_ref().is_none_or(|old| old.sequence != offer.sequence) {
                self.sound(
                    if self.displayed.is_some() {
                        AudioCue::UnlockSkill
                    } else {
                        AudioCue::OpenPanel
                    },
                    1.0,
                );
                self.displayed = Some(offer.clone());
                self.special_available = special_available;
                self.selected = offer.automatic_choice_index.unwrap_or(0);
                self.detail_index = None;
                self.started_at = if self.waiting_reroll { now - 400.0 } else { now };
                self.waiting_reroll = false;
                self.had_interactive = false;
                self.focus_index = None;
            } else if self.special_available != special_available
                || self.displayed.as_ref() != Some(offer)
            {
                self.displayed = Some(offer.clone());
                self.special_available = special_available;
            }
        }
        if offer.is_none() && self.closing.is_none() && self.displayed.is_some() {
            self.closing = Some((now, -0.75));
        }
        let Some(displayed) = &self.displayed else { return Ok(()) };
        let reveal = self.closing.map_or_else(
            || skill_picker_reveal(now - self.started_at),
            |(started, direction)| skill_picker_close(now - started, direction),
        );
        let interactive = reveal.interactive
            && !self.waiting_reroll
            && displayed.automatic_choice_index.is_none();
        if interactive && !self.had_interactive {
            self.focus_index = Some(0);
        }
        self.had_interactive = interactive;
        if reveal.interactive
            && self.closing.is_none()
            && now - self.started_at >= 1400.0
            && let Some(index) =
                self.displayed.as_ref().and_then(|offer| offer.automatic_choice_index)
        {
            self.choose(index, now);
        }
        Ok(())
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some(offer) = self.displayed.clone() else { return Ok(()) };
        frame.input_scope(&format!("skill-picker:{}:{:?}", offer.sequence, self.detail_index));
        frame.ctx.input(|input| {
            if input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerButton { pressed: true, .. }))
            {
                self.touch =
                    input.events.iter().any(|event| matches!(event, egui::Event::Touch { .. }));
            }
        });
        let now = self.now;
        let reveal = self.closing.map_or_else(
            || skill_picker_reveal(now - self.started_at),
            |(started, direction)| skill_picker_close(now - started, direction),
        );
        let interactive =
            reveal.interactive && !self.waiting_reroll && offer.automatic_choice_index.is_none();
        let (width, height) = (self.viewport.0 as f64, self.viewport.1 as f64);
        let (scale, center_y) = if self.compact {
            let logical_height = if self.special_available { 960.0 } else { 850.0 };
            (((width - 16.0) / 480.0).min((height - 16.0) / logical_height), logical_height / 2.0)
        } else if width < 1000.0 {
            let logical_width = skill_picker_panel(offer.options.len()).width
                + if self.special_available { 480.0 } else { 32.0 };
            (((width - 16.0) / logical_width).min((height - 16.0) / 560.0), 450.0)
        } else {
            ((width / 1600.0).min(height / 900.0), 450.0)
        };
        let origin = egui::pos2(
            ((width - 1600.0 * scale) / 2.0) as f32,
            (height / 2.0 - center_y * scale) as f32,
        );
        let layer = self.layer_id();
        let transform = egui::emath::TSTransform::new(origin.to_vec2(), scale as f32);
        let mut events = Vec::new();
        let mut error = None;
        let ctx = frame.ctx.clone();
        crate::player_panels::chrome::native_area(&ctx, layer, transform).show(&ctx, |ui| {
            ui.set_min_size(egui::vec2(1600.0, if self.compact { 960.0 } else { 900.0 }));
            let p = ui.painter();
            let bounds = egui::Rect::from_min_size(
                egui::pos2(-origin.x / scale as f32, -origin.y / scale as f32),
                egui::vec2(width as f32 / scale as f32, height as f32 / scale as f32),
            );
            p.rect_filled(
                bounds,
                0.0,
                egui::Color32::BLACK.gamma_multiply(reveal.curtain_alpha as f32),
            );
            let painted = self.paint_offer(frame, ui, &offer, reveal, interactive, &mut events);
            if let Err(message) = painted {
                error = Some(message);
            }
        });
        if interactive {
            for key in frame.key_codes() {
                if key.pressed
                    && !key.repeat
                    && matches!(
                        key.code.as_str(),
                        "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown"
                    )
                {
                    events.push(PickerEvent::Key(key.code.clone()));
                }
            }
        }
        for event in events {
            self.apply_event(event, now)?;
        }
        self.paint_detail(frame, origin, scale as f32)?;
        error.map_or(Ok(()), Err)
    }
    fn paint_offer(
        &mut self,
        frame: &UiFrame<'_>,
        ui: &egui::Ui,
        offer: &PlayerSkillOffer,
        reveal: SkillPickerReveal,
        interactive: bool,
        events: &mut Vec<PickerEvent>,
    ) -> Result<(), String> {
        let p = ui.painter();
        let panel_alpha = reveal.panel_alpha as f32;
        let ambient_alpha = reveal.ambient_alpha as f32;
        let white = egui::Color32::WHITE.gamma_multiply(panel_alpha);
        let ambient = frame.with_blend(solomon_graphics::BlendMode::Add);
        for index in 0..8 {
            self.ui.paint_centered(
                &ambient,
                p,
                3,
                egui::pos2(800.0, 450.0),
                1.9,
                (index as f64 * 45.0 + self.now / 1000.0 * 0.08 * 180.0 / std::f64::consts::PI)
                    .to_radians() as f32,
                egui::Color32::WHITE.gamma_multiply(ambient_alpha),
            )?;
        }
        for y in [350.0, 550.0] {
            self.ui.paint_centered(
                &ambient,
                p,
                62,
                egui::pos2(800.0, y),
                1.6,
                0.0,
                egui::Color32::WHITE.gamma_multiply(ambient_alpha),
            )?;
        }
        let panel = if self.compact {
            HudRect::new(580.0, 125.0, 440.0, offer.options.len().div_ceil(2) as f64 * 315.0 + 30.0)
        } else {
            skill_picker_panel(offer.options.len())
        };
        self.ui.paint_centered(
            frame,
            p,
            37,
            egui::pos2(800.0, if self.compact { 62.0 } else { 215.0 }),
            if self.compact { 0.7 } else { 1.0 },
            0.0,
            white,
        )?;
        self.ui.paint_centered(
            frame,
            p,
            59,
            egui::pos2(
                800.0,
                if self.compact { (panel.y + panel.height + 28.0) as f32 } else { 655.5 },
            ),
            if self.compact { 0.7 } else { 1.0 },
            0.0,
            white,
        )?;
        self.ui.paint_tiles(frame, p, 49, rect(panel), 1.0, white)?;
        for y in [panel.y - 2.0, panel.y + panel.height - 15.0] {
            self.ui.paint_tiles(
                frame,
                p,
                10,
                rect(HudRect::new(panel.x + 10.0, y, panel.width - 20.0, 19.0)),
                1.0,
                white,
            )?;
        }
        for x in [panel.x, panel.x + panel.width - 17.0] {
            self.ui.paint_tiles(
                frame,
                p,
                79,
                rect(HudRect::new(x, panel.y + 10.0, 21.0, panel.height - 20.0)),
                1.0,
                white,
            )?;
        }
        for (record, x, y) in [
            (107, panel.x + 37.0, panel.y + 37.0),
            (108, panel.x + panel.width - 37.0, panel.y + 37.0),
            (109, panel.x + 37.0, panel.y + panel.height - 37.0),
            (110, panel.x + panel.width - 37.0, panel.y + panel.height - 37.0),
        ] {
            self.ui.paint_centered(
                frame,
                p,
                record,
                egui::pos2(x as f32, y as f32),
                1.0,
                0.0,
                white,
            )?;
        }
        let level = if self.compact {
            format!("YOU ARE NOW LEVEL {}", offer.level)
        } else {
            format!("Y O U   A R E   N O W   L E V E L   {}", offer.level)
        };
        let level_y = if self.compact { 112.0 } else { 262.5 };
        for degrees in (0..360).step_by(45) {
            let angle = degrees as f64 * std::f64::consts::PI / 180.0;
            native_text(
                frame,
                p,
                &TextSpec {
                    tint: 0,
                    alpha: panel_alpha as f64,
                    ..TextSpec::new(
                        Font::Menu,
                        &level,
                        800.0 + angle.cos() * 5.0,
                        level_y + angle.sin() * 5.0,
                    )
                },
            );
        }
        native_text(
            frame,
            p,
            &TextSpec {
                tint: 0xe0bd58,
                alpha: panel_alpha as f64,
                ..TextSpec::new(Font::Menu, &level, 800.0, level_y)
            },
        );
        for (index, option) in offer.options.iter().enumerate() {
            let card = skill_picker_card(option);
            let x = if self.compact {
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
            let top = if self.compact { 145.0 + (index / 2) as f64 * 315.0 } else { 302.5 };
            let r = rect(HudRect::new(x - 100.0, top, 200.0, 295.0));
            let mut child =
                crate::player_panels::chrome::child_ui(ui, "hud-admission", ui.max_rect());
            if !(interactive) {
                child.disable();
            }
            let response = child.interact(
                r,
                egui::Id::unique(("picker-choice", offer.sequence, index)),
                egui::Sense::click(),
            );
            frame.record(
                &format!("picker:choice:{index}"),
                &format!(
                    "{}{}, {}. {}",
                    if option.insight { "Insight. " } else { "" },
                    card.name,
                    card.family_label.trim(),
                    card.quick_description
                ),
                "button",
                &response,
            );
            frame.record_value(
                &format!("picker:choice:{index}"),
                &option.skill_id.to_string(),
                Some(self.selected == index),
            );
            if self.focus_index == Some(index) {
                response.request_focus();
                self.focus_index = None;
            }
            if response.clicked() {
                events.push(PickerEvent::Activate(
                    PickerTarget::Choice(index),
                    self.touch,
                    offer.sequence,
                ));
            }
            if response.hovered() || response.has_focus() {
                events.push(PickerEvent::Select(index));
            }
            let pulse = if option.insight {
                0.85 + 0.3
                    * float::sin(
                        (self.now - self.started_at) / 10.0 * 2.0 * std::f64::consts::PI / 180.0,
                    )
                    .max(0.0)
            } else {
                1.0
            };
            let tint = egui::Color32::WHITE.gamma_multiply(panel_alpha * pulse.min(1.0) as f32);
            self.skills.paint_frame(frame, p, 0, r, 30.0, tint)?;
            if self.selected == index {
                p.rect_stroke(
                    r,
                    2.0,
                    egui::Stroke::new(2.0, color(0xd9ba70).gamma_multiply(panel_alpha)),
                    egui::StrokeKind::Inside,
                );
            }
            let center = egui::pos2(x as f32, (top + 80.0) as f32);
            self.skills.paint_centered(frame, p, 13, center, 1.15, 0.0, tint)?;
            if card.glow_tints.len() == 2 {
                crate::player_panels::chrome::paint_split_art(
                    frame,
                    p,
                    &self.skills,
                    164,
                    center,
                    1.15,
                    [card.glow_tints[0], card.glow_tints[1]]
                        .map(|tint| color(tint).gamma_multiply(panel_alpha)),
                )?;
            } else {
                self.skills.paint_centered(
                    frame,
                    p,
                    164,
                    center,
                    1.15,
                    0.0,
                    color(card.glow_tints[0]).gamma_multiply(panel_alpha),
                )?;
            }
            self.skills.paint_centered(frame, p, card.frame_record, center, 1.0, 0.0, tint)?;
            self.skills.paint_centered(
                frame,
                p,
                card.icon_record,
                center + egui::vec2(4.0, 4.0),
                1.0,
                0.0,
                egui::Color32::BLACK.gamma_multiply(panel_alpha),
            )?;
            self.skills.paint_centered(frame, p, card.icon_record, center, 1.0, 0.0, tint)?;
            if pulse > 1.0 {
                self.skills.paint_centered(
                    &ambient,
                    p,
                    card.icon_record,
                    center,
                    1.0,
                    0.0,
                    egui::Color32::WHITE.gamma_multiply(panel_alpha * (pulse - 1.0) as f32),
                )?;
            }
            let names = native_skill_page_wrapped_lines(&card.name);
            let descriptions = native_skill_page_wrapped_lines(card.quick_description);
            let text_tint = if option.insight { 0xd9ba70 } else { card.root_tint };
            for (font, source, baseline) in [
                (Font::Medium, names.join("\n"), top + 150.0),
                (
                    Font::SkillUppercase,
                    card.family_label.to_owned(),
                    top + 150.0 + native_skill_page_text_height(&names),
                ),
            ] {
                let line_height = (font == Font::Medium).then_some(17.0);
                if !option.insight {
                    native_text(
                        frame,
                        p,
                        &TextSpec {
                            tint: 0,
                            alpha: panel_alpha as f64,
                            line_height,
                            ..TextSpec::new(font, &source, x + 1.0, baseline + 1.0)
                        },
                    );
                }
                native_text(
                    frame,
                    p,
                    &TextSpec {
                        tint: text_tint,
                        alpha: panel_alpha as f64,
                        line_height,
                        ..TextSpec::new(font, &source, x, baseline)
                    },
                );
            }
            native_text(
                frame,
                p,
                &TextSpec {
                    tint: if option.insight { 0xd9ba70 } else { 0xffffff },
                    alpha: panel_alpha as f64,
                    line_height: Some(17.0),
                    ..TextSpec::new(
                        Font::Medium,
                        &descriptions.join("\n"),
                        x,
                        top + 230.0 - native_skill_page_text_height(&descriptions) / 2.0,
                    )
                },
            );
            if option.insight {
                native_text(
                    frame,
                    p,
                    &TextSpec {
                        tint: 0xd9ba70,
                        alpha: panel_alpha as f64,
                        ..TextSpec::new(Font::Body, "Insight", x, top + 3.0)
                    },
                );
            }
            let info = child.interact(
                rect(HudRect::new(x - 43.5, top + 36.0, 87.0, 88.0)),
                egui::Id::unique(("picker-detail", offer.sequence, index)),
                egui::Sense::click(),
            );
            frame.record(
                &format!("picker:detail:{index}"),
                &format!("Show details for {}", card.name),
                "button",
                &info,
            );
            if info.clicked() {
                events.push(PickerEvent::Activate(
                    PickerTarget::Detail(index),
                    self.touch,
                    offer.sequence,
                ));
            }
            if info.hovered() || info.has_focus() {
                events.push(PickerEvent::Hover(Some(index)));
            } else if self.detail_index == Some(index) && !self.touch && !self.compact_detail {
                events.push(PickerEvent::Hover(None));
            }
        }
        if self.special_available {
            for (record, target, action, x) in [
                (57, PickerTarget::Save, "save", 800.0 - panel.width / 2.0 - 140.0),
                (56, PickerTarget::Reroll, "reroll", 800.0 + panel.width / 2.0 + 40.0),
            ] {
                let (left, top, width, art_scale) = if self.compact {
                    (if action == "save" { 590.0 } else { 810.0 }, 835.0, 200.0, 0.75)
                } else {
                    (x, 322.5, 255.0, 1.0)
                };
                self.ui.paint_centered(
                    frame,
                    p,
                    record,
                    egui::pos2((left + width / 2.0) as f32, (top + 50.0) as f32),
                    art_scale,
                    0.0,
                    white,
                )?;
                let mut child =
                    crate::player_panels::chrome::child_ui(ui, "hud-admission", ui.max_rect());
                if !(interactive) {
                    child.disable();
                }
                let response = child.interact(
                    rect(HudRect::new(left, top, width, 100.0)),
                    egui::Id::unique(("picker-special", offer.sequence, action)),
                    egui::Sense::click(),
                );
                frame.record(
                    &format!("picker:{action}"),
                    if action == "save" { "Save Skill" } else { "Roll Again" },
                    "button",
                    &response,
                );
                if response.clicked() {
                    events.push(PickerEvent::Activate(target, self.touch, offer.sequence));
                }
            }
        }
        frame.record_rect(
            "picker:offer",
            &level,
            "dialog",
            frame.ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default() * rect(panel),
            false,
        );
        Ok(())
    }
    fn paint_detail(
        &mut self,
        frame: &UiFrame<'_>,
        origin: egui::Pos2,
        scale: f32,
    ) -> Result<(), String> {
        let Some(index) = self.detail_index else { return Ok(()) };
        let Some(offer) = &self.displayed else { return Ok(()) };
        let Some(option) = offer.options.get(index) else { return Ok(()) };
        let lines = skill_picker_detail_lines(option);
        let layout = native_skill_hover_box_layout(
            &lines,
            solomon_sim::Vec2::new(skill_picker_centers(offer.options.len())[index], 382.5),
        );
        let mut close = false;
        egui::Area::new(egui::Id::unique("picker-tooltip"))
            .fade_in(false)
            .order(egui::Order::Tooltip)
            .fixed_pos(egui::Pos2::ZERO)
            .constrain(false)
            .show(frame.ctx, |ui| {
                let (position, card_scale) = if self.compact_detail {
                    let s = ((self.viewport.0 as f32 - 24.0) / layout.rect.width as f32).min(1.0);
                    (
                        egui::pos2(
                            12.0,
                            self.viewport.1 as f32 - 12.0 - layout.rect.height as f32 * s - 76.0,
                        ),
                        s,
                    )
                } else {
                    (origin + egui::vec2(layout.rect.x as f32, layout.rect.y as f32) * scale, scale)
                };
                crate::ui::skill_hover::paint_skill_hover_box(
                    frame,
                    ui.painter(),
                    &layout,
                    option.skill_id,
                    position,
                    card_scale,
                );
                frame.record_rect(
                    "picker:tooltip",
                    &lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n"),
                    "tooltip",
                    egui::Rect::from_min_size(
                        position,
                        egui::vec2(layout.rect.width as f32, layout.rect.height as f32)
                            * card_scale,
                    ),
                    false,
                );
                if self.compact_detail {
                    let r = egui::Rect::from_min_size(
                        position + egui::vec2(0.0, layout.rect.height as f32 * card_scale + 16.0),
                        egui::vec2(layout.rect.width as f32 * card_scale, 44.0),
                    );
                    let response = ui.put(r, egui::Button::new("DONE"));
                    frame.record("picker:close-detail", "DONE", "button", &response);
                    close = response.clicked();
                }
            });
        if close {
            self.detail_index = None;
        }
        Ok(())
    }
}
