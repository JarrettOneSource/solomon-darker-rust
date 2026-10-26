//! Courtyard students and trader presentation, ported from Website
//! `core-server/hub-students.ts`, `hub-world.ts`, `hub-skorcha.ts`, and
//! `hub-presentation.ts`. `hub-npc-goldens.json` pins the population and its actor contacts.
//!
//! The pinned Website uses an independent xorshift stream for students, and the native
//! generator for trader clocks. Keeping those owners separate preserves its draw order.

mod physics;
mod storage;
pub use storage::{StudentActors, StudentMut, StudentRead, StudentRef};
mod splines;
mod traders;

pub use splines::{HUB_STUDENT_SPLINES, HubStudentSplines};
pub use traders::{
    DEFAULT_HUB_TRADER_ANIMATION_SEED, HUB_SKORCHA_PLACEMENTS, HubCommonTraderClock,
    HubHagathaClock, HubHagathaParticle, HubPotionTraderClock, HubSkorcha, HubSkorchaSchedule,
};

use crate::Vec2;
use crate::float;
use crate::float::hypot;
use crate::heading::{heading_degrees_from_vector, heading_index};
use crate::hub::painter::HUB_FIRST_PLAYER_PAINTER_ORDINAL;
use crate::hub::{HubWizard, HubWizardPlan};
use crate::painter::{WorldManagerLane, WorldPainterOrder};

use physics::{ActorBody, resolve_actor_motion};

/// Native admission ticker, independent of the rendering frame rate.
pub const HUB_STUDENT_SPAWN_REQUEST_TICKS: u64 = 35;
/// Student doorway collision membership refreshes once per fifteen simulation ticks.
pub const HUB_STUDENT_STATIC_COLLISION_REFRESH_TICKS: u64 = 15;
const PRESENTATION_WARMUP_TICKS: u64 = 900;
const STUDENT_SEED: u32 = 0x51d0_7e57;

/// Courtyard NPC world state. Traders and students advance once per fixed simulation tick.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubNpcState {
    pub students: HubStudentPopulation,
    pub hagatha: HubHagathaClock,
    pub luthacus: HubCommonTraderClock,
    pub potion: HubPotionTraderClock,
    pub skorcha: HubSkorchaSchedule,
}

impl Default for HubNpcState {
    fn default() -> Self {
        Self::new(DEFAULT_HUB_TRADER_ANIMATION_SEED, None)
    }
}

impl HubNpcState {
    #[must_use]
    pub fn new(seed: u32, painter_order: Option<&mut WorldPainterOrder>) -> Self {
        Self::with_students(seed, HubStudentPopulation::new(painter_order))
    }

    /// Save restoration constructs the Hub with a live painter allocator. Births that
    /// retire during its warmup still consume registrations (`restoreGameSaveDocument`).
    #[must_use]
    pub fn with_live_student_painters(seed: u32, painter_order: &mut WorldPainterOrder) -> Self {
        Self::with_students(seed, HubStudentPopulation::with_live_painters(painter_order))
    }

    fn with_students(seed: u32, students: HubStudentPopulation) -> Self {
        Self {
            students,
            hagatha: HubHagathaClock::new(seed ^ 5001),
            luthacus: HubCommonTraderClock::new(seed ^ 5005),
            potion: HubPotionTraderClock::new(),
            skorcha: HubSkorchaSchedule::new(seed),
        }
    }

    /// Resolves the courtyard actor epoch and advances the trader-owned animation clocks.
    pub fn step(
        &mut self,
        wizard: Option<(&mut HubWizard, &HubWizardPlan, bool)>,
        collision_rng: &mut u32,
        painter_order: Option<&mut WorldPainterOrder>,
    ) {
        self.step_with_onboarding(wizard, false, collision_rng, painter_order);
    }

    /// College admission bypasses only student/player pairs; trader roots still collide.
    pub fn step_with_onboarding(
        &mut self,
        wizard: Option<(&mut HubWizard, &HubWizardPlan, bool)>,
        onboarding: bool,
        collision_rng: &mut u32,
        painter_order: Option<&mut WorldPainterOrder>,
    ) {
        self.skorcha.step();
        self.students.step_with_onboarding(
            wizard,
            onboarding,
            collision_rng,
            self.skorcha.skorcha.as_ref().map(|skorcha| skorcha.position),
            painter_order,
        );
        self.hagatha.step();
        self.luthacus.step();
        self.potion.step();
    }
}

/// One independently tinted carried object from College records 165..188.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubStudentProp {
    pub angle: f64,
    pub palette_index: usize,
    pub radius: f64,
}

/// The stock PlayerActor circle and push coefficients of one student.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubStudentPhysicalProfile {
    pub push_resistance: f64,
    pub push_strength: f64,
    pub radius: f64,
}

/// One saved or constructed student. Runtime ownership separates its body and route.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize), serde(from = "StudentRecord"))]
pub struct HubStudentState {
    pub body: StudentBody,
    pub controller: StudentRoute,
}

#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct StudentBody {
    pub current_speed: f64,
    pub frame_phase: f64,
    pub gait_degrees: f64,
    pub heading: f64,
    pub heading_index: usize,
    pub id: usize,
    pub registration_ordinal: usize,
    pub position: Vec2,
    pub profile: HubStudentPhysicalProfile,
    pub props: Vec<HubStudentProp>,
    pub reading: bool,
    pub retired: bool,
    pub scale: f64,
    pub static_collision_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
pub struct StudentRoute {
    pub desired_speed: f64,
    pub path_cursor: f64,
    pub path_id: usize,
    pub path_step: i32,
    pub rng_state: u32,
    pub tick: u64,
    pub wander: Vec2,
}

// The importer needs the original flat field names and typed visitor.
#[cfg(feature = "persistence")]
#[derive(serde::Deserialize)]
#[serde(rename = "HubStudentState")]
struct StudentRecord {
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
    registration_ordinal: usize,
    position: Vec2,
    profile: HubStudentPhysicalProfile,
    props: Vec<HubStudentProp>,
    reading: bool,
    retired: bool,
    rng_state: u32,
    scale: f64,
    static_collision_enabled: bool,
    tick: u64,
    wander: Vec2,
}
#[cfg(feature = "persistence")]
impl From<StudentRecord> for HubStudentState {
    fn from(row: StudentRecord) -> Self {
        Self {
            body: StudentBody {
                current_speed: row.current_speed,
                frame_phase: row.frame_phase,
                gait_degrees: row.gait_degrees,
                heading: row.heading,
                heading_index: row.heading_index,
                id: row.id,
                registration_ordinal: row.registration_ordinal,
                position: row.position,
                profile: row.profile,
                props: row.props,
                reading: row.reading,
                retired: row.retired,
                scale: row.scale,
                static_collision_enabled: row.static_collision_enabled,
            },
            controller: StudentRoute {
                desired_speed: row.desired_speed,
                path_cursor: row.path_cursor,
                path_id: row.path_id,
                path_step: row.path_step,
                rng_state: row.rng_state,
                tick: row.tick,
                wander: row.wander,
            },
        }
    }
}

impl HubStudentState {
    /// `createHubStudentState`: draws the complete native appearance and physical profile.
    #[must_use]
    pub fn new(
        splines: &HubStudentSplines,
        id: usize,
        path_id: usize,
        source_state: u32,
        desired_speed: f64,
        registration_ordinal: usize,
    ) -> Self {
        let mut rng = source_state;
        let strength = random_float(&mut rng, 5.0);
        let radius = random_float(&mut rng, 5.0);
        let gait = random_float(&mut rng, 360.0);
        let frame = random_float(&mut rng, 4.0);
        // Native construction consumes this draw before the spline supplies the heading.
        let _heading = random_float(&mut rng, 360.0);
        let scale = random_float(&mut rng, 0.35);
        let reading = random_integer(&mut rng, 3);
        let count = random_integer(&mut rng, 3) + 2;
        let mut props = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let palette_index = random_integer(&mut rng, 5) as usize;
            let radius = random_signed_float(&mut rng, 2.0);
            let angle = random_float(&mut rng, 45.0) + 45.0;
            props.push(HubStudentProp { angle, palette_index, radius });
        }
        let position = splines.evaluate(path_id, 0.0);
        let next = splines.evaluate(path_id, 0.01);
        let heading = heading_degrees_from_vector(next.x - position.x, next.y - position.y);
        Self {
            body: StudentBody {
                current_speed: desired_speed,
                frame_phase: frame,
                gait_degrees: gait,
                heading,
                heading_index: heading_index(heading),
                id,
                registration_ordinal,
                position,
                profile: HubStudentPhysicalProfile {
                    push_resistance: 1.0,
                    push_strength: 11.0 + strength,
                    radius: 12.0 + radius,
                },
                props,
                reading: reading == 1,
                retired: false,
                scale: 0.75 + scale,
                static_collision_enabled: false,
            },
            controller: StudentRoute {
                desired_speed,
                path_cursor: 0.0,
                path_id,
                path_step: 1,
                rng_state: rng,
                tick: 0,
                wander: Vec2::ZERO,
            },
        }
    }

    /// `commitHubStudentRoute`: animation advances by the resolved displacement.
    pub fn commit(&mut self, position: Vec2) {
        let distance = hypot(position.x - self.position.x, position.y - self.position.y);
        self.frame_phase = (self.frame_phase + distance * 0.2) % 5.0;
        self.gait_degrees = (self.gait_degrees + distance * 6.0) % 360.0;
        self.heading_index = heading_index(self.heading);
        self.position = position;
    }
}

/// One student route tick before world collision resolves the requested displacement.
#[derive(Clone, Debug, PartialEq)]
pub struct HubStudentRoutePlan {
    pub delta: Vec2,
    pub state: HubStudentState,
}

/// `hubStudentStaticCollisionEnabled`: courtyard inset minus the four entry doorways.
#[must_use]
pub fn hub_student_static_collision_enabled(position: Vec2) -> bool {
    let contains = |[x, y, width, height]: [f64; 4]| {
        position.x >= x && position.x <= x + width && position.y >= y && position.y <= y + height
    };
    contains([40.0, 40.0, 1920.0, 1020.0])
        && ![
            [752.0, 134.0, 44.0, 45.0],
            [584.0, 34.0, 121.0, 67.0],
            [1288.0, 80.0, 179.0, 148.0],
            [1771.0, -11.0, 309.0, 255.0],
        ]
        .into_iter()
        .any(contains)
}

/// `planHubStudentRoute`: native cursor pursuit, crowd following, turning and wander draws.
#[must_use]
pub fn plan_hub_student_route<'a>(
    splines: &HubStudentSplines,
    source: &impl StudentRead,
    students: impl IntoIterator<Item = StudentRef<'a>>,
) -> HubStudentRoutePlan {
    let mut state = source.snapshot();
    if source.retired {
        return HubStudentRoutePlan { delta: Vec2::ZERO, state };
    }
    if source.controller().tick.is_multiple_of(HUB_STUDENT_STATIC_COLLISION_REFRESH_TICKS) {
        state.static_collision_enabled = hub_student_static_collision_enabled(source.position);
    }
    let extent = (HUB_STUDENT_SPLINES[source.controller().path_id].len() - 1) as f64;
    if source.controller().path_cursor < 0.0 || source.controller().path_cursor >= extent {
        state.retired = true;
        return HubStudentRoutePlan { delta: Vec2::ZERO, state };
    }
    if random_integer(&mut state.controller.rng_state, 50) == 3 {
        let magnitude = random_float(
            &mut state.controller.rng_state,
            if source.controller().desired_speed > 1.0 { 30.0 } else { 20.0 },
        );
        let radians =
            random_float(&mut state.controller.rng_state, 360.0) * std::f64::consts::PI / 180.0;
        state.controller.wander =
            Vec2::new(float::sin(radians) * magnitude, -float::cos(radians) * magnitude);
    }
    let wander = state.controller.wander;
    let offset_at = |cursor| {
        let target = splines.evaluate(source.controller().path_id, cursor);
        Vec2::new(target.x + wander.x - source.position.x, target.y + wander.y - source.position.y)
    };
    let mut offset = offset_at(state.controller.path_cursor);
    let mut distance = hypot(offset.x, offset.y);
    state.profile.push_resistance = distance / 5.5;
    state.controller.tick += 1;
    if distance <= source.profile.radius * 2.0 {
        state.controller.path_cursor += f64::from(source.controller().path_step) * 0.1;
        if state.controller.path_cursor < 0.0 || state.controller.path_cursor >= extent {
            state.retired = true;
            return HubStudentRoutePlan { delta: Vec2::ZERO, state };
        }
        offset = offset_at(state.controller.path_cursor);
        distance = hypot(offset.x, offset.y);
    }
    let target_heading = if distance > f64::EPSILON {
        heading_degrees_from_vector(offset.x, offset.y)
    } else {
        source.heading
    };
    let delta = signed_angle_delta(source.heading, target_heading);
    let turn_step = if source.current_speed > 1.0 { 4.5 } else { 1.5 };
    state.heading = if delta.abs() <= turn_step {
        (target_heading + 360.0) % 360.0
    } else {
        (source.heading + delta.signum() * turn_step + 360.0) % 360.0
    };
    state.heading_index = heading_index(state.heading);
    let cap_sample = random_float(&mut state.controller.rng_state, 0.25);
    let travel = distance
        .min((1.0 + cap_sample) * source.current_speed * student_speed_factor(source, students));
    state.current_speed = if source.current_speed < source.controller().desired_speed {
        source.controller().desired_speed.min(source.current_speed + 0.01)
    } else {
        source.controller().desired_speed.max(source.current_speed - 0.01)
    };
    let radians = state.heading * std::f64::consts::PI / 180.0;
    HubStudentRoutePlan {
        delta: Vec2::new(float::sin(radians) * travel, -float::cos(radians) * travel),
        state,
    }
}

/// The live native lifecycle, with admission state separate from each student's wander RNG.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubStudentPopulation {
    pub next_id: usize,
    pub rare_path_denominator: u32,
    pub rng_state: u32,
    pub spawning_enabled: bool,
    pub spawn_request_pending: bool,
    pub spawn_ticker_counter: u64,
    pub students: StudentActors,
    next_registration_ordinal: usize,
    #[cfg_attr(feature = "persistence", serde(skip))]
    splines: HubStudentSplines,
}

impl Default for HubStudentPopulation {
    fn default() -> Self {
        Self::new(None)
    }
}

impl HubStudentPopulation {
    #[must_use]
    pub fn with_live_painters(painter_order: &mut WorldPainterOrder) -> Self {
        let mut population = Self::cold(STUDENT_SEED, painter_order.next_actor_ordinal);
        for _ in 0..PRESENTATION_WARMUP_TICKS {
            population.step_routes(Some(painter_order));
        }
        population
    }

    /// `createHubStudentPopulation`: advance the already-live courtyard by 900 warmup ticks.
    #[must_use]
    pub fn new(painter_order: Option<&mut WorldPainterOrder>) -> Self {
        let first_ordinal = painter_order
            .as_ref()
            .map_or(HUB_FIRST_PLAYER_PAINTER_ORDINAL + 1, |order| order.next_actor_ordinal);
        let mut population = Self::cold(STUDENT_SEED, first_ordinal);
        for _ in 0..PRESENTATION_WARMUP_TICKS {
            population.step_routes(None);
        }
        if let Some(order) = painter_order {
            // The source warms its population before the live world manager exists.
            // Only survivors enroll after the players; retired warmup births use no slots.
            for mut student in population.students.iter_mut() {
                student.registration_ordinal =
                    order.register(WorldManagerLane::Actor).registration_ordinal;
            }
            population.next_registration_ordinal = order.next_actor_ordinal;
        }
        population
    }

    #[must_use]
    pub fn cold(seed: u32, first_registration_ordinal: usize) -> Self {
        Self {
            next_id: 0,
            rare_path_denominator: 20,
            rng_state: seed,
            spawning_enabled: true,
            spawn_request_pending: true,
            spawn_ticker_counter: 0,
            students: StudentActors::default(),
            next_registration_ordinal: first_registration_ordinal,
            splines: HubStudentSplines::new(),
        }
    }

    /// Free route advancement used by the source's pre-presentation warmup and oracle.
    pub fn step_routes(&mut self, painter_order: Option<&mut WorldPainterOrder>) {
        let mut students = Vec::with_capacity(self.students.len());
        for student in self.students.iter() {
            let plan = plan_hub_student_route(&self.splines, &student, self.students.iter());
            let mut state = plan.state;
            let position =
                Vec2::new(state.position.x + plan.delta.x, state.position.y + plan.delta.y);
            state.commit(position);
            students.push(state);
        }
        self.admit(students, painter_order);
    }

    /// All routes sample the preceding population before any actor commits motion.
    #[must_use]
    pub fn plan_routes(&self) -> Vec<HubStudentRoutePlan> {
        self.students
            .iter()
            .map(|student| plan_hub_student_route(&self.splines, &student, self.students.iter()))
            .collect()
    }

    /// `stepHubWorldTick`'s courtyard actor epoch. A courtyard wizard participates even
    /// during portal walks; `static_collision_enabled` alone controls the doorway bypass.
    /// The caller advances its region transition after this commits the resolved plan.
    pub fn step(
        &mut self,
        wizard: Option<(&mut HubWizard, &HubWizardPlan, bool)>,
        collision_rng: &mut u32,
        skorcha: Option<Vec2>,
        painter_order: Option<&mut WorldPainterOrder>,
    ) {
        self.step_with_onboarding(wizard, false, collision_rng, skorcha, painter_order);
    }

    pub fn step_with_onboarding(
        &mut self,
        wizard: Option<(&mut HubWizard, &HubWizardPlan, bool)>,
        onboarding: bool,
        collision_rng: &mut u32,
        skorcha: Option<Vec2>,
        painter_order: Option<&mut WorldPainterOrder>,
    ) {
        let plans = self.plan_routes();
        let mut bodies = Vec::with_capacity(plans.len() + 7);
        if let Some((wizard, plan, static_collision)) = &wizard {
            bodies.push(ActorBody::wizard(wizard.position, plan.delta, *static_collision));
        }
        let student_start = bodies.len();
        for plan in &plans {
            let mut body = ActorBody::student(&plan.state, plan.delta);
            body.player_collision_enabled = !onboarding || student_start == 0;
            bodies.push(body);
        }
        // The five fixed Courtyard bodies occur in this exact hub-world.ts order.
        for (position, radius) in [
            (Vec2::new(1340.0, 280.0), 15.0),
            (Vec2::new(1397.0, 664.0), 30.0),
            (Vec2::new(895.5, 455.5), 8.0),
            (Vec2::new(1700.5, 449.5), 25.0),
            (Vec2::new(576.5, 710.5), 25.0),
        ] {
            bodies.push(ActorBody::fixed(position, radius));
        }
        if let Some(position) = skorcha {
            bodies.push(ActorBody::fixed(position, 10.0));
        }
        resolve_actor_motion(&mut bodies, collision_rng);
        if let Some((wizard, plan, _)) = wizard {
            wizard.commit(plan, bodies[0].position);
        }
        let mut students = Vec::with_capacity(plans.len());
        for (plan, body) in plans.into_iter().zip(&bodies[student_start..]) {
            let mut state = plan.state;
            state.commit(body.position);
            students.push(state);
        }
        self.admit(students, painter_order);
    }

    pub(crate) fn admit(
        &mut self,
        students: Vec<HubStudentState>,
        mut painter_order: Option<&mut WorldPainterOrder>,
    ) {
        self.spawn_ticker_counter += 1;
        if self.spawn_ticker_counter >= HUB_STUDENT_SPAWN_REQUEST_TICKS {
            self.spawn_request_pending = true;
            self.spawn_ticker_counter = 0;
        }
        assert_eq!(students.len(), self.students.len(), "route plans cover the current population");
        for (mut stored, planned) in self.students.iter_mut().zip(students) {
            assert_eq!(stored.id, planned.id, "route plans retain admission order");
            stored.replace(planned);
        }
        self.students.retain(|student| !student.retired);
        if !self.spawning_enabled || !self.spawn_request_pending {
            return;
        }
        self.spawn_request_pending = false;
        let count = self.students.len();
        let divisor = if count < 9 {
            2
        } else if count > 25 {
            60
        } else if count > 17 {
            30
        } else if count > 12 {
            15
        } else {
            7
        };
        let spawn_roll = random_integer(&mut self.rng_state, 2.max(divisor / 2));
        if spawn_roll != 1 && count >= 5 {
            return;
        }
        let count_roll = random_integer(&mut self.rng_state, 8);
        let speed_sample = random_signed_float(&mut self.rng_state, 0.1);
        let path_roll = random_integer(&mut self.rng_state, 19);
        if path_roll == 0 {
            return;
        }
        let rare_path = random_integer(&mut self.rng_state, self.rare_path_denominator) == 3;
        if rare_path {
            self.rare_path_denominator += 10;
        }
        let count = if !rare_path && count_roll == 1 { 2 } else { 1 };
        let desired_speed = if rare_path { 2.0 } else { (0.5 - speed_sample) * 1.5 };
        for _ in 0..count {
            let registration_ordinal = match painter_order.as_deref_mut() {
                Some(order) => order.register(WorldManagerLane::Actor).registration_ordinal,
                None => self.next_registration_ordinal,
            };
            let student = HubStudentState::new(
                &self.splines,
                self.next_id,
                (path_roll - 1) as usize,
                self.rng_state,
                desired_speed,
                registration_ordinal,
            );
            self.next_id += 1;
            self.next_registration_ordinal = registration_ordinal + 1;
            self.rng_state = student.controller.rng_state;
            self.students.push(student);
        }
    }
}

/// `hubStudentPropOffset`: independent radial and stack offsets in continuous heading.
#[must_use]
pub fn hub_student_prop_offset(heading: f64, prop: HubStudentProp, index: usize) -> Vec2 {
    let angle = (heading + prop.angle) * std::f64::consts::PI / 180.0;
    Vec2::new(
        prop.radius * float::sin(angle),
        -prop.radius * float::cos(angle) * 2.0 - index as f64 * 3.0,
    )
}

/// `hubStudentHeadOffset`: unscaled head pass, including small-student registration.
#[must_use]
pub fn hub_student_head_offset(student: &impl StudentRead) -> Vec2 {
    let gait = student.gait_degrees * std::f64::consts::PI / 180.0;
    let perpendicular = (student.heading + 90.0) * std::f64::consts::PI / 180.0;
    let lateral = -float::cos(gait) * 0.5 * student.scale;
    let registration =
        if student.scale < 1.0 { (1.0 - (student.scale - 0.75) * 4.0) * 5.0 } else { 0.0 };
    Vec2::new(
        float::sin(perpendicular) * lateral,
        -float::cos(perpendicular) * lateral - float::sin(gait).abs() * 1.5 + registration,
    )
}

fn signed_angle_delta(current: f64, target: f64) -> f64 {
    ((target - current + 540.0) % 360.0) - 180.0
}

fn student_speed_factor<'a>(
    student: &impl StudentRead,
    students: impl IntoIterator<Item = StudentRef<'a>>,
) -> f64 {
    let radians = student.heading * std::f64::consts::PI / 180.0;
    let ahead = Vec2::new(
        student.position.x + float::sin(radians) * 15.0,
        student.position.y - float::cos(radians) * 15.0,
    );
    for other in students {
        if other.id == student.id
            || other.controller.desired_speed > student.current_speed
            || signed_angle_delta(student.heading, other.heading).abs() > 90.0
        {
            continue;
        }
        let dx = other.position.x - ahead.x;
        let dy = other.position.y - ahead.y;
        if dx * dx + dy * dy < student.profile.radius * student.profile.radius {
            return 0.9;
        }
    }
    1.0
}

fn next_random(state: &mut u32) -> f64 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    *state = if value == 0 { 0x6d2b_79f5 } else { value };
    f64::from(value) / 4_294_967_296.0
}

fn random_float(state: &mut u32, maximum: f64) -> f64 {
    next_random(state) * maximum
}
fn random_integer(state: &mut u32, bound: u32) -> u32 {
    (bound - 1).min((next_random(state) * f64::from(bound)).floor() as u32)
}
fn random_signed_float(state: &mut u32, maximum: f64) -> f64 {
    let magnitude = random_float(state, maximum);
    magnitude * if next_random(state) < 0.5 { -1.0 } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn students_enter_through_doors_before_enabling_wall_collision() {
        assert!(!hub_student_static_collision_enabled(Vec2::new(760.0, 150.0)));
        assert!(!hub_student_static_collision_enabled(Vec2::new(1577.0, -29.0)));
        assert!(hub_student_static_collision_enabled(Vec2::new(800.0, 200.0)));
    }

    #[test]
    fn route_end_retires_a_student_instead_of_teleporting_it() {
        let splines = HubStudentSplines::new();
        let mut student = HubStudentState::new(&splines, 0, 0, STUDENT_SEED, 0.75, 49);
        student.controller.path_cursor = 12.99;
        student.position = splines.evaluate(0, 12.99);
        let plan = plan_hub_student_route(&splines, &student, [student.view()]);
        assert!(plan.state.retired);
        assert_eq!(plan.delta, Vec2::ZERO);
        assert_eq!(plan.state.position, student.position);
    }

    #[test]
    fn student_admissions_share_the_actor_sequence_with_later_world_actors() {
        let mut order = WorldPainterOrder { next_actor_ordinal: 100, next_transient_ordinal: 70 };
        let mut population = HubStudentPopulation::new(Some(&mut order));
        assert_eq!(order.next_actor_ordinal, 100 + population.students.len());
        assert_eq!(order.next_transient_ordinal, 70);
        let spell = order.register(WorldManagerLane::Actor);
        population.students.clear();
        population.rng_state = STUDENT_SEED;
        population.spawn_request_pending = true;
        population.step_routes(Some(&mut order));
        let admitted = population.students.get(0).expect("the seeded cold admission");
        assert_eq!(admitted.registration_ordinal, spell.registration_ordinal + 1);
    }
}
