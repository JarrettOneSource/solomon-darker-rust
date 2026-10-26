//! The stock 60-kind planner from `native-secondary-presentation.ts`. Renderer
//! pooling is unnecessary here: each result is a self-contained pure draw plan.

use super::bursts::*;
use super::draws::*;
use super::fields::{magic_circle_ring_draws_at_age, prismatic_wave_draws_at_age};
use super::golem::{
    native_golem_death_presentation_plan_at_age, native_golem_presentation_plan_at_age,
};
use super::weather::{
    freeze_wave_visual_draws_at_age, storm_auxiliary_draws_at_age, storm_cloud_draws_at_age,
    storm_weather_composite_at_age,
};
use super::*;
use crate::enemies::presentation::{
    NativeEnemyProjectileKind, NativeEnemyProjectilePayload, NativeEnemyProjectileVisual,
    native_death_magic_layers, native_enemy_projectile_plan,
};
use crate::float;
use crate::float::{fround, hypot};
use crate::secondary::NativeSecondaryActorKind as Kind;
use crate::secondary::SecondaryActorRead;
use crate::secondary::leviathan::{
    native_leviathan_appendage_local_root, native_leviathan_appendage_record,
};
use crate::spells::ether_presentation::ether_primary_compositor_plan;
use crate::spells::fire_presentation::{native_fire_ember_plan, native_fire_explosion_plan};
use crate::spells::model::FireExplosionPresentation;
use crate::spells::presentation::{NativeSpellSprite, SpellAtlas, SpellBlend};
use crate::{NativeRng, Vec2};

#[must_use]
pub fn native_secondary_presentation_plan(
    actor: &impl SecondaryActorRead,
    frame: f64,
    point_gain: f64,
) -> NativeSecondaryPresentationPlan {
    native_secondary_presentation_plan_at_age(
        actor,
        actor.age_ticks as f64,
        frame,
        point_gain,
        actor.enhanced,
    )
}

/// Samples the same source planner at a display age. Continuous geometry keeps
/// the fraction; source particle reconstruction still truncates complete steps.
/// Neither the actor nor its captured gameplay RNG is changed.
/// The live quality argument is independent of retained actor birth fields;
/// the ordinary snapshot wrapper uses Source's `actor.enhanced` default.
#[must_use]
pub fn native_secondary_presentation_plan_at_age(
    actor: &impl SecondaryActorRead,
    age_ticks: f64,
    frame: f64,
    point_gain: f64,
    enhanced_effects: bool,
) -> NativeSecondaryPresentationPlan {
    let whole_age_ticks = age_ticks.trunc().max(0.0) as u64;
    let mut plan = NativeSecondaryPresentationPlan::new(actor.position);
    let make = |atlas, entry| NativeSecondarySpriteDraw::new(actor, atlas, entry);
    use NativeSecondaryAtlas::{BadGuys, DeadHawg};
    use NativeSecondaryBlend::Add;
    match actor.kind {
        Kind::GolemKnockback => {}
        Kind::PlanewalkerMote => {
            let mut draw = make(BadGuys, 11);
            draw.blend = Add;
            draw.role = "planewalker-mote".into();
            draw.rotation_radians = actor.rotation_radians;
            draw.tint = 0x8080ff;
            draw.scale_y = actor.scale * 0.75;
            plan.draws.push(draw);
        }
        Kind::StoneskinChip => {
            let mut chip = make(BadGuys, 77);
            chip.alpha = actor.alpha.min(1.0);
            chip.offset = Vec2::new(0.0, actor.phase);
            chip.rotation_radians = actor.rotation_radians;
            chip.scale_x = 1.0;
            chip.scale_y = 1.0;
            chip.role = "stoneskin-hit-chip".into();
            if actor.enhanced {
                let mut shadow = chip.clone();
                shadow.offset.y = 2.0;
                shadow.scale_y = 0.75;
                shadow.tint = 0;
                shadow.role = "stoneskin-hit-shadow".into();
                plan.background_draws.push(shadow);
            }
            plan.background_draws.push(chip);
            plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
        }
        Kind::GolemAssemblyDebris => {
            let mut rock = make(BadGuys, actor.frame as usize);
            rock.alpha = actor.alpha.min(1.0);
            rock.offset = Vec2::new(0.0, actor.phase);
            rock.rotation_radians = actor.rotation_radians;
            rock.role = "golem-assembly-rock".into();
            if actor.enhanced && actor.phase != 0.0 {
                let mut shadow = rock.clone();
                shadow.offset.y = 2.0;
                shadow.scale_y = actor.scale * 0.75;
                shadow.tint = 0;
                shadow.role = "golem-assembly-shadow".into();
                plan.draws.push(shadow);
            }
            plan.draws.push(rock);
            plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
            plan.sort_bias = -15.0;
        }
        Kind::Leviathan => {
            let mut galaxy = make(BadGuys, 75);
            galaxy.alpha = 1.0;
            galaxy.blend = Add;
            galaxy.role = "leviathan-plane-galaxy".into();
            galaxy.rotation_radians = degrees_to_radians((frame % 120.0) * 3.0);
            galaxy.scale_x = -0.8 * actor.scale;
            galaxy.scale_y = 0.64 * actor.scale;
            galaxy.tint = 0xff80ff;
            let mut shimmer = make(BadGuys, 38);
            shimmer.alpha = 1.0;
            shimmer.role = "leviathan-plane-shimmer".into();
            plan.draws.extend([galaxy, shimmer]);
            plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
            plan.sort_bias = -0.11;
        }
        Kind::LeviathanAppendage => {
            let heading = actor.rotation_radians * 180.0 / std::f64::consts::PI;
            let local = native_leviathan_appendage_local_root(
                actor.endpoint,
                actor.midpoint,
                actor.velocity.x,
                actor.slow_factor,
            );
            let parent_y = actor.position.y - actor.scale * local.y;
            let mut draw =
                make(BadGuys, native_leviathan_appendage_record(actor.phase as u32, heading));
            draw.rotation_radians = degrees_to_radians(float::sin(actor.rotation_radians) * 5.0);
            draw.scale_x = actor.radius * actor.scale;
            draw.scale_y = draw.scale_x;
            plan.draws.push(draw);
            plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
            plan.sort_bias = parent_y + (actor.frame - 100.0) * 0.001 - actor.position.y;
        }
        Kind::LeviathanMote | Kind::PlaneOrbParticle => {
            let mut draw = make(
                BadGuys,
                if actor.kind == Kind::LeviathanMote { 11 } else { actor.variant as usize },
            );
            draw.blend = Add;
            draw.rotation_radians = actor.rotation_radians;
            draw.scale_y = actor.scale * 0.8;
            draw.tint = pack_normalized_rgb(1.0, actor.quantity, 1.0);
            plan.draws.push(draw);
        }
        Kind::EtherBolt => {
            let mut draw = make(BadGuys, 22);
            draw.alpha = 0.5 + 0.5 * hash_unit(actor.id, frame.floor() as u64);
            draw.blend = Add;
            draw.offset = Vec2::new(0.0, -25.0);
            draw.rotation_radians = actor.rotation_radians;
            draw.scale_x = 1.0;
            draw.scale_y = 1.0;
            plan.draws.push(draw);
        }
        Kind::EtherFade => {
            let fade = native_ether_fade_scalar(actor.alpha, actor.slow_factor, frame);
            let source = ether_primary_compositor_plan(
                actor.id,
                (actor.quantity + frame).floor() as u64,
                actor.quantity + frame,
                actor.scale,
                fade,
            );
            let mut pass = 0;
            let mut index = 0;
            for draw in source.draws {
                let role = match draw.record {
                    110 if index == 0 => "outer-core",
                    110 => "inner-core",
                    111 if index == 2 => "fixed-spark",
                    111 => "radial-spark",
                    112 => "ray",
                    _ => unreachable!("closed Ether compositor"),
                };
                let converted = spell_draw(draw, format!("ether-fade-{pass}-{role}"));
                if converted.entry == 112 {
                    pass += 1;
                    index = 0;
                } else {
                    index += 1;
                }
                plan.draws.push(converted);
            }
            plan.sort_bias = if actor.variant == 1 { 100.0 } else { 0.0 };
        }
        Kind::PlaneOrbShot => {
            let mut draw = make(BadGuys, 75);
            draw.blend = Add;
            draw.rotation_radians = degrees_to_radians((frame % 360.0) * 1.5);
            draw.scale_x = -0.75 * actor.scale;
            draw.scale_y = 0.6 * actor.scale;
            plan.draws.push(draw);
            plan.meshes.push(plane_orb_mesh(actor, frame, enhanced_effects));
        }
        Kind::PhaseBurst => {
            let mut draw = make(BadGuys, 53);
            draw.alpha = actor.alpha.min(1.0);
            draw.blend = Add;
            draw.rotation_radians = actor.rotation_radians + std::f64::consts::PI / 2.0;
            plan.draws.push(draw);
            plan.sort_bias = 15.0;
        }
        Kind::MovingFire | Kind::FirePatch => {
            let scale = FIRE_DRAW_SCALE * actor.scale * actor.radius;
            let mut draw = make(DeadHawg, clamp_entry(actor.frame, 46, 77));
            draw.alpha = (actor.alpha * actor.slow_factor).min(1.0);
            draw.blend = Add;
            draw.offset = Vec2::new(0.0, -20.0);
            draw.scale_x = scale * actor.quantity;
            draw.scale_y = scale;
            plan.draws.push(draw);
            plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
        }
        Kind::FireBurnFlame | Kind::EtherBurnFlare => {
            let range = if actor.kind == Kind::FireBurnFlame { (333, 342) } else { (246, 250) };
            let mut draw = make(BadGuys, clamp_entry(actor.frame, range.0, range.1));
            draw.blend = Add;
            plan.draws.push(draw);
        }
        Kind::FireBurn
        | Kind::EtherBurn
        | Kind::Shockwave
        | Kind::MindblastShockwave
        | Kind::RescueShockwave
        | Kind::FreezeWave
        | Kind::IceBlast
        | Kind::EarthquakeSceneryWobble
        | Kind::ElectricBurn => {}
        Kind::MindblastBurst => {
            plan.draws = mindblast_burst_draws_at_age(actor, whole_age_ticks);
        }
        Kind::StormCloud => {
            plan.draws = storm_cloud_draws_at_age(actor, age_ticks, frame, enhanced_effects);
            plan.draws.extend(storm_auxiliary_draws_at_age(actor, age_ticks));
            plan.storm_composite = storm_weather_composite_at_age(actor, age_ticks);
            plan.world_y = actor.position.y + 350.0;
        }
        Kind::StormDrop | Kind::AcidDrop => {
            if actor.phase < 0.0 {
                let acid = actor.kind == Kind::AcidDrop;
                if acid {
                    let mut draw = make(BadGuys, 0);
                    draw.alpha = 0.25;
                    draw.role = "acid-raindrop-falling".into();
                    draw.scale_x = 1.0;
                    draw.scale_y = 1.0;
                    draw.tint = 0xb3f2bf;
                    plan.draws.push(draw);
                }
                plan.gradients.push(raindrop_gradient(actor, acid));
            } else {
                let mut draw = make(BadGuys, 63);
                draw.alpha = (1.0 - actor.scale * actor.scale).max(0.0);
                draw.role = if actor.kind == Kind::AcidDrop {
                    "acid-raindrop-ground"
                } else {
                    "storm-raindrop-ground"
                }
                .into();
                draw.tint = if actor.kind == Kind::AcidDrop { 0xccffcc } else { 0xccffff };
                plan.draws.push(draw);
            }
        }
        Kind::StormStrike | Kind::ElectricBurnArc => {
            plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic
        }
        Kind::ElectricBurnFlare => {
            let corona = crate::spells::air::build_native_air_corona_plan(
                crate::spells::air::NativeAirCoronaInput {
                    alpha: actor.alpha,
                    angle: actor.rotation_radians,
                    center: actor.position,
                    random_salt: 0x46414445 ^ actor.age_ticks as u32,
                    scale: actor.scale,
                    seed: actor.id,
                },
            );
            for sprite in corona.draws {
                let mut draw = make(BadGuys, sprite.record);
                draw.alpha = actor.alpha * sprite.alpha;
                draw.blend = Add;
                draw.scale_x = sprite.scale.x;
                draw.scale_y = sprite.scale.y;
                draw.rotation_radians = sprite.rotation;
                draw.tint = sprite.tint;
                draw.role = if sprite.record == 110 {
                    "electric-burn-corona"
                } else {
                    "electric-burn-fork"
                }
                .into();
                plan.draws.push(draw);
            }
            plan.sort_bias = 50.0;
        }
        Kind::PrismaticWave => {
            plan.draws = prismatic_wave_draws_at_age(actor, whole_age_ticks, frame);
        }
        Kind::FreezeWaveVisual => {
            let visual = freeze_wave_visual_draws_at_age(actor, whole_age_ticks);
            plan.background_draws = visual.background_draws;
            plan.draws = visual.draws;
            plan.underlay_draws = visual.underlay_draws;
        }
        Kind::FrostBurnFlare => {
            let mut draw = make(BadGuys, clamp_entry(actor.frame, 10, 11));
            draw.blend = Add;
            draw.rotation_radians = actor.rotation_radians;
            draw.tint = actor.quantity.trunc() as u32;
            plan.draws.push(draw);
        }
        Kind::FlashResponseGrow | Kind::FlashResponseFade => {
            let mut draw =
                make(BadGuys, if actor.kind == Kind::FlashResponseGrow { 16 } else { 15 });
            draw.alpha = actor.alpha.min(1.0);
            draw.blend = Add;
            draw.role = if actor.kind == Kind::FlashResponseGrow {
                "flash-response-grow-perspective"
            } else {
                "flash-response-fade"
            }
            .into();
            if actor.kind == Kind::FlashResponseGrow {
                draw.scale_y = fround(actor.scale * (0.8_f32 as f64));
            }
            plan.draws.push(draw);
        }
        Kind::Earthquake => {
            for (entry, angle) in [(200, 0.0), (201, 170.0), (202, 305.0)] {
                if entry == 201 && actor.phase <= 0.6 || entry == 202 && actor.phase <= 3.0 {
                    continue;
                }
                let mut draw = make(DeadHawg, entry);
                draw.alpha = actor.alpha * 0.75;
                draw.rotation_radians = actor.rotation_radians + degrees_to_radians(angle);
                draw.scale_x = 1.5;
                draw.scale_y = 1.2;
                plan.draws.push(draw);
            }
            if actor.quantity > 0.0 {
                for index in 0..plan.draws.len() {
                    let mut draw = plan.draws[index].clone();
                    draw.alpha *= actor.quantity.min(1.0);
                    draw.tint = 0x00ff00;
                    plan.draws.push(draw);
                }
            }
            plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
        }
        Kind::EarthquakeQuake => {
            let mut draw = make(BadGuys, 62);
            draw.alpha = (actor.alpha * float::sin(degrees_to_radians(actor.phase)).abs()).min(1.0);
            draw.role = "earthquake-quake-child".into();
            draw.rotation_radians = actor.rotation_radians;
            draw.scale_y = actor.slow_factor;
            plan.draws.push(draw);
        }
        Kind::EarthquakeDust => {
            let mut draw = make(BadGuys, 10);
            draw.alpha = actor.alpha.min(1.0);
            draw.role = "earthquake-scenery-dust".into();
            draw.rotation_radians = actor.rotation_radians;
            draw.tint = 0x1e1100;
            plan.underlay_draws.push(draw);
        }
        Kind::EarthquakeDebris => {
            if actor.enhanced {
                let mut draw = make(BadGuys, (2008 + actor.variant) as usize);
                draw.alpha = actor.alpha.min(1.0);
                draw.offset = Vec2::new(1.0, actor.phase + 2.0);
                draw.role = "earthquake-boulder-dark-underlay".into();
                draw.rotation_radians = actor.rotation_radians;
                draw.scale_x = actor.scale * 0.75;
                draw.scale_y = draw.scale_x;
                draw.tint = 0;
                plan.draws.push(draw);
            }
            let mut draw = make(BadGuys, (2008 + actor.variant) as usize);
            draw.alpha = actor.alpha.min(1.0);
            draw.offset = Vec2::new(0.0, actor.phase);
            draw.role = "earthquake-boulder-bit".into();
            draw.rotation_radians = actor.rotation_radians;
            plan.draws.push(draw);
            plan.sort_bias = -15.0;
        }
        Kind::Golem => return native_golem_presentation_plan_at_age(actor, age_ticks, frame),
        Kind::GolemDeath => return native_golem_death_presentation_plan_at_age(actor, age_ticks),
        Kind::TeleportBurst => {
            let mut draw = make(BadGuys, 90);
            draw.alpha = actor.alpha.min(1.0);
            draw.blend = Add;
            draw.rotation_radians = actor.rotation_radians;
            plan.draws.push(draw);
        }
        Kind::MagicCircle => {
            plan.draws = magic_circle_ring_draws_at_age(actor, whole_age_ticks);
        }
        Kind::MagicCirclePlayerFlash => {
            let mut draw = make(BadGuys, 7);
            draw.alpha = actor.alpha.min(1.0);
            draw.blend = Add;
            draw.role = "magic-circle-player-recovery-pulse".into();
            draw.rotation_radians = actor.rotation_radians;
            draw.tint = 0x80ffff;
            plan.draws.push(draw);
        }
        Kind::MagicTrap => plan.draws = magic_trap_draws(actor, age_ticks),
        Kind::MagicTrapShimmer => {
            let mut draw = make(BadGuys, 16);
            draw.role = "magic-trap-shimmer".into();
            draw.rotation_radians = actor.rotation_radians;
            draw.scale_y = fround(actor.scale * (0.8_f32 as f64));
            draw.tint = magic_trap_tint(actor.variant);
            plan.draws.push(draw);
        }
        Kind::MagicTrapBurst => {
            plan.draws = magic_trap_burst_draws_at_age(actor, whole_age_ticks);
        }
        Kind::DampenWave => plan.draws = dampen_draws_at_age(actor, whole_age_ticks),
        Kind::DampenedSmoke => {
            let mut draw = make(BadGuys, actor.frame as usize);
            draw.alpha = actor.alpha * actor.phase;
            draw.blend = if actor.variant == 3 { NativeSecondaryBlend::Normal } else { Add };
            draw.role = "dampened-smoke".into();
            draw.rotation_radians = actor.rotation_radians;
            draw.tint = actor.quantity as u32;
            plan.draws.push(draw);
        }
        Kind::DampenedProjectile => plan.draws = dampened_projectile_draws(actor, age_ticks),
        Kind::ShieldBreak => {
            let mut draw = make(BadGuys, 68);
            draw.blend = Add;
            draw.rotation_radians = actor.rotation_radians;
            plan.draws.push(draw);
        }
        Kind::ShieldExplosion => {
            plan.draws = shield_explosion_draws_at_age(actor, whole_age_ticks);
        }
        Kind::RingFireExplosion => {
            let source = native_fire_explosion_plan(
                age_ticks,
                actor.position,
                FireExplosionPresentation::Fire,
                actor.scale,
                point_gain,
            );
            for draw in source.draws {
                let role = if draw.record == 15 {
                    "explosion-core"
                } else if draw.record < 420 {
                    "explosion-array"
                } else {
                    "explosion-lit-array"
                };
                plan.draws.push(spell_draw(draw, role.into()));
            }
        }
        Kind::RingFireFragment => {
            let source = native_fire_ember_plan(
                actor.id,
                actor.phase,
                actor.alpha,
                actor.frame,
                actor.position,
                frame.floor() as u64,
                enhanced_effects,
            );
            let roles: &[&str] = if enhanced_effects {
                &["body", "additive-body-1", "additive-body-2", "glow"]
            } else {
                &["body", "additive-body-1", "glow"]
            };
            for (draw, &role) in source.draws.into_iter().zip(roles) {
                plan.draws.push(spell_draw(draw, role.into()));
            }
            plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
            if enhanced_effects && actor.phase < 0.0 {
                let width = fround(38.0 * 0.75);
                let height = fround(37.0 * (0.6_f32 as f64));
                plan.quads.push(NativeSecondaryQuadDraw {
                    alpha: fround((1.0 - actor.phase / -50.0 * 0.5) * 0.25),
                    atlas: Some(BadGuys),
                    blend: NativeSecondaryBlend::Normal,
                    entry: Some(15),
                    role: "ring-fire-fragment-enhanced-ground-glow".into(),
                    tint: 0xff8040,
                    vertices: [
                        -width / 2.0,
                        -height / 2.0,
                        width / 2.0,
                        -height / 2.0,
                        -width / 2.0,
                        height / 2.0,
                        width / 2.0,
                        height / 2.0,
                    ],
                });
            }
        }
        Kind::AcidRain => return acid_rain_plan(actor, age_ticks),
        Kind::AcidSplash => {
            let mut draw = make(BadGuys, 10);
            draw.alpha = (actor.alpha / 0.25).min(1.0);
            draw.blend = Add;
            draw.role = "acid-rain-splash".into();
            draw.rotation_radians = actor.rotation_radians;
            draw.tint = 0x80ff80;
            plan.draws.push(draw);
        }
        Kind::EtherDrain => plan.draws = ether_drain_draws(actor, frame),
        Kind::EtherDrainCloud => {
            let mut draw = make(BadGuys, (10 + actor.variant) as usize);
            draw.alpha = (float::sin(degrees_to_radians(actor.phase)) * actor.alpha).max(0.0);
            draw.blend = Add;
            draw.role = "ether-drain-suck-cloud".into();
            draw.rotation_radians = actor.rotation_radians;
            plan.draws.push(draw);
        }
        Kind::EtherDrainDebris => {
            let mut draw = make(DeadHawg, (177 + actor.variant) as usize);
            draw.role = "ether-drain-suck-debris".into();
            draw.rotation_radians = actor.rotation_radians;
            draw.scale_x = 1.0;
            draw.scale_y = 1.0;
            plan.draws.push(draw);
        }
        Kind::EtherDrainCaptureFlare => {
            let mut draw = make(BadGuys, 36);
            draw.blend = Add;
            draw.role = "ether-drain-capture-flare".into();
            plan.draws.push(draw);
        }
        Kind::Comet => {
            let distance = (400.0 - age_ticks) * 20.0 + 70.0;
            let mut draw = make(DeadHawg, 5);
            draw.offset = Vec2::new(
                float::cos(actor.rotation_radians) * distance,
                -float::sin(actor.rotation_radians) * distance,
            );
            draw.role = "comet-body".into();
            draw.rotation_radians = actor.rotation_radians + degrees_to_radians(150.0);
            draw.scale_x = 2.0;
            draw.scale_y = 2.0;
            plan.draws.push(draw);
            plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
        }
        Kind::CometTrail => {
            let mut draw = make(BadGuys, 51);
            draw.alpha = (actor.phase / 0.5).min(1.0);
            draw.blend = Add;
            draw.role = "comet-trail".into();
            draw.rotation_radians = actor.rotation_radians;
            draw.tint = pack_gray(actor.alpha);
            plan.draws.push(draw);
        }
        Kind::CometImpact => {
            let additive = (5.0 - age_ticks * 0.01).max(0.0);
            let ring = (10.0 - age_ticks * 0.01).max(0.0);
            if additive > 0.0 {
                let mut draw = make(BadGuys, 15);
                draw.alpha = additive.min(1.0);
                draw.blend = Add;
                draw.role = "comet-impact-additive".into();
                draw.scale_x = 10.0;
                draw.scale_y = 10.0;
                draw.tint = 0xbfbfbf;
                plan.draws.push(draw);
            }
            if ring > 0.0 {
                let mut draw = make(DeadHawg, 6);
                draw.alpha = ring.min(1.0);
                draw.role = "comet-impact-ring".into();
                draw.scale_x = 2.0;
                draw.scale_y = 2.0;
                plan.draws.push(draw);
            }
        }
        Kind::CometDebris => {
            let mut draw = make(DeadHawg, (203 + actor.variant) as usize);
            draw.alpha = actor.alpha.min(1.0);
            draw.offset = Vec2::new(0.0, actor.phase);
            draw.role = "comet-impact-debris".into();
            draw.rotation_radians = actor.rotation_radians;
            plan.draws.push(draw);
        }
        Kind::TurnUndead => {
            let mut draw = make(BadGuys, 48);
            draw.rotation_radians = actor.rotation_radians;
            draw.scale_y = actor.scale * 0.8;
            draw.tint = 0x808080;
            plan.draws.push(draw);
        }
    }
    plan
}

fn spell_draw(draw: NativeSpellSprite, role: String) -> NativeSecondarySpriteDraw {
    NativeSecondarySpriteDraw {
        alpha: draw.alpha,
        atlas: match draw.atlas {
            SpellAtlas::BadGuys => NativeSecondaryAtlas::BadGuys,
            SpellAtlas::DeadHawg => NativeSecondaryAtlas::DeadHawg,
        },
        blend: match draw.blend {
            SpellBlend::Normal => NativeSecondaryBlend::Normal,
            SpellBlend::Add => NativeSecondaryBlend::Add,
        },
        color_mode: None,
        entry: draw.record,
        offset: draw.offset,
        role,
        rotation_radians: draw.rotation,
        scale_x: draw.scale.x,
        scale_y: draw.scale.y,
        tint: draw.tint,
    }
}

fn dampened_projectile_draws(
    actor: &impl SecondaryActorRead,
    age_ticks: f64,
) -> Vec<NativeSecondarySpriteDraw> {
    let layers = if actor.variant == 3 {
        native_death_magic_layers(
            Vec2::ZERO,
            1.25,
            actor.phase,
            &mut NativeRng::new((actor.id + age_ticks.trunc() as u64) as i64),
        )
    } else {
        native_enemy_projectile_plan(
            &NativeEnemyProjectileVisual {
                age_ticks: 0,
                heading_degrees: actor.rotation_radians * 180.0 / std::f64::consts::PI,
                id: actor.target_id.map_or(actor.id, |id| id as u64),
                kind: if actor.variant == 0 {
                    NativeEnemyProjectileKind::Firebolt
                } else {
                    NativeEnemyProjectileKind::GuidedMissile
                },
                lifetime_ticks: 400,
                payload: match actor.variant {
                    1 => NativeEnemyProjectilePayload::Poison,
                    2 => NativeEnemyProjectilePayload::Cold,
                    _ => NativeEnemyProjectilePayload::Fire,
                },
                position: Vec2::ZERO,
                speed: hypot(actor.velocity.x, actor.velocity.y),
                vertical_offset: 0.0,
                visual_phase_degrees: actor.phase,
                visual_scale: 1.0,
            },
            age_ticks,
        )
        .layers
    };
    let mut draws = Vec::new();
    for layer in layers {
        let atlas = match layer.atlas {
            SpellAtlas::BadGuys => NativeSecondaryAtlas::BadGuys,
            SpellAtlas::DeadHawg => NativeSecondaryAtlas::DeadHawg,
        };
        let mut draw = NativeSecondarySpriteDraw::new(actor, atlas, layer.entry);
        draw.alpha = layer.alpha;
        draw.blend = match layer.blend {
            SpellBlend::Normal => NativeSecondaryBlend::Normal,
            SpellBlend::Add => NativeSecondaryBlend::Add,
        };
        draw.offset = layer.offset;
        draw.role = format!(
            "{}{}",
            if actor.variant == 3 { "dampened-" } else { "dampened-projectile-" },
            layer.role
        );
        draw.rotation_radians = layer.rotation_radians;
        draw.scale_x = layer.scale;
        draw.scale_y = layer.scale_y.unwrap_or(actor.scale);
        draw.tint = layer.tint;
        draws.push(draw);
    }
    draws
}

fn magic_trap_draws(
    actor: &impl SecondaryActorRead,
    age_ticks: f64,
) -> Vec<NativeSecondarySpriteDraw> {
    let radians = degrees_to_radians(age_ticks);
    let sine = float::sin(radians);
    let bob = fround(5.0 * sine - 12.0);
    let multiplier = if actor.scale < MAGIC_TRAP_FULL_DRAW_THRESHOLD { 0.75 } else { 1.0 };
    let scale = fround(fround(0.5 + fround(0.5 * actor.scale)) * multiplier);
    let halo = fround(fround(0.5 - fround(0.125 * sine)) * multiplier);
    let selector = fround(fround(0.375 - fround(0.125 * sine)) * multiplier);
    let mut shadow = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 15);
    shadow.alpha = 0.5;
    shadow.role = "magic-trap-shadow".into();
    shadow.scale_x = 0.75;
    shadow.scale_y = 0.75;
    shadow.tint = 0;
    let mut draws = vec![shadow];
    for (entry, role, rotation) in
        [(111, "magic-trap-clockwise-halo", 2.0), (112, "magic-trap-counterclockwise-halo", -3.0)]
    {
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, entry);
        draw.alpha = halo;
        draw.blend = NativeSecondaryBlend::Add;
        draw.offset = Vec2::new(0.0, bob);
        draw.role = role.into();
        draw.rotation_radians = degrees_to_radians(age_ticks * rotation);
        draw.scale_x = scale;
        draw.scale_y = fround(scale * (0.8_f32 as f64));
        draws.push(draw);
    }
    let mut glow = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 15);
    glow.alpha = selector;
    glow.blend = NativeSecondaryBlend::Add;
    glow.role = "magic-trap-selector-glow".into();
    glow.scale_x = 2.0;
    glow.scale_y = 2.0;
    glow.tint = magic_trap_tint(actor.variant);
    draws.push(glow);
    let mut body = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 85);
    body.alpha = 1.0;
    body.offset = Vec2::new(0.0, bob);
    body.role = "magic-trap-body".into();
    body.scale_x = fround(1.0 - fround(0.1 * float::sin(radians * 2.0)));
    body.scale_y = fround(1.0 + fround(0.1 * float::cos(radians)));
    draws.push(body);
    draws
}

fn acid_rain_plan(
    actor: &impl SecondaryActorRead,
    age_ticks: f64,
) -> NativeSecondaryPresentationPlan {
    let scale = fround(actor.scale);
    let alpha = fround(actor.phase).clamp(0.0, 1.0);
    let phase = fround(actor.rotation_radians);
    let age = fround(age_ticks);
    let first = fround(scale * 5.0);
    let second = fround(fround(scale * 3.75) * 2.0);
    let mut plan = NativeSecondaryPresentationPlan::new(actor.position);
    if alpha > 0.0 {
        let mut cloud = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 78);
        cloud.alpha = fround(alpha * 0.75);
        cloud.offset = Vec2::new(0.0, -175.0);
        cloud.role = "acid-rain-cloud-mottled-source-over".into();
        cloud.rotation_radians = degrees_to_radians(fround(fround(age * 0.03125) * phase));
        cloud.scale_x = first;
        cloud.scale_y = fround(first * (0.8_f32 as f64));
        cloud.tint = 0x698c52;
        let mut additive = cloud.clone();
        additive.blend = NativeSecondaryBlend::Add;
        additive.role = "acid-rain-cloud-mottled-additive".into();
        plan.draws.extend([cloud, additive]);
        let mut circle = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 10);
        circle.alpha = alpha;
        circle.blend = NativeSecondaryBlend::Add;
        circle.offset = Vec2::new(0.0, fround(-175.0 + scale * -50.0));
        circle.role = "acid-rain-cloud-circle-additive".into();
        circle.rotation_radians = degrees_to_radians(fround(age * -0.5));
        // Source keeps the center broad even when its random phase is near zero.
        circle.scale_x = second;
        circle.scale_y = fround(second * (0.8_f32 as f64));
        circle.tint = 0x407326;
        plan.draws.push(circle);
    }
    if actor.alpha > 0.0 {
        let mut residue = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::DeadHawg, 4);
        residue.alpha = actor.alpha.clamp(0.0, 1.0);
        residue.role = "acid-rain-ground-residue".into();
        residue.scale_x = 4.5;
        residue.scale_y = 4.5;
        residue.tint = 0x0d1a0d;
        plan.underlay_draws.push(residue);
    }
    plan.world_y = actor.position.y + 350.0;
    plan
}

fn ether_drain_draws(
    actor: &impl SecondaryActorRead,
    frame: f64,
) -> Vec<NativeSecondarySpriteDraw> {
    let mut draws = Vec::new();
    for (role, alpha, y, rotation, x_scale, y_scale, tint) in [
        (
            "ether-drain-galaxy-near",
            1.0,
            0.0,
            actor.rotation_radians * 1.5,
            actor.scale * -0.8,
            actor.scale * 0.64,
            0xff80ff,
        ),
        (
            "ether-drain-galaxy-middle-near",
            actor.alpha * 0.5,
            -5.0,
            actor.rotation_radians * 0.5,
            -1.5,
            1.2,
            WHITE,
        ),
        (
            "ether-drain-galaxy-middle-far",
            actor.alpha * 0.25,
            -10.0,
            actor.rotation_radians * 0.25,
            -2.5,
            2.0,
            WHITE,
        ),
        (
            "ether-drain-galaxy-far",
            actor.alpha * 0.1,
            -20.0,
            actor.rotation_radians * 0.125,
            -4.5,
            3.6,
            WHITE,
        ),
    ] {
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 75);
        draw.alpha = alpha;
        draw.blend = NativeSecondaryBlend::Add;
        draw.offset = Vec2::new(0.0, y);
        draw.role = role.into();
        draw.rotation_radians = rotation;
        draw.scale_x = x_scale;
        draw.scale_y = y_scale;
        draw.tint = tint;
        draws.push(draw);
    }
    let scale = actor.scale
        * 0.25
        * ((0.98_f32 as f64) + hash_unit(actor.id, frame.trunc() as u64) * 0.06999993324279785);
    let mut shimmer = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 38);
    shimmer.alpha = 1.0;
    shimmer.role = "ether-drain-shimmer".into();
    shimmer.scale_x = scale;
    shimmer.scale_y = scale;
    shimmer.tint = 0xff4080;
    draws.push(shimmer);
    if actor.slow_factor > 0.0 {
        let mut pulse = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 38);
        pulse.alpha = actor.slow_factor;
        pulse.role = "ether-drain-capture-pulse".into();
        pulse.scale_x = actor.slow_factor * actor.scale;
        pulse.scale_y = pulse.scale_x;
        draws.push(pulse);
    }
    draws
}
