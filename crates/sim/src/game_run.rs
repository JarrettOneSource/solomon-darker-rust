//! Session run lifecycle, ported from `core-kernels/game-run.ts`.
//!
//! A terminal run remains visible through the native Game Over gate and fade, then waits
//! for each eligible player to confirm the next loadout before returning to the College.

pub mod presentation;

pub const GAME_OVER_ENTRY_FADE_TICKS: u64 = 40;
pub const GAME_OVER_INPUT_ACCEPT_TICK: u64 = 500;
pub const GAME_OVER_INPUT_EXIT_FADE_TICKS: u64 = 20;
pub const GAME_OVER_AUTOMATIC_ACCEPT_TICK: u64 = 951;
pub const GAME_OVER_AUTOMATIC_EXIT_FADE_TICKS: u64 = 250;

/// Current Source `gameRunWorldTick`: terminal UI keeps ticking while the resident
/// Boneyard and its retained presentation history stay at their stopped world clock.
#[must_use]
pub fn game_run_world_tick(tick: f64, run: &GameRunLifecycle) -> f64 {
    if run.phase == GameRunPhase::GameOver {
        tick.floor() - run.game_over_ticks as f64
    } else {
        tick
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum GameRunPhase {
    Hub,
    Active,
    GameOver,
    Loadout,
}

impl GameRunPhase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Hub => "hub",
            Self::Active => "active",
            Self::GameOver => "game-over",
            Self::Loadout => "loadout",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum GameOverExitKind {
    Automatic,
    Input,
}

impl GameOverExitKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Input => "input",
        }
    }

    #[must_use]
    pub const fn duration_ticks(self) -> u64 {
        match self {
            Self::Automatic => GAME_OVER_AUTOMATIC_EXIT_FADE_TICKS,
            Self::Input => GAME_OVER_INPUT_EXIT_FADE_TICKS,
        }
    }
}

/// Authority for run identity, all-dead detection, Game Over timing and post-run admission.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameRunLifecycle {
    pub eligible_player_ids: Vec<String>,
    pub game_over_event_id: u64,
    pub game_over_exit_kind: Option<GameOverExitKind>,
    pub game_over_exit_ticks: Option<u64>,
    pub game_over_ticks: u64,
    pub last_completed_run_id: Option<String>,
    pub loadout_ready_player_ids: Vec<String>,
    pub next_game_over_event_id: u64,
    pub phase: GameRunPhase,
    pub run_id: Option<String>,
}

impl Default for GameRunLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl GameRunLifecycle {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            eligible_player_ids: Vec::new(),
            game_over_event_id: 0,
            game_over_exit_kind: None,
            game_over_exit_ticks: None,
            game_over_ticks: 0,
            last_completed_run_id: None,
            loadout_ready_player_ids: Vec::new(),
            next_game_over_event_id: 1,
            phase: GameRunPhase::Hub,
            run_id: None,
        }
    }

    /// `startGameRun`: only the College can admit a new run.
    pub fn start(&mut self, run_id: String, player_ids: &[String]) {
        assert!(!run_id.is_empty(), "game run id must not be empty");
        assert_eq!(self.phase, GameRunPhase::Hub, "a game run can start only in the College");
        self.eligible_player_ids = stable_player_ids(player_ids);
        self.game_over_event_id = 0;
        self.game_over_exit_kind = None;
        self.game_over_exit_ticks = None;
        self.game_over_ticks = 0;
        self.loadout_ready_player_ids.clear();
        self.phase = GameRunPhase::Active;
        self.run_id = Some(run_id);
    }

    pub fn synchronize_participants(&mut self, player_ids: &[String]) {
        if self.phase == GameRunPhase::Hub {
            return;
        }
        self.eligible_player_ids = stable_player_ids(player_ids);
        self.loadout_ready_player_ids.retain(|id| self.eligible_player_ids.contains(id));
        if self.phase == GameRunPhase::Loadout
            && !self.eligible_player_ids.is_empty()
            && self.loadout_ready_player_ids.len() == self.eligible_player_ids.len()
        {
            self.complete_post_run_loadout();
        }
    }

    /// Advance once per native tick. A run ends only after every eligible player has died.
    pub fn step(&mut self, alive_player_ids: &[String]) {
        if self.phase == GameRunPhase::GameOver {
            let Some(exit_ticks) = self.game_over_exit_ticks else {
                self.game_over_ticks += 1;
                if self.game_over_ticks == GAME_OVER_AUTOMATIC_ACCEPT_TICK {
                    self.game_over_exit_kind = Some(GameOverExitKind::Automatic);
                    self.game_over_exit_ticks = Some(1);
                }
                return;
            };
            let kind = self.game_over_exit_kind.expect("Game Over exit ticks have an exit kind");
            if exit_ticks < kind.duration_ticks() {
                self.game_over_exit_ticks = Some(exit_ticks + 1);
                self.game_over_ticks += 1;
                return;
            }
            self.game_over_exit_kind = None;
            self.game_over_exit_ticks = None;
            self.game_over_ticks = 0;
            self.last_completed_run_id = self.run_id.take();
            self.loadout_ready_player_ids.clear();
            self.phase = GameRunPhase::Loadout;
            return;
        }
        if self.phase != GameRunPhase::Active || self.eligible_player_ids.is_empty() {
            return;
        }
        if self.eligible_player_ids.iter().any(|id| alive_player_ids.contains(id)) {
            return;
        }
        self.game_over_event_id = self.next_game_over_event_id;
        self.game_over_exit_kind = None;
        self.game_over_exit_ticks = None;
        self.game_over_ticks = 0;
        self.loadout_ready_player_ids.clear();
        self.next_game_over_event_id += 1;
        self.phase = GameRunPhase::GameOver;
    }

    /// Input before tick 500, a stale event, or a duplicate continue cannot skip the fade.
    pub fn continue_game_over(&mut self, run_id: &str, event_id: u64) -> bool {
        if self.phase != GameRunPhase::GameOver
            || self.game_over_exit_ticks.is_some()
            || self.game_over_ticks < GAME_OVER_INPUT_ACCEPT_TICK
            || self.run_id.as_deref() != Some(run_id)
            || self.game_over_event_id != event_id
        {
            return false;
        }
        self.game_over_exit_kind = Some(GameOverExitKind::Input);
        self.game_over_exit_ticks = Some(1);
        true
    }

    pub fn confirm_post_run_loadout(&mut self, player_id: &str) -> bool {
        if self.phase != GameRunPhase::Loadout
            || !self.eligible_player_ids.iter().any(|id| id == player_id)
            || self.loadout_ready_player_ids.iter().any(|id| id == player_id)
        {
            return false;
        }
        self.loadout_ready_player_ids.push(player_id.into());
        self.loadout_ready_player_ids.sort();
        if self.loadout_ready_player_ids.len() == self.eligible_player_ids.len() {
            self.complete_post_run_loadout();
        }
        true
    }

    pub fn continue_post_run_to_college_intro(&mut self) -> bool {
        if self.phase != GameRunPhase::Loadout {
            return false;
        }
        self.complete_post_run_loadout();
        true
    }

    fn complete_post_run_loadout(&mut self) {
        self.eligible_player_ids.clear();
        self.game_over_event_id = 0;
        self.loadout_ready_player_ids.clear();
        self.phase = GameRunPhase::Hub;
    }
}

fn stable_player_ids(player_ids: &[String]) -> Vec<String> {
    let mut ids = player_ids.to_vec();
    ids.sort();
    ids.dedup();
    ids
}
