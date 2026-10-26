//! Source-shaped views shared by the public Game entry and tick replays.

use serde_json::{Value, json};
use solomon_sim::game_run::GameRunLifecycle;
use solomon_sim::hub::regions::HubTransitionPhase;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::score::NativeHallOfFameRunState;
use solomon_sim::session::{ParticipantRead, PlayerLightingState};

pub fn registration(value: PainterRegistration) -> Value {
    json!({"managerLane":match value.manager_lane {
        WorldManagerLane::Actor => "actor", WorldManagerLane::Transient => "transient"
    }, "registrationOrdinal":value.registration_ordinal})
}

pub fn character(session: &impl ParticipantRead) -> Value {
    let session = session.view();
    let wizard = &session.wizard;
    let primary = &wizard.primary_cast;
    let selection = &session.player.selection;
    json!({"config":{"displayName":selection.name,"element":selection.element.as_str(),
        "discipline":selection.discipline.as_str()},
        "footstepTick":wizard.footstep_tick,"gaitDegrees":wizard.gait_degrees,
        "headingIndex":wizard.heading_index,"position":{"x":wizard.position.x,"y":wizard.position.y},
        "velocity":{"x":wizard.velocity.x,"y":wizard.velocity.y},"walkCyclePrimary":wizard.walk_cycle_primary,
        "primaryCast":{"actionTick":primary.action_tick.unwrap_or(-1.0),
            "aimDirection":{"x":primary.aim_direction.x,"y":primary.aim_direction.y},
            "castSequence":primary.cast_sequence,"channelActive":primary.channel_active,
            "emissionSequence":primary.emission_sequence,"etherBlastCharge":primary.ether_blast_charge,
            "etherBlastChargeCueSequence":primary.ether_blast_charge_cue_sequence,
            "fizzleSequence":primary.fizzle_sequence,"held":primary.held,
            "lastWeldPlaybackRate":primary.last_weld_playback_rate,
            "lastWeldSoundVariant":primary.last_weld_sound_variant,
            "oneShotAttackPoseHeld":primary.one_shot_attack_pose_held,
            "selectedPrimaryAgeTicks":primary.selected_primary_age_ticks,
            "selectedPrimaryId":primary.selected_primary_id.map_or(-1,i64::from),
            "targetId":primary.target_id,"underpowered":primary.underpowered,
            "weaponPulse":primary.weapon_pulse}})
}

pub fn lighting(state: &PlayerLightingState) -> Value {
    json!({"blindnessTicksRemaining":state.blindness_ticks_remaining,
        "deathWeaponPainterRegistration":state.death_weapon_painter_registration.map(registration),
        "lightRegistration":registration(state.light_registration),
        "overlayEffectPhase":state.overlay_effect_phase})
}

pub fn run(run: &GameRunLifecycle) -> Value {
    json!({"eligiblePlayerIds":run.eligible_player_ids,"gameOverEventId":run.game_over_event_id,
        "gameOverExitKind":run.game_over_exit_kind.map(|kind|kind.id()),
        "gameOverExitTicks":run.game_over_exit_ticks,"gameOverTicks":run.game_over_ticks,
        "lastCompletedRunId":run.last_completed_run_id,"loadoutReadyPlayerIds":run.loadout_ready_player_ids,
        "nextGameOverEventId":run.next_game_over_event_id,"phase":run.phase.id(),"runId":run.run_id})
}

pub fn score(score: &NativeHallOfFameRunState) -> Value {
    json!({"awesomeness":score.awesomeness,"awesomestKill":score.awesomest_kill,
        "awesomestKillMaximumHealth":score.awesomest_kill_maximum_health,
        "elapsedTicks":score.elapsed_ticks,"killStreak":score.kill_streak,
        "monstersKilled":score.monsters_killed,"portraitHeadingIndex":score.portrait_heading_index,
        "portraitScale":score.portrait_scale,"startedAtTick":score.started_at_tick})
}

pub fn hub_participant(session: &impl ParticipantRead) -> Value {
    let session = session.view();
    let wizard = &session.wizard;
    let intro = session.college_intro.map(|intro| json!({
        "contactCounter":intro.contact_counter,"coverAlpha":intro.cover_alpha,
        "dialogueSequence":intro.dialogue_sequence,"officeSpeed":intro.office_speed,
        "pathCursor":intro.path_cursor,"phase":intro.phase.as_str(),"titleCursor":intro.title_cursor}));
    let transition = wizard.transition.map(|transition| {
        json!({
        "alpha":transition.alpha,"destination":transition.destination.id(),
        "phase":match transition.phase {
            HubTransitionPhase::CollegeIntro => "college-intro",
            HubTransitionPhase::CollegeLoadout => "college-loadout",
            HubTransitionPhase::Outgoing => "outgoing", HubTransitionPhase::Incoming => "incoming"
        },"scriptedSpeed":transition.scripted_speed,
        "scriptedTarget":{"x":transition.scripted_target.x,"y":transition.scripted_target.y},
        "sourceRegion":transition.source_region.id()})
    });
    json!({"collegeIntro":intro,"region":wizard.region.id(),"transition":transition})
}
