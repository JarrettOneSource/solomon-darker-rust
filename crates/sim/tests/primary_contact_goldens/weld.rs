//! Reads source-authored actors at the contact boundary. Birth and lifetime
//! fixtures already own the constructors; these rows preserve their real fields.
use super::{integer, number, transient_common, values, vector};
use serde_json::Value;
use solomon_sim::spells::earth_debris::NativeWeldMeteorDebrisSeed;
use solomon_sim::spells::model::{PrimarySpellProjectileBase, PrimarySpellProjectilePhase};
use solomon_sim::spells::weld::*;

fn flag(value: &Value, key: &str) -> bool {
    value[key].as_bool().unwrap()
}
fn scalars(value: &Value, key: &str) -> Vec<f64> {
    values(value, key).iter().map(|v| v.as_f64().unwrap()).collect()
}
fn strings(value: &Value, key: &str) -> Vec<String> {
    values(value, key).iter().map(|v| v.as_str().unwrap().into()).collect()
}
fn phase(value: &Value) -> PrimarySpellProjectilePhase {
    if value["phase"] == "held" {
        PrimarySpellProjectilePhase::Held
    } else {
        PrimarySpellProjectilePhase::Flight
    }
}
fn common(value: &Value) -> NativeWeldActorBase {
    NativeWeldActorBase {
        owner: transient_common(value),
        birth_tick: integer(value, "birthTick"),
        build_id: integer(value, "buildId") as u32,
        direction: vector(&value["direction"]),
        origin: vector(&value["origin"]),
        vector: scalars(value, "vector"),
    }
}
pub fn projectile(value: &Value, common: PrimarySpellProjectileBase) -> NativeWeldProjectileState {
    NativeWeldProjectileState {
        common,
        ball_lightning_acceleration: value["ballLightningAcceleration"].as_f64(),
        base_presentation_phase_degrees: value["basePresentationPhaseDegrees"].as_f64(),
        frost_pulse_aspect: value["frostPulseAspect"].as_f64(),
        frost_turn_degrees: value["frostTurnDegrees"].as_f64(),
        secondary_presentation_phase_degrees: value["secondaryPresentationPhaseDegrees"].as_f64(),
        build_id: integer(value, "buildId") as u32,
        contacts_remaining: integer(value, "contactsRemaining") as u32,
        cast_playback_rate: number(value, "castPlaybackRate"),
        heading_degrees: number(value, "headingDegrees"),
        speed: number(value, "speed"),
        turn_accumulator: number(value, "turnAccumulator"),
        turn_input: number(value, "turnInput"),
        cast_sound_variant: value["castSoundVariant"].as_u64().map(|v| v as u32),
        presentation_seed: value["presentationSeed"].as_u64().map(|v| v as u32),
        hit_target_ids: strings(value, "hitTargetIds"),
        ground_spark_native_age_ticks: value["groundSparkNativeAgeTicks"].as_u64(),
        ground_spark_turn_ticks_remaining: value["groundSparkTurnTicksRemaining"].as_i64(),
        projectile_index: integer(value, "projectileIndex") as usize,
        reacquires_target: flag(value, "reacquiresTarget"),
        underpowered: flag(value, "underpowered"),
        target_id: value["targetId"].as_str().map(String::from),
        vector: scalars(value, "vector"),
        frost_presentation_lanes: value["frostPresentationLanes"].as_array().map(|lanes| {
            std::array::from_fn(|i| NativeWeldFrostPresentationLane {
                aspect: number(&lanes[i], "aspect"),
                rotation_degrees: number(&lanes[i], "rotationDegrees"),
                scale: number(&lanes[i], "scale"),
            })
        }),
    }
}
pub fn actor(value: &Value) -> NativeWeldWorldActor {
    match value["kind"].as_str().unwrap() {
        "weld-hail-knockback" => NativeWeldWorldActor::HailChild(
            NativeWeldHailChildActorState::Knockback(NativeWeldHailKnockbackState {
                common: common(value),
                delta: vector(&value["delta"]),
                remaining_ticks: integer(value, "remainingTicks"),
                target_id: super::text(value, "targetId"),
            }),
        ),
        "weld-channel" => NativeWeldWorldActor::Channel(NativeWeldChannelActorState {
            common: common(value),
            enhanced_effects: true,
            flame_visual: if value["buildId"] == 1003 {
                Some(NativeFlameChannelVisual::LegacySuppressed)
            } else {
                None
            },
            endpoint: value["endpoint"].as_object().map(|_| vector(&value["endpoint"])),
            midpoint: value["midpoint"].as_object().map(|_| vector(&value["midpoint"])),
            target_id: value["targetId"].as_str().map(String::from),
            underpowered: flag(value, "underpowered"),
            variant: integer(value, "variant") as u32,
        }),
        "weld-persistent" => NativeWeldWorldActor::Persistent(persistent(value)),
        "weld-meteor" => NativeWeldWorldActor::Meteor(meteor(value)),
        "weld-steam" => NativeWeldWorldActor::Steam(steam(value)),
        _ => panic!("unexpected Weld contact input"),
    }
}
fn persistent(value: &Value) -> NativeWeldPersistentActorState {
    if value["buildId"] == 1006 {
        return NativeWeldPersistentActorState::EtherealBoulder(NativeWeldEtherealBoulderState {
            common: common(value),
            phase: phase(value),
            orientation: std::array::from_fn(|i| value["orientation"][i].as_f64().unwrap()),
            assembly_scale: number(value, "assemblyScale"),
            damage: number(value, "damage"),
            maximum_scale: number(value, "maximumScale"),
            remaining_damage: number(value, "remainingDamage"),
            scale: number(value, "scale"),
            shell_scale: number(value, "shellScale"),
            speed_factor: number(value, "speedFactor"),
            toughness: number(value, "toughness"),
            flight_ticks: integer(value, "flightTicks"),
            lifetime_ticks_remaining: integer(value, "lifetimeTicksRemaining"),
            pulse_sequence: integer(value, "pulseSequence"),
            quantity: integer(value, "quantity") as u32,
            hit_target_ids: strings(value, "hitTargetIds"),
            velocity: vector(&value["velocity"]),
        });
    }
    NativeWeldPersistentActorState::Hailstones(NativeWeldHailstonesState {
        common: common(value),
        phase: phase(value),
        collision_radius: number(value, "collisionRadius"),
        damage: number(value, "damage"),
        maximum_scale: number(value, "maximumScale"),
        pushback: number(value, "pushback"),
        scale: number(value, "scale"),
        toughness: number(value, "toughness"),
        widen: number(value, "widen"),
        pulse_sequence: integer(value, "pulseSequence"),
        release_age_ticks: value["releaseAgeTicks"].as_u64(),
        release_fade_scale: value["releaseFadeScale"].as_f64(),
        rocks: values(value, "rocks")
            .iter()
            .map(|value| NativeWeldHailstoneRockState {
                damage_remaining: number(value, "damageRemaining"),
                decay: number(value, "decay"),
                local_position: NativeWeldHailstoneLocalPosition {
                    x: number(&value["localPosition"], "x"),
                    y: number(&value["localPosition"], "y"),
                    z: number(&value["localPosition"], "z"),
                },
                phase: number(value, "phase"),
                rock_id: integer(value, "rockId") as usize,
                release_offset: value["releaseOffset"]
                    .as_object()
                    .map(|_| vector(&value["releaseOffset"])),
                sprite_record: integer(value, "spriteRecord") as usize,
                visual_scale: number(value, "visualScale"),
            })
            .collect(),
    })
}
fn meteor(value: &Value) -> NativeWeldMeteorActorState {
    NativeWeldMeteorActorState {
        common: common(value),
        camera_displacement: value["cameraDisplacement"]
            .as_object()
            .map(|_| vector(&value["cameraDisplacement"])),
        phase: if value["phase"] == "fall" {
            NativeWeldMeteorPhase::Fall
        } else {
            NativeWeldMeteorPhase::Impact
        },
        debris: values(value, "debris").iter().map(debris).collect(),
        body_scale: number(value, "bodyScale"),
        damage: number(value, "damage"),
        fall_heading_degrees: number(value, "fallHeadingDegrees"),
        fall_height: number(value, "fallHeight"),
        fall_step: number(value, "fallStep"),
        impact_radius_scalar: number(value, "impactRadiusScalar"),
        impact_rotation_degrees: number(value, "impactRotationDegrees"),
        impact_age_ticks: integer(value, "impactAgeTicks"),
        impact_ticks_remaining: integer(value, "impactTicksRemaining"),
        pulse_sequence: integer(value, "pulseSequence"),
        pulse_ticks_remaining: integer(value, "pulseTicksRemaining"),
        landing_position: vector(&value["landingPosition"]),
        position: vector(&value["position"]),
        impact_due: flag(value, "impactDue"),
        pulse_due: flag(value, "pulseDue"),
        underpowered: flag(value, "underpowered"),
        impact_sound_pitch: value["impactSoundPitch"].as_f64(),
        impact_throw_fire_pitch: value["impactThrowFirePitch"].as_f64(),
        private_seed: integer(value, "privateSeed") as u32,
    }
}
fn debris(value: &Value) -> NativeWeldMeteorDebrisSeed {
    NativeWeldMeteorDebrisSeed {
        alpha: number(value, "alpha"),
        color_green: number(value, "colorGreen"),
        height: number(value, "height"),
        rotation_degrees: number(value, "rotationDegrees"),
        rotation_step_degrees: number(value, "rotationStepDegrees"),
        scale: number(value, "scale"),
        vertical_velocity: number(value, "verticalVelocity"),
        index: integer(value, "index") as usize,
        record: integer(value, "record") as usize,
        position: vector(&value["position"]),
        velocity: vector(&value["velocity"]),
    }
}
fn steam(value: &Value) -> NativeWeldSteamActorState {
    NativeWeldSteamActorState {
        common: common(value),
        variant: if value["variant"] == "normal" {
            NativeWeldSteamVariant::Normal
        } else {
            NativeWeldSteamVariant::Over
        },
        alpha_multiplier: number(value, "alphaMultiplier"),
        blue: number(value, "blue"),
        color_rise: number(value, "colorRise"),
        contact_damage: number(value, "contactDamage"),
        life: number(value, "life"),
        life_loss: number(value, "lifeLoss"),
        phase: number(value, "phase"),
        remaining_distance: number(value, "remainingDistance"),
        rotation_degrees: number(value, "rotationDegrees"),
        scale: number(value, "scale"),
        stretch: number(value, "stretch"),
        tint_fade: number(value, "tintFade"),
        contact_due: flag(value, "contactDue"),
        contact_enabled: flag(value, "contactEnabled"),
        contact_ticks_remaining: value["contactTicksRemaining"].as_i64().unwrap(),
        position: vector(&value["position"]),
        terminal_position: vector(&value["terminalPosition"]),
        velocity: vector(&value["velocity"]),
    }
}
