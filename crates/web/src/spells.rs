//! Stock primary spell sprites and lightning meshes. The simulation supplies the
//! native draw plans and painter registrations; this view owns only GPU resources.

use solomon_sim::float;
use std::collections::{BTreeMap, BTreeSet};

use solomon_assets::{SpriteRecord, parse_bundle};
use solomon_sim::Vec2;
use solomon_sim::element_vfx::{ElementVfxBlend, ElementVfxDraw};
use solomon_sim::enemies::puppet_hits::EnemyPuppetHit;
use solomon_sim::float::{fround, hypot};
use solomon_sim::harden_effects::native_harden_effect_plan;
use solomon_sim::hub::painter::PainterLayer;
use solomon_sim::painter::PainterRegistration;
use solomon_sim::presentation::multiply_native_tints;
use solomon_sim::spells::air::{NativeAirLightningInput, build_native_air_lightning_plan};
use solomon_sim::spells::air_water::*;
use solomon_sim::spells::earth::{EarthImpactSeed, earth_impact_fragments_at_age};
use solomon_sim::spells::earth_presentation::earth_boulder_sprite_plan_at_age;
use solomon_sim::spells::ether::{
    create_native_ether_blast_particle_program, native_ether_blast_particle_frame,
};
use solomon_sim::spells::ether_presentation::{
    ether_primary_flight_plan, ether_primary_impact_plan, ether_primary_pierce_streak_plan,
};
use solomon_sim::spells::fire_presentation::*;
use solomon_sim::spells::model::*;
use solomon_sim::spells::presentation::*;
use solomon_sim::spells::water::*;
use solomon_sim::spells::weld::{
    NativeFlameLashBounds, NativeWeldLineDraw, NativeWeldMeshUvSpace,
    NativeWeldPersistentActorState, NativeWeldPresentationState, NativeWeldSpriteDraw,
    NativeWeldWorldActor, build_native_flame_lash_split_bands, native_flame_region_clipped_band,
    native_weld_visual_plan_at_age,
};

use crate::courtyard::Camera;
use crate::native_mesh::{
    NativeMeshVertex, clip_native_mesh, clip_native_mesh_in_rect, native_sprite_texel_rect,
};
use crate::presentation::PresentationClocks;
use solomon_graphics::assets;
use solomon_graphics::{BlendMode, Renderer, Texture, TextureColorMode, Vertex};

pub struct SpellPainter {
    pub layer: PainterLayer,
    pub region_light_point: Option<Vec2>,
    /// Source promoteOwnerOverlays keeps these roots above their actual player.
    pub overlay_owner_id: Option<String>,
    /// Retained view identity and insertion order in PrimarySpellWorldView.
    /// Equal promoted depths keep the previous frame's container order.
    pub view_id: u64,
    pub view_creation_order: u64,
}

#[derive(Clone)]
pub struct StockAtlas {
    pub records: Vec<SpriteRecord>,
    pub texture: Texture,
}

enum SpellDraw {
    Sprite {
        position: Vec2,
        draw: NativeWeldSpriteDraw,
    },
    Mesh {
        record: usize,
        triangles: Vec<[NativeMeshVertex; 3]>,
        alpha: f64,
        tint: u32,
        uv_space: NativeWeldMeshUvSpace,
    },
    Line {
        position: Vec2,
        draw: NativeWeldLineDraw,
    },
    GroundGlow {
        position: Vec2,
        alpha: f64,
    },
}

struct SpellRoot {
    painter: SpellPainter,
    draws: Vec<SpellDraw>,
}

pub struct SpellView {
    bad_guys: StockAtlas,
    dead_hawg: StockAtlas,
    roots: Vec<SpellRoot>,
    pre_world: Vec<SpellDraw>,
    post_world: Vec<SpellDraw>,
    fire_explosion_point_gains: BTreeMap<u64, f64>,
    view_creation_orders: BTreeMap<u64, u64>,
    next_view_creation_order: u64,
}

impl SpellView {
    /// Enemy and secondary views share the already-uploaded stock atlas handles.
    pub fn bad_guys(&self) -> &StockAtlas {
        &self.bad_guys
    }

    pub fn dead_hawg(&self) -> &StockAtlas {
        &self.dead_hawg
    }

    pub async fn load(renderer: &Renderer) -> Result<Self, String> {
        Ok(Self {
            bad_guys: load_atlas(renderer, "BadGuys").await?,
            dead_hawg: load_atlas(renderer, "DeadHawg").await?,
            roots: Vec::new(),
            pre_world: Vec::new(),
            post_world: Vec::new(),
            fire_explosion_point_gains: BTreeMap::new(),
            view_creation_orders: BTreeMap::new(),
            next_view_creation_order: 0,
        })
    }

    /// Source FireExplosionSpellView captures this scalar at view creation;
    /// its geometry and logical light keep the same gain while the camera moves.
    #[must_use]
    pub fn fire_explosion_point_gain(&self, id: u64) -> Option<f64> {
        self.fire_explosion_point_gains.get(&id).copied()
    }

    /// A new run may reuse spell IDs; none of the previous world's GPU state survives.
    pub fn reset(&mut self) {
        self.roots.clear();
        self.pre_world.clear();
        self.post_world.clear();
        self.fire_explosion_point_gains.clear();
        self.view_creation_orders.clear();
        self.next_view_creation_order = 0;
    }

    #[allow(clippy::too_many_arguments)] // One Source world draw with its actual clocks, quality and Region mask.
    pub fn prepare(
        &mut self,
        spells: &SpellState,
        world_key: &str,
        presentation_frame: u64,
        point_gain: &dyn Fn(Vec2) -> f64,
        puppet_hits: &solomon_sim::ComponentEntities<EnemyPuppetHit>,
        clocks: Option<&PresentationClocks>,
        enhanced_effects: bool,
        region_view: Option<NativeFlameLashBounds>,
    ) {
        self.fire_explosion_point_gains.retain(|id, _| {
            spells.transients.iter().any(|effect| {
                matches!(effect, PrimarySpellTransient::FireExplosion(spell) if spell.common.id == *id && spell.common.world_key == world_key)
            })
        });
        self.roots.clear();
        self.pre_world.clear();
        self.post_world.clear();
        let mut live_view_ids = BTreeSet::new();
        for spell in &spells.projectiles {
            if spell.common().world_key == world_key {
                live_view_ids.insert(spell.common().id);
            }
        }
        for spell in &spells.transients {
            if spell.common().world_key == world_key {
                live_view_ids.insert(spell.common().id);
            }
        }
        self.view_creation_orders.retain(|id, _| live_view_ids.contains(id));
        for spell in &spells.projectiles {
            let common = spell.common();
            if common.world_key != world_key {
                continue;
            }
            let age = clocks
                .and_then(|clocks| clocks.primary.get(&common.id))
                .copied()
                .unwrap_or(common.age_ticks as f64);
            let first_root = self.roots.len();
            self.admit_view(common.id);
            let plan = match spell {
                PrimarySpellProjectile::Earth(spell) => {
                    // EarthBoulderView uses actor age for its cosmetic program too.
                    earth_boulder_sprite_plan_at_age(spell, age, age)
                }
                PrimarySpellProjectile::Ether(spell) => {
                    let mut plan = ether_primary_flight_plan(
                        common.id,
                        age,
                        spell.speed,
                        spell.visual_scale,
                        spell.underpowered,
                    );
                    plan.position.x += common.position.x;
                    plan.position.y += common.position.y;
                    plan.world_y = common.position.y;
                    plan
                }
                PrimarySpellProjectile::Fire(spell) => native_fireball_plan(
                    NativeFireballPresentationState {
                        age_ticks: age,
                        ..NativeFireballPresentationState::from(spell)
                    },
                    presentation_frame,
                ),
                PrimarySpellProjectile::Weld(spell) => {
                    let target_id = format!("primary:{}", common.id);
                    let puppet_hit = puppet_hits.iter().any(|hit| hit.target_id == target_id);
                    self.append_weld(
                        NativeWeldPresentationState::Projectile(spell),
                        presentation_frame,
                        age,
                        common.registration,
                        false,
                        puppet_hit,
                        enhanced_effects,
                        region_view,
                    );
                    self.identify_roots(first_root, common.id);
                    continue;
                }
            };
            self.append_plan(plan, common.registration);
            if matches!(spell, PrimarySpellProjectile::Earth(_)) {
                self.roots
                    .last_mut()
                    .expect("Earth Boulder owns a world root")
                    .painter
                    .overlay_owner_id = Some(common.owner_id.clone());
            }
            self.identify_roots(first_root, common.id);
        }
        for effect in &spells.transients {
            let common = effect.common();
            if common.world_key != world_key {
                continue;
            }
            let age = clocks
                .and_then(|clocks| clocks.primary.get(&common.id))
                .copied()
                .unwrap_or(common.age_ticks as f64);
            let first_root = self.roots.len();
            self.admit_view(common.id);
            match effect {
                // Staff views share SecondaryView's stock atlas.
                PrimarySpellTransient::Staff(_) => {}
                PrimarySpellTransient::AirHurricane(spell) => {
                    let mut plan = plan_at(spell.position);
                    for draw in native_hurricane_visual_plan(spell) {
                        let core = draw.role == NativeHurricaneSpriteRole::Core;
                        plan.draws.push(NativeSpellSprite {
                            alpha: draw.alpha,
                            atlas: if core { SpellAtlas::DeadHawg } else { SpellAtlas::BadGuys },
                            blend: if draw.additive { SpellBlend::Add } else { SpellBlend::Normal },
                            offset: draw.position,
                            rotation: draw.rotation_radians,
                            scale: draw.scale,
                            tint: draw.tint,
                            ..NativeSpellSprite::new(if core { 15 } else { 84 }, 1.0)
                        });
                    }
                    self.append_plan(plan, common.painter_registrations[0]);
                    self.roots
                        .last_mut()
                        .expect("Hurricane owns a world root")
                        .painter
                        .overlay_owner_id = Some(common.owner_id.clone());
                }
                PrimarySpellTransient::WaterAura(spell) => {
                    let draw = native_water_aura_visual_plan(spell);
                    let mut plan = plan_at(spell.origin);
                    plan.draws.push(NativeSpellSprite {
                        alpha: draw.alpha,
                        blend: SpellBlend::Add,
                        rotation: draw.rotation_radians,
                        tint: draw.tint,
                        ..NativeSpellSprite::new(14, draw.scale)
                    });
                    self.append_plan(plan, common.painter_registrations[0]);
                }
                PrimarySpellTransient::WaterHail(spell) => {
                    let draw = native_hail_visual_plan(spell);
                    let mut plan = plan_at(spell.position);
                    plan.draws.push(NativeSpellSprite {
                        alpha: draw.alpha,
                        offset: Vec2::new(0.0, draw.offset_y),
                        rotation: draw.rotation_radians,
                        ..NativeSpellSprite::new(32, draw.scale)
                    });
                    self.append_plan(plan, common.painter_registrations[0]);
                }
                PrimarySpellTransient::Weld(spell) => {
                    let target_id = format!("primary:{}", common.id);
                    let puppet_hit = puppet_hits.iter().any(|hit| hit.target_id == target_id);
                    // The pinned JS split spreads the parent's painter list onto
                    // every Boulder piece, which its strict planner rejects.
                    // Each piece already has its own actual Actor registration.
                    let registration = match spell {
                        NativeWeldWorldActor::Persistent(
                            NativeWeldPersistentActorState::EtherealBoulder(_),
                        ) => common.light_registration.expect("Boulder owns an Actor registration"),
                        _ => common.painter_registrations[0],
                    };
                    self.append_weld(
                        NativeWeldPresentationState::World(spell),
                        presentation_frame,
                        age,
                        registration,
                        matches!(spell, NativeWeldWorldActor::Channel(_)),
                        puppet_hit,
                        enhanced_effects,
                        region_view,
                    );
                }
                PrimarySpellTransient::Harden(effect) => self.append_plan(
                    native_harden_effect_plan(effect),
                    common.painter_registrations[0],
                ),
                PrimarySpellTransient::EarthBoulderBit(spell) => {
                    let debris = &spell.debris.common;
                    let position = Vec2::new(
                        spell.position.x + debris.position.x,
                        spell.position.y + debris.position.y,
                    );
                    let mut plan = plan_at(position);
                    plan.sort_bias = -15.0;
                    plan.region_light_point = Some(position);
                    if spell.debris.enhanced_shadow && debris.height != 0.0 {
                        plan.draws.push(NativeSpellSprite {
                            alpha: debris.alpha.min(1.0),
                            offset: Vec2::new(0.0, 2.0),
                            rotation: debris.rotation_degrees * std::f64::consts::PI / 180.0,
                            scale: Vec2::new(debris.scale, debris.scale * 0.75),
                            tint: 0,
                            ..NativeSpellSprite::new(debris.record, 1.0)
                        });
                    }
                    plan.draws.push(NativeSpellSprite {
                        alpha: debris.alpha.min(1.0),
                        offset: Vec2::new(0.0, debris.height),
                        rotation: debris.rotation_degrees * std::f64::consts::PI / 180.0,
                        tint: 0xff0000 | ((debris.color_green * 255.0).trunc() as u32) << 8,
                        ..NativeSpellSprite::new(debris.record, debris.scale)
                    });
                    self.append_plan(plan, common.painter_registrations[0]);
                }
                PrimarySpellTransient::Air(spell) => self.append_air(spell),
                PrimarySpellTransient::Water(spell) => self.append_water(spell),
                PrimarySpellTransient::EarthImpact(spell) => {
                    let seed = EarthImpactSeed {
                        birth_tick: spell.birth_tick,
                        charge: spell.charge,
                        enhanced_effects: spell.enhanced_effects,
                        id: common.id,
                    };
                    for fragment in earth_impact_fragments_at_age(seed, age) {
                        let position = Vec2::new(
                            spell.origin.x + fragment.position.x,
                            spell.origin.y + fragment.position.y,
                        );
                        let mut plan = NativeSpellSpritePlan {
                            position,
                            world_y: position.y,
                            sort_bias: -15.0,
                            region_light_point: Some(position),
                            draws: Vec::new(),
                        };
                        plan.draws.push(NativeSpellSprite {
                            alpha: fragment.alpha.clamp(0.0, 1.0),
                            offset: Vec2::new(0.0, fragment.height),
                            rotation: fragment.rotation * std::f64::consts::PI / 180.0,
                            ..NativeSpellSprite::new(fragment.record, fragment.scale)
                        });
                        self.append_plan(plan, common.painter_registrations[fragment.index]);
                    }
                }
                PrimarySpellTransient::EarthCalledRock(spell) => {
                    let mut plan = plan_at(spell.position);
                    if spell.height < 0.0 {
                        plan.draws.push(NativeSpellSprite::new(
                            2008 + spell.variant as usize,
                            spell.scale * 0.75,
                        ));
                    }
                    plan.draws.push(NativeSpellSprite {
                        offset: Vec2::new(0.0, spell.height),
                        rotation: spell.rotation * std::f64::consts::PI / 180.0,
                        ..NativeSpellSprite::new(2008 + spell.variant as usize, spell.scale)
                    });
                    self.append_plan(plan, common.painter_registrations[0]);
                }
                PrimarySpellTransient::EtherImpact(spell) => self
                    .append_plan(ether_primary_impact_plan(spell), common.painter_registrations[0]),
                PrimarySpellTransient::EtherPierceStreak(spell) => self.append_plan(
                    ether_primary_pierce_streak_plan(spell),
                    common.painter_registrations[0],
                ),
                PrimarySpellTransient::EtherBlast(spell) => {
                    let particles = create_native_ether_blast_particle_program(
                        &mut spell.presentation_rng.clone(),
                    );
                    for (index, particle) in particles.into_iter().enumerate() {
                        let frame = native_ether_blast_particle_frame(particle, age);
                        let position = Vec2::new(
                            fround(spell.origin.x + frame.position.x),
                            fround(spell.origin.y + frame.position.y),
                        );
                        let mut plan = plan_at(position);
                        plan.draws.push(NativeSpellSprite {
                            alpha: frame.alpha,
                            blend: SpellBlend::Add,
                            rotation: particle.rotation_degrees * std::f64::consts::PI / 180.0,
                            scale: Vec2::new(
                                particle.scale,
                                fround(particle.scale * 0.8_f32 as f64),
                            ),
                            tint: pack_spell_rgb(particle.red, particle.green, particle.blue),
                            ..NativeSpellSprite::new(particle.sprite_record, 1.0)
                        });
                        self.append_plan(plan, common.painter_registrations[index]);
                    }
                }
                PrimarySpellTransient::FireParticle(spell) => self.append_plan(
                    native_fire_particle_plan_at_age(spell, age),
                    common.painter_registrations[0],
                ),
                PrimarySpellTransient::FireImpact(spell) => self.append_plan(
                    native_fire_impact_plan_at_age(spell, age),
                    common.painter_registrations[0],
                ),
                PrimarySpellTransient::FireExplosion(spell) => {
                    let gain = *self
                        .fire_explosion_point_gains
                        .entry(common.id)
                        .or_insert_with(|| point_gain(spell.origin));
                    self.append_plan(
                        native_fire_explosion_plan(
                            age,
                            spell.origin,
                            spell.presentation,
                            spell.visual_scale,
                            gain,
                        ),
                        common.painter_registrations[0],
                    );
                }
                PrimarySpellTransient::FireEmber(spell) => {
                    let root_index = self.roots.len();
                    self.append_plan(
                        native_fire_ember_plan(
                            common.id,
                            spell.height,
                            spell.life,
                            spell.phase,
                            spell.position,
                            presentation_frame,
                            enhanced_effects,
                        ),
                        common.painter_registrations[0],
                    );
                    if enhanced_effects && spell.height < 0.0 {
                        self.roots[root_index].draws.insert(
                            0,
                            SpellDraw::GroundGlow {
                                position: spell.position,
                                alpha: fround((1.0 - spell.height / -50.0 * 0.5) * 0.25),
                            },
                        );
                    }
                }
                PrimarySpellTransient::FireGoodImp(spell) => self
                    .append_plan(native_fire_good_imp_plan(spell), common.painter_registrations[0]),
                PrimarySpellTransient::FirePatch(spell) => self.append_plan(
                    native_fire_patch_plan(spell, presentation_frame),
                    common.painter_registrations[0],
                ),
            }
            self.identify_roots(first_root, common.id);
        }
    }

    fn admit_view(&mut self, id: u64) {
        if self.view_creation_orders.contains_key(&id) {
            return;
        }
        // Source views are retained, and new containers are appended in the
        // projectile-then-transient traversal, independently of their actor IDs.
        self.view_creation_orders.insert(id, self.next_view_creation_order);
        self.next_view_creation_order += 1;
    }

    fn identify_roots(&mut self, first_root: usize, id: u64) {
        let creation_order = self.view_creation_orders[&id];
        for root in &mut self.roots[first_root..] {
            root.painter.view_id = id;
            root.painter.view_creation_order = creation_order;
        }
    }

    pub fn painters(&self) -> impl Iterator<Item = (usize, &SpellPainter)> {
        self.roots.iter().enumerate().map(|(index, root)| (index, &root.painter))
    }

    /// Includes retained views whose current plan emits no visible painter root.
    pub fn live_view_creation_orders(&self) -> impl Iterator<Item = (u64, u64)> + '_ {
        self.view_creation_orders.iter().map(|(&id, &order)| (id, order))
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
        for draw in &root.draws {
            self.draw_operation(renderer, camera, draw, tint);
        }
    }

    pub fn draw_queue(&self, renderer: &mut Renderer, camera: &Camera, queue: SpellDrawQueue) {
        let draws = match queue {
            SpellDrawQueue::PreWorld => &self.pre_world,
            SpellDrawQueue::PostWorld => &self.post_world,
            SpellDrawQueue::World => return,
        };
        for draw in draws {
            self.draw_operation(renderer, camera, draw, [1.0; 3]);
        }
    }

    /// Stock world particles share the primary atlas uploads and sprite submission path.
    pub fn draw_sprite_plan(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        plan: &NativeSpellSpritePlan,
    ) {
        for draw in &plan.draws {
            self.draw_sprite(renderer, camera, plan.position, (*draw).into(), [1.0; 3], None);
        }
    }

    pub fn draw_element_vfx(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        plan: &[ElementVfxDraw],
    ) {
        for element in plan {
            let draw = NativeSpellSprite {
                alpha: element.alpha,
                blend: if element.blend == ElementVfxBlend::Add {
                    SpellBlend::Add
                } else {
                    SpellBlend::Normal
                },
                offset: Vec2::new(element.x, element.y),
                rotation: element.rotation * std::f64::consts::PI / 180.0,
                tint: pack_spell_rgb(element.color[0], element.color[1], element.color[2]),
                ..NativeSpellSprite::new(element.sprite.records()[element.frame], element.scale)
            };
            self.draw_sprite(renderer, camera, position, draw.into(), [1.0; 3], element.anchor);
        }
    }

    fn append_plan(&mut self, plan: NativeSpellSpritePlan, registration: PainterRegistration) {
        let queue_only = !plan.draws.is_empty()
            && plan.draws.iter().all(|draw| draw.queue != SpellDrawQueue::World);
        let mut draws = Vec::new();
        for draw in plan.draws {
            let operation = SpellDraw::Sprite { position: plan.position, draw: draw.into() };
            match draw.queue {
                SpellDrawQueue::World => draws.push(operation),
                SpellDrawQueue::PreWorld => self.pre_world.push(operation),
                SpellDrawQueue::PostWorld => self.post_world.push(operation),
            }
        }
        // A source pre/post-world queue root never occupies an ordinary world row.
        // Explicitly empty owner plans retain their painter, including hidden Air owners.
        if queue_only {
            return;
        }
        self.roots.push(SpellRoot {
            painter: SpellPainter {
                layer: registration.layer(plan.world_y, plan.sort_bias),
                region_light_point: plan.region_light_point,
                overlay_owner_id: None,
                view_id: 0,
                view_creation_order: 0,
            },
            draws,
        });
    }

    #[allow(clippy::too_many_arguments)] // Retained state plus the live Source compositor inputs.
    fn append_weld(
        &mut self,
        state: NativeWeldPresentationState<'_>,
        frame: u64,
        age: f64,
        registration: PainterRegistration,
        split: bool,
        puppet_hit: bool,
        enhanced_effects: bool,
        region_view: Option<NativeFlameLashBounds>,
    ) {
        let overlay_owner_id = match &state {
            NativeWeldPresentationState::Projectile(actor) => actor.common.owner_id.clone(),
            NativeWeldPresentationState::World(actor) => actor.common().owner.owner_id.clone(),
        };
        let plan = native_weld_visual_plan_at_age(state, frame, age, puppet_hit);
        for draw in plan.underlays {
            self.pre_world.push(SpellDraw::Sprite { position: plan.position, draw });
        }
        if plan.sprites.is_empty() && plan.meshes.is_empty() && plan.lines.is_empty() {
            return;
        }
        if let Some(bounds) = plan.split_bounds {
            let mut owner = registration.layer(plan.position.y + bounds.y, 0.0);
            owner.visible = false;
            self.roots.push(SpellRoot {
                painter: SpellPainter {
                    layer: owner,
                    region_light_point: plan.region_light_point,
                    overlay_owner_id: Some(overlay_owner_id),
                    view_id: 0,
                    view_creation_order: 0,
                },
                draws: Vec::new(),
            });
            let bands =
                build_native_flame_lash_split_bands("flame", bounds, enhanced_effects, region_view)
                    .expect("retained Flame has finite padded bounds and Region view");
            for band in bands {
                let clip = native_flame_region_clipped_band(band.clip, region_view);
                let mut draws = Vec::new();
                for mesh in &plan.meshes {
                    draws.push(SpellDraw::Mesh {
                        record: mesh.record,
                        triangles: clip_native_mesh_in_rect(
                            &mesh.mesh,
                            plan.position,
                            clip,
                            mesh.colors.as_deref(),
                        ),
                        alpha: mesh.alpha,
                        tint: mesh.tint,
                        uv_space: mesh.uv_space,
                    });
                }
                let mut layer = registration.layer(plan.position.y + band.painter_y, 0.0);
                layer.is_insertion = true;
                layer.visible = clip.width > 0.0 && clip.height > 0.0;
                self.roots.push(SpellRoot {
                    painter: SpellPainter {
                        layer,
                        region_light_point: plan.region_light_point,
                        overlay_owner_id: None,
                        view_id: 0,
                        view_creation_order: 0,
                    },
                    draws,
                });
            }
            return;
        }
        if split {
            // These channel meshes retain their birth geometry. Their invisible
            // owner admits the same 25-unit painter bands as NativeZAnimSplit.
            let mut minimum = f64::INFINITY;
            let mut maximum = f64::NEG_INFINITY;
            for mesh in &plan.meshes {
                for point in &mesh.mesh.vertices {
                    minimum = minimum.min(point.y);
                    maximum = maximum.max(point.y);
                }
            }
            let mut owner = registration.layer(plan.position.y + minimum, 0.0);
            owner.visible = false;
            self.roots.push(SpellRoot {
                painter: SpellPainter {
                    layer: owner,
                    region_light_point: plan.region_light_point,
                    overlay_owner_id: Some(overlay_owner_id.clone()),
                    view_id: 0,
                    view_creation_order: 0,
                },
                draws: Vec::new(),
            });
            let count = ((maximum - minimum) / 25.0).ceil() as usize;
            for band in 0..count {
                let top = minimum + band as f64 * 25.0;
                let bottom = top + 25.0;
                let mut draws = Vec::new();
                for mesh in &plan.meshes {
                    draws.push(SpellDraw::Mesh {
                        record: mesh.record,
                        triangles: clip_native_mesh(&mesh.mesh, plan.position, top, bottom),
                        alpha: mesh.alpha,
                        tint: mesh.tint,
                        uv_space: mesh.uv_space,
                    });
                }
                let mut layer = registration.layer(plan.position.y + bottom, 0.0);
                layer.is_insertion = true;
                self.roots.push(SpellRoot {
                    painter: SpellPainter {
                        layer,
                        region_light_point: plan.region_light_point,
                        overlay_owner_id: None,
                        view_id: 0,
                        view_creation_order: 0,
                    },
                    draws,
                });
            }
            return;
        }
        let mut draws = Vec::new();
        for draw in plan.lines {
            draws.push(SpellDraw::Line { position: plan.position, draw });
        }
        for mesh in plan.meshes {
            draws.push(SpellDraw::Mesh {
                record: mesh.record,
                triangles: clip_native_mesh(
                    &mesh.mesh,
                    plan.position,
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                ),
                alpha: mesh.alpha,
                tint: mesh.tint,
                uv_space: mesh.uv_space,
            });
        }
        for draw in plan.sprites {
            draws.push(SpellDraw::Sprite { position: plan.position, draw });
        }
        self.roots.push(SpellRoot {
            painter: SpellPainter {
                layer: registration.layer(plan.world_y, plan.sort_bias),
                region_light_point: plan.region_light_point,
                overlay_owner_id: Some(overlay_owner_id),
                view_id: 0,
                view_creation_order: 0,
            },
            draws,
        });
    }

    fn append_water(&mut self, spell: &PrimarySpellWaterTransient) {
        let water = water_frost_jet_plan(spell);
        let mut plan = plan_at(water.position);
        plan.world_y = water.world_y;
        let queue = if water_frost_jet_painter_lane(water.kind)
            == WaterFrostJetPainterLane::PostWorldQueue
        {
            SpellDrawQueue::PostWorld
        } else {
            SpellDrawQueue::World
        };
        // Pixi's child insertion order is normal core, additive core, glint.
        for pass in
            [WaterFrostJetPass::Core, WaterFrostJetPass::AdditiveCore, WaterFrostJetPass::Glint]
        {
            for draw in &water.draws {
                if draw.pass != pass {
                    continue;
                }
                plan.draws.push(NativeSpellSprite {
                    alpha: quantize_water_frost_alpha(draw.alpha),
                    blend: if draw.blend == WaterFrostJetBlend::Add {
                        SpellBlend::Add
                    } else {
                        SpellBlend::Normal
                    },
                    offset: Vec2::new(
                        draw.position.x - water.position.x,
                        draw.position.y - water.position.y,
                    ),
                    queue,
                    rotation: draw.rotation,
                    tint: pack_water_frost_tint(draw.color),
                    ..NativeSpellSprite::new(
                        if draw.sprite == WaterFrostJetSprite::Core { 30 } else { 28 },
                        draw.scale,
                    )
                });
            }
        }
        self.append_plan(plan, spell.common.painter_registrations[0]);
    }

    fn append_air(&mut self, spell: &PrimarySpellAirTransient) {
        let plan = build_native_air_lightning_plan(NativeAirLightningInput {
            chained: spell.chained,
            enhanced_effects: spell.enhanced_effects,
            age_ticks: spell.common.age_ticks,
            birth_tick: spell.birth_tick,
            endpoint: Vec2::new(
                spell.endpoint.x - spell.origin.x,
                spell.endpoint.y - spell.origin.y,
            ),
            id: spell.common.id,
            midpoint: Vec2::new(
                spell.midpoint.x - spell.origin.x,
                spell.midpoint.y - spell.origin.y,
            ),
            underpowered: spell.underpowered,
        });
        let registrations = &spell.common.painter_registrations;
        if let Some(layers) = plan.body {
            let mut minimum = f64::INFINITY;
            let mut maximum = f64::NEG_INFINITY;
            for layer in &layers {
                for point in &layer.mesh.vertices {
                    minimum = minimum.min(point.y);
                    maximum = maximum.max(point.y);
                }
                if let Some(branch) = &layer.branch {
                    for point in &branch.mesh.vertices {
                        minimum = minimum.min(point.y);
                        maximum = maximum.max(point.y);
                    }
                }
            }
            let count = ((maximum - minimum) / 25.0).ceil() as usize;
            let mut owner = registrations[0].layer(spell.origin.y + minimum, 0.0);
            owner.visible = false;
            self.roots.push(SpellRoot {
                painter: SpellPainter {
                    layer: owner,
                    region_light_point: None,
                    overlay_owner_id: None,
                    view_id: 0,
                    view_creation_order: 0,
                },
                draws: Vec::new(),
            });
            for band in 0..count {
                let top = minimum + band as f64 * 25.0;
                let bottom = top + 25.0;
                let mut draws = Vec::new();
                for layer in &layers {
                    draws.push(SpellDraw::Mesh {
                        record: layer.texture_record,
                        triangles: clip_native_mesh(&layer.mesh, spell.origin, top, bottom),
                        alpha: layer.alpha,
                        tint: layer.tint,
                        uv_space: NativeWeldMeshUvSpace::Record,
                    });
                    if let Some(branch) = &layer.branch {
                        draws.push(SpellDraw::Mesh {
                            record: branch.texture_record,
                            triangles: clip_native_mesh(&branch.mesh, spell.origin, top, bottom),
                            alpha: layer.alpha,
                            tint: layer.tint,
                            uv_space: NativeWeldMeshUvSpace::Record,
                        });
                    }
                }
                let mut layer = registrations[0].layer(spell.origin.y + bottom, 0.0);
                layer.is_insertion = true;
                self.roots.push(SpellRoot {
                    painter: SpellPainter {
                        layer,
                        region_light_point: None,
                        overlay_owner_id: None,
                        view_id: 0,
                        view_creation_order: 0,
                    },
                    draws,
                });
            }
        }
        if let Some(corona) = plan.source_corona {
            let mut source = plan_at(Vec2::new(
                spell.origin.x + corona.center.x,
                spell.origin.y + corona.center.y,
            ));
            source.draws = corona.draws;
            for draw in &mut source.draws {
                draw.alpha *= corona.alpha;
            }
            self.append_plan(source, registrations[1]);
        }
        let corona = plan.contact_corona;
        if corona.alpha > 0.0 {
            let mut contact = plan_at(Vec2::new(
                spell.origin.x + corona.center.x,
                spell.origin.y + corona.center.y,
            ));
            contact.sort_bias = 50.0;
            contact.draws = corona.draws;
            for draw in &mut contact.draws {
                draw.alpha *= corona.alpha;
            }
            self.append_plan(contact, registrations[2]);
        }
    }

    fn draw_operation(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        operation: &SpellDraw,
        tint: [f32; 3],
    ) {
        match operation {
            SpellDraw::Sprite { position, draw } => {
                self.draw_sprite(renderer, camera, *position, *draw, tint, None)
            }
            SpellDraw::Mesh { record, triangles, alpha, tint, uv_space } => {
                let record = &self.bad_guys.records[*record];
                let tint = rgba(*tint, *alpha, [1.0; 3]);
                let texels = match uv_space {
                    NativeWeldMeshUvSpace::Record => native_sprite_texel_rect(record),
                    NativeWeldMeshUvSpace::BadGuysAtlas => {
                        assert_eq!(
                            [self.bad_guys.texture.width, self.bad_guys.texture.height],
                            [2048.0; 2],
                            "Flame requires the original 2048-square BadGuys page"
                        );
                        [0.0, 0.0, 2048.0, 2048.0]
                    }
                };
                let triangles: Vec<[Vertex; 3]> = triangles
                    .iter()
                    .map(|triangle| {
                        triangle.map(|vertex| {
                            let projected =
                                camera.project(vertex.position.x, vertex.position.y, 0.0, 0.0);
                            Vertex {
                                position: [projected[0], projected[1]],
                                texel: [
                                    texels[0] + vertex.uv.x as f32 * texels[2],
                                    texels[1] + vertex.uv.y as f32 * texels[3],
                                ],
                                tint: std::array::from_fn(|channel| {
                                    tint[channel] * vertex.color[channel] as f32
                                }),
                            }
                        })
                    })
                    .collect();
                renderer.draw_triangles(&self.bad_guys.texture, &triangles, BlendMode::Add);
            }
            SpellDraw::Line { position, draw } => {
                let delta = Vec2::new(draw.end.x - draw.start.x, draw.end.y - draw.start.y);
                let length = hypot(delta.x, delta.y);
                if length == 0.0 {
                    return;
                }
                let normal = Vec2::new(
                    -delta.y / length * draw.width * 0.5,
                    delta.x / length * draw.width * 0.5,
                );
                let corners = [
                    Vec2::new(draw.start.x + normal.x, draw.start.y + normal.y),
                    Vec2::new(draw.end.x + normal.x, draw.end.y + normal.y),
                    Vec2::new(draw.end.x - normal.x, draw.end.y - normal.y),
                    Vec2::new(draw.start.x - normal.x, draw.start.y - normal.y),
                ]
                .map(|point| {
                    let projected =
                        camera.project(position.x + point.x, position.y + point.y, 0.0, 0.0);
                    Vertex {
                        position: [projected[0], projected[1]],
                        texel: [0.5; 2],
                        tint: rgba(draw.color, draw.alpha, tint),
                    }
                });
                renderer.draw_solid_triangles(
                    &[[corners[0], corners[1], corners[2]], [corners[0], corners[2], corners[3]]],
                    BlendMode::Normal,
                );
            }
            SpellDraw::GroundGlow { position, alpha } => {
                let width = fround(38.0 * 0.75);
                let height = fround(37.0 * 0.6_f32 as f64);
                let [x, y, width, height] = camera.project(
                    position.x - width / 2.0,
                    position.y - height / 2.0,
                    width,
                    height,
                );
                renderer.draw_quad(
                    &self.bad_guys.texture,
                    native_sprite_texel_rect(&self.bad_guys.records[15]),
                    [[x, y], [x + width, y], [x + width, y + height], [x, y + height]],
                    [rgba(0xff8040, *alpha, [1.0; 3]); 4],
                    BlendMode::Normal,
                );
            }
        }
    }

    fn draw_sprite(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        draw: NativeWeldSpriteDraw,
        region_tint: [f32; 3],
        anchor: Option<[f64; 2]>,
    ) {
        let matrix = draw.matrix;
        let color_mode =
            if draw.diffuse { TextureColorMode::AlphaMask } else { TextureColorMode::Texture };
        let draw = draw.sprite;
        if draw.alpha <= 0.0 {
            return;
        }
        let atlas = match draw.atlas {
            SpellAtlas::BadGuys => &self.bad_guys,
            SpellAtlas::DeadHawg => &self.dead_hawg,
        };
        let record = &atlas.records[draw.record];
        let width = f64::from(record.width);
        let height = f64::from(record.height);
        let (left, top) = if let Some(anchor) = anchor {
            (-width * anchor[0], -height * anchor[1])
        } else {
            (-width / 2.0 + f64::from(record.center_x), -height / 2.0 + f64::from(record.center_y))
        };
        let local =
            [(left, top), (left + width, top), (left + width, top + height), (left, top + height)];
        let (sine, cosine) = float::sin_cos(draw.rotation);
        let corners = local.map(|(x, y)| {
            let (x, y) = if let Some(matrix) = matrix {
                (matrix.a * x + matrix.c * y + matrix.tx, matrix.b * x + matrix.d * y + matrix.ty)
            } else {
                let scaled_x = x * draw.scale.x;
                let scaled_y = y * draw.scale.y;
                (
                    draw.offset.x + scaled_x * cosine - scaled_y * sine,
                    draw.offset.y + scaled_x * sine + scaled_y * cosine,
                )
            };
            let projected = camera.project(position.x + x, position.y + y, 0.0, 0.0);
            [projected[0], projected[1]]
        });
        renderer.draw_quad_with_color_mode(
            &atlas.texture,
            native_sprite_texel_rect(record),
            corners,
            [rgba(draw.tint, draw.alpha, region_tint); 4],
            if draw.blend == SpellBlend::Add { BlendMode::Add } else { BlendMode::Normal },
            color_mode,
        );
    }
}

async fn load_atlas(renderer: &Renderer, name: &str) -> Result<StockAtlas, String> {
    let records = parse_bundle(&assets::fetch_bytes(&format!("assets/stock/{name}.bundle")).await?)
        .map_err(|error| format!("{name}.bundle: {error}"))?;
    let image = assets::load_image(&format!("assets/stock/{name}.png")).await?;
    Ok(StockAtlas { records, texture: renderer.upload(&image)? })
}

fn plan_at(position: Vec2) -> NativeSpellSpritePlan {
    NativeSpellSpritePlan {
        position,
        world_y: position.y,
        sort_bias: 0.0,
        region_light_point: None,
        draws: Vec::new(),
    }
}

fn rgba(tint: u32, alpha: f64, light: [f32; 3]) -> [f32; 4] {
    let light = pack_spell_rgb(f64::from(light[0]), f64::from(light[1]), f64::from(light[2]));
    let tint = multiply_native_tints(tint, light);
    [
        ((tint >> 16) & 255) as f32 / 255.0,
        ((tint >> 8) & 255) as f32 / 255.0,
        (tint & 255) as f32 / 255.0,
        // Pixi packs group alpha into one byte, including source plans such
        // as Hail's birth fade whose authored alpha temporarily exceeds one.
        (alpha.clamp(0.0, 1.0) * 255.0).trunc() as f32 / 255.0,
    ]
}
