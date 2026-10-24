//! Outbound input ordering from `client/game-client-session.ts::sendInput`.
//!
//! Movement changes may replace the pending input for a tick. A primary or quickbar edge
//! gets a later tick so a press and release between snapshots both reach the authority.
//! `client-input-goldens.mjs` records the unchanged source method and its acknowledgements.

use solomon_sim::belt::PLAYER_BELT_SLOT_COUNT;
use solomon_sim::float::hypot;
use solomon_sim::session::GameSessionInput;

use crate::ClientMessage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingInput {
    sequence: u64,
    target_tick: u64,
}

#[derive(Default)]
pub struct ClientInputSequence {
    sequence: u64,
    sent: GameSessionInput,
    pending: Vec<PendingInput>,
}

impl ClientInputSequence {
    /// Call after a Welcome and against the latest accepted snapshot tick. Holds include
    /// authority pause/grace, a level-up barrier, the local offer/death and GameOver.
    pub fn queue_input(
        &mut self,
        snapshot_tick: u64,
        requested: GameSessionInput,
        held: bool,
    ) -> Result<Option<ClientMessage>, &'static str> {
        let mut input = if held { GameSessionInput::default() } else { requested };
        if !input.movement.x.is_finite() || !input.movement.y.is_finite() {
            return Err("game input must contain finite movement coordinates");
        }
        if input.aim.is_some_and(|aim| !aim.x.is_finite() || !aim.y.is_finite()) {
            return Err("game input must contain finite aim coordinates");
        }
        if input.quickbar.is_some_and(|slot| slot >= PLAYER_BELT_SLOT_COUNT) {
            return Err("game input must contain a primary level and native skill quickbar slot");
        }
        if !input.viewport.x.is_finite() || input.viewport.x < 1.0 {
            return Err("game input must contain a positive finite viewport width");
        }
        if !input.viewport.y.is_finite() || input.viewport.y < 1.0 {
            return Err("game input must contain a positive finite viewport height");
        }
        let length = hypot(input.movement.x, input.movement.y);
        if length > 1.0 {
            input.movement.x /= length;
            input.movement.y /= length;
        }
        if input == self.sent {
            return Ok(None);
        }

        let cast_transition =
            input.primary != self.sent.primary || input.quickbar != self.sent.quickbar;
        self.sent = input;
        self.sequence += 1;
        let next_tick = snapshot_tick + 1;
        let target_tick = self
            .pending
            .iter()
            .max_by_key(|entry| (entry.target_tick, entry.sequence))
            .map_or(next_tick, |entry| {
                next_tick.max(entry.target_tick + u64::from(cast_transition))
            });
        let pending = PendingInput { sequence: self.sequence, target_tick };
        if let Some(existing) =
            self.pending.iter_mut().find(|entry| entry.target_tick == target_tick)
        {
            *existing = pending;
        } else {
            self.pending.push(pending);
        }
        Ok(Some(ClientMessage::Input { sequence: self.sequence, target_tick, input }))
    }

    pub fn acknowledge(&mut self, sequence: u64) {
        self.pending.retain(|input| input.sequence > sequence);
    }

    /// The authority clears held input on pause/grace changes and level-up barriers.
    /// Preserve the connection's monotonic sequence while forgetting the old cast level.
    pub fn reset(&mut self) {
        self.sent = GameSessionInput::default();
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn all_three_hotbars_preserve_press_release_edges() {
        for slot in [0, 7, 8, 15, 16, 23] {
            let mut client = ClientInputSequence::default();
            let press = GameSessionInput { quickbar: Some(slot), ..GameSessionInput::default() };
            let Some(ClientMessage::Input { sequence: 1, target_tick: 11, input }) =
                client.queue_input(10, press, false).unwrap()
            else {
                panic!("the selected hotbar slot must publish its press");
            };
            assert_eq!(input.quickbar, Some(slot));
            let Some(ClientMessage::Input { sequence: 2, target_tick: 12, input }) =
                client.queue_input(10, GameSessionInput::default(), false).unwrap()
            else {
                panic!("the release must occupy a separate tick");
            };
            assert_eq!(input.quickbar, None);
        }
        for slot in [24, usize::MAX] {
            let mut client = ClientInputSequence::default();
            let invalid = GameSessionInput { quickbar: Some(slot), ..GameSessionInput::default() };
            assert!(client.queue_input(10, invalid, false).is_err());
            assert_eq!(client.sequence, 0);
        }
    }
    use solomon_sim::Vec2;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SourceInput {
        aim: Option<Vec2>,
        cast: SourceCast,
        movement: Vec2,
        viewport_width: f64,
        viewport_height: f64,
    }

    #[derive(Deserialize)]
    struct SourceCast {
        primary: bool,
        quickbar: Option<usize>,
    }

    impl From<SourceInput> for GameSessionInput {
        fn from(input: SourceInput) -> Self {
            Self {
                aim: input.aim,
                movement: input.movement,
                primary: input.cast.primary,
                quickbar: input.cast.quickbar,
                viewport: Vec2::new(input.viewport_width, input.viewport_height),
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Action {
        Send {
            input: SourceInput,
            tick: u64,
            #[serde(default)]
            pause: bool,
            #[serde(default)]
            grace: bool,
            #[serde(default)]
            barrier: bool,
            #[serde(default)]
            offer: bool,
            #[serde(default)]
            dead: bool,
            #[serde(default, rename = "gameOver")]
            game_over: bool,
        },
        Ack {
            sequence: u64,
        },
        Reset,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Output {
        Send {
            sequence: u64,
            #[serde(rename = "targetTick")]
            target_tick: u64,
            input: SourceInput,
        },
        Error {
            error: String,
        },
    }

    #[derive(Deserialize)]
    struct Fixture {
        scenarios: Vec<Scenario>,
    }
    #[derive(Deserialize)]
    struct Scenario {
        name: String,
        steps: Vec<Step>,
    }
    #[derive(Deserialize)]
    struct Step {
        action: Action,
        expected: Expected,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Expected {
        output: Vec<Output>,
        sequence: u64,
        pending_inputs: Vec<SourcePending>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SourcePending {
        sequence: u64,
        target_tick: u64,
    }

    #[test]
    fn source_input_edges_acknowledgements_and_holds() {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../tests/fixtures/website/client-input-goldens.json"
        ))
        .unwrap();
        for scenario in fixture.scenarios {
            let mut client = ClientInputSequence::default();
            for (index, step) in scenario.steps.into_iter().enumerate() {
                let actual = match step.action {
                    Action::Send { input, tick, pause, grace, barrier, offer, dead, game_over } => {
                        client.queue_input(
                            tick,
                            input.into(),
                            pause || grace || barrier || offer || dead || game_over,
                        )
                    }
                    Action::Ack { sequence } => {
                        client.acknowledge(sequence);
                        Ok(None)
                    }
                    Action::Reset => {
                        client.reset();
                        Ok(None)
                    }
                };
                assert!(step.expected.output.len() <= 1);
                match (actual, step.expected.output.into_iter().next()) {
                    (Ok(None), None) => {}
                    (
                        Ok(Some(ClientMessage::Input { sequence, target_tick, input })),
                        Some(Output::Send {
                            sequence: expected_sequence,
                            target_tick: expected_tick,
                            input: expected_input,
                        }),
                    ) => {
                        assert_eq!(
                            (sequence, target_tick, input),
                            (expected_sequence, expected_tick, expected_input.into()),
                            "{} step {index}",
                            scenario.name
                        );
                    }
                    (Err(error), Some(Output::Error { error: expected })) => {
                        assert_eq!(error, expected)
                    }
                    _ => panic!("unexpected output in {} step {index}", scenario.name),
                }
                assert_eq!(client.sequence, step.expected.sequence);
                assert_eq!(
                    client.pending,
                    step.expected
                        .pending_inputs
                        .into_iter()
                        .map(|entry| PendingInput {
                            sequence: entry.sequence,
                            target_tick: entry.target_tick
                        })
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
