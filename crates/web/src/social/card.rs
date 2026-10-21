//! The actual host profile card and its Message/Invite commands.

use solomon_sim::Vec2;
use solomon_sim::profile::native_wizard_class_title;
use solomon_sim::social::{
    GameChatSender, GameChatWorldKind, GamePlayerCardProfile, GameSessionKind,
    hub_player_party_invite_available, native_player_card_college_invite_available,
};

use super::{SocialUi, Surface, action_button, attribute};
use crate::ui::{append, element};

impl SocialUi {
    pub(super) fn current_card(&self) -> Option<&GamePlayerCardProfile> {
        let Some(Surface::Card(card)) = &self.surface else {
            return None;
        };
        Some(card)
    }

    pub(super) fn whisper_to_card(&mut self) {
        let Some(card) = self.current_card().cloned().or_else(|| self.hub_player_profile()) else {
            return;
        };
        if (!self.hosted || matches!(self.surface, Some(Surface::HubPlayer(_))))
            && self
                .snapshot
                .as_ref()
                .is_none_or(|snapshot| snapshot.local_player_id == card.player_id)
        {
            return;
        }
        self.whisper = Some(GameChatSender {
            display_name: card.display_name,
            player_id: card.player_id,
            player_reference: card.player_reference,
        });
        self.open_chat(self.now);
        self.channel = solomon_sim::social::GameChatChannel::Whisper;
        self.manually_selected_channel = true;
        self.unread[super::channel_index(self.channel)] = 0;
    }

    pub(super) fn draw_card(&self) -> Result<(), String> {
        let actor_profile = self.hub_player_profile();
        let Some(card) = self.current_card().or(actor_profile.as_ref()) else {
            return Ok(());
        };
        let snapshot = self.snapshot.as_ref().expect("a card belongs to a host snapshot");
        let is_actor = matches!(self.surface, Some(Surface::HubPlayer(_)));
        let dialog = element(&self.document, "section", "hub-player-profile")?;
        for (name, value) in [
            ("role", "dialog"),
            ("aria-modal", "true"),
            ("aria-labelledby", "social-player-name"),
            ("tabindex", "-1"),
            ("data-social-focus", "true"),
            ("data-profile-player", card.player_id.as_str()),
            ("data-profile-element", card.element.as_str()),
            ("data-profile-origin", if is_actor { "actor" } else { "hosted-profile" }),
        ] {
            attribute(&dialog, name, value)?;
        }
        for class in [
            "hub-player-profile-corner",
            "hub-player-profile-corner hub-player-profile-corner-right",
        ] {
            let corner = self.ui.sprite(&self.document, 17, class)?;
            corner
                .style()
                .set_property("position", "absolute")
                .map_err(|_| "card corner positioning")?;
            append(&dialog, &corner)?;
        }
        let body = element(&self.document, "div", "hub-player-profile-body")?;
        let header = element(&self.document, "header", "hub-player-profile-header")?;
        let portrait = element(&self.document, "span", "hub-wizard-portrait")?;
        let wizard =
            self.portraits.wizard(&self.document, card.element, 12, Vec2::new(31.5, 66.5), 1.15)?;
        wizard.orb.set_hidden(true);
        wizard
            .root
            .style()
            .set_property("transform-origin", "top left")
            .map_err(|_| "card portrait origin")?;
        append(&portrait, &wizard.root)?;
        let frame = self.skills.sprite(&self.document, 14, "hub-wizard-portrait-frame")?;
        let geometry = self.skills.record(14)?;
        // Current Website PlayerCardDialog uses the native Skills frame, with its
        // logical dimensions scaled to the responsive portrait's square.
        frame
            .style()
            .set_property("position", "absolute")
            .map_err(|_| "player card frame position")?;
        frame
            .style()
            .set_property(
                "transform",
                &format!(
                    "scale(calc(var(--hub-portrait-size) / {}), calc(var(--hub-portrait-size) / {}))",
                    geometry.logical_width, geometry.logical_height,
                ),
            )
            .map_err(|_| "player card frame scale")?;
        append(&portrait, &frame)?;
        append(&header, &portrait)?;
        let title = element(&self.document, "div", "hub-player-profile-title")?;
        let name = element(&self.document, "h2", "")?;
        attribute(&name, "id", "social-player-name")?;
        name.set_text_content(Some(&card.display_name));
        append(&title, &name)?;
        let class = element(&self.document, "p", "hub-player-profile-class")?;
        class.set_text_content(Some(native_wizard_class_title(card.element, card.discipline)));
        append(&title, &class)?;
        let badge = element(&self.document, "p", "hub-player-profile-badge")?;
        attribute(&badge, "data-registered", &card.account_username.is_some().to_string())?;
        badge.set_text_content(Some(
            &card
                .account_username
                .as_ref()
                .map_or_else(|| "Guest wizard".to_owned(), |name| format!("Registered · {name}")),
        ));
        append(&title, &badge)?;
        if is_actor {
            let actor = self.hub_players.iter().find(|actor| actor.player_id == card.player_id);
            if let Some(activity) = actor.and_then(|actor| actor.activity) {
                let label = element(&self.document, "p", "hub-player-profile-activity")?;
                let (kind, text) = match activity {
                    solomon_protocol::HubActivity::Occupied => ("occupied", "Occupied"),
                    solomon_protocol::HubActivity::Paused => ("paused", "Paused"),
                };
                attribute(&label, "data-profile-activity", kind)?;
                label.set_text_content(Some(text));
                append(&title, &label)?;
            }
        } else {
            let activity = element(&self.document, "p", "hub-player-profile-activity")?;
            let session_label = match card.session_kind {
                GameSessionKind::PrivateCollege => "PRIVATE COLLEGE",
                GameSessionKind::GlobalHub => "GLOBAL HUB",
                GameSessionKind::Standalone => "LOCAL SESSION",
            };
            let world_label = match card.activity {
                GameChatWorldKind::Hub => "IN COLLEGE",
                GameChatWorldKind::Boneyard => "IN BONEYARD",
            };
            activity.set_text_content(Some(&format!("{session_label} · {world_label}")));
            append(&title, &activity)?;
        }
        append(&header, &title)?;
        append(&body, &header)?;
        let stats = element(&self.document, "dl", "hub-player-profile-stats")?;
        let minutes = card.total_playtime_ms.map(|time| time / 60_000);
        let playtime = minutes.map_or_else(
            || "—".to_owned(),
            |minutes| {
                if minutes >= 60 {
                    format!("{}h {}m", minutes / 60, minutes % 60)
                } else {
                    format!("{minutes}m")
                }
            },
        );
        for (label, value, record) in [
            ("Gold", format_integer(card.gold), "bag"),
            (
                "Highest Wave",
                card.highest_wave.map_or_else(|| "—".to_owned(), format_integer),
                "wave",
            ),
            ("Time in the Dark", playtime, "infinity"),
        ] {
            let row = element(&self.document, "div", "hub-player-profile-stat")?;
            let icon = element(&self.document, "img", "social-profile-icon")?;
            attribute(&icon, "src", &format!("assets/stock/icon-{record}.png"))?;
            attribute(&icon, "alt", "")?;
            append(&row, &icon)?;
            let term = element(&self.document, "dt", "")?;
            term.set_text_content(Some(label));
            append(&row, &term)?;
            let value_node = element(&self.document, "dd", "")?;
            value_node.set_text_content(Some(&value));
            if label == "Gold" {
                attribute(&value_node, "data-profile-gold", &card.gold.to_string())?;
            }
            append(&row, &value_node)?;
            append(&stats, &row)?;
        }
        append(&body, &stats)?;
        let actions = element(&self.document, "div", "hub-player-profile-actions")?;
        if ((!is_actor && self.hosted) || card.player_id != snapshot.local_player_id)
            && self.context.chat_enabled
        {
            append(
                &actions,
                &action_button(
                    &self.document,
                    "Message",
                    "card-message",
                    "hub-player-profile-message",
                )?,
            )?;
        }
        let can_invite = if is_actor {
            hub_player_party_invite_available(
                snapshot.party_state.as_ref().map(|state| &state.party),
                &snapshot.local_player_id,
                &card.player_id,
            )
        } else if self.hosted {
            // MainMenuScene2259: invitation opens the recipient's consent flow;
            // this is deliberately distinct from an in-session party invitation.
            native_player_card_college_invite_available(
                snapshot.session_kind,
                snapshot.world_kind,
                snapshot
                    .party_state
                    .as_ref()
                    .is_some_and(|state| state.party.leader_player_id == snapshot.local_player_id),
                card.activity,
            )
        } else {
            snapshot.directory_available
                && card.player_id != snapshot.local_player_id
                && snapshot.party_state.as_ref().is_some_and(|state| {
                    state.party.leader_player_id == snapshot.local_player_id
                        && !state.party.member_player_ids.contains(&card.player_id)
                })
        };
        if can_invite {
            append(
                &actions,
                &action_button(
                    &self.document,
                    "Invite to Party",
                    "card-invite",
                    "hub-player-profile-invite",
                )?,
            )?;
        }
        append(
            &actions,
            &action_button(&self.document, "Close", "close", "hub-player-profile-close")?,
        )?;
        append(&body, &actions)?;
        append(&dialog, &body)?;
        append(&self.modal, &dialog)
    }

    fn hub_player_profile(&self) -> Option<GamePlayerCardProfile> {
        let Some(Surface::HubPlayer(player_id)) = &self.surface else {
            return None;
        };
        let actor = self.hub_players.iter().find(|actor| actor.player_id == *player_id)?;
        let snapshot = self.snapshot.as_ref()?;
        let profile = snapshot.party_state.as_ref().and_then(|state| {
            state.hub_players.iter().find(|profile| profile.player_id == *player_id)
        });
        Some(GamePlayerCardProfile {
            player_id: player_id.clone(),
            account_username: profile.and_then(|profile| profile.account_username.clone()),
            activity: GameChatWorldKind::Hub,
            discipline: actor.selection.discipline,
            display_name: profile.map_or_else(
                || actor.selection.name.clone(),
                |profile| profile.display_name.clone(),
            ),
            element: actor.selection.element,
            gold: actor.gold,
            highest_wave: profile.and_then(|profile| profile.highest_wave),
            player_reference: player_id.clone(),
            session_kind: snapshot.session_kind,
            total_playtime_ms: profile.and_then(|profile| profile.total_playtime_ms),
        })
    }
}

fn format_integer(value: u64) -> String {
    let digits = value.to_string();
    let mut result = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}
