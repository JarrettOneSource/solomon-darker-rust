//! College chat and its embedded selectors. The simulation owns every authored line,
//! price, unlock and action; this module owns only the current reading position.

use solomon_sim::Vec2;
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
use wasm_bindgen::JsCast;

use super::chrome::{art, bitmap_text, button, place, tint};
use super::{
    DialogueState, HubDialogueContext, PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot,
    PlayerPanels, Target,
};
use crate::ui::{append, element, mod_icon::mod_icon, text::paint_text_layout};

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

    /// The session calls this only for its accepted Arch-dialogue sequence. Ordinary
    /// Office conversations never acquire the College acknowledgement side effect.
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
            self.dirty = true;
            self.navigation.release();
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
        self.dirty = true;
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

    pub(super) fn update_dialogue(&mut self, now: f64) -> Result<(), String> {
        if let Some(dialogue) = &mut self.dialogue
            && dialogue.pending_response.as_ref().is_some_and(|(at, _)| now >= *at)
        {
            let (_, content) = dialogue.pending_response.take().expect("ready selector response");
            self.begin_chat_content(content, now);
            return Ok(());
        }
        let Some(dialogue) = &self.dialogue else {
            return Ok(());
        };
        if !matches!(dialogue.content, HubNpcChatContent::Speech { .. }) {
            self.stage
                .set_attribute("data-native-chat-state", "choices")
                .map_err(|_| "chat choices progress")?;
            self.stage
                .remove_attribute("data-native-chat-accelerated")
                .map_err(|_| "chat choices speed")?;
            return Ok(());
        }
        let Some(copy) =
            self.content.query_selector(".panel-chat-copy").map_err(|_| "chat copy")?
        else {
            return Ok(());
        };
        let height = copy
            .get_attribute("data-content-height")
            .ok_or("chat content height")?
            .parse::<f64>()
            .map_err(|_| "chat content height value")?;
        let frame =
            hub_chat_scroll_frame(dialogue.started_at, dialogue.accelerated_at, now, height);
        copy.set_attribute(
            "style",
            &format!("height:{height}px;transform:translateY({}px)", frame.content_y),
        )
        .map_err(|_| "chat scroll")?;
        self.stage
            .set_attribute(
                "data-native-chat-state",
                if frame.complete { "complete" } else { "scrolling" },
            )
            .map_err(|_| "chat progress")?;
        self.stage
            .set_attribute(
                "data-native-chat-accelerated",
                if dialogue.accelerated_at.is_some() { "true" } else { "false" },
            )
            .map_err(|_| "chat speed")?;
        if frame.complete {
            // Completion is the authored text's native wrap height crossing the clip.
            self.activate_dialogue_next(now)?;
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

    pub(super) fn scroll_selector_pixels(&mut self, delta: f64) {
        let Some(dialogue) = &mut self.dialogue else {
            return;
        };
        if matches!(dialogue.content, HubNpcChatContent::Selector(_)) {
            let maximum = self
                .content
                .query_selector(".panel-selector-copy")
                .ok()
                .flatten()
                .map_or(0.0, |copy| (f64::from(copy.scroll_height()) - 400.0).max(0.0));
            dialogue.scroll = (dialogue.scroll + delta).clamp(0.0, maximum);
            self.dirty = true;
        }
    }

    pub(super) fn reveal_selector_focus(&mut self, id: &HubNpcSelectorId) -> Result<(), String> {
        let nodes = self
            .content
            .query_selector_all("[data-npc-selector]")
            .map_err(|_| "selector focus rows")?;
        let mut selected = None;
        for index in 0..nodes.length() {
            if let Some(node) = nodes.item(index)
                && let Ok(node) = node.dyn_into::<web_sys::Element>()
                && super::npc_selector_from_element(&node).as_ref() == Some(id)
            {
                selected = Some(node);
                break;
            }
        }
        let Some(node) = selected else { return Ok(()) };
        let Some(viewport) = self
            .content
            .query_selector(".panel-selector-viewport")
            .map_err(|_| "selector focus viewport")?
        else {
            return Ok(());
        };
        let row = node.get_bounding_client_rect();
        let clip = viewport.get_bounding_client_rect();
        let delta = if row.top() < clip.top() {
            row.top() - clip.top()
        } else if row.bottom() > clip.bottom() {
            row.bottom() - clip.bottom()
        } else {
            0.0
        };
        if delta != 0.0 {
            self.scroll_selector_pixels(
                delta * 1600.0 / self.stage.get_bounding_client_rect().width(),
            );
        }
        Ok(())
    }

    pub(super) fn draw_dialogue(&self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        let dialogue = self.dialogue.as_ref().expect("dialogue panel has reading state");
        if let HubNpcChatContent::Selector(selector) = dialogue.content {
            return self.draw_selector(selector, snapshot);
        }
        let frame = self.ui.nine_slice(&self.document, 11, 647.0, 420.0)?;
        place(&frame, HudRect::new(476.5, 26.0, 647.0, 420.0))?;
        append(&self.content, &frame)?;
        let definition = dialogue.context.interaction.definition(dialogue.context.story_office);
        self.stage.set_attribute("aria-label", definition.name).map_err(|_| "chat title")?;
        bitmap_text(
            &self.document,
            &self.content,
            &TextSpec {
                tint: 0xd9ba70,
                ..TextSpec::new(Font::Menu, &definition.name.to_uppercase(), 800.0, 90.0)
            },
        )?;
        let viewport = element(&self.document, "div", "panel-chat-viewport")?;
        place(&viewport, HudRect::new(561.5, 111.0, 477.0, 250.0))?;
        append(&self.content, &viewport)?;
        match &dialogue.content {
            HubNpcChatContent::Choices => {
                let choices = hub_npc_chat_choices(
                    dialogue.context.interaction,
                    dialogue.context.story_office,
                    &self.dialogue_history,
                );
                let row_height = (250.0 / choices.len().max(1) as f64).min(52.0);
                for (index, choice) in choices.iter().enumerate() {
                    let (label, command) = match choice {
                        HubNpcChatChoice::Command { label, .. } => (*label, true),
                        HubNpcChatChoice::Question { label, .. } => (*label, false),
                    };
                    let baseline = 54.0 + index as f64 * row_height;
                    let action = button(
                        &self.document,
                        &viewport,
                        &format!("dialogue-choice:{index}"),
                        label,
                        HudRect::new(0.0, baseline - row_height + 8.0, 477.0, row_height),
                    )?;
                    bitmap_text(
                        &self.document,
                        &action,
                        &TextSpec {
                            tint: if command { 0x8cbf8c } else { 0xd9ba70 },
                            scale: if command { 1.25 } else { 1.0 },
                            ..TextSpec::new(Font::Menu, label, 238.5, row_height - 8.0)
                        },
                    )?;
                }
            }
            HubNpcChatContent::Speech { lines, key, .. } => {
                viewport.set_attribute("data-dialogue-key", key).map_err(|_| "dialogue key")?;
                let copy = element(&self.document, "div", "panel-chat-copy")?;
                let mut content_height = 0.0;
                for line in lines {
                    let runs = hub_chat_text_runs(line);
                    let wrapped = wrap_native_ui_text_runs(&runs, Font::Menu, 477.0);
                    for (index, line) in wrapped.iter().enumerate() {
                        let node = element(&self.document, "span", "panel-chat-line")?;
                        node.set_attribute(
                            "style",
                            "position:absolute;inset:0;pointer-events:none",
                        )
                        .map_err(|_| "chat glyph pen")?;
                        let layout = layout_native_ui_text_runs(
                            Font::Menu,
                            line,
                            0.0,
                            content_height + index as f64 * 27.0,
                            0xd9ba70,
                        );
                        paint_text_layout(&self.document, &node, &layout)?;
                        append(&copy, &node)?;
                    }
                    content_height += wrapped.len() as f64 * 27.0 + 22.0;
                }
                copy.set_attribute("data-content-height", &content_height.to_string())
                    .map_err(|_| "chat native content height")?;
                copy.set_attribute("aria-label", &lines.join("\n\n").replace('*', ""))
                    .map_err(|_| "chat semantic text")?;
                append(&viewport, &copy)?;
                button(
                    &self.document,
                    &self.content,
                    "dialogue-accelerate",
                    "Accelerate dialogue",
                    HudRect::new(561.5, 111.0, 477.0, 250.0),
                )?;
            }
            HubNpcChatContent::Selector(_) => unreachable!(),
        }
        let speech = matches!(dialogue.content, HubNpcChatContent::Speech { .. });
        let done = button(
            &self.document,
            &self.content,
            "dialogue-next",
            if speech { "Skip" } else { "Done" },
            HudRect::new(730.0, 370.0, 140.0, 45.0),
        )?;
        bitmap_text(
            &self.document,
            &done,
            &TextSpec {
                tint: 0xd9ba70,
                ..TextSpec::new(Font::Menu, if speech { "Skip" } else { "Done" }, 70.0, 26.0)
            },
        )?;
        Ok(())
    }

    fn draw_selector(
        &self,
        selector: HubNpcListSelector,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        // Native Boast's outer box also hosts the book and teacher selectors.
        let frame = self.ui.nine_slice(&self.document, 11, 700.0, 560.0)?;
        place(&frame, HudRect::new(450.0, 26.0, 700.0, 560.0))?;
        append(&self.content, &frame)?;
        let title = hub_npc_selector_title(selector);
        self.stage.set_attribute("aria-label", title).map_err(|_| "selector title")?;
        bitmap_text(
            &self.document,
            &self.content,
            &TextSpec { tint: 0xd9ba70, ..TextSpec::new(Font::Menu, title, 800.0, 90.0) },
        )?;
        let viewport = element(&self.document, "div", "panel-selector-viewport")?;
        viewport
            .set_attribute("data-panel-action", "selector-scroll")
            .map_err(|_| "selector swipe target")?;
        place(&viewport, HudRect::new(540.0, 106.0, 520.0, 400.0))?;
        append(&self.content, &viewport)?;
        let copy = element(&self.document, "div", "panel-selector-copy")?;
        let dialogue = self.dialogue.as_ref().expect("open selector");
        copy.style()
            .set_property("transform", &format!("translateY(-{}px)", dialogue.scroll))
            .map_err(|_| "selector scroll")?;
        let rows = hub_npc_selector_rows_with_mods(
            selector,
            &snapshot.economy.npc,
            &snapshot.book.advanced_unlocks,
            snapshot.mod_content,
        );
        copy.style()
            .set_property(
                "height",
                &format!("{}px", if rows.is_empty() { 0 } else { 50 + (rows.len() - 1) * 90 + 85 }),
            )
            .map_err(|_| "selector content height")?;
        append(&viewport, &copy)?;
        if rows.is_empty() {
            bitmap_text(
                &self.document,
                &self.content,
                &TextSpec {
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
                },
            )?;
        }
        for (index, row) in rows.iter().enumerate() {
            let node = button(
                &self.document,
                &copy,
                &match &row.id {
                    HubNpcSelectorId::Native(id) => format!("dialogue-selector:{id}"),
                    HubNpcSelectorId::ModBoast(_) => "selector-scroll".to_owned(),
                },
                &row.label,
                HudRect::new(15.0, 25.0 + index as f64 * 90.0, 490.0, 85.0),
            )?;
            match &row.id {
                HubNpcSelectorId::Native(id) => node
                    .set_attribute("data-npc-selector", &id.to_string())
                    .map_err(|_| "native selector identity")?,
                HubNpcSelectorId::ModBoast(selection) => {
                    node.set_attribute("data-npc-selector", &selection.content_id)
                        .map_err(|_| "mod Boast identity")?;
                    node.set_attribute("data-npc-mod", &selection.mod_id)
                        .map_err(|_| "mod Boast owner")?;
                }
            }
            let frame = self.ui.nine_slice(&self.document, 50, 490.0, 85.0)?;
            let active = self
                .hovered_npc_selector
                .as_ref()
                .or_else(|| dialogue.pending_selection.as_ref().map(|(_, id)| id));
            let selected = active == Some(&row.id);
            if selected {
                tint(&self.document, &frame, 0x9feb9f)?;
            }
            append(&node, &frame)?;
            let color = if selected { 0x9feb9f } else { 0xd9ba70 };
            match selector {
                HubNpcListSelector::Boast => {
                    if let Some(PreparedModBoastIcon::Stock { record, .. }) = &row.boast_icon {
                        let record = *record;
                        let width = f64::from(self.ui.record(record)?.logical_width);
                        let left = art(
                            &self.document,
                            &node,
                            &self.ui,
                            record,
                            [15.0 + width / 2.0, 42.5],
                            [1.0, 1.0],
                            0.0,
                        )?;
                        let right = art(
                            &self.document,
                            &node,
                            &self.ui,
                            record,
                            [475.0 - width / 2.0, 42.5],
                            [-1.0, 1.0],
                            0.0,
                        )?;
                        if selected {
                            tint(&self.document, &left, color)?;
                            tint(&self.document, &right, color)?;
                        }
                    }
                    if let Some(PreparedModBoastIcon::Mod { frame, image_path, .. }) =
                        &row.boast_icon
                        && let HubNpcSelectorId::ModBoast(selection) = &row.id
                    {
                        let images =
                            snapshot.mod_assets.ok_or("Mod Boast images are not installed")?;
                        for (center, mirrored) in [
                            (15.0 + frame.logical_width / 2.0, false),
                            (475.0 - frame.logical_width / 2.0, true),
                        ] {
                            let icon = mod_icon(
                                &self.document,
                                images,
                                &selection.mod_id,
                                image_path,
                                *frame,
                                Vec2::new(center, 42.5),
                                mirrored,
                            )?;
                            if selected {
                                tint(&self.document, &icon, 0x9feb9f)?;
                            }
                            append(&node, &icon)?;
                        }
                    }
                    bitmap_text(
                        &self.document,
                        &node,
                        &TextSpec {
                            tint: color,
                            ..TextSpec::new(Font::SpecialUppercase, &row.label, 245.0, 27.5)
                        },
                    )?;
                    bitmap_text(
                        &self.document,
                        &node,
                        &TextSpec {
                            tint: color,
                            max_width: Some(370.0),
                            line_height: Some(17.0),
                            ..TextSpec::new(Font::Medium, &row.detail, 245.0, 47.5)
                        },
                    )?;
                }
                HubNpcListSelector::Books => {
                    let record = book_art_record(&row.label);
                    let width = f64::from(self.library.record(record)?.logical_width);
                    let icon = art(
                        &self.document,
                        &node,
                        &self.library,
                        record,
                        [15.0 + width / 2.0, 42.5],
                        [1.0, 1.0],
                        0.0,
                    )?;
                    tint(&self.document, &icon, color)?;
                    bitmap_text(
                        &self.document,
                        &node,
                        &TextSpec {
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
                        },
                    )?;
                }
                HubNpcListSelector::TeacherSpells => {
                    let HubNpcSelectorId::Native(id) = &row.id else {
                        unreachable!("teacher rows are native skills")
                    };
                    let backing = art(
                        &self.document,
                        &node,
                        &self.skills,
                        164,
                        [43.0, 42.5],
                        [1.0, 1.0],
                        0.0,
                    )?;
                    let root = solomon_sim::profile::native_skill_root(*id as usize)
                        .expect("teacher skill root");
                    tint(
                        &self.document,
                        &backing,
                        solomon_sim::skill_picker::SKILL_PICKER_ROOT_TINTS[root],
                    )?;
                    art(&self.document, &node, &self.skills, 5, [43.0, 42.5], [0.75, 0.75], 0.0)?;
                    art(
                        &self.document,
                        &node,
                        &self.skills,
                        NATIVE_SKILL_CATALOG[*id as usize].icon_record,
                        [43.0, 42.5],
                        [1.0, 1.0],
                        0.0,
                    )?;
                    bitmap_text(
                        &self.document,
                        &node,
                        &TextSpec {
                            align: Align::Left,
                            tint: color,
                            ..TextSpec::new(Font::SpecialUppercase, &row.label, 90.0, 31.0)
                        },
                    )?;
                    bitmap_text(
                        &self.document,
                        &node,
                        &TextSpec {
                            align: Align::Left,
                            tint: color,
                            scale: 0.75,
                            max_width: Some(333.0),
                            line_height: Some(68.0 / 3.0),
                            ..TextSpec::new(Font::Medium, &row.detail.to_uppercase(), 90.0, 49.0)
                        },
                    )?;
                    if let Some(price) = row.price {
                        bitmap_text(
                            &self.document,
                            &node,
                            &TextSpec {
                                align: Align::Right,
                                tint: if price > snapshot.economy.gold {
                                    0xff6680
                                } else {
                                    0xd9ba70
                                },
                                ..TextSpec::new(Font::Body, &price.to_string(), 487.0, 80.0)
                            },
                        )?;
                    }
                }
            }
        }
        if selector != HubNpcListSelector::Books {
            art(&self.document, &self.content, &self.ui, 21, [570.0, 539.0], [1.0, 1.0], 0.0)?;
            bitmap_text(
                &self.document,
                &self.content,
                &TextSpec {
                    align: Align::Left,
                    tint: 0xffffff,
                    ..TextSpec::new(Font::Body, &snapshot.economy.gold.to_string(), 580.0, 555.0)
                },
            )?;
        }
        let done = button(
            &self.document,
            &self.content,
            "selector-done",
            "Done",
            HudRect::new(700.0, 511.0, 200.0, 40.0),
        )?;
        bitmap_text(
            &self.document,
            &done,
            &TextSpec { tint: 0xd9ba70, ..TextSpec::new(Font::Menu, "DONE", 100.0, 25.0) },
        )?;
        Ok(())
    }
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
