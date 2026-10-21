//! Authored mutations from `game-simulation.ts::applyFilteredManaDelta` and the
//! entity-level boast resolver. Prepared VMs stay outside Session/checkpoints.

use crate::economy::HubEquipmentState;
use crate::hub::interaction::BoastFailureProducer;
pub(super) use crate::modding::extensions::game_boast_rules as boast_rules;
use crate::modding::extensions::{
    GameSimulationExtensions, GameSimulationManaFilterInput, GameSimulationManaSource,
    finite_mod_mutation,
};
use crate::player::PlayerState;

pub(super) fn fail_player_boast(
    player: &mut PlayerState,
    producer: BoastFailureProducer,
    extensions: Option<&dyn GameSimulationExtensions>,
) {
    player.fail_boast_with_extensions(producer, extensions);
}

pub(super) fn filter_mana_delta(
    player: &PlayerState,
    player_id: &str,
    delta: f64,
    source: GameSimulationManaSource,
    tick: u64,
    extensions: Option<&dyn GameSimulationExtensions>,
    field: &str,
) -> Result<f64, String> {
    filter_mana_input(
        GameSimulationManaFilterInput {
            current_mana: player.combat.current_mana,
            delta,
            maximum_mana: player.combat.maximum_mana,
            player_id,
            source,
            tick,
        },
        &player.economy.equipment,
        extensions,
        field,
    )
}

pub(super) fn filter_mana_input(
    input: GameSimulationManaFilterInput<'_>,
    equipment: &HubEquipmentState,
    extensions: Option<&dyn GameSimulationExtensions>,
    field: &str,
) -> Result<f64, String> {
    let Some(extensions) = extensions else { return Ok(input.delta) };
    finite_mod_mutation(extensions.filter_mana(input, equipment)?, field)
}

pub(super) fn apply_filtered_mana_delta(
    player: &mut PlayerState,
    player_id: &str,
    requested_delta: f64,
    source: GameSimulationManaSource,
    tick: u64,
    extensions: Option<&dyn GameSimulationExtensions>,
    divergence_message: Option<&str>,
) -> Result<(), String> {
    let delta = filter_mana_delta(
        player,
        player_id,
        requested_delta,
        source,
        tick,
        extensions,
        "filtered mana delta",
    )?;
    if delta < 0.0 {
        if -delta > player.combat.current_mana {
            fail_player_boast(player, BoastFailureProducer::ManaUnderflow, extensions);
        }
        let accepted = player.try_debit_mana(-delta).accepted;
        if !accepted
            && extensions.is_none()
            && let Some(message) = divergence_message
        {
            return Err(message.into());
        }
    } else if delta > 0.0 {
        player.combat.restore_mana(delta);
    }
    Ok(())
}
