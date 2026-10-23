//! Publication observation and the durable-save/transport retirement barrier.

#[derive(Clone, Debug)]
pub(super) struct Publication {
    pub game_revision: String,
    pub sequence: u64,
    pub ready: bool,
}

pub(super) struct DeploymentState {
    current_revision: String,
    observed_publication: Option<u64>,
    session_active: bool,
    restart: Option<RestartBarrier>,
}

struct RestartBarrier {
    target_revision: String,
    sequence: u64,
    saved: bool,
    transport_retired: bool,
}

impl DeploymentState {
    pub fn new(current_revision: String) -> Self {
        Self { current_revision, observed_publication: None, session_active: false, restart: None }
    }

    pub fn begin(&mut self, target_revision: String, sequence: u64) {
        self.restart = Some(RestartBarrier {
            target_revision,
            sequence,
            saved: false,
            transport_retired: false,
        });
    }

    pub fn saved(&mut self) {
        if let Some(restart) = &mut self.restart {
            restart.saved = true;
        }
    }

    pub fn transport_retired(&mut self) {
        if let Some(restart) = &mut self.restart {
            restart.transport_retired = true;
        }
    }

    pub fn set_session_active(&mut self, active: bool) {
        self.session_active = active;
    }

    pub fn observe(&mut self, publication: &Publication) -> bool {
        if !publication.ready {
            return false;
        }
        if let Some(restart) = &self.restart {
            return restart.saved
                && restart.transport_retired
                && publication.sequence >= restart.sequence
                && publication.game_revision == restart.target_revision;
        }
        if self.session_active {
            return false;
        }
        if publication.game_revision != self.current_revision {
            return true;
        }
        match &self.observed_publication {
            Some(previous) => publication.sequence > *previous,
            None => {
                self.observed_publication = Some(publication.sequence);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn publication(game: &str, sequence: u64, ready: bool) -> Publication {
        Publication { game_revision: game.into(), sequence, ready }
    }

    #[test]
    fn unchanged_game_waits_for_durable_save_retirement_and_its_healthy_publication() {
        let mut state = DeploymentState::new("game-a".into());
        state.set_session_active(true);
        let target = publication("game-a", 2, true);
        assert!(!state.observe(&target));
        state.begin("game-a".into(), 2);
        assert!(!state.observe(&target));
        state.saved();
        assert!(!state.observe(&target));
        state.transport_retired();
        assert!(!state.observe(&publication("game-a", 1, true)));
        assert!(!state.observe(&publication("game-a", 2, false)));
        assert!(!state.observe(&publication("game-b", 2, true)));
        assert!(state.observe(&target));
    }

    #[test]
    fn changed_game_uses_the_same_barrier_and_accepts_reverse_callback_order() {
        let mut state = DeploymentState::new("game-a".into());
        state.begin("game-b".into(), 2);
        let target = publication("game-b", 2, true);
        state.transport_retired();
        assert!(!state.observe(&target));
        state.saved();
        assert!(!state.observe(&publication("game-c", 2, true)));
        assert!(state.observe(&target));
    }

    #[test]
    fn idle_observes_healthy_publications_and_active_sessions_require_an_announcement() {
        let mut state = DeploymentState::new("game-a".into());
        assert!(!state.observe(&publication("game-a", 1, false)));
        assert!(!state.observe(&publication("game-a", 1, true)));
        assert!(!state.observe(&publication("game-a", 1, true)));
        state.set_session_active(true);
        assert!(!state.observe(&publication("game-b", 2, true)));
        assert!(!state.observe(&publication("game-a", 2, true)));
        state.set_session_active(false);
        assert!(state.observe(&publication("game-a", 2, true)));
        let mut idle = DeploymentState::new("game-a".into());
        assert!(idle.observe(&publication("game-b", 2, true)));
    }
    #[test]
    fn later_healthy_publications_fulfill_missed_or_retried_updates() {
        for target in ["game-a", "game-b"] {
            let mut state = DeploymentState::new("game-a".into());
            state.begin(target.into(), 2);
            state.saved();
            state.transport_retired();
            assert!(!state.observe(&publication(target, 1, true)));
            assert!(!state.observe(&publication(target, 3, false)));
            assert!(!state.observe(&publication("game-c", 3, true)));
            assert!(state.observe(&publication(target, 2, true)));
            assert!(state.observe(&publication(target, 3, true)));
        }
    }
}
