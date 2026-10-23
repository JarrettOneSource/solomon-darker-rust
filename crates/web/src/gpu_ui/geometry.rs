//! GPU geometry shared by clipped portraits, authored minimaps and stock pieces.

use egui::{Pos2, Rect, vec2};

/// The authored overlay's CSS timing curve, also used by stock hover transitions.
pub fn css_bezier(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let curve = |v: f32, p1: f32, p2: f32| {
        3.0 * (1.0 - v) * (1.0 - v) * v * p1 + 3.0 * (1.0 - v) * v * v * p2 + v * v * v
    };
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..16 {
        let middle = (low + high) * 0.5;
        if curve(middle, x1, x2) < t { low = middle } else { high = middle }
    }
    curve((low + high) * 0.5, y1, y2)
}

/// Clips a convex polygon against an ellipse's 64-plane approximation.
pub fn clip_polygon_ellipse(mut points: Vec<Pos2>, bounds: Rect) -> Vec<Pos2> {
    if !bounds.is_positive() {
        return Vec::new();
    }
    let center = bounds.center();
    let radius = bounds.size() / 2.0;
    for index in 0..64 {
        if points.is_empty() {
            break;
        }
        let angle = (index as f32 + 0.5) * std::f32::consts::TAU / 64.0;
        let normal = vec2(angle.cos() / radius.x, angle.sin() / radius.y);
        let limit = (std::f32::consts::PI / 64.0).cos();
        let input = std::mem::take(&mut points);
        let mut previous = *input.last().expect("nonempty clipped polygon");
        let mut previous_distance = (previous - center).dot(normal) - limit;
        for point in input {
            let distance = (point - center).dot(normal) - limit;
            if (distance <= 0.0) != (previous_distance <= 0.0) {
                points.push(
                    previous
                        + (point - previous) * (previous_distance / (previous_distance - distance)),
                );
            }
            if distance <= 0.0 {
                points.push(point);
            }
            previous = point;
            previous_distance = distance;
        }
    }
    points
}

/// Crops geometry and UVs together before inherited HUD rotation is applied.
pub(super) fn clip_image_rect(source: Rect, destination: Rect, clip: Rect) -> Option<(Rect, Rect)> {
    let cropped = destination.intersect(clip);
    if !cropped.is_positive() {
        return None;
    }
    let offset_min = (cropped.min - destination.min) / destination.size();
    let offset_max = (cropped.max - destination.min) / destination.size();
    Some((
        Rect::from_min_max(
            source.min + source.size() * offset_min,
            source.min + source.size() * offset_max,
        ),
        cropped,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    #[test]
    fn clipped_portrait_vertices_stay_inside_the_same_ellipse() {
        let bounds = Rect::from_min_size(pos2(10.0, 20.0), vec2(100.0, 60.0));
        let points = clip_polygon_ellipse(
            vec![
                bounds.left_top(),
                bounds.right_top(),
                bounds.right_bottom(),
                bounds.left_bottom(),
            ],
            bounds,
        );
        assert!(points.len() >= 60);
        for point in points {
            let normalized = (point - bounds.center()) / (bounds.size() / 2.0);
            assert!(normalized.length_sq() <= 1.00001);
        }
    }

    #[test]
    fn cropped_mirrored_uvs_preserve_the_original_texel_scale() {
        let source = Rect::from_min_max(pos2(1.0, 0.0), pos2(0.0, 1.0));
        let destination = Rect::from_min_size(pos2(20.0, 30.0), vec2(100.0, 50.0));
        let clip = Rect::from_min_size(pos2(45.0, 30.0), vec2(50.0, 25.0));
        let (uv, cropped) = clip_image_rect(source, destination, clip).unwrap();
        assert_eq!(uv, Rect::from_min_max(pos2(0.75, 0.0), pos2(0.25, 0.5)));
        assert_eq!(cropped, clip);
    }
}
