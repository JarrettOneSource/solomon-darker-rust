//! Wall-clock gameplay holds from Website `host/game-host.ts:6269..6838`.
//! The session owns skill barriers; this host owns renderer-ready and resume-ready ACKs,
//! the pause owner and the two-second countdown while its authoritative clock is stopped.

use std::collections::BTreeSet;

use solomon_protocol::{GameplayPause, GameplayResumeGrace, PauseSource, ResumeGraceReason};

const RESUME_GRACE_MS: f64 = 2_000.0;

struct RendererReadiness {
    run_id: String,
    required: BTreeSet<String>,
    ready: BTreeSet<String>,
}

struct ResumeGrace {
    reason: ResumeGraceReason,
    sequence: u64,
    deadline_ms: Option<f64>,
    waiting: BTreeSet<String>,
    ready: BTreeSet<String>,
    required: BTreeSet<String>,
}

/// Borrowed membership and phase facts from the one canonical session. No player is
/// copied or simulated here; the host only decides whether its next tick may run.
pub struct ControlContext<'a> {
    pub active_run: bool,
    pub skill_barrier: bool,
    pub materialized: &'a BTreeSet<String>,
    pub connected: &'a BTreeSet<String>,
}

pub struct GameplayControl {
    pause: Option<GameplayPause>,
    grace: Option<ResumeGrace>,
    renderers: Option<RendererReadiness>,
    next_grace_sequence: u64,
    pub college_intro_ready: BTreeSet<String>,
}

impl Default for GameplayControl {
    fn default() -> Self {
        Self {
            pause: None,
            grace: None,
            renderers: None,
            next_grace_sequence: 1,
            college_intro_ready: BTreeSet::new(),
        }
    }
}

impl GameplayControl {
    #[must_use]
    pub fn pause(&self) -> Option<&GameplayPause> {
        self.pause.as_ref()
    }

    #[must_use]
    pub fn resume_grace(&self, now_ms: f64) -> Option<GameplayResumeGrace> {
        self.grace.as_ref().map(|grace| GameplayResumeGrace {
            reason: grace.reason,
            remaining_ms: grace
                .deadline_ms
                .map(|deadline| (deadline - now_ms).ceil().clamp(1.0, RESUME_GRACE_MS) as u64),
            sequence: grace.sequence,
        })
    }

    #[must_use]
    pub fn loading_run_id(&self) -> Option<&str> {
        self.renderers.as_ref().map(|readiness| readiness.run_id.as_str())
    }

    #[must_use]
    pub fn blocks_tick(&self) -> bool {
        self.pause.is_some() || self.grace.is_some() || self.renderers.is_some()
    }

    /// Arm after the canonical run and its native navigation have both been installed.
    pub fn arm_renderers(&mut self, run_id: &str, connected: &BTreeSet<String>) {
        self.renderers = (!connected.is_empty()).then(|| RendererReadiness {
            run_id: run_id.into(),
            required: connected.clone(),
            ready: BTreeSet::new(),
        });
    }

    /// A reconnect has a new renderer; peers already displaying this run retain theirs.
    /// Preserve any other clients still loading the initial world at this boundary.
    pub fn require_renderer(&mut self, player_id: &str, run_id: &str) {
        if let Some(readiness) = &mut self.renderers
            && readiness.run_id == run_id
        {
            readiness.required.insert(player_id.into());
            readiness.ready.remove(player_id);
        } else {
            self.arm_renderers(run_id, &BTreeSet::from([player_id.into()]));
        }
    }

    pub fn renderer_ready(&mut self, player_id: &str, run_id: &str) {
        let Some(readiness) = &mut self.renderers else { return };
        if readiness.run_id == run_id && readiness.required.contains(player_id) {
            readiness.ready.insert(player_id.into());
        }
    }

    /// A new run or rejoin invalidates previous resume-ready ACKs. Pending required
    /// rejoin identities survive, exactly as the source's `pendingExisting` branch does.
    pub fn begin_loading_grace(
        &mut self,
        player_id: &str,
        reason: ResumeGraceReason,
        connected: &BTreeSet<String>,
        required: &[String],
    ) {
        let previous = self.grace.take().filter(|grace| grace.deadline_ms.is_none());
        let mut waiting = connected.clone();
        waiting.insert(player_id.into());
        let mut required_players = BTreeSet::new();
        let mut reason = reason;
        if let Some(previous) = previous {
            waiting.extend(previous.waiting);
            required_players = previous.required;
            if previous.reason == ResumeGraceReason::PartyRejoinWait {
                reason = previous.reason;
            }
        }
        required_players.extend(required.iter().cloned());
        waiting.extend(required_players.iter().cloned());
        let sequence = self.take_grace_sequence();
        self.grace = Some(ResumeGrace {
            reason,
            sequence,
            deadline_ms: None,
            waiting,
            ready: BTreeSet::new(),
            required: required_players,
        });
    }

    pub fn begin_surface_grace(
        &mut self,
        player_id: &str,
        reason: ResumeGraceReason,
        context: &ControlContext<'_>,
        now_ms: f64,
    ) {
        if self.grace.is_some() {
            self.start_grace_if_ready(context, now_ms);
            return;
        }
        if !context.active_run {
            return;
        }
        let picker_close = reason == ResumeGraceReason::SkillPickerClosed;
        if picker_close && !context.connected.contains(player_id) {
            return;
        }
        let mut waiting = BTreeSet::new();
        if picker_close {
            waiting.insert(player_id.into());
        }
        let sequence = self.take_grace_sequence();
        self.grace = Some(ResumeGrace {
            reason,
            sequence,
            deadline_ms: (!picker_close).then_some(now_ms + RESUME_GRACE_MS),
            waiting,
            ready: BTreeSet::new(),
            required: BTreeSet::new(),
        });
    }

    pub fn resume_ready(
        &mut self,
        player_id: &str,
        sequence: u64,
        context: &ControlContext<'_>,
        now_ms: f64,
    ) {
        let Some(grace) = &mut self.grace else { return };
        if grace.sequence == sequence && grace.waiting.contains(player_id) {
            grace.ready.insert(player_id.into());
            self.start_grace_if_ready(context, now_ms);
        }
    }

    /// Only the current owner can change or release a pause. The host neutralizes every
    /// connection input when this changes, so queued casts cannot escape a closed surface.
    pub fn request_pause(
        &mut self,
        player_id: &str,
        display_name: &str,
        source: PauseSource,
        context: &ControlContext<'_>,
    ) -> bool {
        if !context.active_run || self.grace.is_some() {
            return false;
        }
        if let Some(pause) = &mut self.pause {
            if pause.owner_player_id != player_id || pause.source == source {
                return false;
            }
            pause.source = source;
            return true;
        }
        if context.skill_barrier {
            return false;
        }
        self.pause = Some(GameplayPause {
            owner_player_id: player_id.into(),
            owner_display_name: display_name.into(),
            source,
        });
        true
    }

    pub fn release_pause(
        &mut self,
        player_id: &str,
        owner_resumed: bool,
        context: &ControlContext<'_>,
        now_ms: f64,
    ) -> bool {
        if self.pause.as_ref().is_none_or(|pause| pause.owner_player_id != player_id) {
            return false;
        }
        let pause = self.pause.take().expect("the matching pause owner is present");
        if owner_resumed && let Some(reason) = pause.source.resume_grace_reason() {
            self.begin_surface_grace(player_id, reason, context, now_ms);
        }
        self.start_grace_if_ready(context, now_ms);
        true
    }

    pub fn disconnected(&mut self, player_id: &str, context: &ControlContext<'_>, now_ms: f64) {
        self.college_intro_ready.remove(player_id);
        if let Some(readiness) = &mut self.renderers {
            readiness.required.remove(player_id);
            readiness.ready.remove(player_id);
        }
        if let Some(grace) = &mut self.grace
            && grace.deadline_ms.is_none()
        {
            grace.ready.remove(player_id);
            if !grace.required.contains(player_id) {
                grace.waiting.remove(player_id);
                if grace.waiting.is_empty() {
                    self.grace = None;
                }
            }
        }
        self.release_pause(player_id, false, context, now_ms);
        self.advance(context, now_ms);
    }

    /// Run before deciding whether to tick. Readiness is tested against actual current
    /// membership, and an expired countdown clears while the gameplay clock is frozen.
    pub fn advance(&mut self, context: &ControlContext<'_>, now_ms: f64) {
        if !context.active_run {
            self.pause = None;
            self.grace = None;
            self.renderers = None;
            return;
        }
        if self.renderers.as_ref().is_some_and(|readiness| {
            readiness
                .required
                .iter()
                .all(|id| !context.connected.contains(id) || readiness.ready.contains(id))
        }) {
            self.renderers = None;
        }
        let Some(grace) = &mut self.grace else { return };
        if let Some(deadline) = grace.deadline_ms {
            if now_ms >= deadline {
                self.grace = None;
            }
            return;
        }
        self.start_grace_if_ready(context, now_ms);
    }

    fn start_grace_if_ready(&mut self, context: &ControlContext<'_>, now_ms: f64) {
        let Some(grace) = &mut self.grace else { return };
        if !context.active_run || grace.deadline_ms.is_some() {
            return;
        }
        if context.skill_barrier
            || self.pause.is_some()
            || grace
                .waiting
                .iter()
                .any(|id| !grace.ready.contains(id) || !context.materialized.contains(id))
        {
            return;
        }
        if grace.reason == ResumeGraceReason::SkillPickerClosed {
            self.grace = None;
            return;
        }
        grace.deadline_ms = Some(now_ms + RESUME_GRACE_MS);
    }

    fn take_grace_sequence(&mut self) -> u64 {
        let sequence = self.next_grace_sequence;
        self.next_grace_sequence = if sequence == 0x7fff_ffff { 1 } else { sequence + 1 };
        sequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn reason(value: &Value) -> ResumeGraceReason {
        serde_json::from_value(json!(value.as_str().unwrap().replace('-', "_"))).unwrap()
    }

    #[test]
    fn renderer_pause_picker_and_rejoin_holds_replay_the_pinned_host() {
        let fixture: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/website/host-control-goldens.json"
        )))
        .unwrap();
        for scenario in fixture["scenarios"].as_array().unwrap() {
            let mut control = GameplayControl::default();
            let mut connected: BTreeSet<_> = ["a".to_owned(), "b".to_owned()].into();
            let mut materialized = connected.clone();
            let mut now = 0.0;
            let mut barrier = false;
            for (index, record) in scenario["steps"].as_array().unwrap().iter().enumerate() {
                let action = &record["action"];
                if let Some(value) = action["now"].as_f64() {
                    now = value;
                }
                let player = action["player"].as_str().unwrap_or("a");
                let kind = action["kind"].as_str().unwrap();
                if kind == "disconnect" {
                    connected.remove(player);
                    materialized.remove(player);
                }
                if kind == "rejoin" {
                    connected.insert(player.into());
                    materialized.insert(player.into());
                }
                let context = ControlContext {
                    active_run: true,
                    skill_barrier: barrier,
                    connected: &connected,
                    materialized: &materialized,
                };
                match kind {
                    "arm" => control.arm_renderers("run-a", &connected),
                    "loading" => {
                        let required: Vec<_> = action["required"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|id| id.as_str().unwrap().to_owned())
                            .collect();
                        control.begin_loading_grace(
                            player,
                            reason(&action["reason"]),
                            &connected,
                            &required,
                        );
                    }
                    "renderer" => control.renderer_ready(player, action["run"].as_str().unwrap()),
                    "resume_ready" => control.resume_ready(
                        player,
                        action["sequence"].as_u64().unwrap(),
                        &context,
                        now,
                    ),
                    "advance" => control.advance(&context, now),
                    "pause" => {
                        let source = serde_json::from_value(json!(
                            action["source"].as_str().unwrap().replace('-', "_")
                        ))
                        .unwrap();
                        control.request_pause(player, &player.to_uppercase(), source, &context);
                    }
                    "unpause" => {
                        control.release_pause(player, true, &context, now);
                    }
                    "surface" => {
                        control.begin_surface_grace(
                            player,
                            reason(&action["reason"]),
                            &context,
                            now,
                        );
                    }
                    "barrier" => barrier = action["enabled"].as_bool().unwrap(),
                    "disconnect" => control.disconnected(player, &context, now),
                    "rejoin" => {}
                    "sequence" => {
                        control.next_grace_sequence = action["value"].as_u64().unwrap();
                    }
                    _ => panic!("the source fixture uses a known action"),
                }
                let pause = control.pause().map(|pause| {
                    json!({
                        "ownerPlayerId":pause.owner_player_id,
                        "ownerDisplayName":pause.owner_display_name,
                        "source":serde_json::to_value(pause.source).unwrap().as_str().unwrap().replace('_',"-")
                    })
                });
                let grace = control.resume_grace(now).map(|grace| {
                    json!({
                        "reason":serde_json::to_value(grace.reason).unwrap().as_str().unwrap().replace('_',"-"),
                        "remainingMs":grace.remaining_ms,
                        "sequence":grace.sequence
                    })
                });
                let actual = json!({
                    "pause":pause,
                    "grace":grace,
                    "loadingRunId":control.loading_run_id(),
                    "blocked":control.blocks_tick()
                });
                assert_eq!(actual, record["expected"], "{} step {index}", scenario["name"]);
            }
        }
    }
}
