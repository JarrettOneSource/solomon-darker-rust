use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::spells::model::{PrimarySpellTransientBase, PrimarySpellWaterTransient};
use solomon_sim::spells::water::*;

#[derive(Clone, Copy, Deserialize)]
struct Point {
    x: f64,
    y: f64,
}
impl From<Point> for Vec2 {
    fn from(point: Point) -> Self {
        Self::new(point.x, point.y)
    }
}

#[derive(Deserialize)]
struct Obstruction {
    distance: f64,
    point: Point,
}
#[derive(Deserialize)]
struct EmissionSample {
    widen: f64,
    count: u32,
    speed: f64,
    ordinal: u32,
    id: u64,
    direction: [f64; 2],
    origin: [f64; 2],
    jitter: f64,
    obstruction: Option<Obstruction>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WaterInput {
    id: u64,
    age_ticks: u64,
    underpowered: bool,
    direction: Point,
    origin: Point,
    speed: f64,
    obstruction_distance: Option<f64>,
    obstruction_point: Option<Point>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanSample {
    input: WaterInput,
    header: [f64; 10],
    draws: Vec<[f64; 13]>,
    lifetime_ticks: u64,
    lane: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChainSample {
    age_ticks: u64,
    header: [f64; 10],
    draws: Vec<[f64; 13]>,
}
#[derive(Deserialize)]
struct Goldens {
    emissions: Vec<EmissionSample>,
    plans: Vec<PlanSample>,
    chains: Vec<ChainSample>,
}

fn compare_plan(
    plan: WaterFrostJetPlan,
    expected_header: [f64; 10],
    expected_draws: Vec<[f64; 13]>,
) {
    let header = [
        plan.core_scale,
        plan.glint_scale,
        plan.heading,
        f64::from(plan.kind == WaterFrostJetKind::Over),
        plan.lifetime,
        plan.position.x,
        plan.position.y,
        plan.velocity.x,
        plan.velocity.y,
        plan.world_y,
    ];
    for (index, expected) in expected_header.into_iter().enumerate() {
        if index == 2 {
            assert!((header[index] - expected).abs() < 1e-9, "trigonometric heading");
        } else {
            assert_eq!(header[index], expected, "water plan header {index}");
        }
    }
    assert_eq!(plan.draws.len(), expected_draws.len());
    for (draw, expected) in plan.draws.into_iter().zip(expected_draws) {
        let pass = match draw.pass {
            WaterFrostJetPass::AdditiveCore => 0.0,
            WaterFrostJetPass::Core => 1.0,
            WaterFrostJetPass::Glint => 2.0,
        };
        let actual = [
            draw.alpha,
            f64::from(draw.blend == WaterFrostJetBlend::Add),
            draw.color[0],
            draw.color[1],
            draw.color[2],
            pass,
            draw.position.x,
            draw.position.y,
            draw.rotation,
            draw.scale,
            f64::from(draw.sprite == WaterFrostJetSprite::Glint),
            f64::from(pack_water_frost_tint(draw.color)),
            quantize_water_frost_alpha(draw.alpha),
        ];
        for (index, expected) in expected.into_iter().enumerate() {
            if index == 8 {
                assert!((actual[index] - expected).abs() < 1e-9, "trigonometric rotation");
            } else {
                assert_eq!(actual[index], expected, "water draw field {index}");
            }
        }
    }
}

#[test]
fn frost_births_clipping_motion_and_draw_passes_replay_the_website() {
    let goldens: Goldens =
        serde_json::from_str(include_str!("fixtures/website/water-spell-goldens.json")).unwrap();
    for sample in goldens.emissions {
        assert_eq!(water_frost_jet_particle_count(sample.widen, true), sample.count);
        assert_eq!(water_frost_jet_speed(sample.widen), sample.speed);
        let emission = water_frost_jet_emission(WaterFrostJetEmissionInput {
            emitter: Vec2::new(110.0, 120.0),
            base_direction: Vec2::new(0.6, 0.8),
            tick: 17,
            ordinal: sample.ordinal,
            id: sample.id,
            particle_count: sample.count,
            speed: sample.speed,
            widen_half_degrees: sample.widen,
        });
        assert_eq!(emission.direction, Vec2::new(sample.direction[0], sample.direction[1]));
        assert_eq!(emission.origin, Vec2::new(sample.origin[0], sample.origin[1]));
        assert_eq!(emission.jitter_radius, sample.jitter);
        let obstruction = water_frost_jet_obstruction(
            emission,
            Vec2::new(100.0, 100.0),
            sample.id,
            &|_, _| Some(Vec2::new(101.0, 101.0)),
            false,
        );
        assert_eq!(
            obstruction,
            sample.obstruction.map(|value| WaterFrostJetObstruction {
                distance: value.distance,
                point: value.point.into()
            })
        );
    }
    for sample in goldens.plans {
        let input = sample.input;
        let state = PrimarySpellWaterTransient {
            common: PrimarySpellTransientBase {
                age_ticks: input.age_ticks,
                id: input.id,
                light_registration: None,
                owner_id: "oracle".into(),
                painter_registrations: Vec::new(),
                world_key: "boneyard".into(),
            },
            direction: input.direction.into(),
            origin: input.origin.into(),
            speed: input.speed,
            underpowered: input.underpowered,
            obstruction_distance: input.obstruction_distance,
            obstruction_point: input.obstruction_point.map(Into::into),
            variant: 0,
        };
        let plan = water_frost_jet_plan(&state);
        let lane = match water_frost_jet_painter_lane(plan.kind) {
            WaterFrostJetPainterLane::WorldSorted => "world-sorted",
            WaterFrostJetPainterLane::PostWorldQueue => "post-world-queue",
        };
        assert_eq!(lane, sample.lane);
        assert_eq!(water_frost_jet_lifetime_ticks(input.id), sample.lifetime_ticks);
        compare_plan(plan, sample.header, sample.draws);
    }
    for sample in goldens.chains {
        let plan = water_frost_jet_chaining_plan(
            7,
            sample.age_ticks,
            Vec2::new(0.6, 0.8),
            Vec2::new(100.2, 100.7),
        );
        compare_plan(plan, sample.header, sample.draws);
    }
}
