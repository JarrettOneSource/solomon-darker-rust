//! Complete current secondary state and outboxes, retaining optional actor payloads.

use serde_json::{Value, json};
use solomon_sim::secondary::*;
use solomon_sim::spells::special_death::NativeSpellSpecialDeathKind;

use super::support;

/// The original adapter remains useful to historical fixtures. Current journeys also
/// observe private death/drain animations and the shared camera/warp state.
pub fn state_snapshot(state: &NativeSecondarySimulationState) -> Value {
    let mut value = support::state_snapshot(state);
    for (actor, value) in state.actors.iter().zip(value["actors"].as_array_mut().unwrap()) {
        if let Some(animation) = actor.golem_death() {
            value["golemDeath"] = serde_json::to_value(animation).unwrap();
        }
        if actor.kind == NativeSecondaryActorKind::EtherDrain || actor.ether_drain().is_some() {
            value["etherDrain"] = serde_json::to_value(actor.ether_drain()).unwrap();
        }
    }
    value["cameraDisplacements"] = serde_json::to_value(&state.camera_displacements).unwrap();
    value["stoneskinWarp"] = json!(state.stoneskin_warp);
    value
}

#[allow(dead_code)] // State-only fixtures share this module without tick-result projections.
fn damage_snapshot(contact: &NativeSecondaryDamageContact) -> Value {
    let mut value = json!({
        "amount":contact.amount,
        "kind":match contact.kind {
            NativeSecondaryDamageKind::Acid=>"acid",
            NativeSecondaryDamageKind::Fire=>"fire",
            NativeSecondaryDamageKind::Ice=>"ice",
            NativeSecondaryDamageKind::Lightning=>"lightning",
            NativeSecondaryDamageKind::Magic=>"magic",
            NativeSecondaryDamageKind::Physical=>"physical",
        },
        "ownerId":contact.owner_id,"sourceActorId":contact.source_actor_id,
        "targetId":contact.target_id,
    });
    if contact.ether_drain {
        value["etherDrain"] = json!(true);
    }
    if let Some(strength) = contact.hit_strength {
        value["hitStrength"] = json!(strength);
    }
    if contact.suppress_hit_reaction {
        value["suppressHitReaction"] = json!(true);
    }
    if contact.suppress_hurt_sound {
        value["suppressHurtSound"] = json!(true);
    }
    if let Some(payload) = contact.special_death_payload {
        value["specialDeathPayload"] = json!({
            "kind":match payload.kind {
                NativeSpellSpecialDeathKind::Fire=>"fire",
                NativeSpellSpecialDeathKind::Steam=>"steam",
            },
            "emberDamage":payload.ember_damage,"emberFragments":payload.ember_fragments,
            "explodeDamage":payload.explode_damage,"explodeRadius":payload.explode_radius,
            "privateSeed":payload.private_seed,
        });
    }
    value
}

#[allow(dead_code)] // State-only fixtures share this module without tick-result projections.
pub fn result_snapshot(result: &NativeSecondaryTickResult) -> Value {
    let mut value = support::cast_snapshot(result);
    value.as_object_mut().unwrap().remove("facingHeadingIndexes");
    value["facingHeadingDegreesByPlayer"] = json!(result.facing_heading_degrees_by_player);
    value["damage"] = Value::Array(result.damage.iter().map(damage_snapshot).collect());
    value["etherDrainContacts"] = serde_json::to_value(&result.ether_drain_contacts).unwrap();
    value["etherDrainWorldAnimationContacts"] =
        serde_json::to_value(&result.ether_drain_world_animation_contacts).unwrap();
    value["knockbacks"] = Value::Array(result.knockbacks.iter().map(|contact| json!({
        "delta":support::point_snapshot(contact.delta),"sourceActorId":contact.source_actor_id,
        "targetId":contact.target_id,
    })).collect());
    value["targetHeadingChanges"] = Value::Array(
        result
            .target_heading_changes
            .iter()
            .map(|contact| {
                json!({
                    "degrees":contact.degrees,"mode":match contact.mode {
                        NativeSecondaryHeadingMode::Absolute=>"absolute",
                        NativeSecondaryHeadingMode::Relative=>"relative",
                    },"targetId":contact.target_id,
                })
            })
            .collect(),
    );
    value["disruptedTargetIds"] = json!(result.disrupted_target_ids);
    value["healthRecovered"] = json!(result.health_recovered);
    value["overloadedPlayerIds"] = json!(result.overloaded_player_ids);
    value["primaryOverridePlayerIds"] = json!(result.primary_override_player_ids);
    value["staffCastPulsePlayerIds"] = json!(result.staff_cast_pulse_player_ids);
    value
}
