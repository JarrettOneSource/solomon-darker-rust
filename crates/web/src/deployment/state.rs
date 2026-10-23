//! Publication observation and the durable-save/transport retirement barrier.

#[derive(Clone, Debug)]
pub(super) struct Publication {
    pub game_revision: String,
    pub deployment_id: String,
    pub ready: bool,
}

pub(super) struct DeploymentState {
    current_revision: String,
    observed_publication: Option<String>,
    session_active: bool,
    restart: Option<RestartBarrier>,
}

struct RestartBarrier {
    target_revision: String,
    deployment_id: String,
    saved: bool,
    transport_retired: bool,
}

impl DeploymentState {
    pub fn new(current_revision: String) -> Self {
        Self { current_revision, observed_publication: None, session_active: false, restart: None }
    }

    pub fn begin(&mut self, target_revision: String, deployment_id: String) {
        self.restart = Some(RestartBarrier {
            target_revision,
            deployment_id,
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
                && publication.deployment_id == restart.deployment_id
                && publication.game_revision == restart.target_revision;
        }
        if self.session_active {
            return false;
        }
        if publication.game_revision != self.current_revision {
            return true;
        }
        match &self.observed_publication {
            Some(previous) => previous != &publication.deployment_id,
            None => {
                self.observed_publication = Some(publication.deployment_id.clone());
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn publication(game: &str, deployment: &str, ready: bool) -> Publication {
        Publication { game_revision: game.into(), deployment_id: deployment.into(), ready }
    }

    #[test]
    fn unchanged_game_waits_for_durable_save_retirement_and_its_healthy_publication() {
        let mut state = DeploymentState::new("game-a".into());
        state.set_session_active(true);
        let target = publication("game-a", "publication-b", true);
        assert!(!state.observe(&target));
        state.begin("game-a".into(), "publication-b".into());
        assert!(!state.observe(&target));
        state.saved();
        assert!(!state.observe(&target));
        state.transport_retired();
        assert!(!state.observe(&publication("game-a", "publication-a", true)));
        assert!(!state.observe(&publication("game-a", "publication-b", false)));
        assert!(!state.observe(&publication("game-b", "publication-b", true)));
        assert!(state.observe(&target));
    }

    #[test]
    fn changed_game_uses_the_same_barrier_and_accepts_reverse_callback_order() {
        let mut state = DeploymentState::new("game-a".into());
        state.begin("game-b".into(), "publication-b".into());
        let target = publication("game-b", "publication-b", true);
        state.transport_retired();
        assert!(!state.observe(&target));
        state.saved();
        assert!(!state.observe(&publication("game-c", "publication-b", true)));
        assert!(state.observe(&target));
    }

    #[test]
    fn idle_observes_healthy_publications_and_active_sessions_require_an_announcement() {
        let mut state = DeploymentState::new("game-a".into());
        assert!(!state.observe(&publication("game-a", "pending", false)));
        assert!(!state.observe(&publication("game-a", "publication-a", true)));
        assert!(!state.observe(&publication("game-a", "publication-a", true)));
        state.set_session_active(true);
        assert!(!state.observe(&publication("game-b", "publication-b", true)));
        assert!(!state.observe(&publication("game-a", "publication-b", true)));
        state.set_session_active(false);
        assert!(state.observe(&publication("game-a", "publication-b", true)));
        let mut idle = DeploymentState::new("game-a".into());
        assert!(idle.observe(&publication("game-b", "publication-b", true)));
    }
}
