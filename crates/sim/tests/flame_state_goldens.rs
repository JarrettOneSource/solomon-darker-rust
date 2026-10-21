//! Current Source birth-owned Flame fade and Region camera clocks, including RNG.
use serde::Deserialize;
use solomon_sim::spells::weld::flame_state::*;
use solomon_sim::{NativeRng, Vec2};

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
struct Random {
    index_a: usize,
    index_b: usize,
    words: Vec<u32>,
}

#[derive(Deserialize)]
struct Camera {
    seed: i64,
    underpowered: bool,
    displacement: Option<Point>,
    rng: Random,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fade {
    birth_alpha: f64,
    alpha_step: f64,
    age: u64,
    alpha: f64,
    wrapper_intensity: f64,
}

#[derive(Deserialize)]
struct Decay {
    value: Point,
    updates: u64,
    squared: f64,
    result: Point,
}

#[derive(Deserialize)]
struct Fixture {
    cameras: Vec<Camera>,
    fades: Vec<Fade>,
    decays: Vec<Decay>,
}

#[test]
fn native_flame_birth_fade_and_camera_clocks_match_current_source() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/flame-state-goldens.json")).unwrap();
    for expected in fixture.cameras {
        let mut rng = NativeRng::new(expected.seed);
        let actual = draw_native_flame_camera_displacement(&mut rng, expected.underpowered);
        assert_eq!(actual, expected.displacement.map(Point::vector));
        assert_eq!(rng.index_a, expected.rng.index_a);
        assert_eq!(rng.index_b, expected.rng.index_b);
        assert_eq!(rng.words.as_slice(), expected.rng.words);
    }
    for expected in fixture.fades {
        assert_eq!(
            native_flame_fade_alpha(expected.birth_alpha, expected.alpha_step, expected.age),
            expected.alpha
        );
        assert_eq!(
            native_flame_fade_wrapper_intensity(expected.birth_alpha, expected.age),
            expected.wrapper_intensity
        );
    }
    for expected in fixture.decays {
        assert_eq!(native_camera_magnitude_squared(expected.value.vector()), expected.squared);
        assert_eq!(
            decay_native_region_camera_displacement(expected.value.vector(), expected.updates),
            expected.result.vector()
        );
    }
}
