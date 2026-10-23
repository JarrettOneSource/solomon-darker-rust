//! NativeElectricBurnArcView's body-only, independently masked painter bands.
use super::SecondaryView;
use super::draws::rgba;
use crate::courtyard::Camera;
use crate::native_mesh::clip_native_mesh;
use solomon_graphics::{BlendMode, Renderer, Vertex};
use solomon_sim::secondary::presentation::NativeSecondaryAtlas;

impl SecondaryView {
    pub(super) fn draw_electric_burn_arc_band(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        actor_index: usize,
        band_index: usize,
        tint: [f32; 3],
    ) {
        let actor = &self.actors[actor_index];
        let arc = actor.electric_arc.as_ref().expect("ElectricBurn painter retains its born body");
        let band = arc.bands[band_index];
        for layer in &arc.body {
            let meshes = std::iter::once((&layer.mesh, layer.texture_record))
                .chain(layer.branch.iter().map(|branch| (&branch.mesh, branch.texture_record)));
            let color = rgba(layer.tint, layer.alpha, tint);
            for (mesh, entry) in meshes {
                let record = self.record(NativeSecondaryAtlas::BadGuys, entry);
                let [top_left, _, bottom_right, _] = super::draws::texels(&record);
                let triangles = clip_native_mesh(
                    mesh,
                    actor.plan.root,
                    band.clip_y,
                    band.clip_y + band.clip_height,
                )
                .into_iter()
                .map(|triangle| {
                    triangle.map(|vertex| {
                        let projected =
                            camera.project(vertex.position.x, vertex.position.y, 0.0, 0.0);
                        Vertex {
                            position: [projected[0], projected[1]],
                            texel: [
                                top_left[0] + vertex.uv.x as f32 * (bottom_right[0] - top_left[0]),
                                top_left[1] + vertex.uv.y as f32 * (bottom_right[1] - top_left[1]),
                            ],
                            tint: color,
                        }
                    })
                })
                .collect::<Vec<_>>();
                renderer.draw_triangles(record.texture, &triangles, BlendMode::Add);
            }
        }
    }
}
