//! Pickup message coalescing and single-precision clocks from
//! `loot-message-presentation.ts`. The host admits only the local player's events.

use std::fmt;

use super::{BoneyardLootEvent, BoneyardLootSound};
use crate::combat::PlayerLifeState;
use crate::float::fround;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub const LOOT_MESSAGE_INITIAL_LIFETIME: f64 = 1.5;
pub const LOOT_MESSAGE_LIFETIME_LOSS: f64 = 0.005_f32 as f64;
pub const LOOT_MESSAGE_INITIAL_OFFSET: f64 = -18.0;
pub const LOOT_MESSAGE_RISE_PER_TICK: f64 = 1.0;
pub const LOOT_MESSAGE_INSERT_SHIFT: f64 = 4.0;
pub const LOOT_MESSAGE_INSERT_LIFETIME_LOSS: f64 = 0.1_f32 as f64;
pub const LOOT_MESSAGE_SCALE_DENOMINATOR: f64 = 250.0;

/// The four independent accepted event lanes in Website fa08 `WorldNotificationInput`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorldNotificationSource {
    Loot,
    Book,
    Combat,
    Secondary,
}

/// Book sequence numbers render as Source's negative book event ID; other lanes stay positive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorldNotificationKey {
    pub source: WorldNotificationSource,
    pub event_id: u64,
}

impl fmt::Display for WorldNotificationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = match self.source {
            WorldNotificationSource::Loot => "loot",
            WorldNotificationSource::Book => "book",
            WorldNotificationSource::Combat => "combat",
            WorldNotificationSource::Secondary => "secondary",
        };
        let sign = if self.source == WorldNotificationSource::Book && self.event_id > 0 {
            "-"
        } else {
            ""
        };
        write!(formatter, "{source}:{sign}{}", self.event_id)
    }
}

/// A concrete source event; the caller supplies only its accepted local notification.
#[derive(Clone, Copy, Debug)]
pub struct WorldNotificationInput<'a> {
    pub source: WorldNotificationSource,
    pub event_id: u64,
    pub tick: u64,
    pub text: &'a str,
    pub tint: u32,
}

#[must_use]
pub const fn world_notifications_visible(
    life_state: Option<PlayerLifeState>,
    combat_hud: bool,
) -> bool {
    combat_hud
        && matches!(life_state, Some(PlayerLifeState::Alive | PlayerLifeState::LethalPending))
}

#[derive(Clone, Debug, PartialEq)]
pub struct LootMessageState {
    pub event_id: u64,
    pub key: WorldNotificationKey,
    pub lifetime: f64,
    pub offset: f64,
    pub text: String,
    pub tint: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LootMessageVisual {
    pub message: LootMessageState,
    pub alpha: f64,
    pub scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LootMessagePresentation {
    /// Numeric identity kept for the established loot-only API; selection uses the source key.
    pub active_event_id: Option<u64>,
    active_key: Option<WorldNotificationKey>,
    pub last_tick: u64,
    pub messages: Vec<LootMessageState>,
}

impl LootMessagePresentation {
    #[must_use]
    pub fn new(initial_tick: u64) -> Self {
        Self {
            active_event_id: None,
            active_key: None,
            last_tick: initial_tick,
            messages: Vec::new(),
        }
    }

    pub fn consume(&mut self, event: &BoneyardLootEvent) -> bool {
        let Some(text) = &event.text else {
            return false;
        };
        self.consume_text(&WorldNotificationInput {
            source: WorldNotificationSource::Loot,
            event_id: event.event_id,
            tick: event.tick,
            text,
            tint: loot_message_tint(event),
        })
    }

    /// Current `consumeText`, with source identity independent of numeric event ID.
    pub fn consume_text(&mut self, event: &WorldNotificationInput<'_>) -> bool {
        let text = event.text;
        self.advance(self.last_tick.max(event.tick.saturating_sub(1)));
        let active_index = self.active_index();
        if let Some(index) = active_index {
            let active = &mut self.messages[index];
            if let (Some(gold), Some(active_gold)) = (gold_amount(text), gold_amount(&active.text))
                && active.lifetime > 1.0
            {
                active.lifetime = LOOT_MESSAGE_INITIAL_LIFETIME;
                // JavaScript adds Number values, including rounding a sum above 2^53.
                active.text = format!("{:.0} GOLD", active_gold + gold);
                return true;
            }
            while self.messages[index].offset < 0.0 {
                for (message_index, message) in self.messages.iter_mut().enumerate() {
                    if message_index != index {
                        message.lifetime =
                            fround(message.lifetime - LOOT_MESSAGE_INSERT_LIFETIME_LOSS);
                    }
                    message.offset = fround(message.offset + LOOT_MESSAGE_INSERT_SHIFT);
                }
            }
        }
        self.messages.push(LootMessageState {
            event_id: event.event_id,
            key: WorldNotificationKey { source: event.source, event_id: event.event_id },
            lifetime: LOOT_MESSAGE_INITIAL_LIFETIME,
            offset: LOOT_MESSAGE_INITIAL_OFFSET,
            text: text.to_owned(),
            tint: event.tint,
        });
        self.active_event_id = Some(event.event_id);
        self.active_key =
            Some(WorldNotificationKey { source: event.source, event_id: event.event_id });
        true
    }

    /// The established ed0a loot fixture's unrounded visual scale. Production world
    /// notifications use `sample_world_notifications` on this same clock and state.
    pub fn sample(&mut self, tick: u64) -> Vec<LootMessageVisual> {
        self.advance(tick);
        self.messages
            .iter()
            .map(|message| LootMessageVisual {
                message: message.clone(),
                alpha: message.lifetime.clamp(0.0, 1.0),
                scale: fround(1.0 - message.offset.max(0.0) / LOOT_MESSAGE_SCALE_DENOMINATOR)
                    .max(0.0),
            })
            .collect()
    }

    /// Website fa08 rounds the already-f32 scale to two decimal places, then narrows again.
    pub fn sample_world_notifications(&mut self, tick: u64) -> Vec<LootMessageVisual> {
        let mut visuals = self.sample(tick);
        for visual in &mut visuals {
            // An f32 value multiplied by integer 100 fits exactly in f64. Positive
            // nearest-decimal rounding therefore matches Number(...toFixed(2)).
            visual.scale = fround((visual.scale * 100.0).round() / 100.0);
        }
        visuals
    }

    fn active_index(&self) -> Option<usize> {
        self.messages.iter().position(|message| Some(message.key) == self.active_key)
    }

    fn advance(&mut self, tick: u64) {
        if tick > MAX_SAFE_INTEGER || tick < self.last_tick {
            return;
        }
        for _ in self.last_tick..tick {
            self.step();
        }
        self.last_tick = tick;
    }

    fn step(&mut self) {
        if let Some(active_index) = self.active_index()
            && self.messages[active_index].offset < 0.0
        {
            let mut remaining = self.messages.len();
            for (index, message) in self.messages.iter_mut().enumerate() {
                let minimum_lifetime = fround(1.0 - remaining as f64 * (0.4_f32 as f64));
                remaining -= 1;
                if index != active_index {
                    message.lifetime =
                        fround(message.lifetime - (0.025_f32 as f64)).max(minimum_lifetime);
                }
                message.offset = fround(message.offset + LOOT_MESSAGE_RISE_PER_TICK);
            }
        }
        for message in &mut self.messages {
            message.lifetime = fround(message.lifetime - LOOT_MESSAGE_LIFETIME_LOSS);
        }
        self.messages.retain(|message| message.lifetime > 0.0);
        if self.active_index().is_none() {
            self.active_event_id = None;
            self.active_key = None;
        }
    }
}

fn gold_amount(text: &str) -> Option<f64> {
    let digits = text.strip_suffix(" GOLD")?;
    if digits.is_empty() || !digits.bytes().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let amount = digits.parse::<f64>().ok()?;
    (amount <= MAX_SAFE_INTEGER as f64).then_some(amount)
}

#[must_use]
pub fn loot_message_tint(event: &BoneyardLootEvent) -> u32 {
    let text = event.text.as_deref();
    if event.sound == Some(BoneyardLootSound::PickupCoin) || text == Some("DAMAGE x4") {
        return 0xd9ba70;
    }
    if text == Some("BONUS SKILL POINT") {
        return 0xff8080;
    }
    if text.is_some_and(|text| text.ends_with(" +1")) {
        return 0x80ffff;
    }
    0xffffff
}
