//! Ordinary death/spectator camera and status from current
//! `renderer/boneyard-render-contract.ts` and `native-spectator-hud.ts`.
//! This selects display ownership only; it never changes the local participant.

use super::BoneyardBounds;
use super::waves::director::WavePhase;
use crate::Vec2;
use crate::combat::PlayerLifeState;
use crate::game_run::GameRunPhase;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoneyardSpectatorCameraState {
    pub run_id: Option<String>,
    pub target_player_id: Option<String>,
}

#[derive(Clone, Copy)]
pub struct BoneyardSpectatorPlayer<'a> {
    pub id: &'a str,
    pub display_name: &'a str,
    pub position: Vec2,
    pub life_state: PlayerLifeState,
}

pub struct BoneyardSpectatorSnapshot<'a> {
    pub players: &'a [BoneyardSpectatorPlayer<'a>],
    pub eligible_player_ids: &'a [String],
    pub phase: GameRunPhase,
    pub run_id: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardCameraFocus {
    pub player_id: Option<String>,
    pub position: Vec2,
}

#[derive(Clone, Copy)]
pub struct BoneyardSpectatorWaves {
    pub pending_spawn_budget: usize,
    pub wave_ordinal: u64,
    pub phase: WavePhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoneyardSpectatorStatusPresentation {
    pub accessible_label: String,
    pub active_enemy_count: usize,
    pub display_text: String,
    pub incoming_enemy_count: usize,
    pub instruction: Option<&'static str>,
    pub respawn_text: String,
    pub run_id: String,
    pub target_player_id: Option<String>,
    pub title: String,
    pub wave_ordinal: Option<u64>,
    pub wave_phase: Option<WavePhase>,
}

#[must_use]
pub fn boneyard_spectator_camera_state(
    snapshot: &BoneyardSpectatorSnapshot<'_>,
    local_player_id: &str,
    current: &BoneyardSpectatorCameraState,
    advance: bool,
) -> BoneyardSpectatorCameraState {
    let run_id = snapshot.run_id.map(str::to_owned);
    let local = snapshot.players.iter().find(|player| player.id == local_player_id);
    if snapshot.phase != GameRunPhase::Active
        || run_id.is_none()
        || local.is_none_or(|player| player.life_state != PlayerLifeState::Spectating)
    {
        return BoneyardSpectatorCameraState { run_id, target_player_id: None };
    }
    let target_id =
        if current.run_id == run_id { current.target_player_id.as_deref() } else { None };
    if !advance
        && let Some(id) = target_id
        && snapshot.eligible_player_ids.iter().any(|eligible| eligible == id)
        && snapshot.players.iter().any(|player| {
            player.id == id
                && matches!(
                    player.life_state,
                    PlayerLifeState::Alive
                        | PlayerLifeState::LethalPending
                        | PlayerLifeState::Dying
                )
        })
    {
        return BoneyardSpectatorCameraState { run_id, target_player_id: Some(id.to_owned()) };
    }
    let mut living: Vec<_> = snapshot
        .eligible_player_ids
        .iter()
        .filter(|id| {
            id.as_str() != local_player_id
                && snapshot.players.iter().any(|player| {
                    player.id == id.as_str() && player.life_state == PlayerLifeState::Alive
                })
        })
        .collect();
    // Source compares semantic string IDs lexically, not registration ordinals.
    living.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    let index = if advance {
        target_id
            .and_then(|id| living.iter().position(|candidate| candidate.as_str() == id))
            .map_or(0, |index| (index + 1) % living.len())
    } else {
        0
    };
    BoneyardSpectatorCameraState {
        run_id,
        target_player_id: living.get(index).map(|id| (*id).clone()),
    }
}

#[must_use]
pub fn boneyard_camera_focus(
    snapshot: &BoneyardSpectatorSnapshot<'_>,
    local_player_id: &str,
    spectator: &BoneyardSpectatorCameraState,
    fallback: Vec2,
) -> BoneyardCameraFocus {
    let local = snapshot.players.iter().find(|player| player.id == local_player_id);
    let target = if snapshot.phase == GameRunPhase::Active
        && snapshot.run_id == spectator.run_id.as_deref()
        && local.is_some_and(|player| player.life_state == PlayerLifeState::Spectating)
    {
        spectator
            .target_player_id
            .as_deref()
            .and_then(|id| snapshot.players.iter().find(|player| player.id == id))
    } else {
        None
    };
    let player = target.or(local);
    BoneyardCameraFocus {
        player_id: player.map(|player| player.id.to_owned()),
        position: player.map_or(fallback, |player| player.position),
    }
}

#[must_use]
pub fn boneyard_spectator_status(
    snapshot: &BoneyardSpectatorSnapshot<'_>,
    local_player_id: &str,
    spectator: &BoneyardSpectatorCameraState,
    active_enemy_count: usize,
    waves: Option<BoneyardSpectatorWaves>,
) -> Option<BoneyardSpectatorStatusPresentation> {
    let run_id = snapshot.run_id?;
    if snapshot.phase != GameRunPhase::Active
        || spectator.run_id.as_deref() != Some(run_id)
        || !snapshot.players.iter().any(|player| {
            player.id == local_player_id && player.life_state == PlayerLifeState::Spectating
        })
    {
        return None;
    }
    let target = spectator
        .target_player_id
        .as_deref()
        .and_then(|id| snapshot.players.iter().find(|player| player.id == id));
    let title = target.map_or_else(
        || "Spectating - waiting for an alive player".to_owned(),
        |player| format!("Spectating {}", player.display_name),
    );
    let instruction = target.map(|_| "Left / Right click: next player");
    let display_text = instruction
        .map_or_else(|| title.clone(), |instruction| format!("{title}  |  {instruction}"));
    let target_label = if instruction.is_some() {
        format!("{title}. Left or right click to select the next player.")
    } else {
        format!("{title}.")
    };
    let (respawn_text, respawn_label, incoming_enemy_count) = match waves {
        None => (
            format!("RESPAWN: WAITING FOR NEXT WAVE  |  {active_enemy_count} ACTIVE"),
            format!(
                "Respawn is waiting for the next wave. {}.",
                enemy_count_label(active_enemy_count, "active")
            ),
            0,
        ),
        Some(wave) => {
            let (label, accessible) = if wave.wave_ordinal == 0 {
                (String::from("OPENING"), String::from("The opening wave"))
            } else {
                (format!("WAVE {}", wave.wave_ordinal), format!("Wave {}", wave.wave_ordinal))
            };
            (
                format!(
                    "RESPAWN NEXT WAVE  |  {label}  |  {active_enemy_count} ACTIVE + {} INCOMING",
                    wave.pending_spawn_budget
                ),
                format!(
                    "Respawn at the next wave. {accessible} has {} and {}.",
                    enemy_count_label(active_enemy_count, "active"),
                    enemy_count_label(wave.pending_spawn_budget, "incoming")
                ),
                wave.pending_spawn_budget,
            )
        }
    };
    Some(BoneyardSpectatorStatusPresentation {
        accessible_label: format!("{target_label} {respawn_label}"),
        active_enemy_count,
        display_text,
        incoming_enemy_count,
        instruction,
        respawn_text,
        run_id: run_id.to_owned(),
        target_player_id: target.map(|player| player.id.to_owned()),
        title,
        wave_ordinal: waves.map(|wave| wave.wave_ordinal),
        wave_phase: waves.map(|wave| wave.phase),
    })
}

fn enemy_count_label(count: usize, state: &str) -> String {
    format!("{count} {state} {}", if count == 1 { "enemy" } else { "enemies" })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSpectatorHudRail {
    pub bounds: BoneyardBounds,
    pub record: usize,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSpectatorHudCorner {
    pub center: Vec2,
    pub record: usize,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSpectatorHudLayout {
    pub surface: BoneyardBounds,
    pub text: Vec2,
    pub horizontal_rails: [NativeSpectatorHudRail; 2],
    pub vertical_rails: [NativeSpectatorHudRail; 2],
    pub corners: [NativeSpectatorHudCorner; 4],
}

#[must_use]
pub fn native_spectator_hud_layout(viewport: Vec2) -> NativeSpectatorHudLayout {
    assert!(
        viewport.x.is_finite() && viewport.y.is_finite() && viewport.x > 0.0 && viewport.y > 0.0,
        "spectator HUD viewport must be positive and finite"
    );
    let surface = BoneyardBounds::new(
        viewport.x * 0.2,
        viewport.y * 0.055,
        viewport.x * 0.60,
        viewport.y * 0.075,
    );
    let centers = [
        (37.0, 37.0),
        (surface.width - 37.0, 37.0),
        (37.0, surface.height - 37.0),
        (surface.width - 37.0, surface.height - 37.0),
    ];
    NativeSpectatorHudLayout {
        surface,
        text: Vec2::new(18.0, 20.0),
        horizontal_rails: [-2.0, surface.height - 15.0].map(|y| NativeSpectatorHudRail {
            record: 10,
            bounds: BoneyardBounds::new(10.0, y, surface.width - 20.0, 19.0),
        }),
        vertical_rails: [0.0, surface.width - 17.0].map(|x| NativeSpectatorHudRail {
            record: 79,
            bounds: BoneyardBounds::new(x, 10.0, 21.0, surface.height - 20.0),
        }),
        corners: std::array::from_fn(|index| NativeSpectatorHudCorner {
            center: Vec2::new(centers[index].0, centers[index].1),
            record: 107 + index,
        }),
    }
}
