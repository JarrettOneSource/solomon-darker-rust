//! Input sequencing from `host/game-host.ts:2005` and `applyQueuedInput` at 7857.
//! The source keeps cast edges on distinct ticks while coalescing movement at one tick.

use std::collections::BTreeMap;
use std::fmt;

use solomon_sim::float::hypot;
use solomon_sim::session::GameSessionInput;

const INPUT_LOOKAHEAD_TICKS: u64 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputRejection {
    NonFiniteCoordinates,
    MovementMagnitude,
    Viewport,
    Quickbar,
    TargetTooFarAhead,
    QueueTooFarAhead,
}
impl fmt::Display for InputRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonFiniteCoordinates => "input coordinates must be finite",
            Self::MovementMagnitude => "input movement magnitude exceeds one",
            Self::Viewport => "input viewport must be within 1 through 32768",
            Self::Quickbar => "input quickbar must be null or a slot from 0 through 7",
            Self::TargetTooFarAhead => "Input targets too far ahead of the server tick.",
            Self::QueueTooFarAhead => "Input queue extends too far ahead of the server tick.",
        })
    }
}
impl std::error::Error for InputRejection {}

/// The source protocol admits its unit-vector tolerance and does not normalize it again.
/// JSON decoding has already checked field types; these are the actual numeric contracts.
pub fn validate_input(input: &GameSessionInput) -> Result<(), InputRejection> {
    if !input.movement.x.is_finite()
        || !input.movement.y.is_finite()
        || input.aim.is_some_and(|aim| !aim.x.is_finite() || !aim.y.is_finite())
    {
        return Err(InputRejection::NonFiniteCoordinates);
    }
    if hypot(input.movement.x, input.movement.y) > 1.001 {
        return Err(InputRejection::MovementMagnitude);
    }
    if !input.viewport.x.is_finite()
        || !input.viewport.y.is_finite()
        || !(1.0..=32_768.0).contains(&input.viewport.x)
        || !(1.0..=32_768.0).contains(&input.viewport.y)
    {
        return Err(InputRejection::Viewport);
    }
    if input.quickbar.is_some_and(|slot| slot > 7) {
        return Err(InputRejection::Quickbar);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAdmission {
    Ignored,
    Queued {
        target_tick: u64,
    },
    /// The source acknowledges blocked input immediately and publishes the neutral state.
    Neutralized,
}

#[derive(Clone, Copy)]
struct QueuedInput {
    sequence: u64,
    input: GameSessionInput,
}

/// Per-connection input state. It contains no wizard, player component or gameplay clock.
#[derive(Default)]
pub struct InputQueue {
    last_received_sequence: u64,
    acknowledged_sequence: u64,
    active: GameSessionInput,
    queued: BTreeMap<u64, QueuedInput>,
}
impl InputQueue {
    #[must_use]
    pub fn last_received_sequence(&self) -> u64 {
        self.last_received_sequence
    }
    /// Bot observations sample the held input before this epoch drains queued
    /// human input. Reading it never advances an acknowledgement cursor.
    #[must_use]
    pub const fn active_input(&self) -> GameSessionInput {
        self.active
    }

    #[must_use]
    pub fn acknowledged_sequence(&self) -> u64 {
        self.acknowledged_sequence
    }

    pub fn accept(
        &mut self,
        sequence: u64,
        target_tick: u64,
        input: GameSessionInput,
        current_tick: u64,
        blocked: bool,
    ) -> Result<InputAdmission, InputRejection> {
        validate_input(&input)?;
        if sequence <= self.last_received_sequence {
            return Ok(InputAdmission::Ignored);
        }
        if blocked {
            self.last_received_sequence = sequence;
            self.acknowledged_sequence = sequence;
            self.neutralize();
            return Ok(InputAdmission::Neutralized);
        }
        let last_allowed_tick = current_tick + INPUT_LOOKAHEAD_TICKS;
        if target_tick > last_allowed_tick {
            return Err(InputRejection::TargetTooFarAhead);
        }
        let next_tick = current_tick + 1;
        let queued_tick = self.queued.last_key_value().map_or(next_tick, |(&tick, previous)| {
            let changed_cast = previous.input.primary != input.primary
                || previous.input.quickbar != input.quickbar;
            tick + u64::from(changed_cast)
        });
        let target_tick = next_tick.max(target_tick).max(queued_tick);
        if target_tick > last_allowed_tick {
            return Err(InputRejection::QueueTooFarAhead);
        }
        self.last_received_sequence = sequence;
        // Every admitted sequence increases, so this replaces only an older command at
        // the same tick. A primary/quickbar edge was assigned a separate tick above.
        self.queued.insert(target_tick, QueuedInput { sequence, input });
        Ok(InputAdmission::Queued { target_tick })
    }

    /// Called once before the authority's next tick. Commands retain their level until a
    /// newer eligible input replaces it; a missing network update does not invent a release.
    pub fn for_tick(&mut self, next_tick: u64) -> GameSessionInput {
        while self.queued.first_key_value().is_some_and(|(&tick, _)| tick <= next_tick) {
            let (_, queued) = self.queued.pop_first().expect("an eligible input is present");
            if queued.sequence <= self.acknowledged_sequence {
                continue;
            }
            self.active = queued.input;
            self.acknowledged_sequence = queued.sequence;
        }
        self.active
    }

    /// `stopAllClientInputs`: pause/loading surfaces clear held and queued commands. The
    /// ACK cursor changes only when the source admits or applies an input.
    pub fn neutralize(&mut self) {
        self.active = GameSessionInput::default();
        self.queued.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use solomon_sim::Vec2;

    #[test]
    fn all_three_hotbars_rebase_late_edges_without_losing_a_press() {
        for slot in [0, 7, 8, 15, 16, 23] {
            let mut queue = InputQueue::default();
            let press = GameSessionInput { quickbar: Some(slot), ..GameSessionInput::default() };
            assert_eq!(
                queue.accept(1, 11, press, 100, false),
                Ok(InputAdmission::Queued { target_tick: 101 })
            );
            assert_eq!(
                queue.accept(2, 12, GameSessionInput::default(), 100, false),
                Ok(InputAdmission::Queued { target_tick: 102 })
            );
            assert_eq!(queue.for_tick(101).quickbar, Some(slot));
            assert_eq!(queue.acknowledged_sequence(), 1);
            assert_eq!(queue.for_tick(102).quickbar, None);
            assert_eq!(queue.acknowledged_sequence(), 2);
        }
        for slot in [24, usize::MAX] {
            let mut queue = InputQueue::default();
            let invalid = GameSessionInput { quickbar: Some(slot), ..GameSessionInput::default() };
            assert_eq!(queue.accept(1, 11, invalid, 10, false), Err(InputRejection::Quickbar));
            assert_eq!(queue.last_received_sequence(), 0);
        }
    }

    fn point(value: &Value) -> Vec2 {
        Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
    }
    fn input(value: &Value) -> GameSessionInput {
        GameSessionInput {
            aim: (!value["aim"].is_null()).then(|| point(&value["aim"])),
            movement: point(&value["movement"]),
            primary: value["cast"]["primary"].as_bool().unwrap(),
            quickbar: value["cast"]["quickbar"].as_u64().map(|slot| slot as usize),
            viewport: Vec2::new(
                value["viewportWidth"].as_f64().unwrap(),
                value["viewportHeight"].as_f64().unwrap(),
            ),
        }
    }
    fn assert_snapshot(queue: &InputQueue, expected: &Value) {
        assert_eq!(
            queue.last_received_sequence,
            expected["lastReceivedSequence"].as_u64().unwrap()
        );
        assert_eq!(queue.acknowledged_sequence, expected["acknowledgedSequence"].as_u64().unwrap());
        assert_eq!(queue.active, input(&expected["activeInput"]));
        let actual: Vec<_> = queue
            .queued
            .iter()
            .map(|(&tick, queued)| (tick, queued.sequence, queued.input))
            .collect();
        let expected: Vec<_> = expected["queuedInputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|queued| {
                (
                    queued["targetTick"].as_u64().unwrap(),
                    queued["sequence"].as_u64().unwrap(),
                    input(&queued["input"]),
                )
            })
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn input_edges_acknowledgements_and_blockers_match_the_pinned_host() {
        let fixture: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/website/host-input-goldens.json"
        )))
        .unwrap();
        for scenario in fixture["scenarios"].as_array().unwrap() {
            let mut queue = InputQueue::default();
            for record in scenario["steps"].as_array().unwrap() {
                let action = &record["action"];
                let mut output = Vec::new();
                if action["kind"] == "input" {
                    let context = &action["context"];
                    let enabled = |field| context[field].as_bool().unwrap_or(false);
                    let blocked = enabled("pause")
                        || enabled("grace")
                        || enabled("barrier")
                        || enabled("offer")
                        || (enabled("hub") && enabled("activity"));
                    match queue.accept(
                        action["sequence"].as_u64().unwrap(),
                        action["targetTick"].as_u64().unwrap(),
                        input(&action["input"]),
                        context["tick"].as_u64().unwrap(),
                        blocked,
                    ) {
                        Ok(InputAdmission::Neutralized) => output.push(json!("snapshot")),
                        Ok(_) => {}
                        Err(error) => output
                            .push(json!({"code":"invalid-message","reason":error.to_string()})),
                    }
                } else {
                    queue.for_tick(action["tick"].as_u64().unwrap());
                }
                assert_eq!(json!(output), record["output"], "{}", scenario["name"]);
                assert_snapshot(&queue, &record["expected"]);
            }
        }
        for record in fixture["validations"].as_array().unwrap() {
            assert_eq!(
                validate_input(&input(&record["input"])).is_ok(),
                record["accepted"].as_bool().unwrap()
            );
        }
        let non_finite =
            GameSessionInput { movement: Vec2::new(f64::NAN, 0.0), ..GameSessionInput::default() };
        assert_eq!(validate_input(&non_finite), Err(InputRejection::NonFiniteCoordinates));
    }
}
