//! Illuminated Silk and fragment triangles from `native-silk-presentation.ts`
//! and `renderer/native-spider-web-views.ts`.

use super::silk::NativeSilkState;
use super::silk_force::NativeFadeLineState;
use crate::float::fround;
use crate::line::{native_line_color, native_line_vertices};
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnemyWebMesh {
    pub vertices: Vec<Vec2>,
    pub colors: Vec<u32>,
    pub indices: Vec<u32>,
}

impl EnemyWebMesh {
    fn append(&mut self, quad: [f64; 8], colors: [u32; 4]) {
        let offset = self.vertices.len() as u32;
        for point in quad.as_chunks::<2>().0 {
            self.vertices.push(Vec2::new(point[0], point[1]));
        }
        self.colors.extend(colors);
        self.indices.extend([offset, offset + 1, offset + 2, offset + 1, offset + 2, offset + 3]);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeSilkMesh {
    pub mesh: EnemyWebMesh,
    pub segment_count: usize,
    pub sparkle_count: usize,
}

#[must_use]
pub fn native_silk_curve_point(source: &NativeSilkState, phase: f64) -> Vec2 {
    let center = source.center.evaluate(phase);
    let wave = source.wave.evaluate(phase);
    Vec2::new(
        fround(center.x + fround(wave.x * source.wave_scale) + source.drift.x),
        fround(
            center.y + fround(wave.y * source.wave_scale) + source.drift.y - source.height - 1.0,
        ),
    )
}

/// Silk::Render retains sixteen spline units behind the current phase (0x00606A10).
#[must_use]
pub fn native_silk_mesh(
    source: &NativeSilkState,
    light_at: &dyn Fn(Vec2) -> f64,
    rng: &mut NativeRng,
) -> NativeSilkMesh {
    let mut result = NativeSilkMesh::default();
    if source.phase <= 0.0 {
        return result;
    }
    let lower = (source.phase - 16.0).max(0.0);
    let span = source.phase - lower;
    let step = fround(4.0 / source.wave_scale) * fround(0.1);
    let head_light = light_at(source.center.evaluate(source.phase));
    let tail_light = light_at(source.center.evaluate(lower));
    let mut cursor = source.phase;
    let mut fraction = 1.0;
    let mut first_alpha = 0.0;
    let mut second_alpha = fround(source.alpha * 0.5);
    loop {
        let next = fround(cursor - step);
        let head_fraction = fround((cursor - lower) / span);
        let tail_fraction = fround((next - lower).max(0.0) / span);
        let first_light = fround(tail_light * (1.0 - head_fraction) + head_light * head_fraction);
        let second_light = fround(tail_light * (1.0 - tail_fraction) + head_light * tail_fraction);
        let first = native_silk_curve_point(source, cursor);
        let second = native_silk_curve_point(source, next);
        if first != second {
            let first_color = native_line_color(
                fround(0.95) * first_light,
                first_light,
                first_light,
                first_alpha,
            );
            let second_color = native_line_color(
                fround(0.95) * second_light,
                second_light,
                second_light,
                second_alpha,
            );
            result.mesh.append(
                native_line_vertices(first, second, 2.0),
                [first_color, first_color, second_color, second_color],
            );
        }
        let sparkle = rng.draw_integer(3);
        if sparkle == 1 && result.segment_count > 0 {
            let alpha = rng.draw_scaled_float(fround(source.alpha * fraction), false);
            let position = rng.draw_scaled_float(1.0, false);
            let x = first.x + (second.x - first.x) * position - 0.5;
            let y = first.y + (second.y - first.y) * position - 0.5;
            let color = native_line_color(1.0, 1.0, 1.0, alpha * first_light);
            result.mesh.append([x, y, x + 1.0, y, x, y + 1.0, x + 1.0, y + 1.0], [color; 4]);
            result.sparkle_count += 1;
        }
        result.segment_count += 1;
        first_alpha = fround(fraction * fround(source.alpha * 0.5));
        fraction = fround((next - lower) / span);
        second_alpha = fround(fraction * fround(source.alpha * 0.5));
        cursor = next;
        if fraction <= 0.0 || cursor < lower {
            return result;
        }
    }
}

#[must_use]
pub fn native_fade_line_mesh(source: &NativeFadeLineState) -> EnemyWebMesh {
    let mut mesh = EnemyWebMesh::default();
    let transparent = native_line_color(1.0, 1.0, 1.0, 0.0);
    if source.start != source.middle {
        let color = native_line_color(1.0, 1.0, 1.0, source.opacity.min(1.0));
        mesh.append(
            native_line_vertices(source.start, source.middle, 2.0),
            [transparent, transparent, color, color],
        );
    }
    if source.middle != source.end {
        let color = native_line_color(1.0, 1.0, 1.0, source.opacity);
        mesh.append(
            native_line_vertices(source.middle, source.end, 2.0),
            [color, color, transparent, transparent],
        );
    }
    mesh
}
