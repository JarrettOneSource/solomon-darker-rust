//! `native-compact-mask-view.ts`: additive diffuse stamps, a radial Multiply
//! pass and one retained 256² composite per actual participant.

use std::collections::{BTreeMap, BTreeSet};

use solomon_sim::Vec2;
use solomon_sim::boneyard::compact_masks::{
    NATIVE_COMPACT_MASK_COMPOSITE_ANCHOR, NATIVE_COMPACT_MASK_COMPOSITE_SCALE,
    NATIVE_COMPACT_MASK_RADIAL_ENTRY, NATIVE_COMPACT_MASK_RADIAL_SCALE,
    NATIVE_COMPACT_MASK_TARGET_SIDE, NATIVE_DIRECT_ENVIRONMENT_LIGHT_ENTRY,
    NativeCompactGroundSurface, NativeCompactMask, NativeCompactMaskPlan,
    NativeCompactMaskPlayerPlan, NativeCompactMasks, native_direct_environment_light_alpha,
};
use solomon_sim::boneyard::{BoneyardBounds, BoneyardScene};
use solomon_sim::enemies::spider_remains::SpiderRemains;
use solomon_sim::spells::presentation::{
    NativeSpellSprite, NativeSpellSpritePlan, SpellAtlas, SpellBlend,
};

use crate::courtyard::Camera;
use crate::native_mesh::native_sprite_texel_rect;
use crate::spells::SpellView;
use solomon_graphics::{BlendMode, Quad, RenderTarget, Renderer};

#[derive(Clone, Copy)]
pub struct CompactMaskPlayer<'a> {
    pub id: &'a str,
    pub position: Vec2,
}

/// `BoneyardEnvironmentLightView`: when scene environment mode is nonzero,
/// draw this aperture immediately before the same participant's compact mask.
pub fn draw_environment_aperture(
    renderer: &mut Renderer,
    camera: &Camera,
    spells: &SpellView,
    position: Vec2,
    now_ms: f64,
    slot: usize,
) {
    let atlas = spells.dead_hawg();
    let record = &atlas.records[NATIVE_DIRECT_ENVIRONMENT_LIGHT_ENTRY];
    let alpha = native_direct_environment_light_alpha(now_ms, slot);
    renderer.draw(
        &atlas.texture,
        Quad {
            source: native_sprite_texel_rect(record),
            destination: camera.project(
                position.x - 168.0,
                position.y - 153.0,
                f64::from(record.width),
                f64::from(record.height),
            ),
            tint: [1.0, 1.0, 1.0, ((alpha * 255.0).trunc() / 255.0) as f32],
            blend: BlendMode::Add,
        },
    );
}

pub struct CompactMaskView {
    masks: NativeCompactMasks,
    surface: NativeCompactGroundSurface,
    plan: NativeCompactMaskPlan,
    ground_order: Vec<String>,
    targets: BTreeMap<String, PlayerMaskTarget>,
}

impl CompactMaskView {
    pub fn new(scene: &BoneyardScene) -> Self {
        Self {
            masks: NativeCompactMasks::new(scene),
            surface: NativeCompactGroundSurface::new(scene),
            plan: NativeCompactMaskPlan::default(),
            ground_order: Vec::new(),
            targets: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn special_surface_at(
        &self,
        position: Vec2,
        remains: &solomon_sim::ComponentEntities<SpiderRemains>,
        spells: &SpellView,
    ) -> bool {
        self.surface.contains(position, remains, &|entry| {
            let record = &spells.dead_hawg().records[entry];
            Vec2::new(f64::from(record.width), f64::from(record.height))
        })
    }

    /// Call on the default framebuffer before any nested light-target capture.
    /// Existing canvas batches are flushed before allocating or repainting targets.
    /// `world_bounds` is sealed combat bounds or the full scene bounds. Players
    /// match the environment apertures and exclude materializing participants.
    pub fn prepare(
        &mut self,
        renderer: &mut Renderer,
        spells: &SpellView,
        players: &[CompactMaskPlayer<'_>],
        remains: &solomon_sim::ComponentEntities<SpiderRemains>,
        world_bounds: BoneyardBounds,
        presentation_frame: f64,
    ) -> Result<(), String> {
        let positions: Vec<Vec2> = players.iter().map(|player| player.position).collect();
        self.plan =
            self.masks.plan(&positions, remains, world_bounds, presentation_frame, &|entry| {
                let record = &spells.dead_hawg().records[entry];
                Vec2::new(f64::from(record.width), f64::from(record.height))
            });
        let mut live_ground = BTreeSet::new();
        for mask in &self.plan.masks {
            if mask.dynamic {
                live_ground.insert(mask.id.clone());
            }
        }
        self.ground_order.retain(|id| live_ground.contains(id));
        for mask in &self.plan.masks {
            if mask.dynamic && !self.ground_order.contains(&mask.id) {
                self.ground_order.push(mask.id.clone());
            }
        }
        renderer.flush();
        for (player, plan) in players.iter().zip(&self.plan.players) {
            if plan.mask_indices.is_empty() {
                if let Some(target) = self.targets.get_mut(player.id) {
                    // Source retains a hidden player's target for later reuse.
                    target.visible = false;
                }
                continue;
            }
            if !self.targets.contains_key(player.id) {
                let target = renderer.create_render_target(NATIVE_COMPACT_MASK_TARGET_SIDE)?;
                self.targets.insert(
                    player.id.to_owned(),
                    PlayerMaskTarget {
                        target,
                        position: plan.position,
                        alpha: 0.0,
                        visible: false,
                    },
                );
            }
            self.targets
                .get_mut(player.id)
                .expect("a player with candidates has a compact target")
                .update(renderer, spells, &self.plan.masks, plan);
        }
        self.targets.retain(|id, target| {
            if players.iter().any(|player| player.id == id) {
                true
            } else {
                renderer.destroy_render_target(&target.target);
                false
            }
        });
        Ok(())
    }

    /// Dynamic Spider stains enter the source pre-world group at depth -0.5.
    /// Authored ground stamps already belong to the static scenery renderer.
    pub fn draw_ground(&self, renderer: &mut Renderer, camera: &Camera, spells: &SpellView) {
        for id in &self.ground_order {
            let mask = self.plan.masks.iter().find(|mask| &mask.id == id).unwrap();
            draw_mask(renderer, camera, spells, mask, SpellBlend::Normal);
        }
    }

    /// Interleave with environment apertures in participant order: aperture at
    /// slot*2, then this composite at slot*2+1, in the source ground-light group.
    pub fn draw_player(&self, renderer: &mut Renderer, camera: &Camera, player_id: &str) {
        let Some(target) = self.targets.get(player_id).filter(|target| target.visible) else {
            return;
        };
        let side = f64::from(NATIVE_COMPACT_MASK_TARGET_SIDE);
        let scale = NATIVE_COMPACT_MASK_COMPOSITE_SCALE;
        let anchor = NATIVE_COMPACT_MASK_COMPOSITE_ANCHOR * scale;
        renderer.draw(
            &target.target.texture,
            Quad {
                source: [0.0, side as f32, side as f32, -side as f32],
                destination: camera.project(
                    target.position.x - anchor,
                    target.position.y - anchor,
                    side * scale,
                    side * scale,
                ),
                tint: [1.0, 1.0, 1.0, ((target.alpha * 255.0).trunc() / 255.0) as f32],
                blend: BlendMode::Add,
            },
        );
    }

    pub fn counts(&self) -> [usize; 2] {
        [self.targets.len(), self.ground_order.len()]
    }

    pub fn destroy(self, renderer: &Renderer) {
        for target in self.targets.values() {
            renderer.destroy_render_target(&target.target);
        }
    }
}

struct PlayerMaskTarget {
    target: RenderTarget,
    position: Vec2,
    alpha: f64,
    visible: bool,
}

impl PlayerMaskTarget {
    fn update(
        &mut self,
        renderer: &mut Renderer,
        spells: &SpellView,
        masks: &[NativeCompactMask],
        plan: &NativeCompactMaskPlayerPlan,
    ) {
        let side = f64::from(NATIVE_COMPACT_MASK_TARGET_SIDE);
        let camera = Camera {
            origin: Vec2::new(plan.position.x - side, plan.position.y - side),
            scale: 0.5,
        };
        renderer.render_diffuse_to(&self.target, [side as f32; 2], |renderer| {
            for &index in &plan.mask_indices {
                draw_mask(renderer, &camera, spells, &masks[index], SpellBlend::Add);
            }
        });
        renderer.render_onto(&self.target, [side as f32; 2], |renderer| {
            let atlas = spells.dead_hawg();
            let record = &atlas.records[NATIVE_COMPACT_MASK_RADIAL_ENTRY];
            let scale = NATIVE_COMPACT_MASK_RADIAL_SCALE;
            let width = f64::from(record.width);
            let height = f64::from(record.height);
            renderer.draw(
                &atlas.texture,
                Quad {
                    source: native_sprite_texel_rect(record),
                    destination: [
                        (128.0 + (-width / 2.0 + f64::from(record.center_x)) * scale) as f32,
                        (128.0 + (-height / 2.0 + f64::from(record.center_y)) * scale) as f32,
                        (width * scale) as f32,
                        (height * scale) as f32,
                    ],
                    tint: [1.0; 4],
                    blend: BlendMode::Multiply,
                },
            );
        });
        self.position = plan.position;
        self.alpha = plan.alpha;
        self.visible = true;
    }
}

fn draw_mask(
    renderer: &mut Renderer,
    camera: &Camera,
    spells: &SpellView,
    mask: &NativeCompactMask,
    blend: SpellBlend,
) {
    let draw = NativeSpellSprite {
        alpha: mask.alpha,
        atlas: SpellAtlas::DeadHawg,
        blend,
        rotation: mask.rotation_degrees * std::f64::consts::PI / 180.0,
        scale: mask.scale,
        ..NativeSpellSprite::new(mask.entry, 1.0)
    };
    spells.draw_sprite_plan(
        renderer,
        camera,
        &NativeSpellSpritePlan {
            position: mask.position,
            world_y: mask.position.y,
            sort_bias: 0.0,
            region_light_point: None,
            draws: vec![draw],
        },
    );
}
