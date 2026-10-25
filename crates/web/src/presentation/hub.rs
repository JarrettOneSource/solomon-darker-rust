//! Hub/player field projection from `client/hub-presentation-timeline.ts` and
//! `client/boneyard-presentation-timeline.ts::interpolatePlayer`.

use solomon_sim::combat::PlayerLifeState;
use solomon_sim::hub::college_intro::NativeCollegeIntroPhase;
use solomon_sim::hub::regions::{
    HUB_COLLEGE_INTRO_FADE_RATE, HUB_OUTGOING_FADE_RATE, HubTransitionPhase, hub_incoming_fade_rate,
};
use solomon_sim::profile::interpolate_native_harden_coating;
use solomon_sim::session::{GameSession, SessionParticipant};

// Source native-college-intro.ts exports these values; its sim tick stores the same literals.
const NATIVE_COLLEGE_COVER_FADE_RATE: f64 = 0.0005_f32 as f64;
const NATIVE_COLLEGE_TITLE_CURSOR_STEP: f64 = 0.005200000014156103;

use super::PresentationClocks;
use super::math::{interpolate_members, lerp, lerp_cycle, lerp_vec};

pub(super) fn project_players(
    output: &mut GameSession,
    older: &GameSession,
    newer: &GameSession,
    blend: f64,
    clocks: &mut PresentationClocks,
) {
    // Source player Records retain older entries through this bracket. Hub's
    // separate participant Record retires them at its endpoint, so they are no
    // longer visible there; Boneyard has no corresponding participant Record.
    output.participant_order.clear();
    for id in &older.participant_order {
        let first = &older.participants[id];
        let second = newer.participants.get(id);
        if older.boneyard.is_none() && blend >= 1.0 && second.is_none() {
            continue;
        }
        // The selected endpoint already supplied each resident's discrete fields.
        // Only Boneyard retirement can retain a player absent from the newer copy.
        let participant = output.participants.entry(id.clone()).or_insert_with(|| first.clone());
        if let Some(second) = second {
            project_player(participant, first, second, blend, older.boneyard.is_some(), clocks);
        }
        output.participant_order.push(id.clone());
    }
    if blend >= 1.0 {
        for id in &newer.participant_order {
            if !older.participants.contains_key(id) {
                output.participant_order.push(id.clone());
            }
        }
    }
}

fn project_player(
    output: &mut SessionParticipant,
    first: &SessionParticipant,
    second: &SessionParticipant,
    blend: f64,
    boneyard: bool,
    clocks: &mut PresentationClocks,
) {
    let previous = &first.wizard;
    let next = &second.wizard;
    let wizard = &mut output.wizard;
    wizard.position = lerp_vec(previous.position, next.position, blend);
    wizard.velocity = lerp_vec(previous.velocity, next.velocity, blend);
    wizard.gait_degrees = lerp_cycle(previous.gait_degrees, next.gait_degrees, blend, 360.0);
    wizard.set_facing(lerp_cycle(previous.heading_degrees, next.heading_degrees, blend, 360.0));
    wizard.walk_cycle_primary =
        lerp_cycle(previous.walk_cycle_primary, next.walk_cycle_primary, blend, 5.0);
    wizard.primary_cast.weapon_pulse =
        lerp(previous.primary_cast.weapon_pulse, next.primary_cast.weapon_pulse, blend);
    wizard.primary_cast.ether_blast_charge =
        lerp(previous.primary_cast.ether_blast_charge, next.primary_cast.ether_blast_charge, blend);
    output.lighting.overlay_effect_phase =
        lerp(first.lighting.overlay_effect_phase, second.lighting.overlay_effect_phase, blend);
    output.player.skill_runtime.harden.coating = interpolate_native_harden_coating(
        first.player.skill_runtime.harden.coating,
        second.player.skill_runtime.harden.coating,
        previous.primary_cast.cast_sequence == next.primary_cast.cast_sequence,
        blend,
    );
    let first_damage = first.player.progression.damage_x4_ticks_remaining;
    let second_damage = second.player.progression.damage_x4_ticks_remaining;
    let damage = if first_damage > 0
        && second_damage > 0
        && first_damage >= second_damage
        && first_damage - second_damage <= 10
    {
        lerp(first_damage as f64, second_damage as f64, blend)
    } else {
        output.player.progression.damage_x4_ticks_remaining as f64
    };
    clocks.player_damage_x4.insert(first.id.clone(), damage);
    // Canonical clocks remain integers. The renderer uses the exact fractional
    // value above; this private display copy cannot become a checkpoint.
    output.player.progression.damage_x4_ticks_remaining = damage.floor() as u64;
    if boneyard {
        let first_combat = &first.player.combat;
        let second_combat = &second.player.combat;
        let death_presentation =
            |state| matches!(state, PlayerLifeState::Dying | PlayerLifeState::Spectating);
        if first_combat.death_epoch == second_combat.death_epoch
            && death_presentation(first_combat.life_state)
            && death_presentation(second_combat.life_state)
        {
            output.player.combat.death_tick = lerp(
                first_combat.death_tick as f64,
                second_combat.death_tick as f64,
                blend,
            )
            .floor() as u64;
        }
    } else {
        project_participant(output, first, second, blend);
    }
}

fn project_participant(
    output: &mut SessionParticipant,
    first: &SessionParticipant,
    second: &SessionParticipant,
    blend: f64,
) {
    if let (Some(previous), Some(next)) = (first.wizard.transition, second.wizard.transition)
        && previous.phase == next.phase
        && previous.destination == next.destination
        && previous.source_region == next.source_region
    {
        let mut transition = next;
        transition.alpha = lerp(previous.alpha, next.alpha, blend);
        transition.scripted_speed = lerp(previous.scripted_speed, next.scripted_speed, blend);
        transition.scripted_target =
            lerp_vec(previous.scripted_target, next.scripted_target, blend);
        output.wizard.transition = Some(transition);
        if let (Some(previous), Some(next)) = (&first.college_intro, &second.college_intro)
            && previous.phase == next.phase
        {
            let mut intro = *next;
            if blend < 1.0 {
                intro.contact_counter = previous.contact_counter;
                intro.dialogue_sequence = previous.dialogue_sequence;
            }
            intro.cover_alpha = lerp(previous.cover_alpha, next.cover_alpha, blend);
            intro.office_speed = lerp(previous.office_speed, next.office_speed, blend);
            intro.path_cursor = lerp(previous.path_cursor, next.path_cursor, blend);
            intro.title_cursor = lerp(previous.title_cursor, next.title_cursor, blend);
            output.college_intro = Some(intro);
        }
    }
}

pub(super) fn project_world(
    output: &mut GameSession,
    older: &GameSession,
    newer: &GameSession,
    blend: f64,
) {
    let first = &older.ambient;
    let next = &newer.ambient;
    output.ambient.seal_core_phase =
        lerp_cycle(first.seal_core_phase, next.seal_core_phase, blend, 3.0);
    output.ambient.seal_glyph_phase =
        lerp_cycle(first.seal_glyph_phase, next.seal_glyph_phase, blend, 3.0);
    output.ambient.statue_phase_degrees =
        lerp_cycle(first.statue_phase_degrees, next.statue_phase_degrees, blend, 360.0);
    output.ambient.fountain_particles = interpolate_members(
        &first.fountain_particles,
        &next.fountain_particles,
        blend,
        |particle| particle.id,
        |first, next, blend| {
            let mut particle = if blend < 1.0 { *first } else { *next };
            particle.remaining = lerp(first.remaining, next.remaining, blend);
            particle.scale = lerp(first.scale, next.scale, blend);
            particle
        },
    );
    output.npcs.students.students = interpolate_members(
        &older.npcs.students.students,
        &newer.npcs.students.students,
        blend,
        |student| student.id,
        |first, next, blend| {
            let mut student = if blend < 1.0 { first.clone() } else { next.clone() };
            student.frame_phase = lerp_cycle(first.frame_phase, next.frame_phase, blend, 5.0);
            student.gait_degrees = lerp_cycle(first.gait_degrees, next.gait_degrees, blend, 360.0);
            student.heading = lerp_cycle(first.heading, next.heading, blend, 360.0);
            student.heading_index =
                lerp_cycle(first.heading_index as f64, next.heading_index as f64, blend, 24.0)
                    .round() as usize
                    % 24;
            student.position = lerp_vec(first.position, next.position, blend);
            student
        },
    );
}

/// Hub alone replaces its local timeline player with the newest endpoint. Its
/// local transition cursors may advance at most one network interval further.
pub(super) fn project_local(
    output: &mut GameSession,
    newest: &GameSession,
    local_id: &str,
    elapsed_ticks: f64,
    clocks: &mut PresentationClocks,
) {
    let Some(source) = newest.participants.get(local_id) else { return };
    let mut participant = source.clone();
    if let Some(intro) = &mut participant.college_intro
        && intro.phase == NativeCollegeIntroPhase::CourtyardWalk
    {
        intro.cover_alpha =
            (intro.cover_alpha - NATIVE_COLLEGE_COVER_FADE_RATE * elapsed_ticks).clamp(0.0, 1.0);
        intro.title_cursor =
            (intro.title_cursor + NATIVE_COLLEGE_TITLE_CURSOR_STEP * elapsed_ticks).clamp(0.0, 5.0);
    }
    if let Some(transition) = &mut participant.wizard.transition {
        let rate = match transition.phase {
            HubTransitionPhase::CollegeLoadout => 0.0,
            HubTransitionPhase::CollegeIntro => -HUB_COLLEGE_INTRO_FADE_RATE,
            HubTransitionPhase::Outgoing => HUB_OUTGOING_FADE_RATE,
            HubTransitionPhase::Incoming => -hub_incoming_fade_rate(participant.wizard.region),
        };
        transition.alpha = (transition.alpha + rate * elapsed_ticks).clamp(0.0, 1.0);
    }
    clocks
        .player_damage_x4
        .insert(local_id.to_owned(), source.player.progression.damage_x4_ticks_remaining as f64);
    output.participants.insert(local_id.to_owned(), participant);
}
