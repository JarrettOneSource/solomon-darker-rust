//! Directional scenery shadows from `renderer/boneyard-complex-shadows.ts`.
//!
//! Authored outlines stay in the scenery manifest. This module projects the live light
//! field through those outlines and the stock fence, gate, rail and wall programs.

use crate::Vec2;
use crate::float::{fround, hypot};

use super::lighting::{
    NATIVE_LIGHT_OUTER_DISTANCE, NATIVE_LIGHT_VERTICAL_SCALE, NativeBoneyardLightIndex,
    NativeBoneyardLightSource, stable_string_hash,
};

const FENCE_BAR_STEP: f64 = 13.333333015441895;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeShadowCaster {
    pub id: String,
    pub position: Vec2,
    pub outline: Vec<Vec2>,
    pub program: Option<NativeShadowProgram>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativeShadowProgram {
    BrokenFenceGrate { start: Vec2, end: Vec2, step: Vec2, count: usize },
    FenceGrate { start: Vec2, end: Vec2, gate: bool },
    Rails { start: Vec2, end: Vec2 },
    Wall { start: Vec2, end: Vec2 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeShadowRecord {
    pub base_alpha: f64,
    pub behind_scalar: f64,
    pub direction: Vec2,
    pub distance_fraction: f64,
    pub projection_distance: f64,
    pub source_position: Vec2,
    pub source_radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeShadowEdge {
    pub base_alpha: f64,
    pub base_end: Vec2,
    pub base_start: Vec2,
    pub tip_alpha: f64,
    pub tip_end: Vec2,
    pub tip_start: Vec2,
}

#[must_use]
pub fn native_shadow_records(
    caster: &NativeShadowCaster,
    lights: &NativeBoneyardLightIndex,
    presentation_frame: u64,
) -> Vec<NativeShadowRecord> {
    let seed = stable_string_hash(&caster.id);
    let mut records = Vec::new();
    for &source_index in lights.source_indices_at(caster.position) {
        let source = lights.accepted_sources()[source_index];
        if !source.casts_directional_shadow || source.radius <= 0.0 {
            continue;
        }
        let world_dx = caster.position.x - source.position.x;
        let world_dy = caster.position.y - source.position.y;
        let dx = world_dx / source.radius;
        let dy = world_dy / (NATIVE_LIGHT_VERTICAL_SCALE * source.radius);
        let distance_squared = dx * dx + dy * dy;
        if distance_squared >= NATIVE_LIGHT_OUTER_DISTANCE * NATIVE_LIGHT_OUTER_DISTANCE {
            continue;
        }
        let direction = normalized(source.position, caster.position);
        records.push(NativeShadowRecord {
            base_alpha: 1.0,
            behind_scalar: lights.scalar_at(add(caster.position, direction)),
            direction,
            distance_fraction: distance_squared
                / (NATIVE_LIGHT_OUTER_DISTANCE * NATIVE_LIGHT_OUTER_DISTANCE),
            projection_distance: (NATIVE_LIGHT_OUTER_DISTANCE
                - presentation_random(seed, source, source_index, presentation_frame))
                * source.radius,
            source_position: source.position,
            source_radius: source.radius,
        });
    }
    for index in 0..records.len() {
        let mut alpha = 1.0;
        for other in 0..records.len() {
            if index == other {
                continue;
            }
            let dot = records[index].direction.x * records[other].direction.x
                + records[index].direction.y * records[other].direction.y;
            alpha *= dot.max(records[other].distance_fraction);
        }
        records[index].base_alpha = alpha.clamp(0.0, 1.0);
    }
    records
}

#[must_use]
pub fn native_shadow_edges(
    caster: &NativeShadowCaster,
    lights: &NativeBoneyardLightIndex,
    presentation_frame: u64,
) -> Vec<NativeShadowEdge> {
    let mut edges = Vec::new();
    for record in native_shadow_records(caster, lights, presentation_frame) {
        match caster.program {
            Some(NativeShadowProgram::BrokenFenceGrate { start, end, step, count }) => {
                if count > 0 {
                    fence_grate_baseline(start, end, step, count, record, &mut edges);
                } else {
                    edges.push(native_shadow_line(start, end, 4.0, 0.0));
                }
            }
            Some(NativeShadowProgram::FenceGrate { start, end, gate }) => {
                fence_grate(start, end, gate, record, &mut edges);
            }
            Some(NativeShadowProgram::Rails { start, end }) => {
                rail_shadows(start, end, record, &mut edges);
            }
            Some(NativeShadowProgram::Wall { start, end }) => {
                edges.push(projected_edge(start, end, record));
            }
            None => {
                if caster.outline.len() < 3 {
                    continue;
                }
                for index in 0..caster.outline.len() {
                    let start = add(caster.position, caster.outline[index]);
                    let end =
                        add(caster.position, caster.outline[(index + 1) % caster.outline.len()]);
                    let edge_x = end.x - start.x;
                    let edge_y = end.y - start.y;
                    let length = hypot(edge_x, edge_y);
                    if length == 0.0 {
                        continue;
                    }
                    let midpoint = Vec2::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
                    let away_x = midpoint.x - record.source_position.x;
                    let away_y = midpoint.y - record.source_position.y;
                    if away_x * (edge_y / length) + away_y * (-edge_x / length) > 0.0 {
                        edges.push(projected_edge(start, end, record));
                    }
                }
            }
        }
    }
    edges
}

fn projected_edge(start: Vec2, end: Vec2, record: NativeShadowRecord) -> NativeShadowEdge {
    NativeShadowEdge {
        base_alpha: record.base_alpha,
        base_start: start,
        base_end: end,
        tip_alpha: ((1.0 - record.behind_scalar) * (1.0 - record.distance_fraction))
            .powf(3.0)
            .clamp(0.0, 1.0),
        tip_start: project_away(start, record.source_position, record.projection_distance),
        tip_end: project_away(end, record.source_position, record.projection_distance),
    }
}

fn fence_grate(
    start: Vec2,
    end: Vec2,
    gate: bool,
    record: NativeShadowRecord,
    edges: &mut Vec<NativeShadowEdge>,
) {
    let dx = fround(end.x - start.x);
    let dy = fround(end.y - start.y);
    let length = stored_length(dx, dy);
    let inset = if gate { 4.0 } else { 12.0 };
    if length <= inset * 2.0 {
        edges.push(native_shadow_line(start, end, 4.0, 0.0));
        return;
    }
    let inverse = fround(1.0 / length);
    let along = Vec2::new(fround(dx * inverse), fround(dy * inverse));
    let short_start = Vec2::new(
        fround(start.x + fround(along.x * inset)),
        fround(start.y + fround(along.y * inset)),
    );
    let short_end =
        Vec2::new(fround(end.x - fround(along.x * inset)), fround(end.y - fround(along.y * inset)));
    let short_length =
        stored_length(fround(short_end.x - short_start.x), fround(short_end.y - short_start.y));
    let nominal_step = if gate { fround(short_length / 4.5) } else { FENCE_BAR_STEP };
    let step = Vec2::new(fround(along.x * nominal_step), fround(along.y * nominal_step));
    let count = ((short_length / stored_length(step.x, step.y)).trunc() as usize + 1).max(1);
    fence_grate_baseline(short_start, short_end, step, count, record, edges);
}

fn fence_grate_baseline(
    short_start: Vec2,
    short_end: Vec2,
    step: Vec2,
    count: usize,
    record: NativeShadowRecord,
    edges: &mut Vec<NativeShadowEdge>,
) {
    for index in 0..count {
        let center = Vec2::new(
            fround(short_start.x + fround(step.x * (index as f64 + 0.5))),
            fround(short_start.y + fround(step.y * (index as f64 + 0.5))),
        );
        let away = normalized(record.source_position, center);
        let perpendicular = Vec2::new(-away.y, away.x);
        let far = Vec2::new(
            center.x + away.x * record.projection_distance,
            center.y + away.y * record.projection_distance,
        );
        edges.push(NativeShadowEdge {
            base_alpha: record.base_alpha,
            base_start: Vec2::new(
                center.x - perpendicular.x * 2.0,
                center.y - perpendicular.y * 2.0,
            ),
            base_end: Vec2::new(center.x + perpendicular.x * 2.0, center.y + perpendicular.y * 2.0),
            tip_alpha: 0.0,
            tip_start: Vec2::new(far.x - perpendicular.x * 8.0, far.y - perpendicular.y * 8.0),
            tip_end: Vec2::new(far.x + perpendicular.x * 8.0, far.y + perpendicular.y * 8.0),
        });
    }
    let rail_distance = record.projection_distance * 0.125;
    edges.push(native_shadow_line(
        project_away(short_start, record.source_position, rail_distance),
        project_away(short_end, record.source_position, rail_distance),
        4.0,
        (0.1 * record.behind_scalar + 0.9 * record.base_alpha).clamp(0.0, 1.0),
    ));
}

fn rail_shadows(
    start: Vec2,
    end: Vec2,
    record: NativeShadowRecord,
    edges: &mut Vec<NativeShadowEdge>,
) {
    let dx = fround(end.x - start.x);
    let dy = fround(end.y - start.y);
    let length = stored_length(dx, dy);
    let inverse = if length > 0.0 { fround(1.0 / length) } else { 0.0 };
    let along = Vec2::new(fround(dx * inverse), fround(dy * inverse));
    let start =
        Vec2::new(fround(start.x + fround(along.x * 4.0)), fround(start.y + fround(along.y * 4.0)));
    let shortened_end =
        Vec2::new(fround(end.x - fround(along.x * 4.0)), fround(end.y - fround(along.y * 4.0)));
    let short_length =
        stored_length(fround(shortened_end.x - start.x), fround(shortened_end.y - start.y));
    let step = Vec2::new(fround(along.x * FENCE_BAR_STEP), fround(along.y * FENCE_BAR_STEP));
    let step_length = stored_length(step.x, step.y);
    let count = if step_length > 0.0 { (short_length / step_length).trunc() + 1.0 } else { 0.0 };
    let far = Vec2::new(
        fround(start.x + fround(count * step.x)),
        fround(start.y + fround(count * step.y)),
    );
    let alpha = (0.9 * record.base_alpha + 0.1 * record.behind_scalar).clamp(0.0, 1.0);
    for divisor in [5.0, 1.5] {
        let project = |point: Vec2| {
            Vec2::new(
                point.x - (record.source_position.x - point.x) / divisor,
                point.y - (record.source_position.y - point.y) / divisor,
            )
        };
        edges.push(native_shadow_line(project(start), project(far), 10.0, alpha));
    }
}

#[must_use]
pub fn native_shadow_line(start: Vec2, end: Vec2, width: f64, alpha: f64) -> NativeShadowEdge {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = hypot(dx, dy);
    let half_width_scale = if length > 0.0 { width / 2.0 / length } else { 0.0 };
    let perpendicular = Vec2::new(-dy * half_width_scale, dx * half_width_scale);
    NativeShadowEdge {
        base_alpha: alpha,
        base_start: Vec2::new(start.x - perpendicular.x, start.y - perpendicular.y),
        base_end: add(start, perpendicular),
        tip_alpha: alpha,
        tip_start: Vec2::new(end.x - perpendicular.x, end.y - perpendicular.y),
        tip_end: add(end, perpendicular),
    }
}

fn stored_length(dx: f64, dy: f64) -> f64 {
    fround(fround(fround(dx * dx) + fround(dy * dy)).sqrt())
}

fn project_away(point: Vec2, source: Vec2, distance: f64) -> Vec2 {
    let direction = normalized(source, point);
    Vec2::new(point.x + direction.x * distance, point.y + direction.y * distance)
}

fn normalized(source: Vec2, point: Vec2) -> Vec2 {
    let dx = point.x - source.x;
    let dy = point.y - source.y;
    let length = hypot(dx, dy);
    if length == 0.0 { Vec2::ZERO } else { Vec2::new(dx / length, dy / length) }
}

fn add(left: Vec2, right: Vec2) -> Vec2 {
    Vec2::new(left.x + right.x, left.y + right.y)
}

fn presentation_random(
    seed: u32,
    source: NativeBoneyardLightSource,
    index: usize,
    frame: u64,
) -> f64 {
    let mut value = mix_seed(seed ^ frame as u32);
    value = mix_seed(value ^ index as u32);
    value = mix_seed(value ^ (source.position.x as f32).to_bits());
    value = mix_seed(value ^ (source.position.y as f32).to_bits());
    value = mix_seed(value ^ (source.radius as f32).to_bits());
    f64::from(value ^ (value >> 15)) / 4_294_967_296.0
}

fn mix_seed(seed: u32) -> u32 {
    let value = (seed ^ (seed >> 16)).wrapping_mul(0x21f0_aaad);
    let value = (value ^ (value >> 15)).wrapping_mul(0x735a_2d97);
    value ^ (value >> 15)
}
