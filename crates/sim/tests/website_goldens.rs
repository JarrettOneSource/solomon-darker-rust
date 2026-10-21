//! Differential replays of the Website's TypeScript kernels.
//!
//! `scripts/record-website-goldens.sh` runs the reference kernels under Node through the
//! `tools/website-oracle/*-goldens.mjs` scripts and records what they produce. These tests
//! replay the same inputs through the Rust port and require the same numbers, bit for bit;
//! only trigonometry is allowed a last-bit tolerance.

use serde::Deserialize;
use solomon_sim::appearance::{StarterColorOwner, roll_native_starter_equipment_appearance};
use solomon_sim::hub::ambient::{
    HubAmbientState, HubColor, hub_fountain_particle_alpha, hub_seal_colors, hub_statue_offsets,
};
use solomon_sim::hub::painter::{
    HUB_COLLEGE_STATUE, HUB_COLLEGE_STATUE_PAINTER_ORDINAL,
    HUB_COURTYARD_OBSTACLE_PAINTER_ORDINAL_BASE, HUB_COURTYARD_OBSTACLES, HUB_FIXED_PAINTER_IDS,
    HUB_NORTH_ARCH_BIAS_RECT, HubFixedActor, courtyard_player_sort_bias, hub_fixed_painter_ordinal,
};
use solomon_sim::hub::regions::{
    HUB_COLLEGE_INTRO_FADE_RATE, HUB_OUTGOING_FADE_RATE, HUB_PORTALS, HubRegion,
    begin_hub_transition, hub_incoming_fade_rate, hub_incoming_placement,
    hub_portal_touched_by_player, is_hub_transition_edge, plan_hub_scripted_movement,
};
use solomon_sim::hub::teacher::{
    HubTeacherBurstMember, HubTeacherPhase, hub_teacher_burst_at, hub_teacher_frame_at,
    hub_teacher_phase_at,
};
use solomon_sim::hub::{HUB_COURTYARD_SEGMENTS, HubWizard, NATIVE_HUB_COLLISION_RNG_SEED};
use solomon_sim::random_domain::{native_random_float_from_word, native_random_int_from_word};
use solomon_sim::{NativeRng, Vec2};

#[derive(Deserialize)]
struct WalkGoldens {
    scenarios: Vec<WalkScenario>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalkScenario {
    name: String,
    start: [f64; 2],
    stages: Vec<Stage>,
    fallback_samples: u32,
    /// Per tick: position x/y, velocity x/y, heading index, gait, walk cycle, sampler state.
    ticks: Vec<[f64; 8]>,
}

#[derive(Deserialize)]
struct Stage {
    movement: [f64; 2],
    ticks: usize,
}

#[derive(Deserialize)]
struct AppearanceGoldens {
    cases: Vec<AppearanceCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppearanceCase {
    owner: String,
    seed: i64,
    primary_tint: u32,
    secondary_tint: u32,
}

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/website/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

#[test]
fn hub_walks_replay_the_website_kernels_exactly() {
    let goldens: WalkGoldens = serde_json::from_str(&fixture("hub-walk-goldens.json")).unwrap();
    assert!(!goldens.scenarios.is_empty());
    for scenario in &goldens.scenarios {
        let mut wizard = HubWizard::at(Vec2::new(scenario.start[0], scenario.start[1]));
        let mut rng = NATIVE_HUB_COLLISION_RNG_SEED;
        let mut fallback_samples = 0;
        let mut expected = scenario.ticks.iter().enumerate();
        for stage in &scenario.stages {
            for _ in 0..stage.ticks {
                let before = rng;
                wizard.tick_against(
                    Vec2::new(stage.movement[0], stage.movement[1]),
                    &mut rng,
                    &HUB_COURTYARD_SEGMENTS,
                );
                if rng != before {
                    fallback_samples += 1;
                }
                let (index, tick) = expected.next().expect("a golden record for every tick");
                let actual = [
                    wizard.position.x,
                    wizard.position.y,
                    wizard.velocity.x,
                    wizard.velocity.y,
                    wizard.heading_index as f64,
                    wizard.gait_degrees,
                    wizard.walk_cycle_primary,
                    f64::from(rng),
                ];
                assert_eq!(actual, *tick, "scenario {} diverged at tick {index}", scenario.name);
            }
        }
        assert!(expected.next().is_none(), "scenario {} has unreplayed ticks", scenario.name);
        assert_eq!(fallback_samples, scenario.fallback_samples, "scenario {}", scenario.name);
    }
}

#[test]
fn starter_appearances_match_the_website_roll() {
    let goldens: AppearanceGoldens =
        serde_json::from_str(&fixture("starter-appearance-goldens.json")).unwrap();
    assert!(!goldens.cases.is_empty());
    for case in &goldens.cases {
        let owner = match case.owner.as_str() {
            "air" => StarterColorOwner::Air,
            "earth" => StarterColorOwner::Earth,
            "ether" => StarterColorOwner::Ether,
            "fire" => StarterColorOwner::Fire,
            "water" => StarterColorOwner::Water,
            "college" => StarterColorOwner::College,
            other => panic!("unknown owner {other}"),
        };
        let mut rng = NativeRng::new(case.seed);
        let appearance = roll_native_starter_equipment_appearance(&mut rng, owner);
        assert_eq!(
            (appearance.primary_tint, appearance.secondary_tint),
            (case.primary_tint, case.secondary_tint),
            "{} seed {}: expected {:06x}, rolled {:06x}",
            case.owner,
            case.seed,
            case.primary_tint,
            appearance.primary_tint
        );
    }
}

/// One live drop of fountain spray: id, remaining lifetime, scale.
type FountainDrop = (u32, f64, f64);
/// One ambient tick: sampler state, seal core phase, seal glyph phase, statue phase, teacher
/// tick, the release index while the burst needs world painters (−1 otherwise), live drops.
type AmbientTick = (u32, f64, f64, f64, u64, i64, Vec<FountainDrop>, Vec<(String, usize)>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmbientGoldens {
    initial: AmbientInitial,
    ticks: Vec<AmbientTick>,
    samples: Vec<AmbientSample>,
    world_samples: Vec<AmbientWorldSample>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmbientWorldSample {
    tick: u64,
    registrations: Option<[usize; 2]>,
    next_actor: usize,
    next_transient: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmbientInitial {
    rng_state: u32,
    seal_core_phase: f64,
    seal_glyph_phase: f64,
    statue_phase_degrees: f64,
    teacher_tick: u64,
    fountain_particles: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmbientSample {
    tick: usize,
    seal_core: [f64; 4],
    seal_glyphs: [f64; 4],
    statue_aura: [f64; 2],
    statue_body: [f64; 2],
    fountain_alphas: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeacherGoldens {
    /// Per tick and seed: the phase name and the pose frame.
    phases: Vec<(u64, u32, String, u32)>,
    bursts: Vec<TeacherBurst>,
    random_domain: RandomDomainGoldens,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeacherBurst {
    tick: u64,
    seed: u32,
    /// −1 while the burst is hidden.
    age_ticks: i64,
    release_index: u64,
    visible: bool,
    /// Each member: alpha, scale x, scale y, visible.
    core: [f64; 4],
    flare: [f64; 4],
    column: [f64; 4],
    /// Alpha, frame, scale x, scale y, visible.
    frames: [f64; 5],
}

#[derive(Deserialize)]
struct RandomDomainGoldens {
    /// Word, exclusive bound, result.
    ints: Vec<(u32, u32, u32)>,
    /// Word, maximum, result.
    floats: Vec<(u32, f64, f64)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegionGoldens {
    college_intro_fade_rate: f64,
    outgoing_fade_rate: f64,
    regions: Vec<RegionRecord>,
    portals: Vec<PortalRecord>,
    /// Source, destination, whether a doorway joins them.
    edges: Vec<(String, String, u8)>,
    placements: Vec<PlacementRecord>,
    grid_step: i64,
    /// Region index, x, y, portal index, scripted target x and y, in grid order.
    portal_hits: Vec<(usize, f64, f64, usize, f64, f64)>,
    portal_misses: usize,
    scripted_walks: Vec<ScriptedWalk>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegionRecord {
    id: String,
    native_id: u8,
    size: [f64; 2],
    segment_count: usize,
    incoming_fade_rate: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortalRecord {
    source: String,
    destination: String,
    trigger: [f64; 4],
    scripted_speed: f64,
    scripted_target: [f64; 2],
    preserve_contact_x: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlacementRecord {
    source: String,
    destination: String,
    position: [f64; 2],
    scripted_speed: f64,
    scripted_target: [f64; 2],
}

#[derive(Deserialize)]
struct ScriptedWalk {
    name: String,
    start: [f64; 2],
    target: [f64; 2],
    speed: f64,
    /// Per tick: position x and y, requested velocity x and y, movement active.
    steps: Vec<[f64; 5]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PainterGoldens {
    fixed_painter_ids: Vec<String>,
    obstacles: Vec<FixedActorRecord>,
    statue: FixedActorRecord,
    north_arch_bias_rect: [f64; 4],
    /// x, y, heading index, bias.
    bias_samples: Vec<(f64, f64, i64, f64)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixedActorRecord {
    id: String,
    position: [f64; 2],
    radius: f64,
    sort_bias: f64,
}

fn vec2(point: [f64; 2]) -> Vec2 {
    Vec2::new(point[0], point[1])
}

fn region(id: &str) -> HubRegion {
    HubRegion::from_id(id).unwrap_or_else(|| panic!("unknown region {id}"))
}

fn flag(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

fn color(color: HubColor) -> [f64; 4] {
    [color.red, color.green, color.blue, color.alpha]
}

fn member(member: HubTeacherBurstMember) -> [f64; 4] {
    [member.alpha, member.scale_x, member.scale_y, flag(member.visible)]
}

fn actor(actor: &HubFixedActor) -> ([f64; 2], f64, f64) {
    ([actor.position.x, actor.position.y], actor.radius, actor.sort_bias)
}

/// Sine and cosine are the one place the port may differ from V8 in the last bit.
const TRIG_TOLERANCE: f64 = 1e-9;

fn assert_close(actual: Vec2, expected: [f64; 2], context: &str) {
    let [x, y] = expected;
    assert!(
        (actual.x - x).abs() <= TRIG_TOLERANCE && (actual.y - y).abs() <= TRIG_TOLERANCE,
        "{context}: expected ({x}, {y}), got ({}, {})",
        actual.x,
        actual.y
    );
}

#[test]
fn courtyard_ambient_replays_the_website_state_and_presentation() {
    let goldens: AmbientGoldens =
        serde_json::from_str(&fixture("hub-ambient-goldens.json")).unwrap();
    assert!(!goldens.ticks.is_empty());
    let mut state = HubAmbientState::new();
    let initial = &goldens.initial;
    assert_eq!(state.rng_state, initial.rng_state);
    assert_eq!(state.seal_core_phase, initial.seal_core_phase);
    assert_eq!(state.seal_glyph_phase, initial.seal_glyph_phase);
    assert_eq!(state.statue_phase_degrees, initial.statue_phase_degrees);
    assert_eq!(state.teacher_tick, initial.teacher_tick);
    assert_eq!(state.fountain_particles.len(), initial.fountain_particles);

    let mut samples = goldens.samples.iter().peekable();
    for (index, expected) in goldens.ticks.iter().enumerate() {
        let tick = index + 1;
        state.step(None);
        let (
            rng_state,
            core_phase,
            glyph_phase,
            statue_phase,
            teacher_tick,
            release,
            drops,
            registrations,
        ) = expected;
        assert_eq!(state.rng_state, *rng_state, "sampler state at tick {tick}");
        assert_eq!(state.seal_core_phase, *core_phase, "seal core phase at tick {tick}");
        assert_eq!(state.seal_glyph_phase, *glyph_phase, "seal glyph phase at tick {tick}");
        assert_eq!(state.statue_phase_degrees, *statue_phase, "statue phase at tick {tick}");
        assert_eq!(state.teacher_tick, *teacher_tick, "teacher tick at tick {tick}");
        let burst = state.teacher_burst();
        let world_release =
            if burst.needs_world_painters() { burst.release_index as i64 } else { -1 };
        assert_eq!(world_release, *release, "teacher world release at tick {tick}");
        let mut actual_registrations = Vec::new();
        if let Some(release) = state.teacher_world_release {
            for registration in release.painter_registrations {
                actual_registrations
                    .push(("transient".to_owned(), registration.registration_ordinal));
            }
        }
        assert_eq!(
            &actual_registrations, registrations,
            "teacher world painter registrations at tick {tick}"
        );
        let actual_drops: Vec<FountainDrop> = state
            .fountain_particles
            .iter()
            .map(|drop| (drop.id, drop.remaining, drop.scale))
            .collect();
        assert_eq!(&actual_drops, drops, "fountain drops at tick {tick}");

        if samples.peek().is_some_and(|sample| sample.tick == tick) {
            let sample = samples.next().unwrap();
            let seals = hub_seal_colors(&state);
            assert_eq!(color(seals.core), sample.seal_core, "seal core colour at tick {tick}");
            assert_eq!(color(seals.glyphs), sample.seal_glyphs, "seal glyph colour at tick {tick}");
            let statue = hub_statue_offsets(&state);
            assert_close(statue.aura, sample.statue_aura, &format!("statue aura at tick {tick}"));
            assert_close(statue.body, sample.statue_body, &format!("statue body at tick {tick}"));
            let alphas: Vec<f64> =
                state.fountain_particles.iter().map(hub_fountain_particle_alpha).collect();
            assert_eq!(alphas, sample.fountain_alphas, "fountain alphas at tick {tick}");
        }
    }
    assert!(samples.next().is_none(), "every presentation sample was replayed");
    let mut managed = HubAmbientState::new();
    let mut order = solomon_sim::painter::WorldPainterOrder {
        next_actor_ordinal: 100,
        next_transient_ordinal: 17,
    };
    for sample in goldens.world_samples {
        while managed.teacher_tick < sample.tick {
            managed.step(Some(&mut order));
        }
        let registrations = managed.teacher_world_release.map(|release| {
            release.painter_registrations.map(|registration| registration.registration_ordinal)
        });
        assert_eq!(
            registrations, sample.registrations,
            "managed teacher release at tick {}",
            sample.tick
        );
        assert_eq!(order.next_actor_ordinal, sample.next_actor);
        assert_eq!(order.next_transient_ordinal, sample.next_transient);
    }
}

#[test]
fn teacher_cycle_burst_and_random_domain_replay_the_website_kernels() {
    let goldens: TeacherGoldens =
        serde_json::from_str(&fixture("hub-teacher-goldens.json")).unwrap();
    assert!(!goldens.phases.is_empty() && !goldens.bursts.is_empty());
    for (tick, seed, phase, frame) in &goldens.phases {
        let expected_phase = match phase.as_str() {
            "cast" => HubTeacherPhase::Cast,
            "release" => HubTeacherPhase::Release,
            "idle" => HubTeacherPhase::Idle,
            other => panic!("unknown phase {other}"),
        };
        assert_eq!(hub_teacher_phase_at(*tick), expected_phase, "phase at tick {tick}");
        assert_eq!(hub_teacher_frame_at(*tick, *seed), *frame, "frame at tick {tick} seed {seed}");
    }
    for expected in &goldens.bursts {
        let burst = hub_teacher_burst_at(expected.tick, expected.seed);
        let context = format!("burst at tick {} seed {}", expected.tick, expected.seed);
        let age = burst.age_ticks.map_or(-1, |age| age as i64);
        assert_eq!(age, expected.age_ticks, "{context}: age");
        assert_eq!(burst.release_index, expected.release_index, "{context}: release index");
        assert_eq!(burst.visible, expected.visible, "{context}: visible");
        assert_eq!(member(burst.core), expected.core, "{context}: core");
        assert_eq!(member(burst.flare), expected.flare, "{context}: flare");
        assert_eq!(member(burst.column), expected.column, "{context}: column");
        let strip = burst.frames.member;
        let frames = [
            strip.alpha,
            f64::from(burst.frames.frame),
            strip.scale_x,
            strip.scale_y,
            flag(strip.visible),
        ];
        assert_eq!(frames, expected.frames, "{context}: frames");
    }
    for (word, bound, result) in &goldens.random_domain.ints {
        assert_eq!(
            native_random_int_from_word(*word, *bound),
            *result,
            "integer from word {word:#x} below {bound}"
        );
    }
    for (word, maximum, result) in &goldens.random_domain.floats {
        assert_eq!(
            native_random_float_from_word(*word, *maximum),
            *result,
            "float from word {word:#x} up to {maximum}"
        );
    }
}

#[test]
fn hub_regions_doorways_and_scripted_walks_replay_the_website_kernels() {
    let goldens: RegionGoldens =
        serde_json::from_str(&fixture("hub-regions-goldens.json")).unwrap();
    assert_eq!(goldens.college_intro_fade_rate, HUB_COLLEGE_INTRO_FADE_RATE);
    assert_eq!(goldens.outgoing_fade_rate, HUB_OUTGOING_FADE_RATE);

    let regions: Vec<HubRegion> = goldens.regions.iter().map(|record| region(&record.id)).collect();
    assert_eq!(regions, HubRegion::ALL, "region order");
    for (record, region) in goldens.regions.iter().zip(regions.iter().copied()) {
        assert_eq!(region.native_id(), record.native_id, "{} native id", record.id);
        assert_eq!(region.size(), vec2(record.size), "{} size", record.id);
        assert_eq!(hub_incoming_fade_rate(region), record.incoming_fade_rate, "{} fade", record.id);
        if region == HubRegion::Courtyard {
            assert_eq!(HUB_COURTYARD_SEGMENTS.len(), record.segment_count, "courtyard segments");
        }
    }

    assert_eq!(goldens.portals.len(), HUB_PORTALS.len());
    for (index, (record, portal)) in goldens.portals.iter().zip(HUB_PORTALS.iter()).enumerate() {
        assert_eq!(portal.source, region(&record.source), "portal {index} source");
        assert_eq!(portal.destination, region(&record.destination), "portal {index} destination");
        let trigger = [portal.trigger.x1, portal.trigger.y1, portal.trigger.x2, portal.trigger.y2];
        assert_eq!(trigger, record.trigger, "portal {index} trigger");
        assert_eq!(portal.scripted_speed, record.scripted_speed, "portal {index} speed");
        assert_eq!(portal.scripted_target, vec2(record.scripted_target), "portal {index} target");
        assert_eq!(portal.preserve_contact_x, record.preserve_contact_x, "portal {index} contact");
    }
    for (source, destination, edge) in &goldens.edges {
        assert_eq!(
            is_hub_transition_edge(region(source), region(destination)),
            *edge == 1,
            "edge {source} -> {destination}"
        );
    }
    for record in &goldens.placements {
        let placement = hub_incoming_placement(region(&record.source), region(&record.destination));
        let context = format!("arrival {} -> {}", record.source, record.destination);
        assert_eq!(placement.position, vec2(record.position), "{context}: position");
        assert_eq!(placement.scripted_speed, record.scripted_speed, "{context}: speed");
        assert_eq!(placement.scripted_target, vec2(record.scripted_target), "{context}: target");
    }

    // The oracle swept a grid across and just outside every region; walk it in the same order.
    let mut hits = goldens.portal_hits.iter();
    let mut misses = 0;
    for (region_index, region) in regions.iter().copied().enumerate() {
        let size = region.size();
        let (width, height) = (size.x as i64, size.y as i64);
        let mut y = -64;
        while y <= height + 64 {
            let mut x = -64;
            while x <= width + 64 {
                let position = Vec2::new(x as f64 + 0.5, y as f64 + 0.5);
                if let Some(portal) = hub_portal_touched_by_player(region, position) {
                    let expected = hits.next().unwrap_or_else(|| {
                        panic!("unexpected doorway hit in {} at {position:?}", region.id())
                    });
                    let portal_index =
                        HUB_PORTALS.iter().position(|candidate| candidate == portal).unwrap();
                    let target = begin_hub_transition(portal, position).scripted_target;
                    let actual =
                        (region_index, position.x, position.y, portal_index, target.x, target.y);
                    assert_eq!(actual, *expected, "doorway hit in {}", region.id());
                } else {
                    misses += 1;
                }
                x += goldens.grid_step;
            }
            y += goldens.grid_step;
        }
    }
    assert!(hits.next().is_none(), "every recorded doorway hit was reached");
    assert_eq!(misses, goldens.portal_misses, "doorway misses");

    for walk in &goldens.scripted_walks {
        assert!(!walk.steps.is_empty(), "walk {} recorded no steps", walk.name);
        let target = vec2(walk.target);
        let mut position = vec2(walk.start);
        for (index, expected) in walk.steps.iter().enumerate() {
            let plan = plan_hub_scripted_movement(position, target, walk.speed);
            position = Vec2::new(position.x + plan.delta.x, position.y + plan.delta.y);
            let actual = [
                position.x,
                position.y,
                plan.requested_velocity.x,
                plan.requested_velocity.y,
                flag(plan.movement_active),
            ];
            assert_eq!(actual, *expected, "walk {} diverged at step {index}", walk.name);
        }
    }
}

#[test]
fn courtyard_painter_tables_and_arch_bias_match_the_website() {
    let goldens: PainterGoldens =
        serde_json::from_str(&fixture("hub-painter-goldens.json")).unwrap();
    assert_eq!(goldens.fixed_painter_ids, HUB_FIXED_PAINTER_IDS.map(str::to_owned));
    assert_eq!(goldens.north_arch_bias_rect, HUB_NORTH_ARCH_BIAS_RECT);
    assert_eq!(goldens.obstacles.len(), HUB_COURTYARD_OBSTACLES.len());
    for (index, (record, obstacle)) in
        goldens.obstacles.iter().zip(HUB_COURTYARD_OBSTACLES.iter()).enumerate()
    {
        assert_eq!(
            hub_fixed_painter_ordinal(&record.id),
            Some(HUB_COURTYARD_OBSTACLE_PAINTER_ORDINAL_BASE + index),
            "{} ordinal",
            record.id
        );
        assert_eq!(
            actor(obstacle),
            (record.position, record.radius, record.sort_bias),
            "{}",
            record.id
        );
    }
    let statue = &goldens.statue;
    assert_eq!(hub_fixed_painter_ordinal(&statue.id), Some(HUB_COLLEGE_STATUE_PAINTER_ORDINAL));
    assert_eq!(actor(&HUB_COLLEGE_STATUE), (statue.position, statue.radius, statue.sort_bias));
    assert!(!goldens.bias_samples.is_empty());
    for (x, y, heading, bias) in &goldens.bias_samples {
        assert_eq!(
            courtyard_player_sort_bias(Vec2::new(*x, *y), *heading),
            *bias,
            "sort bias at ({x}, {y}) heading {heading}"
        );
    }
}

#[derive(Deserialize)]
struct DepthGoldens {
    bands: Vec<(String, f64)>,
    actors: Vec<[f64; 3]>,
    layers: Vec<(String, String, usize, f64)>,
    order: Vec<usize>,
    insertion_cases: Vec<PainterInsertionCase>,
}

#[derive(Deserialize)]
struct PainterInsertionCase {
    layers: Vec<(String, String, usize, f64, bool, bool)>,
    order: Vec<usize>,
}

#[test]
fn depth_bands_and_manager_lanes_replay_the_website() {
    use solomon_sim::hub::depth::{HUB_WORLD_DEPTH, hub_actor_depth};
    use solomon_sim::hub::painter::{PainterLane, PainterLayer, hub_painter_order};
    let goldens: DepthGoldens = serde_json::from_str(&fixture("hub-depth-goldens.json")).unwrap();
    let bands: Vec<_> =
        HUB_WORLD_DEPTH.iter().map(|band| (band.name().to_owned(), band.value())).collect();
    assert_eq!(bands, goldens.bands);
    for [y, reference, expected] in goldens.actors {
        assert_eq!(
            hub_actor_depth(y, reference),
            expected,
            "actor depth at {y} relative to {reference}"
        );
    }
    let layers: Vec<_> = goldens
        .layers
        .iter()
        .map(|(_, lane, ordinal, y)| PainterLayer {
            visible: true,
            is_insertion: false,
            manager_lane: match lane.as_str() {
                "actor" => PainterLane::Actor,
                "scenery" => PainterLane::Scenery,
                "transient" => PainterLane::Transient,
                other => panic!("unknown painter lane {other}"),
            },
            registration_ordinal: *ordinal,
            world_y: *y,
            sort_bias: 0.0,
        })
        .collect();
    assert_eq!(hub_painter_order(&layers, 100.0), goldens.order);
    for case in goldens.insertion_cases {
        let layers: Vec<_> = case
            .layers
            .into_iter()
            .map(|(_, lane, registration_ordinal, world_y, visible, is_insertion)| PainterLayer {
                manager_lane: match lane.as_str() {
                    "actor" => PainterLane::Actor,
                    "scenery" => PainterLane::Scenery,
                    _ => PainterLane::Transient,
                },
                registration_ordinal,
                world_y,
                visible,
                is_insertion,
                sort_bias: 0.0,
            })
            .collect();
        assert_eq!(hub_painter_order(&layers, 100.0), case.order, "native insertion bucket order");
    }
}

#[derive(Deserialize)]
struct WizardAttachmentGoldens {
    samples: Vec<WizardAttachmentSample>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WizardAttachmentSample {
    heading: usize,
    gait: f64,
    walk_cycle: f64,
    fixed_robe_pose: usize,
    attachment_pose: usize,
    robe_pose: usize,
    fixed: [f64; 2],
    front: [f64; 2],
    head: [f64; 2],
    unselected: bool,
}

#[test]
fn wizard_attachments_replay_every_facing_and_the_unselected_robe_pose() {
    use solomon_sim::presentation::wizard_draw_plan;
    let goldens: WizardAttachmentGoldens =
        serde_json::from_str(&fixture("wizard-attachments-goldens.json")).unwrap();
    for sample in goldens.samples {
        let plan = wizard_draw_plan(sample.heading, sample.walk_cycle, sample.gait);
        assert!(sample.unselected);
        assert_eq!(plan.fixed_robe_pose, sample.fixed_robe_pose);
        assert_eq!(plan.attachment_pose, sample.attachment_pose);
        assert_eq!(plan.robe_pose, sample.robe_pose);
        let context = format!("heading {} gait {}", sample.heading, sample.gait);
        assert_close(plan.fixed_robe_offset, sample.fixed, &context);
        assert_close(plan.front_attachment_offset, sample.front, &context);
        assert_close(plan.head_offset, sample.head, &context);
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeacherViewGoldens {
    position: [f64; 2],
    rune_center: [f64; 2],
    rune_alpha: f64,
    samples: Vec<TeacherViewSample>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeacherViewSample {
    tick: u64,
    seed: u32,
    actor_frame: u32,
    release_origin: [f64; 2],
    world_painters: Option<[usize; 2]>,
    core: [f64; 4],
    flare: [f64; 4],
    column: [f64; 4],
    frames: [f64; 5],
}

#[test]
fn teacher_view_keeps_its_cast_origin_and_transient_painters_through_the_release() {
    use solomon_sim::hub::painter::PainterLane;
    use solomon_sim::hub::teacher::{
        HUB_TEACHER_POSITION, HUB_TEACHER_RUNE_ALPHA, HUB_TEACHER_RUNE_CENTER,
        hub_teacher_draw_plan,
    };
    let goldens: TeacherViewGoldens =
        serde_json::from_str(&fixture("hub-teacher-view-goldens.json")).unwrap();
    assert_eq!(HUB_TEACHER_POSITION, vec2(goldens.position));
    assert_eq!(HUB_TEACHER_RUNE_CENTER, vec2(goldens.rune_center));
    assert_eq!(HUB_TEACHER_RUNE_ALPHA, goldens.rune_alpha);
    for sample in goldens.samples {
        let plan = hub_teacher_draw_plan(sample.tick, sample.seed, None);
        assert_eq!(plan.actor_frame, sample.actor_frame);
        assert_eq!(plan.release_origin, vec2(sample.release_origin));
        assert_eq!(
            plan.world_painters.map(|layers| layers.map(|layer| layer.registration_ordinal)),
            sample.world_painters
        );
        if let Some(layers) = plan.world_painters {
            for layer in layers {
                assert_eq!(layer.manager_lane, PainterLane::Transient);
                assert_eq!(layer.world_y, plan.release_origin.y);
            }
        }
        assert_eq!(member(plan.burst.core), sample.core);
        assert_eq!(member(plan.burst.flare), sample.flare);
        assert_eq!(member(plan.burst.column), sample.column);
        let frames = plan.burst.frames;
        let expected = [
            frames.member.alpha,
            frames.member.scale_x,
            frames.member.scale_y,
            flag(frames.member.visible),
            f64::from(frames.frame),
        ];
        assert_eq!(expected, sample.frames);
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomGoldens {
    rooms: Vec<RoomGolden>,
    walks: Vec<RoomWalkGolden>,
    memorator_headings: Vec<([f64; 2], usize)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomGolden {
    id: String,
    segments: Vec<[f64; 4]>,
    actors: Vec<RoomActorGolden>,
    props: Vec<RoomPropGolden>,
    grid: Vec<RoomGridGolden>,
    flames: Vec<[f64; 2]>,
    flame_frames: Vec<RoomFlameGolden>,
    cameras: Vec<RoomCameraGolden>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomActorGolden {
    id: String,
    position: [f64; 2],
    radius: f64,
    visual_position: [f64; 2],
    painter_y: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomPropGolden {
    id: String,
    painter_id: String,
    position: [f64; 2],
    radius: f64,
    visual: Option<RoomPropVisualGolden>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomPropVisualGolden {
    kind: String,
    frame_index: usize,
    position: [f64; 2],
    painter_y: f64,
    marker: bool,
}

#[derive(Deserialize)]
struct RoomGridGolden {
    position: [f64; 2],
    clear: bool,
    moves: Vec<[f64; 5]>,
}

#[derive(Deserialize)]
struct RoomFlameGolden {
    tick: u64,
    samples: Vec<[f64; 3]>,
}

#[derive(Deserialize)]
struct RoomCameraGolden {
    position: [f64; 2],
    origin: [f64; 2],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomWalkGolden {
    name: String,
    region: String,
    start: [f64; 2],
    direction: [f64; 2],
    initial_transition: Option<RoomTransitionGolden>,
    samples: Vec<[f64; 14]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomTransitionGolden {
    alpha: f64,
    destination: String,
    phase: String,
    scripted_speed: f64,
    scripted_target: Vec2Golden,
    source_region: String,
}

#[derive(Deserialize)]
struct Vec2Golden {
    x: f64,
    y: f64,
}

#[test]
fn private_rooms_collision_presentation_and_doorway_lifecycles_match_the_website() {
    use solomon_sim::hub::regions::{HubTransition, HubTransitionPhase};
    use solomon_sim::hub::rooms::{
        HubRoomPropKind, hub_memorator_heading_index, hub_room_flame_transform,
    };
    use solomon_sim::hub::{
        hub_region_camera_origin, is_traversable_against_segments, move_with_segments,
    };
    let goldens: RoomGoldens = serde_json::from_str(&fixture("hub-rooms-goldens.json")).unwrap();
    for (position, heading) in &goldens.memorator_headings {
        assert_eq!(hub_memorator_heading_index(vec2(*position)), *heading);
    }
    for room in &goldens.rooms {
        let region = region(&room.id);
        let segments: Vec<_> = region.segments().iter().map(|s| [s.x1, s.y1, s.x2, s.y2]).collect();
        assert_eq!(segments, room.segments, "{} segment table", room.id);
        assert_eq!(region.room_actors().len(), room.actors.len());
        for (actual, expected) in region.room_actors().iter().zip(&room.actors) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.position, vec2(expected.position));
            assert_eq!(actual.radius, expected.radius);
            assert_eq!(actual.visual_position, vec2(expected.visual_position));
            assert_eq!(actual.painter_y, expected.painter_y);
            let painter_id = if actual.id == "dowser" { "shlorio" } else { actual.id };
            assert_eq!(Some(actual.registration_ordinal), hub_fixed_painter_ordinal(painter_id));
        }
        assert_eq!(region.room_props().len(), room.props.len());
        for (actual, expected) in region.room_props().iter().zip(&room.props) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.position, vec2(expected.position));
            assert_eq!(actual.radius, expected.radius);
            assert_eq!(
                Some(actual.registration_ordinal),
                hub_fixed_painter_ordinal(&expected.painter_id)
            );
            match (actual.visual, &expected.visual) {
                (None, None) => {}
                (Some(visual), Some(expected)) => {
                    let kind = match expected.kind.as_str() {
                        "portrait" => HubRoomPropKind::Portrait,
                        "room-frame" => HubRoomPropKind::RoomFrame,
                        "room-layer" => HubRoomPropKind::RoomLayer,
                        other => panic!("unknown room prop kind {other}"),
                    };
                    assert_eq!(visual.kind, kind);
                    assert_eq!(visual.frame_index, expected.frame_index);
                    assert_eq!(visual.position, vec2(expected.position));
                    assert_eq!(visual.painter_y, expected.painter_y);
                    assert_eq!(visual.marker, expected.marker);
                }
                _ => panic!("{} visual mismatch", actual.id),
            }
        }
        for grid in &room.grid {
            let position = vec2(grid.position);
            assert_eq!(
                is_traversable_against_segments(position, 25.0, region.segments()),
                grid.clear,
                "{} traversable at {position:?}",
                room.id
            );
            for [dx, dy, x, y, rng] in &grid.moves {
                let moved = move_with_segments(
                    position,
                    Vec2::new(*dx, *dy),
                    25.0,
                    NATIVE_HUB_COLLISION_RNG_SEED,
                    region.segments(),
                );
                assert_eq!(
                    moved.position,
                    Vec2::new(*x, *y),
                    "{} lattice move at {position:?}",
                    room.id
                );
                assert_eq!(f64::from(moved.rng_state), *rng);
            }
        }
        assert_eq!(
            region.flame_anchors(),
            room.flames.iter().copied().map(vec2).collect::<Vec<_>>()
        );
        for frame in &room.flame_frames {
            for (index, expected) in frame.samples.iter().enumerate() {
                let transform = hub_room_flame_transform(region, frame.tick, index);
                assert_eq!(
                    [transform.rotation, transform.scale_x, transform.scale_y],
                    *expected,
                    "{} flame {index} at tick {}",
                    room.id,
                    frame.tick
                );
            }
        }
        for camera in &room.cameras {
            assert_eq!(
                hub_region_camera_origin(region, vec2(camera.position)),
                vec2(camera.origin)
            );
        }
    }
    for walk in &goldens.walks {
        let mut wizard = HubWizard::at(vec2(walk.start));
        wizard.region = region(&walk.region);
        wizard.transition = walk.initial_transition.as_ref().map(|tr| HubTransition {
            alpha: tr.alpha,
            destination: region(&tr.destination),
            phase: match tr.phase.as_str() {
                "college-intro" => HubTransitionPhase::CollegeIntro,
                other => panic!("unknown initial transition {other}"),
            },
            scripted_speed: tr.scripted_speed,
            scripted_target: Vec2::new(tr.scripted_target.x, tr.scripted_target.y),
            source_region: region(&tr.source_region),
        });
        let mut rng = NATIVE_HUB_COLLISION_RNG_SEED;
        for (tick, expected) in walk.samples.iter().enumerate() {
            wizard.tick(vec2(walk.direction), &mut rng);
            let (phase, alpha, speed, target) =
                wizard.transition.map_or((-1.0, 0.0, 0.0, Vec2::ZERO), |tr| {
                    let phase = match tr.phase {
                        HubTransitionPhase::CollegeIntro => 0.0,
                        HubTransitionPhase::CollegeLoadout => 1.0,
                        HubTransitionPhase::Outgoing => 2.0,
                        HubTransitionPhase::Incoming => 3.0,
                    };
                    (phase, tr.alpha, tr.scripted_speed, tr.scripted_target)
                });
            let actual = [
                wizard.position.x,
                wizard.position.y,
                wizard.velocity.x,
                wizard.velocity.y,
                wizard.heading_index as f64,
                wizard.gait_degrees,
                wizard.walk_cycle_primary,
                f64::from(rng),
                f64::from(wizard.region.native_id()),
                phase,
                alpha,
                speed,
                target.x,
                target.y,
            ];
            assert_eq!(actual, *expected, "{} tick {tick}", walk.name);
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcGoldens {
    spline_samples: Vec<NpcSplineSamples>,
    spawn_cases: Vec<NpcSpawnCase>,
    warm_population: NpcPopulationSnapshot,
    population_ticks: Vec<NpcPopulationSnapshot>,
    world_ticks: Vec<NpcWorldTick>,
    trader_cases: Vec<NpcTraderCase>,
    offsets: Vec<NpcOffsets>,
}

#[derive(Clone, Copy, Deserialize)]
struct NpcPoint {
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcSplineSamples {
    path_id: usize,
    samples: Vec<NpcSplineSample>,
}
#[derive(Deserialize)]
struct NpcSplineSample {
    cursor: f64,
    position: NpcPoint,
}
#[derive(Deserialize)]
struct NpcSpawnCase {
    seed: u32,
    student: NpcStudent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcStudent {
    current_speed: f64,
    desired_speed: f64,
    frame_phase: f64,
    gait_degrees: f64,
    heading: f64,
    heading_index: usize,
    id: usize,
    path_cursor: f64,
    path_id: usize,
    path_step: i32,
    painter_registration: NpcRegistration,
    position: NpcPoint,
    profile: NpcStudentProfile,
    props: Vec<NpcProp>,
    reading: bool,
    retired: bool,
    rng_state: u32,
    scale: f64,
    static_collision_enabled: bool,
    tick: u64,
    wander: NpcPoint,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcStudentProfile {
    push_resistance: f64,
    push_strength: f64,
    radius: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcRegistration {
    registration_ordinal: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcProp {
    angle: f64,
    palette_index: usize,
    radius: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcPopulationSnapshot {
    #[serde(default)]
    tick: u64,
    next_id: usize,
    rare_path_denominator: u32,
    rng_state: u32,
    spawn_request_pending: bool,
    spawn_ticker_counter: u64,
    students: Vec<NpcStudent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcWorldTick {
    tick: u64,
    player: NpcPlayer,
    collision_rng_state: u32,
    population: NpcPopulationSnapshot,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcPlayer {
    position: NpcPoint,
    velocity: NpcPoint,
    heading_index: usize,
    gait_degrees: f64,
    walk_cycle_primary: f64,
}

#[derive(Deserialize)]
struct NpcTraderCase {
    seed: u32,
    ticks: Vec<NpcTraderTick>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcTraderTick {
    tick: u64,
    common: usize,
    potion: NpcPotionFrame,
    hagatha: NpcHagathaFrame,
    skorcha: Option<NpcSkorchaFrame>,
    transition_ticks_remaining: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcPotionFrame {
    actor_frame: usize,
    balloon_frame: usize,
    balloon_offset_y: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcHagathaFrame {
    body_frame: usize,
    particles: Option<Vec<NpcHagathaParticle>>,
}
#[derive(Deserialize)]
struct NpcHagathaParticle {
    alpha: f64,
    frame: usize,
    id: u64,
    offset: NpcPoint,
    scale: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcSkorchaFrame {
    variant: usize,
    position: NpcPoint,
    gesture: usize,
    hat_frame: usize,
    gesture_ticks_remaining: u64,
}
#[derive(Deserialize)]
struct NpcOffsets {
    student: NpcStudent,
    head: NpcPoint,
    props: Vec<NpcPoint>,
}

fn npc_close(actual: f64, expected: f64, context: &str) {
    // These values integrate sin/cos/atan2 from the student route or presentation kernels.
    assert!(
        (actual - expected).abs() < TRIG_TOLERANCE,
        "{context}: actual {actual:?}, expected {expected:?}"
    );
}

fn npc_point(actual: Vec2, expected: NpcPoint, context: &str) {
    npc_close(actual.x, expected.x, context);
    npc_close(actual.y, expected.y, context);
}

fn npc_student(actual: &solomon_sim::hub::npc::HubStudentState, expected: &NpcStudent) {
    let context = format!("student {} tick {}", expected.id, expected.tick);
    assert_eq!(actual.id, expected.id, "{context}");
    assert_eq!(actual.current_speed, expected.current_speed, "{context}");
    assert_eq!(actual.desired_speed, expected.desired_speed, "{context}");
    npc_close(actual.frame_phase, expected.frame_phase, &context);
    npc_close(actual.gait_degrees, expected.gait_degrees, &context);
    npc_close(actual.heading, expected.heading, &context);
    assert_eq!(actual.heading_index, expected.heading_index, "{context}");
    assert_eq!(actual.path_cursor, expected.path_cursor, "{context}");
    assert_eq!(actual.path_id, expected.path_id, "{context}");
    assert_eq!(actual.path_step, expected.path_step, "{context}");
    assert_eq!(
        actual.registration_ordinal, expected.painter_registration.registration_ordinal,
        "{context}"
    );
    npc_point(actual.position, expected.position, &context);
    npc_close(actual.profile.push_resistance, expected.profile.push_resistance, &context);
    assert_eq!(actual.profile.push_strength, expected.profile.push_strength, "{context}");
    assert_eq!(actual.profile.radius, expected.profile.radius, "{context}");
    assert_eq!(actual.props.len(), expected.props.len(), "{context}");
    for (actual, expected) in actual.props.iter().zip(&expected.props) {
        assert_eq!(actual.angle, expected.angle, "{context}");
        assert_eq!(actual.palette_index, expected.palette_index, "{context}");
        assert_eq!(actual.radius, expected.radius, "{context}");
    }
    assert_eq!(actual.reading, expected.reading, "{context}");
    assert_eq!(actual.retired, expected.retired, "{context}");
    assert_eq!(actual.rng_state, expected.rng_state, "{context}");
    assert_eq!(actual.scale, expected.scale, "{context}");
    assert_eq!(actual.static_collision_enabled, expected.static_collision_enabled, "{context}");
    assert_eq!(actual.tick, expected.tick, "{context}");
    npc_point(actual.wander, expected.wander, &context);
}

fn npc_population(
    actual: &solomon_sim::hub::npc::HubStudentPopulation,
    expected: &NpcPopulationSnapshot,
) {
    assert_eq!(actual.next_id, expected.next_id);
    assert_eq!(actual.rare_path_denominator, expected.rare_path_denominator);
    assert_eq!(actual.rng_state, expected.rng_state);
    assert_eq!(actual.spawn_request_pending, expected.spawn_request_pending);
    assert_eq!(actual.spawn_ticker_counter, expected.spawn_ticker_counter);
    assert_eq!(actual.students.len(), expected.students.len());
    for (actual, expected) in actual.students.iter().zip(&expected.students) {
        npc_student(actual, expected);
    }
}

#[test]
fn students_actor_contacts_and_trader_clocks_replay_the_website() {
    use solomon_sim::hub::npc::{
        HubCommonTraderClock, HubHagathaClock, HubPotionTraderClock, HubSkorchaSchedule,
        HubStudentPopulation, HubStudentSplines, HubStudentState, hub_student_head_offset,
        hub_student_prop_offset,
    };
    use solomon_sim::hub::plan_hub_wizard_tick;

    let goldens: NpcGoldens = serde_json::from_str(&fixture("hub-npc-goldens.json")).unwrap();
    let splines = HubStudentSplines::new();
    for path in &goldens.spline_samples {
        for sample in &path.samples {
            let actual = splines.evaluate(path.path_id, sample.cursor);
            assert_eq!([actual.x, actual.y], [sample.position.x, sample.position.y]);
        }
    }
    for case in &goldens.spawn_cases {
        let expected = &case.student;
        let student = HubStudentState::new(
            &splines,
            expected.id,
            expected.path_id,
            case.seed,
            0.75,
            expected.id,
        );
        npc_student(&student, expected);
    }
    let mut population = HubStudentPopulation::new(None);
    npc_population(&population, &goldens.warm_population);
    let mut tick = 0;
    for expected in &goldens.population_ticks {
        while tick < expected.tick {
            population.step_routes(None);
            tick += 1;
        }
        npc_population(&population, expected);
    }
    let mut population = HubStudentPopulation::new(None);
    let mut wizard = HubWizard::spawn();
    let mut collision_rng = NATIVE_HUB_COLLISION_RNG_SEED;
    let mut tick = 0;
    for expected in &goldens.world_ticks {
        while tick < expected.tick {
            let movement = match tick {
                0..200 => Vec2::new(0.0, 1.0),
                200..550 => Vec2::new(1.0, 0.0),
                550..900 => Vec2::new(0.0, 1.0),
                900..1450 => Vec2::new(-1.0, 0.0),
                1450..1800 => Vec2::new(0.0, -1.0),
                _ => Vec2::ZERO,
            };
            let plan = plan_hub_wizard_tick(wizard.velocity, movement, 1.0);
            population.step(Some((&mut wizard, &plan, true)), &mut collision_rng, None, None);
            tick += 1;
        }
        npc_point(wizard.position, expected.player.position, "world player position");
        npc_point(wizard.velocity, expected.player.velocity, "world player velocity");
        assert_eq!(wizard.heading_index, expected.player.heading_index);
        assert_eq!(wizard.gait_degrees, expected.player.gait_degrees);
        assert_eq!(wizard.walk_cycle_primary, expected.player.walk_cycle_primary);
        assert_eq!(collision_rng, expected.collision_rng_state);
        npc_population(&population, &expected.population);
    }
    for case in &goldens.trader_cases {
        let mut common = HubCommonTraderClock::new(case.seed ^ 5005);
        let mut hagatha = HubHagathaClock::new(case.seed ^ 5001);
        let mut potion = HubPotionTraderClock::new();
        let mut skorcha = HubSkorchaSchedule::with_windows(case.seed, Some(70), Some(90));
        for expected in &case.ticks {
            if expected.tick > 0 {
                common.step();
                hagatha.step();
                potion.step();
                skorcha.step();
            }
            assert_eq!(common.frame(), expected.common, "common tick {}", expected.tick);
            assert_eq!(potion.actor.frame(), expected.potion.actor_frame);
            assert_eq!(potion.balloon_frame(), expected.potion.balloon_frame);
            npc_close(potion.balloon_offset_y(), expected.potion.balloon_offset_y, "balloon drift");
            assert_eq!(hagatha.frame(), expected.hagatha.body_frame);
            if let Some(particles) = &expected.hagatha.particles {
                assert_eq!(hagatha.particles.len(), particles.len());
                for (actual, expected) in hagatha.particles.iter().zip(particles) {
                    assert_eq!(actual.id, expected.id);
                    assert_eq!(actual.frame, expected.frame);
                    assert_eq!(actual.scale, expected.scale);
                    assert_eq!(
                        [actual.offset.x, actual.offset.y],
                        [expected.offset.x, expected.offset.y]
                    );
                    npc_close(actual.alpha(), expected.alpha, "Hagatha particle alpha");
                }
            }
            assert_eq!(skorcha.transition_ticks_remaining, expected.transition_ticks_remaining);
            assert_eq!(skorcha.skorcha.is_some(), expected.skorcha.is_some());
            if let (Some(actual), Some(expected)) = (&skorcha.skorcha, &expected.skorcha) {
                assert_eq!(actual.variant, expected.variant);
                assert_eq!(
                    [actual.position.x, actual.position.y],
                    [expected.position.x, expected.position.y]
                );
                assert_eq!(actual.gesture, expected.gesture);
                assert_eq!(actual.hat_frame(), expected.hat_frame);
                assert_eq!(actual.gesture_ticks_remaining, expected.gesture_ticks_remaining);
            }
        }
    }
    let population = HubStudentPopulation::new(None);
    for expected in &goldens.offsets {
        let student = goldens
            .warm_population
            .students
            .iter()
            .position(|student| student.id == expected.student.id)
            .unwrap();
        let actual = &population.students[student];
        npc_point(hub_student_head_offset(actual), expected.head, "student head offset");
        for (index, prop) in actual.props.iter().enumerate() {
            npc_point(
                hub_student_prop_offset(actual.heading, *prop, index),
                expected.props[index],
                "student prop offset",
            );
        }
    }
}

#[test]
fn skillbook_acquisition_and_every_authored_stat_row_replay_the_website() {
    use serde_json::{Value, json};
    use solomon_sim::profile::{
        NATIVE_SKILL_CATALOG, NativeSkillBookState, NativeSkillValue, WizardDiscipline,
        WizardElement,
    };

    fn snapshot(book: &NativeSkillBookState) -> Value {
        json!({
            "advancedUnlocks": book.advanced_unlocks,
            "disciplineRoot": book.discipline_root,
            "effectiveRanks": &book.effective_ranks[..],
            "elementRoot": book.element_root,
            "learnedSkillOrder": book.learned_skill_order,
            "permanentRanks": &book.permanent_ranks[..],
            "primarySkillId": book.primary_skill_id,
            "weldBuildId": book.weld_build_id,
            "weldComponentRanks": book.weld_component_ranks,
        })
    }

    let fixture: Value = serde_json::from_str(&fixture("profile-goldens.json")).unwrap();
    for (actual, expected) in
        NATIVE_SKILL_CATALOG.iter().zip(fixture["statBook"].as_array().unwrap())
    {
        assert_eq!(actual.id as u64, expected["id"].as_u64().unwrap());
        assert_eq!(u64::from(actual.cap_level), expected["capLevel"].as_u64().unwrap());
        assert_eq!(u64::from(actual.maximum_level), expected["maximumLevel"].as_u64().unwrap());
        assert_eq!(actual.description, expected["description"].as_str().unwrap());
        assert_eq!(actual.quick_description, expected["quickDescription"].as_str().unwrap());
        let config = &fixture["catalog"][actual.id]["config"];
        for (actual_lines, key) in [(actual.stat_lines, "mStats"), (actual.bonus_lines, "mBonus")] {
            let mut expected_lines = Vec::new();
            if let Some(lines) = config[key].as_array() {
                for line in lines {
                    expected_lines.push(line.as_str().unwrap());
                }
            }
            assert_eq!(actual_lines, expected_lines, "skill {} {key}", actual.id);
        }
        let properties = expected["numericProperties"].as_object().unwrap();
        assert_eq!(actual.numeric_properties.len(), properties.len());
        for property in actual.numeric_properties {
            let expected = &properties[property.name];
            match property.value {
                NativeSkillValue::Scalar(value) => assert_eq!(value, expected.as_f64().unwrap()),
                NativeSkillValue::Ranks(values) => {
                    let expected: Vec<_> = expected
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_f64().unwrap())
                        .collect();
                    assert_eq!(values, expected);
                }
            }
        }
    }
    for case in fixture["cases"].as_array().unwrap() {
        let element = WizardElement::ALL
            .into_iter()
            .find(|element| Some(element.as_str()) == case["element"].as_str())
            .unwrap();
        let discipline = WizardDiscipline::ALL
            .into_iter()
            .find(|discipline| Some(discipline.as_str()) == case["discipline"].as_str())
            .unwrap();
        let mut book = NativeSkillBookState::new(element, discipline);
        assert_eq!(snapshot(&book), case["fresh"]);
        book.grant_ranks(72, 2);
        assert_eq!(snapshot(&book), case["granted"]);
        book.grant_ranks(book.primary_skill_id, 100);
        assert_eq!(snapshot(&book), case["capped"]);
    }
    for case in fixture["welds"].as_array().unwrap() {
        let mut book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
        book.grant_weld_build(case["build"]["id"].as_u64().unwrap() as u32);
        assert_eq!(snapshot(&book), case["learned"]);
    }
}

#[test]
fn stock_inventory_and_every_college_service_replay_the_website() {
    use serde_json::{Value, json};
    use solomon_sim::economy::*;
    use solomon_sim::equipment::NativeEquipmentEffect;
    use solomon_sim::profile::WizardElement;

    fn equipment_type(value: &Value) -> Option<EquipmentType> {
        Some(match value.as_str()? {
            "amulet" => EquipmentType::Amulet,
            "hat" => EquipmentType::Hat,
            "ring" => EquipmentType::Ring,
            "robe" => EquipmentType::Robe,
            "staff" => EquipmentType::Staff,
            "wand" => EquipmentType::Wand,
            other => panic!("unknown equipment type {other}"),
        })
    }
    fn slot(value: &Value) -> EquipmentSlot {
        EquipmentSlot::ALL.into_iter().find(|slot| Some(slot.as_str()) == value.as_str()).unwrap()
    }
    fn kind(value: &Value) -> HubItemKind {
        match value.as_str().unwrap() {
            "antidote" => HubItemKind::Antidote,
            "dye" => HubItemKind::Dye,
            "equipment" => HubItemKind::Equipment,
            "health-potion" => HubItemKind::HealthPotion,
            "key" => HubItemKind::Key,
            "mana-potion" => HubItemKind::ManaPotion,
            "mind-chug" => HubItemKind::MindChug,
            "rejuvenation-potion" => HubItemKind::RejuvenationPotion,
            "sack" => HubItemKind::Sack,
            "skill-book" => HubItemKind::SkillBook,
            "wizard-chug" => HubItemKind::WizardChug,
            other => panic!("unknown item kind {other}"),
        }
    }
    fn read_item(value: &Value) -> HubInventoryItem {
        HubInventoryItem {
            mod_content: None,
            mod_item_content: None,
            mod_affixes: None,
            contents: value["contents"]
                .as_array()
                .map(|items| items.iter().map(read_item).collect()),
            equipment_type: equipment_type(&value["equipmentType"]),
            generated_level: value["generatedLevel"].as_u64().map(|value| value as u32),
            icon_records: value["iconRecords"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as usize)
                .collect(),
            icon_tints: value["iconTints"].as_array().map(|values| {
                [
                    values[0].as_u64().map(|value| value as u32),
                    values[1].as_u64().map(|value| value as u32),
                ]
            }),
            id: value["id"].as_u64().unwrap(),
            inventory_slot: value["inventorySlot"].as_u64().map(|value| value as usize),
            kind: kind(&value["kind"]),
            name: value["name"].as_str().unwrap().to_owned(),
            native_subtype: value["nativeSubtype"].as_u64().map(|value| value as u32),
            native_selector: value["nativeSelector"].as_u64().map(|value| value as u32),
            native_effects: value["nativeEffects"].as_array().map(|values| {
                values
                    .iter()
                    .map(|effect| NativeEquipmentEffect {
                        kind: effect["kind"].as_u64().unwrap() as u32,
                        magnitude: effect["magnitude"].as_f64().unwrap(),
                        operator: effect["operator"].as_u64().unwrap() as u32,
                        target: effect["target"].as_u64().unwrap() as u32,
                    })
                    .collect()
            }),
            native_type_id: value["nativeTypeId"].as_u64().unwrap() as u32,
            quantity: value["quantity"].as_u64().unwrap() as u32,
            rarity: value["rarity"].as_str().map(|rarity| match rarity {
                "Epic" => ItemRarity::Epic,
                "Rare" => ItemRarity::Rare,
                other => panic!("rarity {other}"),
            }),
            recipe_index: value["recipeIndex"].as_u64().map(|value| value as usize),
        }
    }
    fn item_snapshot(item: &HubInventoryItem) -> Value {
        json!({ "id":item.id, "kind":item.kind.as_str(), "name":item.name,
            "nativeTypeId":item.native_type_id, "nativeSubtype":item.native_subtype,
            "quantity":item.quantity, "inventorySlot":item.inventory_slot,
            "equipmentType":item.equipment_type.map(EquipmentType::as_str),
            "recipeIndex":item.recipe_index, "generatedLevel":item.generated_level,
            "iconRecords":item.icon_records, "iconTints":item.icon_tints,
            "nativeSelector":item.native_selector,
            "nativeEffects":item.native_effects.as_ref().map(|effects| effects.iter().map(|effect| json!({
                "kind":effect.kind,"magnitude":effect.magnitude,"operator":effect.operator,"target":effect.target,
            })).collect::<Vec<_>>()),
            "rarity":item.rarity.map(|rarity| match rarity { ItemRarity::Epic => "Epic", ItemRarity::Rare => "Rare" }),
            "contents":item.contents.as_ref().map(|items| items.iter().map(item_snapshot).collect::<Vec<_>>()),
        })
    }
    fn items(value: &Value) -> Vec<HubInventoryItem> {
        value.as_array().unwrap().iter().map(read_item).collect()
    }
    fn read_state(value: &Value) -> HubEconomyState {
        let mut state = HubEconomyState::new(0, WizardElement::Ether);
        state.backpack = items(&value["backpack"]);
        state.storage = items(&value["storage"]);
        for slot in EquipmentSlot::ALL {
            let candidate = match slot {
                EquipmentSlot::Ring0 => &value["equipment"]["rings"][0],
                EquipmentSlot::Ring1 => &value["equipment"]["rings"][1],
                EquipmentSlot::Ring2 => &value["equipment"]["rings"][2],
                _ => &value["equipment"][slot.as_str()],
            };
            state
                .equipment
                .set(slot, if candidate.is_null() { None } else { Some(read_item(candidate)) });
        }
        state.fomentius_stock = value["fomentiusStock"]
            .as_array()
            .unwrap()
            .iter()
            .map(|stock| HubShopItem {
                item: read_item(&stock["item"]),
                price: stock["price"].as_u64().unwrap(),
            })
            .collect();
        state.gold = value["gold"].as_u64().unwrap();
        state.charm_capacity = value["charmCapacity"].as_u64().unwrap() as u32;
        state.college_intro_pending = value["collegeIntroPending"].as_bool().unwrap();
        state.tutorial_pending = value["tutorialPending"].as_bool().unwrap();
        state.dowsing_fee = value["dowsingFee"].as_u64().unwrap();
        state.dowsing_rolled = value["dowsingRolled"].as_bool().unwrap();
        state.dowsing_offers = value["dowsingOffers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|offer| DowsingOffer {
                id: offer["id"].as_u64().unwrap(),
                price: offer["price"].as_u64().unwrap(),
                recipe_index: offer["recipeIndex"].as_u64().unwrap() as usize,
            })
            .collect();
        let selectors = |key: &str| {
            value[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect()
        };
        state.first_mixed_selectors = selectors("firstMixedSelectors");
        state.hagatha_bundle_selectors = selectors("hagathaBundleSelectors");
        state.owned_perk_selectors = selectors("ownedPerkSelectors");
        state.next_item_id = value["nextItemId"].as_u64().unwrap();
        state.next_offer_id = value["nextOfferId"].as_u64().unwrap();
        state.revision = value["revision"].as_u64().unwrap();
        state.tonic_purchases = value["tonicPurchases"].as_u64().unwrap() as u32;
        state.rng.index_a = value["rng"]["indexA"].as_u64().unwrap() as usize;
        state.rng.index_b = value["rng"]["indexB"].as_u64().unwrap() as usize;
        state.rng.words = value["rng"]["words"]
            .as_array()
            .unwrap()
            .iter()
            .map(|word| word.as_u64().unwrap() as u32)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let bonuses = &value["unforgeBonuses"];
        state.unforge_bonuses = NativeUnforgeBonuses {
            experience: bonuses["experience"].as_f64().unwrap(),
            mana_cost_reduction: bonuses["manaCostReduction"].as_f64().unwrap(),
            maximum_health: bonuses["maximumHealth"].as_f64().unwrap(),
            maximum_mana: bonuses["maximumMana"].as_f64().unwrap(),
            offensive_damage: bonuses["offensiveDamage"].as_f64().unwrap(),
            recipe_attempt_count: bonuses["recipeAttemptCount"].as_u64().unwrap() as u32,
        };
        let npc = &value["npc"];
        state.npc.boast = BoastState {
            failed: npc["boast"]["failed"].as_bool().unwrap(),
            failure_sequence: npc["boast"]["failureSequence"].as_u64().unwrap(),
            selected: npc["boast"]["selected"].as_u64().map(|value| (value as u32).into()),
            succeeded: npc["boast"]["succeeded"].as_bool().unwrap(),
        };
        state.npc.help_flags = npc["helpFlags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_bool().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        state.npc.librarian_lace_read = npc["librarianLaceRead"].as_bool().unwrap();
        state
    }
    fn state_snapshot(state: &HubEconomyState) -> Value {
        let equipment = |slot| state.equipment.at(slot).as_ref().map(item_snapshot);
        json!({
            "backpack":state.backpack.iter().map(item_snapshot).collect::<Vec<_>>(),
            "storage":state.storage.iter().map(item_snapshot).collect::<Vec<_>>(),
            "equipment":{"amulet":equipment(EquipmentSlot::Amulet),"hat":equipment(EquipmentSlot::Hat),
                "robe":equipment(EquipmentSlot::Robe),"weapon":equipment(EquipmentSlot::Weapon),
                "rings":[equipment(EquipmentSlot::Ring0),equipment(EquipmentSlot::Ring1),equipment(EquipmentSlot::Ring2)]},
            "fomentiusStock":state.fomentius_stock.iter().map(|stock|json!({"item":item_snapshot(&stock.item),"price":stock.price})).collect::<Vec<_>>(),
            "gold":state.gold,"charmCapacity":state.charm_capacity,"collegeIntroPending":state.college_intro_pending,
            "tutorialPending":state.tutorial_pending,"dowsingFee":state.dowsing_fee,
            "dowsingRolled":state.dowsing_rolled,
            "dowsingOffers":state.dowsing_offers.iter().map(|offer|json!({"id":offer.id,"price":offer.price,"recipeIndex":offer.recipe_index})).collect::<Vec<_>>(),
            "firstMixedSelectors":state.first_mixed_selectors,"hagathaBundleSelectors":state.hagatha_bundle_selectors,
            "nextItemId":state.next_item_id,"nextOfferId":state.next_offer_id,
            "npc":{"boast":{"failed":state.npc.boast.failed,"failureSequence":state.npc.boast.failure_sequence,
                "selected":state.npc.boast.selected_native(),"succeeded":state.npc.boast.succeeded},
                "helpFlags":state.npc.help_flags,"librarianLaceRead":state.npc.librarian_lace_read},
            "ownedPerkSelectors":state.owned_perk_selectors,"revision":state.revision,
            "rng":{"indexA":state.rng.index_a,"indexB":state.rng.index_b,"words":&state.rng.words[..]},
            "tonicPurchases":state.tonic_purchases,"unforgeBonuses":{
                "experience":state.unforge_bonuses.experience,"manaCostReduction":state.unforge_bonuses.mana_cost_reduction,
                "maximumHealth":state.unforge_bonuses.maximum_health,"maximumMana":state.unforge_bonuses.maximum_mana,
                "offensiveDamage":state.unforge_bonuses.offensive_damage,"recipeAttemptCount":state.unforge_bonuses.recipe_attempt_count},
        })
    }
    fn result_snapshot(result: &HubEconomyResult, state: &HubEconomyState) -> Value {
        json!({"accepted":result.accepted,"reason":result.reason.map(HubEconomyRejection::as_str),
            "dyeSession":result.dye_session.as_ref().map(|session|json!({
                "id":session.id,"dyeItemId":session.dye_item_id,"kitConsumed":session.kit_consumed})),
            "dowsingPitch":result.dowsing_pitch,"unforgeOutcome":result.unforge_outcome.as_ref().map(|outcome|json!({
                "amount":outcome.amount,"itemName":outcome.item_name,"kind":outcome.kind.as_str()})),
            "state":state_snapshot(state)})
    }
    fn assert_snapshot(actual: &Value, expected: &Value, path: &str) {
        match (actual, expected) {
            (Value::Number(actual), Value::Number(expected)) => {
                assert_eq!(actual.as_f64(), expected.as_f64(), "{path}")
            }
            (Value::Array(actual), Value::Array(expected)) => {
                assert_eq!(actual.len(), expected.len(), "{path} length");
                for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                    assert_snapshot(actual, expected, &format!("{path}[{index}]"));
                }
            }
            (Value::Object(actual), Value::Object(expected)) => {
                assert_eq!(actual.len(), expected.len(), "{path} fields");
                for (key, expected) in expected {
                    assert_snapshot(&actual[key], expected, &format!("{path}.{key}"));
                }
            }
            _ => assert_eq!(actual, expected, "{path}"),
        }
    }
    fn command(operation: &Value) -> HubInventoryAction {
        let id = |key: &str| operation[key].as_u64().unwrap();
        match operation["type"].as_str().unwrap() {
            "buy-fomentius" => HubInventoryAction::BuyFomentius { item_id: id("itemId") },
            "buy-hagatha" => HubInventoryAction::BuyHagatha {
                selector: operation["selector"].as_i64().unwrap() as i32,
            },
            "remove-hagatha" => {
                HubInventoryAction::RemoveHagatha { selector: id("selector") as u32 }
            }
            "close-hagatha" => HubInventoryAction::CloseHagatha,
            "dowse" => HubInventoryAction::Dowse { reference_item_id: None },
            "buy-dowsing" => HubInventoryAction::BuyDowsing { offer_id: id("offerId") },
            "close-dowsing" => HubInventoryAction::CloseDowsing,
            "move" => HubInventoryAction::Move {
                item_id: id("itemId"),
                destination_sack_id: operation["destinationSackId"].as_u64(),
                destination_slot: operation["destinationSlot"].as_u64().map(|value| value as usize),
            },
            "transfer" => HubInventoryAction::Transfer {
                item_id: id("itemId"),
                gesture: solomon_sim::economy::HubTransferGesture::Drag,
                direction: if operation["direction"] == "to-storage" {
                    TransferDirection::ToStorage
                } else {
                    TransferDirection::ToBackpack
                },
            },
            "consume" => HubInventoryAction::Consume { item_id: id("itemId") },
            "equip" => {
                HubInventoryAction::Equip { item_id: id("itemId"), slot: slot(&operation["slot"]) }
            }
            "unequip" => HubInventoryAction::Unequip {
                slot: slot(&operation["slot"]),
                destination_sack_id: operation["destinationSackId"].as_u64(),
                destination_slot: operation["destinationSlot"].as_u64().map(|slot| slot as usize),
            },
            "equip-sack-contents" => {
                HubInventoryAction::EquipSackContents { item_id: id("itemId") }
            }
            "open-dye" => HubInventoryAction::OpenDye {
                dye_item_id: id("dyeItemId"),
                session_id: operation["sessionId"].as_str().unwrap().into(),
            },
            "dye" => HubInventoryAction::Dye {
                session_id: operation["sessionId"].as_str().unwrap().into(),
                target_item_id: id("targetItemId"),
                layer: if operation["layer"] == "cloth" {
                    NativeDyeLayer::Cloth
                } else {
                    NativeDyeLayer::Trim
                },
                swatch_rows: operation["swatchRows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_u64().unwrap() as usize)
                    .collect(),
            },
            "select-boast" => {
                HubInventoryAction::SelectBoast { boast_id: (id("boastId") as u32).into() }
            }
            "read-librarian-book" => {
                HubInventoryAction::ReadLibrarianBook { book_id: id("bookId") as u32 }
            }
            "buy-teacher-spell" => {
                HubInventoryAction::BuyTeacherSpell { skill_id: id("skillId") as usize }
            }
            "read-skill-book" => HubInventoryAction::ReadSkillBook { item_id: id("itemId") },
            other => panic!("unknown economy operation {other}"),
        }
    }
    let fixture: Value = serde_json::from_str(&fixture("hub-economy-goldens.json")).unwrap();
    for case in fixture["creations"].as_array().unwrap() {
        let element = WizardElement::ALL
            .into_iter()
            .find(|element| Some(element.as_str()) == case["element"].as_str())
            .unwrap();
        let actual = HubEconomyState::new(case["seed"].as_i64().unwrap(), element);
        assert_snapshot(&state_snapshot(&actual), &case["state"], "fresh profile");
    }
    for family in ["serviceCases", "inventoryCases"] {
        for case in fixture[family].as_array().unwrap() {
            let mut actual = read_state(&case["initial"]);
            let mut dye_session = None;
            for (index, record) in case["records"].as_array().unwrap().iter().enumerate() {
                let operation = &record["operation"];
                let mut context = HubEconomyContext {
                    player_level: operation["playerLevel"].as_u64().unwrap_or(1) as u32,
                    creativity_rank: operation["creativityRank"].as_u64().unwrap_or(0) as u32,
                    ..HubEconomyContext::default()
                };
                if operation["alreadyUnlocked"] == true {
                    context.advanced_unlocks.fill(true);
                }
                let result = actual.apply_with_dye_session(
                    &command(operation),
                    &context,
                    None,
                    dye_session.as_ref(),
                );
                if result.accepted && result.dye_session.is_some() {
                    dye_session = result.dye_session.clone();
                }
                assert_snapshot(
                    &result_snapshot(&result, &actual),
                    &record["result"],
                    &format!("{} record {index}", case["name"]),
                );
            }
        }
    }
    for case in fixture["unforgeCases"].as_array().unwrap() {
        let mut actual = read_state(&case["initial"]);
        for (index, record) in case["records"].as_array().unwrap().iter().enumerate() {
            let item = read_item(&record["item"]);
            let action = HubInventoryAction::Unforge { item_id: item.id };
            actual.backpack = vec![item];
            let vitals = &record["vitals"];
            let context = HubEconomyContext {
                vitals: NativeUnforgeVitals {
                    current_health: vitals["currentHealth"].as_f64().unwrap(),
                    current_mana: vitals["currentMana"].as_f64().unwrap(),
                    maximum_health: vitals["maximumHealth"].as_f64().unwrap(),
                    maximum_mana: vitals["maximumMana"].as_f64().unwrap(),
                },
                ..HubEconomyContext::default()
            };
            let result = actual.apply(&action, &context);
            assert_snapshot(
                &result_snapshot(&result, &actual),
                &record["result"],
                &format!("Unforge seed {} record {index}", case["seed"]),
            );
        }
    }
    for case in fixture["dyeCases"].as_array().unwrap() {
        let rows: Vec<_> = case["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(native_dye_mixed_tint(&rows).map(u64::from), case["mixed"].as_u64());
        assert_eq!(native_dye_committed_tint(&rows).map(u64::from), case["committed"].as_u64());
    }
    for case in fixture["archiveCases"].as_array().unwrap() {
        let mut actual = read_state(&case["initial"]);
        let input = &case["archive"];
        let ground_items = items(&input["groundItems"]);
        let element = WizardElement::ALL
            .into_iter()
            .find(|element| Some(element.as_str()) == input["starterElement"].as_str())
            .unwrap();
        actual.archive_completed_run(CompletedRunEconomyArchive {
            display_name: input["displayName"].as_str().unwrap(),
            ground_gold: input["groundGold"].as_u64().unwrap(),
            ground_items: &ground_items,
            starter_element: element,
            transfer_carried_items: input["transferCarriedItems"].as_bool().unwrap(),
        });
        assert_snapshot(
            &state_snapshot(&actual),
            &case["result"],
            &format!("archive {}", case["name"]),
        );
    }
    for case in fixture["lastWordCases"].as_array().unwrap() {
        let mut actual = read_state(&case["initial"]);
        let retained = items(&case["items"]);
        assert_eq!(
            actual.archive_hagatha_last_word_items(&retained, case["name"].as_str().unwrap()),
            case["accepted"].as_bool().unwrap()
        );
        assert_snapshot(&state_snapshot(&actual), &case["result"], "Last Word archive");
    }
}

#[test]
fn every_native_ui_font_and_text_layout_replays_the_website() {
    use serde_json::{Value, json};
    use solomon_sim::native_ui_text::*;

    fn font(value: &Value) -> NativeUiFontName {
        NativeUiFontName::ALL
            .into_iter()
            .find(|font| Some(font.as_str()) == value.as_str())
            .expect("oracle font exists")
    }

    fn compare(actual: &Value, expected: &Value, path: &str) {
        match (actual, expected) {
            (Value::Number(actual), Value::Number(expected)) => {
                assert_eq!(actual.as_f64(), expected.as_f64(), "{path}");
            }
            (Value::Array(actual), Value::Array(expected)) => {
                assert_eq!(actual.len(), expected.len(), "{path} length");
                for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                    compare(actual, expected, &format!("{path}[{index}]"));
                }
            }
            (Value::Object(actual), Value::Object(expected)) => {
                assert_eq!(actual.len(), expected.len(), "{path} fields");
                for (key, expected) in expected {
                    compare(&actual[key], expected, &format!("{path}.{key}"));
                }
            }
            _ => assert_eq!(actual, expected, "{path}"),
        }
    }

    fn layout_record(layout: NativeUiTextLayout) -> Value {
        let mut glyphs = Vec::new();
        for glyph in &layout.glyphs {
            let ink = native_ui_glyph_ink_bounds(glyph);
            glyphs.push(json!({
                "record": glyph.glyph.record, "codePoint": glyph.glyph.code_point,
                "centerX": glyph.center_x, "centerY": glyph.center_y,
                "scale": glyph.scale, "alpha": glyph.alpha, "tint": glyph.tint,
                "italic": glyph.italic,
                "ink": { "left": ink.left, "top": ink.top, "width": ink.width, "height": ink.height },
            }));
        }
        let lines: Vec<_> = layout.lines.iter().map(|line| json!({
            "text": line.text, "x": line.x, "baselineY": line.baseline_y, "width": line.width,
        })).collect();
        json!({
            "font": layout.font.as_str(), "width": layout.width, "height": layout.height,
            "lines": lines, "unsupportedCodePoints": layout.unsupported_code_points, "glyphs": glyphs,
        })
    }

    fn read_runs(value: &Value) -> Vec<NativeUiTextRun<'_>> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|run| NativeUiTextRun {
                text: run["text"].as_str().unwrap(),
                scale: run["scale"].as_f64(),
                advance_scale: run["advanceScale"].as_f64(),
                offset_x: run["offsetX"].as_f64().unwrap_or(0.0),
                offset_y: run["offsetY"].as_f64().unwrap_or(0.0),
                italic: run["italic"].as_bool().unwrap_or(false),
            })
            .collect()
    }

    fn run_record(run: &NativeUiTextRun<'_>) -> Value {
        let mut value = json!({ "text": run.text });
        if let Some(scale) = run.scale {
            value["scale"] = json!(scale);
        }
        if let Some(scale) = run.advance_scale {
            value["advanceScale"] = json!(scale);
        }
        if run.offset_x != 0.0 {
            value["offsetX"] = json!(run.offset_x);
        }
        if run.offset_y != 0.0 {
            value["offsetY"] = json!(run.offset_y);
        }
        if run.italic {
            value["italic"] = json!(true);
        }
        value
    }

    let fixture: Value = serde_json::from_str(&fixture("native-ui-text-goldens.json")).unwrap();
    assert_eq!(fixture["fonts"].as_array().unwrap().len(), NativeUiFontName::ALL.len());
    for record in fixture["fonts"].as_array().unwrap() {
        let name = font(&record["name"]);
        let actual = native_ui_font(name);
        assert_eq!(actual.atlas.as_str(), record["atlas"].as_str().unwrap());
        compare(&json!(actual.atlas.dimensions()), &record["atlasDimensions"], name.as_str());
        compare(&json!(actual.metrics), &record["metrics"], name.as_str());
        assert_eq!(actual.space_advance, record["spaceAdvance"].as_f64().unwrap());
        let glyphs: Vec<_> = actual.glyphs.iter().map(|glyph| json!({
            "codePoint": glyph.code_point, "record": glyph.record, "frame": glyph.frame,
            "logicalSize": glyph.logical_size, "trimOrigin": glyph.trim_origin, "metrics": glyph.metrics,
        })).collect();
        compare(&json!(glyphs), &record["glyphs"], name.as_str());
        let kerning: Vec<_> = actual
            .kerning
            .iter()
            .map(|pair| json!([pair.left, pair.right, pair.adjustment,]))
            .collect();
        compare(&json!(kerning), &record["kerning"], name.as_str());
        for pair in record["kerningWidths"].as_array().unwrap() {
            let left = char::from_u32(pair["left"].as_u64().unwrap() as u32).unwrap();
            let right = char::from_u32(pair["right"].as_u64().unwrap() as u32).unwrap();
            assert_eq!(
                measure_native_ui_text(&format!("{left}{right}"), name, 1.0),
                pair["width"].as_f64().unwrap(),
                "{} {left}{right}",
                name.as_str(),
            );
        }
    }
    for (index, record) in fixture["layouts"].as_array().unwrap().iter().enumerate() {
        let input = &record["spec"];
        let spec = NativeUiTextSpec {
            align: match input["align"].as_str() {
                Some("left") => NativeUiTextAlign::Left,
                Some("right") => NativeUiTextAlign::Right,
                _ => NativeUiTextAlign::Center,
            },
            placement: if input["placement"] == "box" {
                NativeUiTextPlacement::Box
            } else {
                NativeUiTextPlacement::Baseline
            },
            scale: input["scale"].as_f64().unwrap_or(1.0),
            alpha: input["alpha"].as_f64().unwrap_or(1.0),
            tint: input["tint"].as_u64().unwrap_or(0xffffff) as u32,
            max_width: input["maxWidth"].as_f64(),
            line_height: input["lineHeight"].as_f64(),
            ..NativeUiTextSpec::new(
                font(&input["font"]),
                input["text"].as_str().unwrap(),
                input["x"].as_f64().unwrap(),
                input["y"].as_f64().unwrap(),
            )
        };
        compare(
            &layout_record(layout_native_ui_text(&spec)),
            &record["expected"],
            &format!("layout {index}"),
        );
    }
    for (index, record) in fixture["runs"].as_array().unwrap().iter().enumerate() {
        let spec = &record["spec"];
        let layout = layout_native_ui_text_runs(
            font(&spec["font"]),
            &read_runs(&spec["runs"]),
            spec["x"].as_f64().unwrap(),
            spec["y"].as_f64().unwrap(),
            spec["tint"].as_u64().unwrap() as u32,
        );
        compare(&layout_record(layout), &record["expected"], &format!("runs {index}"));
    }
    for (index, record) in fixture["wrappedRuns"].as_array().unwrap().iter().enumerate() {
        let runs = read_runs(&record["runs"]);
        let lines = wrap_native_ui_text_runs(
            &runs,
            font(&record["font"]),
            record["maxWidth"].as_f64().unwrap(),
        );
        let actual: Vec<Vec<_>> =
            lines.iter().map(|line| line.iter().map(run_record).collect()).collect();
        compare(&json!(actual), &record["expected"], &format!("wrapped runs {index}"));
    }
    for record in fixture["msgBoxes"].as_array().unwrap() {
        let text = record["text"].as_str().unwrap();
        assert_eq!(
            json!(wrap_native_ui_msg_box_text(
                text,
                font(&record["font"]),
                record["maxWidth"].as_f64().unwrap()
            )),
            record["expected"],
            "MsgBox {text}",
        );
    }
}

#[test]
fn wizard_names_use_the_source_glyph_admission_and_centered_ink() {
    use solomon_sim::menu::create_name::*;
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture("create-name-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let value = case["value"].as_str().unwrap();
        assert_eq!(
            initial_create_wizard_name(value),
            case["initial"].as_str().unwrap(),
            "initial {value:?}"
        );
        let finalization = finalize_create_wizard_name(value);
        let expected = &case["finalization"];
        assert_eq!(finalization.is_ok(), expected["ok"].as_bool().unwrap(), "finalize {value:?}");
        match finalization {
            Ok(name) => {
                assert_eq!(name, expected["value"].as_str().unwrap(), "final name {value:?}")
            }
            Err(reason) => assert_eq!(
                reason.message(),
                expected["reason"].as_str().unwrap(),
                "final rejection {value:?}"
            ),
        }
        let rejection = validate_create_wizard_name(value);
        assert_eq!(
            rejection.is_none(),
            case["validation"]["ok"].as_bool().unwrap(),
            "admission {value:?}"
        );
        if let Some(reason) = rejection {
            assert_eq!(
                reason.message(),
                case["validation"]["reason"].as_str().unwrap(),
                "reason {value:?}"
            );
            continue;
        }
        let layout = layout_create_wizard_name(value);
        let expected = &case["layout"];
        for (name, actual) in [
            ("left", layout.left),
            ("right", layout.right),
            ("top", layout.top),
            ("width", layout.width),
            ("height", layout.height),
        ] {
            assert_eq!(actual, expected[name].as_f64().unwrap(), "{value:?}.{name}");
        }
        assert_eq!(layout.value, expected["value"].as_str().unwrap());
        let glyphs = expected["glyphs"].as_array().unwrap();
        assert_eq!(layout.glyphs.len(), glyphs.len());
        for (index, (actual, expected)) in layout.glyphs.iter().zip(glyphs).enumerate() {
            assert_eq!(actual.glyph.record, expected["record"].as_u64().unwrap() as usize);
            assert_eq!(
                actual.center_x,
                expected["centerX"].as_f64().unwrap(),
                "{value:?} glyph{index}.x"
            );
            assert_eq!(
                actual.center_y,
                expected["centerY"].as_f64().unwrap(),
                "{value:?} glyph{index}.y"
            );
        }
    }
}

#[test]
fn party_codes_and_directory_actions_match_the_source_join_flow() {
    use solomon_sim::menu::party_directory::*;
    use solomon_sim::social::PartyVisibility;
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture("party-directory-goldens.json")).unwrap();
    for case in fixture["codes"].as_array().unwrap() {
        let value = case["value"].as_str().unwrap();
        assert_eq!(normalize_party_code(value), case["normalized"].as_str().unwrap(), "{value:?}");
        assert_eq!(complete_party_code(value), case["complete"].as_bool().unwrap(), "{value:?}");
    }
    for case in fixture["listings"].as_array().unwrap() {
        let visibility = match case["visibility"].as_str().unwrap() {
            "invite-only" => PartyVisibility::InviteOnly,
            "public" => PartyVisibility::Public,
            _ => panic!("fixture has an unknown party visibility"),
        };
        let action = directory_party_action(visibility, case["status"] == "playing");
        let action_name = match action {
            DirectoryPartyAction::Join => "join",
            DirectoryPartyAction::Request => "request",
            DirectoryPartyAction::Wait => "wait",
        };
        assert_eq!(action_name, case["action"].as_str().unwrap());
        let presentation = directory_party_presentation(
            case["memberCount"].as_u64().unwrap() as usize,
            case["maxMembers"].as_u64().unwrap() as usize,
            case["boneyardName"].as_str(),
        );
        assert_eq!(presentation.location, case["presentation"]["location"].as_str().unwrap());
        assert_eq!(presentation.squad, case["presentation"]["squad"].as_str().unwrap());
        assert_eq!(presentation.status, case["presentation"]["status"].as_str().unwrap());
    }
}

#[test]
fn save_mod_mismatch_keeps_source_identity_matching_and_array_order() {
    use serde_json::json;
    use solomon_sim::menu::save_mods::game_save_mod_mismatch;
    use solomon_sim::modding::identity::GameContentIdentity;
    fn identity(value: &serde_json::Value) -> GameContentIdentity {
        GameContentIdentity {
            content_sha256: value["contentSha256"].as_str().unwrap().to_owned(),
            id: value["id"].as_str().unwrap().to_owned(),
            version: value["version"].as_str().unwrap().to_owned(),
        }
    }
    fn record(value: &GameContentIdentity) -> serde_json::Value {
        json!({"contentSha256":value.content_sha256,"id":value.id,"version":value.version})
    }
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture("save-mod-mismatch-goldens.json")).unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let saved: Vec<_> = case["saved"].as_array().unwrap().iter().map(identity).collect();
        let active: Vec<_> = case["active"].as_array().unwrap().iter().map(identity).collect();
        let actual = game_save_mod_mismatch(&saved, &active).map_or(serde_json::Value::Null, |mismatch| {
            json!({
                "added":mismatch.added.iter().map(record).collect::<Vec<_>>(),
                "removed":mismatch.removed.iter().map(record).collect::<Vec<_>>(),
                "changed":mismatch.changed.iter().map(|entry| json!({"active":record(&entry.active),"saved":record(&entry.saved)})).collect::<Vec<_>>(),
            })
        });
        assert_eq!(actual, case["expected"], "save mod mismatch {index}");
    }
}

#[test]
fn hosted_social_goldens() {
    use solomon_sim::social::{
        GameChatWorldKind, GameSessionKind, native_player_card_college_invite_available,
    };
    let recorded: serde_json::Value =
        serde_json::from_str(&fixture("hosted-social-goldens.json")).unwrap();
    for case in recorded["invites"].as_array().unwrap() {
        let session = match case["sessionKind"].as_str().unwrap() {
            "global-hub" => GameSessionKind::GlobalHub,
            "private-college" => GameSessionKind::PrivateCollege,
            "standalone" => GameSessionKind::Standalone,
            _ => panic!("unknown fixture session kind"),
        };
        let world = if case["worldKind"] == "hub" {
            GameChatWorldKind::Hub
        } else {
            GameChatWorldKind::Boneyard
        };
        let activity = if case["activity"] == "hub" {
            GameChatWorldKind::Hub
        } else {
            GameChatWorldKind::Boneyard
        };
        assert_eq!(
            native_player_card_college_invite_available(
                session,
                world,
                case["leader"].as_bool().unwrap(),
                activity
            ),
            case["expected"].as_bool().unwrap(),
            "{case}"
        );
    }
}

#[test]
fn cheat_menu_goldens() {
    use serde_json::Value;
    use solomon_sim::menu::cheats::*;
    use solomon_sim::modding::value::{LuaObject, LuaValue};
    use solomon_sim::profile::{WizardDiscipline, WizardElement};

    fn lua(value: &Value) -> LuaValue {
        match value {
            Value::Null => LuaValue::Null,
            Value::Bool(value) => LuaValue::Boolean(*value),
            Value::Number(value) => LuaValue::Number(value.as_f64().unwrap()),
            Value::String(value) => LuaValue::String(value.clone()),
            Value::Array(values) => LuaValue::Array(values.iter().map(lua).collect()),
            Value::Object(values) => {
                let mut object = LuaObject::new();
                for (key, value) in values {
                    object.insert(key.clone(), lua(value));
                }
                LuaValue::Object(object)
            }
        }
    }
    let recorded: Value = serde_json::from_str(&fixture("cheat-menu-goldens.json")).unwrap();
    assert_eq!(CHEAT_MENU_CATALOG_QUERY, recorded["catalogQuery"].as_str().unwrap());
    for case in recorded["authority"].as_array().unwrap() {
        assert_eq!(
            game_cheat_menu_available(
                case["developerAccess"].as_bool().unwrap(),
                case["cheatsEnabled"].as_bool().unwrap(),
                case["isHost"].as_bool().unwrap()
            ),
            case["expected"].as_bool().unwrap()
        );
    }
    for case in recorded["commands"].as_array().unwrap() {
        let input = &case["action"];
        let text = |key: &str| input[key].as_str().unwrap().to_owned();
        let number = |key: &str| input[key].as_f64().unwrap_or(f64::NAN);
        let action = match input["kind"].as_str().unwrap() {
            "restore-health" => CheatMenuAction::RestoreHealth { player_id: text("playerId") },
            "restore-mana" => CheatMenuAction::RestoreMana { player_id: text("playerId") },
            "set-gold" => {
                CheatMenuAction::SetGold { gold: number("gold"), player_id: text("playerId") }
            }
            "grant-experience" => CheatMenuAction::GrantExperience {
                amount: number("amount"),
                player_id: text("playerId"),
            },
            "set-run-seed" => CheatMenuAction::SetRunSeed { seed: number("seed") },
            "spawn-next-wave" => CheatMenuAction::SpawnNextWave,
            "spawn-enemy" => CheatMenuAction::SpawnEnemy {
                count: number("count"),
                enemy_key: text("enemyKey"),
                player_id: text("playerId"),
            },
            "grant-item" => CheatMenuAction::GrantItem {
                item_key: text("itemKey"),
                player_id: text("playerId"),
                quantity: number("quantity"),
            },
            "grant-skill" => CheatMenuAction::GrantSkill {
                player_id: text("playerId"),
                ranks: number("ranks"),
                skill_id: number("skillId"),
            },
            "grant-weld" => CheatMenuAction::GrantWeld {
                build_id: number("buildId"),
                player_id: text("playerId"),
            },
            "summon-bot" => CheatMenuAction::SummonBot {
                discipline: WizardDiscipline::ALL
                    .into_iter()
                    .find(|value| value.as_str() == text("discipline"))
                    .unwrap(),
                element: WizardElement::ALL
                    .into_iter()
                    .find(|value| value.as_str() == text("element"))
                    .unwrap(),
            },
            _ => panic!("unknown cheat fixture action"),
        };
        let result = compile_cheat_menu_action(&action);
        if case["expected"]["ok"] == true {
            assert_eq!(result.unwrap(), case["expected"]["value"].as_str().unwrap(), "{input}");
        } else {
            assert_eq!(result.unwrap_err(), case["expected"]["error"].as_str().unwrap(), "{input}");
        }
    }
    for case in recorded["catalogs"].as_array().unwrap() {
        let values = case["values"].as_array().unwrap().iter().map(lua).collect::<Vec<_>>();
        let result = decode_cheat_menu_catalogs(&values);
        let expected = &case["expected"];
        if expected["ok"] != true {
            assert_eq!(result.unwrap_err(), expected["error"].as_str().unwrap());
            continue;
        }
        let actual = result.unwrap();
        let expected = &expected["value"];
        assert_eq!(actual.enemies.len(), expected["enemies"].as_array().unwrap().len());
        for (row, expected) in actual.enemies.iter().zip(expected["enemies"].as_array().unwrap()) {
            assert_eq!(row.base, expected["base"].as_str().unwrap());
            assert_eq!(row.key, expected["key"].as_str().unwrap());
            assert_eq!(row.native_type_id, expected["nativeTypeId"].as_i64().unwrap());
        }
        assert_eq!(actual.items.len(), expected["items"].as_array().unwrap().len());
        for (row, expected) in actual.items.iter().zip(expected["items"].as_array().unwrap()) {
            assert_eq!(row.key, expected["key"].as_str().unwrap());
            assert_eq!(row.kind, expected["kind"].as_str().unwrap());
            assert_eq!(row.name, expected["name"].as_str().unwrap());
            assert_eq!(row.native_type_id, expected["nativeTypeId"].as_i64().unwrap());
        }
        assert_eq!(actual.skills.len(), expected["skills"].as_array().unwrap().len());
        for (row, expected) in actual.skills.iter().zip(expected["skills"].as_array().unwrap()) {
            assert_eq!(row.family, expected["family"].as_str().unwrap());
            assert_eq!(row.id, expected["id"].as_i64().unwrap());
            assert_eq!(row.maximum_rank, expected["maximumRank"].as_i64().unwrap());
            assert_eq!(row.name, expected["name"].as_str().unwrap());
            assert_eq!(row.weld_only, expected["weldOnly"].as_bool().unwrap());
        }
        assert_eq!(actual.welds.len(), expected["welds"].as_array().unwrap().len());
        for (row, expected) in actual.welds.iter().zip(expected["welds"].as_array().unwrap()) {
            assert_eq!(row.id, expected["id"].as_i64().unwrap());
            assert_eq!(row.name, expected["name"].as_str().unwrap());
            assert_eq!(
                row.component_skill_ids,
                expected["componentSkillIds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap())
                    .collect::<Vec<_>>()
            );
        }
    }
    let mut history = Vec::new();
    for case in recorded["history"].as_array().unwrap() {
        append_cheat_console_history(&mut history, case["code"].as_str().unwrap());
        assert_eq!(
            history,
            case["expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn party_ui_goldens() {
    use serde_json::{Value, json};
    use solomon_sim::hud::HudRect;
    use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign};
    use solomon_sim::social::PartyVisibility;
    use solomon_sim::social::party_ui::{
        NATIVE_PARTY_INVITATION_FRAME, NATIVE_PARTY_INVITATION_TITLE, NativePartyMenuMember,
        NativePartyMenuRequest, NativePartyMenuSpec, NativePartyMenuTab, NativePartyMenuTag,
        fit_native_party_menu_text, native_party_invitation_action_bounds,
        native_party_invitation_body, native_party_menu_name_font, plan_native_party_menu,
    };

    fn rect(bounds: HudRect) -> Value {
        json!([bounds.x, bounds.y, bounds.width, bounds.height])
    }
    fn visibility(value: &str) -> PartyVisibility {
        match value {
            "public" => PartyVisibility::Public,
            "private" => PartyVisibility::Private,
            "invite-only" => PartyVisibility::InviteOnly,
            _ => panic!("unknown source Party visibility"),
        }
    }
    fn javascript_numbers(value: &mut Value) {
        match value {
            // JSON stores integer and float spellings separately; Source numbers
            // are all doubles. Normalize representation, preserving exact values.
            Value::Number(number) => *value = json!(number.as_f64().unwrap()),
            Value::Array(values) => values.iter_mut().for_each(javascript_numbers),
            Value::Object(values) => values.values_mut().for_each(javascript_numbers),
            _ => {}
        }
    }
    let recorded: Value =
        serde_json::from_str(include_str!("fixtures/website/party-ui-goldens.json")).unwrap();
    let invitation = &recorded["invitation"];
    let mut actual = json!({
        "frame": rect(NATIVE_PARTY_INVITATION_FRAME),
        "title": NATIVE_PARTY_INVITATION_TITLE,
        "actions": native_party_invitation_action_bounds().map(rect),
    });
    let mut expected = json!({
        "frame": invitation["frame"],
        "title": invitation["title"],
        "actions": invitation["actions"],
    });
    javascript_numbers(&mut actual);
    javascript_numbers(&mut expected);
    assert_eq!(actual, expected, "Party invitation geometry and title");
    for body in invitation["bodies"].as_array().unwrap() {
        assert_eq!(
            native_party_invitation_body(body["inviter"].as_str().unwrap()),
            body["expected"].as_str().unwrap()
        );
    }
    for (index, case) in recorded["cases"].as_array().unwrap().iter().enumerate() {
        let input = &case["input"];
        let members = input["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let tags = row["tags"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|tag| match tag.as_str().unwrap() {
                        "you" => NativePartyMenuTag::You,
                        "leader" => NativePartyMenuTag::Leader,
                        "offline" => NativePartyMenuTag::Offline,
                        _ => panic!("unknown source Party tag"),
                    })
                    .collect();
                NativePartyMenuMember {
                    id: row["id"].as_str().unwrap().to_owned(),
                    name: row["name"].as_str().unwrap().to_owned(),
                    removable: row["removable"].as_bool().unwrap(),
                    tags,
                }
            })
            .collect::<Vec<_>>();
        let requests = input["requests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| NativePartyMenuRequest {
                id: row["id"].as_str().unwrap().to_owned(),
                name: row["name"].as_str().unwrap().to_owned(),
            })
            .collect::<Vec<_>>();
        let options = input["visibilityOptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| visibility(option["id"].as_str().unwrap()))
            .collect::<Vec<_>>();
        let tab = NativePartyMenuTab::ALL
            .into_iter()
            .find(|tab| tab.id() == input["selectedTab"].as_str().unwrap())
            .unwrap();
        let plan = plan_native_party_menu(&NativePartyMenuSpec {
            code: input["code"].as_str(),
            copied: input["copyLabel"] == "COPIED",
            error: input["error"].as_str(),
            leader: input["leader"].as_bool().unwrap(),
            leave_label: input["leaveLabel"].as_str(),
            members: &members,
            requests: &requests,
            scroll_y: input["scrollY"].as_f64().unwrap_or(0.0),
            selected_tab: tab,
            visibility: visibility(input["visibility"].as_str().unwrap()),
            visibility_options: &options,
        });
        let rows = plan.rows.iter().map(|row| json!({"bounds":rect(row.bounds),"id":row.id,"visibleBounds":row.visible_bounds.map(rect)})).collect::<Vec<_>>();
        let row_actions = plan.row_actions.iter().map(|action| json!({"bounds":rect(action.bounds),"id":action.id,"label":action.label,"rowId":action.row_id})).collect::<Vec<_>>();
        let actions = plan.actions.iter().map(|action| json!({"bounds":rect(action.bounds),"disabled":action.disabled,"id":action.id})).collect::<Vec<_>>();
        let texts = plan.texts.iter().map(|text| json!({
            "label":text.label,"text":text.text,"font":text.font.as_str(),
            "align":match text.align {NativeUiTextAlign::Left=>"left", NativeUiTextAlign::Center=>"center", NativeUiTextAlign::Right=>"right"},
            "scale":text.scale,"tint":text.tint,"x":text.x,"y":text.y,"clipped":text.clipped,
        })).collect::<Vec<_>>();
        let mut actual = json!({"contentHeight":plan.content_height,"maximumScrollY":plan.maximum_scroll_y,"scrollY":plan.scroll_y,"rows":rows,"rowActions":row_actions,"actions":actions,"texts":texts});
        let mut expected = case["expected"].clone();
        javascript_numbers(&mut actual);
        javascript_numbers(&mut expected);
        assert_eq!(actual, expected, "Party plan case {index} ({})", tab.id());
    }
    for case in recorded["fits"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let font = NativeUiFontName::ALL
            .into_iter()
            .find(|font| font.as_str() == case["font"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            fit_native_party_menu_text(text, font, case["maximum"].as_f64().unwrap(), 1.0),
            case["expected"].as_str().unwrap()
        );
        assert_eq!(native_party_menu_name_font(text).as_str(), case["nameFont"].as_str().unwrap());
    }
}
