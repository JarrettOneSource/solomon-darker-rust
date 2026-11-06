//! Arena ground geometry built from the admitted scene: the tiled field and the roads.
//!
//! `field` owns the retained field lattice; `road` builds the separate road
//! meshes. Neither consumes game RNG.

mod field;
mod road;

use crate::Vec2;

pub use field::{
    ArenaFieldDescriptor, ArenaFieldLattice, ArenaFieldRange, arena_field_descriptor,
    arena_field_mesh_plan, arena_field_record, arena_field_tile_corners, arena_field_visible_range,
    create_arena_field_lattice,
};
pub use road::{road_endpoint_alphas, road_mesh_plan};

/// World vertices and normalized texture coordinates. The plans already apply the
/// GPU's f32 stores, so each position/UV has been narrowed to f32 and widened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneyardSurfaceMesh {
    pub positions: Vec<Vec2>,
    pub uvs: Vec<Vec2>,
    pub colors: Vec<[u8; 4]>,
    pub indices: Vec<usize>,
}

/// A road's surface mesh and the index of the road style it was built with.
#[derive(Clone, Debug, PartialEq)]
pub struct RoadMeshPlan {
    pub mesh: BoneyardSurfaceMesh,
    pub style: usize,
}
