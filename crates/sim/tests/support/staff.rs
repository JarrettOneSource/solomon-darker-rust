//! Source staff fixture projections; native ownership fields stay in the shared base.

use serde_json::{Value, json};
use solomon_sim::profile::runtime::{PlayerStaffAttackOutcome, PlayerStaffDamageLane};
use solomon_sim::spells::model::PrimarySpellTransientBase;
use solomon_sim::spells::staff::*;
use solomon_sim::{NativeRng, Vec2};

pub fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

pub fn position(value: Vec2) -> Value {
    json!({ "x": value.x, "y": value.y })
}

pub fn strings(value: &Value) -> Vec<String> {
    value.as_array().unwrap().iter().map(|value| value.as_str().unwrap().into()).collect()
}

pub fn rng_snapshot(value: &NativeRng) -> Value {
    json!({ "indexA": value.index_a, "indexB": value.index_b, "words": &value.words[..] })
}

pub fn assert_snapshot(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}");
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_snapshot(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} field count");
            for (key, expected) in expected {
                assert_snapshot(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn row(common: &PrimarySpellTransientBase, kind: &str, fields: Value) -> Value {
    // Constructors do not enroll painters or lights; that is the real host policy.
    assert!(common.painter_registrations.is_empty());
    assert_eq!(common.light_registration, None);
    let mut value = json!({ "ageTicks": common.age_ticks, "id": common.id,
        "ownerId": common.owner_id, "worldKey": common.world_key, "kind": kind });
    value.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    value
}

pub fn outcome(value: &str) -> PlayerStaffAttackOutcome {
    match value {
        "normal" => PlayerStaffAttackOutcome::Normal,
        "knockback" => PlayerStaffAttackOutcome::Knockback,
        "disabling-hit" => PlayerStaffAttackOutcome::DisablingHit,
        "critical-hit" => PlayerStaffAttackOutcome::CriticalHit,
        "whirl" => PlayerStaffAttackOutcome::Whirl,
        other => panic!("unknown staff outcome {other}"),
    }
}

pub fn outcome_id(value: PlayerStaffAttackOutcome) -> &'static str {
    match value {
        PlayerStaffAttackOutcome::Normal => "normal",
        PlayerStaffAttackOutcome::Knockback => "knockback",
        PlayerStaffAttackOutcome::DisablingHit => "disabling-hit",
        PlayerStaffAttackOutcome::CriticalHit => "critical-hit",
        PlayerStaffAttackOutcome::Whirl => "whirl",
    }
}

pub fn lane(value: &str) -> PlayerStaffDamageLane {
    match value {
        "primary" => PlayerStaffDamageLane::Primary,
        "secondary" => PlayerStaffDamageLane::Secondary,
        other => panic!("unknown staff lane {other}"),
    }
}

pub fn read_action(value: &Value) -> NativePlayerStaffAction {
    let base = NativePlayerStaffActionBase {
        common: PrimarySpellTransientBase {
            age_ticks: value["ageTicks"].as_u64().unwrap(),
            id: value["id"].as_u64().unwrap(),
            owner_id: value["ownerId"].as_str().unwrap().into(),
            world_key: value["worldKey"].as_str().unwrap().into(),
            painter_registrations: Vec::new(),
            light_registration: None,
        },
        contact_sequence: value["contactSequence"].as_u64().unwrap(),
        heading_degrees: value["headingDegrees"].as_f64().unwrap(),
        origin: point(&value["origin"]),
        outcome: outcome(value["outcome"].as_str().unwrap()),
        swoosh_pitch: value["swooshPitch"].as_f64().unwrap(),
    };
    match value["kind"].as_str().unwrap() {
        "player-staff-melee" => NativePlayerStaffAction::Melee(NativePlayerStaffMeleeAction {
            base,
            action_timing_factor: value["actionTimingFactor"].as_f64().unwrap(),
            base_progress_per_tick: value["baseProgressPerTick"].as_f64().unwrap(),
            lane: lane(value["lane"].as_str().unwrap()),
            progress: value["progress"].as_f64().unwrap(),
        }),
        "player-staff-spin" => NativePlayerStaffAction::Spin(NativePlayerStaffSpinAction {
            base,
            countdown: value["countdown"].as_f64().unwrap(),
            turn_sign: value["turnSign"].as_i64().unwrap() as i8,
        }),
        other => panic!("unknown staff action {other}"),
    }
}

pub fn action_snapshot(action: &NativePlayerStaffAction) -> Value {
    let base = action.base();
    let mut fields = json!({ "contactSequence": base.contact_sequence,
        "headingDegrees": base.heading_degrees, "origin": position(base.origin),
        "outcome": outcome_id(base.outcome), "swooshPitch": base.swoosh_pitch });
    let (kind, specific) = match action {
        NativePlayerStaffAction::Melee(action) => (
            "player-staff-melee",
            json!({
            "actionTimingFactor": action.action_timing_factor,
            "baseProgressPerTick": action.base_progress_per_tick,
            "lane": action.lane.as_str(), "progress": action.progress }),
        ),
        NativePlayerStaffAction::Spin(action) => (
            "player-staff-spin",
            json!({
            "countdown": action.countdown, "turnSign": action.turn_sign }),
        ),
    };
    fields.as_object_mut().unwrap().extend(specific.as_object().unwrap().clone());
    row(action.common(), kind, fields)
}

pub fn read_target(value: &Value) -> NativeStaffTarget {
    NativeStaffTarget {
        collision_radius: value["collisionRadius"].as_f64().unwrap(),
        id: value["id"].as_str().unwrap().into(),
        position: point(&value["position"]),
        registration_order: value["registrationOrder"].as_u64().map(|value| value as usize),
    }
}

pub fn impact_snapshot(value: &NativeStaffPhysicalImpact) -> Value {
    json!({ "contactKnockbackDelta": value.contact_knockback_delta.map(position),
        "pikeBreakPresentationRng": value.pike_break_presentation_rng.as_ref().map(rng_snapshot),
        "soundPitch": value.sound_pitch, "targetId": value.target_id,
        "verticalVelocity": value.vertical_velocity })
}

pub fn event_snapshot(value: &NativePlayerStaffContactEvent) -> Value {
    row(
        &value.common,
        "player-staff-contact",
        json!({
        "impactSoundPitches": value.impact_sound_pitches, "origin": position(value.origin),
        "outcome": outcome_id(value.outcome), "procSound": value.proc_sound.map(NativeStaffProcSound::as_str),
        "procSoundPitches": value.proc_sound_pitches, "pikeBreakSoundIndexes": value.pike_break_sound_indexes,
        "swooshPitch": value.swoosh_pitch, "targetIds": value.target_ids }),
    )
}

pub fn vfx_snapshot(value: &NativePlayerStaffVfx) -> Value {
    let (kind, fields) = match value {
        NativePlayerStaffVfx::Smoke(vfx) => (
            "player-staff-smoke",
            json!({
            "alpha": vfx.alpha, "alphaLoss": vfx.alpha_loss, "angularVelocityDegrees": vfx.angular_velocity_degrees,
            "entry": vfx.entry, "position": position(vfx.position), "rotationDegrees": vfx.rotation_degrees,
            "scale": vfx.scale }),
        ),
        NativePlayerStaffVfx::MoveFade(vfx) => (
            "player-staff-move-fade",
            json!({
            "alpha": vfx.alpha, "alphaLoss": vfx.alpha_loss, "entry": vfx.entry,
            "position": position(vfx.position), "rotationDegrees": vfx.rotation_degrees,
            "scale": vfx.scale, "tint": vfx.tint, "velocity": position(vfx.velocity),
            "velocityFactor": vfx.velocity_factor }),
        ),
        NativePlayerStaffVfx::PerspectiveFade(vfx) => (
            "player-staff-perspective-fade",
            json!({
            "alpha": vfx.alpha, "alphaLoss": vfx.alpha_loss, "entry": vfx.entry,
            "position": position(vfx.position), "rotationDegrees": vfx.rotation_degrees,
            "scale": vfx.scale, "tint": vfx.tint }),
        ),
    };
    row(value.common(), kind, fields)
}

pub fn read_vfx(value: &Value) -> NativePlayerStaffVfx {
    let common = PrimarySpellTransientBase {
        age_ticks: value["ageTicks"].as_u64().unwrap(),
        id: value["id"].as_u64().unwrap(),
        owner_id: value["ownerId"].as_str().unwrap().into(),
        world_key: value["worldKey"].as_str().unwrap().into(),
        painter_registrations: Vec::new(),
        light_registration: None,
    };
    let alpha = value["alpha"].as_f64().unwrap();
    let alpha_loss = value["alphaLoss"].as_f64().unwrap();
    let entry = value["entry"].as_u64().unwrap() as usize;
    let position = point(&value["position"]);
    let rotation_degrees = value["rotationDegrees"].as_f64().unwrap();
    let scale = value["scale"].as_f64().unwrap();
    match value["kind"].as_str().unwrap() {
        "player-staff-smoke" => NativePlayerStaffVfx::Smoke(NativePlayerStaffSmokeVfx {
            common,
            alpha,
            alpha_loss,
            entry,
            position,
            rotation_degrees,
            scale,
            angular_velocity_degrees: value["angularVelocityDegrees"].as_f64().unwrap(),
        }),
        "player-staff-move-fade" => NativePlayerStaffVfx::MoveFade(NativePlayerStaffMoveFadeVfx {
            common,
            alpha,
            alpha_loss,
            entry,
            position,
            rotation_degrees,
            scale,
            tint: value["tint"].as_u64().unwrap() as u32,
            velocity: point(&value["velocity"]),
            velocity_factor: value["velocityFactor"].as_f64().unwrap(),
        }),
        "player-staff-perspective-fade" => {
            NativePlayerStaffVfx::PerspectiveFade(NativePlayerStaffPerspectiveVfx {
                common,
                alpha,
                alpha_loss,
                entry,
                position,
                rotation_degrees,
                scale,
                tint: value["tint"].as_u64().unwrap() as u32,
            })
        }
        other => panic!("unknown staff VFX {other}"),
    }
}

pub fn knockback_snapshot(value: &NativeStaffKnockbackActor) -> Value {
    row(
        &value.common,
        "player-staff-knockback",
        json!({
        "arcDegrees": value.arc_degrees, "origin": position(value.origin),
        "remainingDistance": value.remaining_distance, "targetIds": value.target_ids }),
    )
}

pub fn knockback_step_snapshot(value: &NativeStaffKnockbackStep, rng: &NativeRng) -> Value {
    let displacements = value
        .displacements
        .iter()
        .map(|value| {
            json!({
        "delta": position(value.delta), "targetId": value.target_id })
        })
        .collect::<Vec<_>>();
    let headings = value
        .heading_perturbations
        .iter()
        .map(|value| {
            json!({
        "headingDegrees": value.heading_degrees, "targetId": value.target_id })
        })
        .collect::<Vec<_>>();
    json!({ "actor": value.actor.as_ref().map(knockback_snapshot),
        "cameraDisplacement": position(value.camera_displacement),
        "dazzledTargetIds": value.dazzled_target_ids, "displacements": displacements,
        "headingPerturbations": headings, "rng": rng_snapshot(rng) })
}

pub fn contact_knockback_snapshot(value: &NativePlayerStaffContactKnockback) -> Value {
    row(
        &value.common,
        "player-staff-contact-knockback",
        json!({
        "delta": position(value.delta), "remainingTicks": value.remaining_ticks, "targetId": value.target_id }),
    )
}

pub fn pike_snapshot(value: &NativePlayerStaffPikeBreakVfx) -> Value {
    row(
        &value.common,
        "player-staff-pike-break",
        json!({
        "enhancedShadow":value.enhanced_shadow,
        "headingDegrees": value.heading_degrees, "position": position(value.position),
        "presentationRng": rng_snapshot(&value.presentation_rng), "targetId": value.target_id }),
    )
}
