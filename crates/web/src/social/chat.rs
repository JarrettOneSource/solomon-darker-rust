//! The retained native-browser composer and transcript from Website `GameChat.tsx`.

use solomon_sim::settings::game_binding_label;
use solomon_sim::social::{GameChatChannel, GameChatMessage};
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlElement, HtmlInputElement};

use super::{SocialUi, action_button, attribute, channel_id, channel_index};
use crate::ui::{append, element};

pub(super) struct ChatView {
    pub root: HtmlElement,
    panel: HtmlElement,
    header: HtmlElement,
    tabs: HtmlElement,
    pub messages: HtmlElement,
    status: HtmlElement,
    pub form: HtmlElement,
    pub input: HtmlInputElement,
    rune: HtmlElement,
    help: HtmlElement,
    pub opener: HtmlElement,
    key: HtmlElement,
    unread: HtmlElement,
    scrim: HtmlElement,
}

impl ChatView {
    pub fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "section", "game-chat")?;
        attribute(&root, "aria-label", "Game chat")?;
        let scrim = element(document, "div", "game-chat-scrim")?;
        attribute(&scrim, "data-social-action", "chat-close")?;
        append(&root, &scrim)?;
        let panel = element(document, "div", "game-chat-panel")?;
        let header = element(document, "header", "game-chat-header")?;
        let tabs = element(document, "div", "game-chat-tabs")?;
        attribute(&tabs, "role", "tablist")?;
        attribute(&tabs, "aria-label", "Chat channel")?;
        append(&header, &tabs)?;
        append(&header, &action_button(document, "×", "chat-close", "game-chat-close")?)?;
        append(&panel, &header)?;
        let messages = element(document, "ol", "game-chat-messages")?;
        attribute(&messages, "aria-live", "polite")?;
        attribute(&messages, "aria-atomic", "false")?;
        append(&panel, &messages)?;
        let status = element(document, "p", "game-chat-status")?;
        attribute(&status, "role", "status")?;
        append(&panel, &status)?;
        let form = element(document, "form", "game-chat-form")?;
        let rune = element(document, "span", "game-chat-rune")?;
        attribute(&rune, "aria-hidden", "true")?;
        append(&form, &rune)?;
        let input: HtmlInputElement = element(document, "input", "game-chat-input")?
            .dyn_into()
            .map_err(|_| "chat input element")?;
        input.set_max_length(180);
        for (name, value) in [
            ("aria-label", "Chat message"),
            ("autocomplete", "off"),
            ("autocapitalize", "sentences"),
            ("enterkeyhint", "send"),
            ("spellcheck", "true"),
        ] {
            input.set_attribute(name, value).map_err(|_| "chat input attribute")?;
        }
        form.append_child(&input).map_err(|_| "chat input attachment")?;
        let send = element(document, "button", "game-chat-send")?;
        attribute(&send, "type", "submit")?;
        send.set_text_content(Some("Send"));
        append(&form, &send)?;
        append(&panel, &form)?;
        let help = element(document, "p", "game-chat-help")?;
        help.set_text_content(Some("Enter send · Tab channel · Esc close"));
        append(&panel, &help)?;
        append(&root, &panel)?;
        let opener = action_button(document, "Open chat", "chat-open", "game-chat-open")?;
        // Trusted authored SVG from GameChat.tsx. Player text only uses textContent below.
        opener.set_inner_html("<svg aria-hidden='true' class='game-chat-open-icon' viewBox='0 0 24 24'><path d='M5 4.2h14a2.6 2.6 0 0 1 2.6 2.6v7.6A2.6 2.6 0 0 1 19 17h-6.4l-4.1 3.8V17H5a2.6 2.6 0 0 1-2.6-2.6V6.8A2.6 2.6 0 0 1 5 4.2Z'/><circle cx='8.1' cy='10.6' r='1.15'/><circle cx='12' cy='10.6' r='1.15'/><circle cx='15.9' cy='10.6' r='1.15'/></svg>");
        let key = element(document, "kbd", "")?;
        let unread = element(document, "span", "game-chat-open-unread")?;
        append(&opener, &key)?;
        append(&opener, &unread)?;
        append(&root, &opener)?;
        Ok(Self {
            root,
            panel,
            header,
            tabs,
            messages,
            status,
            form,
            input,
            rune,
            help,
            opener,
            key,
            unread,
            scrim,
        })
    }
}

impl SocialUi {
    pub(super) fn draw_chat(&self) -> Result<(), String> {
        let Some(snapshot) = &self.snapshot else {
            return Ok(());
        };
        let view = &self.chat;
        view.root.set_hidden(!self.context.chat_enabled || self.context.input_suspended);
        attribute(&view.root, "data-chat-open", &self.chat_open.to_string())?;
        attribute(&view.root, "data-chat-channel", channel_id(self.channel))?;
        let channels = self.channels();
        attribute(
            &view.root,
            "data-chat-channels",
            &channels.iter().map(|channel| channel_id(*channel)).collect::<Vec<_>>().join(","),
        )?;
        let unread: u16 = self.unread.iter().map(|value| u16::from(*value)).sum();
        attribute(&view.root, "data-chat-unread-total", &unread.to_string())?;
        view.root
            .style()
            .set_property(
                "--game-chat-gold-clear-left",
                &format!("{}px", self.context.gold_clear_left),
            )
            .map_err(|_| "chat gold clearance")?;
        view.root
            .style()
            .set_property("--game-chat-vvh", &format!("{}px", self.size.1))
            .map_err(|_| "chat viewport height")?;
        view.scrim.set_hidden(!self.chat_open);
        view.header.set_hidden(!self.chat_open);
        view.form.set_hidden(!self.chat_open);
        view.help.set_hidden(!self.chat_open);
        view.opener.set_hidden(self.chat_open);
        view.unread.set_hidden(unread == 0);
        view.unread.set_text_content(Some(&unread.to_string()));
        let key = game_binding_label(&self.controls.open_chat);
        attribute(&view.opener, "aria-keyshortcuts", &key)?;
        view.key.set_text_content(Some(&key));
        view.status.set_text_content(self.chat_status.as_deref());
        view.status.set_hidden(self.chat_status.is_none());
        view.tabs.set_text_content(None);
        for channel in channels {
            let button = action_button(
                &self.document,
                channel.label(),
                &format!("channel:{}", channel_id(channel)),
                "game-chat-tab",
            )?;
            attribute(&button, "role", "tab")?;
            attribute(&button, "data-channel", channel_id(channel))?;
            attribute(&button, "aria-selected", &(channel == self.channel).to_string())?;
            attribute(&button, "tabindex", if channel == self.channel { "0" } else { "-1" })?;
            let unread = self.unread[channel_index(channel)];
            if unread > 0 {
                let badge = element(&self.document, "span", "game-chat-unread")?;
                badge.set_text_content(Some(&unread.to_string()));
                append(&button, &badge)?;
            }
            append(&view.tabs, &button)?;
        }
        let placeholder = if self.channel == GameChatChannel::Whisper {
            format!("Whisper {}", self.whisper.as_ref().map_or("", |target| &target.display_name))
        } else {
            format!("Message {}", self.channel.label())
        };
        view.input.set_placeholder(&placeholder);
        attribute(&view.form, "aria-label", &format!("{} chat message", self.channel.label()))?;
        attribute(&view.rune, "data-channel", channel_id(self.channel))?;
        let pinned = view.messages.scroll_height()
            - view.messages.scroll_top()
            - view.messages.client_height()
            < 48;
        let scroll_top = view.messages.scroll_top();
        view.messages.set_text_content(None);
        let messages: Vec<_> = if self.chat_open {
            snapshot
                .chat_messages
                .iter()
                .filter(|message| message.channel == self.channel)
                .collect()
        } else {
            snapshot
                .chat_messages
                .iter()
                .skip(snapshot.chat_messages.len().saturating_sub(5))
                .collect()
        };
        if messages.is_empty() && self.chat_open {
            let empty = element(&self.document, "li", "game-chat-empty")?;
            empty.set_text_content(Some(&format!(
                "No {} messages yet.",
                self.channel.label().to_lowercase()
            )));
            append(&view.messages, &empty)?;
        }
        for message in messages {
            self.draw_message(message)?;
        }
        if self.chat_open {
            view.messages.set_scroll_top(if pinned {
                view.messages.scroll_height()
            } else {
                scroll_top
            });
        }
        view.panel.set_hidden(
            !self.chat_open && snapshot.chat_messages.is_empty() && self.chat_status.is_none(),
        );
        Ok(())
    }

    fn draw_message(&self, message: &GameChatMessage) -> Result<(), String> {
        let snapshot = self.snapshot.as_ref().expect("chat message has a host snapshot");
        let own = message.sender.player_id == snapshot.local_player_id;
        let row = element(&self.document, "li", "game-chat-message")?;
        attribute(&row, "data-message-channel", channel_id(message.channel))?;
        attribute(&row, "data-message-sequence", &message.sequence.to_string())?;
        attribute(&row, "data-sender-player-id", &message.sender.player_id)?;
        if let Some(recipient) = &message.recipient {
            attribute(&row, "data-recipient-player-id", &recipient.player_id)?;
        }
        if message.activity.is_some() {
            let activity = element(&self.document, "span", "game-chat-activity")?;
            append(
                &activity,
                &action_button(
                    &self.document,
                    &message.sender.display_name,
                    &format!("card:{}", message.sender.player_reference),
                    "game-chat-player-name",
                )?,
            )?;
            let suffix =
                message.text.strip_prefix(&message.sender.display_name).unwrap_or(&message.text);
            let text = element(&self.document, "span", "")?;
            text.set_text_content(Some(suffix));
            append(&activity, &text)?;
            append(&row, &activity)?;
        } else {
            let author = element(&self.document, "strong", "")?;
            let label = if message.channel == GameChatChannel::Whisper {
                if own {
                    format!(
                        "To {}",
                        message.recipient.as_ref().map_or("", |recipient| &recipient.display_name)
                    )
                } else {
                    format!("From {}", message.sender.display_name)
                }
            } else if own {
                "You".to_owned()
            } else {
                message.sender.display_name.clone()
            };
            let target = if message.channel == GameChatChannel::Whisper && own {
                message.recipient.as_ref()
            } else if own {
                None
            } else {
                Some(&message.sender)
            };
            if let Some(target) = target {
                append(
                    &author,
                    &action_button(
                        &self.document,
                        &label,
                        &format!("card:{}", target.player_reference),
                        "game-chat-player-name",
                    )?,
                )?;
            } else {
                author.set_text_content(Some(&label));
            }
            append(&row, &author)?;
            let text = element(&self.document, "span", "")?;
            text.set_text_content(Some(&message.text));
            append(&row, &text)?;
        }
        append(&self.chat.messages, &row)
    }
}
