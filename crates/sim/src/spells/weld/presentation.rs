//! Stock Weld compositors: the sprites, meshes and lines that draw each Weld projectile
//! and world actor, in draw order, including affine overlays and textured ribbons.

use super::*;
use crate::Vec2;
use crate::element_vfx::{ElementVfxBlend, element_vfx_plan_at_phase};
use crate::float;
use crate::float::{atan2, fround};
use crate::profile::{WeldBuildId, WizardElement};
use crate::spells::air::{AirCoronaInput, AirMesh, build_air_corona_plan};
use crate::spells::earth_presentation::{
    EarthBoulderPresentationState, earth_boulder_presentation_plan,
};
use crate::spells::fire_presentation::{FireballPresentationState, fireball_plan};
use crate::spells::model::PrimarySpellProjectilePhase;
use crate::spells::presentation::{SpellAtlas, SpellBlend, SpellSprite, pack_spell_rgb};
use crate::spells::water::{
    WaterFrostJetBlend, WaterFrostJetSprite, water_frost_jet_chaining_plan,
};

const DEG: f64 = std::f64::consts::PI / 180.0;

/// A 2D affine transform applied to a Weld sprite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeldAffineMatrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

/// One Weld sprite draw with an optional affine transform. A diffuse draw uses the sprite
/// only as an alpha mask, as a hit flash does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeldSpriteDraw {
    pub sprite: SpellSprite,
    pub matrix: Option<WeldAffineMatrix>,
    pub diffuse: bool,
}

impl WeldSpriteDraw {
    fn new(record: usize, scale: f64) -> Self {
        Self { sprite: SpellSprite::new(record, scale), matrix: None, diffuse: false }
    }
}

impl From<SpellSprite> for WeldSpriteDraw {
    fn from(sprite: SpellSprite) -> Self {
        Self { sprite, matrix: None, diffuse: false }
    }
}

/// Whether a Weld mesh's UVs are relative to its sprite record or absolute on the
/// BadGuys atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeldMeshUvSpace {
    Record,
    BadGuysAtlas,
}

/// A textured Weld mesh draw, such as a ribbon: mesh, record, UV space, vertex colors,
/// tint, alpha and blend.
#[derive(Clone, Debug, PartialEq)]
pub struct WeldMeshDraw {
    pub alpha: f64,
    pub blend: SpellBlend,
    pub colors: Option<Vec<u32>>,
    pub mesh: AirMesh,
    pub record: usize,
    pub tint: u32,
    pub uv_space: WeldMeshUvSpace,
}

/// A straight line in a Weld drawing: endpoints, width, color and alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeldLineDraw {
    pub alpha: f64,
    pub color: u32,
    pub end: Vec2,
    pub start: Vec2,
    pub width: f64,
}

/// Everything drawn for one Weld actor: sprites, ground underlays, meshes and lines, with
/// its painter Y, sort bias, split bounds and light sample point.
#[derive(Clone, Debug, PartialEq)]
pub struct WeldVisualPlan {
    pub lines: Vec<WeldLineDraw>,
    pub meshes: Vec<WeldMeshDraw>,
    pub position: Vec2,
    pub region_light_point: Option<Vec2>,
    pub sort_bias: f64,
    pub split_bounds: Option<FlameLashBounds>,
    pub sprites: Vec<WeldSpriteDraw>,
    pub underlays: Vec<WeldSpriteDraw>,
    pub world_y: f64,
}

impl WeldVisualPlan {
    fn at(position: Vec2) -> Self {
        Self {
            lines: Vec::new(),
            meshes: Vec::new(),
            position,
            region_light_point: None,
            sort_bias: 0.0,
            split_bounds: None,
            sprites: Vec::new(),
            underlays: Vec::new(),
            world_y: position.y,
        }
    }
}

/// The Weld state being drawn: a projectile or a world actor.
#[derive(Clone, Copy, Debug)]
pub enum WeldPresentationState<'a> {
    Projectile(&'a WeldProjectileState),
    World(&'a WeldWorldActor),
}

#[must_use]
pub fn weld_visual_plan(
    state: WeldPresentationState<'_>,
    frame: u64,
    puppet_hit: bool,
) -> WeldVisualPlan {
    let age = match state {
        WeldPresentationState::Projectile(actor) => actor.common.age_ticks,
        WeldPresentationState::World(actor) => actor.common().owner.age_ticks,
    };
    weld_visual_plan_at_age(state, frame, age as f64, puppet_hit)
}

/// Sample the same compositor with a received-snapshot fractional age.
#[must_use]
pub fn weld_visual_plan_at_age(
    state: WeldPresentationState<'_>,
    frame: u64,
    age: f64,
    puppet_hit: bool,
) -> WeldVisualPlan {
    let actor = match state {
        WeldPresentationState::Projectile(projectile) => {
            return projectile_plan(projectile, frame, age);
        }
        WeldPresentationState::World(actor) => actor,
    };
    match actor {
        WeldWorldActor::Channel(actor) => channel_plan(actor, age),
        WeldWorldActor::BlizzardGlow(actor) => {
            let mut plan = WeldVisualPlan::at(actor.common.origin);
            plan.sprites =
                lightning_core(actor.common.owner.id, frame, actor.scale, 1.0, frame as f64 * DEG);
            for draw in &mut plan.sprites {
                draw.sprite.rotation += actor.rotation_degrees * DEG;
            }
            plan
        }
        WeldWorldActor::BlizzardChainFrost(actor) => {
            let water = water_frost_jet_chaining_plan(
                actor.common.owner.id,
                actor.common.owner.age_ticks,
                actor.common.direction,
                actor.common.origin,
            );
            let mut plan = WeldVisualPlan::at(water.position);
            for draw in water.draws {
                plan.sprites.push(
                    SpellSprite {
                        alpha: draw.alpha,
                        blend: if draw.blend == WaterFrostJetBlend::Add {
                            SpellBlend::Add
                        } else {
                            SpellBlend::Normal
                        },
                        offset: Vec2::new(
                            fround(draw.position.x - water.position.x),
                            fround(draw.position.y - water.position.y),
                        ),
                        rotation: draw.rotation,
                        tint: pack_spell_rgb(draw.color[0], draw.color[1], draw.color[2]),
                        ..SpellSprite::new(
                            if draw.sprite == WaterFrostJetSprite::Core { 30 } else { 28 },
                            draw.scale,
                        )
                    }
                    .into(),
                );
            }
            plan
        }
        WeldWorldActor::FlameLashFade(actor) => {
            let mut plan = WeldVisualPlan::at(actor.position);
            if !matches!(actor.flame_visual, FlameFadeVisual::Birth(_)) || actor.alpha <= 0.0 {
                return plan;
            }
            let jitter = crate::random_domain::signed_random_float_from_words(
                visual_word(actor.common.owner.id, frame, 0x57370),
                visual_word(actor.common.owner.id, frame, 0x57371),
                0.25,
            );
            let scale = fround(actor.base_scale * fround(2.0 + jitter));
            plan.sort_bias =
                if actor.variant == WeldFlameLashFadeVariant::Chain { 50.0 } else { 100.0 };
            plan.sprites.push(
                SpellSprite {
                    alpha: actor.alpha,
                    blend: SpellBlend::Add,
                    ..SpellSprite::new(35, scale)
                }
                .into(),
            );
            plan
        }
        WeldWorldActor::GroundSparkFade(actor) => {
            let seed = actor.seed;
            let mut plan = WeldVisualPlan::at(seed.position);
            plan.sprites.push(
                SpellSprite {
                    alpha: seed.alpha,
                    rotation: seed.rotation_degrees * DEG,
                    ..SpellSprite::new(seed.record, seed.scale)
                }
                .into(),
            );
            plan
        }
        WeldWorldActor::FrostFade(actor) => {
            let mut plan = WeldVisualPlan::at(actor.common.origin);
            let fade_rate =
                if actor.common.build_id == WeldBuildId::BlizzardBeam { 0.4 } else { 0.05 };
            let alpha = (1.0 - age * fade_rate).max(0.0);
            plan.sprites =
                element_vfx_sprites(WizardElement::Water, frame as f64, actor.scale, alpha);
            plan
        }
        WeldWorldActor::HailChild(actor) => hail_child_plan(actor),
        WeldWorldActor::HailRockFade(actor) => {
            let mut plan = WeldVisualPlan::at(actor.position);
            plan.sprites.push(
                SpellSprite {
                    alpha: (4.0 - age * 0.01).max(0.0),
                    rotation: actor.rotation_degrees * DEG,
                    ..SpellSprite::new(18, 0.5)
                }
                .into(),
            );
            plan
        }
        WeldWorldActor::Impact(actor) => impact_plan(actor, frame, age),
        WeldWorldActor::Meteor(actor) => meteor_plan(actor, frame, puppet_hit),
        WeldWorldActor::MeteorFlash(actor) => {
            let mut plan = WeldVisualPlan::at(actor.position);
            plan.sprites.push(
                SpellSprite {
                    alpha: actor.alpha,
                    blend: SpellBlend::Add,
                    tint: 0xff8000,
                    ..SpellSprite::new(15, actor.scale)
                }
                .into(),
            );
            plan
        }
        WeldWorldActor::MeteorMarker(actor) => {
            let mut plan = WeldVisualPlan::at(actor.common.origin);
            plan.sprites.push(
                SpellSprite {
                    alpha: actor.alpha,
                    blend: SpellBlend::Add,
                    rotation: actor.rotation_degrees * DEG,
                    tint: 0xff0000 | ((actor.color_green * 255.0).round() as u32) << 8,
                    ..SpellSprite::new(51, actor.scale)
                }
                .into(),
            );
            plan
        }
        WeldWorldActor::BoulderDebris(actor) => debris_plan(actor),
        WeldWorldActor::Persistent(actor) => match actor {
            WeldPersistentActorState::EtherealBoulder(actor) => {
                ethereal_boulder_plan(actor, frame, age)
            }
            WeldPersistentActorState::Hailstones(actor) => hailstones_plan(actor, frame, age),
            WeldPersistentActorState::MeteorField(actor) => WeldVisualPlan::at(actor.common.origin),
        },
        WeldWorldActor::Steam(actor) => steam_plan(actor),
    }
}

fn projectile_plan(state: &WeldProjectileState, frame: u64, age: f64) -> WeldVisualPlan {
    let mut plan = WeldVisualPlan::at(state.common.position);
    plan.region_light_point = Some(state.common.position);
    let id = state.common.id;
    let alpha = if state.underpowered { 0.5 } else { 1.0 };
    match state.build_id {
        WeldBuildId::BurningBolt => {
            let fire = fireball_plan(
                FireballPresentationState {
                    age_ticks: age,
                    direction: state.common.direction,
                    id,
                    position: state.common.position,
                    underpowered: state.underpowered,
                },
                frame,
            );
            plan.sprites = fire.draws.into_iter().map(WeldSpriteDraw::from).collect();
        }
        WeldBuildId::FrostMissile => {
            plan.sprites.push(
                SpellSprite {
                    alpha: alpha * (0.75 + visual_unit(id, frame, 0) * 0.25),
                    rotation: age * DEG,
                    ..SpellSprite::new(271 + ((age / 4.0).floor() % 12.0) as usize, 1.7)
                }
                .into(),
            );
            plan.sprites.push(
                SpellSprite {
                    alpha: alpha * (0.2 + visual_unit(id, frame, 1) * 0.25),
                    blend: SpellBlend::Add,
                    rotation: visual_unit(id, frame, 2) * std::f64::consts::PI * 2.0,
                    tint: 0x80bfff,
                    ..SpellSprite::new(5, 1.0)
                }
                .into(),
            );
            let pulse = float::sin(age * 15.0 * DEG).abs() * 2.0;
            plan.sprites.push(
                SpellSprite {
                    alpha: alpha * (0.2 + visual_unit(id, frame, 3) * 0.25),
                    blend: SpellBlend::Add,
                    tint: 0xff80ff,
                    ..SpellSprite::new(110, pulse)
                }
                .into(),
            );
            for lane in
                state.frost_presentation_lanes.expect("Frost Missile owns two presentation lanes")
            {
                plan.sprites.push(
                    SpellSprite {
                        alpha,
                        blend: SpellBlend::Add,
                        rotation: lane.rotation_degrees * DEG,
                        scale: Vec2::new(lane.scale, lane.scale * lane.aspect),
                        tint: 0x80ffff,
                        ..SpellSprite::new(16, 1.0)
                    }
                    .into(),
                );
            }
            if state.vector[5] > 0.0 {
                let first = compose_affine(&[
                    translation_affine(float::sin(age * DEG).abs() * 3.0, 0.0),
                    rotation_affine(state.frost_turn_degrees.unwrap_or(0.0)),
                    scale_affine(1.4, 1.12),
                    rotation_affine(age * 8.0),
                ]);
                let second = compose_affine(&[
                    rotation_affine(state.frost_turn_degrees.unwrap_or(0.0)),
                    scale_affine(1.75, 1.4),
                    rotation_affine(age * (-12.0)),
                ]);
                for matrix in [first, second] {
                    let mut draw = WeldSpriteDraw::new(87, 1.0);
                    draw.matrix = Some(matrix);
                    draw.sprite.alpha = alpha * 0.15;
                    draw.sprite.blend = SpellBlend::Add;
                    draw.sprite.tint = 0xbfffff;
                    plan.sprites.push(draw);
                }
            }
        }
        WeldBuildId::BallLightning => {
            plan.sprites = lightning_core(
                id,
                frame,
                0.75 + visual_unit(id, frame, 0) * 0.5,
                alpha,
                frame as f64 * DEG,
            );
            let body_scale = 1.25 + visual_unit(id, frame, 0x71) * 0.1;
            plan.sprites.push(
                SpellSprite {
                    alpha: alpha * visual_unit(id, frame, 0x70),
                    offset: Vec2::new(0.0, -10.0),
                    ..SpellSprite::new(70, body_scale)
                }
                .into(),
            );
            plan.sprites.extend(element_vfx_sprites(
                WizardElement::Ether,
                state.base_presentation_phase_degrees.unwrap_or(0.0),
                1.0 + visual_unit(id, frame, 0x535a30) * 0.5,
                0.1,
            ));
        }
        WeldBuildId::CrawlingShock => {} // Its visible forks are separate tick-created fade actors.
        _ => unreachable!("native one-shot Weld build"),
    }
    plan
}

fn channel_plan(state: &WeldChannelActorState, age: f64) -> WeldVisualPlan {
    if state.common.build_id == WeldBuildId::FlameLash {
        // The constructor already consumed the first child update. Age one is
        // retained lifecycle state, but never receives a second ribbon draw.
        let Some(FlameChannelVisual::Birth(birth)) = &state.flame_visual else {
            return WeldVisualPlan::at(Vec2::ZERO);
        };
        if age >= 1.0 {
            return WeldVisualPlan::at(Vec2::ZERO);
        }
        let geometry = FlameLashFactoryInput {
            start: birth.start,
            control: birth.control,
            end: birth.end,
            offset: birth.offset,
        };
        let (phase_degrees, short_width_factor) = match birth.ribbon {
            FlameRibbonBirth::Short { width_factor } => (0.0, Some(width_factor)),
            FlameRibbonBirth::Long { phase_degrees } => (phase_degrees, None),
        };
        let ribbon = build_flame_lash_ribbon(FlameLashRibbonInput {
            geometry,
            width: birth.width,
            weak: state.underpowered,
            enhanced_effects: state.enhanced_effects,
            phase_degrees,
            short_width_factor,
        })
        .expect("retained Flame birth has validated native geometry");
        let mut plan = WeldVisualPlan::at(Vec2::ZERO);
        plan.split_bounds = Some(
            flame_lash_factory_geometry(geometry)
                .expect("retained Flame birth has validated native geometry")
                .split_bounds,
        );
        plan.meshes.push(WeldMeshDraw {
            alpha: 1.0,
            blend: SpellBlend::Add,
            colors: Some(ribbon.colors),
            mesh: AirMesh {
                indices: ribbon.indices.into_iter().map(u32::from).collect(),
                uvs: ribbon.uvs,
                vertices: ribbon.vertices,
            },
            record: 25,
            tint: 0xffffff,
            uv_space: WeldMeshUvSpace::BadGuysAtlas,
        });
        return plan;
    }
    let mut plan = WeldVisualPlan::at(state.common.origin);
    let Some(endpoint) = state.endpoint else {
        return plan;
    };
    let endpoint =
        Vec2::new(endpoint.x - state.common.origin.x, endpoint.y - state.common.origin.y);
    let beam = weld_blizzard_beam_plan(
        state.common.birth_tick,
        endpoint,
        Vec2::ZERO,
        state.underpowered,
        state.common.vector[6],
    );
    for quad in beam.quads {
        let mut vertices = Vec::new();
        for point in quad.vertices.as_chunks::<2>().0 {
            vertices.push(Vec2::new(point[0], point[1]));
        }
        plan.meshes.push(WeldMeshDraw {
            alpha: 1.0,
            blend: SpellBlend::Add,
            colors: None,
            mesh: AirMesh {
                indices: vec![0, 1, 2, 1, 3, 2],
                uvs: vec![
                    Vec2::ZERO,
                    Vec2::new(1.0, 0.0),
                    Vec2::new(0.0, 1.0),
                    Vec2::new(1.0, 1.0),
                ],
                vertices,
            },
            record: quad.record,
            tint: beam.tint,
            uv_space: WeldMeshUvSpace::Record,
        });
    }
    plan
}

fn impact_plan(state: &WeldImpactActorState, frame: u64, age: f64) -> WeldVisualPlan {
    let mut plan = WeldVisualPlan::at(state.position);
    match state.common.build_id {
        WeldBuildId::EtherealBoulder => {
            plan.sprites = element_vfx_sprites(
                WizardElement::Ether,
                frame as f64,
                state.presentation_scale,
                state.alpha,
            );
            plan.region_light_point = Some(state.position);
        }
        WeldBuildId::FrostMissile => {
            plan.sprites = element_vfx_sprites(
                WizardElement::Water,
                frame as f64,
                state.presentation_scale,
                state.alpha,
            )
        }
        WeldBuildId::BallLightning | WeldBuildId::CrawlingShock => {
            plan.sprites = lightning_core(
                state.common.owner.id,
                frame,
                state.presentation_scale,
                state.alpha,
                (state.presentation_rotation_degrees.unwrap_or(0.0) + age) * DEG,
            )
        }
        _ => {}
    }
    plan
}

fn meteor_plan(state: &WeldMeteorActorState, frame: u64, puppet_hit: bool) -> WeldVisualPlan {
    let mut plan = WeldVisualPlan::at(state.position);
    if state.phase == WeldMeteorPhase::Fall {
        let fall_offset = Vec2::new(0.0, fround(-768.0 * state.fall_height));
        let body_scale = if visual_unit(state.common.owner.id, frame, 0x5e16c0) < 0.5 {
            -state.body_scale
        } else {
            state.body_scale
        };
        let corona_scale = 3.0 + visual_unit(state.common.owner.id, frame, 0) * 0.5;
        plan.sprites.push(
            SpellSprite {
                offset: fall_offset,
                tint: 0xff8000,
                ..SpellSprite::new(15, corona_scale)
            }
            .into(),
        );
        plan.sprites.push(
            SpellSprite {
                blend: SpellBlend::Add,
                offset: fall_offset,
                rotation: (state.fall_heading_degrees * 0.5 + 180.0) * DEG,
                scale: Vec2::new(body_scale, (state.body_scale * 2.0).max(0.0)),
                ..SpellSprite::new(50, 1.0)
            }
            .into(),
        );
        if puppet_hit {
            let mut diffuse = plan.sprites.clone();
            for draw in &mut diffuse {
                draw.diffuse = true;
            }
            plan.sprites.extend(diffuse);
        }
        let ground_alpha = (1.0 - state.fall_height).max(0.0) * 0.5;
        if ground_alpha > 0.0 {
            plan.underlays.push(
                SpellSprite {
                    alpha: ground_alpha,
                    atlas: SpellAtlas::DeadHawg,
                    scale: Vec2::new(2.0, 1.6),
                    ..SpellSprite::new(19, 1.0)
                }
                .into(),
            );
        }
    } else {
        plan.underlays.push(
            SpellSprite {
                alpha: (state.impact_ticks_remaining as f64 / 100.0).min(1.0),
                blend: SpellBlend::Add,
                rotation: state.impact_rotation_degrees * DEG,
                scale: Vec2::new(state.impact_radius_scalar, state.impact_radius_scalar * 0.8),
                ..SpellSprite::new(67, 1.0)
            }
            .into(),
        );
    }
    plan
}

fn debris_plan(state: &WeldBoulderDebrisActorState) -> WeldVisualPlan {
    let debris = &state.debris.common;
    let position = Vec2::new(
        fround(state.position.x + debris.position.x),
        fround(state.position.y + debris.position.y),
    );
    let mut plan = WeldVisualPlan::at(position);
    plan.region_light_point = Some(position);
    plan.sort_bias = -15.0;
    if state.debris.enhanced_shadow && debris.height != 0.0 {
        plan.sprites.push(
            SpellSprite {
                alpha: debris.alpha.min(1.0),
                offset: Vec2::new(0.0, 2.0),
                rotation: debris.rotation_degrees * DEG,
                scale: Vec2::new(debris.scale, debris.scale * 0.75),
                tint: 0,
                ..SpellSprite::new(debris.record, 1.0)
            }
            .into(),
        );
    }
    plan.sprites.push(
        SpellSprite {
            alpha: debris.alpha.min(1.0),
            offset: Vec2::new(0.0, debris.height),
            rotation: debris.rotation_degrees * DEG,
            tint: 0xff0000 | ((debris.color_green * 255.0).trunc() as u32) << 8,
            ..SpellSprite::new(debris.record, debris.scale)
        }
        .into(),
    );
    plan
}

fn ethereal_boulder_plan(state: &WeldEtherealBoulderState, frame: u64, age: f64) -> WeldVisualPlan {
    let geometry = earth_boulder_presentation_plan(
        &EarthBoulderPresentationState {
            age_ticks: age,
            assembly_charge: state.assembly_scale,
            charge: state.scale,
            flight_ticks: state.flight_ticks,
            id: state.common.owner.id,
            orientation: state.orientation,
            phase: state.phase,
            shell_charge: state.shell_scale,
        },
        frame as f64,
    );
    let centers = if state.phase == PrimarySpellProjectilePhase::Held {
        ethereal_boulder_held_centers(state)
    } else {
        vec![Vec2::ZERO]
    };
    let body_tint = pack_spell_rgb(geometry.body_alpha, geometry.body_alpha * 0.75, 1.0);
    let mut plan = WeldVisualPlan::at(state.common.origin);
    plan.region_light_point = Some(state.common.origin);
    plan.sort_bias = geometry.sort_bias;
    plan.sprites.push(
        SpellSprite {
            alpha: geometry.aura.alpha,
            offset: geometry.visual_offset,
            tint: pack_spell_rgb(1.0, 0.9, 1.0),
            ..SpellSprite::new(15, geometry.aura.scale)
        }
        .into(),
    );
    plan.sprites.push(
        SpellSprite {
            alpha: geometry.opening_flash.alpha,
            blend: SpellBlend::Add,
            offset: geometry.visual_offset,
            rotation: geometry.opening_flash.rotation,
            ..SpellSprite::new(86, geometry.opening_flash.scale)
        }
        .into(),
    );
    for (center_index, center) in centers.iter().enumerate() {
        for rock in &geometry.rocks {
            let offset = Vec2::new(
                geometry.visual_offset.x + center.x + rock.position.x,
                geometry.visual_offset.y + center.y + rock.position.y,
            );
            if rock.shell_index.is_none() {
                let scale = (1.0
                    + visual_unit(state.common.owner.id, frame, 0x60c540 + center_index as u32)
                        * 0.1)
                    * 2.0
                    * state.scale;
                let mut sprites =
                    element_vfx_sprites(WizardElement::Ether, frame as f64, scale, 1.0);
                for draw in &mut sprites {
                    draw.sprite.offset.x += offset.x;
                    draw.sprite.offset.y += offset.y;
                }
                plan.sprites.extend(sprites);
            } else {
                plan.sprites.push(
                    SpellSprite {
                        offset,
                        rotation: rock.rotation,
                        tint: body_tint,
                        ..SpellSprite::new(rock.record, (rock.stored_scale * 0.75).max(0.25))
                    }
                    .into(),
                );
            }
        }
    }
    plan.sprites.push(boulder_auxiliary_sprite(
        state.common.owner.id,
        frame,
        state.scale,
        state.phase,
    ));
    plan
}

fn ethereal_boulder_held_centers(state: &WeldEtherealBoulderState) -> Vec<Vec2> {
    let direction = state.common.direction;
    let perpendicular = Vec2::new(direction.y, -direction.x);
    let along = |amount: f64| Vec2::new(direction.x * amount, direction.y * amount);
    let across = |amount: f64| Vec2::new(perpendicular.x * amount, perpendicular.y * amount);
    let centers = match state.quantity {
        1 => vec![Vec2::ZERO],
        2 => vec![across(30.0), across(-30.0)],
        3 => vec![along(30.0), across(30.0), across(-30.0)],
        _ => vec![along(30.0), across(30.0), across(-30.0), along(-15.0)],
    };
    let mut ordered: Vec<_> = centers.into_iter().enumerate().collect();
    // Native compares numeric Y, then list order, treating -0 and +0 as ties.
    ordered.sort_by(|(left_index, left), (right_index, right)| {
        left.y.partial_cmp(&right.y).unwrap().then(left_index.cmp(right_index))
    });
    ordered.into_iter().map(|(_, center)| Vec2::new(fround(center.x), fround(center.y))).collect()
}

fn hailstones_plan(state: &WeldHailstonesState, frame: u64, age: f64) -> WeldVisualPlan {
    let mut plan = WeldVisualPlan::at(state.common.origin);
    plan.region_light_point = Some(state.common.origin);
    let held = state.phase == PrimarySpellProjectilePhase::Held;
    let opening_mix = (1.0 - age * 0.03500000014901161).max(0.0);
    if held {
        plan.sprites.push(
            SpellSprite {
                alpha: 0.35 + visual_unit(state.common.owner.id, frame, 0x611160) * 0.25,
                tint: pack_spell_rgb(1.0, 1.0, 0.9),
                ..SpellSprite::new(15, state.scale * 4.1_f32 as f64)
            }
            .into(),
        );
        plan.sprites.extend(element_vfx_sprites(WizardElement::Water, frame as f64, 1.75, 1.0));
    }
    for rock in &state.rocks {
        let offset = if held {
            Vec2::new(rock.local_position.x, rock.local_position.y)
        } else {
            weld_hailstone_draw_offset(rock)
        };
        let scale = rock.visual_scale.max(0.45) * if held { 0.85 } else { 0.75 };
        plan.sprites.push(
            SpellSprite {
                offset,
                tint: if held {
                    pack_spell_rgb(1.0 - opening_mix, 1.0 - opening_mix, 1.0 - opening_mix)
                } else {
                    0xffffff
                },
                ..SpellSprite::new(rock.sprite_record, scale)
            }
            .into(),
        );
        plan.sprites.push(
            SpellSprite {
                alpha: rock.phase * if held { 0.8 } else { 1.0 },
                offset,
                rotation: atan2(state.common.direction.y, state.common.direction.x),
                ..SpellSprite::new(32, 1.0)
            }
            .into(),
        );
        plan.sort_bias = plan.sort_bias.max(rock.local_position.z);
    }
    plan.sprites.push(boulder_auxiliary_sprite(
        state.common.owner.id,
        frame,
        state.scale,
        state.phase,
    ));
    plan
}

fn boulder_auxiliary_sprite(
    id: u64,
    frame: u64,
    scale: f64,
    phase: PrimarySpellProjectilePhase,
) -> WeldSpriteDraw {
    if phase == PrimarySpellProjectilePhase::Held {
        SpellSprite {
            alpha: 0.25 + visual_unit(id, frame, 0x5e5530) * 0.5,
            tint: pack_spell_rgb(0.85, 1.0, 0.85),
            ..SpellSprite::new(15, scale * 2.5)
        }
        .into()
    } else {
        WeldSpriteDraw::new(67, scale * 2.5)
    }
}

fn hail_child_plan(state: &WeldHailChildActorState) -> WeldVisualPlan {
    match state {
        WeldHailChildActorState::Knockback(actor) => WeldVisualPlan::at(actor.common.origin),
        WeldHailChildActorState::Flash(actor) => {
            let mut plan = WeldVisualPlan::at(actor.position);
            plan.sprites
                .push(SpellSprite { alpha: actor.alpha, ..SpellSprite::new(15, 1.0) }.into());
            plan
        }
        WeldHailChildActorState::Line(actor) => {
            let mut plan = WeldVisualPlan::at(actor.start);
            let midpoint = Vec2::new(
                fround((actor.end.x - actor.start.x) * 0.5),
                fround((actor.end.y - actor.start.y) * 0.5),
            );
            let end =
                Vec2::new(fround(actor.end.x - actor.start.x), fround(actor.end.y - actor.start.y));
            plan.lines.push(WeldLineDraw {
                alpha: actor.alpha * 0.5,
                color: 0x00ffff,
                end: midpoint,
                start: Vec2::ZERO,
                width: actor.width,
            });
            plan.lines.push(WeldLineDraw {
                alpha: actor.alpha * actor.end_alpha,
                color: 0xffffff,
                end,
                start: midpoint,
                width: actor.width,
            });
            plan
        }
        WeldHailChildActorState::TerrainParticle(actor) => {
            let mut plan = WeldVisualPlan::at(actor.position);
            plan.sprites.push(
                SpellSprite {
                    alpha: actor.alpha,
                    blend: SpellBlend::Add,
                    rotation: actor.rotation_degrees * DEG,
                    tint: actor.tint,
                    ..SpellSprite::new(45, actor.scale)
                }
                .into(),
            );
            plan
        }
        WeldHailChildActorState::TerrainBouncer(actor) => {
            let mut plan = WeldVisualPlan::at(actor.position);
            if actor.enhanced_shadow && actor.height != 0.0 {
                plan.sprites.push(
                    SpellSprite {
                        alpha: actor.alpha.min(1.0),
                        offset: Vec2::new(0.0, 2.0),
                        rotation: actor.rotation_degrees * DEG,
                        scale: Vec2::new(actor.scale, actor.scale * 0.75),
                        tint: 0,
                        ..SpellSprite::new(32, 1.0)
                    }
                    .into(),
                );
            }
            plan.sprites.push(
                SpellSprite {
                    alpha: actor.alpha.min(1.0),
                    offset: Vec2::new(0.0, actor.height),
                    rotation: actor.rotation_degrees * DEG,
                    ..SpellSprite::new(32, actor.scale)
                }
                .into(),
            );
            plan
        }
    }
}

fn steam_plan(state: &WeldSteamActorState) -> WeldVisualPlan {
    let scale = state.scale * 2.0;
    let mut plan = WeldVisualPlan::at(state.position);
    if state.variant == WeldSteamVariant::Over {
        plan.sprites.push(
            SpellSprite {
                alpha: 0.25 * state.life.min(state.phase),
                rotation: state.rotation_degrees * DEG,
                ..SpellSprite::new(76, scale)
            }
            .into(),
        );
        return plan;
    }
    let alpha = state.alpha_multiplier * (state.life * state.life).min(state.phase);
    let green = (1.0 - state.tint_fade).max(0.0);
    let blue = (1.0 - state.tint_fade * 4.0).max(0.0);
    let draw = WeldSpriteDraw::from(SpellSprite {
        alpha,
        blend: if state.tint_fade > 0.0 { SpellBlend::Add } else { SpellBlend::Normal },
        rotation: state.rotation_degrees * DEG,
        tint: pack_spell_rgb(1.0, green, blue),
        ..SpellSprite::new(76, scale)
    });
    plan.sprites.push(draw);
    if state.tint_fade > 0.0 {
        plan.sprites.push(draw);
    }
    plan
}

fn element_vfx_sprites(
    element: WizardElement,
    phase: f64,
    scale: f64,
    alpha: f64,
) -> Vec<WeldSpriteDraw> {
    let mut sprites = Vec::new();
    for draw in element_vfx_plan_at_phase(element, phase, scale) {
        sprites.push(
            SpellSprite {
                alpha: alpha * draw.alpha,
                blend: if draw.blend == ElementVfxBlend::Add {
                    SpellBlend::Add
                } else {
                    SpellBlend::Normal
                },
                offset: Vec2::new(draw.x, draw.y),
                rotation: draw.rotation * DEG,
                tint: pack_spell_rgb(draw.color[0], draw.color[1], draw.color[2]),
                ..SpellSprite::new(draw.sprite.records()[draw.frame], draw.scale)
            }
            .into(),
        );
    }
    sprites
}

fn lightning_core(id: u64, frame: u64, scale: f64, alpha: f64, angle: f64) -> Vec<WeldSpriteDraw> {
    let corona = build_air_corona_plan(AirCoronaInput {
        alpha,
        angle,
        center: Vec2::ZERO,
        random_salt: 0x536380 ^ (frame as u32),
        scale,
        seed: id,
    });
    let mut draws = Vec::with_capacity(corona.draws.len());
    for mut draw in corona.draws {
        draw.alpha *= corona.alpha;
        draws.push(draw.into());
    }
    draws
}

fn visual_unit(id: u64, frame: u64, lane: u32) -> f64 {
    f64::from(visual_word(id, frame, lane)) / 4_294_967_296.0
}

fn visual_word(id: u64, frame: u64, lane: u32) -> u32 {
    let mut value = (id as u32)
        ^ (frame as u32).wrapping_add(1).wrapping_mul(0x9e3779b1)
        ^ lane.wrapping_add(1).wrapping_mul(0x85ebca6b);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value
}

fn compose_affine(matrices: &[WeldAffineMatrix]) -> WeldAffineMatrix {
    let mut left = WeldAffineMatrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };
    for right in matrices {
        left = WeldAffineMatrix {
            a: left.a * right.a + left.c * right.b,
            b: left.b * right.a + left.d * right.b,
            c: left.a * right.c + left.c * right.d,
            d: left.b * right.c + left.d * right.d,
            tx: left.a * right.tx + left.c * right.ty + left.tx,
            ty: left.b * right.tx + left.d * right.ty + left.ty,
        };
    }
    left
}
fn translation_affine(x: f64, y: f64) -> WeldAffineMatrix {
    WeldAffineMatrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: x, ty: y }
}
fn scale_affine(x: f64, y: f64) -> WeldAffineMatrix {
    WeldAffineMatrix { a: x, b: 0.0, c: 0.0, d: y, tx: 0.0, ty: 0.0 }
}
fn rotation_affine(degrees: f64) -> WeldAffineMatrix {
    let (sine, cosine) = float::sin_cos(degrees * DEG);
    WeldAffineMatrix { a: cosine, b: sine, c: -sine, d: cosine, tx: 0.0, ty: 0.0 }
}
