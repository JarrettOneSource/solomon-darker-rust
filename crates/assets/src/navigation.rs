//! Prepared NavMesh loading for the exact shipped Boneyard catalog.
//!
//! Build-time preparation is `tools/website-oracle/navigation-goldens.mjs`. Runtime queries
//! live in Rust `sim::boneyard::navigation`; the asset identity must match the loaded scene.

mod catalog;

pub use catalog::CatalogNavigation;

use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::boneyard::navigation::{
    NavigationMesh, NavigationSet, NavigationTriangle, PreparedNavigationMesh,
    boneyard_world_navigation_preparations,
};
use solomon_sim::boneyard::{BoneyardBounds, LoadedBoneyard};

/// Reject mismatched sources, missing preparations and broken graph references before play.
pub fn decode_navigation(json: &str, loaded: &LoadedBoneyard) -> Result<NavigationSet, String> {
    let document: NavigationDocument =
        serde_json::from_str(json).map_err(|error| format!("Boneyard navigation: {error}"))?;
    if document.source_sha256 != loaded.source_sha256
        || document.geometry_sha256 != loaded.geometry_sha256
    {
        return Err("Boneyard navigation identity does not match the loaded scene".into());
    }
    let mut meshes = Vec::with_capacity(document.meshes.len());
    for prepared in document.meshes {
        let bounds = BoneyardBounds::new(
            prepared.bounds.x,
            prepared.bounds.y,
            prepared.bounds.w,
            prepared.bounds.h,
        );
        let points: Vec<Vec2> =
            prepared.mesh.points.into_iter().map(|point| Vec2::new(point.x, point.y)).collect();
        let mut triangles = Vec::with_capacity(prepared.mesh.triangles.len());
        let triangle_count = prepared.mesh.triangles.len();
        if points.is_empty() || triangle_count == 0 {
            return Err("Boneyard navigation mesh is empty".into());
        }
        for (index, triangle) in prepared.mesh.triangles.into_iter().enumerate() {
            if triangle.id != index
                || triangle.vertices.iter().any(|vertex| *vertex >= points.len())
                || triangle.neighbors.iter().any(|neighbor| *neighbor >= triangle_count)
            {
                return Err("Boneyard navigation mesh has an invalid triangle reference".into());
            }
            triangles.push(NavigationTriangle {
                center: Vec2::new(triangle.center.x, triangle.center.y),
                id: triangle.id,
                neighbors: triangle.neighbors,
                vertices: triangle.vertices,
            });
        }
        meshes.push(PreparedNavigationMesh {
            bounds,
            clearance: prepared.clearance,
            mesh: NavigationMesh { points, triangles },
        });
    }
    // Install only the domains the source eagerly prepares. Other domains retain
    // their actual first-query collision, including moving gates, in the runtime cache.
    let mut prepared = Vec::new();
    for (bounds, clearance) in boneyard_world_navigation_preparations(loaded) {
        let Some(index) =
            meshes.iter().position(|mesh| mesh.bounds == bounds && mesh.clearance == clearance)
        else {
            return Err(
                "Boneyard navigation lacks a required bounds and clearance preparation".into()
            );
        };
        prepared.push(meshes.remove(index));
    }
    Ok(NavigationSet::new(document.source_sha256, document.geometry_sha256, prepared))
}

#[derive(Deserialize)]
struct PointDocument {
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
struct BoundsDocument {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Deserialize)]
struct TriangleDocument {
    center: PointDocument,
    id: usize,
    neighbors: Vec<usize>,
    vertices: [usize; 3],
}

#[derive(Deserialize)]
struct MeshDocument {
    points: Vec<PointDocument>,
    triangles: Vec<TriangleDocument>,
}

#[derive(Deserialize)]
struct PreparedDocument {
    bounds: BoundsDocument,
    clearance: f64,
    mesh: MeshDocument,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NavigationDocument {
    source_sha256: String,
    geometry_sha256: String,
    meshes: Vec<PreparedDocument>,
}
