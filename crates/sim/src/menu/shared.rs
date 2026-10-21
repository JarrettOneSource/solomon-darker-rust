//! Source `gameplay-resume-grace.ts` and `pause-menu-contract.ts` presentation.
//! These projections explain an existing host hold; they never start or clear one.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameplayPausePresentation {
    Owner,
    Waiting { label: String, detail: String },
}

#[must_use]
pub fn gameplay_pause_presentation(
    owner_player_id: &str,
    local_player_id: &str,
    owner_display_name: &str,
    activity: Option<&str>,
) -> GameplayPausePresentation {
    if owner_player_id == local_player_id {
        return GameplayPausePresentation::Owner;
    }
    let (label, detail) = match activity {
        Some(activity) => (
            format!("{owner_display_name} is using {activity}."),
            format!("Waiting for {owner_display_name} to close {activity}."),
        ),
        None => (
            format!("{owner_display_name} has paused the game."),
            format!("Waiting for {owner_display_name} to resume."),
        ),
    };
    GameplayPausePresentation::Waiting { label, detail }
}

/// Interpolate only the received two-second progress bar. `None` remains an
/// indefinite wait; reaching one does not authorize the client to resume play.
#[must_use]
pub fn gameplay_resume_grace_progress(remaining_ms: Option<f64>, elapsed_ms: f64) -> Option<f64> {
    let remaining_ms = remaining_ms?;
    let remaining = (remaining_ms - elapsed_ms.max(0.0)).max(0.0);
    Some((1.0 - remaining / 2000.0).clamp(0.0, 1.0))
}
