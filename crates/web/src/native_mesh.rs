//! Native ZAnimSplit masks shared by primary and enemy beam views.

use solomon_sim::Vec2;
use solomon_sim::spells::air::NativeAirMesh;
use solomon_sim::spells::weld::NativeFlameLashBounds;

#[derive(Clone, Copy)]
pub(crate) struct NativeMeshVertex {
    pub position: Vec2,
    pub uv: Vec2,
    /// Straight native channels. Clipping interpolates color with position/UV.
    pub color: [f64; 4],
}

/// Clip stock beam triangles into their native ZAnimSplit painter bands.
/// Interpolating UVs preserves the actual atlas glyph where a band cuts a triangle.
pub(crate) fn clip_native_mesh(
    mesh: &NativeAirMesh,
    origin: Vec2,
    top: f64,
    bottom: f64,
) -> Vec<[NativeMeshVertex; 3]> {
    clip_mesh(
        mesh,
        origin,
        [(1, top, true), (1, bottom, false), (0, -origin.x, true), (0, 10_000.0 - origin.x, false)],
        None,
    )
}

/// Flame masks intersect the complete Region rectangle, and its transparent
/// native caps carry independently interpolated vertex RGB and alpha.
pub(crate) fn clip_native_mesh_in_rect(
    mesh: &NativeAirMesh,
    origin: Vec2,
    clip: NativeFlameLashBounds,
    colors: Option<&[u32]>,
) -> Vec<[NativeMeshVertex; 3]> {
    if clip.width <= 0.0 || clip.height <= 0.0 {
        return Vec::new();
    }
    clip_mesh(
        mesh,
        origin,
        [
            (1, clip.y - origin.y, true),
            (1, clip.y + clip.height - origin.y, false),
            (0, clip.x - origin.x, true),
            (0, clip.x + clip.width - origin.x, false),
        ],
        colors,
    )
}

fn clip_mesh(
    mesh: &NativeAirMesh,
    origin: Vec2,
    edges: [(usize, f64, bool); 4],
    colors: Option<&[u32]>,
) -> Vec<[NativeMeshVertex; 3]> {
    let mut result = Vec::new();
    let (triangles, _) = mesh.indices.as_chunks::<3>();
    for triangle in triangles {
        let mut polygon: Vec<NativeMeshVertex> = triangle
            .iter()
            .map(|index| {
                let index = *index as usize;
                NativeMeshVertex {
                    position: mesh.vertices[index],
                    uv: mesh.uvs[index],
                    color: colors.map_or([1.0; 4], |values| {
                        let value = values[index];
                        // Pixi's aColor is unorm8x4 in packed native byte order.
                        [value & 255, (value >> 8) & 255, (value >> 16) & 255, value >> 24]
                            .map(|channel| f64::from(channel) / 255.0)
                    }),
                }
            })
            .collect();
        for (axis, limit, keep_greater) in edges {
            polygon = clip_edge(polygon, axis, limit, keep_greater);
        }
        for index in 1..polygon.len().saturating_sub(1) {
            let mut triangle = [polygon[0], polygon[index], polygon[index + 1]];
            for vertex in &mut triangle {
                vertex.position.x += origin.x;
                vertex.position.y += origin.y;
            }
            result.push(triangle);
        }
    }
    result
}

fn clip_edge(
    polygon: Vec<NativeMeshVertex>,
    axis: usize,
    limit: f64,
    keep_greater: bool,
) -> Vec<NativeMeshVertex> {
    let coordinate =
        |vertex: NativeMeshVertex| if axis == 0 { vertex.position.x } else { vertex.position.y };
    let inside = |value: f64| if keep_greater { value >= limit } else { value <= limit };
    let mut result = Vec::new();
    let Some(mut previous) = polygon.last().copied() else {
        return result;
    };
    for current in polygon {
        let previous_coordinate = coordinate(previous);
        let current_coordinate = coordinate(current);
        if inside(previous_coordinate) != inside(current_coordinate) {
            let fraction =
                (limit - previous_coordinate) / (current_coordinate - previous_coordinate);
            result.push(NativeMeshVertex {
                position: Vec2::new(
                    previous.position.x + (current.position.x - previous.position.x) * fraction,
                    previous.position.y + (current.position.y - previous.position.y) * fraction,
                ),
                uv: Vec2::new(
                    previous.uv.x + (current.uv.x - previous.uv.x) * fraction,
                    previous.uv.y + (current.uv.y - previous.uv.y) * fraction,
                ),
                color: std::array::from_fn(|channel| {
                    previous.color[channel]
                        + (current.color[channel] - previous.color[channel]) * fraction
                }),
            });
        }
        if inside(current_coordinate) {
            result.push(current);
        }
        previous = current;
    }
    result
}

/// `native-sprite-record-texture.ts::nativeSpriteRecordUvs`: native samples
/// start half a texel into the record and end a quarter texel beyond its edge.
pub(crate) fn native_sprite_texel_rect(record: &solomon_assets::SpriteRecord) -> [f32; 4] {
    native_sprite_frame_texel_rect([
        f64::from(record.x),
        f64::from(record.y),
        f64::from(record.width),
        f64::from(record.height),
    ])
}

/// Bitmap glyphs share the native texture's half/quarter-texel registration.
pub(crate) fn native_sprite_frame_texel_rect(frame: [f64; 4]) -> [f32; 4] {
    let [x, y, width, height] = frame;
    [x as f32 + 0.5, y as f32 + 0.5, width as f32 - 0.25, height as f32 - 0.25]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_masks_interpolate_native_caps_and_atlas_coordinates_together() {
        let mesh = NativeAirMesh {
            indices: vec![0, 1, 2],
            vertices: vec![Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0)],
            uvs: vec![Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
        };
        let triangles = clip_native_mesh_in_rect(
            &mesh,
            Vec2::new(100.0, 200.0),
            NativeFlameLashBounds { x: 102.0, y: 202.0, width: 4.0, height: 4.0 },
            Some(&[0xffff_ffff, 0, 0]),
        );
        assert!(!triangles.is_empty());
        for vertex in triangles.iter().flatten() {
            assert!((102.0..=106.0).contains(&vertex.position.x));
            assert!((202.0..=206.0).contains(&vertex.position.y));
            assert!((vertex.position.x + vertex.position.y - 300.0) <= 10.0);
            assert!((vertex.uv.x - (vertex.position.x - 100.0) / 10.0).abs() < 1e-12);
            assert!((vertex.uv.y - (vertex.position.y - 200.0) / 10.0).abs() < 1e-12);
            for channel in vertex.color {
                assert!((channel - (1.0 - vertex.uv.x - vertex.uv.y)).abs() < 1e-12);
            }
        }
    }
}
