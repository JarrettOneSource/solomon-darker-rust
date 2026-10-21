use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::lighting::native_boss_spell_light;
use solomon_sim::enemies::boss_spell::{
    BossSpellKind, BossSpellLight, BossSpellOwner, NativeBossSpell,
};
use solomon_sim::enemies::faculty_spells::NativeDireFireState;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::spells::fire::{CreateNativeFirePatchOptions, create_native_fire_patch};
use solomon_sim::spells::model::FirePatchType;

#[path = "support/website_rows.rs"]
mod website_rows;

// The factory reads only age, alpha, light and identity. Other canonical payload
// fields are inert here; full actor births and ticks have their own source replay.
fn spell(value: &Value) -> NativeBossSpell {
    let alpha = value["alpha"].as_f64().unwrap();
    let kind = match value["kind"].as_str().unwrap() {
        "ultra-banish" => BossSpellKind::UltraBanish {
            remaining_ticks: 0,
            alpha,
            flash_alpha: 0.0,
            light_radius: value["lightRadius"].as_f64().unwrap(),
            mega_death: false,
        },
        "unholy-soul" => BossSpellKind::UnholySoul {
            origin: Vec2::ZERO,
            angle_deg: 0.0,
            angular_speed: 0.0,
            scale: 1.0,
            height: 0.0,
            vertical_speed: 0.0,
            life: 1.0,
            fast: false,
        },
        "eye-laser" => {
            BossSpellKind::EyeLaser { heading_deg: 0.0, phase_deg: 0.0, velocity: Vec2::ZERO }
        }
        "unholy-spit" => BossSpellKind::UnholySpit {
            origin: Vec2::ZERO,
            travel: Vec2::ZERO,
            spread_direction: Vec2::ZERO,
            progress: 0.0,
            progress_per_tick: 0.0,
            height: 0.0,
            spawn_fire: false,
            spawn_imps: false,
        },
        "green-fire" => {
            let mut fire = create_native_fire_patch(
                CreateNativeFirePatchOptions {
                    atlas_phase_step: None,
                    burn_damage: 0.0,
                    damage: 0.0,
                    draw_alpha: None,
                    fade_alpha: None,
                    id: 1,
                    life: None,
                    native_type: FirePatchType::Fire,
                    owner_id: "boss".into(),
                    painter_registration: None,
                    position: Vec2::ZERO,
                    scale: None,
                    supplemental_contact: false,
                    velocity: None,
                    velocity_multiplier: None,
                    world_key: "arena".into(),
                },
                0.0,
                1.0,
            );
            fire.life = value["fire"]["life"].as_f64().unwrap();
            BossSpellKind::GreenFire {
                fire: Box::new(fire),
                glow: value["glow"].as_bool().unwrap(),
            }
        }
        "unholy-burst" => BossSpellKind::UnholyBurst {
            frame_phase: 0.0,
            frame_velocity: 0.0,
            offset_y: 0.0,
            offset_step_y: 0.0,
            light_intensity: value["lightIntensity"].as_f64().unwrap(),
        },
        "mouth-beam-segment" => BossSpellKind::MouthBeamSegment {
            vertices: [Vec2::ZERO; 4],
            uv_offset: 0.0,
            start_alpha: 1.0,
            end_alpha: 1.0,
        },
        "heartmonger-flicker" => BossSpellKind::HeartmongerFlicker { phase_deg: 0.0 },
        "heartmonger-soul" => {
            BossSpellKind::HeartmongerSoul { life_phase_deg: 0.0, bob_phase_deg: 0.0 }
        }
        "blightning" => BossSpellKind::Blightning {
            endpoint: Vec2::ZERO,
            midpoint: Vec2::ZERO,
            enhanced_effects: true,
        },
        "death-magic" => BossSpellKind::DeathMagic {
            scale: 1.0,
            alpha,
            alpha_loss_per_tick: 0.0,
            painter_sort_bias: 0.0,
            light: value["light"].as_object().map(|_| BossSpellLight {
                intensity: value["light"]["intensity"].as_f64().unwrap(),
                radius: value["light"]["radius"].as_f64().unwrap(),
                loss_per_tick: 0.0,
            }),
        },
        "falling-bone" => {
            BossSpellKind::FallingBone { entry: 0, height: 0.0, color_ramp: 0.0, rotation_deg: 0.0 }
        }
        "skull-missile" => BossSpellKind::SkullMissile {
            heading_deg: 0.0,
            minimum_speed: 0.0,
            phase_deg: 0.0,
            remaining_ticks: 0.0,
            speed: 0.0,
            turn_rate: 0.0,
            visual_scale: 1.0,
            target_player_id: None,
        },
        "dark-fireball" => BossSpellKind::DarkFireball {
            arc_phase: 0.0,
            arc_phase_step: 0.0,
            heading_deg: 0.0,
            remaining_ticks: 0.0,
            velocity: Vec2::ZERO,
            ground_fire_damage: 0.0,
        },
        "rain-of-bones" => BossSpellKind::RainOfBones {
            alpha,
            phase: 0.0,
            remaining_ticks: 0,
            rotation_sign: 1.0,
            scale: 1.0,
        },
        "tragic-circle" => BossSpellKind::TragicCircle { remaining_ticks: 0 },
        "dire-fire" => BossSpellKind::DireFire(NativeDireFireState {
            alpha,
            glow: false,
            phase: 0.0,
            ramp: 0.0,
            rotation_phases: [0.0; 2],
            rotation_rates: [0.0; 2],
            rotation_signs: [1.0; 2],
            scale: 1.0,
            scale_sign: 1.0,
        }),
        kind => panic!("uncovered boss light kind {kind}"),
    };
    NativeBossSpell {
        owner: BossSpellOwner {
            age_ticks: value["ageTicks"].as_u64().unwrap(),
            damage: 0.0,
            id: value["id"].as_u64().unwrap() as usize,
            painter_registration: PainterRegistration {
                manager_lane: WorldManagerLane::Actor,
                registration_ordinal: 0,
            },
            owner_actor_id: 0,
            position: Vec2::new(
                value["position"]["x"].as_f64().unwrap(),
                value["position"]["y"].as_f64().unwrap(),
            ),
            spawn_tick: 0,
        },
        kind,
    }
}

#[test]
fn boss_light_providers_preserve_private_frame_rng_and_shadow_disposition() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/boss-light-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let spell = spell(&case["spell"]);
        let actual = native_boss_spell_light(
            &spell,
            case["presentationFrame"].as_u64().unwrap(),
            case["multipleShadows"].as_bool().unwrap(),
        )
        .map(|light| {
            json!({
                "castsDirectionalShadow": light.casts_directional_shadow,
                "intensity": light.intensity,
                "position": {"x": light.position.x, "y": light.position.y},
                "radius": light.radius,
            })
        });
        website_rows::compare(&json!(actual), &case["expected"], spell.kind.id());
    }
}
