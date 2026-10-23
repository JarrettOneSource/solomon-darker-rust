//! College chat and its embedded selectors. The simulation owns every authored line,
//! price, unlock and action; this module owns only the current reading position.

use super::chrome::PanelCanvas;
use super::{
    DialogueState, HubDialogueContext, PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot,
    PlayerPanels, Target,
};
use solomon_sim::audio::{AudioCue, AudioEvent};
use solomon_sim::economy::{HubInventoryAction, NativeHubNpcState};
use solomon_sim::hub::interaction::{
    HubNpcChatChoice, HubNpcChatContent, HubNpcChatNext, HubNpcListSelector, HubNpcSelectorId,
    consume_hub_npc_speech, create_hub_npc_chat_content, hub_npc_chat_choices, hub_npc_question,
    hub_npc_selector_action_with_mods, hub_npc_selector_response_with_mods,
    hub_npc_selector_rows_with_mods, hub_npc_selector_title,
};
use solomon_sim::hud::HudRect;
use solomon_sim::modding::content::PreparedModBoastIcon;
use solomon_sim::modding::projection::ModContentProjection;
use solomon_sim::native_ui_notebox::NativeNoteboxKind;
use solomon_sim::native_ui_text::{
    NativeUiFontName as Font, NativeUiTextAlign as Align, NativeUiTextSpec as TextSpec,
    layout_native_ui_text_runs, wrap_native_ui_text_runs,
};
use solomon_sim::player_panels::dialogue::{hub_chat_scroll_frame, hub_chat_text_runs};
use solomon_sim::profile::NATIVE_SKILL_CATALOG;
impl PlayerPanels {
    pub fn show_dialogue(
        &mut self,
        context: HubDialogueContext,
        npc: &NativeHubNpcState,
        now: f64,
    ) -> Result<(), String> {
        let content = create_hub_npc_chat_content(
            context.interaction,
            npc,
            context.random_index,
            context.eulogy_override,
            context.story_office,
            &self.dialogue_history,
        );
        self.show(PlayerPanel::Dialogue, now)?;
        if let HubNpcChatContent::Speech { key, .. } = &content {
            consume_hub_npc_speech(
                &mut self.dialogue_history,
                context.interaction,
                key,
                context.story_office,
            );
        }
        self.hovered_npc_selector = None;
        self.dialogue = Some(DialogueState {
            context,
            content,
            started_at: now,
            accelerated_at: None,
            scroll: 0.0,
            pending_selection: None,
            pending_response: None,
            college_intro_pending_ack: false,
        });
        self.update_chat_audio();
        self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::AcknowledgeNpcHint {
            interaction_id: context.interaction,
        }));
        Ok(())
    }
    pub fn show_college_intro_dialogue(
        &mut self,
        context: HubDialogueContext,
        npc: &NativeHubNpcState,
        now: f64,
    ) -> Result<(), String> {
        self.show_dialogue(context, npc, now)?;
        self.dialogue.as_mut().expect("opened College dialogue").college_intro_pending_ack = true;
        Ok(())
    }
    pub(super) fn acknowledge_college_dialogue(&mut self) {
        if let Some(dialogue) = &mut self.dialogue
            && dialogue.college_intro_pending_ack
        {
            // Source acknowledges the first choice or closing the College-owned surface,
            // rather than the scrolling speech's transition into its Choices page.
            dialogue.college_intro_pending_ack = false;
            self.actions.push(PlayerPanelAction::AcknowledgeCollegeIntroDialogue);
        }
    }
    fn begin_chat_content(&mut self, content: HubNpcChatContent, now: f64) {
        self.hovered_npc_selector = None;
        if let Some(dialogue) = &mut self.dialogue {
            if let HubNpcChatContent::Speech { key, .. } = &content {
                consume_hub_npc_speech(
                    &mut self.dialogue_history,
                    dialogue.context.interaction,
                    key,
                    dialogue.context.story_office,
                );
            }
            dialogue.content = content;
            dialogue.started_at = now;
            dialogue.accelerated_at = None;
            dialogue.scroll = 0.0;
            dialogue.pending_selection = None;
            dialogue.pending_response = None;
            self.selected = None;
        }
        self.update_chat_audio();
    }
    fn update_chat_audio(&mut self) {
        self.audio_events.push(AudioEvent::StopStream(AudioCue::ArchIntro0));
        if self.dialogue.as_ref().is_some_and(|dialogue| {
            matches!(&dialogue.content, HubNpcChatContent::Speech { key, .. } if key == "ARCH_INTRO_0")
        }) {
            self.sound(AudioCue::ArchIntro0);
        }
    }
    pub(super) fn activate_dialogue(
        &mut self,
        target: Target,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
    ) -> Result<(), String> {
        if let Target::DialogueSelector(id) = target {
            return self.select_npc(HubNpcSelectorId::Native(id), snapshot, now);
        }
        let Some(dialogue) = &self.dialogue else {
            return Ok(());
        };
        if target == Target::DialogueAccelerate {
            let dialogue = self.dialogue.as_mut().expect("open dialogue");
            if matches!(dialogue.content, HubNpcChatContent::Speech { .. })
                && dialogue.accelerated_at.is_none()
            {
                dialogue.accelerated_at = Some(now);
            }
            return Ok(());
        }
        let context = dialogue.context;
        if target != Target::DialogueNext
            || !matches!(dialogue.content, HubNpcChatContent::Speech { .. })
        {
            self.sound(AudioCue::Click);
        }
        let dialogue = self.dialogue.as_ref().expect("open dialogue");
        match target {
            Target::DialogueNext => {
                if let HubNpcChatContent::Speech { next, .. } = dialogue.content {
                    match next {
                        HubNpcChatNext::Choices => {
                            self.begin_chat_content(HubNpcChatContent::Choices, now);
                        }
                        HubNpcChatNext::Close => self.close_dialogue_response(now)?,
                    }
                } else {
                    self.hide()?;
                    self.actions.push(PlayerPanelAction::Close);
                }
            }
            Target::SelectorDone => self.begin_chat_content(HubNpcChatContent::Choices, now),
            Target::DialogueChoice(index) => {
                if dialogue.content != HubNpcChatContent::Choices {
                    return Ok(());
                }
                let choices = hub_npc_chat_choices(
                    context.interaction,
                    context.story_office,
                    &self.dialogue_history,
                );
                let Some(choice) = choices.get(index) else {
                    return Ok(());
                };
                self.acknowledge_college_dialogue();
                match choice {
                    HubNpcChatChoice::Command { selector, .. } => {
                        if let Some(trader) = selector.trader() {
                            self.show_service(trader, now)?;
                        } else if let Some(selector) = selector.list_selector() {
                            self.begin_chat_content(HubNpcChatContent::Selector(selector), now);
                        }
                    }
                    HubNpcChatChoice::Question { key, .. } => {
                        if let Some(content) = hub_npc_question(
                            context.interaction,
                            key,
                            context.story_office,
                            &self.dialogue_history,
                        ) {
                            self.begin_chat_content(content, now);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn select_npc(
        &mut self,
        id: HubNpcSelectorId,
        snapshot: &PlayerPanelSnapshot<'_>,
        _now: f64,
    ) -> Result<(), String> {
        let Some(dialogue) = &self.dialogue else { return Ok(()) };
        let HubNpcChatContent::Selector(selector) = dialogue.content else { return Ok(()) };
        if dialogue.pending_selection.is_some() || self.notice.is_some() {
            return Ok(());
        }
        // The event retains the clicked namespaced identity. Reordered content
        // cannot turn an old row index into a different accepted Boast.
        let rows = hub_npc_selector_rows_with_mods(
            selector,
            &snapshot.economy.npc,
            &snapshot.book.advanced_unlocks,
            snapshot.mod_content,
        );
        if !rows.iter().any(|row| row.id == id) {
            return Ok(());
        }
        let Some(action) = hub_npc_selector_action_with_mods(selector, &id) else { return Ok(()) };
        self.sound(AudioCue::Click);
        self.dialogue.as_mut().expect("open selector").pending_selection = Some((selector, id));
        self.actions.push(PlayerPanelAction::Inventory(action));

        Ok(())
    }
    pub(super) fn accept_selector_action(
        &mut self,
        action: &HubInventoryAction,
        now: f64,
        mods: Option<&ModContentProjection>,
    ) -> bool {
        let pending =
            self.dialogue.as_ref().and_then(|dialogue| dialogue.pending_selection.clone());
        let Some((selector, id)) = pending else {
            return false;
        };
        if hub_npc_selector_action_with_mods(selector, &id).as_ref() != Some(action) {
            return false;
        }
        self.sound(if matches!(action, HubInventoryAction::BuyTeacherSpell { .. }) {
            AudioCue::DropCoins
        } else {
            AudioCue::PickSkill
        });
        if let Some(content) = hub_npc_selector_response_with_mods(selector, &id, mods) {
            // The native selector keeps its accepted highlight for 100 ticks before
            // presenting the response. A rejected transaction never starts this clock.
            self.dialogue.as_mut().expect("accepted selector").pending_response =
                Some((now + 1000.0, content));
        } else if let Some(dialogue) = &mut self.dialogue {
            dialogue.pending_selection = None;
        }
        true
    }
    pub(super) fn back_dialogue(&mut self, now: f64) -> Result<(), String> {
        if self.dialogue.is_none() {
            return Ok(());
        }
        self.sound(AudioCue::Click);
        let dialogue = self.dialogue.as_ref().expect("open dialogue");
        match dialogue.content {
            HubNpcChatContent::Selector(_) => {
                self.begin_chat_content(HubNpcChatContent::Choices, now);
            }
            HubNpcChatContent::Speech { next: HubNpcChatNext::Close, .. } => {
                self.close_dialogue_response(now)?
            }
            _ => {
                self.hide()?;
                self.actions.push(PlayerPanelAction::Close);
            }
        }
        Ok(())
    }
    fn activate_dialogue_next(&mut self, now: f64) -> Result<(), String> {
        let Some(dialogue) = &self.dialogue else {
            return Ok(());
        };
        let HubNpcChatContent::Speech { next, .. } = dialogue.content else {
            return Ok(());
        };
        match next {
            HubNpcChatNext::Choices => self.begin_chat_content(HubNpcChatContent::Choices, now),
            HubNpcChatNext::Close => self.close_dialogue_response(now)?,
        }
        Ok(())
    }
    fn close_dialogue_response(&mut self, now: f64) -> Result<(), String> {
        let boast_response = self.dialogue.as_ref().is_some_and(|dialogue| {
            matches!(&dialogue.content, HubNpcChatContent::Speech { key, .. }
                if key.starts_with("ANNAL_") || key.starts_with("MOD_BOAST_"))
        });
        if boast_response && let Some(instruction) = &self.boast_instruction {
            self.npc_notebox.show(&self.ui, NativeNoteboxKind::Instruction, instruction, now)?;
        }
        self.hide()?;
        self.actions.push(PlayerPanelAction::Close);
        Ok(())
    }
    pub(super) fn scroll_selector(&mut self, delta: f64) {
        // Native SwipeBox uses one 25-pixel step per wheel direction, independent
        // of device-specific wheel magnitudes.
        self.scroll_selector_pixels(delta.signum() * 25.0);
    }
    pub(super) fn update_dialogue(&mut self, now: f64) -> Result<(), String> {
        if let Some(dialogue) = &mut self.dialogue
            && dialogue.pending_response.as_ref().is_some_and(|(at, _)| now >= *at)
        {
            let (_, content) =
                dialogue.pending_response.take().expect("accepted selector response");
            self.begin_chat_content(content, now);
            return Ok(());
        }
        if let Some(dialogue) = &self.dialogue
            && let HubNpcChatContent::Speech { lines, .. } = &dialogue.content
        {
            let height = chat_content_height(lines);
            let frame =
                hub_chat_scroll_frame(dialogue.started_at, dialogue.accelerated_at, now, height);
            if frame.complete {
                self.activate_dialogue_next(now)?;
            }
        }
        Ok(())
    }
    pub(super) fn scroll_selector_pixels(&mut self, delta: f64) {
        if let Some(dialogue) = &mut self.dialogue
            && matches!(dialogue.content, HubNpcChatContent::Selector(_))
        {
            dialogue.scroll =
                (dialogue.scroll + delta).clamp(0.0, (self.selector_height - 400.0).max(0.0));
        }
    }
    pub(super) fn reveal_selector_focus(&mut self, id: &HubNpcSelectorId) -> Result<(), String> {
        if let Some(hit) = self.hits.iter().find(|hit| hit.npc.as_ref() == Some(id)) {
            let delta = if hit.rect.top() < 106.0 {
                hit.rect.top() - 106.0
            } else if hit.rect.bottom() > 506.0 {
                hit.rect.bottom() - 506.0
            } else {
                0.0
            };
            self.scroll_selector_pixels(delta as f64);
        }
        Ok(())
    }
    pub(super) fn draw_dialogue(
        &self,
        c: &PanelCanvas<'_, '_>,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let dialogue = self.dialogue.as_ref().expect("dialogue panel state");
        if let HubNpcChatContent::Selector(selector) = dialogue.content {
            return self.draw_selector(c, selector, s);
        }
        c.native_frame(&self.ui, 11, HudRect::new(476.5, 26.0, 647.0, 420.0), true, 0xffffff)?;
        let def = dialogue.context.interaction.definition(dialogue.context.story_office);
        c.text(&TextSpec {
            tint: 0xd9ba70,
            ..TextSpec::new(Font::Menu, &def.name.to_uppercase(), 800.0, 90.0)
        });
        c.semantic("panel:dialogue", def.name, "dialog", HudRect::new(476.5, 26.0, 647.0, 420.0));
        let viewport = c.at(561.5, 111.0).clip(HudRect::new(0.0, 0.0, 477.0, 250.0));
        match &dialogue.content {
            HubNpcChatContent::Choices => {
                let choices = hub_npc_chat_choices(
                    dialogue.context.interaction,
                    dialogue.context.story_office,
                    &self.dialogue_history,
                );
                let height = (250.0 / choices.len().max(1) as f64).min(52.0);
                for (index, choice) in choices.iter().enumerate() {
                    let (label, command) = match choice {
                        HubNpcChatChoice::Command { label, .. } => (*label, true),
                        HubNpcChatChoice::Question { label, .. } => (*label, false),
                    };
                    let baseline = 54.0 + index as f64 * height;
                    let action = viewport.button(
                        &format!("dialogue-choice:{index}"),
                        label,
                        HudRect::new(0.0, baseline - height + 8.0, 477.0, height),
                    );
                    action.text(&TextSpec {
                        tint: if command { 0x8cbf8c } else { 0xd9ba70 },
                        scale: if command { 1.25 } else { 1.0 },
                        ..TextSpec::new(Font::Menu, label, 238.5, height - 8.0)
                    });
                }
            }
            HubNpcChatContent::Speech { lines, key, .. } => {
                let height = chat_content_height(lines);
                let scroll = hub_chat_scroll_frame(
                    dialogue.started_at,
                    dialogue.accelerated_at,
                    self.now,
                    height,
                );
                let copy = viewport.at(0.0, scroll.content_y);
                let mut y = 0.0;
                for line in lines {
                    let runs = hub_chat_text_runs(line);
                    let wrapped = wrap_native_ui_text_runs(&runs, Font::Menu, 477.0);
                    for (index, line) in wrapped.iter().enumerate() {
                        copy.layout(&layout_native_ui_text_runs(
                            Font::Menu,
                            line,
                            0.0,
                            y + index as f64 * 27.0,
                            0xd9ba70,
                        ));
                    }
                    y += wrapped.len() as f64 * 27.0 + 22.0;
                }
                c.button(
                    "dialogue-accelerate",
                    "Accelerate dialogue",
                    HudRect::new(561.5, 111.0, 477.0, 250.0),
                );
                c.semantic_value(
                    "panel:dialogue-key",
                    &lines.join("\n\n").replace('*', ""),
                    "text",
                    HudRect::new(561.5, 111.0, 477.0, 250.0),
                    key,
                );
            }
            HubNpcChatContent::Selector(_) => unreachable!(),
        }
        let speech = matches!(dialogue.content, HubNpcChatContent::Speech { .. });
        let done = c.button(
            "dialogue-next",
            if speech { "Skip" } else { "Done" },
            HudRect::new(730.0, 370.0, 140.0, 45.0),
        );
        done.text(&TextSpec {
            tint: 0xd9ba70,
            ..TextSpec::new(Font::Menu, if speech { "Skip" } else { "Done" }, 70.0, 26.0)
        });
        Ok(())
    }
    fn draw_selector(
        &self,
        c: &PanelCanvas<'_, '_>,
        selector: HubNpcListSelector,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        c.native_frame(&self.ui, 11, HudRect::new(450.0, 26.0, 700.0, 560.0), true, 0xffffff)?;
        let title = hub_npc_selector_title(selector);
        c.text(&TextSpec { tint: 0xd9ba70, ..TextSpec::new(Font::Menu, title, 800.0, 90.0) });
        c.semantic("panel:dialogue", title, "dialog", HudRect::new(450.0, 26.0, 700.0, 560.0));
        let dialogue = self.dialogue.as_ref().expect("selector state");
        let viewport = c
            .button("selector-scroll", "Scroll choices", HudRect::new(540.0, 106.0, 520.0, 400.0))
            .clip(HudRect::new(0.0, 0.0, 520.0, 400.0));
        let copy = viewport.at(0.0, -dialogue.scroll);
        let rows = hub_npc_selector_rows_with_mods(
            selector,
            &s.economy.npc,
            &s.book.advanced_unlocks,
            s.mod_content,
        );
        if rows.is_empty() {
            c.text(&TextSpec {
                tint: 0xd9ba70,
                ..TextSpec::new(
                    Font::Menu,
                    if selector == HubNpcListSelector::TeacherSpells {
                        "ALL SPELLS\nALREADY BOUGHT!"
                    } else {
                        "NO ENTRIES"
                    },
                    800.0,
                    306.0,
                )
            });
        }
        for (index, row) in rows.iter().enumerate() {
            let action = match &row.id {
                HubNpcSelectorId::Native(id) => format!("dialogue-selector:{id}"),
                HubNpcSelectorId::ModBoast(_) => "selector-scroll".into(),
            };
            let node = copy.npc_button(
                &action,
                &row.label,
                HudRect::new(15.0, 25.0 + index as f64 * 90.0, 490.0, 85.0),
                Some(row.id.clone()),
            );
            let active = self
                .hovered_npc_selector
                .as_ref()
                .or_else(|| dialogue.pending_selection.as_ref().map(|(_, id)| id));
            let selected = active == Some(&row.id);
            let color = if selected { 0x9feb9f } else { 0xd9ba70 };
            node.native_frame(
                &self.ui,
                50,
                HudRect::new(0.0, 0.0, 490.0, 85.0),
                true,
                if selected { 0x9feb9f } else { 0xffffff },
            )?;
            match selector {
                HubNpcListSelector::Boast => {
                    if let Some(PreparedModBoastIcon::Stock { record, .. }) = &row.boast_icon {
                        let width = self.ui.record(*record)?.logical_width as f64;
                        for (x, sx) in [(15.0 + width / 2.0, 1.0), (475.0 - width / 2.0, -1.0)] {
                            node.art(
                                &self.ui,
                                *record,
                                [x, 42.5],
                                [sx, 1.0],
                                0.0,
                                if selected { color } else { 0xffffff },
                            )?;
                        }
                    }
                    if let Some(PreparedModBoastIcon::Mod { frame, image_path, .. }) =
                        &row.boast_icon
                        && let HubNpcSelectorId::ModBoast(selection) = &row.id
                    {
                        let (url, dimensions) = self
                            .mod_boast_images
                            .get(&format!("{}:{image_path}", selection.mod_id))
                            .ok_or("Mod Boast image is not installed")?;
                        for (x, mirror) in [
                            (15.0 + frame.logical_width / 2.0, false),
                            (475.0 - frame.logical_width / 2.0, true),
                        ] {
                            node.mod_frame(
                                url,
                                *dimensions,
                                *frame,
                                [x, 42.5],
                                mirror,
                                if selected { color } else { 0xffffff },
                            );
                        }
                    }
                    node.text(&TextSpec {
                        tint: color,
                        ..TextSpec::new(Font::SpecialUppercase, &row.label, 245.0, 27.5)
                    });
                    node.text(&TextSpec {
                        tint: color,
                        max_width: Some(370.0),
                        line_height: Some(17.0),
                        ..TextSpec::new(Font::Medium, &row.detail, 245.0, 47.5)
                    });
                }
                HubNpcListSelector::Books => {
                    let record = book_art_record(&row.label);
                    let width = self.library.record(record)?.logical_width as f64;
                    node.art(
                        &self.library,
                        record,
                        [15.0 + width / 2.0, 42.5],
                        [1.0, 1.0],
                        0.0,
                        color,
                    )?;
                    node.text(&TextSpec {
                        align: Align::Left,
                        tint: color,
                        max_width: Some(375.0),
                        line_height: Some(22.0),
                        ..TextSpec::new(
                            Font::SpecialUppercase,
                            &row.label.to_uppercase(),
                            100.0,
                            27.5,
                        )
                    });
                }
                HubNpcListSelector::TeacherSpells => {
                    let HubNpcSelectorId::Native(id) = &row.id else {
                        unreachable!("native teacher skills")
                    };
                    let root = solomon_sim::profile::native_skill_root(*id as usize)
                        .expect("teacher root");
                    node.art(
                        &self.skills,
                        164,
                        [43.0, 42.5],
                        [1.0, 1.0],
                        0.0,
                        solomon_sim::skill_picker::SKILL_PICKER_ROOT_TINTS[root],
                    )?;
                    node.art(&self.skills, 5, [43.0, 42.5], [0.75, 0.75], 0.0, 0xffffff)?;
                    node.art(
                        &self.skills,
                        NATIVE_SKILL_CATALOG[*id as usize].icon_record,
                        [43.0, 42.5],
                        [1.0, 1.0],
                        0.0,
                        0xffffff,
                    )?;
                    node.text(&TextSpec {
                        align: Align::Left,
                        tint: color,
                        ..TextSpec::new(Font::SpecialUppercase, &row.label, 90.0, 31.0)
                    });
                    node.text(&TextSpec {
                        align: Align::Left,
                        tint: color,
                        scale: 0.75,
                        max_width: Some(333.0),
                        line_height: Some(68.0 / 3.0),
                        ..TextSpec::new(Font::Medium, &row.detail.to_uppercase(), 90.0, 49.0)
                    });
                    if let Some(price) = row.price {
                        node.text(&TextSpec {
                            align: Align::Right,
                            tint: if price > s.economy.gold { 0xff6680 } else { 0xd9ba70 },
                            ..TextSpec::new(Font::Body, &price.to_string(), 487.0, 80.0)
                        });
                    }
                }
            }
        }
        if selector != HubNpcListSelector::Books {
            c.art(&self.ui, 21, [570.0, 539.0], [1.0, 1.0], 0.0, 0xffffff)?;
            c.text(&TextSpec {
                align: Align::Left,
                ..TextSpec::new(Font::Body, &s.economy.gold.to_string(), 580.0, 555.0)
            });
        }
        let done = c.button("selector-done", "Done", HudRect::new(700.0, 511.0, 200.0, 40.0));
        done.text(&TextSpec { tint: 0xd9ba70, ..TextSpec::new(Font::Menu, "DONE", 100.0, 25.0) });
        Ok(())
    }
}
fn chat_content_height(lines: &[String]) -> f64 {
    lines
        .iter()
        .map(|line| {
            wrap_native_ui_text_runs(&hub_chat_text_runs(line), Font::Menu, 477.0).len() as f64
                * 27.0
                + 22.0
        })
        .sum()
}
fn book_art_record(title: &str) -> usize {
    // Preserve JavaScript's UTF-16 indexing and truncation used by hubNpcBookArtRecord.
    let mut hash = 1_u64;
    for (index, code) in title.encode_utf16().enumerate() {
        if index % 2 == 0 {
            hash *= u64::from(code);
        } else {
            hash += u64::from(code);
        }
        if hash > 2_000_000 {
            hash /= index as u64;
        }
        if hash == 0 {
            hash = u64::from(code);
        }
    }
    13 + (hash % 4) as usize
}
