//! Spider's native movement-call state machine from `core-kernels/native-spider.ts`.
//! Cadence belongs to the actor wrapper; one call owns its precise random draw order.
use super::model::SpiderBrain;
use super::silk::{NativeSilkCreation, create_native_silk};
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::spells::targeting::direction_from_heading;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpiderTarget {
    pub position: Vec2,
    pub heading_deg: f64,
    pub radius: f64,
    pub is_player: bool,
    pub web_severity: f64,
    pub velocity_per_tick: Vec2,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpiderRouteGoal {
    pub goal: Vec2,
    pub following_route: bool,
    pub turn_around: bool,
}
pub struct SpiderMovementRequest<'a> {
    pub route_goal: Option<&'a mut dyn FnMut(Vec2) -> SpiderRouteGoal>,
    pub heading_deg: f64,
    pub position: Vec2,
    pub speed: f64,
    pub status_factor: f64,
    pub light: f64,
    pub target: Option<SpiderTarget>,
    pub wander_heading_deg: f64,
    pub spit_webs: bool,
    pub cocoon_health: f64,
    pub live_silk_count: usize,
    pub shared_spit_ticks_remaining: u64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SpiderMovementResult {
    pub state: SpiderBrain,
    pub heading_deg: f64,
    pub position: Vec2,
    pub goal: Vec2,
    pub bite: bool,
    pub suck: bool,
    pub silk: Option<NativeSilkCreation>,
    pub bite_sounds: Option<[usize; 2]>,
    pub shared_spit_ticks_remaining: u64,
}
struct MovementWork<'rng, 'route> {
    state: SpiderBrain,
    heading_deg: f64,
    position: Vec2,
    goal: Vec2,
    bite: bool,
    suck: bool,
    silk: Option<NativeSilkCreation>,
    bite_sounds: Option<[usize; 2]>,
    shared_spit_ticks_remaining: u64,
    rng: &'rng mut NativeRng,
    request: SpiderMovementRequest<'route>,
}

#[must_use]
pub fn move_native_spider(
    source: &SpiderBrain,
    request: SpiderMovementRequest<'_>,
    rng: &mut NativeRng,
) -> SpiderMovementResult {
    let mut work = MovementWork {
        state: source.clone(),
        heading_deg: request.heading_deg,
        position: request.position,
        goal: request.position,
        bite: false,
        suck: false,
        silk: None,
        bite_sounds: None,
        shared_spit_ticks_remaining: request.shared_spit_ticks_remaining,
        rng,
        request,
    };
    if let Some(target) = work.request.target {
        match work.state.action_state {
            0 => work.approach(target),
            1 => work.close_movement(target),
            2 => work.strafe(target),
            3 => {
                work.state.frame = 3.0;
                if work.state.action_ticks_remaining < 1 {
                    work.state.action_state = 1;
                    work.state.frame = work.integer(3) as f64;
                }
            }
            4 => work.retreat(target),
            5 => work.attach(target),
            other => panic!("invalid native Spider action state {other}"),
        }
        if work.state.action_state != 5 {
            work.state.attached = false;
        }
        if !work.state.attached {
            work.state.vertical_offset = (work.state.vertical_offset + 2.0).min(0.0);
        }
    } else {
        work.state.frame = work.integer(3) as f64;
        work.goal = offset(work.position, work.request.wander_heading_deg, 10_000.0);
    }
    let routed = work.request.route_goal.as_mut().map(|route| route(work.goal));
    if let Some(routed) = routed {
        work.goal = routed.goal;
        if routed.turn_around {
            work.heading_deg += 180.0;
        }
    }
    let mut dx = work.goal.x - work.position.x;
    let mut dy = work.goal.y - work.position.y;
    let distance = hypot(dx, dy);
    if work.state.action_state != 5 && distance > work.request.speed {
        dx *= work.request.speed / distance;
        dy *= work.request.speed / distance;
        work.goal = Vec2::new(work.position.x + dx, work.position.y + dy);
    }
    if routed.is_some_and(|routed| routed.following_route) || work.request.target.is_none() {
        work.state.body_heading_deg = work.heading_deg;
        work.heading_deg = heading_degrees_from_vector(dx, dy);
        if work.request.target.is_none() {
            work.heading_deg += work.floating(45.0, true);
            work.goal = offset(work.position, work.heading_deg, work.request.speed * 1.5);
        }
    }
    // This counter intentionally becomes negative in native idle/approach states.
    work.state.action_ticks_remaining -= 1;
    SpiderMovementResult {
        state: work.state,
        heading_deg: work.heading_deg,
        position: work.position,
        goal: work.goal,
        bite: work.bite,
        suck: work.suck,
        silk: work.silk,
        bite_sounds: work.bite_sounds,
        shared_spit_ticks_remaining: work.shared_spit_ticks_remaining,
    }
}

impl MovementWork<'_, '_> {
    fn integer(&mut self, bound: i32) -> i64 {
        i64::from(self.rng.draw_integer(bound))
    }
    fn floating(&mut self, maximum: f64, signed: bool) -> f64 {
        self.rng.draw_scaled_float(maximum, signed)
    }
    fn range(&mut self, low: f64, high: f64) -> f64 {
        self.rng.draw_float_range(low, high)
    }
    fn can_spit(&self, target: SpiderTarget) -> bool {
        self.request.spit_webs && target.is_player
    }

    fn approach(&mut self, target: SpiderTarget) {
        let near = distance_squared(self.position, target.position)
            <= (self.state.preferred_distance * fround(0.9)).powi(2);
        let eligible = self.can_spit(target);
        let wants_spit = if near {
            eligible && self.integer(2) == 1
        } else {
            eligible && self.state.spit_ticks_remaining < 1 && self.request.light == 0.0
        };
        if wants_spit
            && target.web_severity <= 2.0
            && self.request.live_silk_count < 4
            && self.shared_spit_ticks_remaining < 1
        {
            self.spit(target);
            self.state.action_ticks_remaining *= 3;
            self.shared_spit_ticks_remaining = 5 + self.integer(46) as u64;
            self.state.spit_ticks_remaining = 200 + self.integer(401) as u64;
            return;
        }
        if near {
            self.enter_close();
            return;
        }
        let heading = bearing(self.position, target.position) + self.floating(10.0, true);
        self.turn(heading);
        self.goal = offset(
            self.position,
            self.heading_deg,
            self.request.speed * self.request.status_factor,
        );
        self.state.frame = self.integer(3) as f64;
    }
    fn enter_close(&mut self) {
        self.state.action_state = 1;
        self.state.move_ticks_remaining = 10 + self.integer(41);
        self.heading_deg += self.range(-10.0, 10.0);
        self.state.action_ticks_remaining = 50 + self.integer(51);
    }
    fn close_movement(&mut self, target: SpiderTarget) {
        if distance_squared(self.position, target.position)
            < ((target.radius + 15.0) * 1.25).powi(2)
        {
            self.state.action_state = 5;
            return;
        }
        self.state.move_ticks_remaining -= 1;
        if self.state.move_ticks_remaining < 1 {
            self.state.move_ticks_remaining = 50 + self.integer(151);
            self.heading_deg = bearing(self.position, target.position);
            let distance = self.range(5.0, 15.0);
            let heading = self.floating(360.0, false);
            self.goal = offset(self.position, heading, distance);
            self.state.body_heading_deg = heading;
            self.state.frame = self.integer(3) as f64;
            self.limit_body_turn();
            if self.state.action_ticks_remaining < 1 {
                self.choose_close_action(target);
            }
        }
        if self.request.light < fround(0.05) {
            self.state.action_state = 0;
            if self.state.preferred_distance > 150.0 {
                self.state.preferred_distance = fround(self.state.preferred_distance * fround(0.8));
            }
        }
    }
    fn choose_close_action(&mut self, target: SpiderTarget) {
        let severity = if target.is_player { target.web_severity } else { 5.0 };
        let far = distance_squared(self.position, target.position)
            >= self.state.preferred_distance.powi(2);
        if self.integer(4) == 1 {
            if self.can_spit(target) && severity <= 2.0 && self.request.live_silk_count < 4 {
                self.spit(target);
            } else if far {
                self.start_strafe();
            } else {
                self.state.action_state = 5;
            }
            return;
        }
        if severity < 3.0 {
            if self.integer(3) == 1 {
                self.start_strafe();
            }
            return;
        }
        if (self.integer(5) == 1 && far) || self.request.light <= fround(0.1) {
            self.start_strafe();
        } else {
            self.state.action_state = 5;
        }
    }
    fn start_strafe(&mut self) {
        self.state.strafe_sign = self.rng.draw_sign(1.0);
        self.state.action_state = 2;
        self.state.strafe_ticks_remaining = 20 + self.integer(21);
    }
    fn strafe(&mut self, target: SpiderTarget) {
        let heading = bearing(self.position, target.position)
            + (10.0 + self.floating(10.0, false)) * self.state.strafe_sign;
        let direction = direction_from_heading(heading);
        let jitter = self.floating(5.0, false);
        let jitter_heading = self.floating(360.0, false);
        let jitter_vector = offset(Vec2::ZERO, jitter_heading, jitter);
        let speed = self.range(self.request.speed * 0.5, self.request.speed * 2.0);
        self.goal = Vec2::new(
            self.position.x
                + (direction.y * self.state.strafe_sign * speed + jitter_vector.x)
                    * self.request.status_factor,
            self.position.y
                + (-direction.x * self.state.strafe_sign * speed + jitter_vector.y)
                    * self.request.status_factor,
        );
        let heading = heading + self.floating(25.0, true);
        self.turn(heading);
        self.state.frame = self.integer(3) as f64;
        self.state.strafe_ticks_remaining -= 1;
        if self.state.strafe_ticks_remaining < 1 {
            self.state.action_state = 1;
            self.state.move_ticks_remaining = 0;
            self.state.action_ticks_remaining = 100 + self.integer(201);
        }
    }
    fn retreat(&mut self, target: SpiderTarget) {
        if distance_squared(self.position, target.position)
            >= (self.state.preferred_distance * fround(0.9)).powi(2)
        {
            self.enter_close();
            return;
        }
        let heading = self.heading_deg + self.floating(10.0, true);
        self.turn(heading);
        self.goal = offset(
            self.position,
            self.heading_deg,
            -self.request.speed * self.request.status_factor,
        );
        self.state.frame = self.integer(3) as f64;
    }
    fn attach(&mut self, target: SpiderTarget) {
        self.state.frame = 3.0;
        let distance = distance_squared(self.position, target.position).sqrt();
        if distance > (target.radius + if self.state.attached { 5.0 } else { 15.0 }) * 1.5 {
            self.heading_deg = bearing(self.position, target.position);
            self.state.body_heading_deg = self.heading_deg;
            self.goal = offset(
                self.position,
                self.heading_deg,
                self.request.speed * self.request.status_factor * 3.0,
            );
            if target.is_player && target.web_severity < 3.0 {
                self.detach(false);
            }
            return;
        }
        if !self.state.attached {
            self.bite = true;
            self.bite_sounds = Some([self.integer(2) as usize, self.integer(2) as usize]);
            self.state.vertical_offset = -self.range(10.0, 20.0);
            self.state.attachment_angle_deg =
                bearing(target.position, self.position) - target.heading_deg;
            self.state.attachment_distance = distance.min(target.radius + 15.0);
            self.state.suck_ticks = 0;
        }
        self.state.attached = target.is_player && target.web_severity >= 3.0;
        if !self.state.attached {
            self.detach(true);
            return;
        }
        self.position = offset(
            target.position,
            target.heading_deg + self.state.attachment_angle_deg,
            self.state.attachment_distance,
        );
        self.goal = self.position;
        self.heading_deg = bearing(self.position, target.position);
        self.state.body_heading_deg = self.heading_deg + self.floating(10.0, true);
        self.state.suck_ticks += 1;
        if self.state.suck_ticks > 25 {
            self.state.suck_ticks = 0;
            self.suck = true;
        }
    }
    fn detach(&mut self, contacted: bool) {
        self.state.action_state = 4;
        self.heading_deg += self.floating(25.0, true);
        if contacted {
            self.state.vertical_offset = -35.0;
        }
    }
    fn spit(&mut self, target: SpiderTarget) {
        let mut position = offset(self.position, self.heading_deg, 20.0);
        position.y += 10.0;
        let silk = create_native_silk(
            position,
            target.position,
            target.velocity_per_tick,
            self.request.cocoon_health,
            self.rng,
        );
        self.state.action_state = 3;
        self.state.action_ticks_remaining = 50;
        self.silk = Some(silk);
    }
    fn turn(&mut self, heading: f64) {
        self.state.body_heading_deg = self.heading_deg;
        self.heading_deg = heading;
        self.limit_body_turn();
    }
    fn limit_body_turn(&mut self) {
        let delta = ((self.state.body_heading_deg - self.heading_deg + 540.0) % 360.0 + 360.0)
            % 360.0
            - 180.0;
        if delta.abs() > 40.0 {
            self.state.body_heading_deg = self.heading_deg;
        }
    }
}
fn bearing(from: Vec2, to: Vec2) -> f64 {
    heading_degrees_from_vector(to.x - from.x, to.y - from.y)
}
fn offset(point: Vec2, heading: f64, distance: f64) -> Vec2 {
    let direction = direction_from_heading(heading);
    Vec2::new(point.x + direction.x * distance, point.y + direction.y * distance)
}
fn distance_squared(a: Vec2, b: Vec2) -> f64 {
    (a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)
}
