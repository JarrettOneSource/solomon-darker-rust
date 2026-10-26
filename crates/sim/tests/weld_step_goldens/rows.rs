//! Physical/ownership projection used by the full primary-world journey fixture.
//! Family fixtures retain the complete individual actor rows and draw programs.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::spells::model::PrimaryTransientRef;
use solomon_sim::spells::model::*;
use solomon_sim::spells::weld::WeldHailChildRef;
use solomon_sim::spells::weld::WeldPersistentRef;
use solomon_sim::spells::weld::WeldWorldRef;
use solomon_sim::spells::weld::*;

pub fn point(value: Vec2) -> [f64; 2] {
    [value.x, value.y]
}
fn registration(value: PainterRegistration) -> Value {
    json!([
        if value.manager_lane == WorldManagerLane::Actor { "actor" } else { "transient" },
        value.registration_ordinal
    ])
}
fn owner(
    value: &PrimarySpellTransientBase,
    kind: &str,
    build: Option<u32>,
    physics: Value,
) -> Value {
    json!({"age":value.age_ticks,"build":build,"id":value.id,"kind":kind,"light":value.light_registration.map(registration),"painters":value.painter_registrations.iter().copied().map(registration).collect::<Vec<_>>(),"physics":physics})
}
pub fn projectile(value: PrimaryProjectileRef<'_>) -> Value {
    let PrimaryProjectileRef::Weld(spell) = value else { panic!("Weld journey projectile") };
    let common = &spell.common;
    json!({"age":common.age_ticks,"build":spell.build_id,"id":common.id,"kind":"weld","light":registration(common.registration),"painters":[registration(common.registration)],
        "physics":{"damage":common.damage,"contactsRemaining":spell.contacts_remaining,"hitTargetIds":spell.hit_target_ids,"headingDegrees":spell.heading_degrees,"underpowered":spell.underpowered,"targetId":spell.target_id,"flightTicks":common.flight_ticks,"direction":point(common.direction),"position":point(common.position),"velocity":point(common.velocity),"phase":"flight"}})
}
pub fn transient(effect: PrimaryTransientRef<'_>) -> Value {
    match effect {
        PrimaryTransientRef::Weld(actor) => world(actor),
        PrimaryTransientRef::FireParticle(actor) => owner(
            &actor.common,
            "fire",
            None,
            json!({"direction":point(actor.direction),"origin":point(actor.origin),"variant":actor.variant}),
        ),
        PrimaryTransientRef::FireImpact(actor) => {
            owner(&actor.common, "fire-impact", None, json!({"origin":point(actor.origin)}))
        }
        PrimaryTransientRef::FireExplosion(actor) => owner(
            &actor.common,
            "fire-explosion",
            None,
            json!({"damage":actor.damage,"origin":point(actor.origin)}),
        ),
        PrimaryTransientRef::FireEmber(actor) => owner(
            &actor.common,
            "fire-ember",
            None,
            json!({"damage":actor.damage,"life":actor.life,"contactDue":actor.contact_due,"phase":actor.phase,"height":actor.height,"verticalVelocity":actor.vertical_velocity,"position":point(actor.position)}),
        ),
        _ => panic!("Weld journey effect"),
    }
}
fn world(actor: WeldWorldRef<'_>) -> Value {
    let common = actor.common();
    let (kind, mut physics) = match actor {
        WeldWorldRef::BlizzardChainFrost(_) => ("weld-blizzard-chain-frost", json!({})),
        WeldWorldRef::BlizzardGlow(actor) => (
            "weld-blizzard-glow",
            json!({"rotationDegrees":actor.rotation_degrees,"scale":actor.scale,"variant":actor.variant}),
        ),
        WeldWorldRef::Channel(actor) => (
            "weld-channel",
            json!({"endpoint":actor.endpoint.map(point),"midpoint":actor.midpoint.map(point),"targetId":actor.target_id,"underpowered":actor.underpowered,"variant":actor.variant}),
        ),
        WeldWorldRef::BoulderDebris(actor) => {
            let debris = &actor.debris.common;
            (
                "weld-boulder-debris",
                json!({"position":point(actor.position),"debris":[debris.alpha,debris.height,point(debris.position),point(debris.velocity),debris.vertical_velocity]}),
            )
        }
        WeldWorldRef::FlameLashFade(actor) => (
            "weld-flame-lash-fade",
            json!({"alpha":actor.alpha,"position":point(actor.position),"rotationDegrees":actor.rotation_degrees,"variant":match actor.variant {NativeWeldFlameLashFadeVariant::Chain=>"chain",NativeWeldFlameLashFadeVariant::Endpoint=>"endpoint",NativeWeldFlameLashFadeVariant::Source=>"source"}}),
        ),
        WeldWorldRef::FrostFade(actor) => ("weld-frost-fade", json!({"scale":actor.scale})),
        WeldWorldRef::GroundSparkFade(actor) => {
            let seed = actor.seed;
            (
                "weld-ground-spark-fade",
                json!({"alpha":seed.alpha,"position":point(seed.position),"rotationDegrees":seed.rotation_degrees,"scale":seed.scale}),
            )
        }
        WeldWorldRef::HailRockFade(actor) => (
            "weld-hail-rock-fade",
            json!({"position":point(actor.position),"rotationDegrees":actor.rotation_degrees}),
        ),
        WeldWorldRef::HailChild(actor) => match actor {
            WeldHailChildRef::Flash(actor) => {
                ("weld-hail-flash", json!({"alpha":actor.alpha,"position":point(actor.position)}))
            }
            WeldHailChildRef::Knockback(actor) => (
                "weld-hail-knockback",
                json!({"remainingTicks":actor.remaining_ticks,"targetId":actor.target_id,"delta":point(actor.delta)}),
            ),
            WeldHailChildRef::Line(actor) => (
                "weld-hail-line",
                json!({"alpha":actor.alpha,"start":point(actor.start),"end":point(actor.end)}),
            ),
            WeldHailChildRef::TerrainBouncer(actor) => (
                "weld-hail-terrain-bouncer",
                json!({"alpha":actor.alpha,"scale":actor.scale,"height":actor.height,"rotationDegrees":actor.rotation_degrees,"verticalVelocity":actor.vertical_velocity,"position":point(actor.position),"velocity":point(actor.velocity)}),
            ),
            WeldHailChildRef::TerrainParticle(actor) => (
                "weld-hail-terrain-particle",
                json!({"alpha":actor.alpha,"scale":actor.scale,"rotationDegrees":actor.rotation_degrees,"position":point(actor.position),"velocity":point(actor.velocity)}),
            ),
        },
        WeldWorldRef::Impact(actor) => (
            "weld-impact",
            json!({"alpha":actor.alpha,"position":point(actor.position),"boulderTerminalCharge":actor.boulder_terminal_charge}),
        ),
        WeldWorldRef::Meteor(actor) => (
            "weld-meteor",
            json!({"damage":actor.damage,"pulseSequence":actor.pulse_sequence,"impactDue":actor.impact_due,"fallHeight":actor.fall_height,"impactTicksRemaining":actor.impact_ticks_remaining,"underpowered":actor.underpowered,"phase":if actor.phase==NativeWeldMeteorPhase::Fall {"fall"}else{"impact"},"position":point(actor.position)}),
        ),
        WeldWorldRef::MeteorFlash(actor) => (
            "weld-meteor-flash",
            json!({"alpha":actor.alpha,"position":point(actor.position),"scale":actor.scale}),
        ),
        WeldWorldRef::MeteorMarker(actor) => (
            "weld-meteor-marker",
            json!({"alpha":actor.alpha,"rotationDegrees":actor.rotation_degrees,"scale":actor.scale}),
        ),
        WeldWorldRef::Persistent(actor) => {
            let physics = match actor {
                WeldPersistentRef::EtherealBoulder(actor) => {
                    json!({"scale":actor.scale,"damage":actor.damage,"remainingDamage":actor.remaining_damage,"hitTargetIds":actor.hit_target_ids,"pulseSequence":actor.pulse_sequence,"lifetimeTicksRemaining":actor.lifetime_ticks_remaining,"flightTicks":actor.flight_ticks,"quantity":actor.quantity,"assemblyScale":actor.assembly_scale,"shellScale":actor.shell_scale,"phase":if actor.phase==PrimarySpellProjectilePhase::Held {"held"}else{"flight"},"velocity":point(actor.velocity)})
                }
                WeldPersistentRef::Hailstones(actor) => {
                    json!({"scale":actor.scale,"damage":actor.damage,"pulseSequence":actor.pulse_sequence,"collisionRadius":actor.collision_radius,"releaseAgeTicks":actor.release_age_ticks,"phase":if actor.phase==PrimarySpellProjectilePhase::Held {"held"}else{"flight"},"rocks":actor.rocks.iter().map(|rock|json!([rock.rock_id,rock.damage_remaining,[rock.local_position.x,rock.local_position.y,rock.local_position.z],rock.release_offset.map(point),rock.phase,rock.decay])).collect::<Vec<_>>()})
                }
                WeldPersistentRef::MeteorField(actor) => {
                    json!({"phase":"held","pulseSequence":actor.pulse_sequence})
                }
            };
            ("weld-persistent", physics)
        }
        WeldWorldRef::Steam(actor) => (
            "weld-steam",
            json!({"scale":actor.scale,"life":actor.life,"phase":actor.phase,"contactDue":actor.contact_due,"contactDamage":actor.contact_damage,"contactEnabled":actor.contact_enabled,"position":point(actor.position),"rotationDegrees":actor.rotation_degrees,"velocity":point(actor.velocity),"variant":if actor.variant==NativeWeldSteamVariant::Normal {"normal"}else{"over"}}),
        ),
    };
    physics["direction"] = json!(point(common.direction));
    physics["origin"] = json!(point(common.origin));
    owner(&common.owner, kind, Some(common.build_id), physics)
}
