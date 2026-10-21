//! Ordered secondary target modifiers from native-secondary-abilities.ts.
//! Movement factors are composed in application order, with a float32 store at each link.

use crate::float::fround;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeSecondaryMovementModifierKind {
    ColdSlow,
    CircleSlow,
    Frozen,
    Stun,
    Dazzle,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondaryElectricBurnEffectState {
    pub arc_count: usize,
    pub damage_per_tick: f64,
    pub owner_id: String,
    pub source_actor_id: u64,
    pub stun_factor: f64,
    pub ticks: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondarySteamedEffectState {
    pub damage_per_tick: f64,
    pub ember_damage: f64,
    pub ember_fragments: usize,
    pub explode_damage: f64,
    pub explode_radius: f64,
    pub owner_id: String,
    pub source_actor_id: u64,
    pub ticks: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondaryTargetEffectState {
    pub circle_slow_factor: f64,
    pub circle_slow_ticks: u64,
    pub cold_slow_factor: f64,
    pub cold_slow_material: bool,
    pub cold_slow_ticks: u64,
    pub dazzle_maximum_ticks: u64,
    pub dazzle_ticks: u64,
    pub disrupted_ticks: u64,
    pub electric_burn: Option<NativeSecondaryElectricBurnEffectState>,
    pub flee_ticks: u64,
    pub frost_burn_damage_per_tick: f64,
    pub frost_burn_owner_id: Option<String>,
    pub frost_burn_skill_id: Option<usize>,
    pub frost_burn_source_actor_id: Option<u64>,
    pub frost_burn_ticks: u64,
    pub frozen_ticks: u64,
    pub frozen_time_scale: f64,
    pub movement_modifier_order: Vec<NativeSecondaryMovementModifierKind>,
    pub prismatic_ticks: u64,
    pub stun_factor: f64,
    pub stun_ticks: u64,
    pub steamed: Option<NativeSecondarySteamedEffectState>,
    pub target_id: usize,
    pub time_scale: f64,
    pub weaken_factor: f64,
    pub world_key: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeSecondaryTargetEffectPatch {
    pub circle_slow_factor: Option<f64>,
    pub circle_slow_ticks: Option<u64>,
    pub cold_slow_factor: Option<f64>,
    pub cold_slow_material: Option<bool>,
    pub cold_slow_ticks: Option<u64>,
    pub dazzle_ticks: Option<u64>,
    pub disrupted_ticks: Option<u64>,
    /// None leaves the modifier unchanged; Some(None) explicitly removes it.
    pub electric_burn: Option<Option<NativeSecondaryElectricBurnEffectState>>,
    pub flee_ticks: Option<u64>,
    pub frost_burn_damage_per_tick: Option<f64>,
    pub frost_burn_owner_id: Option<String>,
    pub frost_burn_skill_id: Option<usize>,
    pub frost_burn_source_actor_id: Option<u64>,
    pub frost_burn_ticks: Option<u64>,
    pub frozen_ticks: Option<u64>,
    pub frozen_time_scale: Option<f64>,
    pub prismatic_ticks: Option<u64>,
    pub stun_factor: Option<f64>,
    pub stun_ticks: Option<u64>,
    pub steamed: Option<Option<NativeSecondarySteamedEffectState>>,
    pub weaken_factor: Option<f64>,
}

impl NativeSecondaryTargetEffectState {
    #[must_use]
    pub fn new(world_key: String, target_id: usize) -> Self {
        Self {
            circle_slow_factor: 1.0,
            circle_slow_ticks: 0,
            cold_slow_factor: 1.0,
            cold_slow_material: false,
            cold_slow_ticks: 0,
            dazzle_maximum_ticks: 0,
            dazzle_ticks: 0,
            disrupted_ticks: 0,
            electric_burn: None,
            flee_ticks: 0,
            frost_burn_damage_per_tick: 0.0,
            frost_burn_owner_id: None,
            frost_burn_skill_id: None,
            frost_burn_source_actor_id: None,
            frost_burn_ticks: 0,
            frozen_ticks: 0,
            frozen_time_scale: 1.0,
            movement_modifier_order: Vec::new(),
            prismatic_ticks: 0,
            stun_factor: 1.0,
            stun_ticks: 0,
            steamed: None,
            target_id,
            time_scale: 1.0,
            weaken_factor: 1.0,
            world_key,
        }
    }

    pub fn apply(&mut self, patch: NativeSecondaryTargetEffectPatch) {
        if let Some(factor) = patch.stun_factor {
            assert!(
                factor.is_finite() && (0.0..=1.0).contains(&factor),
                "Stun factor is inside [0,1]"
            );
        }
        use NativeSecondaryMovementModifierKind::*;
        for (kind, current_ticks, incoming_ticks) in [
            (ColdSlow, self.cold_slow_ticks, patch.cold_slow_ticks),
            (CircleSlow, self.circle_slow_ticks, patch.circle_slow_ticks),
            (Frozen, self.frozen_ticks, patch.frozen_ticks),
            (Stun, self.stun_ticks, patch.stun_ticks),
            (Dazzle, self.dazzle_ticks, patch.dazzle_ticks),
        ] {
            if incoming_ticks.unwrap_or(0) > 0
                && current_ticks == 0
                && !self.movement_modifier_order.contains(&kind)
            {
                self.movement_modifier_order.push(kind);
            }
        }
        if let Some(ticks) = patch.cold_slow_ticks {
            self.cold_slow_factor =
                self.cold_slow_factor.min(patch.cold_slow_factor.unwrap_or(1.0));
            if self.cold_slow_ticks < ticks {
                self.cold_slow_material =
                    patch.cold_slow_material.unwrap_or(self.cold_slow_material);
            } else if self.cold_slow_ticks == ticks {
                self.cold_slow_material |= patch.cold_slow_material.unwrap_or(false);
            }
            self.cold_slow_ticks = self.cold_slow_ticks.max(ticks);
        }
        if let Some(ticks) = patch.circle_slow_ticks {
            self.circle_slow_factor =
                self.circle_slow_factor.min(patch.circle_slow_factor.unwrap_or(1.0));
            self.circle_slow_ticks = self.circle_slow_ticks.max(ticks);
        }
        if let Some(ticks) = patch.frozen_ticks {
            self.frozen_time_scale =
                self.frozen_time_scale.min(patch.frozen_time_scale.unwrap_or(0.0));
            self.frozen_ticks = self.frozen_ticks.max(ticks);
        }
        if let Some(ticks) = patch.stun_ticks {
            self.stun_factor = self.stun_factor.min(patch.stun_factor.unwrap_or(1.0));
            self.stun_ticks = self.stun_ticks.max(ticks);
        }
        self.dazzle_maximum_ticks = self.dazzle_maximum_ticks.max(patch.dazzle_ticks.unwrap_or(0));
        self.dazzle_ticks = self.dazzle_ticks.max(patch.dazzle_ticks.unwrap_or(0));
        self.disrupted_ticks = self.disrupted_ticks.max(patch.disrupted_ticks.unwrap_or(0));
        self.flee_ticks = self.flee_ticks.max(patch.flee_ticks.unwrap_or(0));
        self.prismatic_ticks = self.prismatic_ticks.max(patch.prismatic_ticks.unwrap_or(0));
        let frost_burn_ticks = self.frost_burn_ticks.max(patch.frost_burn_ticks.unwrap_or(0));
        if frost_burn_ticks > self.frost_burn_ticks {
            self.frost_burn_damage_per_tick =
                patch.frost_burn_damage_per_tick.unwrap_or(self.frost_burn_damage_per_tick);
            self.frost_burn_owner_id =
                patch.frost_burn_owner_id.or_else(|| self.frost_burn_owner_id.clone());
            self.frost_burn_skill_id = patch.frost_burn_skill_id.or(self.frost_burn_skill_id);
            self.frost_burn_source_actor_id =
                patch.frost_burn_source_actor_id.or(self.frost_burn_source_actor_id);
        } else {
            self.frost_burn_damage_per_tick = self
                .frost_burn_damage_per_tick
                .max(patch.frost_burn_damage_per_tick.unwrap_or(0.0));
            if self.frost_burn_owner_id.is_none() {
                self.frost_burn_owner_id = patch.frost_burn_owner_id;
            }
            self.frost_burn_skill_id = self.frost_burn_skill_id.or(patch.frost_burn_skill_id);
            self.frost_burn_source_actor_id =
                self.frost_burn_source_actor_id.or(patch.frost_burn_source_actor_id);
        }
        self.frost_burn_ticks = frost_burn_ticks;
        if let Some(incoming) = patch.electric_burn {
            merge_electric_burn(&mut self.electric_burn, incoming);
        }
        if let Some(incoming) = patch.steamed {
            merge_steamed(&mut self.steamed, incoming);
        }
        if self.weaken_factor >= 1.0 {
            self.weaken_factor = patch.weaken_factor.unwrap_or(self.weaken_factor);
        }
        self.compose_time_scale();
    }

    /// The world processes damage pulses from the preceding snapshot before dropping their
    /// final tick. This method only advances the retained status snapshot for the new tick.
    pub fn step(&mut self) {
        if self.dazzle_ticks <= 1 {
            self.dazzle_maximum_ticks = 0;
        }
        self.circle_slow_ticks = self.circle_slow_ticks.saturating_sub(1);
        self.cold_slow_ticks = self.cold_slow_ticks.saturating_sub(1);
        self.dazzle_ticks = self.dazzle_ticks.saturating_sub(1);
        self.disrupted_ticks = self.disrupted_ticks.saturating_sub(1);
        self.flee_ticks = self.flee_ticks.saturating_sub(1);
        self.frost_burn_ticks = self.frost_burn_ticks.saturating_sub(1);
        self.frozen_ticks = self.frozen_ticks.saturating_sub(1);
        self.prismatic_ticks = self.prismatic_ticks.saturating_sub(1);
        self.stun_ticks = self.stun_ticks.saturating_sub(1);
        if let Some(burn) = &mut self.electric_burn {
            burn.ticks = burn.ticks.saturating_sub(1);
            if burn.ticks == 0 {
                self.electric_burn = None;
            }
        }
        if let Some(steamed) = &mut self.steamed {
            steamed.ticks = steamed.ticks.saturating_sub(1);
            if steamed.ticks == 0 {
                self.steamed = None;
            }
        }
        if self.frozen_ticks == 0 {
            self.frozen_time_scale = 1.0;
        } else if self.frozen_ticks <= 200 {
            self.frozen_time_scale = fround((self.frozen_time_scale + (0.005_f32 as f64)).min(1.0));
        }
        if self.circle_slow_ticks == 0 {
            self.circle_slow_factor = 1.0;
        }
        if self.cold_slow_ticks == 0 {
            self.cold_slow_factor = 1.0;
            self.cold_slow_material = false;
        }
        if self.stun_ticks == 0 {
            self.stun_factor = 1.0;
        }
        if self.frost_burn_ticks == 0 {
            self.frost_burn_damage_per_tick = 0.0;
            self.frost_burn_owner_id = None;
            self.frost_burn_skill_id = None;
            self.frost_burn_source_actor_id = None;
        }
        let mut order = Vec::new();
        for kind in &self.movement_modifier_order {
            if self.modifier_ticks(*kind) > 0 {
                order.push(*kind);
            }
        }
        self.movement_modifier_order = order;
        self.compose_time_scale();
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.circle_slow_ticks > 0
            || self.cold_slow_ticks > 0
            || self.dazzle_ticks > 0
            || self.disrupted_ticks > 0
            || self.electric_burn.is_some()
            || self.flee_ticks > 0
            || self.frost_burn_ticks > 0
            || self.frozen_ticks > 0
            || self.steamed.is_some()
            || self.prismatic_ticks > 0
            || self.stun_ticks > 0
            || self.weaken_factor < 1.0
    }

    fn modifier_ticks(&self, kind: NativeSecondaryMovementModifierKind) -> u64 {
        match kind {
            NativeSecondaryMovementModifierKind::ColdSlow => self.cold_slow_ticks,
            NativeSecondaryMovementModifierKind::CircleSlow => self.circle_slow_ticks,
            NativeSecondaryMovementModifierKind::Frozen => self.frozen_ticks,
            NativeSecondaryMovementModifierKind::Stun => self.stun_ticks,
            NativeSecondaryMovementModifierKind::Dazzle => self.dazzle_ticks,
        }
    }

    fn compose_time_scale(&mut self) {
        let mut scale = fround(1.0);
        for kind in &self.movement_modifier_order {
            use NativeSecondaryMovementModifierKind::*;
            let factor = match kind {
                ColdSlow => {
                    if self.cold_slow_ticks > 0 {
                        self.cold_slow_factor
                    } else {
                        1.0
                    }
                }
                CircleSlow => {
                    if self.circle_slow_ticks > 0 {
                        self.circle_slow_factor
                    } else {
                        1.0
                    }
                }
                Frozen => {
                    if self.frozen_ticks > 0 {
                        self.frozen_time_scale
                    } else {
                        1.0
                    }
                }
                Stun => {
                    if self.stun_ticks > 0 {
                        self.stun_factor
                    } else {
                        1.0
                    }
                }
                Dazzle => {
                    if self.dazzle_ticks == 0 || self.dazzle_maximum_ticks == 0 {
                        1.0
                    } else {
                        (1.0 / self.dazzle_maximum_ticks as f64)
                            .max(1.0 - self.dazzle_ticks as f64 / self.dazzle_maximum_ticks as f64)
                    }
                }
            };
            scale = fround(scale * factor);
        }
        self.time_scale = scale;
    }
}

pub fn apply_native_secondary_target_effect(
    effects: &mut Vec<NativeSecondaryTargetEffectState>,
    world_key: &str,
    target_id: usize,
    patch: NativeSecondaryTargetEffectPatch,
) {
    if let Some(effect) = effects
        .iter_mut()
        .find(|effect| effect.world_key == world_key && effect.target_id == target_id)
    {
        effect.apply(patch);
    } else {
        let mut effect = NativeSecondaryTargetEffectState::new(world_key.to_owned(), target_id);
        effect.apply(patch);
        effects.push(effect);
    }
}

pub fn apply_native_secondary_dazzle(
    effects: &mut Vec<NativeSecondaryTargetEffectState>,
    world_key: &str,
    target_id: usize,
    duration_ticks: u64,
) {
    if duration_ticks > 0 {
        apply_native_secondary_target_effect(
            effects,
            world_key,
            target_id,
            NativeSecondaryTargetEffectPatch {
                dazzle_ticks: Some(duration_ticks),
                ..NativeSecondaryTargetEffectPatch::default()
            },
        );
    }
}

#[must_use]
pub fn native_secondary_target_effect<'a>(
    effects: &'a [NativeSecondaryTargetEffectState],
    world_key: &str,
    target_id: usize,
) -> Option<&'a NativeSecondaryTargetEffectState> {
    effects.iter().find(|effect| effect.world_key == world_key && effect.target_id == target_id)
}

#[must_use]
pub fn native_secondary_target_material_tint(
    world_tint: u32,
    effect: Option<&NativeSecondaryTargetEffectState>,
) -> u32 {
    let Some(effect) = effect else {
        return world_tint;
    };
    let mut red = 1.0;
    let mut green = 1.0;
    if effect.cold_slow_ticks > 0 && effect.cold_slow_material {
        red = fround(red * 0.75);
    }
    if effect.frozen_ticks > 0 {
        let updates =
            (effect.frozen_time_scale / (0.005_f32 as f64) + 0.5).floor().clamp(0.0, 200.0) as u32;
        let mut frozen_red = 0.15_f32 as f64;
        let mut frozen_green = 0.5;
        for _ in 0..updates {
            frozen_red = fround(frozen_red + (0.00425_f32 as f64));
            frozen_green = fround(frozen_green + (0.0025_f32 as f64));
        }
        red = fround(red * fround((1.0 + frozen_red) * 0.5));
        green = fround(green * fround((1.0 + frozen_green) * 0.5));
    }
    let red = (f64::from(world_tint >> 16 & 0xff) * red + 0.5).floor().clamp(0.0, 255.0) as u32;
    let green = (f64::from(world_tint >> 8 & 0xff) * green + 0.5).floor().clamp(0.0, 255.0) as u32;
    red << 16 | green << 8 | world_tint & 0xff
}

fn merge_electric_burn(
    current: &mut Option<NativeSecondaryElectricBurnEffectState>,
    incoming: Option<NativeSecondaryElectricBurnEffectState>,
) {
    let Some(mut incoming) = incoming else {
        *current = None;
        return;
    };
    assert!(
        incoming.damage_per_tick.is_finite()
            && incoming.damage_per_tick >= 0.0
            && incoming.source_actor_id > 0
            && incoming.stun_factor.is_finite()
            && (0.0..=1.0).contains(&incoming.stun_factor)
            && incoming.ticks > 0
            && !incoming.owner_id.is_empty(),
        "ElectricBurn effect is valid"
    );
    if let Some(previous) = current {
        let ticks = incoming.ticks.max(previous.ticks);
        let damage = incoming.damage_per_tick.max(previous.damage_per_tick);
        if incoming.damage_per_tick < previous.damage_per_tick {
            incoming = previous.clone();
        }
        incoming.ticks = ticks;
        incoming.damage_per_tick = damage;
    }
    *current = Some(incoming);
}

fn merge_steamed(
    current: &mut Option<NativeSecondarySteamedEffectState>,
    incoming: Option<NativeSecondarySteamedEffectState>,
) {
    let Some(mut incoming) = incoming else {
        *current = None;
        return;
    };
    assert!(
        [
            incoming.damage_per_tick,
            incoming.ember_damage,
            incoming.explode_damage,
            incoming.explode_radius
        ]
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0)
            && incoming.source_actor_id > 0
            && incoming.ticks > 0
            && !incoming.owner_id.is_empty(),
        "Steamed effect is valid"
    );
    if let Some(previous) = current {
        let damage = incoming.damage_per_tick.max(previous.damage_per_tick);
        let ember_damage = incoming.ember_damage.max(previous.ember_damage);
        let fragments = incoming.ember_fragments.max(previous.ember_fragments);
        let explosion = incoming.explode_damage.max(previous.explode_damage);
        let radius = incoming.explode_radius.max(previous.explode_radius);
        let ticks = incoming.ticks.max(previous.ticks);
        if incoming.damage_per_tick < previous.damage_per_tick {
            incoming = previous.clone();
        }
        incoming.damage_per_tick = damage;
        incoming.ember_damage = ember_damage;
        incoming.ember_fragments = fragments;
        incoming.explode_damage = explosion;
        incoming.explode_radius = radius;
        incoming.ticks = ticks;
    }
    *current = Some(incoming);
}
