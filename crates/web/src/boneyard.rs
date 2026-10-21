//! Runtime scenery with live native painter order, lighting, gates and tree fading.
//!
//! `boneyard/compile.rs` translates the current Website's static-world compiler.
//! Every scene uses its accepted geometry and one immutable original glyph catalog.

mod canvas;
mod catalog;
mod compile;
mod shadow;
mod tree;

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::boneyard::gate::GateLeaf;
use solomon_sim::boneyard::lighting::{
    NATIVE_DEFAULT_LIGHT_QUALITY, NativeBoneyardLightIndex, NativeBoneyardLightQuery,
    native_region_light_target_plan,
};
use solomon_sim::boneyard::presentation::{
    BoneyardPainterEntry, BoneyardPainterProxy, TreeOcclusion, boneyard_painter_order,
    native_gate_art,
};
use solomon_sim::boneyard::scenery::{
    NativeSceneryShadowGlyph, NativeSceneryShadowQuad, NativeTreeOcclusionPresentation,
    native_tree_secondary_position,
};
use solomon_sim::boneyard::shadows::{
    NativeShadowCaster, NativeShadowEdge, NativeShadowProgram, native_shadow_edges,
    native_shadow_line,
};
use solomon_sim::boneyard::surface::{
    NativeArenaFieldLattice, NativeArenaFieldRange, NativeBoneyardSurfaceMesh,
    native_arena_field_mesh_plan, native_arena_field_visible_range,
};
use solomon_sim::boneyard::{BoneyardBounds, LoadedBoneyard};
use solomon_sim::enemies::puppet_hits::{EnemyPuppetHit, EnemyPuppetHitKind};
use solomon_sim::float::fround;
use solomon_sim::hub::painter::{PainterLane, PainterLayer};
use solomon_sim::painter::overlays::{PainterOverlayOrder, PainterOwnerOverlay};
use solomon_sim::presentation::native_puppet_hit_tint;
use web_sys::HtmlImageElement;

use crate::courtyard::{ActorPainters, Camera};
use crate::presentation::PresentationClocks;
use crate::render::{BlendMode, Quad, RenderTarget, Renderer, Texture, TextureColorMode, Vertex};

use shadow::{SceneryShadowGlyph, SceneryShadowPlans, ScrubDocument, ScrubView};

const ASSET_ROOT: &str = "assets/stock/boneyard";
const ROAD_IMAGES: [&str; 5] = ["road.png", "road2.png", "road3.png", "road4.png", "road5.png"];
const WHITE: [f32; 4] = [1.0; 4];

pub(crate) use tree::TreeFrameSource;

#[derive(Clone, Copy)]
struct SceneryMaterial {
    alpha: f64,
    tint: u32,
    diffuse: bool,
    hit_alpha: Option<f64>,
}

impl SceneryMaterial {
    fn color_mode(self) -> TextureColorMode {
        if self.diffuse { TextureColorMode::AlphaMask } else { TextureColorMode::Texture }
    }

    fn rgba(self, scalar: f64) -> [f32; 4] {
        let shade = fround(fround(scalar.clamp(0.0, 1.0)) * 255.0).trunc();
        let channel = |shift| {
            // Pixi's multiplyHexColors truncates inherited RGB before batching.
            (shade * f64::from((self.tint >> shift) & 255_u32) / 255.0).trunc() as f32 / 255.0
        };
        [
            channel(16),
            channel(8),
            channel(0),
            (self.alpha.clamp(0.0, 1.0) * 255.0).trunc() as f32 / 255.0,
        ]
    }
}

struct SceneryHitRedraw {
    body: SceneryMaterial,
    proxy_visible: bool,
}

fn scenery_hit_redraw(
    parent_alpha: f64,
    hit_alpha: f64,
    complex_lighting: bool,
    building: bool,
) -> SceneryHitRedraw {
    let vertex_colors_own_material = complex_lighting && building;
    SceneryHitRedraw {
        body: SceneryMaterial {
            alpha: parent_alpha * if vertex_colors_own_material { 1.0 } else { hit_alpha },
            tint: if vertex_colors_own_material {
                0xffffff
            } else {
                native_puppet_hit_tint(complex_lighting, 0xffffff)
            },
            diffuse: true,
            hit_alpha: Some(hit_alpha),
        },
        proxy_visible: complex_lighting || !building,
    }
}

/// Decoded images are prepared without borrowing the running game across browser awaits.
pub struct PreparedBoneyardView {
    document: SceneryDocument,
    images: BTreeMap<String, PreparedImage>,
    puppet_target_ids: Vec<Option<String>>,
}

impl PreparedBoneyardView {
    pub async fn load(loaded: &LoadedBoneyard) -> Result<Self, String> {
        compile::load(loaded).await
    }

    pub fn upload(
        self,
        renderer: &Renderer,
        logical_view: Vec2,
        device_resolution: f64,
        initial_tick: u64,
        initial_stopped_world_tick: u64,
    ) -> Result<BoneyardView, String> {
        let mut textures = BTreeMap::new();
        for (name, image) in self.images {
            let texture = match image {
                PreparedImage::Image(image) => {
                    let repeat = name == "fencegrate.png" || ROAD_IMAGES.contains(&name.as_str());
                    renderer.upload_with_repeat(&image, repeat)?
                }
                PreparedImage::Pixels { width, height, pixels } => {
                    renderer.upload_rgba(width, height, &pixels)?
                }
            };
            textures.insert(name, texture);
        }
        let mut trees = BTreeMap::new();
        for tree in &self.document.tree_inputs {
            let bounds = &tree.bounds;
            trees.insert(
                tree.eid.clone(),
                TreeOcclusion::new(
                    &tree.eid,
                    tree.position.into(),
                    [bounds.x, bounds.y, bounds.w, bounds.h],
                    tree.polygon.iter().copied().map(Into::into).collect(),
                ),
            );
        }
        let mut proxies = BTreeMap::new();
        for (index, layer) in self.document.layers.iter().enumerate() {
            if layer.band == SceneryBand::Proxy {
                let id = layer.object_id.as_ref().ok_or("a scenery proxy has no owner")?;
                if proxies.insert(id.clone(), index).is_some() {
                    return Err(format!("scenery owner {id} has two proxies"));
                }
            }
        }
        let light_plan = native_region_light_target_plan(
            logical_view,
            device_resolution,
            NATIVE_DEFAULT_LIGHT_QUALITY,
        );
        let light_target = renderer.create_render_target(light_plan.physical_side)?;
        let shadows = self
            .document
            .layers
            .iter()
            .map(|layer| layer.shadow.as_ref().map(ShadowDocument::caster))
            .collect();
        let scrubs = self
            .document
            .layers
            .iter()
            .enumerate()
            .filter_map(|(index, layer)| {
                layer
                    .scrub
                    .as_ref()
                    .map(|scrub| (index, ScrubView::new(scrub, initial_stopped_world_tick)))
            })
            .collect();
        Ok(BoneyardView {
            document: self.document,
            textures,
            trees: NativeTreeOcclusionPresentation::new(trees, initial_tick),
            proxies,
            shadows,
            scrubs,
            light_target,
            light_logical_side: light_plan.logical_side,
            overlay_order: PainterOverlayOrder::default(),
            tree_frame_history: tree::TreeFrameHistory::default(),
            puppet_target_ids: self.puppet_target_ids,
            cleanup_applied: false,
            field_range: None,
        })
    }
}

pub struct BoneyardView {
    document: SceneryDocument,
    textures: BTreeMap<String, Texture>,
    trees: NativeTreeOcclusionPresentation,
    proxies: BTreeMap<String, usize>,
    shadows: Vec<Option<NativeShadowCaster>>,
    scrubs: BTreeMap<usize, ScrubView>,
    light_target: RenderTarget,
    light_logical_side: f64,
    overlay_order: PainterOverlayOrder,
    tree_frame_history: tree::TreeFrameHistory,
    puppet_target_ids: Vec<Option<String>>,
    cleanup_applied: bool,
    field_range: Option<NativeArenaFieldRange>,
}

pub struct BoneyardFrame<'a> {
    pub camera: &'a Camera,
    pub logical_view: Vec2,
    pub lights: &'a NativeBoneyardLightIndex,
    pub reference_y: f64,
    pub gates: &'a [GateLeaf],
    pub presentation_frame: u64,
    pub presentation_tick: f64,
    pub stopped_world_tick: f64,
    pub puppet_hits: &'a [EnemyPuppetHit],
    pub clocks: Option<&'a PresentationClocks>,
    pub tree_wobbles: &'a BTreeMap<String, f64>,
    pub cleanup_bounds: Option<BoneyardBounds>,
    pub enhanced_effects: bool,
    pub complex_lighting: bool,
    pub complex_shadows: bool,
    pub special_surface_at: &'a dyn Fn(Vec2) -> bool,
    pub goodies: &'a [BoneyardGoodieShadow<'a>],
}

pub struct BoneyardGoodieShadow<'a> {
    pub actor_index: usize,
    pub goodie: &'a solomon_sim::boneyard::loot::BoneyardGoodieState,
}

#[derive(Clone, Copy, Debug)]
pub enum BoneyardPass {
    PreWorld,
    LightComposite,
    Actor(usize),
    PostWorld,
}

impl BoneyardView {
    pub fn set_light_quality(
        &mut self,
        renderer: &Renderer,
        viewport: Vec2,
        resolution: f64,
        quality: f64,
    ) -> Result<(), String> {
        let plan = native_region_light_target_plan(viewport, resolution, quality);
        if plan.physical_side as f32 != self.light_target.texture.width
            || plan.logical_side != self.light_logical_side
        {
            let target = renderer.create_render_target(plan.physical_side)?;
            renderer.destroy_render_target(&self.light_target);
            self.light_target = target;
            self.light_logical_side = plan.logical_side;
        }
        Ok(())
    }

    pub(crate) fn advance_to(&mut self, source: &TreeFrameSource<'_>) {
        self.tree_frame_history.update(&mut self.trees, source);
    }

    /// Ground and the multiplied light field precede the row queue. Each proxy is admitted
    /// only when its owning tree or building paints, preserving the source's same-row ties.
    pub fn draw(
        &mut self,
        renderer: &mut Renderer,
        frame: &BoneyardFrame<'_>,
        actors: &ActorPainters<'_>,
        mut draw: impl FnMut(&mut Renderer, BoneyardPass),
    ) {
        if !self.cleanup_applied
            && let Some(bounds) = frame.cleanup_bounds
        {
            assert_eq!(
                self.document.cleanup.bounds.as_ref().map(NativeBoundsDocument::world_bounds),
                Some(bounds),
                "validated scenery cleanup domain matches the displayed world",
            );
            self.cleanup_applied = true;
        }
        self.update_field(frame);
        self.update_scrub_shadows(frame);
        renderer.set_saturation(0.65);
        self.draw_mesh(
            renderer,
            frame.camera,
            &self.document.ground,
            &self.textures["ground.png"],
            true,
        );
        for (index, road) in self.document.roads.iter().enumerate() {
            if self.cleanup_applied && self.document.cleanup.retired_roads.contains(&index) {
                continue;
            }
            self.draw_mesh(
                renderer,
                frame.camera,
                &road.mesh,
                &self.textures[ROAD_IMAGES[road.style]],
                false,
            );
        }
        for (index, layer) in self.document.layers.iter().enumerate() {
            if layer.band == SceneryBand::Base {
                self.draw_layer(renderer, frame, index);
            }
        }
        draw(renderer, BoneyardPass::PreWorld);
        if frame.complex_lighting {
            self.draw_light_field(renderer, frame);
        }
        draw(renderer, BoneyardPass::LightComposite);

        let scenery_count = self.document.layers.len();
        let gate_start = scenery_count;
        let actor_start = gate_start + frame.gates.len();
        let mut entries =
            Vec::with_capacity(scenery_count + frame.gates.len() + actors.layers.len());
        for (index, layer) in self.document.layers.iter().enumerate() {
            // Live Goodies retain this authored Scenery registration and supply their
            // own sack/damage art. Admitting the placeholder would duplicate the row.
            if layer.band != SceneryBand::Main
                || layer.type_id == Some(2061)
                || self.layer_retired(index)
            {
                continue;
            }
            let proxy = layer
                .object_id
                .as_ref()
                .filter(|_| layer.inserts_proxy)
                .and_then(|id| self.proxies.get(id))
                .map(|&index| {
                    let layer = &self.document.layers[index];
                    BoneyardPainterProxy {
                        draw_id: index,
                        world_y: layer.world_y,
                        sort_bias: layer.sort_bias,
                    }
                });
            entries.push(BoneyardPainterEntry {
                draw_id: index,
                layer: PainterLayer {
                    visible: true,
                    is_insertion: false,
                    manager_lane: PainterLane::Scenery,
                    registration_ordinal: layer.source_order,
                    world_y: layer.world_y,
                    sort_bias: layer.sort_bias,
                },
                proxy,
            });
        }
        // Gate bodies take their authored fence's registration slot. The exporter omits
        // the fixed gate body, leaving the two live leaves in its place.
        for (index, gate) in frame.gates.iter().enumerate() {
            let registration = self
                .document
                .gates
                .iter()
                .find(|entry| entry.fence_eid == gate.fence_eid && entry.side == gate.side)
                .expect("validated stock gate has an authored painter");
            entries.push(BoneyardPainterEntry {
                draw_id: gate_start + index,
                layer: PainterLayer {
                    visible: true,
                    is_insertion: false,
                    manager_lane: PainterLane::Scenery,
                    registration_ordinal: registration.source_order,
                    world_y: native_gate_art(gate.hinge, gate.tip).painter_y,
                    sort_bias: registration.sort_bias,
                },
                proxy: None,
            });
        }
        for (index, layer) in actors.layers.iter().enumerate() {
            entries.push(BoneyardPainterEntry {
                draw_id: actor_start + index,
                layer: *layer,
                proxy: None,
            });
        }
        let order = boneyard_painter_order(&entries, frame.reference_y);
        let mut positioned = Vec::with_capacity(order.len());
        let mut depth = 1.0;
        let mut static_band = false;
        for draw_id in order {
            let is_static =
                draw_id < scenery_count && self.document.layers[draw_id].band == SceneryBand::Main;
            if static_band && !is_static {
                depth += 1.0;
            }
            positioned.push((draw_id, depth));
            if !is_static {
                depth += 1.0;
            }
            static_band = is_static;
        }
        let overlays: Vec<_> = actors
            .owner_overlays
            .iter()
            .map(|overlay| PainterOwnerOverlay {
                draw_id: actor_start + overlay.draw_id,
                owner_draw_id: actor_start + overlay.owner_draw_id,
                view_id: overlay.view_id,
            })
            .collect();
        for draw_id in self.overlay_order.apply(&positioned, &overlays, actors.live_views) {
            if draw_id < gate_start {
                self.draw_layer(renderer, frame, draw_id);
            } else if draw_id < actor_start {
                self.draw_gate(renderer, frame, &frame.gates[draw_id - gate_start]);
            } else {
                let actor_index = draw_id - actor_start;
                if let Some(owner) =
                    frame.goodies.iter().find(|owner| owner.actor_index == actor_index)
                {
                    self.draw_goodie_shadow(renderer, frame, owner.goodie);
                }
                draw(renderer, BoneyardPass::Actor(actor_index));
            }
        }
        draw(renderer, BoneyardPass::PostWorld);
        renderer.set_saturation(1.0);
    }

    pub fn destroy(self, renderer: &Renderer) {
        for texture in self.textures.values() {
            renderer.destroy_texture(texture);
        }
        renderer.destroy_render_target(&self.light_target);
    }

    fn update_field(&mut self, frame: &BoneyardFrame<'_>) {
        let lattice = &self.document.field_lattice;
        let view = BoneyardBounds::new(
            frame.camera.origin.x,
            frame.camera.origin.y,
            frame.logical_view.x / frame.camera.scale,
            frame.logical_view.y / frame.camera.scale,
        );
        let range = native_arena_field_visible_range(lattice, view)
            .expect("active camera has finite field bounds");
        if self.field_range == Some(range) {
            return;
        }
        self.document.ground = MeshDocument::native(
            &native_arena_field_mesh_plan(lattice, range, self.document.field_mode)
                .expect("visible native field fits the checked view limit"),
        );
        self.field_range = Some(range);
    }

    fn draw_mesh(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        mesh: &MeshDocument,
        texture: &Texture,
        field_coverage: bool,
    ) {
        let pixel_scale = renderer.physical_pixel_scale();
        let triangles: Vec<_> = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|indices| {
                indices.map(|index| {
                    let point = Vec2::new(mesh.positions[index * 2], mesh.positions[index * 2 + 1]);
                    let mut position = project(camera, point);
                    let mut uv = [mesh.uvs[index * 2] as f32, mesh.uvs[index * 2 + 1] as f32];
                    if field_coverage {
                        let base = index / 4 * 4;
                        let dx = (mesh.positions[(base + 1) * 2] - mesh.positions[base * 2]) as f32;
                        let dy = (mesh.positions[(base + 2) * 2 + 1] - mesh.positions[base * 2 + 1])
                            as f32;
                        let slopes = [
                            ((mesh.uvs[(base + 1) * 2] - mesh.uvs[base * 2]) / f64::from(dx))
                                as f32,
                            ((mesh.uvs[(base + 2) * 2 + 1] - mesh.uvs[base * 2 + 1])
                                / f64::from(dy)) as f32,
                        ];
                        for axis in 0..2 {
                            let device = position[axis] * pixel_scale[axis];
                            let offset = device.ceil() - device - 0.5;
                            // Source installs a half-device-pixel projection, then
                            // applies FIELD_COVERAGE_BIT. Its resulting edge is ceil.
                            position[axis] += (offset + 0.5) / pixel_scale[axis];
                            uv[axis] +=
                                offset * slopes[axis] / (camera.scale as f32 * pixel_scale[axis]);
                        }
                    }
                    Vertex {
                        position,
                        texel: [uv[0] * texture.width, uv[1] * texture.height],
                        tint: [
                            mesh.colors[index * 4] as f32 / 255.0,
                            mesh.colors[index * 4 + 1] as f32 / 255.0,
                            mesh.colors[index * 4 + 2] as f32 / 255.0,
                            mesh.colors[index * 4 + 3] as f32 / 255.0,
                        ],
                    }
                })
            })
            .collect();
        renderer.draw_triangles(texture, &triangles, BlendMode::Normal);
    }

    fn draw_light_field(&self, renderer: &mut Renderer, frame: &BoneyardFrame<'_>) {
        let glyph = &self.textures[&self.document.sprites["light-glyph"].image];
        let anchor = self.document.sprites["light-glyph"].anchor;
        let side = self.light_logical_side;
        let origin = Vec2::new(fround(frame.camera.origin.x), fround(frame.camera.origin.y));
        renderer.render_to(&self.light_target, [side as f32; 2], |renderer| {
            for light in frame.lights.accepted_sources() {
                let scale = light.raster_scale.unwrap_or(light.radius);
                renderer.draw(
                    glyph,
                    Quad {
                        source: [0.0, 0.0, glyph.width, glyph.height],
                        destination: [
                            (light.position.x - origin.x - anchor[0] * scale) as f32,
                            (light.position.y - origin.y - anchor[1] * scale) as f32,
                            glyph.width * scale as f32,
                            glyph.height * scale as f32,
                        ],
                        tint: [1.0, 1.0, 1.0, light.intensity as f32],
                        blend: BlendMode::Normal,
                    },
                );
            }
        });
        let texture = &self.light_target.texture;
        renderer.draw(
            texture,
            Quad {
                source: [0.0, texture.height, texture.width, -texture.height],
                destination: frame.camera.project(origin.x, origin.y, side, side),
                tint: WHITE,
                blend: BlendMode::Multiply,
            },
        );
    }

    fn draw_layer(&self, renderer: &mut Renderer, frame: &BoneyardFrame<'_>, index: usize) {
        if self.layer_retired(index) {
            return;
        }
        let layer = &self.document.layers[index];
        if layer.initial_alpha == 0.0 || !layer.bounds.visible(frame) {
            return;
        }
        let tree = layer.tree_id.as_ref().and_then(|id| self.trees.get(id));
        let alpha = tree.map_or(layer.initial_alpha, |tree| tree.current_alpha);
        self.draw_layer_shadows(renderer, frame, index, alpha);
        if frame.complex_lighting
            && frame.complex_shadows
            && let Some(caster) = &self.shadows[index]
        {
            draw_shadow_edges(
                renderer,
                frame.camera,
                &native_shadow_edges(caster, frame.lights, frame.presentation_frame),
            );
        }
        let material = SceneryMaterial { alpha, tint: 0xffffff, diffuse: false, hit_alpha: None };
        self.draw_layer_art(renderer, frame, index, material);
        let Some(target_id) = &self.puppet_target_ids[index] else { return };
        let Some(hit) = frame
            .puppet_hits
            .iter()
            .find(|hit| hit.kind == EnemyPuppetHitKind::Scenery && &hit.target_id == target_id)
        else {
            return;
        };
        let timer = frame
            .clocks
            .and_then(|clocks| clocks.puppet_hit_timers.get(target_id))
            .copied()
            .unwrap_or_else(|| hit.feedback.timer_at(frame.presentation_tick));
        if timer <= 0.0 {
            return;
        }
        let building =
            matches!(&layer.surface_lighting, Some(SurfaceLightingDocument::Building { .. }));
        let hit_alpha = frame
            .clocks
            .and_then(|clocks| clocks.puppet_hits.get(target_id))
            .copied()
            .unwrap_or_else(|| hit.feedback.alpha(frame.presentation_tick));
        let redraw = scenery_hit_redraw(alpha, hit_alpha, frame.complex_lighting, building);
        if layer.band == SceneryBand::Proxy {
            // Main enqueues a second ordinary upper proxy, without the body's hit RGBA.
            if redraw.proxy_visible {
                self.draw_layer_art(renderer, frame, index, material);
            }
            return;
        }
        if layer.band != SceneryBand::Main {
            return;
        }
        self.draw_layer_art(renderer, frame, index, redraw.body);
    }

    fn draw_layer_art(
        &self,
        renderer: &mut Renderer,
        frame: &BoneyardFrame<'_>,
        index: usize,
        material: SceneryMaterial,
    ) {
        let layer = &self.document.layers[index];
        let tree = layer.tree_id.as_ref().and_then(|id| self.trees.get(id));
        let light_position =
            tree.map(|tree| tree.position).or_else(|| layer.light_position.map(Into::into));
        let factors = if !frame.complex_lighting || layer.band == SceneryBand::Base {
            solomon_sim::boneyard::lighting::NativeBoneyardLightFactors {
                radial: 1.0,
                elevated: 1.0,
            }
        } else {
            light_position.map_or(
                solomon_sim::boneyard::lighting::NativeBoneyardLightFactors {
                    radial: 1.0,
                    elevated: 1.0,
                },
                |position| {
                    frame.lights.factors_at(
                        position,
                        NativeBoneyardLightQuery {
                            camera_origin: frame.camera.origin,
                            ambient: 0.0,
                        },
                    )
                },
            )
        };
        let scalar = fround(factors.radial * factors.elevated);
        if let Some(surface) = &layer.surface_lighting {
            self.draw_surface(
                renderer,
                frame,
                layer,
                surface,
                material,
                &self.textures[&layer.image],
            );
            return;
        }
        let cleaned = if self.cleanup_applied {
            self.document
                .cleanup
                .base_images
                .iter()
                .find(|base| base.layer == index)
                .map(|base| base.image.as_str())
        } else {
            None
        };
        let alpha = tree.map_or(layer.initial_alpha, |tree| tree.current_alpha);
        let colors = if let Some(kind) = layer.glyph_material {
            let callback = if let Some(hit_alpha) = material.hit_alpha {
                let light = if frame.complex_lighting { factors.radial } else { 1.0 };
                [
                    fround((if frame.complex_lighting { 0.65_f32 as f64 } else { 1.0 }) * light),
                    0.0,
                    0.0,
                    fround(hit_alpha * light),
                ]
            } else {
                let light = fround(scalar).clamp(0.0, 1.0);
                [light, light, light, 1.0]
            };
            scenery_glyph_colors((kind == SceneryGlyphMaterial::Tree).then_some(alpha), callback)
        } else {
            [material.rgba(scalar); 4]
        };
        let mut translation = Vec2::ZERO;
        if layer.band == SceneryBand::Proxy
            && let Some(tree) = tree
        {
            let position = native_tree_secondary_position(
                tree.position,
                BoneyardBounds::new(
                    frame.camera.origin.x,
                    frame.camera.origin.y,
                    frame.logical_view.x / frame.camera.scale,
                    frame.logical_view.y / frame.camera.scale,
                ),
            );
            translation = Vec2::new(position.x - tree.position.x, position.y - tree.position.y);
        }
        for draw in &layer.draws {
            let texture = draw.image.as_ref().map(|image| &self.textures[cleaned.unwrap_or(image)]);
            let mut points: Vec<Vec2> = draw.mesh.points().collect();
            if let Some(tree) = tree
                && let Some(phase) =
                    layer.tree_id.as_ref().and_then(|id| frame.tree_wobbles.get(id))
            {
                let angle = phase * std::f64::consts::PI / 180.0;
                let sine = solomon_sim::float::sin(angle);
                let cosine = solomon_sim::float::cos(angle);
                for point in &mut points {
                    let x = point.x - tree.position.x;
                    let y = point.y - tree.position.y;
                    point.x = tree.position.x + x * cosine - y * sine;
                    point.y = tree.position.y + x * sine + y * cosine;
                }
            }
            let triangles: Vec<_> = draw
                .mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|indices| {
                    indices.map(|index| {
                        let point = points[index];
                        let color: [f32; 4] = std::array::from_fn(|channel| {
                            f32::from(draw.mesh.colors[index * 4 + channel]) / 255.0
                        });
                        let inherited = colors[index % 4];
                        Vertex {
                            position: project(
                                frame.camera,
                                Vec2::new(point.x + translation.x, point.y + translation.y),
                            ),
                            texel: texture.map_or([0.0; 2], |texture| {
                                [
                                    draw.mesh.uvs[index * 2] as f32 * texture.width,
                                    draw.mesh.uvs[index * 2 + 1] as f32 * texture.height,
                                ]
                            }),
                            tint: [
                                inherited[0] * color[0],
                                inherited[1] * color[1],
                                inherited[2] * color[2],
                                inherited[3] * color[3],
                            ],
                        }
                    })
                })
                .collect();
            if let Some(texture) = texture {
                renderer.draw_triangles_with_color_mode(
                    texture,
                    &triangles,
                    BlendMode::Normal,
                    material.color_mode(),
                );
            } else {
                renderer.draw_solid_triangles(&triangles, BlendMode::Normal);
            }
        }
    }

    fn draw_surface(
        &self,
        renderer: &mut Renderer,
        frame: &BoneyardFrame<'_>,
        layer: &LayerDocument,
        surface: &SurfaceLightingDocument,
        material: SceneryMaterial,
        texture: &Texture,
    ) {
        let (side, scalars) = match surface {
            SurfaceLightingDocument::Building { sample_points, sample_points_coarse } => {
                let sample_points =
                    if frame.enhanced_effects { sample_points } else { sample_points_coarse };
                let scalars: Vec<_> = sample_points
                    .iter()
                    .map(|point| {
                        if frame.complex_lighting {
                            fround(frame.lights.surface_scalar_with_query(
                                (*point).into(),
                                NativeBoneyardLightQuery {
                                    camera_origin: frame.camera.origin,
                                    ambient: 0.0,
                                },
                            ))
                        } else {
                            1.0
                        }
                    })
                    .collect();
                ((sample_points.len() as f64).sqrt() as usize, scalars)
            }
            SurfaceLightingDocument::Wall { start, end, vertex_weights } => {
                let start_scalar = if frame.complex_lighting {
                    frame.lights.surface_scalar_with_query(
                        (*start).into(),
                        NativeBoneyardLightQuery {
                            camera_origin: frame.camera.origin,
                            ambient: 0.0,
                        },
                    )
                } else {
                    1.0
                };
                let end_scalar = if frame.complex_lighting {
                    frame.lights.surface_scalar_with_query(
                        (*end).into(),
                        NativeBoneyardLightQuery {
                            camera_origin: frame.camera.origin,
                            ambient: 0.0,
                        },
                    )
                } else {
                    1.0
                };
                (
                    2,
                    vertex_weights
                        .iter()
                        .map(|weight| fround(start_scalar + (end_scalar - start_scalar) * weight))
                        .collect(),
                )
            }
        };
        let mut vertices = Vec::with_capacity(side * side);
        for row in 0..side {
            for column in 0..side {
                let u = column as f64 / (side - 1) as f64;
                let v = row as f64 / (side - 1) as f64;
                vertices.push(Vertex {
                    position: project(
                        frame.camera,
                        Vec2::new(
                            layer.bounds.x + fround(layer.bounds.width * u),
                            layer.bounds.y + fround(layer.bounds.height * v),
                        ),
                    ),
                    texel: [texture.width * u as f32, texture.height * v as f32],
                    tint: material.rgba(scalars[row * side + column]),
                });
            }
        }
        let mut triangles = Vec::new();
        for row in 0..side - 1 {
            for column in 0..side - 1 {
                let index = row * side + column;
                triangles.push([vertices[index], vertices[index + 1], vertices[index + side]]);
                triangles.push([
                    vertices[index + 1],
                    vertices[index + side],
                    vertices[index + side + 1],
                ]);
            }
        }
        renderer.draw_triangles_with_color_mode(
            texture,
            &triangles,
            BlendMode::Normal,
            material.color_mode(),
        );
    }

    fn layer_retired(&self, index: usize) -> bool {
        self.cleanup_applied && self.document.cleanup.retired_layers.contains(&index)
    }

    fn draw_gate(&self, renderer: &mut Renderer, frame: &BoneyardFrame<'_>, gate: &GateLeaf) {
        let art = native_gate_art(gate.hinge, gate.tip);
        let midpoint =
            Vec2::new((gate.hinge.x + gate.tip.x) / 2.0, (gate.hinge.y + gate.tip.y) / 2.0);
        let root = if gate.tip.y < midpoint.y { midpoint } else { gate.tip };
        if frame.complex_lighting && frame.complex_shadows {
            let caster = NativeShadowCaster {
                id: format!("gate:{}:{}", gate.fence_eid, gate.side),
                position: root,
                outline: Vec::new(),
                program: Some(NativeShadowProgram::FenceGrate {
                    start: gate.hinge,
                    end: gate.tip,
                    gate: true,
                }),
            };
            draw_shadow_edges(
                renderer,
                frame.camera,
                &native_shadow_edges(&caster, frame.lights, frame.presentation_frame),
            );
        }
        let shade = packed_light_scalar(if frame.complex_lighting {
            let factors = frame.lights.factors_at(
                root,
                NativeBoneyardLightQuery { camera_origin: frame.camera.origin, ambient: 0.0 },
            );
            fround(factors.radial * factors.elevated)
        } else {
            1.0
        });
        let tint = [shade, shade, shade, 1.0];
        let leaf = &self.textures[&self.document.sprites["gate-leaf"].image];
        renderer.draw_quad(
            leaf,
            [0.0, 0.0, leaf.width, leaf.height],
            [art.vertices[0], art.vertices[1], art.vertices[3], art.vertices[2]]
                .map(|point| project(frame.camera, point)),
            [tint; 4],
            BlendMode::Normal,
        );
        for [start, end] in art.lines {
            draw_shadow_edges(renderer, frame.camera, &[native_shadow_line(start, end, 3.0, 1.0)]);
        }
        let sprite = &self.document.sprites["gate-hinge"];
        let hinge = &self.textures[&sprite.image];
        renderer.draw(
            hinge,
            Quad {
                source: [0.0, 0.0, hinge.width, hinge.height],
                destination: frame.camera.project(
                    art.hinge_sprite.x - sprite.anchor[0],
                    art.hinge_sprite.y - sprite.anchor[1],
                    f64::from(hinge.width),
                    f64::from(hinge.height),
                ),
                tint,
                blend: BlendMode::Normal,
            },
        );
    }
}

fn scenery_glyph_colors(tree_alpha: Option<f64>, callback: [f64; 4]) -> [[f32; 4]; 4] {
    let mut top = callback.map(fround);
    let bottom = top;
    if let Some(alpha) = tree_alpha.map(fround).filter(|alpha| *alpha < 0.5) {
        top[3] = fround(alpha * 0.5);
    }
    let pack = |value: f64| fround(value * 255.0).trunc() as f32 / 255.0;
    let top = top.map(pack);
    let bottom = bottom.map(pack);
    [top, top, bottom, bottom]
}

#[derive(Clone, Copy, PartialEq)]
enum SceneryGlyphMaterial {
    Ordinary,
    Tree,
}

enum PreparedImage {
    Image(HtmlImageElement),
    Pixels { width: u32, height: u32, pixels: Vec<u8> },
}

struct LayerDrawDocument {
    image: Option<String>,
    mesh: MeshDocument,
}

#[cfg(test)]
fn tree_wobble_corners(bounds: [f64; 4], position: Vec2, phase: f64) -> [Vec2; 4] {
    let [x, y, width, height] = bounds;
    let angle = phase * std::f64::consts::PI / 180.0;
    let (sine, cosine) = angle.sin_cos();
    let pivot_x = position.x - x;
    let pivot_y = position.y - y;
    let translation_x = position.x - (pivot_x * cosine + pivot_y * -sine);
    let translation_y = position.y - (pivot_x * sine + pivot_y * cosine);
    [[0.0, 0.0], [width, 0.0], [width, height], [0.0, height]].map(|[x, y]| {
        Vec2::new(cosine * x + -sine * y + translation_x, sine * x + cosine * y + translation_y)
    })
}

fn packed_light_scalar(value: f64) -> f32 {
    fround(fround(value.clamp(0.0, 1.0)) * 255.0).trunc() as f32 / 255.0
}

fn packed_scalar(value: f64) -> f32 {
    (value.clamp(0.0, 1.0) * 255.0).trunc() as f32 / 255.0
}

fn project(camera: &Camera, point: Vec2) -> [f32; 2] {
    [
        ((point.x - camera.origin.x) * camera.scale) as f32,
        ((point.y - camera.origin.y) * camera.scale) as f32,
    ]
}

fn draw_shadow_edges(renderer: &mut Renderer, camera: &Camera, edges: &[NativeShadowEdge]) {
    let mut triangles = Vec::with_capacity(edges.len() * 2);
    for edge in edges {
        let vertex = |position, alpha| Vertex {
            position: project(camera, position),
            texel: [0.0; 2],
            tint: [0.0, 0.0, 0.0, packed_scalar(alpha)],
        };
        let vertices = [
            vertex(edge.base_start, edge.base_alpha),
            vertex(edge.base_end, edge.base_alpha),
            vertex(edge.tip_start, edge.tip_alpha),
            vertex(edge.tip_end, edge.tip_alpha),
        ];
        triangles.push([vertices[0], vertices[1], vertices[2]]);
        triangles.push([vertices[1], vertices[3], vertices[2]]);
    }
    renderer.draw_solid_triangles(&triangles, BlendMode::Normal);
}

struct SceneryDocument {
    layers: Vec<LayerDocument>,
    gates: Vec<GateDocument>,
    tree_inputs: Vec<TreeDocument>,
    ground: MeshDocument,
    field_lattice: NativeArenaFieldLattice,
    field_mode: u8,
    roads: Vec<RoadDocument>,
    sprites: BTreeMap<String, SpriteDocument>,
    shadow_glyphs: BTreeMap<usize, SceneryShadowGlyph>,
    goodie_shadow_outline: Vec<Vec2>,
    cleanup: CleanupDocument,
}

struct CleanupDocument {
    bounds: Option<NativeBoundsDocument>,
    retired_layers: BTreeSet<usize>,
    retired_roads: BTreeSet<usize>,
    base_images: Vec<CleanedBaseImage>,
}

struct CleanedBaseImage {
    layer: usize,
    image: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct PointDocument {
    x: f64,
    y: f64,
}

impl From<PointDocument> for Vec2 {
    fn from(point: PointDocument) -> Self {
        Self::new(point.x, point.y)
    }
}

#[derive(Clone, Debug)]
struct BoundsDocument {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<Vec2> for PointDocument {
    fn from(point: Vec2) -> Self {
        Self { x: point.x, y: point.y }
    }
}

impl From<BoneyardBounds> for NativeBoundsDocument {
    fn from(bounds: BoneyardBounds) -> Self {
        Self { x: bounds.x, y: bounds.y, w: bounds.width, h: bounds.height }
    }
}

impl BoundsDocument {
    fn world_bounds(&self) -> BoneyardBounds {
        BoneyardBounds::new(self.x, self.y, self.width, self.height)
    }

    fn points(points: &[Vec2]) -> Self {
        let bounds = solomon_sim::boneyard::scenery::native_scenery_points_bounds(points)
            .expect("a scenery primitive has vertices");
        Self { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }
    }

    fn overlaps_inclusive(&self, bounds: BoneyardBounds) -> bool {
        self.x <= bounds.x + bounds.width
            && self.y <= bounds.y + bounds.height
            && self.x + self.width >= bounds.x
            && self.y + self.height >= bounds.y
    }
    fn visible(&self, frame: &BoneyardFrame<'_>) -> bool {
        self.x + self.width >= frame.camera.origin.x - 32.0
            && self.y + self.height >= frame.camera.origin.y - 32.0
            && self.x <= frame.camera.origin.x + frame.logical_view.x / frame.camera.scale + 32.0
            && self.y <= frame.camera.origin.y + frame.logical_view.y / frame.camera.scale + 32.0
    }
}

#[derive(PartialEq)]
enum SceneryBand {
    Base,
    Main,
    Proxy,
}

struct LayerDocument {
    type_id: Option<u32>,
    image: String,
    bounds: BoundsDocument,
    band: SceneryBand,
    world_y: f64,
    sort_bias: f64,
    source_order: usize,
    inserts_proxy: bool,
    object_id: Option<String>,
    tree_id: Option<String>,
    initial_alpha: f64,
    light_position: Option<PointDocument>,
    surface_lighting: Option<SurfaceLightingDocument>,
    shadow: Option<ShadowDocument>,
    shadow_plans: SceneryShadowPlans,
    scrub: Option<ScrubDocument>,
    draws: Vec<LayerDrawDocument>,
    source_key: Option<String>,
    puppet_target_id: Option<String>,
    glyph_material: Option<SceneryGlyphMaterial>,
}

enum SurfaceLightingDocument {
    Building { sample_points: Vec<PointDocument>, sample_points_coarse: Vec<PointDocument> },
    Wall { start: PointDocument, end: PointDocument, vertex_weights: Vec<f64> },
}

struct SpriteDocument {
    image: String,
    anchor: [f64; 2],
}

struct GateDocument {
    fence_eid: String,
    side: u8,
    source_order: usize,
    sort_bias: f64,
}

#[derive(Clone, Copy, Deserialize)]
struct NativeBoundsDocument {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl NativeBoundsDocument {
    fn world_bounds(&self) -> BoneyardBounds {
        BoneyardBounds::new(self.x, self.y, self.w, self.h)
    }
}

struct TreeDocument {
    eid: String,
    position: PointDocument,
    bounds: NativeBoundsDocument,
    polygon: Vec<PointDocument>,
}

#[derive(Clone)]
struct MeshDocument {
    positions: Vec<f64>,
    uvs: Vec<f64>,
    colors: Vec<u8>,
    indices: Vec<usize>,
}

impl MeshDocument {
    fn native(mesh: &NativeBoneyardSurfaceMesh) -> Self {
        Self {
            positions: mesh.positions.iter().flat_map(|point| [point.x, point.y]).collect(),
            uvs: mesh.uvs.iter().flat_map(|point| [point.x, point.y]).collect(),
            colors: mesh.colors.iter().flatten().copied().collect(),
            indices: mesh.indices.clone(),
        }
    }

    fn quad(corners: [Vec2; 4], uvs: [[f64; 2]; 4]) -> Self {
        Self {
            positions: corners
                .iter()
                .flat_map(|point| [fround(point.x), fround(point.y)])
                .collect(),
            uvs: uvs.into_iter().flatten().map(fround).collect(),
            colors: vec![255; 16],
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }

    fn line(start: Vec2, end: Vec2, width: f64) -> Self {
        let edge = native_shadow_line(start, end, width, 1.0);
        let mut mesh = Self::quad(
            [edge.base_start, edge.base_end, edge.tip_end, edge.tip_start],
            [[0.0; 2]; 4],
        );
        for color in mesh.colors.as_chunks_mut::<4>().0 {
            color.copy_from_slice(&[0, 0, 0, 255]);
        }
        mesh
    }

    fn points(&self) -> impl Iterator<Item = Vec2> + '_ {
        self.positions.as_chunks::<2>().0.iter().map(|point| Vec2::new(point[0], point[1]))
    }
}

struct RoadDocument {
    style: usize,
    mesh: MeshDocument,
}

struct ShadowDocument {
    id: String,
    position: PointDocument,
    outline: Vec<PointDocument>,
    program: Option<ShadowProgramDocument>,
}

impl ShadowDocument {
    fn caster(&self) -> NativeShadowCaster {
        NativeShadowCaster {
            id: self.id.clone(),
            position: self.position.into(),
            outline: self.outline.iter().copied().map(Into::into).collect(),
            program: self.program.as_ref().map(|program| match program {
                ShadowProgramDocument::BrokenFenceGrate { start, end, step, count } => {
                    NativeShadowProgram::BrokenFenceGrate {
                        start: (*start).into(),
                        end: (*end).into(),
                        step: (*step).into(),
                        count: *count,
                    }
                }
                ShadowProgramDocument::FenceGrate { start, end, construction } => {
                    NativeShadowProgram::FenceGrate {
                        start: (*start).into(),
                        end: (*end).into(),
                        gate: construction == "gate",
                    }
                }
                ShadowProgramDocument::Rails { start, end } => {
                    NativeShadowProgram::Rails { start: (*start).into(), end: (*end).into() }
                }
                ShadowProgramDocument::Wall { start, end } => {
                    NativeShadowProgram::Wall { start: (*start).into(), end: (*end).into() }
                }
            }),
        }
    }
}

enum ShadowProgramDocument {
    BrokenFenceGrate { start: PointDocument, end: PointDocument, step: PointDocument, count: usize },
    FenceGrate { start: PointDocument, end: PointDocument, construction: String },
    Rails { start: PointDocument, end: PointDocument },
    Wall { start: PointDocument, end: PointDocument },
}

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::combat::PuppetHitState;

    #[test]
    fn current_original_glyph_colors_match_source_tree_and_hit_branches() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../sim/tests/fixtures/website/scenery-material-goldens.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let callback = std::array::from_fn(|index| case["callback"][index].as_f64().unwrap());
            let actual = scenery_glyph_colors(case["treeAlpha"].as_f64(), callback);
            for (actual, packed) in actual.iter().zip(case["colors"].as_array().unwrap()) {
                let packed = packed.as_u64().unwrap() as u32;
                let expected =
                    std::array::from_fn(|channel| ((packed >> (channel * 8)) & 255) as f32 / 255.0);
                assert_eq!(*actual, expected);
            }
        }
    }

    #[test]
    fn scenery_redraws_and_tree_wobble_match_actual_source_views() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../sim/tests/fixtures/website/scenery-views-goldens.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input = &case["input"];
            let number = |key| input[key].as_f64().unwrap();
            let feedback =
                PuppetHitState { strength: number("strength"), tick: 120, timer: number("timer") };
            let tick = number("tick");
            assert_eq!(feedback.timer_at(tick), case["hitTimer"].as_f64().unwrap());
            let alpha = feedback.alpha(tick);
            assert_eq!(alpha, case["hitAlpha"].as_f64().unwrap());
            let building = input["kind"] == "building";
            let parent_alpha = number("parentAlpha");
            let redraw = scenery_hit_redraw(
                parent_alpha,
                alpha,
                input["complex"].as_bool().unwrap(),
                building,
            );
            let body = &case["body"];
            assert_eq!(redraw.body.alpha, body["alpha"].as_f64().unwrap() * parent_alpha);
            assert_eq!(redraw.body.tint, body["tint"].as_u64().unwrap() as u32);
            assert_eq!(redraw.body.diffuse, body["diffuse"].as_bool().unwrap());
            assert_eq!(feedback.timer_at(tick) > 0.0, body["visible"].as_bool().unwrap());
            let has_proxy = building || input["kind"] == "tree";
            assert_eq!(
                has_proxy && feedback.timer_at(tick) > 0.0 && redraw.proxy_visible,
                case["proxyVisible"].as_bool().unwrap(),
            );
            let scalar = if building { fround(number("scalar")) } else { number("scalar") };
            for (actual, expected) in
                redraw.body.rgba(scalar).iter().zip(body["rgba"].as_array().unwrap())
            {
                assert_eq!(*actual, expected.as_f64().unwrap() as f32);
            }
        }
        for case in fixture["wobbles"].as_array().unwrap() {
            let bounds = std::array::from_fn(|index| case["bounds"][index].as_f64().unwrap());
            let position = Vec2::new(
                case["position"]["x"].as_f64().unwrap(),
                case["position"]["y"].as_f64().unwrap(),
            );
            let actual = tree_wobble_corners(bounds, position, case["phase"].as_f64().unwrap());
            for (actual, expected) in actual.iter().zip(case["corners"].as_array().unwrap()) {
                assert!((actual.x - expected["x"].as_f64().unwrap()).abs() < 1.0e-10);
                assert!((actual.y - expected["y"].as_f64().unwrap()).abs() < 1.0e-10);
            }
        }
    }
}
