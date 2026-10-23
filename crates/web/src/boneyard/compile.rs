//! Runtime `buildStaticWorld` composition. Source references are
//! `boneyard-static-world.ts`, `boneyard-native-static-art.ts`, and
//! `boneyard-off-camera-cleanup.ts`; scene hashes identify data, never art files.

use super::catalog::SceneryCatalog;
use super::*;
use solomon_graphics::assets;
use solomon_sim::Vec2;
use solomon_sim::boneyard::scenery::{
    NativeSceneryLayer, NativeSceneryPlan, NativeSceneryShadowTexture, NativeScenerySource,
    native_fence_grate, native_scenery_object_entry, native_scenery_plan,
    native_scenery_shadow_plan, native_wall_skirt_plan,
};
use solomon_sim::boneyard::surface::{
    create_native_arena_field_lattice, native_arena_field_mesh_plan, native_road_mesh_plan,
};
use solomon_sim::boneyard::{BoneyardBounds, BoneyardScene, LoadedBoneyard};
use solomon_sim::float::{fround, hypot};
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn load(loaded: &LoadedBoneyard) -> Result<PreparedBoneyardView, String> {
    let url = format!("{ASSET_ROOT}/catalog.json");
    let catalog: SceneryCatalog = serde_json::from_slice(&assets::fetch_bytes(&url).await?)
        .map_err(|error| format!("{url}: {error}"))?;
    let scene = &loaded.scene;
    let plan = native_scenery_plan(scene, &|entry| catalog.record(entry).is_none());
    let cleanup_bounds = cleanup_bounds(loaded);
    let mut compiler = SceneCompiler {
        catalog: &catalog,
        scene,
        plan: &plan,
        images: BTreeMap::new(),
        layers: Vec::new(),
        visual_bounds: BTreeMap::new(),
    };

    // Only generated Terrain pixels enter tiles. Original glyphs retain their
    // own source registration and direct GPU art; Walls have independent grids.
    compiler.load_surface_images().await?;
    let tiles = canvas::terrain_tiles(scene, &compiler.images, &BTreeSet::new()).await?;
    let mut base_tiles = BTreeMap::new();
    for (tile_id, painted) in tiles {
        let name = format!("generated:terrain:{tile_id}");
        compiler.images.insert(name.clone(), painted.image);
        base_tiles.insert(tile_id, compiler.layers.len());
        compiler.layers.push(canvas_layer(name, painted.bounds));
    }
    for &(index, entry) in &plan.underlays {
        let object = &scene.objects[index];
        compiler.base_art(
            entry,
            object.position,
            0.0,
            Vec2::new(1.0, 1.0),
            1.0,
            format!("object:{}", object.eid),
        )?;
    }
    for &(index, entry) in &plan.compact {
        let sprite = &scene.sprites[index];
        let scale = sprite.s1.max(0.0);
        let alpha =
            if (25..=28).contains(&sprite.atlas_entry) { 0.75 } else { sprite.s2.clamp(0.0, 1.0) };
        compiler.base_art(
            entry,
            sprite.position,
            sprite.s0,
            Vec2::new(scale * if sprite.flags & 1 == 1 { 0.8 } else { 1.0 }, scale),
            alpha,
            format!("sprite:{}", sprite.eid),
        )?;
    }
    for (index, layer) in plan.walls.iter().enumerate() {
        let NativeScenerySource::FenceBody { index: fence_index, .. } = layer.source else {
            unreachable!()
        };
        let fence = &scene.fences[fence_index];
        let Some(painted) = canvas::wall(fence)? else {
            continue;
        };
        let name = format!("generated:wall:{fence_index}");
        compiler.images.insert(name.clone(), painted.image);
        let mut resident = canvas_layer(name, painted.bounds);
        resident.light_position = Some(layer.position.into());
        let shadow = compiler.shadow(layer, plan.main.len() + index)?;
        if let Some(ShadowDocument {
            program: Some(ShadowProgramDocument::Wall { start, end }),
            ..
        }) = &shadow
        {
            let start: Vec2 = (*start).into();
            let end: Vec2 = (*end).into();
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let length_squared = dx * dx + dy * dy;
            let bounds = &resident.bounds;
            let weights = [
                [bounds.x, bounds.y],
                [bounds.x + bounds.width, bounds.y],
                [bounds.x, bounds.y + bounds.height],
                [bounds.x + bounds.width, bounds.y + bounds.height],
            ]
            .map(|[x, y]| {
                if length_squared == 0.0 {
                    0.0
                } else {
                    fround(((x - start.x) * dx + (y - start.y) * dy) / length_squared)
                        .clamp(0.0, 1.0)
                }
            });
            resident.surface_lighting = Some(SurfaceLightingDocument::Wall {
                start: start.into(),
                end: end.into(),
                vertex_weights: weights.to_vec(),
            });
            let skirt = native_wall_skirt_plan(start, end);
            resident.shadow_plans = SceneryShadowPlans { off: vec![skirt], on: vec![skirt] };
        }
        resident.shadow = shadow;
        compiler.layers.push(resident);
    }
    let mut gates = Vec::new();
    for (index, layer) in plan.main.iter().enumerate() {
        if let NativeScenerySource::FenceBody { index: fence_index, piece, .. } = layer.source
            && scene.fences[fence_index].code() == 2
        {
            gates.push(GateDocument {
                fence_eid: scene.fences[fence_index].eid.clone(),
                side: piece as u8,
                source_order: layer.source_order,
                sort_bias: layer.sort_bias,
            });
            continue;
        }
        if let Some(resident) = compiler.main(layer, index)? {
            compiler.layers.push(resident);
        }
    }
    let mut trees = Vec::new();
    for layer in &plan.proxies {
        let NativeScenerySource::Object { index, entry } = layer.source else { unreachable!() };
        let object = &scene.objects[index];
        let Some(mut resident) =
            compiler.glyph(entry, object.position, 0.0, Vec2::new(1.0, 1.0), 1.0)
        else {
            continue;
        };
        resident.band = SceneryBand::Proxy;
        resident.world_y = layer.world_y;
        resident.source_order = layer.source_order;
        resident.object_id = Some(object.eid.clone());
        resident.source_key = Some(format!("object:{}", object.eid));
        resident.light_position = Some(object.position.into());
        if object.type_id == 2001 {
            resident.tree_id = Some(object.eid.clone());
            let variant =
                object.secondary_variant.unwrap_or(entry.saturating_sub(243) as u32) as usize;
            trees.push(TreeDocument {
                eid: object.eid.clone(),
                position: object.position.into(),
                bounds: catalog
                    .tree_bounds
                    .get(variant)
                    .cloned()
                    .ok_or("Tree secondary bounds are unavailable")?,
                polygon: catalog
                    .tree_polygons
                    .get(variant)
                    .cloned()
                    .ok_or("Tree secondary polygon is unavailable")?,
            });
        }
        if object.type_id == 2040 {
            resident.surface_lighting = Some(compiler.building_grid(index)?);
        }
        compiler.merge_bounds(&resident);
        compiler.layers.push(resident);
    }

    // Cleanup decides membership from each complete source, including all
    // composite pieces. Repaint Terrain from the same source after retirement.
    let retired = cleanup_bounds.map(|bounds| compiler.retired_sources(bounds)).unwrap_or_default();
    let retired_layers = compiler
        .layers
        .iter()
        .enumerate()
        .filter_map(|(index, layer)| {
            layer.source_key.as_ref().filter(|key| retired.contains(*key)).map(|_| index)
        })
        .collect();
    let retired_roads = scene
        .roads
        .iter()
        .enumerate()
        .filter_map(|(index, road)| {
            retired.contains(&format!("road:{}", road.eid)).then_some(index)
        })
        .collect();
    let mut base_images = Vec::new();
    if !retired.is_empty() && cleanup_bounds.is_some() {
        let cleaned: BTreeMap<_, _> =
            canvas::terrain_tiles(scene, &compiler.images, &retired).await?.into_iter().collect();
        for (tile_id, layer_index) in base_tiles {
            let name = format!("generated:terrain-clean:{tile_id}");
            // A retired tile can be wholly empty. Keep its existing rectangle
            // transparent so the source base painter can clear its old pixels.
            let image = if let Some(painted) = cleaned.get(&tile_id) {
                let original = &compiler.layers[layer_index].bounds;
                expand_paint(painted, original)
            } else {
                let bounds = &compiler.layers[layer_index].bounds;
                PreparedImage::Pixels {
                    width: bounds.width as u32,
                    height: bounds.height as u32,
                    pixels: vec![0; bounds.width as usize * bounds.height as usize * 4],
                }
            };
            compiler.images.insert(name.clone(), image);
            base_images.push(CleanedBaseImage { layer: layer_index, image: name });
        }
    }
    let lattice = create_native_arena_field_lattice(scene.bounds)?;
    let range = NativeArenaFieldRange { x_start: 0, x_end: 0, y_start: 0, y_end: 0, tile_count: 0 };
    let mode =
        if matches!(scene.environment_mode, 1 | 2) { scene.environment_mode as u8 } else { 0 };
    let ground = MeshDocument::native(&native_arena_field_mesh_plan(&lattice, range, mode)?);
    let mut roads = Vec::with_capacity(scene.roads.len());
    for road in &scene.roads {
        let plan = native_road_mesh_plan(road)?;
        roads.push(RoadDocument { style: plan.style, mesh: MeshDocument::native(&plan.mesh) });
    }
    let mut sprites = BTreeMap::new();
    for (name, entry) in [("gate-leaf", 7), ("gate-hinge", 8), ("light-glyph", 18)] {
        let record = catalog.record(entry).ok_or("Native scene support glyph is unavailable")?;
        sprites.insert(
            name.into(),
            SpriteDocument {
                image: record.file.clone(),
                anchor: [record.anchor_x, record.anchor_y],
            },
        );
    }
    let mut required = BTreeSet::new();
    let mut shadow_entries = BTreeSet::from([145, 146, 147]);
    for layer in &compiler.layers {
        for draw in &layer.draws {
            if let Some(image) = &draw.image {
                required.insert(image.clone());
            }
        }
        for plan in [&layer.shadow_plans.off, &layer.shadow_plans.on] {
            for quad in plan {
                if let NativeSceneryShadowTexture::Glyph(entry) = quad.texture {
                    shadow_entries.insert(entry);
                }
            }
        }
        if let Some(scrub) = &layer.scrub {
            shadow_entries.insert(scrub.entry);
            shadow_entries.insert(21);
        }
    }
    let mut shadow_glyphs = BTreeMap::new();
    for entry in shadow_entries {
        let record = catalog.record(entry).ok_or("Native shadow glyph is unavailable")?;
        required.insert(record.file.clone());
        shadow_glyphs.insert(
            entry,
            SceneryShadowGlyph { image: record.file.clone(), geometry: record.shadow_glyph() },
        );
    }
    for sprite in sprites.values() {
        required.insert(sprite.image.clone());
    }
    for name in required {
        if let std::collections::btree_map::Entry::Vacant(entry) = compiler.images.entry(name) {
            let image = assets::load_image(&format!("{ASSET_ROOT}/{}", entry.key())).await?;
            entry.insert(PreparedImage::Image(image));
        }
    }
    let puppet_target_ids =
        compiler.layers.iter().map(|layer| layer.puppet_target_id.clone()).collect();
    Ok(PreparedBoneyardView {
        document: SceneryDocument {
            layers: compiler.layers,
            gates,
            tree_inputs: trees,
            ground,
            field_lattice: lattice,
            field_mode: mode,
            roads,
            sprites,
            shadow_glyphs,
            goodie_shadow_outline: catalog
                .outline("Goodie", 0)?
                .into_iter()
                .map(Into::into)
                .collect(),
            cleanup: CleanupDocument {
                bounds: cleanup_bounds.map(Into::into),
                retired_layers,
                retired_roads,
                base_images,
            },
        },
        images: compiler.images,
        puppet_target_ids,
    })
}

struct SceneCompiler<'a> {
    catalog: &'a SceneryCatalog,
    scene: &'a BoneyardScene,
    plan: &'a NativeSceneryPlan,
    images: BTreeMap<String, PreparedImage>,
    layers: Vec<LayerDocument>,
    visual_bounds: BTreeMap<String, BoneyardBounds>,
}

impl SceneCompiler<'_> {
    async fn load_surface_images(&mut self) -> Result<(), String> {
        for name in [
            "ground.png",
            "road.png",
            "road2.png",
            "road3.png",
            "road4.png",
            "road5.png",
            "river.png",
            "rise.png",
            "fencegrate.png",
        ] {
            let image = assets::load_image(&format!("{ASSET_ROOT}/{name}")).await?;
            if name == "ground.png"
                && (image.natural_width() != 2048 || image.natural_height() != 2048)
            {
                return Err("Native Arena field requires the original full atlas page".into());
            }
            self.images.insert(name.into(), PreparedImage::Image(image));
        }
        Ok(())
    }

    fn glyph(
        &self,
        entry: usize,
        position: Vec2,
        rotation: f64,
        scale: Vec2,
        alpha: f64,
    ) -> Option<LayerDocument> {
        let record = self.catalog.record(entry)?;
        let corners = record.corners(position, rotation, scale);
        let mut layer = canvas_layer(record.file.clone(), record.bounds(position, rotation, scale));
        layer.initial_alpha = alpha;
        layer.draws = vec![LayerDrawDocument {
            image: Some(record.file.clone()),
            mesh: MeshDocument::quad(corners, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]),
        }];
        Some(layer)
    }

    fn base_art(
        &mut self,
        entry: usize,
        position: Vec2,
        rotation: f64,
        scale: Vec2,
        alpha: f64,
        key: String,
    ) -> Result<(), String> {
        // Cleanup still owns a zero-alpha/zero-scale authored sprite's true art extent.
        if let Some(record) = self.catalog.record(entry) {
            self.visual_bounds
                .insert(key.clone(), record.bounds(position, rotation, scale).world_bounds());
        }
        if scale.x <= 0.0 || scale.y <= 0.0 || alpha <= 0.0 {
            return Ok(());
        }
        if let Some(mut layer) = self.glyph(entry, position, rotation, scale, alpha) {
            layer.source_key = Some(key);
            self.merge_bounds(&layer);
            self.layers.push(layer);
        }
        Ok(())
    }

    fn main(
        &mut self,
        source: &NativeSceneryLayer,
        shadow_index: usize,
    ) -> Result<Option<LayerDocument>, String> {
        let mut resident = match source.source {
            NativeScenerySource::Object { index, entry } => {
                let object = &self.scene.objects[index];
                let entry = if object.type_id == 2062 {
                    264 + object.variant.unwrap_or(0) as usize
                } else {
                    entry
                };
                let Some(mut layer) =
                    self.glyph(entry, source.position, 0.0, Vec2::new(1.0, 1.0), 1.0)
                else {
                    return Ok(None);
                };
                layer.type_id = Some(object.type_id);
                layer.object_id = Some(object.eid.clone());
                layer.source_key = Some(format!("object:{}", object.eid));
                layer.puppet_target_id = Some(format!("scenery:{}", object.eid));
                layer.inserts_proxy =
                    self.plan.proxies.iter().any(|proxy| proxy.source_order == source.source_order);
                layer.glyph_material = if object.type_id == 2040 {
                    None
                } else {
                    Some(if object.type_id == 2001 {
                        SceneryGlyphMaterial::Tree
                    } else {
                        SceneryGlyphMaterial::Ordinary
                    })
                };
                if object.type_id == 2001 && layer.inserts_proxy {
                    layer.tree_id = Some(object.eid.clone());
                }
                if object.type_id == 2040 {
                    layer.surface_lighting = Some(self.building_grid(index)?);
                }
                if object.type_id == 2061 {
                    layer.initial_alpha = 0.0;
                }
                if object.type_id == 2062 {
                    layer.scrub = Some(ScrubDocument {
                        entry,
                        position: source.position,
                        main_layer_index: shadow_index,
                        id: format!("scrub:{}", object.eid),
                    });
                }
                layer
            }
            NativeScenerySource::FencePost { index, entry } => {
                let Some(mut layer) =
                    self.glyph(entry, source.position, 0.0, Vec2::new(1.0, 1.0), 1.0)
                else {
                    return Ok(None);
                };
                layer.puppet_target_id = Some(format!("fencepost:{index}"));
                layer.glyph_material = Some(SceneryGlyphMaterial::Ordinary);
                layer
            }
            NativeScenerySource::FenceBody { index, broken, .. } => {
                let fence = &self.scene.fences[index];
                if fence.points.len() < 2 {
                    return Ok(None);
                }
                let draws = if let Some(half) = broken {
                    let record =
                        self.catalog.record(3).ok_or("Broken fence glyph is unavailable")?;
                    vec![LayerDrawDocument {
                        image: Some(record.file.clone()),
                        mesh: MeshDocument::quad(
                            [half.corners[0], half.corners[1], half.corners[3], half.corners[2]],
                            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                        ),
                    }]
                } else if fence.code() == 4 {
                    let record = self.catalog.record(23).ok_or("Rail glyph is unavailable")?;
                    let a = fence.points[0];
                    let b = fence.points[1];
                    let length = hypot(b.x - a.x, b.y - a.y);
                    if length == 0.0 {
                        return Ok(None);
                    }
                    let count = (length / record.w).round().max(1.0) as usize;
                    let mut draws = Vec::with_capacity(count);
                    for part in 0..count {
                        let amount = (part as f64 + 0.5) / count as f64;
                        let position =
                            Vec2::new(a.x + (b.x - a.x) * amount, a.y + (b.y - a.y) * amount);
                        draws.push(LayerDrawDocument {
                            image: Some(record.file.clone()),
                            mesh: MeshDocument::quad(
                                record.corners(position, 0.0, Vec2::new(1.0, 1.0)),
                                [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                            ),
                        });
                    }
                    draws
                } else {
                    let Some(grate) = native_fence_grate(&fence.points) else {
                        return Ok(None);
                    };
                    let points = grate.corners;
                    let mut draws = vec![LayerDrawDocument {
                        image: Some("fencegrate.png".into()),
                        mesh: MeshDocument::quad(
                            [points[0], points[1], points[3], points[2]],
                            [[0.0, 0.0], [grate.u_span, 0.0], [grate.u_span, 1.0], [0.0, 1.0]],
                        ),
                    }];
                    for (start, end) in [
                        (
                            Vec2::new(points[0].x, points[0].y + 9.0),
                            Vec2::new(points[1].x, points[1].y + 9.0),
                        ),
                        (
                            Vec2::new(points[2].x, points[2].y - 5.0),
                            Vec2::new(points[3].x, points[3].y - 5.0),
                        ),
                    ] {
                        draws.push(LayerDrawDocument {
                            image: None,
                            mesh: MeshDocument::line(start, end, 3.0),
                        });
                    }
                    draws
                };
                let points: Vec<_> = draws.iter().flat_map(|draw| draw.mesh.points()).collect();
                let mut layer = canvas_layer(String::new(), BoundsDocument::points(&points));
                layer.draws = draws;
                layer
            }
        };
        resident.band = SceneryBand::Main;
        resident.world_y = source.world_y;
        resident.sort_bias = source.sort_bias;
        resident.source_order = source.source_order;
        resident.light_position = Some(source.position.into());
        resident.shadow = self.shadow(source, shadow_index)?;
        let glyph = |entry| self.catalog.record(entry).map(|record| record.shadow_glyph());
        resident.shadow_plans = SceneryShadowPlans {
            off: native_scenery_shadow_plan(self.scene, self.plan, source, false, &glyph),
            on: native_scenery_shadow_plan(self.scene, self.plan, source, true, &glyph),
        };
        self.merge_bounds(&resident);
        Ok(Some(resident))
    }

    fn building_grid(&self, index: usize) -> Result<SurfaceLightingDocument, String> {
        let object = &self.scene.objects[index];
        let entry = native_scenery_object_entry(object).ok_or("Building has no base glyph")?;
        let record = self.catalog.record(entry).ok_or("Building base glyph is unavailable")?;
        let offset = *[135.0, 100.0, 0.0, 0.0]
            .get(object.variant.unwrap_or(0) as usize)
            .ok_or("Unknown native Building selector")?;
        let points = |side: usize| {
            let mut output = Vec::with_capacity(side * side);
            for row in 0..side {
                for column in 0..side {
                    output.push(PointDocument {
                        x: object.position.x - record.anchor_x
                            + record.w * column as f64 / (side - 1) as f64,
                        y: object.position.y - record.anchor_y
                            + record.h * row as f64 / (side - 1) as f64
                            + if row < side - 1 { offset } else { 0.0 },
                    });
                }
            }
            output
        };
        Ok(SurfaceLightingDocument::Building {
            sample_points: points(3),
            sample_points_coarse: points(2),
        })
    }

    fn shadow(
        &self,
        layer: &NativeSceneryLayer,
        index: usize,
    ) -> Result<Option<ShadowDocument>, String> {
        let mut shadow = ShadowDocument {
            id: format!("main:{index}"),
            position: layer.position.into(),
            outline: Vec::new(),
            program: None,
        };
        match layer.source {
            NativeScenerySource::Object { index, entry } => {
                let object = &self.scene.objects[index];
                let (family, base) = match object.type_id {
                    2001 => ("Tree", 264),
                    2029 => ("Gravestone", 97),
                    2009 => ("Monument", 156),
                    2040 => ("Building", 148),
                    2061 => ("Goodie", 145),
                    _ => return Ok(None),
                };
                let variant = if object.type_id == 2061 {
                    0
                } else {
                    object.variant.map_or(entry.saturating_sub(base), |value| value as usize)
                };
                shadow.outline = self.catalog.outline(family, variant)?;
            }
            NativeScenerySource::FencePost { index, .. } => {
                let post = &self.plan.posts[index];
                shadow.outline = self.catalog.outline(
                    if post.style == 1 { "Railpost" } else { "Fencepost" },
                    (post.variant % 7) as usize,
                )?;
            }
            NativeScenerySource::FenceBody { index, broken, .. } => {
                let fence = &self.scene.fences[index];
                let [start, end, ..] = fence.points.as_slice() else {
                    return Ok(None);
                };
                shadow.program = Some(if let Some(half) = broken {
                    ShadowProgramDocument::BrokenFenceGrate {
                        start: half.shadow_start.into(),
                        end: half.shadow_end.into(),
                        step: half.shadow_step.into(),
                        count: half.shadow_count,
                    }
                } else {
                    match fence.code() {
                        0 => ShadowProgramDocument::FenceGrate {
                            start: (*start).into(),
                            end: (*end).into(),
                            construction: "intact".into(),
                        },
                        4 => ShadowProgramDocument::Rails {
                            start: (*start).into(),
                            end: (*end).into(),
                        },
                        3 => {
                            let dx = end.x - start.x;
                            let dy = end.y - start.y;
                            let length = hypot(dx, dy);
                            let (ux, uy) =
                                if length == 0.0 { (0.0, 0.0) } else { (dx / length, dy / length) };
                            let connected = |point: Vec2| {
                                self.scene.fences.iter().any(|other| {
                                    other.code() == 3
                                        && other.eid != fence.eid
                                        && other.points.iter().take(2).any(|other| *other == point)
                                })
                            };
                            ShadowProgramDocument::Wall {
                                start: if connected(*start) {
                                    *start
                                } else {
                                    Vec2::new(start.x - ux * 15.0, start.y - uy * 15.0)
                                }
                                .into(),
                                end: if connected(*end) {
                                    *end
                                } else {
                                    Vec2::new(end.x + ux * 15.0, end.y + uy * 15.0)
                                }
                                .into(),
                            }
                        }
                        _ => return Ok(None),
                    }
                });
            }
        }
        Ok(Some(shadow))
    }

    fn merge_bounds(&mut self, layer: &LayerDocument) {
        let Some(key) = &layer.source_key else {
            return;
        };
        let bounds = layer.bounds.world_bounds();
        self.visual_bounds
            .entry(key.clone())
            .and_modify(|current| {
                let x = current.x.min(bounds.x);
                let y = current.y.min(bounds.y);
                let right = (current.x + current.width).max(bounds.x + bounds.width);
                let bottom = (current.y + current.height).max(bounds.y + bounds.height);
                *current = BoneyardBounds::new(x, y, right - x, bottom - y);
            })
            .or_insert(bounds);
    }

    fn retired_sources(&self, target: BoneyardBounds) -> BTreeSet<String> {
        let mut retired = BTreeSet::new();
        let overlaps = |bounds: BoneyardBounds| {
            bounds.x < target.x + target.width
                && bounds.y < target.y + target.height
                && bounds.x + bounds.width > target.x
                && bounds.y + bounds.height > target.y
        };
        for (key, bounds) in &self.visual_bounds {
            if !overlaps(*bounds) {
                retired.insert(key.clone());
            }
        }
        for road in &self.scene.roads {
            let points = road.quad.as_ref().unwrap_or(&road.points);
            if !points.is_empty() && !overlaps(BoundsDocument::points(points).world_bounds()) {
                retired.insert(format!("road:{}", road.eid));
            }
        }
        for terrain in &self.scene.terrain {
            let fallback = [terrain.position];
            let points = terrain.points.as_deref().unwrap_or(&fallback);
            if !points.is_empty() && !overlaps(BoundsDocument::points(points).world_bounds()) {
                retired.insert(format!("terrain:{}", terrain.eid));
            }
        }
        retired
    }
}

fn canvas_layer(image: String, bounds: BoundsDocument) -> LayerDocument {
    let corners = [
        Vec2::new(bounds.x, bounds.y),
        Vec2::new(bounds.x + bounds.width, bounds.y),
        Vec2::new(bounds.x + bounds.width, bounds.y + bounds.height),
        Vec2::new(bounds.x, bounds.y + bounds.height),
    ];
    LayerDocument {
        type_id: None,
        image: image.clone(),
        bounds,
        band: SceneryBand::Base,
        world_y: 0.0,
        sort_bias: 0.0,
        source_order: 0,
        inserts_proxy: false,
        object_id: None,
        tree_id: None,
        initial_alpha: 1.0,
        light_position: None,
        surface_lighting: None,
        shadow: None,
        shadow_plans: SceneryShadowPlans::default(),
        scrub: None,
        source_key: None,
        puppet_target_id: None,
        glyph_material: None,
        draws: vec![LayerDrawDocument {
            image: Some(image),
            mesh: MeshDocument::quad(corners, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]),
        }],
    }
}

fn cleanup_bounds(loaded: &LoadedBoneyard) -> Option<BoneyardBounds> {
    if loaded.choice_id == "stock-tutorial" {
        Some(solomon_sim::boneyard::tutorial::NATIVE_TUTORIAL_CAMERA_TARGET)
    } else if loaded.choice_id == "default-random" && loaded.scene.solomon_dig.is_some() {
        Some(
            solomon_sim::boneyard::arena::ArenaTransition::new(
                loaded.scene.bounds,
                loaded.scene.spawn.position,
            )
            .combat_bounds,
        )
    } else {
        None
    }
}

fn expand_paint(painted: &canvas::PaintedCanvas, bounds: &BoundsDocument) -> PreparedImage {
    let PreparedImage::Pixels { width, height, pixels } = &painted.image else { unreachable!() };
    let mut output = vec![0; bounds.width as usize * bounds.height as usize * 4];
    for row in 0..*height as usize {
        let x = (painted.bounds.x - bounds.x) as usize;
        let y = (painted.bounds.y - bounds.y) as usize + row;
        let start = (y * bounds.width as usize + x) * 4;
        output[start..start + *width as usize * 4].copy_from_slice(
            &pixels[row * (*width as usize) * 4..(row + 1) * (*width as usize) * 4],
        );
    }
    PreparedImage::Pixels {
        width: bounds.width as u32,
        height: bounds.height as u32,
        pixels: output,
    }
}
