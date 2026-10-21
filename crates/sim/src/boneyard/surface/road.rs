//! `renderer/native-boneyard-surface.ts`: the five native road style programs.

use crate::Vec2;
use crate::boneyard::BoneyardRoad;
use crate::float::fround;

use super::{NativeBoneyardSurfaceMesh, NativeRoadMeshPlan};

struct RoadStyle {
    edge_inset: f64,
    half_width: f64,
    texture_size: f64,
}

const ROAD_STYLES: [RoadStyle; 5] = [
    RoadStyle { edge_inset: 30.0, half_width: 55.0, texture_size: 128.0 },
    RoadStyle { edge_inset: 20.0, half_width: 45.0, texture_size: 128.0 },
    RoadStyle { edge_inset: 20.0, half_width: 55.0, texture_size: 256.0 },
    RoadStyle { edge_inset: 10.0, half_width: 45.0, texture_size: 128.0 },
    RoadStyle { edge_inset: 10.0, half_width: 55.0, texture_size: 128.0 },
];

#[derive(Clone, Copy, PartialEq)]
struct RoadVertex {
    position: Vec2,
    alpha: u8,
}

#[must_use]
pub fn native_road_endpoint_alphas(link_mask: u8) -> [f64; 2] {
    if link_mask == 0 {
        [1.0, 1.0]
    } else {
        let start_alpha = if link_mask & 1 != 0 { 1.0 } else { 0.0 };
        let end_alpha = if link_mask & 2 != 0 { 1.0 } else { 0.0 };
        [start_alpha, end_alpha]
    }
}

pub fn native_road_mesh_plan(road: &BoneyardRoad) -> Result<NativeRoadMeshPlan, String> {
    let style = road.style.unwrap_or(0) as usize;
    let program = ROAD_STYLES
        .get(style)
        .ok_or_else(|| format!("Road {} has invalid native style {style}", road.eid))?;
    let outer = if let Some(quad) = &road.quad
        && quad.len() == 4
    {
        [quad[0], quad[1], quad[2], quad[3]].map(stored_point)
    } else {
        native_road_quad(road, program.half_width)?
    };
    let [start_left, start_right, end_left, end_right] = outer;
    let [start_alpha, end_alpha] = native_road_endpoint_alphas(road.link_mask);
    let [inner_start_left, inner_start_right] =
        inset_pair(start_left, start_right, program.edge_inset);
    let [inner_end_left, inner_end_right] = inset_pair(end_left, end_right, program.edge_inset);
    let vertex = |position, alpha| RoadVertex { position: stored_point(position), alpha };
    let start_alpha = (start_alpha * 255.0) as u8;
    let end_alpha = (end_alpha * 255.0) as u8;
    let source = [
        vertex(inner_start_left, start_alpha),
        vertex(inner_start_right, start_alpha),
        vertex(inner_end_left, end_alpha),
        vertex(inner_start_right, start_alpha),
        vertex(inner_end_left, end_alpha),
        vertex(inner_end_right, end_alpha),
        vertex(start_left, 0),
        vertex(inner_start_left, start_alpha),
        vertex(end_left, 0),
        vertex(inner_start_left, start_alpha),
        vertex(end_left, 0),
        vertex(inner_end_left, end_alpha),
        vertex(inner_start_right, start_alpha),
        vertex(start_right, 0),
        vertex(inner_end_right, end_alpha),
        vertex(start_right, 0),
        vertex(inner_end_right, end_alpha),
        vertex(end_right, 0),
    ];
    let mut unique = Vec::new();
    let mut mesh = NativeBoneyardSurfaceMesh::default();
    for vertex in source {
        let index = if let Some(index) = unique.iter().position(|value| *value == vertex) {
            index
        } else {
            let index = unique.len();
            unique.push(vertex);
            mesh.positions.push(vertex.position);
            mesh.uvs.push(Vec2::new(
                fround(vertex.position.x / program.texture_size),
                fround(fround(vertex.position.y / program.texture_size) / (0.8_f32 as f64)),
            ));
            mesh.colors.push([255, 255, 255, vertex.alpha]);
            index
        };
        mesh.indices.push(index);
    }
    Ok(NativeRoadMeshPlan { mesh, style })
}

fn native_road_quad(road: &BoneyardRoad, half_width: f64) -> Result<[Vec2; 4], String> {
    if road.points.len() < 2 {
        return Err(format!("Road {} requires four outer points", road.eid));
    }
    let [start, end] = [road.points[0], road.points[1]];
    let delta = Vec2::new(fround(end.x - start.x), fround(end.y - start.y));
    let length = stored_length(delta);
    let unit = if length == 0.0 {
        Vec2::ZERO
    } else {
        Vec2::new(fround(delta.x / length), fround(delta.y / length))
    };
    let start_width = fround(half_width * fround(road.start_width_scale.unwrap_or(1.0)));
    let end_width = fround(half_width * fround(road.end_width_scale.unwrap_or(1.0)));
    Ok([
        Vec2::new(
            fround(start.x - fround(unit.y * start_width)),
            fround(start.y + fround(unit.x * start_width)),
        ),
        Vec2::new(
            fround(start.x + fround(unit.y * start_width)),
            fround(start.y - fround(unit.x * start_width)),
        ),
        Vec2::new(
            fround(end.x - fround(unit.y * end_width)),
            fround(end.y + fround(unit.x * end_width)),
        ),
        Vec2::new(
            fround(end.x + fround(unit.y * end_width)),
            fround(end.y - fround(unit.x * end_width)),
        ),
    ])
}

fn inset_pair(left: Vec2, right: Vec2, inset: f64) -> [Vec2; 2] {
    let delta = Vec2::new(fround(right.x - left.x), fround(right.y - left.y));
    let length = stored_length(delta);
    let factor = if length > 0.0 { fround(inset / length) } else { 0.0 };
    let offset = Vec2::new(fround(delta.x * factor), fround(delta.y * factor));
    [
        Vec2::new(fround(left.x + offset.x), fround(left.y + offset.y)),
        Vec2::new(fround(right.x - offset.x), fround(right.y - offset.y)),
    ]
}

fn stored_point(point: Vec2) -> Vec2 {
    Vec2::new(fround(point.x), fround(point.y))
}

fn stored_length(point: Vec2) -> f64 {
    let squared = fround(fround(point.x * point.x) + fround(point.y * point.y));
    if squared == 0.0 { 0.0 } else { fround(squared.sqrt()) }
}
