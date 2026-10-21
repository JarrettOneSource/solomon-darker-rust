//! Mobile editor geometry from Website `mobile-ui-layout.ts`.
//! All snaps use the source's authored grid/edge priorities and fixed-opposite resize
//! anchor. These values are presentation transforms, never gameplay coordinates.

use crate::Vec2;
use crate::belt::{NativeBeltEntry, PlayerBeltComponent};
use crate::float::hypot;

use super::{
    MOBILE_UI_CANONICAL_HEIGHT, MOBILE_UI_CANONICAL_WIDTH, MOBILE_UI_SCALE_MAX,
    MOBILE_UI_SCALE_MIN, MobileUiElementId, MobileUiElementTransform, MobileUiSize, finite_or,
    normalize_mobile_ui_transform, source_clamp,
};

pub const MOBILE_UI_GRID_SIZE: f64 = 16.0;
pub const MOBILE_UI_SNAP_THRESHOLD: f64 = 6.0;
pub const MOBILE_UI_PAGE_ZOOM_MIN: f64 = 0.35;
pub const MOBILE_UI_PAGE_ZOOM_MAX: f64 = 4.0;
const SNAP_EPSILON: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobileUiResizeHandle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

impl MobileUiResizeHandle {
    pub const ALL: [Self; 8] = [
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
        Self::East,
        Self::SouthEast,
        Self::South,
        Self::SouthWest,
        Self::West,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NorthWest => "north-west",
            Self::North => "north",
            Self::NorthEast => "north-east",
            Self::East => "east",
            Self::SouthEast => "south-east",
            Self::South => "south",
            Self::SouthWest => "south-west",
            Self::West => "west",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|handle| handle.as_str() == id)
    }

    const fn direction(self) -> Vec2 {
        match self {
            Self::NorthWest => Vec2::new(-1.0, -1.0),
            Self::North => Vec2::new(0.0, -1.0),
            Self::NorthEast => Vec2::new(1.0, -1.0),
            Self::East => Vec2::new(1.0, 0.0),
            Self::SouthEast => Vec2::new(1.0, 1.0),
            Self::South => Vec2::new(0.0, 1.0),
            Self::SouthWest => Vec2::new(-1.0, 1.0),
            Self::West => Vec2::new(-1.0, 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MobileUiSnapAxis {
    X,
    Y,
}

impl MobileUiSnapAxis {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
        }
    }

    const fn other(self) -> Self {
        match self {
            Self::X => Self::Y,
            Self::Y => Self::X,
        }
    }

    fn point(self, point: Vec2) -> f64 {
        match self {
            Self::X => point.x,
            Self::Y => point.y,
        }
    }
}

/// Declaration order is the source priority: sibling edges, page edges, then grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MobileUiSnapKind {
    Element,
    Page,
    Grid,
}

impl MobileUiSnapKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Element => "element",
            Self::Page => "page",
            Self::Grid => "grid",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobileUiSnapRect {
    pub bottom: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub left: f64,
    pub right: f64,
    pub top: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobileUiSnapGuide {
    pub axis: MobileUiSnapAxis,
    pub kind: MobileUiSnapKind,
    pub position: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobileUiSnapResult {
    pub guides: Vec<MobileUiSnapGuide>,
    pub transform: MobileUiElementTransform,
}

/// Only the first alias of a potion kind uses its independent mobile potion position.
#[must_use]
pub fn mobile_ui_belt_element_id(belt: &PlayerBeltComponent, slot: usize) -> MobileUiElementId {
    assert!(slot < 8, "the mobile belt has eight slots");
    let entry = belt.entries[slot];
    if matches!(entry, Some(NativeBeltEntry::HealthPotion | NativeBeltEntry::ManaPotion))
        && belt.entries.iter().position(|candidate| *candidate == entry) == Some(slot)
    {
        return if entry == Some(NativeBeltEntry::HealthPotion) {
            MobileUiElementId::HealthPotion
        } else {
            MobileUiElementId::ManaPotion
        };
    }
    MobileUiElementId::SLOTS[slot]
}

#[must_use]
pub fn mobile_ui_editor_page_size(width: f64, height: f64, coarse_pointer: bool) -> MobileUiSize {
    if coarse_pointer && width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        return MobileUiSize { width, height };
    }
    MobileUiSize { width: MOBILE_UI_CANONICAL_WIDTH, height: MOBILE_UI_CANONICAL_HEIGHT }
}

#[must_use]
pub fn mobile_ui_element_snap_rect(
    transform: MobileUiElementTransform,
    natural_size: MobileUiSize,
    page: MobileUiSize,
) -> MobileUiSnapRect {
    assert_sizes(natural_size, page);
    let normalized = normalize_mobile_ui_transform(transform);
    let center_x = normalized.x / 100.0 * page.width;
    let center_y = normalized.y / 100.0 * page.height;
    let extent = rotated_half_extent(normalized, natural_size);
    MobileUiSnapRect {
        bottom: center_y + extent.y,
        center_x,
        center_y,
        left: center_x - extent.x,
        right: center_x + extent.x,
        top: center_y - extent.y,
    }
}

#[must_use]
pub fn constrain_mobile_ui_transform(
    transform: MobileUiElementTransform,
    natural_size: MobileUiSize,
    page: MobileUiSize,
) -> MobileUiElementTransform {
    assert_sizes(natural_size, page);
    let normalized = normalize_mobile_ui_transform(transform);
    let radians = normalized.rotation * std::f64::consts::PI / 180.0;
    let cosine = radians.cos().abs();
    let sine = radians.sin().abs();
    let width = (natural_size.width * cosine + natural_size.height * sine) * normalized.scale;
    let height = (natural_size.width * sine + natural_size.height * cosine) * normalized.scale;
    MobileUiElementTransform {
        x: bounded_center(normalized.x, width / page.width * 50.0),
        y: bounded_center(normalized.y, height / page.height * 50.0),
        ..normalized
    }
}

#[must_use]
pub fn snap_mobile_ui_move(
    transform: MobileUiElementTransform,
    natural_size: MobileUiSize,
    page: MobileUiSize,
    siblings: &[MobileUiSnapRect],
    threshold: f64,
) -> MobileUiSnapResult {
    assert_threshold(threshold);
    let bounded = constrain_mobile_ui_transform(transform, natural_size, page);
    let frame = mobile_ui_element_snap_rect(bounded, natural_size, page);
    let horizontal = best_axis_snap(
        &[frame.left, frame.center_x, frame.right],
        frame.center_x,
        frame.center_x - frame.left,
        page.width - (frame.right - frame.center_x),
        &snap_targets(MobileUiSnapAxis::X, page, siblings),
        threshold,
    );
    let vertical = best_axis_snap(
        &[frame.top, frame.center_y, frame.bottom],
        frame.center_y,
        frame.center_y - frame.top,
        page.height - (frame.bottom - frame.center_y),
        &snap_targets(MobileUiSnapAxis::Y, page, siblings),
        threshold,
    );
    let snapped = constrain_mobile_ui_transform(
        MobileUiElementTransform {
            x: (frame.center_x + horizontal.map_or(0.0, |snap| snap.delta)) / page.width * 100.0,
            y: (frame.center_y + vertical.map_or(0.0, |snap| snap.delta)) / page.height * 100.0,
            ..bounded
        },
        natural_size,
        page,
    );
    let frame = mobile_ui_element_snap_rect(snapped, natural_size, page);
    let mut guides = Vec::new();
    for snap in [horizontal, vertical].into_iter().flatten() {
        if frame_guide_aligned(frame, snap.guide) {
            guides.push(snap.guide);
        }
    }
    snap_result(snapped, guides)
}

#[must_use]
pub fn mobile_ui_resize_transform(
    initial: MobileUiElementTransform,
    natural_size: MobileUiSize,
    page: MobileUiSize,
    handle: MobileUiResizeHandle,
    initial_pointer: Vec2,
    current_pointer: Vec2,
) -> MobileUiElementTransform {
    let resize =
        resize_projection(initial, natural_size, page, handle, initial_pointer, current_pointer);
    constrain_mobile_ui_transform(resize_at_scale(resize, resize.scale), natural_size, page)
}

/// The complete source gesture record uses the same snap targets/threshold as movement.
/// Keeping the gesture together avoids reordering
/// its opposite-anchor arithmetic at individual UI callers.
#[must_use]
pub fn snap_mobile_ui_resize(
    gesture: MobileUiResizeGesture,
    siblings: &[MobileUiSnapRect],
    threshold: f64,
) -> MobileUiSnapResult {
    assert_threshold(threshold);
    let resize = resize_projection(
        gesture.initial,
        gesture.natural_size,
        gesture.page,
        gesture.handle,
        gesture.initial_pointer,
        gesture.current_pointer,
    );
    let span_length = hypot(resize.span.x, resize.span.y);
    let mut best = None;
    for axis in [MobileUiSnapAxis::X, MobileUiSnapAxis::Y] {
        let span = axis.point(resize.span);
        if span.abs() <= SNAP_EPSILON {
            continue;
        }
        for target in snap_targets(axis, gesture.page, siblings) {
            let scale = (target.position - axis.point(resize.fixed)) / span;
            if scale < MOBILE_UI_SCALE_MIN || scale > resize.maximum_scale {
                continue;
            }
            let travel = (scale - resize.scale).abs() * span_length;
            if travel > threshold + SNAP_EPSILON {
                continue;
            }
            let candidate = constrain_mobile_ui_transform(
                resize_at_scale(resize, scale),
                gesture.natural_size,
                gesture.page,
            );
            let point = resize_handle_point(
                candidate,
                gesture.natural_size,
                gesture.page,
                resize.direction,
            );
            if (axis.point(point) - target.position).abs() > SNAP_EPSILON {
                continue;
            }
            let next = ScaleSnap { guide: target, scale, travel };
            if better_scale_snap(next, best) {
                best = Some(next);
            }
        }
    }
    let Some(best) = best else {
        return snap_result(
            constrain_mobile_ui_transform(
                resize_at_scale(resize, resize.scale),
                gesture.natural_size,
                gesture.page,
            ),
            Vec::new(),
        );
    };
    let transform = constrain_mobile_ui_transform(
        resize_at_scale(resize, best.scale),
        gesture.natural_size,
        gesture.page,
    );
    let point =
        resize_handle_point(transform, gesture.natural_size, gesture.page, resize.direction);
    let mut guides = vec![best.guide];
    let other_axis = best.guide.axis.other();
    if let Some(other) = preferred_target_at(
        other_axis.point(point),
        &snap_targets(other_axis, gesture.page, siblings),
    ) {
        guides.push(other);
    }
    snap_result(transform, guides)
}

#[derive(Clone, Copy, Debug)]
pub struct MobileUiResizeGesture {
    pub initial: MobileUiElementTransform,
    pub natural_size: MobileUiSize,
    pub page: MobileUiSize,
    pub handle: MobileUiResizeHandle,
    pub initial_pointer: Vec2,
    pub current_pointer: Vec2,
}

#[must_use]
pub fn snap_mobile_ui_scale(
    transform: MobileUiElementTransform,
    natural_size: MobileUiSize,
    page: MobileUiSize,
    siblings: &[MobileUiSnapRect],
    threshold: f64,
) -> MobileUiSnapResult {
    assert_threshold(threshold);
    let bounded = constrain_mobile_ui_transform(transform, natural_size, page);
    let frame = mobile_ui_element_snap_rect(bounded, natural_size, page);
    let unit =
        rotated_half_extent(MobileUiElementTransform { scale: 1.0, ..bounded }, natural_size);
    let mut best = None;
    for axis in [MobileUiSnapAxis::X, MobileUiSnapAxis::Y] {
        let (center, extent, anchors) = match axis {
            MobileUiSnapAxis::X => (frame.center_x, unit.x, [frame.left, frame.right]),
            MobileUiSnapAxis::Y => (frame.center_y, unit.y, [frame.top, frame.bottom]),
        };
        for (anchor, sign) in [(anchors[0], -1.0), (anchors[1], 1.0)] {
            for target in snap_targets(axis, page, siblings) {
                let travel = (target.position - anchor).abs();
                if travel > threshold + SNAP_EPSILON {
                    continue;
                }
                let scale = (target.position - center) / (sign * extent);
                if !(MOBILE_UI_SCALE_MIN..=MOBILE_UI_SCALE_MAX).contains(&scale) {
                    continue;
                }
                let candidate = constrain_mobile_ui_transform(
                    MobileUiElementTransform { scale, ..bounded },
                    natural_size,
                    page,
                );
                let candidate_frame = mobile_ui_element_snap_rect(candidate, natural_size, page);
                let candidate_anchor = match axis {
                    MobileUiSnapAxis::X => {
                        if sign < 0.0 {
                            candidate_frame.left
                        } else {
                            candidate_frame.right
                        }
                    }
                    MobileUiSnapAxis::Y => {
                        if sign < 0.0 {
                            candidate_frame.top
                        } else {
                            candidate_frame.bottom
                        }
                    }
                };
                if (candidate_anchor - target.position).abs() > SNAP_EPSILON {
                    continue;
                }
                let next = ScaleSnap { guide: target, scale, travel };
                if better_scale_snap(next, best) {
                    best = Some(next);
                }
            }
        }
    }
    let Some(best) = best else {
        return snap_result(bounded, Vec::new());
    };
    let transform = constrain_mobile_ui_transform(
        MobileUiElementTransform { scale: best.scale, ..bounded },
        natural_size,
        page,
    );
    let frame = mobile_ui_element_snap_rect(transform, natural_size, page);
    let mut guides = vec![best.guide];
    let other_axis = best.guide.axis.other();
    let anchors = match other_axis {
        MobileUiSnapAxis::X => [frame.left, frame.right],
        MobileUiSnapAxis::Y => [frame.top, frame.bottom],
    };
    let targets = snap_targets(other_axis, page, siblings);
    let mut other: Option<MobileUiSnapGuide> = None;
    for anchor in anchors {
        if let Some(aligned) = preferred_target_at(anchor, &targets)
            && other.is_none_or(|other| aligned.kind < other.kind)
        {
            other = Some(aligned);
        }
    }
    if let Some(other) = other {
        guides.push(other);
    }
    snap_result(transform, guides)
}

#[must_use]
pub fn mobile_ui_element_pinch_scale(
    initial_scale: f64,
    initial_distance: f64,
    current_distance: f64,
) -> f64 {
    if !initial_distance.is_finite()
        || initial_distance <= 0.0
        || !current_distance.is_finite()
        || current_distance < 0.0
    {
        return normalize_scale(initial_scale);
    }
    normalize_scale(initial_scale * current_distance / initial_distance)
}

#[must_use]
pub fn mobile_ui_element_rotation(
    initial_rotation: f64,
    initial_angle: f64,
    current_angle: f64,
    snap: bool,
) -> f64 {
    let rotation = normalize_mobile_ui_transform(MobileUiElementTransform {
        rotation: initial_rotation + (current_angle - initial_angle) * 180.0 / std::f64::consts::PI,
        scale: 1.0,
        x: 50.0,
        y: 50.0,
    })
    .rotation;
    if !snap {
        return rotation;
    }
    let rotation = (rotation / 5.0 + 0.5).floor() * 5.0;
    normalize_mobile_ui_transform(MobileUiElementTransform {
        rotation,
        scale: 1.0,
        x: 50.0,
        y: 50.0,
    })
    .rotation
}

#[must_use]
pub fn mobile_ui_page_pinch_zoom(
    initial_zoom: f64,
    initial_distance: f64,
    current_distance: f64,
) -> f64 {
    let zoom = if !initial_distance.is_finite()
        || initial_distance <= 0.0
        || !current_distance.is_finite()
        || current_distance < 0.0
    {
        initial_zoom
    } else {
        initial_zoom * current_distance / initial_distance
    };
    source_clamp(finite_or(zoom, 1.0), MOBILE_UI_PAGE_ZOOM_MIN, MOBILE_UI_PAGE_ZOOM_MAX)
}

#[derive(Clone, Copy)]
struct AxisSnap {
    delta: f64,
    guide: MobileUiSnapGuide,
}
#[derive(Clone, Copy)]
struct ScaleSnap {
    guide: MobileUiSnapGuide,
    scale: f64,
    travel: f64,
}
#[derive(Clone, Copy)]
struct ResizeProjection {
    direction: Vec2,
    fixed: Vec2,
    initial: MobileUiElementTransform,
    maximum_scale: f64,
    page: MobileUiSize,
    scale: f64,
    span: Vec2,
}

fn rotated_half_extent(transform: MobileUiElementTransform, size: MobileUiSize) -> Vec2 {
    let radians = transform.rotation * std::f64::consts::PI / 180.0;
    let cosine = radians.cos().abs();
    let sine = radians.sin().abs();
    Vec2::new(
        (size.width * cosine + size.height * sine) * transform.scale / 2.0,
        (size.width * sine + size.height * cosine) * transform.scale / 2.0,
    )
}

fn snap_targets(
    axis: MobileUiSnapAxis,
    page: MobileUiSize,
    siblings: &[MobileUiSnapRect],
) -> Vec<MobileUiSnapGuide> {
    let dimension = match axis {
        MobileUiSnapAxis::X => page.width,
        MobileUiSnapAxis::Y => page.height,
    };
    let mut targets = Vec::new();
    let mut position = 0.0;
    while position <= dimension + SNAP_EPSILON {
        targets.push(MobileUiSnapGuide { axis, kind: MobileUiSnapKind::Grid, position });
        position += MOBILE_UI_GRID_SIZE;
    }
    for position in [0.0, dimension / 2.0, dimension] {
        targets.push(MobileUiSnapGuide { axis, kind: MobileUiSnapKind::Page, position });
    }
    for rect in siblings {
        let positions = match axis {
            MobileUiSnapAxis::X => [rect.left, rect.center_x, rect.right],
            MobileUiSnapAxis::Y => [rect.top, rect.center_y, rect.bottom],
        };
        for position in positions {
            if position.is_finite() {
                targets.push(MobileUiSnapGuide { axis, kind: MobileUiSnapKind::Element, position });
            }
        }
    }
    targets
}

fn best_axis_snap(
    anchors: &[f64],
    center: f64,
    minimum_center: f64,
    maximum_center: f64,
    targets: &[MobileUiSnapGuide],
    threshold: f64,
) -> Option<AxisSnap> {
    let mut best: Option<AxisSnap> = None;
    for &target in targets {
        for &anchor in anchors {
            let delta = target.position - anchor;
            if delta.abs() > threshold + SNAP_EPSILON {
                continue;
            }
            let next_center = center + delta;
            if next_center < minimum_center - SNAP_EPSILON
                || next_center > maximum_center + SNAP_EPSILON
            {
                continue;
            }
            let next = AxisSnap { delta, guide: target };
            let better = best.is_none_or(|current| {
                if (delta.abs() - current.delta.abs()).abs() > SNAP_EPSILON {
                    return delta.abs() < current.delta.abs();
                }
                if target.kind != current.guide.kind {
                    return target.kind < current.guide.kind;
                }
                target.position < current.guide.position
            });
            if better {
                best = Some(next);
            }
        }
    }
    best
}

fn better_scale_snap(candidate: ScaleSnap, current: Option<ScaleSnap>) -> bool {
    let Some(current) = current else { return true };
    if (candidate.travel - current.travel).abs() > SNAP_EPSILON {
        return candidate.travel < current.travel;
    }
    if candidate.guide.kind != current.guide.kind {
        return candidate.guide.kind < current.guide.kind;
    }
    if candidate.guide.axis != current.guide.axis {
        return candidate.guide.axis == MobileUiSnapAxis::X;
    }
    candidate.guide.position < current.guide.position
}

fn resize_projection(
    initial: MobileUiElementTransform,
    natural_size: MobileUiSize,
    page: MobileUiSize,
    handle: MobileUiResizeHandle,
    initial_pointer: Vec2,
    current_pointer: Vec2,
) -> ResizeProjection {
    assert!(
        initial_pointer.x.is_finite() && initial_pointer.y.is_finite(),
        "initial resize pointer is finite"
    );
    assert!(
        current_pointer.x.is_finite() && current_pointer.y.is_finite(),
        "current resize pointer is finite"
    );
    let initial = constrain_mobile_ui_transform(initial, natural_size, page);
    let direction = handle.direction();
    let radians = initial.rotation * std::f64::consts::PI / 180.0;
    let active_unit = rotate_point(
        Vec2::new(direction.x * natural_size.width / 2.0, direction.y * natural_size.height / 2.0),
        radians,
    );
    let span = Vec2::new(active_unit.x * 2.0, active_unit.y * 2.0);
    let center = Vec2::new(initial.x / 100.0 * page.width, initial.y / 100.0 * page.height);
    let fixed = Vec2::new(
        center.x - active_unit.x * initial.scale,
        center.y - active_unit.y * initial.scale,
    );
    let delta =
        Vec2::new(current_pointer.x - initial_pointer.x, current_pointer.y - initial_pointer.y);
    let span_squared = span.x * span.x + span.y * span.y;
    let raw_scale =
        normalize_scale(initial.scale + (delta.x * span.x + delta.y * span.y) / span_squared);
    let extent =
        rotated_half_extent(MobileUiElementTransform { scale: 1.0, ..initial }, natural_size);
    let mut maximum = MOBILE_UI_SCALE_MAX;
    for (constant, slope) in
        [(fixed.x, span.x / 2.0 - extent.x), (fixed.y, span.y / 2.0 - extent.y)]
    {
        if slope < -SNAP_EPSILON {
            maximum = maximum.min(constant / -slope);
        }
    }
    for (constant, slope, maximum_position) in [
        (fixed.x, span.x / 2.0 + extent.x, page.width),
        (fixed.y, span.y / 2.0 + extent.y, page.height),
    ] {
        if slope > SNAP_EPSILON {
            maximum = maximum.min((maximum_position - constant) / slope);
        }
    }
    let maximum_scale =
        source_clamp(initial.scale.max(maximum), MOBILE_UI_SCALE_MIN, MOBILE_UI_SCALE_MAX);
    let scale = source_clamp(raw_scale, MOBILE_UI_SCALE_MIN, maximum_scale);
    ResizeProjection { direction, fixed, initial, maximum_scale, page, scale, span }
}

fn resize_at_scale(resize: ResizeProjection, scale: f64) -> MobileUiElementTransform {
    MobileUiElementTransform {
        rotation: resize.initial.rotation,
        scale,
        x: (resize.fixed.x + resize.span.x * scale / 2.0) / resize.page.width * 100.0,
        y: (resize.fixed.y + resize.span.y * scale / 2.0) / resize.page.height * 100.0,
    }
}

fn resize_handle_point(
    transform: MobileUiElementTransform,
    size: MobileUiSize,
    page: MobileUiSize,
    direction: Vec2,
) -> Vec2 {
    let radians = transform.rotation * std::f64::consts::PI / 180.0;
    let offset = rotate_point(
        Vec2::new(
            direction.x * size.width * transform.scale / 2.0,
            direction.y * size.height * transform.scale / 2.0,
        ),
        radians,
    );
    Vec2::new(
        transform.x / 100.0 * page.width + offset.x,
        transform.y / 100.0 * page.height + offset.y,
    )
}

fn preferred_target_at(position: f64, targets: &[MobileUiSnapGuide]) -> Option<MobileUiSnapGuide> {
    let mut preferred: Option<MobileUiSnapGuide> = None;
    for &target in targets {
        if (target.position - position).abs() > SNAP_EPSILON {
            continue;
        }
        if preferred.is_none_or(|current| target.kind < current.kind) {
            preferred = Some(target);
        }
    }
    preferred
}

fn frame_guide_aligned(frame: MobileUiSnapRect, guide: MobileUiSnapGuide) -> bool {
    let anchors = match guide.axis {
        MobileUiSnapAxis::X => [frame.left, frame.center_x, frame.right],
        MobileUiSnapAxis::Y => [frame.top, frame.center_y, frame.bottom],
    };
    anchors.into_iter().any(|position| (position - guide.position).abs() <= SNAP_EPSILON)
}

fn snap_result(
    transform: MobileUiElementTransform,
    guides: Vec<MobileUiSnapGuide>,
) -> MobileUiSnapResult {
    let mut unique: Vec<MobileUiSnapGuide> = Vec::new();
    for guide in guides {
        if let Some(current) = unique.iter_mut().find(|current| **current == guide) {
            *current = guide;
        } else {
            unique.push(guide);
        }
    }
    unique.sort_by(|left, right| {
        left.axis
            .cmp(&right.axis)
            .then_with(|| {
                left.position.partial_cmp(&right.position).expect("finite snap guide positions")
            })
            .then_with(|| left.kind.cmp(&right.kind))
    });
    MobileUiSnapResult { guides: unique, transform }
}

fn rotate_point(point: Vec2, radians: f64) -> Vec2 {
    let cosine = radians.cos();
    let sine = radians.sin();
    Vec2::new(point.x * cosine - point.y * sine, point.x * sine + point.y * cosine)
}

fn normalize_scale(scale: f64) -> f64 {
    source_clamp(finite_or(scale, 1.0), MOBILE_UI_SCALE_MIN, MOBILE_UI_SCALE_MAX)
}

fn bounded_center(center: f64, half_extent_percent: f64) -> f64 {
    if half_extent_percent >= 50.0 {
        50.0
    } else {
        source_clamp(center, half_extent_percent, 100.0 - half_extent_percent)
    }
}

fn assert_sizes(size: MobileUiSize, page: MobileUiSize) {
    assert!(
        size.width.is_finite() && size.width > 0.0 && size.height.is_finite() && size.height > 0.0,
        "mobile element dimensions are positive and finite"
    );
    assert!(
        page.width.is_finite() && page.width > 0.0 && page.height.is_finite() && page.height > 0.0,
        "mobile editor page dimensions are positive and finite"
    );
}

fn assert_threshold(threshold: f64) {
    assert!(threshold.is_finite() && threshold >= 0.0, "snap threshold is non-negative and finite");
}
