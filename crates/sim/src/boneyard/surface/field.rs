//! `native-arena-field.ts`: exact original-page UVs and the f32 tile recurrence.

use crate::Vec2;
use crate::boneyard::BoneyardBounds;
use crate::float::fround;

use super::NativeBoneyardSurfaceMesh;

const TILE_SIZE: f64 = 350.0;
const MAX_AXIS_ENTRIES: usize = 1_000_000;
const MAX_VISIBLE_TILES: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeArenaFieldDescriptor {
    pub entry: usize,
    pub frame: [f64; 4],
    pub uvs: [Vec2; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeArenaFieldLattice {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeArenaFieldRange {
    pub x_start: usize,
    pub x_end: usize,
    pub y_start: usize,
    pub y_end: usize,
    pub tile_count: usize,
}

/// Arena 0046F457/45F: only mode bytes 1 and 2 select record 11.
#[must_use]
pub const fn native_arena_field_record(mode: u8) -> usize {
    if mode == 1 || mode == 2 { 11 } else { 12 }
}

#[must_use]
pub fn native_arena_field_descriptor(mode: u8) -> NativeArenaFieldDescriptor {
    let entry = native_arena_field_record(mode);
    // The original DeadHawg page, as validated by current native-arena-field.ts.
    // These are atlas texels, never the loose 350px crop or a repeating ground tile.
    let [x, y] = if entry == 11 { [778.0, 859.0] } else { [1399.0, 1069.0] };
    let left = fround((x + 0.5) / 2048.0);
    let top = fround((y + 0.5) / 2048.0);
    let right = fround((x + TILE_SIZE + 0.25) / 2048.0);
    let bottom = fround((y + TILE_SIZE + 0.25) / 2048.0);
    NativeArenaFieldDescriptor {
        entry,
        frame: [x, y, TILE_SIZE, TILE_SIZE],
        uvs: [
            Vec2::new(left, top),
            Vec2::new(right, top),
            Vec2::new(left, bottom),
            Vec2::new(right, bottom),
        ],
    }
}

pub fn create_native_arena_field_lattice(
    bounds: BoneyardBounds,
) -> Result<NativeArenaFieldLattice, String> {
    Ok(NativeArenaFieldLattice {
        x: field_axis(bounds.x, bounds.width)?,
        y: field_axis(bounds.y, bounds.height)?,
    })
}

/// Strict primary-view overlap. The shared scenery cull uses different padding
/// and inclusive edges; it must not be substituted for this source range.
pub fn native_arena_field_visible_range(
    lattice: &NativeArenaFieldLattice,
    view: BoneyardBounds,
) -> Result<NativeArenaFieldRange, String> {
    let x = finite_f32(view.x)?;
    let y = finite_f32(view.y)?;
    let width = finite_f32(view.width)?;
    let height = finite_f32(view.height)?;
    if width < 0.0 || height < 0.0 {
        return Err("Native Arena field view size cannot be negative".into());
    }
    let right = finite_f32(x + width)?;
    let bottom = finite_f32(y + height)?;
    // The native end comparison adds 350 without another f32 store.
    let x_start = lattice.x.partition_point(|value| value + TILE_SIZE <= x);
    let y_start = lattice.y.partition_point(|value| value + TILE_SIZE <= y);
    let x_end = x_start.max(lattice.x.partition_point(|value| *value < right));
    let y_end = y_start.max(lattice.y.partition_point(|value| *value < bottom));
    Ok(NativeArenaFieldRange {
        x_start,
        x_end,
        y_start,
        y_end,
        tile_count: (x_end - x_start) * (y_end - y_start),
    })
}

/// TL, TR, BL, BR: SpriteDraw stores its center before adding local corners.
#[must_use]
pub fn native_arena_field_tile_corners(x: f64, y: f64) -> [Vec2; 4] {
    let center_x = fround(x + TILE_SIZE / 2.0);
    let center_y = fround(y + TILE_SIZE / 2.0);
    let left = fround(center_x - TILE_SIZE / 2.0);
    let top = fround(center_y - TILE_SIZE / 2.0);
    let right = fround(center_x + TILE_SIZE / 2.0);
    let bottom = fround(center_y + TILE_SIZE / 2.0);
    [Vec2::new(left, top), Vec2::new(right, top), Vec2::new(left, bottom), Vec2::new(right, bottom)]
}

pub fn native_arena_field_mesh_plan(
    lattice: &NativeArenaFieldLattice,
    range: NativeArenaFieldRange,
    mode: u8,
) -> Result<NativeBoneyardSurfaceMesh, String> {
    if range.x_start > range.x_end
        || range.x_end > lattice.x.len()
        || range.y_start > range.y_end
        || range.y_end > lattice.y.len()
        || range.tile_count != (range.x_end - range.x_start) * (range.y_end - range.y_start)
    {
        return Err("Native Arena field range must fit its retained axes".into());
    }
    if range.tile_count > MAX_VISIBLE_TILES {
        return Err("Native Arena field visible mesh exceeds the safe resource limit".into());
    }
    let descriptor = native_arena_field_descriptor(mode);
    let mut mesh = NativeBoneyardSurfaceMesh::default();
    mesh.positions.reserve(range.tile_count * 4);
    mesh.uvs.reserve(range.tile_count * 4);
    mesh.colors.resize(range.tile_count * 4, [255; 4]);
    mesh.indices.reserve(range.tile_count * 6);
    for &x in &lattice.x[range.x_start..range.x_end] {
        for &y in &lattice.y[range.y_start..range.y_end] {
            let vertex = mesh.positions.len();
            mesh.positions.extend(native_arena_field_tile_corners(x, y));
            mesh.uvs.extend(descriptor.uvs);
            mesh.indices.extend([
                vertex,
                vertex + 1,
                vertex + 2,
                vertex + 1,
                vertex + 3,
                vertex + 2,
            ]);
        }
    }
    Ok(mesh)
}

fn field_axis(origin: f64, extent: f64) -> Result<Vec<f64>, String> {
    let start = finite_f32(origin)?;
    let length = finite_f32(extent)?;
    if extent <= 0.0 || length <= 0.0 {
        return Err("Native Arena field requires positive bounds".into());
    }
    let end = finite_f32(start + length)?;
    let mut values = Vec::new();
    let mut value = start;
    while value < end {
        if values.len() == MAX_AXIS_ENTRIES {
            return Err("Native Arena field axis exceeds the safe resource limit".into());
        }
        values.push(value);
        let next = fround(value + TILE_SIZE);
        if !next.is_finite() || next <= value {
            return Err("Native Arena field axis does not advance in float32".into());
        }
        value = next;
    }
    Ok(values)
}

fn finite_f32(value: f64) -> Result<f64, String> {
    let rounded = fround(value);
    if value.is_finite() && rounded.is_finite() {
        Ok(rounded)
    } else {
        Err("Native Arena field requires finite float32 coordinates".into())
    }
}
