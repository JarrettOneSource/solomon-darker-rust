//! Current Website Arena field and road geometry, built from the admitted scene.
//!
//! `native-arena-field.ts` owns the retained field lattice; the separate road
//! program is `renderer/native-boneyard-surface.ts`. Neither consumes game RNG.

mod field;
mod road;

use crate::Vec2;

pub use field::{
    NativeArenaFieldDescriptor, NativeArenaFieldLattice, NativeArenaFieldRange,
    create_native_arena_field_lattice, native_arena_field_descriptor, native_arena_field_mesh_plan,
    native_arena_field_record, native_arena_field_tile_corners, native_arena_field_visible_range,
};
pub use road::{native_road_endpoint_alphas, native_road_mesh_plan};

/// World vertices and normalized texture coordinates. GPU stores already occur
/// in the source plans, so each position/UV has been narrowed to f32 and widened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeBoneyardSurfaceMesh {
    pub positions: Vec<Vec2>,
    pub uvs: Vec<Vec2>,
    pub colors: Vec<[u8; 4]>,
    pub indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeRoadMeshPlan {
    pub mesh: NativeBoneyardSurfaceMesh,
    pub style: usize,
}
