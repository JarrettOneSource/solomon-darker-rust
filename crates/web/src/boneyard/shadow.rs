//! Original class shadow meshes from `boneyard-{scenery,scrub}-shadow-presentation.ts`.
//! Immutable glyph plans use the owner's existing painter slot. Only Scrub's
//! private visual clock and lazy surface result survive between display frames.

use solomon_sim::boneyard::loot::{BoneyardGoodiePhase, BoneyardGoodieState};
use solomon_sim::boneyard::scenery::{
    NativeSceneryShadowRole, NativeSceneryShadowTexture, NativeScrubShadowState,
    native_goodie_shadow_plan, native_scrub_shadow_plan,
};
use solomon_sim::boneyard::shadows::native_shadow_records;

use super::*;

pub(super) struct SceneryShadowGlyph {
    pub image: String,
    pub geometry: NativeSceneryShadowGlyph,
}

#[derive(Default)]
pub(super) struct SceneryShadowPlans {
    pub off: Vec<NativeSceneryShadowQuad>,
    pub on: Vec<NativeSceneryShadowQuad>,
}

pub(super) struct ScrubDocument {
    pub entry: usize,
    pub position: Vec2,
    pub main_layer_index: usize,
    pub id: String,
}

pub(super) struct ScrubView {
    state: NativeScrubShadowState,
    quads: Vec<NativeSceneryShadowQuad>,
}

impl ScrubView {
    pub fn new(document: &ScrubDocument, initial_tick: u64) -> Self {
        Self {
            state: NativeScrubShadowState::new(document.main_layer_index, initial_tick),
            quads: Vec::new(),
        }
    }
}

impl BoneyardView {
    pub(super) fn update_scrub_shadows(&mut self, frame: &BoneyardFrame<'_>) {
        for (&index, view) in &mut self.scrubs {
            // The Source advances every resident, including off-camera and retired
            // owners. This stream is independent of all authoritative random draws.
            view.state.advance_to(frame.stopped_world_tick);
            let layer = &self.document.layers[index];
            let scrub = layer.scrub.as_ref().expect("Scrub view has its immutable placement");
            view.quads.clear();
            if (self.cleanup_applied && self.document.cleanup.retired_layers.contains(&index))
                || layer.initial_alpha == 0.0
                || !layer.bounds.visible(frame)
            {
                continue;
            }
            let records = if frame.complex_lighting && frame.complex_shadows {
                native_shadow_records(
                    &NativeShadowCaster {
                        id: scrub.id.clone(),
                        position: scrub.position,
                        outline: Vec::new(),
                        program: None,
                    },
                    frame.lights,
                    frame.presentation_frame,
                )
            } else {
                Vec::new()
            };
            view.quads = native_scrub_shadow_plan(
                scrub.entry,
                self.document.shadow_glyphs[&scrub.entry].geometry,
                scrub.position,
                &mut view.state,
                &records,
                frame.complex_shadows,
                frame.special_surface_at,
                self.document.shadow_glyphs[&21].geometry,
            );
        }
    }

    pub(super) fn draw_layer_shadows(
        &self,
        renderer: &mut Renderer,
        frame: &BoneyardFrame<'_>,
        index: usize,
        tree_alpha: f64,
    ) {
        let layer = &self.document.layers[index];
        let plan =
            if frame.complex_shadows { &layer.shadow_plans.on } else { &layer.shadow_plans.off };
        for quad in plan {
            let alpha = if quad.role == NativeSceneryShadowRole::SceneryFlatShadow {
                tree_alpha
            } else {
                1.0
            };
            self.draw_shadow_quad(renderer, frame.camera, quad, Vec2::ZERO, alpha);
        }
        if let Some(view) = self.scrubs.get(&index) {
            let position = layer.scrub.as_ref().expect("resident Scrub placement").position;
            for quad in &view.quads {
                self.draw_shadow_quad(renderer, frame.camera, quad, position, 1.0);
            }
        }
    }

    pub(super) fn draw_goodie_shadow(
        &self,
        renderer: &mut Renderer,
        frame: &BoneyardFrame<'_>,
        goodie: &BoneyardGoodieState,
    ) {
        let phase = match goodie.phase {
            BoneyardGoodiePhase::Intact => 0,
            BoneyardGoodiePhase::Opening => 1,
            BoneyardGoodiePhase::Broken => 2,
        };
        let glyph = self.document.shadow_glyphs[&(145 + phase)].geometry;
        let bounds = BoundsDocument {
            x: goodie.position.x - glyph.anchor_x,
            y: goodie.position.y - glyph.anchor_y,
            width: glyph.width,
            height: glyph.height,
        };
        if !bounds.visible(frame) {
            return;
        }
        for quad in native_goodie_shadow_plan(
            goodie.position,
            goodie.subtype,
            phase,
            frame.complex_shadows,
            &|entry| self.document.shadow_glyphs.get(&entry).map(|glyph| glyph.geometry),
        ) {
            self.draw_shadow_quad(renderer, frame.camera, &quad, Vec2::ZERO, 1.0);
        }
        if frame.complex_lighting && frame.complex_shadows {
            assert_eq!(goodie.subtype, 0, "native Goodie complex-shadow subtype");
            let caster = NativeShadowCaster {
                id: format!("goodie:{}", goodie.id),
                position: goodie.position,
                outline: self.document.goodie_shadow_outline.clone(),
                program: None,
            };
            draw_shadow_edges(
                renderer,
                frame.camera,
                &native_shadow_edges(&caster, frame.lights, frame.presentation_frame),
            );
        }
    }

    fn draw_shadow_quad(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        quad: &NativeSceneryShadowQuad,
        position: Vec2,
        alpha: f64,
    ) {
        let texture = match quad.texture {
            NativeSceneryShadowTexture::Glyph(entry) => {
                Some(&self.textures[&self.document.shadow_glyphs[&entry].image])
            }
            NativeSceneryShadowTexture::FenceGrate => Some(&self.textures["fencegrate.png"]),
            NativeSceneryShadowTexture::White => None,
        };
        // Native batches first pack each color, then multiply its byte channels
        // by the owner's packed color. Multiplying unquantized alphas differs.
        let parent_alpha = (alpha.clamp(0.0, 1.0) * 255.0).trunc();
        let vertices: [Vertex; 4] = std::array::from_fn(|index| {
            let point = quad.vertices[index];
            let packed_alpha = (quad.alphas[index].clamp(0.0, 1.0) * 255.0).trunc();
            Vertex {
                position: project(camera, Vec2::new(point.x + position.x, point.y + position.y)),
                texel: texture.map_or([0.0; 2], |texture| {
                    [
                        (quad.uvs[index].x * f64::from(texture.width)) as f32,
                        (quad.uvs[index].y * f64::from(texture.height)) as f32,
                    ]
                }),
                tint: [
                    ((quad.tint >> 16) & 255) as f32 / 255.0,
                    ((quad.tint >> 8) & 255) as f32 / 255.0,
                    (quad.tint & 255) as f32 / 255.0,
                    (packed_alpha * parent_alpha / 255.0).trunc() as f32 / 255.0,
                ],
            }
        });
        let triangles =
            [[vertices[0], vertices[1], vertices[2]], [vertices[1], vertices[3], vertices[2]]];
        if let Some(texture) = texture {
            renderer.draw_triangles(texture, &triangles, BlendMode::Normal);
        } else {
            renderer.draw_solid_triangles(&triangles, BlendMode::Normal);
        }
    }
}
