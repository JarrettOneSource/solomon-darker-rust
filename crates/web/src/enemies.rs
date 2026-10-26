//! Stock Boneyard enemies. The simulation owns all rules and draw plans; this view
//! owns atlas handles, private paint-time children, and WebGL submission.

mod art;
mod attacks;
mod meshes;
mod prepare;

use std::collections::BTreeMap;

use solomon_assets::SpriteRecord;
use solomon_sim::Vec2;
use solomon_sim::boneyard::shadows::NativeShadowRecord;
use solomon_sim::enemies::actor_presentation::{
    EnemyArtAtlas, EnemyArtBlend, EnemySegmentLayer, EnemySpriteLayer,
};
use solomon_sim::enemies::death_presentation::EnemyBanishGradient;
use solomon_sim::enemies::model::{EnemyActor, EnemyDeathEffect, EnemyEvent};
use solomon_sim::enemies::rain_presentation::NativeRainCloud;
use solomon_sim::enemies::tendrils::HeartmongerTendrils;
use solomon_sim::enemies::underlay::EnemyUnderlayLayer;
use solomon_sim::enemies::web_presentation::EnemyWebMesh;
use solomon_sim::enemies::world::EnemyWorld;
use solomon_sim::hub::painter::PainterLayer;

use crate::courtyard::Camera;
use crate::native_mesh::NativeMeshVertex;
use solomon_graphics::{Renderer, Texture};

pub struct EnemyViewPlayer<'a> {
    pub id: &'a str,
    pub position: Vec2,
    pub primary_tint: u32,
}

pub struct EnemyViewShadow {
    pub records: Vec<NativeShadowRecord>,
    pub light_scalar: f64,
}

/// Accepted world facts used by the source presenters; no gameplay state is reconstructed.
pub struct EnemyViewFrame<'a> {
    pub(crate) presentation: Option<&'a crate::presentation::EnemyPresentationMetadata>,
    /// Already sampled at the fractional display tick; do not age these values again.
    pub puppet_hit_alphas: Option<&'a BTreeMap<String, f64>>,
    /// Loot deaths use the same source effect presenter and painter ownership as enemy deaths.
    pub extra_death_effects: &'a solomon_sim::ComponentEntities<EnemyDeathEffect>,
    pub tick: u64,
    pub application_tick: u64,
    pub complex_lighting: bool,
    pub complex_shadows: bool,
    pub view_height: f64,
    pub players: &'a [EnemyViewPlayer<'a>],
    pub light_at: &'a dyn Fn(Vec2) -> f64,
    pub point_gain: &'a dyn Fn(Vec2) -> f64,
    pub actor_shadows: &'a dyn Fn(&EnemyActor) -> EnemyViewShadow,
    pub actor_tint: &'a dyn Fn(&EnemyActor) -> u32,
}

pub struct EnemyPainter {
    pub layer: PainterLayer,
    pub region_light_point: Option<Vec2>,
}

struct EnemyRoot {
    painter: EnemyPainter,
    group: EnemyDrawGroup,
}

struct EnemyDrawGroup {
    position: Vec2,
    scale: f64,
    material_tint: Option<u32>,
    draws: Vec<EnemyDraw>,
}

impl EnemyDrawGroup {
    fn at(position: Vec2) -> Self {
        Self { position, scale: 1.0, material_tint: None, draws: Vec::new() }
    }

    fn sprites(position: Vec2, layers: Vec<EnemySpriteLayer>, region_tint: bool) -> Self {
        let mut group = Self::at(position);
        for layer in layers {
            group.draws.push(EnemyDraw::Sprite { layer, region_tint });
        }
        group
    }
}

enum EnemyDraw {
    Sprite {
        layer: EnemySpriteLayer,
        region_tint: bool,
    },
    Segment(EnemySegmentLayer),
    Underlay(EnemyUnderlayLayer),
    Gradient(EnemyBanishGradient),
    AtlasMesh {
        atlas: EnemyArtAtlas,
        entry: usize,
        triangles: Vec<[NativeMeshVertex; 3]>,
        alpha: f64,
        tint: u32,
        blend: EnemyArtBlend,
    },
    Web(EnemyWebMesh),
    MouthBeam {
        vertices: [Vec2; 4],
        uv_offset: f64,
        start_alpha: f64,
        end_alpha: f64,
    },
}

pub struct EnemyView {
    art: art::EnemyAtlases,
    green_plasma: Texture,
    roots: Vec<EnemyRoot>,
    background: Vec<EnemyDrawGroup>,
    underlays: Vec<(usize, EnemyDrawGroup)>,
    pre_world: Vec<EnemyDrawGroup>,
    post_world: Vec<EnemyDrawGroup>,
    late_world: Vec<EnemyDrawGroup>,
    rain_clouds: BTreeMap<usize, NativeRainCloud>,
    tendrils: BTreeMap<usize, HeartmongerTendrils>,
    attack_origins: BTreeMap<usize, Vec2>,
    pending_events: Vec<EnemyEvent>,
    last_event_id: usize,
    auxiliary: BTreeMap<usize, attacks::EnemyAttackEffect>,
}

impl EnemyView {
    /// Reuse the primary view's two shared atlas uploads; only the four boss banks are new.
    pub async fn load(
        renderer: &Renderer,
        bad_guys_records: Vec<SpriteRecord>,
        bad_guys_texture: Texture,
        dead_hawg_records: Vec<SpriteRecord>,
        dead_hawg_texture: Texture,
    ) -> Result<Self, String> {
        let art = art::EnemyAtlases::load(
            renderer,
            bad_guys_records,
            bad_guys_texture,
            dead_hawg_records,
            dead_hawg_texture,
        )
        .await?;
        let image =
            solomon_graphics::assets::load_image("assets/stock/enemy-green-plasma.png").await?;
        let green_plasma = renderer.upload_with_repeat(&image, true)?;
        Ok(Self {
            art,
            green_plasma,
            roots: Vec::new(),
            background: Vec::new(),
            underlays: Vec::new(),
            pre_world: Vec::new(),
            post_world: Vec::new(),
            late_world: Vec::new(),
            rain_clouds: BTreeMap::new(),
            tendrils: BTreeMap::new(),
            attack_origins: BTreeMap::new(),
            pending_events: Vec::new(),
            last_event_id: 0,
            auxiliary: BTreeMap::new(),
        })
    }

    pub fn prepare(&mut self, world: &EnemyWorld, frame: &EnemyViewFrame<'_>) {
        world.assert_painter_registrations_committed();
        self.consume_events(&world.events);
        self.clear_prepared();
        self.prepare_actors(world, frame);
        self.prepare_attack_effects(world, frame);
        self.prepare_projectiles(world, frame);
        self.prepare_effects(world, frame);
        self.prepare_boss_spells(world, frame);
        self.prepare_spider_webs(world, frame);
        self.prepare_crows_and_remains(world, frame);
        self.underlays.sort_by_key(|(ordinal, _)| *ordinal);
    }

    /// Accepted tick events are queued until the next paint so skipped browser frames
    /// do not lose flashes. Positions come from that paint's current actor plan.
    pub fn consume_events(&mut self, events: &[EnemyEvent]) {
        for event in events {
            if event.event_id <= self.last_event_id {
                continue;
            }
            self.last_event_id = event.event_id;
            self.pending_events.push(event.clone());
        }
    }

    /// Begin another Boneyard scene while retaining the already uploaded stock art.
    pub fn reset(&mut self) {
        self.clear_prepared();
        self.rain_clouds.clear();
        self.tendrils.clear();
        self.pending_events.clear();
        self.last_event_id = 0;
        self.auxiliary.clear();
    }

    fn clear_prepared(&mut self) {
        self.roots.clear();
        self.background.clear();
        self.underlays.clear();
        self.pre_world.clear();
        self.post_world.clear();
        self.late_world.clear();
        self.attack_origins.clear();
    }

    pub fn painters(&self) -> impl Iterator<Item = (usize, &EnemyPainter)> {
        self.roots.iter().enumerate().map(|(index, root)| (index, &root.painter))
    }

    pub fn draw(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        index: usize,
        region_tint: [f32; 3],
    ) {
        self.draw_group(renderer, camera, &self.roots[index].group, region_tint);
    }

    pub fn draw_background(&self, renderer: &mut Renderer, camera: &Camera) {
        for group in &self.background {
            self.draw_group(renderer, camera, group, [1.0; 3]);
        }
    }

    pub fn draw_underlays(&self, renderer: &mut Renderer, camera: &Camera) {
        for (_, group) in &self.underlays {
            self.draw_group(renderer, camera, group, [1.0; 3]);
        }
    }

    pub fn draw_pre_world(&self, renderer: &mut Renderer, camera: &Camera) {
        for group in &self.pre_world {
            self.draw_group(renderer, camera, group, [1.0; 3]);
        }
    }

    pub fn draw_post_world(&self, renderer: &mut Renderer, camera: &Camera) {
        for group in &self.post_world {
            self.draw_group(renderer, camera, group, [1.0; 3]);
        }
    }

    pub fn draw_late_world(&self, renderer: &mut Renderer, camera: &Camera) {
        for group in &self.late_world {
            self.draw_group(renderer, camera, group, [1.0; 3]);
        }
    }
}
