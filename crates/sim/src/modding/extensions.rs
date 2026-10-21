//! Borrowed authored-content hooks from `core-server/game-simulation.ts`.
//! A host supplies its prepared owner for an epoch; no VM or callback enters a
//! Session checkpoint, and filters read the actual player's equipment row.

use crate::economy::{BoastSelection, HubEquipmentState, HubInventoryItem};
use crate::enemies::EnemyToken;
use crate::hub::interaction::BoastRules;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameSimulationLootInput {
    pub actor_seed: u64,
    pub enemy_token: EnemyToken,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSimulationDamageKind {
    Magic,
    Physical,
    Poison,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameSimulationDamageFilterInput<'a> {
    pub amount: f64,
    pub damage_kind: GameSimulationDamageKind,
    pub source_actor_id: Option<usize>,
    pub target_player_id: &'a str,
    pub tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSimulationManaSource {
    EnemyContact,
    Overload,
    PassiveRecovery,
    PrimaryCast,
    SecondaryCast,
    StockOrb,
    StockPotion,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameSimulationManaFilterInput<'a> {
    pub current_mana: f64,
    pub delta: f64,
    pub maximum_mana: f64,
    pub player_id: &'a str,
    pub source: GameSimulationManaSource,
    pub tick: u64,
}

/// The source hooks execute synchronously and can throw. The caller propagates
/// those execution errors, validates mutation
/// bounds and propagates a failed epoch; it must never replace a rejected custom
/// result with a stock amount. The supplemental equipment borrow replaces the
/// source closure's read of the same live player store without aliasing Session.
pub trait GameSimulationExtensions {
    fn create_loot_items(&self, input: GameSimulationLootInput) -> Vec<HubInventoryItem>;

    fn filter_damage(
        &self,
        input: GameSimulationDamageFilterInput<'_>,
        equipment: &HubEquipmentState,
    ) -> Result<f64, String>;

    fn filter_mana(
        &self,
        input: GameSimulationManaFilterInput<'_>,
        equipment: &HubEquipmentState,
    ) -> Result<f64, String>;

    fn has_consumable(&self, content_id: &str) -> bool;

    fn resolve_boast(&self, selection: &BoastSelection) -> Option<BoastRules<'_>>;
}

/// `gameBoastResolver`: stock selectors retain their native meaning; authored
/// selections resolve only through the actual admitted prepared catalog.
#[must_use]
pub fn game_boast_rules<'a>(
    selection: &BoastSelection,
    extensions: Option<&'a dyn GameSimulationExtensions>,
) -> Option<BoastRules<'a>> {
    crate::hub::interaction::native_boast_rules_for_selection(selection)
        .or_else(|| extensions.and_then(|extensions| extensions.resolve_boast(selection)))
}

/// `player-contact-system.ts::finiteModMutation`: invalid custom output is an
/// execution error, including finite values outside the source mutation budget.
pub fn finite_mod_mutation(value: f64, field: &str) -> Result<f64, String> {
    if !value.is_finite() || value.abs() > 1_000_000.0 {
        return Err(format!("{field} must be finite and within +/-1000000"));
    }
    Ok(value)
}
