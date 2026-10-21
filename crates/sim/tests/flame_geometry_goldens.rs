//! Actual current-Source Flame native geometry, including stored float/ARGB/UV data.
use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::spells::weld::flame_geometry::*;

#[derive(Clone, Copy, Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn vector(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Input {
    start: Point,
    control: Point,
    end: Point,
    offset: Point,
    width: f64,
    weak: bool,
    enhanced_effects: bool,
    phase_degrees: f64,
    short_width_factor: Option<f64>,
}

impl Input {
    fn geometry(&self) -> NativeFlameLashFactoryInput {
        NativeFlameLashFactoryInput {
            start: self.start.vector(),
            control: self.control.vector(),
            end: self.end.vector(),
            offset: self.offset.vector(),
        }
    }
    fn ribbon(&self) -> NativeFlameLashRibbonInput {
        NativeFlameLashRibbonInput {
            geometry: self.geometry(),
            width: self.width,
            weak: self.weak,
            enhanced_effects: self.enhanced_effects,
            phase_degrees: self.phase_degrees,
            short_width_factor: self.short_width_factor,
        }
    }
}

#[derive(Deserialize)]
struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Geometry {
    kind: String,
    start: Point,
    control: Point,
    extended_end: Point,
    body_end: Point,
    span: f64,
    split_bounds: Bounds,
}

#[derive(Deserialize)]
struct Ribbon {
    kind: String,
    vertices: Vec<f64>,
    indices: Vec<u16>,
    uvs: Vec<f64>,
    colors: Vec<u32>,
    parameters: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    input: Input,
    geometry: Geometry,
    path_points: Vec<Point>,
    normal_pair_count: usize,
    coarse_pair_count: usize,
    ribbon: Ribbon,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrivateSample {
    state: i32,
    angle_degrees: f64,
    radius: f64,
    next_state: i32,
}

#[derive(Deserialize)]
struct Spline {
    points: [Point; 3],
    parameter: f64,
    point: Point,
    normal: Point,
}

#[derive(Deserialize)]
struct ErrorCase {
    operation: String,
    index: usize,
    budget: usize,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    cases: Vec<Case>,
    phases: Vec<(u64, f64)>,
    private_samples: Vec<PrivateSample>,
    splines: Vec<Spline>,
    errors: Vec<ErrorCase>,
}

fn kind(kind: NativeFlameLashGeometryKind) -> &'static str {
    match kind {
        NativeFlameLashGeometryKind::Short => "short",
        NativeFlameLashGeometryKind::Long => "long",
    }
}

fn flattened(points: &[Vec2]) -> Vec<f64> {
    let mut values = Vec::with_capacity(points.len() * 2);
    for point in points {
        values.extend([point.x, point.y]);
    }
    values
}

#[test]
fn retained_native_flame_geometry_matches_current_source() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/flame-geometry-goldens.json")).unwrap();
    for (index, case) in fixture.cases.iter().enumerate() {
        let actual = native_flame_lash_factory_geometry(case.input.geometry()).unwrap();
        let expected = &case.geometry;
        assert_eq!(kind(actual.kind), expected.kind, "geometry {index}");
        assert_eq!(actual.start, expected.start.vector());
        assert_eq!(actual.control, expected.control.vector());
        assert_eq!(actual.extended_end, expected.extended_end.vector());
        assert_eq!(actual.body_end, expected.body_end.vector());
        assert_eq!(actual.span, expected.span);
        let bounds = actual.split_bounds;
        let expected_bounds = &expected.split_bounds;
        assert_eq!(
            [bounds.x, bounds.y, bounds.width, bounds.height],
            [expected_bounds.x, expected_bounds.y, expected_bounds.width, expected_bounds.height]
        );
        let points =
            native_flame_lash_path_points(&actual, NATIVE_FLAME_MAXIMUM_PATH_POINTS).unwrap();
        let expected_points: Vec<_> = case.path_points.iter().map(|point| point.vector()).collect();
        assert_eq!(points, expected_points, "path {index}");
        assert_eq!(
            native_flame_lash_ribbon_pair_count(&actual, true, NATIVE_FLAME_MAXIMUM_RIBBON_PAIRS)
                .unwrap(),
            case.normal_pair_count
        );
        assert_eq!(
            native_flame_lash_ribbon_pair_count(&actual, false, NATIVE_FLAME_MAXIMUM_RIBBON_PAIRS)
                .unwrap(),
            case.coarse_pair_count
        );
        let ribbon = build_native_flame_lash_ribbon(case.input.ribbon()).unwrap();
        assert_eq!(kind(ribbon.kind), case.ribbon.kind);
        assert_eq!(flattened(&ribbon.vertices), case.ribbon.vertices, "vertices {index}");
        assert_eq!(ribbon.indices, case.ribbon.indices, "indices {index}");
        assert_eq!(flattened(&ribbon.uvs), case.ribbon.uvs, "UVs {index}");
        assert_eq!(ribbon.colors, case.ribbon.colors, "ARGB {index}");
        assert_eq!(ribbon.parameters, case.ribbon.parameters, "parameters {index}");
    }
    for (tick, expected) in fixture.phases {
        assert_eq!(native_flame_lash_birth_phase(tick), expected);
    }
    for expected in fixture.private_samples {
        let actual = native_flame_lash_private_sample(expected.state);
        assert_eq!(actual.angle_degrees, expected.angle_degrees);
        assert_eq!(actual.radius, expected.radius);
        assert_eq!(actual.next_state, expected.next_state);
    }
    for expected in fixture.splines {
        let points = expected.points.map(Point::vector);
        assert_eq!(
            native_flame_lash_spline_point(points, expected.parameter),
            expected.point.vector()
        );
        assert_eq!(
            native_flame_lash_spline_normal(points, expected.parameter),
            expected.normal.vector()
        );
    }
    for expected in fixture.errors {
        let geometry =
            native_flame_lash_factory_geometry(fixture.cases[expected.index].input.geometry())
                .unwrap();
        let error = if expected.operation == "path" {
            native_flame_lash_path_points(&geometry, expected.budget).unwrap_err()
        } else {
            native_flame_lash_ribbon_pair_count(&geometry, true, expected.budget).unwrap_err()
        };
        assert_eq!(error, expected.message);
    }
}
