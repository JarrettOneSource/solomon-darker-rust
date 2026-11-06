//! Current Website Arena field and road geometry, built from the admitted scene.
//!
//! `native-arena-field.ts` owns the retained field lattice; the separate road
//! program is `renderer/native-boneyard-surface.ts`. Neither consumes game RNG.

mod field;
mod road;

use crate::Vec2;

pub use field::{
    ArenaFieldDescriptor, ArenaFieldLattice, ArenaFieldRange, arena_field_descriptor,
    arena_field_mesh_plan, arena_field_record, arena_field_tile_corners, arena_field_visible_range,
    create_arena_field_lattice,
};
pub use road::{road_endpoint_alphas, road_mesh_plan};

/// World vertices and normalized texture coordinates. GPU stores already occur
/// in the source plans, so each position/UV has been narrowed to f32 and widened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneyardSurfaceMesh {
    pub positions: Vec<Vec2>,
    pub uvs: Vec<Vec2>,
    pub colors: Vec<[u8; 4]>,
    pub indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoadMeshPlan {
    pub mesh: BoneyardSurfaceMesh,
    pub style: usize,
}
