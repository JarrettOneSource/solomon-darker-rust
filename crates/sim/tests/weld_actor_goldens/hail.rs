use super::{common_row, point, random, vector, website_rows, weld_visual_rows};
use serde_json::{Value, json};
use solomon_sim::NativeRng;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::spells::model::PrimarySpellProjectilePhase;
use solomon_sim::spells::weld::*;

fn number(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap()
}

fn hail_actor(value: &Value) -> NativeWeldHailstonesState {
    let values: Vec<f64> =
        value["vector"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let birth = NativeWeldActorBirth {
        build_id: 1008,
        direction: vector(&value["direction"]),
        id: value["id"].as_u64().unwrap(),
        origin: vector(&value["origin"]),
        owner_id: "hero",
        tick: value["birthTick"].as_u64().unwrap(),
        vector: &values,
        world_key: "boneyard:weld",
    };
    let mut common = birth.actor();
    common.owner.age_ticks = value["ageTicks"].as_u64().unwrap();
    common.owner.light_registration = Some(PainterRegistration {
        manager_lane: WorldManagerLane::Actor,
        registration_ordinal: value["lightRegistration"]["registrationOrdinal"].as_u64().unwrap()
            as usize,
    });
    NativeWeldHailstonesState {
        common,
        collision_radius: number(value, "collisionRadius"),
        damage: number(value, "damage"),
        maximum_scale: number(value, "maximumScale"),
        phase: PrimarySpellProjectilePhase::Flight,
        pulse_sequence: value["pulseSequence"].as_u64().unwrap(),
        pushback: number(value, "pushback"),
        release_age_ticks: value["releaseAgeTicks"].as_u64(),
        release_fade_scale: value["releaseFadeScale"].as_f64(),
        rocks: value["rocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|rock| NativeWeldHailstoneRockState {
                damage_remaining: number(rock, "damageRemaining"),
                decay: number(rock, "decay"),
                local_position: NativeWeldHailstoneLocalPosition {
                    x: number(&rock["localPosition"], "x"),
                    y: number(&rock["localPosition"], "y"),
                    z: number(&rock["localPosition"], "z"),
                },
                phase: number(rock, "phase"),
                rock_id: rock["rockId"].as_u64().unwrap() as usize,
                release_offset: Some(vector(&rock["releaseOffset"])),
                sprite_record: rock["spriteRecord"].as_u64().unwrap() as usize,
                visual_scale: number(rock, "visualScale"),
            })
            .collect(),
        scale: number(value, "scale"),
        toughness: number(value, "toughness"),
        widen: number(value, "widen"),
    }
}

fn row(actor: &NativeWeldHailChildActorState) -> Value {
    let mut row = common_row(actor.common());
    let fields = match actor {
        NativeWeldHailChildActorState::Flash(actor) => {
            json!({"kind":"weld-hail-flash","alpha":actor.alpha,
            "alphaStep":actor.alpha_step,"position":point(actor.position),"record":actor.record})
        }
        NativeWeldHailChildActorState::Knockback(actor) => json!({"kind":"weld-hail-knockback",
            "delta":point(actor.delta),"remainingTicks":actor.remaining_ticks,"targetId":actor.target_id}),
        NativeWeldHailChildActorState::Line(actor) => {
            json!({"kind":"weld-hail-line","alpha":actor.alpha,
            "alphaStep":actor.alpha_step,"end":point(actor.end),"endAlpha":actor.end_alpha,
            "start":point(actor.start),"width":actor.width})
        }
        NativeWeldHailChildActorState::TerrainParticle(actor) => {
            json!({"kind":"weld-hail-terrain-particle",
            "alpha":actor.alpha,"alphaStep":actor.alpha_step,"position":point(actor.position),"record":actor.record,
            "rotationDegrees":actor.rotation_degrees,"scale":actor.scale,"tint":actor.tint,
            "velocity":point(actor.velocity),"velocityFactor":actor.velocity_factor})
        }
        NativeWeldHailChildActorState::TerrainBouncer(actor) => {
            json!({"kind":"weld-hail-terrain-bouncer",
            "alpha":actor.alpha,"bounceVelocity":actor.bounce_velocity,"enhancedShadow":actor.enhanced_shadow,
            "height":actor.height,"position":point(actor.position),"record":actor.record,"rotationDegrees":actor.rotation_degrees,
            "rotationStepDegrees":actor.rotation_step_degrees,"scale":actor.scale,"velocity":point(actor.velocity),
            "verticalVelocity":actor.vertical_velocity})
        }
    };
    row.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    row
}

#[test]
fn hail_terrain_children_keep_native_birth_draws_contact_presentation_and_bounce_cadence() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/website/weld-actor-goldens.json")).unwrap();
    for case in fixture["hailChildCases"].as_array().unwrap() {
        let actor = hail_actor(&case["actor"]);
        let mut rng = NativeRng::new(731);
        let mut actors = create_native_weld_hail_terrain_impact(
            &actor,
            case["enhancedEffects"].as_bool().unwrap(),
            700,
            200,
            &mut rng,
        );
        let next_id = 700 + actors.len() as u64;
        actors.extend(create_native_weld_hail_contact_presentation(
            &actor,
            solomon_sim::Vec2::new(11.0, 22.0),
            solomon_sim::Vec2::new(111.0, 222.0),
            next_id,
            200,
            &mut rng,
        ));
        if let Some(knockback) =
            create_native_weld_hail_knockback(&actor, next_id + 2, "enemy:1", 200)
        {
            actors.push(NativeWeldHailChildActorState::Knockback(knockback));
        }
        for tick in 0..=220 {
            if tick > 0 {
                actors.retain_mut(|actor| {
                    step_native_weld_hail_child(&mut actor.view_mut(), &mut rng)
                });
            }
            if let Some(expected) =
                case["frames"].as_array().unwrap().iter().find(|frame| frame["tick"] == tick)
            {
                let actual = json!({"tick":tick,"actors":actors.iter().map(row).collect::<Vec<_>>(),"rng":random(&rng)});
                website_rows::compare(&actual, expected, &format!("Hail children tick{tick}"));
                if let Some(sample) = weld_visual_rows::sample(case, tick) {
                    let actors: Vec<_> =
                        actors.iter().cloned().map(NativeWeldWorldActor::HailChild).collect();
                    weld_visual_rows::compare(
                        actors.iter().map(|actor| NativeWeldPresentationState::World(actor.view())),
                        sample,
                        "Hail contact compositor",
                    );
                }
            }
        }
    }
}
