//! Incoming hit and Flash decisions from `player-harmful-contact.ts`. This pure
//! stage owns its RNG draws; the host applies its damage through existing combat state.

use crate::float;
use crate::float::fround;
use crate::harden_effects::{NativeHardenChip, create_native_harden_chip};
use crate::player_rescue::{NativePlayerRescueProtection, native_rescue_damage};
use crate::profile::{PlayerProgressionState, PlayerSkillDerivedStats, PlayerSkillRuntimeState};
use crate::{NativeRng, Vec2};

pub const NATIVE_CONCENTRATED_DEFLECT_DAMAGE_FACTOR: f64 = 5.0;
pub const NATIVE_DEFLECT_REFLECTION_PADDING: f64 = 25.0;
pub const NATIVE_FLASH_RESPONSE_RADIUS: f64 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerIncomingDamage {
    pub physical_damage: f64,
    pub magic_damage: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerHarmfulContactResult {
    pub physical_damage: f64,
    pub magic_damage: f64,
    pub harden_chip: Option<NativeHardenChip>,
    pub shield_damage: f64,
    pub reflected_damage: f64,
    /// Some(pitch) is the source's deflected branch; None is the received branch.
    pub deflect_pitch: Option<f64>,
}

pub struct PlayerHarmfulContactContext<'a> {
    pub runtime: &'a PlayerSkillRuntimeState,
    pub derived: &'a PlayerSkillDerivedStats,
    pub progression: &'a PlayerProgressionState,
    pub rescue_protection: &'a NativePlayerRescueProtection,
    pub reflection_source_in_range: bool,
    pub position: Vec2,
    pub enhanced_effects: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerFlashResponse {
    pub camera_displacement: Vec2,
    pub duration_ticks: u64,
    pub grow_scales: Vec<f64>,
    pub pitch: f64,
}

pub fn resolve_player_harmful_contact(
    context: PlayerHarmfulContactContext<'_>,
    damage: PlayerIncomingDamage,
    rng: &mut NativeRng,
) -> PlayerHarmfulContactResult {
    let runtime = context.runtime;
    let derived = context.derived;
    assert!(
        damage.physical_damage.is_finite()
            && damage.physical_damage >= 0.0
            && damage.magic_damage.is_finite()
            && damage.magic_damage >= 0.0,
        "incoming player damage is finite and non-negative"
    );
    let draw = rng.draw_integer(100);
    if f64::from(draw) < derived.deflect_chance_percent.trunc() {
        let pitch = rng.draw_scaled_float(0.1_f32 as f64, true);
        let reflected = damage.physical_damage > 0.0
            && context.reflection_source_in_range
            && runtime.is_concentrated(context.progression, 68);
        return PlayerHarmfulContactResult {
            physical_damage: 0.0,
            magic_damage: 0.0,
            shield_damage: 0.0,
            harden_chip: None,
            deflect_pitch: Some(fround(1.0 + pitch)),
            reflected_damage: if reflected {
                damage.physical_damage * NATIVE_CONCENTRATED_DEFLECT_DAMAGE_FACTOR
            } else {
                0.0
            },
        };
    }
    // Chipping precedes resistance and flat armor, even if armor blocks the hit.
    let harden_chip = if damage.physical_damage > 0.0 && runtime.harden.coating > (0.2_f32 as f64) {
        create_native_harden_chip(context.position, rng, context.enhanced_effects)
    } else {
        None
    };
    let physical = fround(
        damage.physical_damage * (1.0 - derived.damage_resistance) * derived.incoming_damage_factor,
    );
    let magic = fround(
        damage.magic_damage * (1.0 - derived.magic_resistance) * derived.incoming_damage_factor,
    );
    PlayerHarmfulContactResult {
        harden_chip,
        physical_damage: native_rescue_damage(
            fround(physical - runtime.harden.armor).max(0.0),
            context.rescue_protection,
        ),
        magic_damage: native_rescue_damage(magic, context.rescue_protection),
        // Shield interception precedes armor in the Source contact system, so its
        // two rescued lanes must be added independently at this final float store.
        shield_damage: fround(
            native_rescue_damage(physical, context.rescue_protection)
                + native_rescue_damage(magic, context.rescue_protection),
        ),
        deflect_pitch: None,
        reflected_damage: 0.0,
    }
}

pub fn resolve_player_flash_response(
    derived: &PlayerSkillDerivedStats,
    rng: &mut NativeRng,
) -> Option<PlayerFlashResponse> {
    if derived.flash_chance_percent <= 0.0 {
        return None;
    }
    let chance = rng.draw_integer(100);
    if chance <= 0 || f64::from(chance) > derived.flash_chance_percent.trunc() {
        return None;
    }
    let pitch = rng.draw_scaled_float(0.2_f32 as f64, false);
    let heading = rng.draw_integer(100001);
    let degrees = fround(fround(f64::from(heading) / 100000.0) * 360.0);
    let radians = degrees * std::f64::consts::PI / 180.0;
    let mut grow_scales = Vec::with_capacity(8);
    for _ in 0..8 {
        grow_scales.push(fround(2.0 - rng.draw_scaled_float(1.0, false)));
    }
    Some(PlayerFlashResponse {
        camera_displacement: Vec2::new(
            fround(float::sin(radians) * 3.0),
            fround(-float::cos(radians) * 3.0),
        ),
        duration_ticks: derived.flash_duration_ticks,
        grow_scales,
        pitch: fround(1.0 + pitch),
    })
}

#[must_use]
pub fn player_deflect_reflection_source_in_range(
    player: Vec2,
    player_radius: f64,
    source: Vec2,
    source_radius: f64,
) -> bool {
    assert!(
        player_radius.is_finite()
            && player_radius >= 0.0
            && source_radius.is_finite()
            && source_radius >= 0.0,
        "reflection radii are finite and non-negative"
    );
    let dx = source.x - player.x;
    let dy = source.y - player.y;
    assert!(dx.is_finite() && dy.is_finite(), "reflection positions are finite");
    let reach = player_radius + source_radius + NATIVE_DEFLECT_REFLECTION_PADDING;
    dx * dx + dy * dy < reach * reach
}
