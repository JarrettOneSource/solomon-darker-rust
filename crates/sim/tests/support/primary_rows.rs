//! Full Website actor rows shared by the single and multi-participant primary replays.
use serde_json::{Value, json};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::profile::primary::NativeSpentEmber;
use solomon_sim::spells::model::*;
use solomon_sim::{NativeRng, Vec2};

pub fn point(point: Vec2) -> [f64; 2] {
    [point.x, point.y]
}
pub fn registration(value: PainterRegistration) -> Value {
    let lane = match value.manager_lane {
        WorldManagerLane::Actor => "actor",
        WorldManagerLane::Transient => "transient",
    };
    json!([lane, value.registration_ordinal])
}
pub fn random_state(rng: &NativeRng) -> Value {
    json!({ "indexA": rng.index_a, "indexB": rng.index_b, "words": rng.words.as_slice() })
}
pub fn projectile_row(projectile: &PrimarySpellProjectile) -> Value {
    let (kind, detail) = match projectile {
        PrimarySpellProjectile::Weld(_) => unreachable!("this fixture casts the five elements"),
        PrimarySpellProjectile::Ether(spell) => (
            "ether",
            json!([
                spell.damage_retention,
                spell.heading_degrees,
                spell.pierces_remaining,
                spell.reacquires_target,
                spell.speed,
                spell.target_id,
                spell.turn_input,
                spell.turn_accumulator,
                spell.underpowered,
                spell.visual_scale,
            ]),
        ),
        PrimarySpellProjectile::Fire(spell) => {
            let spent = match spell.spent_ember {
                NativeSpentEmber::None => json!({ "kind": "none" }),
                NativeSpentEmber::Immolate { damage } => {
                    json!({ "kind": "immolate", "damage": damage })
                }
                NativeSpentEmber::Imp { damage, lifetime_ticks } => {
                    json!({ "kind": "imp", "damage": damage, "lifetimeTicks": lifetime_ticks })
                }
            };
            (
                "fire",
                json!([
                    spell.burn_damage,
                    spell.ember_damage,
                    spell.ember_fragments,
                    spell.explode_damage,
                    spell.explode_radius,
                    spell.private_seed,
                    spent,
                    spell.underpowered
                ]),
            )
        }
        PrimarySpellProjectile::Earth(spell) => (
            "earth",
            json!([
                spell.assembly_charge,
                spell.hit_target_ids,
                spell.maximum_charge,
                spell.orientation,
                spell.remaining_damage,
                spell.shell_charge,
                spell.toughness,
            ]),
        ),
    };
    let common = projectile.common();
    json!({ "kind": kind, "id": common.id, "age": common.age_ticks,
        "phase": if common.phase == PrimarySpellProjectilePhase::Held { "held" } else { "flight" },
        "position": point(common.position), "velocity": point(common.velocity),
        "direction": point(common.direction), "charge": common.charge, "damage": common.damage,
        "flight": common.flight_ticks, "registration": registration(common.registration), "detail": detail })
}
pub fn transient_row(effect: &PrimarySpellTransient) -> Value {
    let (kind, detail) = match effect {
        PrimarySpellTransient::Air(spell) => (
            "air",
            json!([
                spell.birth_tick,
                point(spell.direction),
                point(spell.endpoint),
                spell.hurricane_charge,
                point(spell.midpoint),
                point(spell.origin),
                spell.target_id,
                spell.underpowered,
                spell.variant,
            ]),
        ),
        PrimarySpellTransient::Water(spell) => (
            "water",
            json!([
                point(spell.direction),
                spell.obstruction_distance,
                spell.obstruction_point.map(point),
                point(spell.origin),
                spell.speed,
                spell.underpowered,
                spell.variant,
            ]),
        ),
        PrimarySpellTransient::EarthCalledRock(spell) => (
            "earth-called-rock",
            json!([
                spell.fall_velocity,
                spell.falling,
                spell.height,
                spell.lateral_magnitude,
                spell.parent_id,
                point(spell.position),
                spell.rotation,
                spell.rotation_step,
                spell.scale,
                spell.speed,
                spell.target_height,
                spell.variant,
            ]),
        ),
        PrimarySpellTransient::EarthImpact(spell) => (
            "earth-impact",
            json!([spell.birth_tick, spell.charge, spell.lifetime_ticks, point(spell.origin)]),
        ),
        PrimarySpellTransient::EtherImpact(spell) => {
            ("ether-impact", json!([spell.birth_tick, point(spell.origin), spell.visual_scale]))
        }
        PrimarySpellTransient::EtherBlast(spell) => (
            "ether-blast",
            json!([
                spell.birth_tick,
                spell.charges,
                point(spell.origin),
                random_state(&spell.presentation_rng)
            ]),
        ),
        PrimarySpellTransient::FireParticle(spell) => {
            ("fire", json!([point(spell.direction), point(spell.origin), spell.variant]))
        }
        PrimarySpellTransient::FireImpact(spell) => ("fire-impact", json!([point(spell.origin)])),
        _ => panic!("fixture must include the actual actor contract"),
    };
    let common = effect.common();
    let registrations: Vec<Value> =
        common.painter_registrations.iter().copied().map(registration).collect();
    json!({ "kind": kind, "id": common.id, "age": common.age_ticks,
        "light": common.light_registration.map(registration), "registrations": registrations, "detail": detail })
}
