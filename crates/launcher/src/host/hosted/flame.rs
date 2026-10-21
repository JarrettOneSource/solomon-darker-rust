//! Presentation commit hooks surround actual host operations, including same-tick writes.
use super::{HostedGameHost, HostedWorlds};
use solomon_protocol::flame_presentation::{FlamePresentationBaseline, FlamePresentationFrame};
use solomon_sim::session::GameSession;

impl HostedGameHost {
    pub(in crate::host) fn commit_flame_histories(&mut self) -> Result<(), String> {
        let mut scopes = Vec::new();
        match &self.worlds {
            HostedWorlds::Global(worlds) => {
                scopes.push("hub".into());
                self.flame_histories.observe("hub", &worlds.hub)?;
                for run in &worlds.runs {
                    let scope = format!("party:{}", run.party_id);
                    self.flame_histories.observe(&scope, &run.state)?;
                    scopes.push(scope);
                }
            }
            HostedWorlds::Private { state, .. } => {
                scopes.push("private".into());
                self.flame_histories.observe("private", state)?;
            }
        }
        self.flame_histories.retain_scopes(&scopes);
        Ok(())
    }

    fn flame_scope(&self, state: &GameSession) -> Result<String, String> {
        match &self.worlds {
            HostedWorlds::Private { .. } => Ok("private".into()),
            HostedWorlds::Global(worlds) => {
                if std::ptr::eq(state, &worlds.hub) {
                    return Ok("hub".into());
                }
                worlds
                    .runs
                    .iter()
                    .find(|run| std::ptr::eq(state, &run.state))
                    .map(|run| format!("party:{}", run.party_id))
                    .ok_or_else(|| "Flame viewer has no actual world".into())
            }
        }
    }

    pub(super) fn flame_frame(
        &self,
        state: &GameSession,
        baseline: Option<FlamePresentationBaseline>,
    ) -> Result<FlamePresentationFrame, String> {
        self.flame_histories.frame(&self.flame_scope(state)?, baseline)
    }

    pub(super) fn flame_baseline(
        &self,
        state: &GameSession,
    ) -> Result<FlamePresentationBaseline, String> {
        self.flame_histories.baseline(&self.flame_scope(state)?)
    }

    pub(super) fn flame_baseline_for_viewer(
        &self,
        id: &str,
    ) -> Result<FlamePresentationBaseline, String> {
        let state = self
            .worlds
            .state_for_player(id)
            .or_else(|| {
                let slot = self.rejoin_slots.iter().find(|slot| slot.player_id == id)?;
                self.active_run(&slot.party_id, &slot.run_id)
            })
            .ok_or("Flame viewer has no retained world")?;
        self.flame_baseline(state)
    }

    pub(super) fn prune_flame_history(&mut self) -> Result<(), String> {
        let retained: Vec<_> =
            self.connections.values().flat_map(|peer| peer.retained_flame_baselines()).collect();
        self.flame_histories.discard_sent_history(&retained)
    }
}
