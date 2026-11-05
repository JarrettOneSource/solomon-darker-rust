//! Physical/ownership projection used by the full primary-world journey fixture.
//! Family fixtures retain the complete individual actor rows and draw programs.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::spells::model::PrimarySpellTransient;
use solomon_sim::spells::model::*;
use solomon_sim::spells::weld::WeldHailChildActorState;
use solomon_sim::spells::weld::WeldPersistentActorState;
use solomon_sim::spells::weld::WeldWorldActor;
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
pub fn projectile(value: &PrimarySpellProjectile) -> Value {
    let PrimarySpellProjectile::Weld(spell) = value else { panic!("Weld journey projectile") };
    let common = &spell.common;
    json!({"age":common.age_ticks,"build":spell.build_id,"id":common.id,"kind":"weld","light":registration(common.registration),"painters":[registration(common.registration)],
        "physics":{"damage":common.damage,"contactsRemaining":spell.contacts_remaining,"hitTargetIds":spell.hit_target_ids,"headingDegrees":spell.heading_degrees,"underpowered":spell.underpowered,"targetId":spell.target_id,"flightTicks":common.flight_ticks,"direction":point(common.direction),"position":point(common.position),"velocity":point(common.velocity),"phase":"flight"}})
}
pub fn transient(effect: &PrimarySpellTransient) -> Value {
    match effect {
        PrimarySpellTransient::Weld(actor) => world(actor),
        PrimarySpellTransient::FireParticle(actor) => owner(
            &actor.common,
            "fire",
            None,
            json!({"direction":point(actor.direction),"origin":point(actor.origin),"variant":actor.variant}),
        ),
        PrimarySpellTransient::FireImpact(actor) => {
            owner(&actor.common, "fire-impact", None, json!({"origin":point(actor.origin)}))
        }
        PrimarySpellTransient::FireExplosion(actor) => owner(
            &actor.common,
            "fire-explosion",
            None,
            json!({"damage":actor.damage,"origin":point(actor.origin)}),
        ),
        PrimarySpellTransient::FireEmber(actor) => owner(
            &actor.common,
            "fire-ember",
            None,
            json!({"damage":actor.damage,"life":actor.life,"contactDue":actor.contact_due,"phase":actor.phase,"height":actor.height,"verticalVelocity":actor.vertical_velocity,"position":point(actor.position)}),
        ),
        _ => panic!("Weld journey effect"),
    }
}
fn world(actor: &WeldWorldActor) -> Value {
    let common = actor.common();
    let (kind, mut physics) = match actor {
        WeldWorldActor::BlizzardChainFrost(_) => ("weld-blizzard-chain-frost", json!({})),
        WeldWorldActor::BlizzardGlow(actor) => (
            "weld-blizzard-glow",
            json!({"rotationDegrees":actor.rotation_degrees,"scale":actor.scale,"variant":actor.variant}),
        ),
        WeldWorldActor::Channel(actor) => (
            "weld-channel",
            json!({"endpoint":actor.endpoint.map(point),"midpoint":actor.midpoint.map(point),"targetId":actor.target_id,"underpowered":actor.underpowered,"variant":actor.variant}),
        ),
        WeldWorldActor::BoulderDebris(actor) => {
            let debris = &actor.debris.common;
            (
                "weld-boulder-debris",
                json!({"position":point(actor.position),"debris":[debris.alpha,debris.height,point(debris.position),point(debris.velocity),debris.vertical_velocity]}),
            )
        }
        WeldWorldActor::FlameLashFade(actor) => (
            "weld-flame-lash-fade",
            json!({"alpha":actor.alpha,"position":point(actor.position),"rotationDegrees":actor.rotation_degrees,"variant":match actor.variant {WeldFlameLashFadeVariant::Chain=>"chain",WeldFlameLashFadeVariant::Endpoint=>"endpoint",WeldFlameLashFadeVariant::Source=>"source"}}),
        ),
        WeldWorldActor::FrostFade(actor) => ("weld-frost-fade", json!({"scale":actor.scale})),
        WeldWorldActor::GroundSparkFade(actor) => {
            let seed = actor.seed;
            (
                "weld-ground-spark-fade",
                json!({"alpha":seed.alpha,"position":point(seed.position),"rotationDegrees":seed.rotation_degrees,"scale":seed.scale}),
            )
        }
        WeldWorldActor::HailRockFade(actor) => (
            "weld-hail-rock-fade",
            json!({"position":point(actor.position),"rotationDegrees":actor.rotation_degrees}),
        ),
        WeldWorldActor::HailChild(actor) => match actor {
            WeldHailChildActorState::Flash(actor) => {
                ("weld-hail-flash", json!({"alpha":actor.alpha,"position":point(actor.position)}))
            }
            WeldHailChildActorState::Knockback(actor) => (
                "weld-hail-knockback",
                json!({"remainingTicks":actor.remaining_ticks,"targetId":actor.target_id,"delta":point(actor.delta)}),
            ),
            WeldHailChildActorState::Line(actor) => (
                "weld-hail-line",
                json!({"alpha":actor.alpha,"start":point(actor.start),"end":point(actor.end)}),
            ),
            WeldHailChildActorState::TerrainBouncer(actor) => (
                "weld-hail-terrain-bouncer",
                json!({"alpha":actor.alpha,"scale":actor.scale,"height":actor.height,"rotationDegrees":actor.rotation_degrees,"verticalVelocity":actor.vertical_velocity,"position":point(actor.position),"velocity":point(actor.velocity)}),
            ),
            WeldHailChildActorState::TerrainParticle(actor) => (
                "weld-hail-terrain-particle",
                json!({"alpha":actor.alpha,"scale":actor.scale,"rotationDegrees":actor.rotation_degrees,"position":point(actor.position),"velocity":point(actor.velocity)}),
            ),
        },
        WeldWorldActor::Impact(actor) => (
            "weld-impact",
            json!({"alpha":actor.alpha,"position":point(actor.position),"boulderTerminalCharge":actor.boulder_terminal_charge}),
        ),
        WeldWorldActor::Meteor(actor) => (
            "weld-meteor",
            json!({"damage":actor.damage,"pulseSequence":actor.pulse_sequence,"impactDue":actor.impact_due,"fallHeight":actor.fall_height,"impactTicksRemaining":actor.impact_ticks_remaining,"underpowered":actor.underpowered,"phase":if actor.phase==WeldMeteorPhase::Fall {"fall"}else{"impact"},"position":point(actor.position)}),
        ),
        WeldWorldActor::MeteorFlash(actor) => (
            "weld-meteor-flash",
            json!({"alpha":actor.alpha,"position":point(actor.position),"scale":actor.scale}),
        ),
        WeldWorldActor::MeteorMarker(actor) => (
            "weld-meteor-marker",
            json!({"alpha":actor.alpha,"rotationDegrees":actor.rotation_degrees,"scale":actor.scale}),
        ),
        WeldWorldActor::Persistent(actor) => {
            let physics = match actor {
                WeldPersistentActorState::EtherealBoulder(actor) => {
                    json!({"scale":actor.scale,"damage":actor.damage,"remainingDamage":actor.remaining_damage,"hitTargetIds":actor.hit_target_ids,"pulseSequence":actor.pulse_sequence,"lifetimeTicksRemaining":actor.tracking_ticks_remaining,"flightTicks":actor.flight_ticks,"quantity":actor.quantity,"assemblyScale":actor.assembly_scale,"shellScale":actor.shell_scale,"phase":if actor.phase==PrimarySpellProjectilePhase::Held {"held"}else{"flight"},"velocity":point(actor.velocity)})
                }
                WeldPersistentActorState::Hailstones(actor) => {
                    json!({"scale":actor.scale,"damage":actor.damage,"pulseSequence":actor.pulse_sequence,"collisionRadius":actor.collision_radius,"releaseAgeTicks":actor.release_age_ticks,"phase":if actor.phase==PrimarySpellProjectilePhase::Held {"held"}else{"flight"},"rocks":actor.rocks.iter().map(|rock|json!([rock.rock_id,rock.damage_remaining,[rock.local_position.x,rock.local_position.y,rock.local_position.z],rock.release_offset.map(point),rock.phase,rock.decay])).collect::<Vec<_>>()})
                }
                WeldPersistentActorState::MeteorField(actor) => {
                    json!({"phase":"held","pulseSequence":actor.pulse_sequence})
                }
            };
            ("weld-persistent", physics)
        }
        WeldWorldActor::Steam(actor) => (
            "weld-steam",
            json!({"scale":actor.scale,"life":actor.life,"phase":actor.phase,"contactDue":actor.contact_due,"contactDamage":actor.contact_damage,"contactEnabled":actor.contact_enabled,"position":point(actor.position),"rotationDegrees":actor.rotation_degrees,"velocity":point(actor.velocity),"variant":if actor.variant==WeldSteamVariant::Normal {"normal"}else{"over"}}),
        ),
    };
    physics["direction"] = json!(point(common.direction));
    physics["origin"] = json!(point(common.origin));
    owner(&common.owner, kind, Some(common.build_id), physics)
}
