//! PlayerWizard::DrawSpecial `0x00546999..0x00546E49`: two normal-alpha
//! composites of the articulated diffuse body multiplied by Clothes record3.

use solomon_graphics::{TextureColorMode, Vertex};
use solomon_sim::secondary::motion::STONESKIN_GRID_COMPONENTS;

use super::*;
use crate::beam_mesh::sprite_texel_rect;

/// The mesh that draws a Stoneskin wizard's captured body: the warped 10-by-10 birth grid
/// with Enhanced Effects, otherwise a single quad over the whole capture.
pub(super) struct StoneskinMesh {
    birth_positions: Vec<f64>,
    enhanced_effects: bool,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl StoneskinMesh {
    pub(super) fn new(positions: &[f64], enhanced_effects: bool) -> Self {
        assert_eq!(
            positions.len(),
            STONESKIN_GRID_COMPONENTS,
            "Stoneskin has the complete native 10-by-10 birth grid"
        );
        let (vertices, indices) = if enhanced_effects {
            let vertices = positions
                .as_chunks::<2>()
                .0
                .iter()
                .enumerate()
                .map(|(index, xy)| Vertex {
                    position: [xy[0] as f32, xy[1] as f32],
                    texel: [
                        (fround(fround((index / 10) as f64 * 0.05_f32 as f64) + 0.25) * 256.0)
                            as f32,
                        // Render-target rows are inverted relative to uploaded PNGs.
                        ((1.0 - fround(fround((index % 10) as f64 * 0.05_f32 as f64) + 0.25))
                            * 256.0) as f32,
                    ],
                    tint: [1.0; 4],
                })
                .collect();
            let mut indices = Vec::with_capacity(486);
            for x in 0..9_u32 {
                for y in 0..9_u32 {
                    let a = x * 10 + y;
                    indices.extend([a, a + 10, a + 1, a + 1, a + 10, a + 11]);
                }
            }
            (vertices, indices)
        } else {
            // Sprite::Create `0x00417310` subtracts .5 from its whole-target
            // quad; the custom On grid above does not inherit that correction.
            let vertices = [
                ([-128.5, -128.5], [0.0, 256.0]),
                ([127.5, -128.5], [256.0, 256.0]),
                ([-128.5, 127.5], [0.0, 0.0]),
                ([127.5, 127.5], [256.0, 0.0]),
            ]
            .map(|(position, texel)| Vertex { position, texel, tint: [1.0; 4] });
            (vertices.to_vec(), vec![0, 1, 2, 2, 1, 3])
        };
        Self { birth_positions: positions.to_vec(), enhanced_effects, vertices, indices }
    }

    pub(super) fn matches(&self, positions: &[f64], enhanced_effects: bool) -> bool {
        self.enhanced_effects == enhanced_effects && self.birth_positions == positions
    }

    fn draw(&self, renderer: &mut Renderer, target: &Texture, camera: &Camera, position: Vec2) {
        // One view serves all participants. Keep its born mesh and use bounded
        // stack storage for each actor's camera projection without per-frame allocation.
        let mut vertices = [Vertex { position: [0.0; 2], texel: [0.0; 2], tint: [1.0; 4] }; 100];
        for (output, source) in vertices.iter_mut().zip(&self.vertices) {
            let projected = camera.project(
                position.x + f64::from(source.position[0]),
                position.y - 25.0 + f64::from(source.position[1]),
                0.0,
                0.0,
            );
            *output = Vertex { position: [projected[0], projected[1]], ..*source };
        }
        for _ in 0..2 {
            renderer.draw_indexed_with_color_mode(
                target,
                &vertices[..self.vertices.len()],
                &self.indices,
                BlendMode::Normal,
                TextureColorMode::Texture,
            );
        }
    }
}

impl WizardView {
    pub(super) fn draw_stoneskin_material(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
    ) {
        let mut captured_wizard = wizard.clone();
        captured_wizard.position = Vec2::ZERO;
        renderer.render_diffuse_to(&self.diffuse_capture, [256.0; 2], [0.0; 4], |renderer| {
            // Native bit1 suppresses the captured Shield. The ordinary hit
            // overlay is independent too; both stay outside the warped body.
            self.draw_actor(
                renderer,
                &Camera { origin: Vec2::new(-128.0, -153.0), scale: 1.0 },
                &captured_wizard,
                tick,
                spells,
                WizardDrawPass::Diffuse,
            );
        });
        renderer.render_onto(&self.diffuse_capture, [256.0; 2], |renderer| {
            let record = &self.records[3];
            let head = wizard_draw_plan(
                wizard.heading_index,
                wizard.walk_cycle_primary,
                wizard.gait_degrees,
            )
            .head_offset;
            super::material::draw_centered_texture(
                renderer,
                &Camera { origin: Vec2::ZERO, scale: 1.0 },
                &self.atlas.clamped_to(record.atlas_rect()),
                sprite_texel_rect(record),
                Vec2::new(128.0 + head.x, 128.0 + head.y),
                Vec2::new(record.width as f64, record.height as f64),
                wizard.heading_index as f64 * 24.0 * std::f64::consts::PI / 180.0,
                [1.0; 4],
                BlendMode::Multiply,
            );
        });
        self.stoneskin_mesh.as_ref().expect("active Stoneskin has a prepared material mesh").draw(
            renderer,
            &self.diffuse_capture.texture,
            camera,
            wizard.position,
        );
    }
}
