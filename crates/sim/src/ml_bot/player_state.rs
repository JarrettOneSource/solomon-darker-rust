//! Source `ml-bot-policy/player-state.ts`: vitals, cast occupancy and real belt slots.

use crate::profile::primary::{
    NativePrimarySkillProfile, native_primary_skill_profile_with_stat_book,
};
use crate::profile::{MAX_PLAYER_LEVEL, NATIVE_WELD_BUILDS};
use crate::secondary::{
    NATIVE_SECONDARY_GLOBAL_COOLDOWN_TICKS, NativeSecondaryAbilityId, NativeSecondaryPlayerState,
    native_secondary_ability_mana_cost_with_stat_book,
};
use crate::session::GameSession;

use super::numbers::{ratio, unsigned, write};
use super::spec::scales;

#[derive(Clone, Copy, Debug, Default)]
pub struct MlBotPolicySecondarySlot {
    pub affordable: bool,
    pub occupied: bool,
    pub ready: bool,
    pub skill_id: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyPlayerObservation {
    pub block_a: Vec<f32>,
    pub block_b: Vec<f32>,
    pub block_c: Vec<f32>,
    pub primary_affordable: bool,
    pub primary_range: f64,
    pub secondary_slots: [MlBotPolicySecondarySlot; 8],
}

pub fn observe_ml_bot_policy_player_state(
    session: &GameSession,
    player_id: &str,
    primary_effect_active: bool,
    secondary_effect_active: &[bool; 8],
) -> Result<MlBotPolicyPlayerObservation, String> {
    let participant = session
        .participant(player_id)
        .ok_or_else(|| format!("game simulation has no player {player_id}"))?;
    let player = &participant.player;
    let character = &participant.wizard;
    let derived = player.derived_stats();
    let default_secondary = NativeSecondaryPlayerState::default();
    let secondary = session.secondary.players.get(player_id).unwrap_or(&default_secondary);
    let cast_active = character.primary_cast.owns_facing()
        || secondary.cast_action.is_some()
        || secondary.cast_spin_ticks_remaining > 0;
    let cast_ready =
        player.combat.can_cast() && !cast_active && secondary.global_cooldown_ticks == 0.0;
    let offensive_factors = derived.offensive_factors(&player.skill_runtime);
    let primary_profile = native_primary_skill_profile_with_stat_book(
        &player.book,
        &player.stat_book,
        &offensive_factors,
    );
    // Source passes Reverie's manaCostWaived to both primary and secondary
    // cost resolution. It waives the completed cost, including equipment flats.
    let mana_cost_waived = player.progression.hagatha_runtime.reverie_active;
    let primary_mana_cost = if mana_cost_waived { 0.0 } else { primary_profile.common().mana_cost };
    // These are the two Source PRIMARY_SPELL_*_REACH constants, before Water's
    // authored widen modifier; this observer intentionally uses that base reach.
    let primary_range = match &primary_profile {
        NativePrimarySkillProfile::Air(_) | NativePrimarySkillProfile::Water(_) => 205.0,
        _ => scales::RANGE,
    };
    let primary_affordable = player.combat.current_mana >= primary_mana_cost;
    let movement_scale = player.combat.movement_scale();
    let mut block_a = vec![0.0; 32];
    write(
        &mut block_a,
        0,
        &[
            ratio(player.combat.current_health, player.combat.maximum_health),
            ratio(player.combat.current_mana, player.combat.maximum_mana),
            unsigned(f64::from(player.progression.level), f64::from(MAX_PLAYER_LEVEL)),
            session
                .boneyard
                .as_ref()
                .and_then(|world| world.waves.as_ref())
                .map_or(0.0, |waves| unsigned(waves.state.wave_ordinal as f64, scales::WAVE)),
            // Source PLAYER_CHARACTER_STEADY_SPEED is 100, independent of lane cap.
            unsigned(100.0 * derived.movement_factor * movement_scale, scales::VELOCITY),
            f64::from(character.velocity.x != 0.0 || character.velocity.y != 0.0),
            f64::from(cast_active),
            f64::from(cast_ready),
            f64::from(player.combat.poison_ticks_remaining > 0),
            f64::from(player.progression.damage_x4_ticks_remaining > 0),
            unsigned(player.combat.current_mana, scales::MANA),
            unsigned(player.combat.maximum_mana, scales::MANA),
            unsigned(player.combat.maximum_health, scales::HP),
            f64::from(player.combat.cold_slow_ticks_remaining > 0),
            f64::from(player.combat.dazzle_ticks_remaining > 0),
            movement_scale.clamp(-1.0, 1.0),
            f64::from(player.progression.mind_chug_ticks_remaining > 0),
            f64::from(secondary.held_slot.is_some()),
            f64::from(secondary.plane_orb_held),
            ratio(secondary.magic_shield_absorb, secondary.magic_shield_maximum),
            unsigned(
                secondary.stoneskin_ticks_remaining as f64 / scales::TICK_RATE,
                scales::STATUS_DURATION_SECONDS,
            ),
            unsigned(secondary.global_cooldown_ticks, NATIVE_SECONDARY_GLOBAL_COOLDOWN_TICKS),
            f64::from(
                session
                    .boneyard
                    .as_ref()
                    .and_then(|world| world.encounter.as_ref())
                    .is_some_and(|encounter| encounter.player_locked(player_id)),
            ),
            f64::from(
                player.progression.pending_offer.is_some() || session.level_up_barrier.is_some(),
            ),
        ],
    );
    let wave_phase = session
        .boneyard
        .as_ref()
        .and_then(|world| world.waves.as_ref())
        .map_or("dormant", |waves| waves.state.phase.id());
    let phase_index = [
        "dormant",
        "opening",
        "opening-threshold",
        "spawning",
        "wave-threshold",
        "wave-lull-delay",
        "wave-lull",
        "interwave",
    ]
    .iter()
    .position(|phase| *phase == wave_phase)
    .expect("the retained wave director uses a Source phase");
    block_a[24 + phase_index] = 1.0;

    let mut block_b = vec![0.0; 11];
    let primary_id = player.book.primary_skill_id;
    if primary_id == 52 {
        if let Some(weld) =
            NATIVE_WELD_BUILDS.iter().find(|weld| Some(weld.id) == player.book.weld_build_id)
        {
            for id in weld.primary_skill_ids {
                block_b[element_index(id).expect("a native Weld contains elemental primaries")] =
                    1.0;
            }
        }
    } else {
        let index = element_index(primary_id)
            .ok_or_else(|| format!("primary skill {primary_id} has no element"))?;
        block_b[index] = 1.0;
    }
    let build_index = if primary_id == 52 {
        player
            .book
            .weld_build_id
            .map_or(0.0, |id| ((f64::from(id) - 1000.0) / 10.0).clamp(-1.0, 1.0))
    } else {
        match primary_id {
            8 => 0.0,
            16 => 0.2,
            24 => 0.4,
            32 => 0.6,
            40 => 0.8,
            _ => unreachable!(),
        }
    };
    write(
        &mut block_b,
        5,
        &[
            f64::from(primary_id == 52 && player.book.weld_build_id.is_some()),
            build_index,
            unsigned(primary_mana_cost, scales::MANA),
            unsigned(primary_range, scales::RANGE),
            f64::from(primary_affordable),
            f64::from(primary_effect_active),
        ],
    );
    let mut block_c = vec![0.0; 8 * 15];
    let quickbar = player.belt.skill_projection();
    let mut secondary_slots = [MlBotPolicySecondarySlot::default(); 8];
    for (slot, skill_id) in quickbar.into_iter().enumerate() {
        let ability = skill_id.and_then(NativeSecondaryAbilityId::from_skill_id);
        let occupied = ability.is_some();
        let mana_cost = if mana_cost_waived {
            0.0
        } else {
            ability.map_or(0.0, |ability| {
                native_secondary_ability_mana_cost_with_stat_book(
                    &player.book,
                    &player.stat_book,
                    ability,
                    &offensive_factors,
                )
            })
        };
        let cooldown = skill_id
            .and_then(|id| secondary.cooldown_ticks_by_skill.get(id))
            .copied()
            .unwrap_or(0.0);
        let cooldown_maximum = skill_id
            .and_then(|id| secondary.cooldown_maximum_ticks_by_skill.get(id))
            .copied()
            .unwrap_or(0.0);
        let ready = occupied
            && cooldown == 0.0
            && secondary.global_cooldown_ticks == 0.0
            && player.combat.can_cast();
        let affordable = occupied && player.combat.current_mana >= mana_cost;
        let start = slot * 15;
        block_c[start] = f32::from(occupied);
        if let Some(id) = skill_id {
            let element = match id {
                8..=15 => Some(4),
                16..=23 => Some(0),
                24..=31 => Some(3),
                32..=39 => Some(1),
                40..=47 => Some(2),
                _ => None,
            };
            if let Some(element) = element {
                block_c[start + 1 + element] = 1.0;
            }
        }
        write(
            &mut block_c,
            start + 6,
            &[
                skill_id.filter(|id| (8..=47).contains(id)).map_or(0.0, |id| (id % 8) as f64 / 8.0),
                unsigned(mana_cost, scales::MANA),
                unsigned(cooldown_maximum / scales::TICK_RATE, scales::COOLDOWN_SECONDS),
                unsigned(cooldown / scales::TICK_RATE, scales::COOLDOWN_SECONDS),
                f64::from(ready),
                f64::from(affordable),
                f64::from(secondary_effect_active[slot]),
                f64::from(secondary.held_slot == Some(slot)),
                f64::from(skill_id.is_some_and(|id| [8, 16, 24, 32, 40, 52].contains(&id))),
            ],
        );
        secondary_slots[slot] = MlBotPolicySecondarySlot { affordable, occupied, ready, skill_id };
    }
    Ok(MlBotPolicyPlayerObservation {
        block_a,
        block_b,
        block_c,
        primary_affordable,
        primary_range,
        secondary_slots,
    })
}

fn element_index(skill_id: usize) -> Option<usize> {
    match skill_id {
        16 => Some(0),
        32 => Some(1),
        40 => Some(2),
        24 => Some(3),
        8 => Some(4),
        _ => None,
    }
}
