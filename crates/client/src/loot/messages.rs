//! Floating world notifications for the local player: loot messages, Skill Book ranks,
//! mana overloads and charm events, read from accepted snapshots. One clock owns all
//! notification lanes. Loading/restoring seeds cursors without replay.

use std::collections::BTreeMap;

use egui::{LayerId, Order, Rect, pos2, vec2};
use solomon_sim::Vec2;
use solomon_sim::boneyard::loot::messages::{
    LootMessagePresentation, LootMessageVisual, WorldNotificationInput, WorldNotificationKey,
    WorldNotificationSource, world_notifications_visible,
};
use solomon_sim::economy::{SkillBookFeedbackCursor, SkillBookOutcome, skill_book_result_text};
use solomon_sim::enemies::model::EnemyEventKind;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::secondary::SecondaryEventKind;
use solomon_sim::session::GameSession;
use solomon_sim::ui_text::{UiFontName, UiTextSpec, ui_glyph_ink_bounds};

use crate::gpu_ui::{UiFrame, native_text};

/// The local player's floating world notifications and the event cursors that feed them.
#[derive(Default)]
pub struct WorldNotifications {
    state: Option<NotificationState>,
}

struct NotificationState {
    player_id: String,
    run_id: Option<String>,
    tick: u64,
    last_loot_id: u64,
    last_combat_id: usize,
    next_secondary_id: u64,
    book: SkillBookFeedbackCursor,
    presentation: LootMessagePresentation,
    visuals: Vec<LootMessageVisual>,
    announced: BTreeMap<WorldNotificationKey, String>,
    visible: bool,
}

impl WorldNotifications {
    /// The Boneyard notification layer, then the separate Hub layer for book messages.
    pub fn layer_ids(&self) -> [LayerId; 2] {
        [world_notification_layer_id(false), world_notification_layer_id(true)]
    }

    pub fn layer_id(&self) -> LayerId {
        world_notification_layer_id(self.state.as_ref().is_none_or(|state| state.run_id.is_none()))
    }

    /// Call with the accepted authority session, once before composing its GPU frame.
    /// Repeated snapshots do not replay events or advance the notification clock twice.
    pub fn update(
        &mut self,
        session: &GameSession,
        player_id: &str,
        combat_hud: bool,
    ) -> Result<(), String> {
        let Some(participant) = session.participants.get(player_id) else {
            self.reset();
            return Ok(());
        };
        let feedback = participant.player.economy.action_feedback.as_ref();
        let run_id = session.boneyard.as_ref().map(|world| world.loaded.run_id.as_str());
        let visible =
            world_notifications_visible(Some(participant.player.combat.life_state), combat_hud);
        let reset = self.state.as_ref().is_none_or(|state| {
            state.player_id != player_id
                || state.run_id.as_deref() != run_id
                || session.ticks < state.tick
        });
        let terminal = matches!(session.run.phase, GameRunPhase::GameOver | GameRunPhase::Loadout);
        if reset || terminal {
            self.state = Some(NotificationState {
                player_id: player_id.to_owned(),
                run_id: run_id.map(String::from),
                tick: session.ticks,
                last_loot_id: session
                    .boneyard
                    .as_ref()
                    .and_then(|world| world.loot_events.iter().map(|event| event.event_id).max())
                    .unwrap_or(0),
                last_combat_id: session
                    .boneyard
                    .as_ref()
                    .and_then(|world| world.enemy_events.iter().map(|event| event.event_id).max())
                    .unwrap_or(0),
                next_secondary_id: session.secondary.next_event_id,
                book: SkillBookFeedbackCursor::new(feedback),
                presentation: LootMessagePresentation::new(session.ticks),
                visuals: Vec::new(),
                announced: BTreeMap::new(),
                visible: visible && !terminal,
            });
            return Ok(());
        }
        let state = self.state.as_mut().unwrap();
        if let Some(world) = &session.boneyard {
            for event in &world.loot_events {
                if event.event_id > state.last_loot_id {
                    state.last_loot_id = event.event_id;
                    if event.player_id.as_deref() == Some(player_id) {
                        state.presentation.consume(event);
                    }
                }
            }
        }
        if let Some(receipt) = state.book.consume(feedback)
            && let SkillBookOutcome::Rank { skill_id: Some(skill_id) } = receipt.outcome
        {
            state.presentation.consume_text(&WorldNotificationInput {
                source: WorldNotificationSource::Book,
                event_id: receipt.sequence,
                tick: session.ticks,
                text: &skill_book_result_text(skill_id)?,
                tint: 0x8080ff,
            });
        }
        if let Some(world) = &session.boneyard {
            let world_key = format!("boneyard:{}", world.loaded.run_id);
            for event in &session.secondary.events {
                if event.event_id >= state.next_secondary_id
                    && event.kind == SecondaryEventKind::Overload
                    && event.owner_id == player_id
                    && event.world_key == world_key
                {
                    state.presentation.consume_text(&WorldNotificationInput {
                        source: WorldNotificationSource::Secondary,
                        event_id: event.event_id,
                        tick: event.tick,
                        text: "Overloaded Mana!",
                        tint: 0xffffff,
                    });
                }
            }
            for event in &world.enemy_events {
                if event.event_id > state.last_combat_id {
                    state.last_combat_id = event.event_id;
                    let text = match event.kind {
                        // The accepted loss event groups the two until-hurt charms;
                        // it does not identify which charm's effect ended.
                        EnemyEventKind::PlayerCharmLost => "CHARM EFFECT LOST!",
                        EnemyEventKind::PlayerCheatDeath => "CHEAT DEATH!",
                        _ => continue,
                    };
                    if event.target_player_id.as_ref().and_then(Option::as_deref) == Some(player_id)
                    {
                        state.presentation.consume_text(&WorldNotificationInput {
                            source: WorldNotificationSource::Combat,
                            event_id: event.event_id as u64,
                            tick: event.tick,
                            text,
                            tint: 0xffffff,
                        });
                    }
                }
            }
        }
        state.next_secondary_id = session.secondary.next_event_id;
        state.visuals = state.presentation.sample_world_notifications(session.ticks);
        state.tick = session.ticks;
        state.visible = visible;
        state.announced.retain(|key, _| state.visuals.iter().any(|row| row.message.key == *key));
        Ok(())
    }

    /// Native full-gameplay coordinates, independent of Inventory/modal stage transforms.
    pub fn paint(&mut self, frame: &mut UiFrame<'_>, ui_scale: f64) -> Result<(), String> {
        let layer = self.layer_id();
        let Some(state) = self.state.as_mut().filter(|state| state.visible) else {
            return Ok(());
        };
        let viewport = frame.ctx.content_rect();
        let scale = solomon_sim::viewport::game_viewport(Vec2::new(
            f64::from(viewport.width()),
            f64::from(viewport.height()),
        ))
        .display_scale
            * ui_scale;
        let clip = Rect::from_min_max(
            pos2(viewport.left(), viewport.top() + (50.0 * scale) as f32),
            viewport.right_bottom(),
        );
        let painter = frame.ctx.layer_painter(layer).with_clip_rect(clip);
        for visual in state.visuals.iter().filter(|visual| visual.alpha > 0.0 && visual.scale > 0.0)
        {
            let baseline =
                f64::from(viewport.top()) + (67.0 + visual.message.offset.round()) * scale;
            let spec = UiTextSpec {
                scale: visual.scale * scale,
                alpha: visual.alpha,
                tint: visual.message.tint,
                ..UiTextSpec::new(
                    UiFontName::Menu,
                    &visual.message.text,
                    f64::from(viewport.center().x),
                    baseline,
                )
            };
            native_text(
                frame,
                &painter,
                &UiTextSpec { y: baseline + 2.0 * scale, tint: 0, ..spec },
            );
            let layout = native_text(frame, &painter, &spec);
            let ink = layout
                .glyphs
                .iter()
                .map(|glyph| {
                    let ink = ui_glyph_ink_bounds(glyph);
                    Rect::from_min_size(
                        pos2(ink.left as f32, ink.top as f32),
                        vec2(ink.width as f32, ink.height as f32),
                    )
                })
                .fold(Rect::NOTHING, Rect::union);
            let bounds = ink.intersect(clip);
            if bounds.is_positive() {
                let id = format!("world-notification:{}", visual.message.key);
                frame.record_clipped_rect(&id, &visual.message.text, "text", ink, clip, false);
                frame.record_value(
                    &id,
                    &serde_json::json!({
                        "key":visual.message.key.to_string(), "alpha":visual.alpha,
                        "scale":visual.scale, "offset":visual.message.offset,
                        "baseline":baseline, "clipTop":clip.top(),
                    })
                    .to_string(),
                    None,
                );
                if state.announced.get(&visual.message.key) != Some(&visual.message.text) {
                    frame.announce(&id, &visual.message.text);
                    state.announced.insert(visual.message.key, visual.message.text.clone());
                }
            }
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.state = None;
    }
}

fn world_notification_layer_id(hub: bool) -> LayerId {
    LayerId::new(
        Order::Background,
        egui::Id::unique(if hub { "hub-world-notifications" } else { "world-notifications" }),
    )
}

#[cfg(test)]
mod tests;
