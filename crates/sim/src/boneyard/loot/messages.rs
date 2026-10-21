//! Pickup message coalescing and single-precision clocks from
//! `loot-message-presentation.ts`. The host admits only the local player's events.

use super::{BoneyardLootEvent, BoneyardLootSound};
use crate::float::fround;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub const NATIVE_LOOT_MESSAGE_INITIAL_LIFETIME: f64 = 1.5;
pub const NATIVE_LOOT_MESSAGE_LIFETIME_LOSS: f64 = 0.005_f32 as f64;
pub const NATIVE_LOOT_MESSAGE_INITIAL_OFFSET: f64 = -18.0;
pub const NATIVE_LOOT_MESSAGE_RISE_PER_TICK: f64 = 1.0;
pub const NATIVE_LOOT_MESSAGE_INSERT_SHIFT: f64 = 4.0;
pub const NATIVE_LOOT_MESSAGE_INSERT_LIFETIME_LOSS: f64 = 0.1_f32 as f64;
pub const NATIVE_LOOT_MESSAGE_SCALE_DENOMINATOR: f64 = 250.0;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLootMessageState {
    pub event_id: u64,
    pub lifetime: f64,
    pub offset: f64,
    pub text: String,
    pub tint: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLootMessageVisual {
    pub message: NativeLootMessageState,
    pub alpha: f64,
    pub scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLootMessagePresentation {
    pub active_event_id: Option<u64>,
    pub last_tick: u64,
    pub messages: Vec<NativeLootMessageState>,
}

impl NativeLootMessagePresentation {
    #[must_use]
    pub fn new(initial_tick: u64) -> Self {
        Self { active_event_id: None, last_tick: initial_tick, messages: Vec::new() }
    }

    pub fn consume(&mut self, event: &BoneyardLootEvent) -> bool {
        let Some(text) = &event.text else {
            return false;
        };
        self.advance(self.last_tick.max(event.tick.saturating_sub(1)));
        let active_index = self.active_index();
        if let Some(index) = active_index {
            let active = &mut self.messages[index];
            if let (Some(gold), Some(active_gold)) = (gold_amount(text), gold_amount(&active.text))
                && active.lifetime > 1.0
            {
                active.lifetime = NATIVE_LOOT_MESSAGE_INITIAL_LIFETIME;
                // JavaScript adds Number values, including rounding a sum above 2^53.
                active.text = format!("{:.0} GOLD", active_gold + gold);
                return true;
            }
            while self.messages[index].offset < 0.0 {
                for (message_index, message) in self.messages.iter_mut().enumerate() {
                    if message_index != index {
                        message.lifetime =
                            fround(message.lifetime - NATIVE_LOOT_MESSAGE_INSERT_LIFETIME_LOSS);
                    }
                    message.offset = fround(message.offset + NATIVE_LOOT_MESSAGE_INSERT_SHIFT);
                }
            }
        }
        self.messages.push(NativeLootMessageState {
            event_id: event.event_id,
            lifetime: NATIVE_LOOT_MESSAGE_INITIAL_LIFETIME,
            offset: NATIVE_LOOT_MESSAGE_INITIAL_OFFSET,
            text: text.clone(),
            tint: native_loot_message_tint(event),
        });
        self.active_event_id = Some(event.event_id);
        true
    }

    pub fn sample(&mut self, tick: u64) -> Vec<NativeLootMessageVisual> {
        self.advance(tick);
        self.messages
            .iter()
            .map(|message| NativeLootMessageVisual {
                message: message.clone(),
                alpha: message.lifetime.clamp(0.0, 1.0),
                scale: fround(
                    1.0 - message.offset.max(0.0) / NATIVE_LOOT_MESSAGE_SCALE_DENOMINATOR,
                )
                .max(0.0),
            })
            .collect()
    }

    fn active_index(&self) -> Option<usize> {
        self.messages.iter().position(|message| Some(message.event_id) == self.active_event_id)
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
                message.offset = fround(message.offset + NATIVE_LOOT_MESSAGE_RISE_PER_TICK);
            }
        }
        for message in &mut self.messages {
            message.lifetime = fround(message.lifetime - NATIVE_LOOT_MESSAGE_LIFETIME_LOSS);
        }
        self.messages.retain(|message| message.lifetime > 0.0);
        if self.active_index().is_none() {
            self.active_event_id = None;
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
pub fn native_loot_message_tint(event: &BoneyardLootEvent) -> u32 {
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
