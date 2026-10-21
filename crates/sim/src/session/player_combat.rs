//! The entity resource/death phase from `core-server/player-entity-store.ts`.
//! PlayerState retains the components; the session applies returned presentation,
//! Last Word and poison response packets at their source positions in the tick.

use crate::combat::{PlayerCombatTickOptions, PlayerLifeState, player_poison_health_damage};
use crate::economy::{HubInventoryAction, project_inventory_items};
use crate::float::{fround, hypot};
use crate::harmful::{
    PlayerFlashResponse, PlayerHarmfulContactContext, PlayerIncomingDamage,
    resolve_player_flash_response, resolve_player_harmful_contact,
};
use crate::modding::extensions::{
    GameSimulationDamageFilterInput, GameSimulationDamageKind, GameSimulationExtensions,
    GameSimulationManaSource, finite_mod_mutation,
};
use crate::player::PlayerState;
use crate::profile::{
    NATIVE_HAGATHA_LAST_WORD_DEATH_TICK, NativePotionKind, PlayerSkillActivity, WizardElement,
    native_hagatha_drinker_should_use_health_potion, native_hagatha_drinker_should_use_mana_potion,
};
use crate::spells::casting::PlayerPrimaryCastState;
use crate::{HubWizard, NativeRng};

const LAST_WORD_SELECTOR: u32 = 12;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerEntityDamageResult {
    pub auto_health_potion_used: bool,
    pub cheat_death_triggered: bool,
    pub hagatha_charms_lost: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerEntityManaDebitResult {
    pub accepted: bool,
    pub auto_mana_potion_used: bool,
}

impl PlayerState {
    pub fn damage_with_result(
        &mut self,
        amount: f64,
        tick: u64,
        damage_already_scaled: bool,
        record_hit: bool,
        hit_strength: f64,
        blindness_ticks_remaining: f64,
    ) -> PlayerEntityDamageResult {
        let amount = if damage_already_scaled {
            amount
        } else {
            crate::player_rescue::native_rescue_damage(
                fround(amount * self.derived_stats().incoming_damage_factor),
                &self.combat.rescue_protection,
            )
        };
        let before = self.combat.clone();
        self.combat.damage(amount, tick, record_hit, hit_strength);
        if self.combat == before {
            return PlayerEntityDamageResult::default();
        }
        self.resolve_hagatha_damage(amount, blindness_ticks_remaining)
    }

    pub fn try_debit_mana(&mut self, cost: f64) -> PlayerEntityManaDebitResult {
        if self.combat.try_debit_mana(cost) {
            return PlayerEntityManaDebitResult { accepted: true, auto_mana_potion_used: false };
        }
        if !native_hagatha_drinker_should_use_mana_potion(
            &self.economy.owned_perk_selectors,
            self.combat.current_mana,
            self.combat.maximum_mana,
            cost,
        ) || !self.consume_drinker_potion(1)
        {
            return PlayerEntityManaDebitResult::default();
        }
        self.progression.apply_potion(NativePotionKind::Mana, &mut self.combat);
        PlayerEntityManaDebitResult {
            accepted: self.combat.try_debit_mana(cost),
            auto_mana_potion_used: true,
        }
    }

    fn resolve_hagatha_damage(
        &mut self,
        remaining_damage: f64,
        blindness_ticks_remaining: f64,
    ) -> PlayerEntityDamageResult {
        let before = self.progression.hagatha_runtime;
        self.progression.hagatha_runtime.hurt(remaining_damage);
        let mut result = PlayerEntityDamageResult {
            hagatha_charms_lost: self.progression.hagatha_runtime != before,
            ..PlayerEntityDamageResult::default()
        };
        if self.combat.life_state == PlayerLifeState::LethalPending
            && native_hagatha_drinker_should_use_health_potion(
                &self.economy.owned_perk_selectors,
                self.combat.current_health,
            )
            && self.consume_drinker_potion(0)
        {
            self.progression.apply_potion(NativePotionKind::Health, &mut self.combat);
            self.combat.death_age_ticks = 0;
            self.combat.death_tick = 0;
            self.combat.life_state = PlayerLifeState::Alive;
            result.auto_health_potion_used = true;
        }
        if self.combat.life_state == PlayerLifeState::LethalPending
            && blindness_ticks_remaining > 0.0
        {
            self.combat.life_state = PlayerLifeState::Alive;
        }
        if self.combat.life_state == PlayerLifeState::LethalPending
            && let Some(health) =
                self.progression.hagatha_runtime.consume_cheat_death(self.combat.maximum_health)
        {
            self.combat.current_health = health;
            self.combat.death_age_ticks = 0;
            self.combat.death_tick = 0;
            self.combat.rescue_protection.fraction = 1.0;
            self.combat.life_state = PlayerLifeState::Alive;
            result.cheat_death_triggered = true;
        }
        result
    }

    fn consume_drinker_potion(&mut self, subtype: u32) -> bool {
        let potion_id = project_inventory_items(&self.economy.backpack)
            .into_iter()
            .find(|entry| {
                entry.item.native_type_id == 7001
                    && entry.item.native_subtype == Some(subtype)
                    && entry.item.mod_content.is_none()
            })
            .map(|entry| entry.item.id);
        let Some(item_id) = potion_id else {
            return false;
        };
        // Drinker calls the economy command directly. Manual apply_inventory owns
        // its separate PotionUse boast producer and must not run for this path.
        let context = self.economy_context();
        self.economy.apply(&HubInventoryAction::Consume { item_id }, &context).accepted
    }
}

pub struct PlayerCombatContext<'a> {
    pub player_id: &'a str,
    pub player: &'a mut PlayerState,
    pub wizard: &'a HubWizard,
    pub primary_cast: &'a PlayerPrimaryCastState,
    pub acting_staff: bool,
    pub blindness_ticks_remaining: f64,
    pub tick: u64,
    pub mana_ceiling: Option<f64>,
    pub rng: &'a mut NativeRng,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerPoisonResponse {
    Deflect { pitch: f64 },
    Flash { response: PlayerFlashResponse },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerCombatResult {
    pub auto_health_potion_used: bool,
    pub began_death_epoch: bool,
    pub cheat_death_triggered: bool,
    pub hagatha_charms_lost: bool,
    pub completed_death_presentation: bool,
    pub emitted_death_burst: bool,
    pub last_word_archive: bool,
    pub last_word_burst: bool,
    pub poison_responses: Vec<PlayerPoisonResponse>,
}

pub fn step_player_combat(context: PlayerCombatContext<'_>) -> PlayerCombatResult {
    step_player_combat_with_extensions(context, None)
        .expect("stock resource clocks have finite native mutations")
}

pub fn step_player_combat_with_extensions(
    context: PlayerCombatContext<'_>,
    extensions: Option<&dyn GameSimulationExtensions>,
) -> Result<PlayerCombatResult, String> {
    let player = context.player;
    let before_death_tick = player.combat.death_tick;
    let derived = player.derived_stats();
    let channel = if context.primary_cast.channel_active {
        match player.book.primary_skill_id {
            8 => Some(WizardElement::Ether),
            16 => Some(WizardElement::Fire),
            24 => Some(WizardElement::Air),
            32 => Some(WizardElement::Water),
            40 => Some(WizardElement::Earth),
            _ => None,
        }
    } else {
        None
    };
    let recovery = player.skill_runtime.step(
        &derived,
        PlayerSkillActivity {
            acting: context.primary_cast.action_tick.is_some() || context.acting_staff,
            moving: hypot(context.wizard.velocity.x, context.wizard.velocity.y) > 0.01,
            primary_channel: channel,
            primary_active: context.primary_cast.held,
            primary_underpowered: context.primary_cast.underpowered,
        },
    );
    player.progression.step_potion_effects();
    let base_mana_recovery = super::extensions::filter_mana_delta(
        player,
        context.player_id,
        recovery.base_mana_recovery_per_tick,
        GameSimulationManaSource::PassiveRecovery,
        context.tick,
        extensions,
        "filtered mana delta",
    )?;
    let native_poison_damage = crate::player_rescue::native_rescue_damage(
        fround(
            player.combat.poison_damage_per_tick
                * derived.poison_damage_factor
                * derived.incoming_damage_factor,
        ),
        &player.combat.rescue_protection,
    );
    let poison_active = player.combat.life_state == PlayerLifeState::Alive
        && player.combat.poison_ticks_remaining > 0;
    let contact = if poison_active {
        Some(resolve_player_harmful_contact(
            PlayerHarmfulContactContext {
                runtime: &player.skill_runtime,
                derived: &derived,
                progression: &player.progression,
                rescue_protection: &player.combat.rescue_protection,
                reflection_source_in_range: false,
                position: context.wizard.position,
                // This Source resource branch supplies zero physical damage and
                // uses the helper's default quality; it can never birth a shard.
                enhanced_effects: true,
            },
            PlayerIncomingDamage { physical_damage: 0.0, magic_damage: 0.0 },
            context.rng,
        ))
    } else {
        None
    };
    let mut result = PlayerCombatResult::default();
    let deflected = contact.as_ref().and_then(|contact| contact.deflect_pitch);
    if let Some(pitch) = deflected {
        result.poison_responses.push(PlayerPoisonResponse::Deflect { pitch });
    }
    let poison_damage = if deflected.is_some() {
        0.0
    } else if let Some(extensions) = extensions {
        // The source calls this filter even when no poison clock is active. Health
        // damage still requires poisonActive below; a callback can observe this tick.
        finite_mod_mutation(
            extensions.filter_damage(
                GameSimulationDamageFilterInput {
                    amount: native_poison_damage,
                    damage_kind: GameSimulationDamageKind::Poison,
                    source_actor_id: None,
                    target_player_id: context.player_id,
                    tick: context.tick,
                },
                &player.economy.equipment,
            )?,
            "filtered poison damage",
        )?
        .max(0.0)
    } else {
        native_poison_damage
    };
    let poison_health_damage = if poison_active {
        player_poison_health_damage(player.combat.current_health, poison_damage)
    } else {
        0.0
    };
    let combat_tick = player.combat.step(PlayerCombatTickOptions {
        tick: Some(context.tick),
        health_recovery_per_tick: derived.health_recovery_per_tick,
        mana_recovery_per_tick: base_mana_recovery,
        mana_ceiling: context.mana_ceiling,
        poison_damage_per_tick: Some(poison_damage),
    });
    let meditation_recovery = if recovery.meditation_mana_recovery_per_tick == 0.0 {
        0.0
    } else {
        super::extensions::filter_mana_delta(
            player,
            context.player_id,
            recovery.meditation_mana_recovery_per_tick,
            GameSimulationManaSource::PassiveRecovery,
            context.tick,
            extensions,
            "filtered mana delta",
        )?
    };
    if meditation_recovery != 0.0 && player.combat.life_state == PlayerLifeState::Alive {
        let mana = (player.combat.current_mana + meditation_recovery)
            .clamp(0.0, player.combat.maximum_mana);
        player.combat.set_mana(mana);
    }
    let resolved = if poison_health_damage > 0.0 {
        player.resolve_hagatha_damage(poison_health_damage, context.blindness_ticks_remaining)
    } else {
        PlayerEntityDamageResult::default()
    };
    if poison_active
        && deflected.is_none()
        && poison_health_damage == 0.0
        && !(native_poison_damage > 0.0 && poison_damage <= 0.0)
        && player.combat.life_state == PlayerLifeState::Alive
        && let Some(response) = resolve_player_flash_response(&derived, context.rng)
    {
        result.poison_responses.push(PlayerPoisonResponse::Flash { response });
    }
    result.auto_health_potion_used = resolved.auto_health_potion_used;
    result.cheat_death_triggered = resolved.cheat_death_triggered;
    result.hagatha_charms_lost = resolved.hagatha_charms_lost;
    result.began_death_epoch =
        combat_tick.began_death_epoch && player.combat.life_state == PlayerLifeState::Dying;
    result.completed_death_presentation = combat_tick.completed_death_presentation
        && player.combat.life_state == PlayerLifeState::Dying;
    result.emitted_death_burst = combat_tick.emitted_death_burst;
    if player.economy.owned_perk_selectors.contains(&LAST_WORD_SELECTOR) {
        result.last_word_burst = !player.combat.corpse_consumed
            && before_death_tick < NATIVE_HAGATHA_LAST_WORD_DEATH_TICK
            && player.combat.death_tick >= NATIVE_HAGATHA_LAST_WORD_DEATH_TICK;
        result.last_word_archive = combat_tick.completed_death_presentation;
    }
    Ok(result)
}
