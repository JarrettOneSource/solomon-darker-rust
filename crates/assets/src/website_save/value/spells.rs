//! Source discriminators route into the canonical nested Weld, Staff and Harden enums.
//! No wrapper tags are written to disk: the source has one flat `kind` per live actor.

use serde_json::{Value, json};

pub(super) fn route(name: &str, value: &Value) -> Option<&'static str> {
    let kind = value["kind"].as_str()?;
    match name {
        "PrimarySpellTransient" => {
            if kind.starts_with("weld-") {
                Some("Weld")
            } else if kind.starts_with("player-staff-") {
                Some("Staff")
            } else if kind.starts_with("harden-") {
                Some("Harden")
            } else {
                None
            }
        }
        "WeldWorldActor" => match kind {
            "weld-blizzard-chain-frost" => Some("BlizzardChainFrost"),
            "weld-blizzard-glow" => Some("BlizzardGlow"),
            "weld-boulder-debris" => Some("BoulderDebris"),
            "weld-channel" => Some("Channel"),
            "weld-flame-lash-fade" => Some("FlameLashFade"),
            "weld-frost-fade" => Some("FrostFade"),
            "weld-ground-spark-fade" => Some("GroundSparkFade"),
            "weld-hail-rock-fade" => Some("HailRockFade"),
            "weld-impact" => Some("Impact"),
            "weld-meteor" => Some("Meteor"),
            "weld-meteor-flash" => Some("MeteorFlash"),
            "weld-meteor-marker" => Some("MeteorMarker"),
            "weld-persistent" => Some("Persistent"),
            "weld-steam" => Some("Steam"),
            kind if kind.starts_with("weld-hail-") => Some("HailChild"),
            _ => None,
        },
        "WeldPersistentActorState" => match value["buildId"].as_u64() {
            Some(1006) => Some("EtherealBoulder"),
            Some(1007) => Some("MeteorField"),
            Some(1008) => Some("Hailstones"),
            _ => None,
        },
        "WeldHailChildActorState" => match kind {
            "weld-hail-flash" => Some("Flash"),
            "weld-hail-knockback" => Some("Knockback"),
            "weld-hail-line" => Some("Line"),
            "weld-hail-terrain-bouncer" => Some("TerrainBouncer"),
            "weld-hail-terrain-particle" => Some("TerrainParticle"),
            _ => None,
        },
        "PlayerStaffTransient" => match kind {
            "player-staff-melee" | "player-staff-spin" => Some("Action"),
            "player-staff-contact" => Some("Contact"),
            "player-staff-contact-knockback" => Some("ContactKnockback"),
            "player-staff-pike-break" => Some("PikeBreak"),
            "player-staff-knockback" => Some("Knockback"),
            "player-staff-smoke" | "player-staff-move-fade" | "player-staff-perspective-fade" => {
                Some("Vfx")
            }
            _ => None,
        },
        "PlayerStaffAction" => match kind {
            "player-staff-melee" => Some("Melee"),
            "player-staff-spin" => Some("Spin"),
            _ => None,
        },
        "PlayerStaffVfx" => match kind {
            "player-staff-smoke" => Some("Smoke"),
            "player-staff-move-fade" => Some("MoveFade"),
            "player-staff-perspective-fade" => Some("PerspectiveFade"),
            _ => None,
        },
        "HardenEffect" => match kind {
            "harden-shard" => Some("Shard"),
            "harden-burst" => Some("Burst"),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn payload(name: &str, value: Value) -> Value {
    if name == "HardenEffect" {
        if value["kind"].as_str() == Some("harden-shard") {
            json!({"owner":value,"body":value})
        } else {
            json!({"owner":value,"alpha":value["alpha"],"position":value["position"]})
        }
    } else {
        value
    }
}

pub(super) fn prepare(name: &str, value: &mut Value) -> Result<(), String> {
    if name == "GolemDeathAnimation"
        && let Some(fragments) = value["fragments"].as_array_mut()
    {
        for fragment in fragments {
            body_keys(fragment, BOUNCER_FIELDS, false);
        }
    }
    if name == "EtherDrainState" {
        for field in ["loose", "queried"] {
            if let Some(references) = value[field].as_array_mut() {
                for reference in references {
                    body_keys(reference, &["registration_ordinal"], false);
                }
            }
        }
        if let Some(animations) = value["animations"].as_array_mut() {
            for animation in animations {
                body_keys(
                    animation,
                    &[
                        "remaining_distance",
                        "oscillation_degrees",
                        "rotation_degrees",
                        "body_y_offset",
                    ],
                    false,
                );
            }
        }
    }
    let nested = match name {
        "WeldActorBase" => Some("owner"),
        "PlayerStaffMeleeAction" | "PlayerStaffSpinAction" => Some("base"),
        "WeldGroundSparkFadeActorState" => Some("seed"),
        _ => None,
    };
    if let Some(field) = nested {
        value[field] = value.clone();
    }
    // Internally tagged serde enums buffer their body before decoding it, so their
    // nested fields no longer pass through SourceValue. Adapt only those typed
    // bodies here; IDs and arbitrary map keys retain their original spelling.
    if name == "FlameChannelBirth"
        && let Some(ribbon) = value["ribbon"].as_object_mut()
    {
        for (source, native) in [("widthFactor", "width_factor"), ("phaseDegrees", "phase_degrees")]
        {
            if let Some(field) = ribbon.remove(source) {
                ribbon.insert(native.into(), field);
            }
        }
    }
    if value["flameVisual"]["kind"].as_str() == Some("native-birth-v1") {
        use solomon_sim::spells::weld::{FlameChannelBirth, FlameFadeBirth};
        let source = value["flameVisual"].clone();
        let body = match name {
            "WeldChannelActorState" => {
                serde_json::to_value(super::decode::<FlameChannelBirth>(source)?)
            }
            "WeldFlameLashFadeState" => {
                serde_json::to_value(super::decode::<FlameFadeBirth>(source)?)
            }
            _ => return Ok(()),
        }
        .map_err(|error| error.to_string())?;
        value["flameVisual"] = body;
        value["flameVisual"]["kind"] = json!("native-birth-v1");
    }
    Ok(())
}

pub(super) fn finish(name: &str, value: &mut Value) {
    if name == "GolemDeathAnimation"
        && let Some(fragments) = value["fragments"].as_array_mut()
    {
        for fragment in fragments {
            body_keys(fragment, BOUNCER_FIELDS, true);
        }
    }
    if name == "SecondaryActorState" {
        for (field, owner) in [("golemDeath", "golem-death"), ("etherDrain", "ether-drain")] {
            if value[field].is_null() && value["kind"] != owner {
                value.as_object_mut().expect("secondary actor is an object").remove(field);
            }
        }
    }
    // A Meteor field never enters flight; the source still writes its literal phase.
    if name == "WeldMeteorFieldState" {
        value["phase"] = json!("held");
    }
    let nested: &[&str] = match name {
        "WeldActorBase" => &["owner"],
        "PlayerStaffMeleeAction" | "PlayerStaffSpinAction" => &["base"],
        "WeldGroundSparkFadeActorState" => &["seed"],
        "HardenEffect" if value["kind"].as_str() == Some("harden-shard") => &["owner", "body"],
        "HardenEffect" => &["owner"],
        _ => &[],
    };
    for field in nested {
        super::enemies::flatten(value, field);
    }
    // These source families own painter roots without declaring a light slot.
    // The shared Rust transient base retains that slot for native host snapshots.
    if matches!(
        name,
        "PrimarySpellAirHurricane"
            | "PrimarySpellWaterAura"
            | "PrimarySpellWaterHail"
            | "PrimarySpellEtherBlast"
            | "PrimarySpellEtherPierceStreak"
            | "PrimarySpellFirePatch"
            | "HardenEffect"
            | "PlayerStaffActionBase"
            | "PlayerStaffContactEvent"
            | "PlayerStaffContactKnockback"
            | "PlayerStaffPikeBreakVfx"
            | "PlayerStaffSmokeVfx"
            | "PlayerStaffMoveFadeVfx"
            | "PlayerStaffPerspectiveVfx"
            | "StaffKnockbackActor"
    ) {
        value.as_object_mut().expect("serialized spell is an object").remove("lightRegistration");
    }
}

const BOUNCER_FIELDS: &[&str] =
    &["bounce_velocity", "rotation_degrees", "rotation_step_degrees", "vertical_velocity"];

// These bodies cross serde's flattened or internally tagged map buffer. Typed
// parent ownership keeps arbitrary user strings and mod dictionary keys intact.
fn body_keys(value: &mut Value, fields: &[&str], to_source: bool) {
    let Some(value) = value.as_object_mut() else {
        return;
    };
    for native in fields {
        let source = super::source_field("", native);
        let (from, to) =
            if to_source { (*native, source.as_str()) } else { (source.as_str(), *native) };
        if let Some(field) = value.remove(from) {
            value.insert(to.into(), field);
        }
    }
}

pub(super) fn transparent_wrapper(name: &str, variant: &str) -> bool {
    matches!(
        (name, variant),
        ("PrimarySpellTransient", "Weld" | "Staff" | "Harden")
            | ("WeldWorldActor", "Persistent" | "HailChild")
            | ("PlayerStaffTransient", "Action" | "Vfx")
    )
}

pub(super) fn kind(name: &str, variant: &str) -> Option<String> {
    let prefix = match name {
        "WeldWorldActor" => "weld-",
        "WeldHailChildActorState" => "weld-hail-",
        "PlayerStaffTransient" | "PlayerStaffAction" | "PlayerStaffVfx" => "player-staff-",
        "HardenEffect" => "harden-",
        "WeldPersistentActorState" => return Some("weld-persistent".into()),
        _ => return None,
    };
    Some(format!("{prefix}{}", super::kebab(variant)))
}
