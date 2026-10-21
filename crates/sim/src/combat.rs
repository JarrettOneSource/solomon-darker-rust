//! Player health, mana, status ordering and death, from `core-kernels/player-combat.ts`.
//!
//! Puppet hit timing comes from `native-puppet-hit.ts`; cold and dazzle movement factors
//! come from `boneyard-enemy-modifiers.ts`. `tests/combat_goldens.rs` replays the Website's
//! resource transactions, status precedence, death frame edges and respawn contract.

use crate::float::fround;

pub const PLAYER_INITIAL_HEALTH: f64 = 50.0;
pub const PLAYER_INITIAL_MANA: f64 = 100.0;
pub const PLAYER_HEALTH_RECOVERY_PER_TICK: f64 = 0.001;
pub const PLAYER_MANA_RECOVERY_PER_TICK: f64 = 0.1;
pub const PLAYER_LETHAL_HEALTH: f64 = -10.0;
pub const PLAYER_DEATH_PRESENTATION_DURATION_TICKS: u64 = 500;
const DEATH_PRESENTATION_NATIVE_TICKS: u64 = 300;
const DEATH_PRESENTATION_MAXIMUM_HELD_TICK: u64 = 298;
const DEATH_FRAME_ONE_TICK: u64 = 153;
const DEATH_FRAME_TWO_TICK: u64 = 156;
const DEATH_FRAME_THREE_TICK: u64 = 159;
const WRAITH_DAZZLE_TICKS: u64 = 50;
const HIT_TIMER_STEP: f64 = 0.05_f32 as f64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PlayerLifeState {
    Alive,
    LethalPending,
    Dying,
    Spectating,
}

/// Native Puppet +0x78/+0x7C, sampled on the owner's fixed update clock.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PuppetHitState {
    pub strength: f64,
    pub tick: u64,
    pub timer: f64,
}

impl Default for PuppetHitState {
    fn default() -> Self {
        Self { strength: 1.0, tick: 0, timer: 0.0 }
    }
}

impl PuppetHitState {
    pub fn receive(&mut self, tick: u64, strength: f64) {
        *self = Self { strength, tick, timer: 1.0 };
    }

    pub fn step(&mut self, tick: u64, elapsed_ticks: u64) {
        if self.timer == 0.0 {
            return;
        }
        self.timer = step_native_puppet_hit_timer(self.timer, elapsed_ticks);
        self.tick = tick;
    }

    #[must_use]
    pub fn timer_at(self, tick: f64) -> f64 {
        let elapsed = (tick - self.tick as f64).max(0.0);
        let mut timer = self.timer;
        for _ in 0..(elapsed.floor() as u64).min(20) {
            if timer <= 0.0 {
                break;
            }
            timer = fround(timer - HIT_TIMER_STEP).max(0.0);
        }
        (timer - (elapsed - elapsed.floor()) * HIT_TIMER_STEP).max(0.0)
    }

    #[must_use]
    pub fn alpha(self, tick: f64) -> f64 {
        fround(self.strength * self.timer_at(tick)).min(1.0)
    }
}

/// `stepNativePuppetHitTimer`: visual +0x78 and movement reaction +0x80 use the
/// same float32 subtraction, but receiving one latch does not imply the other.
#[must_use]
pub fn step_native_puppet_hit_timer(mut timer: f64, elapsed_ticks: u64) -> f64 {
    for _ in 0..elapsed_ticks.min(20) {
        if timer <= 0.0 {
            break;
        }
        timer = fround(timer - HIT_TIMER_STEP).max(0.0);
    }
    timer
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerCombatState {
    pub rescue_protection: crate::player_rescue::NativePlayerRescueProtection,
    pub hit_feedback: PuppetHitState,
    pub circle_slow_ticks_remaining: u64,
    pub cold_slow_ticks_remaining: u64,
    pub corpse_consumed: bool,
    pub current_health: f64,
    pub current_mana: f64,
    pub dazzle_ticks_remaining: u64,
    pub death_age_ticks: u64,
    pub death_epoch: u64,
    pub death_tick: u64,
    pub life_state: PlayerLifeState,
    pub last_damage_tick: Option<u64>,
    pub maximum_health: f64,
    pub maximum_mana: f64,
    pub poison_damage_per_tick: f64,
    pub poison_before_cold: bool,
    pub poison_ticks_remaining: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerCombatTickResult {
    pub began_death_epoch: bool,
    pub completed_death_presentation: bool,
    pub emitted_death_burst: bool,
}

/// Resolved progression/equipment modifiers are supplied by the owner of the stat book.
/// Combat keeps no duplicate learned-skill or inventory state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerCombatTickOptions {
    pub tick: Option<u64>,
    pub health_recovery_per_tick: f64,
    pub mana_recovery_per_tick: f64,
    pub mana_ceiling: Option<f64>,
    pub poison_damage_per_tick: Option<f64>,
}

impl Default for PlayerCombatTickOptions {
    fn default() -> Self {
        Self {
            tick: None,
            health_recovery_per_tick: PLAYER_HEALTH_RECOVERY_PER_TICK,
            mana_recovery_per_tick: PLAYER_MANA_RECOVERY_PER_TICK,
            mana_ceiling: None,
            poison_damage_per_tick: None,
        }
    }
}

impl Default for PlayerCombatState {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerCombatState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rescue_protection: crate::player_rescue::NativePlayerRescueProtection::default(),
            hit_feedback: PuppetHitState::default(),
            circle_slow_ticks_remaining: 0,
            cold_slow_ticks_remaining: 0,
            corpse_consumed: false,
            current_health: PLAYER_INITIAL_HEALTH,
            current_mana: PLAYER_INITIAL_MANA,
            dazzle_ticks_remaining: 0,
            death_age_ticks: 0,
            death_epoch: 0,
            death_tick: 0,
            life_state: PlayerLifeState::Alive,
            last_damage_tick: None,
            maximum_health: PLAYER_INITIAL_HEALTH,
            maximum_mana: PLAYER_INITIAL_MANA,
            poison_damage_per_tick: 0.0,
            poison_before_cold: false,
            poison_ticks_remaining: 0,
        }
    }

    pub fn cold_slow(&mut self, duration_ticks: u64) {
        if self.life_state != PlayerLifeState::Alive || duration_ticks == 0 {
            return;
        }
        if self.cold_slow_ticks_remaining == 0 {
            self.poison_before_cold = self.poison_ticks_remaining > 0;
        }
        self.cold_slow_ticks_remaining = self.cold_slow_ticks_remaining.max(duration_ticks);
    }

    pub fn dazzle(&mut self, duration_ticks: u64) {
        assert!(duration_ticks <= WRAITH_DAZZLE_TICKS, "dazzle exceeds its native 50 ticks");
        if self.life_state == PlayerLifeState::Alive && duration_ticks > 0 {
            self.dazzle_ticks_remaining = self.dazzle_ticks_remaining.max(duration_ticks);
        }
    }

    #[must_use]
    pub fn movement_scale(&self) -> f64 {
        let cold = if self.cold_slow_ticks_remaining > 0 { 0.5 } else { 1.0 };
        let circle = if self.circle_slow_ticks_remaining > 0 { 0.5 } else { 1.0 };
        let dazzle = if self.dazzle_ticks_remaining == 0 {
            1.0
        } else {
            ((WRAITH_DAZZLE_TICKS as f64 - self.dazzle_ticks_remaining as f64 + 1.0)
                / WRAITH_DAZZLE_TICKS as f64)
                .clamp(1.0 / WRAITH_DAZZLE_TICKS as f64, 1.0)
        };
        cold * dazzle * circle
    }

    pub fn poison(&mut self, damage_per_second: f64, duration_seconds: f64) {
        require_nonnegative(damage_per_second, "player poison damage");
        require_nonnegative(duration_seconds, "player poison duration");
        if self.life_state != PlayerLifeState::Alive || damage_per_second == 0.0 {
            return;
        }
        let damage_per_tick = fround(damage_per_second / 100.0);
        let duration_ticks = (fround(duration_seconds * 100.0).trunc() as u64).max(1);
        if self.poison_ticks_remaining == 0 {
            self.poison_before_cold = self.cold_slow_ticks_remaining == 0;
        }
        self.poison_damage_per_tick = self.poison_damage_per_tick.max(damage_per_tick);
        self.poison_ticks_remaining = self.poison_ticks_remaining.max(duration_ticks);
    }

    pub fn damage(&mut self, damage: f64, tick: u64, record_hit: bool, hit_strength: f64) {
        require_nonnegative(damage, "player damage");
        require_nonnegative(hit_strength, "player hit strength");
        if damage == 0.0
            || matches!(self.life_state, PlayerLifeState::Dying | PlayerLifeState::Spectating)
        {
            return;
        }
        self.current_health -= damage;
        if record_hit {
            self.last_damage_tick = Some(tick);
            self.hit_feedback.receive(tick, hit_strength);
        }
        if self.life_state == PlayerLifeState::LethalPending
            || self.current_health <= PLAYER_LETHAL_HEALTH
        {
            self.life_state = PlayerLifeState::LethalPending;
        }
    }

    #[must_use]
    pub fn hit_overlay_alpha(&self, tick: f64) -> f64 {
        if self.life_state == PlayerLifeState::Alive { self.hit_feedback.alpha(tick) } else { 0.0 }
    }

    pub fn restore_health(&mut self, amount: f64) {
        require_nonnegative(amount, "player health recovery");
        if amount > 0.0 && self.life_state == PlayerLifeState::Alive {
            self.current_health = (self.current_health + amount).min(self.maximum_health);
        }
    }

    pub fn restore_mana(&mut self, amount: f64) {
        require_nonnegative(amount, "player mana recovery");
        if amount > 0.0 && self.life_state == PlayerLifeState::Alive {
            self.current_mana = (self.current_mana + amount).min(self.maximum_mana);
        }
    }

    pub fn set_mana(&mut self, amount: f64) {
        require_nonnegative(amount, "player current mana");
        assert!(amount <= self.maximum_mana, "player mana exceeds its maximum");
        self.current_mana = amount;
    }

    #[must_use]
    pub fn display_health(&self) -> f64 {
        self.current_health.clamp(0.0, self.maximum_health)
    }

    pub fn try_debit_mana(&mut self, cost: f64) -> bool {
        require_nonnegative(cost, "player mana cost");
        if !self.can_cast() || cost > self.current_mana {
            return false;
        }
        self.current_mana -= cost;
        true
    }

    pub fn step(&mut self, options: PlayerCombatTickOptions) -> PlayerCombatTickResult {
        let ceiling = options.mana_ceiling.unwrap_or(self.maximum_mana);
        let poison_damage = options.poison_damage_per_tick.unwrap_or(self.poison_damage_per_tick);
        require_nonnegative(options.health_recovery_per_tick, "player health recovery rate");
        require_nonnegative(ceiling, "player mana ceiling");
        require_nonnegative(poison_damage, "player poison recovery damage");
        assert!(ceiling <= self.maximum_mana, "player mana ceiling exceeds its maximum");
        assert!(
            options.mana_recovery_per_tick.is_finite()
                && options.mana_recovery_per_tick.abs() <= 1_000_000.0,
            "invalid player mana recovery rate"
        );

        match self.life_state {
            PlayerLifeState::LethalPending => {
                self.corpse_consumed = false;
                self.death_age_ticks = 0;
                self.death_epoch += 1;
                self.death_tick = 0;
                self.circle_slow_ticks_remaining = 0;
                self.cold_slow_ticks_remaining = 0;
                self.dazzle_ticks_remaining = 0;
                self.last_damage_tick = None;
                self.hit_feedback =
                    PuppetHitState { tick: self.hit_feedback.tick, ..PuppetHitState::default() };
                self.life_state = PlayerLifeState::Dying;
                self.poison_before_cold = false;
                self.poison_damage_per_tick = 0.0;
                self.poison_ticks_remaining = 0;
                return PlayerCombatTickResult {
                    began_death_epoch: true,
                    ..PlayerCombatTickResult::default()
                };
            }
            PlayerLifeState::Spectating => return PlayerCombatTickResult::default(),
            PlayerLifeState::Dying => {
                if self.death_age_ticks >= PLAYER_DEATH_PRESENTATION_DURATION_TICKS {
                    return PlayerCombatTickResult::default();
                }
                let previous_tick = self.death_tick;
                self.death_age_ticks += 1;
                self.death_tick = player_death_presentation_tick_at_age(self.death_age_ticks);
                return PlayerCombatTickResult {
                    began_death_epoch: false,
                    completed_death_presentation: self.death_age_ticks
                        == PLAYER_DEATH_PRESENTATION_DURATION_TICKS,
                    emitted_death_burst: !self.corpse_consumed
                        && previous_tick < DEATH_FRAME_THREE_TICK
                        && self.death_tick >= DEATH_FRAME_THREE_TICK,
                };
            }
            PlayerLifeState::Alive => {}
        }

        let poisoned = self.poison_ticks_remaining > 0 && self.poison_damage_per_tick > 0.0;
        let tick = options.tick.unwrap_or(self.hit_feedback.tick + 1);
        if self.hit_feedback.tick != tick {
            self.hit_feedback.step(tick, 1);
        }
        self.circle_slow_ticks_remaining = self.circle_slow_ticks_remaining.saturating_sub(1);
        self.cold_slow_ticks_remaining = self.cold_slow_ticks_remaining.saturating_sub(1);
        self.dazzle_ticks_remaining = self.dazzle_ticks_remaining.saturating_sub(1);
        self.poison_ticks_remaining = if poisoned { self.poison_ticks_remaining - 1 } else { 0 };
        self.poison_before_cold = self.poison_ticks_remaining > 0
            && (self.cold_slow_ticks_remaining == 0 || self.poison_before_cold);
        let health_damage = if poisoned {
            player_poison_health_damage(self.current_health, poison_damage)
        } else {
            0.0
        };
        self.current_health = (self.current_health - health_damage
            + options.health_recovery_per_tick)
            .min(self.maximum_health);
        self.current_mana =
            (self.current_mana + options.mana_recovery_per_tick).clamp(0.0, ceiling);
        if self.poison_ticks_remaining == 0 {
            self.poison_damage_per_tick = 0.0;
        }
        // The accepted receiver owns the lethal suffix. In particular, blindness
        // can reject that suffix while retaining the already-applied negative HP.
        PlayerCombatTickResult::default()
    }

    pub fn set_spectating(&mut self) {
        if self.life_state == PlayerLifeState::Dying {
            self.life_state = PlayerLifeState::Spectating;
        }
    }

    pub fn respawn(&mut self) {
        if self.current_health > 0.0 {
            return;
        }
        self.rescue_protection = crate::player_rescue::NativePlayerRescueProtection::default();
        self.current_health = self.maximum_health;
        self.current_mana = self.maximum_mana;
        self.death_age_ticks = 0;
        self.death_tick = 0;
        self.life_state = PlayerLifeState::Alive;
    }

    #[must_use]
    pub fn can_cast(&self) -> bool {
        self.life_state == PlayerLifeState::Alive
    }

    #[must_use]
    pub fn can_accept_input(&self) -> bool {
        self.life_state == PlayerLifeState::Alive
    }

    #[must_use]
    pub fn collision_enabled(&self) -> bool {
        !self.corpse_consumed
            && self.life_state != PlayerLifeState::Spectating
            && !(self.life_state == PlayerLifeState::Dying
                && self.death_tick >= DEATH_FRAME_THREE_TICK)
    }

    #[must_use]
    pub fn collision_enabled_after_tick(&self) -> bool {
        let mut next = self.clone();
        next.step(PlayerCombatTickOptions::default());
        next.collision_enabled()
    }

    #[must_use]
    pub fn death_frame(&self) -> Option<usize> {
        match self.life_state {
            PlayerLifeState::Alive | PlayerLifeState::LethalPending => None,
            PlayerLifeState::Dying | PlayerLifeState::Spectating => {
                Some(player_death_frame_at_tick(self.death_tick))
            }
        }
    }

    pub fn reset_for_new_run(&mut self) {
        let maximum_health = self.maximum_health;
        let maximum_mana = self.maximum_mana;
        let corpse_consumed = self.corpse_consumed;
        *self = Self::new();
        // The Source spreads this retained flag. Only a new death epoch clears it.
        self.corpse_consumed = corpse_consumed;
        self.maximum_health = maximum_health;
        self.current_health = maximum_health;
        self.maximum_mana = maximum_mana;
        self.current_mana = maximum_mana;
    }
}

#[must_use]
pub fn player_death_frame_at_tick(tick: u64) -> usize {
    if tick < DEATH_FRAME_ONE_TICK {
        0
    } else if tick < DEATH_FRAME_TWO_TICK {
        1
    } else if tick < DEATH_FRAME_THREE_TICK {
        2
    } else {
        3
    }
}

/// The Website holds the final native pose while stretching 300 native ticks over 500 ticks.
#[must_use]
pub fn player_death_presentation_tick_at_age(age_ticks: u64) -> u64 {
    let age = age_ticks.min(PLAYER_DEATH_PRESENTATION_DURATION_TICKS);
    (age * DEATH_PRESENTATION_NATIVE_TICKS / PLAYER_DEATH_PRESENTATION_DURATION_TICKS)
        .min(DEATH_PRESENTATION_MAXIMUM_HELD_TICK)
}

fn require_nonnegative(value: f64, name: &str) {
    assert!(value.is_finite() && value >= 0.0, "{name} must be finite and nonnegative");
}

/// `playerPoisonHealthDamage`: poison may reach zero health but never makes it negative.
#[must_use]
pub fn player_poison_health_damage(current_health: f64, amount: f64) -> f64 {
    current_health.max(0.0).min(amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_begins_below_zero_and_keeps_collision_until_the_burst_frame() {
        let mut player = PlayerCombatState::new();
        player.damage(PLAYER_INITIAL_HEALTH + 9.0, 0, true, 1.0);
        assert_eq!(player.life_state, PlayerLifeState::Alive);
        player.damage(1.0, 0, true, 1.0);
        assert_eq!(player.life_state, PlayerLifeState::LethalPending);
        assert!(player.step(PlayerCombatTickOptions::default()).began_death_epoch);
        for _ in 0..264 {
            player.step(PlayerCombatTickOptions::default());
        }
        assert_eq!(player.death_tick, 158);
        assert!(player.collision_enabled());
        assert!(player.step(PlayerCombatTickOptions::default()).emitted_death_burst);
        assert_eq!(player.death_frame(), Some(3));
        assert!(!player.collision_enabled());
    }
}
