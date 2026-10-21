//! Host-connection Flame history from `host/flame-presentation-history.ts`.
//! Only compact presentation recipes are retained; the Session remains the authority.

use std::collections::BTreeMap;

use solomon_protocol::GAME_WEBSOCKET_MAX_PAYLOAD_BYTES;
use solomon_protocol::flame_presentation::{
    FlamePresentationBaseline, FlamePresentationBirth, FlamePresentationFrame,
    FlamePresentationHistory, FlamePresentationHistoryOptions, MAX_FLAME_PRESENTATION_BIRTHS,
    capture_flame_presentation_births,
};
use solomon_sim::game_run::game_run_world_tick;
use solomon_sim::session::GameSession;

struct CommittedWorld {
    history: FlamePresentationHistory,
    births: Vec<FlamePresentationBirth>,
}

#[derive(Default)]
pub(super) struct HostFlameHistories {
    worlds: BTreeMap<String, CommittedWorld>,
    next_epoch: u64,
}

impl HostFlameHistories {
    pub fn observe(&mut self, scope: &str, state: &GameSession) -> Result<(), String> {
        let key = state
            .boneyard
            .as_ref()
            .map_or_else(|| "hub".into(), |world| format!("boneyard:{}", world.loaded.run_id));
        let tick = game_run_world_tick(state.ticks as f64, &state.run) as u64;
        self.observe_births(scope, &key, tick, capture_flame_presentation_births(&state.spells)?)
    }

    fn observe_births(
        &mut self,
        scope: &str,
        key: &str,
        tick: u64,
        births: Vec<FlamePresentationBirth>,
    ) -> Result<(), String> {
        if let Some(committed) = self.worlds.get_mut(scope) {
            let previous_epoch = committed.history.epoch();
            committed.history.advance(key, tick, &committed.births, &births)?;
            if committed.history.epoch() == previous_epoch {
                committed.births = births;
                return Ok(());
            }
            // Source allocates epochs across every world on this host. A local
            // History reset must never reuse another party's live epoch number.
        }
        self.next_epoch = self
            .next_epoch
            .checked_add(1)
            .filter(|epoch| *epoch <= 9_007_199_254_740_991)
            .ok_or("Flame history epoch exhausted")?;
        let history = FlamePresentationHistory::new(
            FlamePresentationHistoryOptions {
                world_key: key.into(),
                tick,
                maximum_births: MAX_FLAME_PRESENTATION_BIRTHS,
                maximum_bytes: Some(GAME_WEBSOCKET_MAX_PAYLOAD_BYTES / 4),
                initial_epoch: self.next_epoch,
            },
            birth_bytes,
        )?;
        self.worlds.insert(scope.into(), CommittedWorld { history, births });
        Ok(())
    }

    pub fn retain_scopes(&mut self, scopes: &[String]) {
        self.worlds.retain(|scope, _| scopes.contains(scope));
    }

    pub fn frame(
        &self,
        scope: &str,
        baseline: Option<FlamePresentationBaseline>,
    ) -> Result<FlamePresentationFrame, String> {
        let world = self.worlds.get(scope).ok_or("Flame history has no committed world")?;
        world.history.frame(baseline, world.history.tick())
    }

    pub fn baseline(&self, scope: &str) -> Result<FlamePresentationBaseline, String> {
        let world = self.worlds.get(scope).ok_or("Flame history has no committed world")?;
        Ok(FlamePresentationBaseline {
            epoch: world.history.epoch(),
            world_tick: world.history.tick(),
        })
    }

    pub fn discard_sent_history(
        &mut self,
        retained: &[FlamePresentationBaseline],
    ) -> Result<(), String> {
        for world in self.worlds.values_mut() {
            let oldest = retained
                .iter()
                .filter(|row| row.epoch == world.history.epoch())
                .map(|row| row.world_tick)
                .min()
                .unwrap_or(world.history.tick());
            world.history.discard_through_tick(oldest)?;
        }
        Ok(())
    }
}

fn birth_bytes(birth: &FlamePresentationBirth) -> Result<usize, String> {
    Ok(solomon_assets::json_value::stringify_serializable(birth)?.encode_utf16().count() * 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use solomon_protocol::flame_presentation::materialize_flame_presentation;
    use solomon_sim::game_run::GameRunPhase;
    use solomon_sim::modding::LuaValue;
    use solomon_sim::session::GameSessionOptions;

    #[test]
    fn actual_host_history_keeps_distinct_world_epochs_and_stopped_terminal_clock() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../sim/tests/fixtures/website/host-flame-goldens.json"
        ))
        .unwrap();
        let mut histories = HostFlameHistories::default();
        for (index, row) in fixture["rows"].as_array().unwrap().iter().enumerate() {
            let input = &row["input"];
            let tick = input["tick"].as_u64().unwrap();
            let stopped = input["gameOverTicks"].as_u64().unwrap_or(0);
            let world_tick = tick - stopped;
            let births: Vec<FlamePresentationBirth> =
                serde_json::from_value(input["births"].clone()).unwrap();
            // Feed complete canonical transients through the same capture the live
            // host uses. Materialization consumes no gameplay/presentation RNG.
            let mut state =
                GameSession::new_participants(Vec::new(), GameSessionOptions::default()).unwrap();
            state.ticks = tick;
            state.spells.transients = births
                .iter()
                .filter_map(|birth| materialize_flame_presentation(birth, world_tick as f64))
                .collect();
            if input["gameOverTicks"].is_u64() {
                state.run.phase = GameRunPhase::GameOver;
                state.run.game_over_ticks = stopped;
            }
            let captured = capture_flame_presentation_births(&state.spells).unwrap();
            histories
                .observe_births(
                    input["scope"].as_str().unwrap(),
                    input["key"].as_str().unwrap(),
                    game_run_world_tick(state.ticks as f64, &state.run) as u64,
                    captured,
                )
                .unwrap();
            let baseline = serde_json::from_value(input["baseline"].clone()).unwrap();
            let actual: LuaValue = serde_json::from_value(
                serde_json::to_value(
                    histories.frame(input["scope"].as_str().unwrap(), baseline).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let expected: LuaValue = serde_json::from_value(row["frame"].clone()).unwrap();
            assert_eq!(actual, expected, "Source host commit {index}");
        }
    }
}
