//! GPU binding for `renderer/native-secondary-world-view.ts`. Canonical sim plans
//! own geometry, clocks and colors; this view owns stock textures and render targets.

mod composites;
mod draws;
mod electric_burn;
mod hits;
mod staff;

use std::collections::{BTreeMap, BTreeSet};

use solomon_assets::parse_bundle;
use solomon_sim::Vec2;
use solomon_sim::hub::painter::PainterLayer;
use solomon_sim::secondary::electric_burn::{
    NativeElectricBurnArcPlan, native_electric_burn_arc_plan,
};
use solomon_sim::secondary::presentation::*;
use solomon_sim::secondary::{
    NativeSecondaryActorKind, NativeSecondaryActorState, NativeSecondaryPlayerState,
};
use solomon_sim::spells::air::{
    NativeAirLightningInput, NativeAirLightningPlan, build_native_air_lightning_plan,
};

use crate::assets;
use crate::courtyard::Camera;
use crate::render::{RenderTarget, Renderer, Texture};
use crate::spells::{SpellView, StockAtlas};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SecondaryPresentationMemory {
    /// The source samples Ring of Fire explosion gain once, at view birth.
    pub fire_explosion_point_gains: BTreeMap<u64, f64>,
}

pub struct SecondaryPainter {
    pub layer: PainterLayer,
    pub region_light_point: Option<Vec2>,
}

/// Projection of the canonical puppet feedback at its fixed tick. The host uses
/// native_puppet_hit_alpha/native_puppet_hit_tint; the view owns no feedback clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondaryPuppetHit {
    pub alpha: f64,
    pub tint: u32,
}

struct ActorPlan {
    id: u64,
    kind: NativeSecondaryActorKind,
    plan: NativeSecondaryPresentationPlan,
    scale: f64,
    lightning: Option<NativeAirLightningPlan>,
    electric_arc: Option<NativeElectricBurnArcPlan>,
}

#[derive(Clone, Copy)]
enum PainterDraw {
    Actor(usize),
    Leviathan(usize),
    ElectricArcBand(usize, usize),
    HiddenOwner,
}

struct PainterRoot {
    painter: SecondaryPainter,
    draw: PainterDraw,
}

/// Shares the primary spell atlas uploads. Golem, the two extracted Clothes
/// cells and the repeating Ether Plane texture are the only additional images.
pub struct SecondaryView {
    bad_guys: StockAtlas,
    dead_hawg: StockAtlas,
    golem: StockAtlas,
    clothes: [Texture; 2],
    ether_plane: Texture,
    actors: Vec<ActorPlan>,
    roots: Vec<PainterRoot>,
    background: Vec<usize>,
    pre_world: Vec<usize>,
    composite_children: BTreeMap<u64, Vec<usize>>,
    leviathan_targets: BTreeMap<u64, RenderTarget>,
    storm_targets: BTreeMap<u64, RenderTarget>,
    hit_targets: BTreeMap<u64, RenderTarget>,
    puppet_hits: BTreeMap<u64, SecondaryPuppetHit>,
    pub memory: SecondaryPresentationMemory,
}

impl SecondaryView {
    pub async fn load(renderer: &Renderer, spells: &SpellView) -> Result<Self, String> {
        let bundle = assets::fetch_bytes("assets/stock/Golem.bundle").await?;
        let records = parse_bundle(&bundle).map_err(|error| format!("Golem.bundle: {error}"))?;
        let golem = assets::load_image("assets/stock/Golem.png").await?;
        let harden = assets::load_image("assets/stock/player-harden-ice.png").await?;
        let mindblast = assets::load_image("assets/stock/player-mindblast-ring.png").await?;
        let ether_plane = assets::load_image("assets/stock/etherplane.png").await?;
        Ok(Self {
            bad_guys: spells.bad_guys().clone(),
            dead_hawg: spells.dead_hawg().clone(),
            golem: StockAtlas { records, texture: renderer.upload(&golem)? },
            clothes: [renderer.upload(&harden)?, renderer.upload(&mindblast)?],
            ether_plane: renderer.upload_with_repeat(&ether_plane, true)?,
            actors: Vec::new(),
            roots: Vec::new(),
            background: Vec::new(),
            pre_world: Vec::new(),
            composite_children: BTreeMap::new(),
            leviathan_targets: BTreeMap::new(),
            storm_targets: BTreeMap::new(),
            hit_targets: BTreeMap::new(),
            puppet_hits: BTreeMap::new(),
            memory: SecondaryPresentationMemory::default(),
        })
    }

    /// Call after begin_frame and before world submission. State remains read-only;
    /// render-target updates happen here, so draw() is a small painter callback.
    #[allow(clippy::too_many_arguments)] // One actual world frame, retained clocks and current render quality.
    pub fn prepare(
        &mut self,
        renderer: &mut Renderer,
        actors: &[NativeSecondaryActorState],
        world_key: &str,
        presentation_frame: Option<f64>,
        presentation_ages: Option<&BTreeMap<u64, f64>>,
        point_gain: &dyn Fn(Vec2) -> f64,
        enhanced_effects: bool,
    ) -> Result<(), String> {
        self.actors.clear();
        self.roots.clear();
        self.background.clear();
        self.pre_world.clear();
        self.composite_children.clear();
        self.puppet_hits.clear();
        let mut indexes = BTreeMap::new();
        let mut live_ids = BTreeSet::new();
        for actor in actors {
            if actor.world_key != world_key
                || actor.kind == NativeSecondaryActorKind::EarthquakeSceneryWobble
            {
                continue;
            }
            live_ids.insert(actor.id);
            let gain = if actor.kind == NativeSecondaryActorKind::RingFireExplosion {
                *self
                    .memory
                    .fire_explosion_point_gains
                    .entry(actor.id)
                    .or_insert_with(|| point_gain(actor.position))
            } else {
                1.0
            };
            let age_ticks = presentation_ages
                .and_then(|ages| ages.get(&actor.id))
                .copied()
                .unwrap_or(actor.age_ticks as f64);
            if actor.kind == NativeSecondaryActorKind::GolemDeath {
                use solomon_sim::death_animations::NATIVE_GOLEM_DEATH_PAINTER_COUNT;
                use solomon_sim::secondary::presentation::{
                    NativeGolemDeathMember, native_golem_death_member_plan,
                };
                indexes.insert(actor.id, self.actors.len());
                for member_index in 0..NATIVE_GOLEM_DEATH_PAINTER_COUNT {
                    let member = if member_index + 1 == NATIVE_GOLEM_DEATH_PAINTER_COUNT {
                        NativeGolemDeathMember::Star
                    } else {
                        NativeGolemDeathMember::Fragment(member_index)
                    };
                    self.actors.push(ActorPlan {
                        id: actor.id,
                        kind: actor.kind,
                        plan: native_golem_death_member_plan(actor, age_ticks, member),
                        scale: actor.scale,
                        lightning: None,
                        electric_arc: None,
                    });
                }
                continue;
            }
            let plan = native_secondary_presentation_plan_at_age(
                actor,
                age_ticks,
                presentation_frame.unwrap_or(age_ticks),
                gain,
                enhanced_effects,
            );
            let lightning = if actor.kind == NativeSecondaryActorKind::StormStrike {
                Some(build_native_air_lightning_plan(NativeAirLightningInput {
                    chained: false,
                    enhanced_effects: true,
                    // native-air-presentation.ts explicitly floors this input.
                    age_ticks: age_ticks.floor().max(0.0) as u64,
                    birth_tick: actor.phase as u64,
                    endpoint: Vec2::new(
                        actor.endpoint.x - actor.position.x,
                        actor.endpoint.y - actor.position.y,
                    ),
                    id: actor.id,
                    midpoint: Vec2::new(
                        actor.midpoint.x - actor.position.x,
                        actor.midpoint.y - actor.position.y,
                    ),
                    underpowered: false,
                }))
            } else {
                None
            };
            indexes.insert(actor.id, self.actors.len());
            let electric_arc = if actor.kind == NativeSecondaryActorKind::ElectricBurnArc {
                native_electric_burn_arc_plan(actor, age_ticks, enhanced_effects)
            } else {
                None
            };
            self.actors.push(ActorPlan {
                id: actor.id,
                kind: actor.kind,
                plan,
                scale: actor.scale,
                lightning,
                electric_arc,
            });
        }
        self.memory.fire_explosion_point_gains.retain(|id, _| live_ids.contains(id));
        retire_targets(renderer, &mut self.leviathan_targets, &live_ids);
        retire_targets(renderer, &mut self.storm_targets, &live_ids);
        retire_targets(renderer, &mut self.hit_targets, &live_ids);
        let mut composite_owner = BTreeMap::new();
        for (child, parent) in native_secondary_composite_owner_entries(actors, world_key) {
            if let (Some(&child_index), Some(_)) = (indexes.get(&child), indexes.get(&parent)) {
                composite_owner.insert(child, parent);
                self.composite_children.entry(parent).or_default().push(child_index);
            }
        }
        for actor in &self.actors {
            if actor.kind == NativeSecondaryActorKind::Leviathan {
                self.composite_children.entry(actor.id).or_default();
            }
        }
        for children in self.composite_children.values_mut() {
            children.sort_by(|&first, &second| {
                let first = &self.actors[first].plan;
                let second = &self.actors[second].plan;
                (first.world_y + first.sort_bias).total_cmp(&(second.world_y + second.sort_bias))
            });
        }
        for actor in actors {
            let Some(&index) = indexes.get(&actor.id) else {
                continue;
            };
            if actor.kind == NativeSecondaryActorKind::GolemDeath {
                assert_eq!(
                    actor.painter_registrations.len(),
                    solomon_sim::death_animations::NATIVE_GOLEM_DEATH_PAINTER_COUNT,
                    "Golem death retains each fragment and star painter"
                );
                for (member, registration) in actor.painter_registrations.iter().enumerate() {
                    let plan = &self.actors[index + member].plan;
                    self.roots.push(PainterRoot {
                        painter: SecondaryPainter {
                            layer: registration.layer(plan.world_y, plan.sort_bias),
                            region_light_point: None,
                        },
                        draw: PainterDraw::Actor(index + member),
                    });
                }
                continue;
            }
            let plan = &self.actors[index].plan;
            if actor.kind == NativeSecondaryActorKind::ElectricBurnArc {
                let Some(arc) = &self.actors[index].electric_arc else {
                    continue;
                };
                let registration = actor.painter_registrations[0];
                let mut owner = registration.layer(actor.position.y + arc.bounds_y, 0.0);
                owner.visible = false;
                self.roots.push(PainterRoot {
                    painter: SecondaryPainter { layer: owner, region_light_point: None },
                    draw: PainterDraw::HiddenOwner,
                });
                for (band_index, band) in arc.bands.iter().enumerate() {
                    let mut layer = registration.layer(actor.position.y + band.painter_y, 0.0);
                    layer.is_insertion = true;
                    self.roots.push(PainterRoot {
                        painter: SecondaryPainter { layer, region_light_point: None },
                        draw: PainterDraw::ElectricArcBand(index, band_index),
                    });
                }
                continue;
            }
            if !plan.background_draws.is_empty() {
                self.background.push(index);
            }
            if !plan.underlay_draws.is_empty() {
                self.pre_world.push(index);
            }
            if composite_owner.contains_key(&actor.id) {
                continue;
            }
            if plan.draws.is_empty()
                && matches!(
                    actor.kind,
                    NativeSecondaryActorKind::AcidRain | NativeSecondaryActorKind::EarthquakeDust
                )
            {
                continue;
            }
            let registration = *actor
                .painter_registrations
                .first()
                .expect("secondary actor has its canonical painter registration");
            let region_light_point = (actor.kind == NativeSecondaryActorKind::EarthquakeDebris)
                .then_some(actor.position);
            let draw = if self.composite_children.contains_key(&actor.id) {
                PainterDraw::Leviathan(index)
            } else {
                PainterDraw::Actor(index)
            };
            if matches!(
                actor.kind,
                NativeSecondaryActorKind::AcidRain | NativeSecondaryActorKind::StormCloud
            ) {
                let mut owner = registration.layer(actor.position.y, 0.0);
                owner.visible = false;
                self.roots.push(PainterRoot {
                    painter: SecondaryPainter { layer: owner, region_light_point },
                    draw: PainterDraw::HiddenOwner,
                });
                let mut inserted = registration.layer(plan.world_y, plan.sort_bias);
                inserted.is_insertion = true;
                self.roots.push(PainterRoot {
                    painter: SecondaryPainter { layer: inserted, region_light_point },
                    draw,
                });
            } else {
                self.roots.push(PainterRoot {
                    painter: SecondaryPainter {
                        layer: registration.layer(plan.world_y, plan.sort_bias),
                        region_light_point,
                    },
                    draw,
                });
            }
        }
        // Allocation changes GL texture bindings. Flush any preceding batch first.
        renderer.flush();
        for actor in &self.actors {
            if actor.plan.storm_composite.is_some() && !self.storm_targets.contains_key(&actor.id) {
                self.storm_targets.insert(actor.id, renderer.create_render_target(256)?);
            }
        }
        for id in self.composite_children.keys() {
            if !self.leviathan_targets.contains_key(id) {
                self.leviathan_targets.insert(*id, renderer.create_render_target(256)?);
            }
        }
        self.update_composites(renderer, &indexes);
        Ok(())
    }

    pub fn painters(&self) -> impl Iterator<Item = (usize, &SecondaryPainter)> {
        self.roots.iter().enumerate().map(|(index, root)| (index, &root.painter))
    }

    pub fn draw(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        index: usize,
        region_tint: [f32; 3],
    ) {
        let root = &self.roots[index];
        let tint = if root.painter.region_light_point.is_some() { region_tint } else { [1.0; 3] };
        match root.draw {
            PainterDraw::Actor(index) => self.draw_actor(renderer, camera, index, tint),
            PainterDraw::ElectricArcBand(actor, band) => {
                self.draw_electric_burn_arc_band(renderer, camera, actor, band, tint)
            }
            PainterDraw::Leviathan(index) => {
                self.draw_actor(renderer, camera, index, tint);
                self.draw_leviathan_outputs(renderer, camera, index, tint);
                self.draw_leviathan_hit(renderer, camera, index, tint);
            }
            PainterDraw::HiddenOwner => {}
        }
    }

    /// Source `background` roots have depth zero, before the pre-world queue.
    /// Their registered owner still participates in the ordinary painter planner.
    pub fn draw_background(&self, renderer: &mut Renderer, camera: &Camera) {
        for &index in &self.background {
            let actor = &self.actors[index];
            for draw in &actor.plan.background_draws {
                self.draw_sprite(renderer, camera, actor.plan.root, draw, [1.0; 3]);
            }
        }
    }

    pub fn draw_pre_world(&self, renderer: &mut Renderer, camera: &Camera) {
        for &index in &self.pre_world {
            let actor = &self.actors[index];
            for draw in &actor.plan.underlay_draws {
                self.draw_sprite(renderer, camera, actor.plan.root, draw, [1.0; 3]);
            }
        }
    }

    pub fn draw_magic_shield(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        player: Option<&NativeSecondaryPlayerState>,
        tick: f64,
    ) {
        let plan = native_player_magic_shield_plan(player, tick);
        if !plan.visible {
            return;
        }
        let draw = NativeSecondarySpriteDraw {
            alpha: plan.alpha,
            atlas: NativeSecondaryAtlas::Clothes,
            blend: NativeSecondaryBlend::Normal,
            color_mode: None,
            entry: NATIVE_PLAYER_MAGIC_SHIELD_ENTRY,
            offset: Vec2::new(0.0, NATIVE_PLAYER_MAGIC_SHIELD_OFFSET_Y),
            role: "player-magic-shield".into(),
            rotation_radians: 0.0,
            scale_x: plan.scale,
            scale_y: plan.scale,
            tint: plan.tint,
        };
        self.draw_sprite(renderer, camera, position, &draw, [1.0; 3]);
    }

    pub fn destroy(&self, renderer: &Renderer) {
        for target in self
            .leviathan_targets
            .values()
            .chain(self.storm_targets.values())
            .chain(self.hit_targets.values())
        {
            renderer.destroy_render_target(target);
        }
        renderer.destroy_texture(&self.golem.texture);
        for texture in &self.clothes {
            renderer.destroy_texture(texture);
        }
        renderer.destroy_texture(&self.ether_plane);
    }
}

fn retire_targets(
    renderer: &Renderer,
    targets: &mut BTreeMap<u64, RenderTarget>,
    live: &BTreeSet<u64>,
) {
    targets.retain(|id, target| {
        if live.contains(id) {
            true
        } else {
            renderer.destroy_render_target(target);
            false
        }
    });
}
