//! The convex paths used by the source's world labels and activity badge.
//! Rounded paths follow installed Pixi `buildCommands/buildCircle.mjs` exactly.

use solomon_sim::Vec2;

use solomon_graphics::{BlendMode, Renderer, Vertex};

pub(super) fn rgba(tint: u32, alpha: f64) -> [f32; 4] {
    [
        ((tint >> 16) & 255) as f32 / 255.0,
        ((tint >> 8) & 255) as f32 / 255.0,
        (tint & 255) as f32 / 255.0,
        (alpha.clamp(0.0, 1.0) * 255.0).trunc() as f32 / 255.0,
    ]
}

pub(super) fn polygon(
    renderer: &mut Renderer,
    origin: Vec2,
    points: &[Vec2],
    color: u32,
    alpha: f64,
) {
    let tint = rgba(color, alpha);
    let vertex = |point: Vec2| Vertex {
        position: [(origin.x + point.x) as f32, (origin.y + point.y) as f32],
        texel: [0.0, 0.0],
        tint,
    };
    let mut triangles = Vec::new();
    for index in 1..points.len().saturating_sub(1) {
        triangles.push([vertex(points[0]), vertex(points[index]), vertex(points[index + 1])]);
    }
    renderer.draw_solid_triangles(&triangles, BlendMode::Normal);
}

pub(super) fn rectangle(
    renderer: &mut Renderer,
    origin: Vec2,
    bounds: [f64; 4],
    color: u32,
    alpha: f64,
) {
    let [x, y, width, height] = bounds;
    renderer.draw_solid(
        [(origin.x + x) as f32, (origin.y + y) as f32, width as f32, height as f32],
        rgba(color, alpha),
    );
}

pub(super) fn chamfered_plate(
    renderer: &mut Renderer,
    origin: Vec2,
    bounds: [f64; 3],
    chamfer: f64,
    color: u32,
    alpha: f64,
) {
    let [half_width, top, height] = bounds;
    let bottom = top + height;
    polygon(
        renderer,
        origin,
        &[
            Vec2::new(-half_width + chamfer, top),
            Vec2::new(half_width - chamfer, top),
            Vec2::new(half_width, top + chamfer),
            Vec2::new(half_width, bottom - chamfer),
            Vec2::new(half_width - chamfer, bottom),
            Vec2::new(-half_width + chamfer, bottom),
            Vec2::new(-half_width, bottom - chamfer),
            Vec2::new(-half_width, top + chamfer),
        ],
        color,
        alpha,
    );
}

pub(super) fn diamond(
    renderer: &mut Renderer,
    origin: Vec2,
    center: Vec2,
    reach: f64,
    color: u32,
    alpha: f64,
) {
    polygon(
        renderer,
        origin,
        &[
            Vec2::new(center.x - reach, center.y),
            Vec2::new(center.x, center.y - reach),
            Vec2::new(center.x + reach, center.y),
            Vec2::new(center.x, center.y + reach),
        ],
        color,
        alpha,
    );
}

pub(super) fn round_rectangle(
    renderer: &mut Renderer,
    origin: Vec2,
    bounds: [f64; 4],
    radius: f64,
    color: u32,
    alpha: f64,
) {
    polygon(renderer, origin, &rounded_points(bounds, radius), color, alpha);
}

pub(super) fn circle(
    renderer: &mut Renderer,
    origin: Vec2,
    center: Vec2,
    radius: f64,
    color: u32,
    alpha: f64,
) {
    round_rectangle(
        renderer,
        origin,
        [center.x - radius, center.y - radius, radius * 2.0, radius * 2.0],
        radius,
        color,
        alpha,
    );
}

/// The badge's closed circle takes the default centered, miter-joined stroke.
pub(super) fn circle_stroke(
    renderer: &mut Renderer,
    origin: Vec2,
    radius: f64,
    width: f64,
    color: u32,
    alpha: f64,
) {
    let points = rounded_points([-radius, -radius, radius * 2.0, radius * 2.0], radius);
    let mut inner = Vec::with_capacity(points.len());
    let mut outer = Vec::with_capacity(points.len());
    for (index, point) in points.iter().enumerate() {
        let previous = points[(index + points.len() - 1) % points.len()];
        let next = points[(index + 1) % points.len()];
        let first = Vec2::new(point.x - previous.x, point.y - previous.y);
        let second = Vec2::new(next.x - point.x, next.y - point.y);
        let first_length = solomon_sim::float::hypot(first.x, first.y);
        let second_length = solomon_sim::float::hypot(second.x, second.y);
        let normal_a = Vec2::new(first.y / first_length, -first.x / first_length);
        let normal_b = Vec2::new(second.y / second_length, -second.x / second_length);
        let factor = width / 2.0 / (1.0 + normal_a.x * normal_b.x + normal_a.y * normal_b.y);
        let offset =
            Vec2::new((normal_a.x + normal_b.x) * factor, (normal_a.y + normal_b.y) * factor);
        inner.push(Vec2::new(point.x - offset.x, point.y - offset.y));
        outer.push(Vec2::new(point.x + offset.x, point.y + offset.y));
    }
    let tint = rgba(color, alpha);
    let vertex = |point: Vec2| Vertex {
        position: [(origin.x + point.x) as f32, (origin.y + point.y) as f32],
        texel: [0.0, 0.0],
        tint,
    };
    let mut triangles = Vec::with_capacity(points.len() * 2);
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        triangles.push([vertex(inner[index]), vertex(outer[index]), vertex(outer[next])]);
        triangles.push([vertex(inner[index]), vertex(outer[next]), vertex(inner[next])]);
    }
    renderer.draw_solid_triangles(&triangles, BlendMode::Normal);
}

fn rounded_points(bounds: [f64; 4], radius: f64) -> Vec<Vec2> {
    let [left, top, width, height] = bounds;
    let half_width = width / 2.0;
    let half_height = height / 2.0;
    let x = left + half_width;
    let y = top + half_height;
    let radius = radius.min(half_width.min(half_height)).max(0.0);
    let dx = half_width - radius;
    let dy = half_height - radius;
    let subdivisions = (2.3 * (radius + radius).sqrt()).ceil() as usize;
    if subdivisions == 0 {
        return vec![
            Vec2::new(x + dx, y + dy),
            Vec2::new(x - dx, y + dy),
            Vec2::new(x - dx, y - dy),
            Vec2::new(x + dx, y - dy),
        ];
    }
    let count = subdivisions * 8 + usize::from(dx != 0.0) * 4 + usize::from(dy != 0.0) * 4;
    let mut coordinates = vec![0.0; count];
    let mut lower_right = 0;
    let mut lower_left = subdivisions * 4 + usize::from(dx != 0.0) * 2 + 2;
    let mut upper_left = lower_left;
    let mut upper_right = count;
    push_point(&mut coordinates, &mut lower_right, x + dx + radius, y + dy);
    prepend_point(&mut coordinates, &mut lower_left, x - dx - radius, y + dy);
    if dy != 0.0 {
        push_point(&mut coordinates, &mut upper_left, x - dx - radius, y - dy);
        prepend_point(&mut coordinates, &mut upper_right, x + dx + radius, y - dy);
    }
    for index in 1..subdivisions {
        let angle = std::f64::consts::PI / 2.0 * (index as f64 / subdivisions as f64);
        let offset_x = dx + angle.cos() * radius;
        let offset_y = dy + angle.sin() * radius;
        push_point(&mut coordinates, &mut lower_right, x + offset_x, y + offset_y);
        prepend_point(&mut coordinates, &mut lower_left, x - offset_x, y + offset_y);
        push_point(&mut coordinates, &mut upper_left, x - offset_x, y - offset_y);
        prepend_point(&mut coordinates, &mut upper_right, x + offset_x, y - offset_y);
    }
    push_point(&mut coordinates, &mut lower_right, x + dx, y + dy + radius);
    prepend_point(&mut coordinates, &mut upper_right, x + dx, y - dy - radius);
    if dx != 0.0 {
        push_point(&mut coordinates, &mut lower_right, x - dx, y + dy + radius);
        prepend_point(&mut coordinates, &mut upper_right, x - dx, y - dy - radius);
    }
    coordinates.as_chunks::<2>().0.iter().map(|point| Vec2::new(point[0], point[1])).collect()
}

fn push_point(coordinates: &mut [f64], index: &mut usize, x: f64, y: f64) {
    coordinates[*index] = x;
    coordinates[*index + 1] = y;
    *index += 2;
}

fn prepend_point(coordinates: &mut [f64], index: &mut usize, x: f64, y: f64) {
    *index -= 2;
    coordinates[*index] = x;
    coordinates[*index + 1] = y;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_label_curves_use_the_source_graphics_vertices() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../sim/tests/fixtures/website/world-labels-goldens.json"
        ))
        .unwrap();
        for case in fixture["shapes"].as_array().unwrap() {
            let shape = &case["shape"];
            let number = |key| shape[key].as_f64().unwrap();
            let radius = number("radius");
            let bounds = if shape["type"] == "circle" {
                [number("x") - radius, number("y") - radius, radius * 2.0, radius * 2.0]
            } else {
                [number("x"), number("y"), number("width"), number("height")]
            };
            let actual = rounded_points(bounds, radius);
            let expected = case["points"].as_array().unwrap();
            assert_eq!(actual.len() * 2, expected.len());
            for (actual, expected) in actual.iter().zip(expected.as_chunks::<2>().0) {
                assert!((actual.x - expected[0].as_f64().unwrap()).abs() < 1.0e-12);
                assert!((actual.y - expected[1].as_f64().unwrap()).abs() < 1.0e-12);
            }
        }
    }
}
