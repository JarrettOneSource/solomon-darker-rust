//! Retained Golem and weapon motion is authority, including its future random draws.

use serde::Deserialize;
use solomon_sim::appearance::equipment::PlayerWeaponAppearance;
use solomon_sim::death_animations::*;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::spells::casting::NativePlayerWeaponKind;
use solomon_sim::{NativeRng, Vec2};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Goldens {
    source_revision: String,
    golems: Vec<Golem>,
    weapons: Vec<Weapon>,
}

#[derive(Deserialize)]
struct Golem {
    seed: i64,
    enhanced: bool,
    frames: Vec<GolemFrame>,
}

#[derive(Deserialize)]
struct GolemFrame {
    tick: u64,
    rng: Vec<u32>,
    star: [f64; 2],
    fragments: Vec<Option<[f64; 10]>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Weapon {
    seed: i64,
    heading_index: usize,
    frames: Vec<WeaponFrame>,
}

#[derive(Deserialize)]
struct WeaponFrame {
    tick: u64,
    rng: Vec<u32>,
    actors: Vec<[f64; 11]>,
}

fn rng_values(rng: &NativeRng) -> Vec<u32> {
    let mut values = vec![rng.index_a as u32, rng.index_b as u32];
    values.extend(rng.words);
    values
}

fn motion_values(motion: &NativeBouncerMotion) -> [f64; 9] {
    [
        motion.bounce_velocity,
        motion.height,
        motion.position.x,
        motion.position.y,
        motion.rotation_degrees,
        motion.rotation_step_degrees,
        motion.velocity.x,
        motion.velocity.y,
        motion.vertical_velocity,
    ]
}

#[test]
fn current_retained_death_fragments_and_weapons_match_the_source_through_grounding_and_expiry() {
    let fixture = include_str!("fixtures/website/current-death-animation-goldens.json");
    let goldens: Goldens = serde_json::from_str(fixture).unwrap();
    assert_eq!(goldens.source_revision, "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in goldens.golems {
        let mut rng = NativeRng::new(case.seed);
        let mut animation =
            create_native_golem_death_animation(&mut rng, Vec2::new(321.25, 245.5), case.enhanced);
        let mut tick = 0;
        for frame in case.frames {
            while tick < frame.tick {
                tick += 1;
                step_native_golem_death_animation(&mut animation, tick, &mut rng);
            }
            let context = format!("Golem seed{} enhanced{} tick{tick}", case.seed, case.enhanced);
            assert_eq!(rng_values(&rng), frame.rng, "{context} shared RNG");
            assert_eq!(
                [animation.star_rotation_degrees, animation.star_step_degrees],
                frame.star,
                "{context} star"
            );
            let fragments: Vec<_> = animation
                .fragments
                .iter()
                .map(|slot| {
                    slot.map(|fragment| {
                        let [bounce, height, x, y, rotation, spin, vx, vy, vertical] =
                            motion_values(&fragment.motion);
                        [fragment.life, bounce, height, x, y, rotation, spin, vx, vy, vertical]
                    })
                })
                .collect();
            assert_eq!(fragments, frame.fragments, "{context} complete fragments");
        }
    }
    for case in goldens.weapons {
        let mut rng = NativeRng::new(case.seed);
        let mut actors = [create_native_death_weapon_actor(
            NativeDeathWeaponBirth {
                death_epoch: 4,
                heading_index: case.heading_index,
                id: 9,
                owner_id: "hero",
                painter_registration: PainterRegistration {
                    manager_lane: WorldManagerLane::Transient,
                    registration_ordinal: 83,
                },
                position: Vec2::new(321.25, 245.5),
                tick: 10,
                weapon: PlayerWeaponAppearance { kind: NativePlayerWeaponKind::Staff, selector: 2 },
            },
            &mut rng,
        )]
        .into_iter()
        .collect::<solomon_sim::ComponentEntities<_>>();
        let mut tick = 10;
        for frame in case.frames {
            while tick < frame.tick {
                tick += 1;
                step_native_death_weapon_actors(&mut actors, tick, &mut rng);
            }
            let context =
                format!("Weapon seed{} facing{} tick{tick}", case.seed, case.heading_index);
            assert_eq!(rng_values(&rng), frame.rng, "{context} shared RNG");
            let values: Vec<_> = actors
                .iter()
                .map(|actor| {
                    let [bounce, height, x, y, rotation, spin, vx, vy, vertical] =
                        motion_values(&actor.motion);
                    [
                        actor.age_ticks as f64,
                        actor.life,
                        bounce,
                        height,
                        x,
                        y,
                        rotation,
                        spin,
                        vx,
                        vy,
                        vertical,
                    ]
                })
                .collect();
            assert_eq!(values, frame.actors, "{context} retained motion");
            let actor = &actors[0];
            assert_eq!((actor.birth_tick, actor.death_epoch, actor.id), (10, 4, 9));
            assert_eq!(actor.owner_id, "hero");
            assert_eq!(
                actor.weapon,
                PlayerWeaponAppearance { kind: NativePlayerWeaponKind::Staff, selector: 2 }
            );
            assert_eq!(actor.painter_registration.registration_ordinal, 83);
        }
    }
}
