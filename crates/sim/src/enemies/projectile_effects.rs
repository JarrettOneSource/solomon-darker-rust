//! Projectile-owned particles and Arrow tumble from `enemies/projectile-effects.ts`.
use super::effects::DeathEffectOwner;
use super::model::*;
use super::projectiles::*;
use super::world::EnemyWorld;
use crate::NativeRng;
use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};

#[derive(Clone, Copy, Debug)]
pub struct ProjectileEffectOwner {
    pub id: usize,
    pub owner_actor_id: usize,
    pub age_ticks: u64,
}
impl From<&EnemyProjectileState> for ProjectileEffectOwner {
    fn from(state: &EnemyProjectileState) -> Self {
        Self { id: state.id, owner_actor_id: state.owner_actor_id, age_ticks: state.age_ticks }
    }
}
pub struct ProjectileEffectSpec {
    pub entry: usize,
    pub lifetime_ticks: u64,
    pub registration_lane: WorldManagerLane,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct ProjectileEffectOverrides {
    pub alpha: Option<f64>,
    pub alpha_loss_per_tick: Option<f64>,
    pub angular_velocity_deg: Option<f64>,
    pub atlas: Option<EnemyEffectAtlas>,
    pub blend_mode: Option<EnemyEffectBlend>,
    pub light_registration: Option<PainterRegistration>,
    pub painter_registration: Option<PainterRegistration>,
    pub phase_origin_ticks: Option<u64>,
    pub rotation_deg: Option<f64>,
    pub scale: Option<f64>,
    pub tint: Option<u32>,
    pub velocity: Option<Vec2>,
}

pub fn create_projectile_effect(
    world: &mut EnemyWorld,
    owner: ProjectileEffectOwner,
    tick: u64,
    position: Vec2,
    spec: ProjectileEffectSpec,
    options: ProjectileEffectOverrides,
    order: &mut WorldPainterOrder,
) -> EnemyProjectileEffectState {
    let id = world.next_projectile_effect_id;
    world.next_projectile_effect_id += 1;
    EnemyProjectileEffectState {
        age_ticks: 0,
        alpha: options.alpha.unwrap_or(1.0),
        alpha_loss_per_tick: options.alpha_loss_per_tick.unwrap_or(0.0),
        angular_velocity_deg: options.angular_velocity_deg.unwrap_or(0.0),
        atlas: options.atlas.unwrap_or(EnemyEffectAtlas::BadGuys),
        blend_mode: options.blend_mode.unwrap_or(EnemyEffectBlend::Normal),
        entry: spec.entry,
        id,
        last_step_tick: tick,
        light_registration: options.light_registration,
        lifetime_ticks: spec.lifetime_ticks,
        owner_actor_id: owner.owner_actor_id,
        owner_projectile_id: owner.id,
        painter_registration: options
            .painter_registration
            .or(options.light_registration)
            .unwrap_or_else(|| world.register_projectile_painter(order, spec.registration_lane)),
        phase_origin_ticks: options.phase_origin_ticks.unwrap_or(owner.age_ticks),
        position,
        rotation_deg: options.rotation_deg.unwrap_or(0.0),
        scale: options.scale.unwrap_or(1.0),
        spawn_tick: tick,
        tint: options.tint.unwrap_or(0xffffff),
        velocity: options.velocity.unwrap_or(Vec2::ZERO),
    }
}

/// Native 0x60413B retains these FireParticle children only for enhanced bomb trails.
/// Their seeded presentation samples do not consume either gameplay random stream.
pub fn spawn_demon_bomb_particle(
    world: &mut EnemyWorld,
    projectile: &EnemyProjectileState,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    use crate::spells::fire::{
        native_fire_presentation_random, native_fire_presentation_random_int,
    };

    let id = world.next_projectile_effect_id as u64;
    let random = |channel, maximum| native_fire_presentation_random(id, 0, channel, maximum);
    let scale = fround(fround(random(4, 1.0) + 0.5) * 1.25);
    let fade = fround(fround(fround(random(5, 0.1) + 0.1) * 0.5) * fround(0.4));
    let radius = random(1, projectile.visual_scale * 10.0);
    let heading = random(2, 360.0) * std::f64::consts::PI / 180.0;
    let position = Vec2::new(
        fround(projectile.position.x + fround(float::sin(heading) * radius)),
        fround(
            projectile.position.y + projectile.vertical_offset
                - fround(float::cos(heading) * radius),
        ),
    );
    let registration = world.register_projectile_painter(order, WorldManagerLane::Transient);
    let state = create_projectile_effect(
        world,
        ProjectileEffectOwner::from(projectile),
        tick,
        position,
        ProjectileEffectSpec {
            entry: 267 + native_fire_presentation_random_int(id, 0, 6, 4) as usize,
            lifetime_ticks: (1.0 / fade).floor() as u64 + 2,
            registration_lane: WorldManagerLane::Transient,
        },
        ProjectileEffectOverrides {
            alpha_loss_per_tick: Some(fade),
            angular_velocity_deg: Some(1.0),
            blend_mode: Some(EnemyEffectBlend::Add),
            painter_registration: Some(registration),
            rotation_deg: Some(random(3, 360.0)),
            scale: Some(scale),
            ..Default::default()
        },
        order,
    );
    world
        .projectile_effects
        .push(EnemyProjectileEffect { state, kind: EnemyProjectileEffectKind::DemonBombParticle });
}

pub fn spawn_firebolt_trail(
    world: &mut EnemyWorld,
    projectile: &EnemyProjectileState,
    previous_tick: u64,
    order: &mut WorldPainterOrder,
) {
    // Trails use the preceding clock even when a projectile is catching up several ticks.
    let tick = previous_tick + 1;
    if !tick.is_multiple_of(2) {
        return;
    }
    let end = projectile.position;
    let magnitude = world.steering_rng.draw_scaled_float(5.0, false);
    let angle = world.steering_rng.draw_scaled_float(360.0, false) * std::f64::consts::PI / 180.0;
    let loss = fround(fround(0.1) * (1.5 + world.steering_rng.draw_scaled_float(0.5, false)));
    let scale = fround(0.75 + world.steering_rng.draw_scaled_float(0.25, false));
    let state = create_projectile_effect(
        world,
        ProjectileEffectOwner::from(projectile),
        tick,
        Vec2::new(
            end.x + float::sin(angle) * magnitude,
            end.y - 15.0 - float::cos(angle) * magnitude,
        ),
        ProjectileEffectSpec {
            entry: 255 + ((projectile.age_ticks) % 12) as usize,
            lifetime_ticks: 8,
            registration_lane: WorldManagerLane::Actor,
        },
        ProjectileEffectOverrides {
            painter_registration: Some(projectile.painter_registration),
            blend_mode: Some(EnemyEffectBlend::Add),
            alpha_loss_per_tick: Some(loss),
            phase_origin_ticks: Some(projectile.age_ticks),
            rotation_deg: Some(projectile.heading_deg + 180.0),
            scale: Some(scale),
            ..ProjectileEffectOverrides::default()
        },
        order,
    );
    world
        .projectile_effects
        .push(EnemyProjectileEffect { state, kind: EnemyProjectileEffectKind::FireboltTrail });
}

pub fn spawn_fire_burst(
    world: &mut EnemyWorld,
    owner: ProjectileEffectOwner,
    tick: u64,
    position: Vec2,
    scale_base: f64,
    scale_range: f64,
    order: &mut WorldPainterOrder,
) {
    world.emit_action_sound(
        DeathEffectOwner { id: owner.owner_actor_id, position },
        tick,
        EnemySound::FireballHit,
        2.0,
    );
    let rotation = world.steering_rng.draw_scaled_float(360.0, false);
    let angular = fround(0.5 + world.steering_rng.draw_scaled_float(1.0, false));
    let angular = world.steering_rng.draw_sign(angular);
    let scale = fround(scale_base + world.steering_rng.draw_scaled_float(scale_range, true));
    let registration = world.register_projectile_painter(order, WorldManagerLane::Transient);
    let state = create_projectile_effect(
        world,
        owner,
        tick,
        Vec2::new(position.x, fround(position.y - 10.0)),
        ProjectileEffectSpec {
            entry: 251,
            lifetime_ticks: 16,
            registration_lane: WorldManagerLane::Transient,
        },
        ProjectileEffectOverrides {
            alpha: Some(0.5),
            alpha_loss_per_tick: Some(0.5 / 16.0),
            angular_velocity_deg: Some(angular),
            blend_mode: Some(EnemyEffectBlend::Add),
            light_registration: Some(registration),
            rotation_deg: Some(rotation),
            scale: Some(scale),
            velocity: Some(Vec2::new(0.0, -1.0)),
            ..ProjectileEffectOverrides::default()
        },
        order,
    );
    world
        .projectile_effects
        .push(EnemyProjectileEffect { state, kind: EnemyProjectileEffectKind::FireBurst });
}

pub fn spawn_projectile_impact_effects(
    world: &mut EnemyWorld,
    projectile: &impl EnemyProjectileRead,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    let state = projectile.state();
    match projectile.kind() {
        EnemyProjectileKind::Arrow => {
            if state.payload == EnemyProjectilePayload::Fire {
                spawn_fire_burst(
                    world,
                    ProjectileEffectOwner::from(state),
                    tick,
                    state.position,
                    0.5,
                    0.1,
                    order,
                );
            }
        }
        EnemyProjectileKind::Firebolt => spawn_fire_burst(
            world,
            ProjectileEffectOwner::from(state),
            tick,
            state.position,
            0.75,
            0.1,
            order,
        ),
        EnemyProjectileKind::GuidedMissile => {
            let pitch = fround(1.0 + world.steering_rng.draw_scaled_float(0.1, false));
            world.emit_action_sound(
                DeathEffectOwner { id: state.owner_actor_id, position: state.position },
                tick,
                EnemySound::MagicMissileHit,
                pitch,
            );
            // The native light/painter is allocated before the impact rotation draw.
            let registration =
                world.register_projectile_painter(order, WorldManagerLane::Transient);
            let rotation = world.steering_rng.draw_scaled_float(360.0, false);
            let cold = state.payload == EnemyProjectilePayload::Cold;
            let effect = create_projectile_effect(
                world,
                ProjectileEffectOwner::from(state),
                tick,
                state.position,
                ProjectileEffectSpec {
                    entry: if cold { 110 } else { 111 },
                    lifetime_ticks: 20,
                    registration_lane: WorldManagerLane::Transient,
                },
                ProjectileEffectOverrides {
                    alpha: Some(2.0),
                    alpha_loss_per_tick: Some(0.1),
                    blend_mode: Some(EnemyEffectBlend::Add),
                    light_registration: Some(registration),
                    rotation_deg: Some(rotation),
                    scale: Some(2.0),
                    tint: Some(if cold { 0x4080ff } else { 0x40ff40 }),
                    ..ProjectileEffectOverrides::default()
                },
                order,
            );
            world.projectile_effects.push(EnemyProjectileEffect {
                state: effect,
                kind: EnemyProjectileEffectKind::GuidedImpact,
            });
        }
        EnemyProjectileKind::DemonBomb | EnemyProjectileKind::PoisonPool => {}
    }
}

impl EnemyWorld {
    pub fn set_arrow_chill_tumble_accumulator(&mut self, id: usize, accumulator: f64) -> bool {
        assert!(
            accumulator.is_finite() && (0.0..=1.0).contains(&accumulator),
            "Arrow Chill accumulator must be finite and within [0,1]"
        );
        let Some(projectile) = self.projectiles.iter_mut().find(|projectile| {
            projectile.state.id == id && projectile.kind() == EnemyProjectileKind::Arrow
        }) else {
            return false;
        };
        projectile.state.chill_tumble_accumulator = accumulator;
        true
    }

    pub fn tumble_arrow(
        &mut self,
        id: usize,
        direction: Vec2,
        tick: u64,
        combat_rng: &mut NativeRng,
        painter_order: Option<&mut WorldPainterOrder>,
    ) -> bool {
        assert!(
            self.last_step_tick.is_none_or(|last| tick >= last),
            "Arrow tumble tick must not precede the store clock"
        );
        assert!(
            direction.x.is_finite() && direction.y.is_finite(),
            "Arrow tumble direction must be finite"
        );
        let length = hypot(direction.x, direction.y);
        assert!(length > 0.0, "Arrow tumble direction must be nonzero");
        let Some(index) = self.projectiles.iter().position(|projectile| {
            projectile.state().id == id && projectile.kind() == EnemyProjectileKind::Arrow
        }) else {
            return false;
        };
        let mut standalone;
        let order = if let Some(order) = painter_order {
            order
        } else {
            standalone = self.standalone_painter_order();
            &mut standalone
        };
        let projectile = self.projectiles.remove(index);
        let state = projectile.state();
        let rotation = combat_rng.draw_scaled_float(360.0, false);
        let magnitude = combat_rng.draw_scaled_float(10.0, false);
        let angular = combat_rng.draw_sign(fround(10.0 + magnitude));
        // Chill's external Arrow tumble is an immediate world producer, even while
        // the ordinary enemy-projectile births are waiting for the session commit.
        let registration = order.register(WorldManagerLane::Actor);
        let effect = create_projectile_effect(
            self,
            ProjectileEffectOwner::from(state),
            tick,
            state.position,
            ProjectileEffectSpec {
                entry: 2,
                lifetime_ticks: 41,
                registration_lane: WorldManagerLane::Actor,
            },
            ProjectileEffectOverrides {
                painter_registration: Some(registration),
                alpha: Some(fround(4.0)),
                alpha_loss_per_tick: Some(fround(0.1)),
                angular_velocity_deg: Some(angular),
                rotation_deg: Some(rotation),
                velocity: Some(Vec2::new(
                    fround(direction.x / length),
                    fround(direction.y / length),
                )),
                ..ProjectileEffectOverrides::default()
            },
            order,
        );
        self.projectile_effects.push(EnemyProjectileEffect {
            state: effect,
            kind: EnemyProjectileEffectKind::ArrowTumble,
        });
        let mut event =
            EnemyEvent::new(EnemyEventKind::ProjectileRetired, state.owner_actor_id, tick);
        event.projectile_id = Some(id);
        event.target_player_id = Some(None);
        self.emit_event(event);
        true
    }

    pub fn step_projectile_effects(&mut self, tick: u64) {
        self.projectile_effects.retain_mut(|effect| {
            if matches!(effect.kind, EnemyProjectileEffectKind::DemonFire { .. })
                || tick <= effect.state.last_step_tick
            {
                return true;
            }
            let state = &mut effect.state;
            let elapsed = tick - state.last_step_tick;
            let age = state.age_ticks + elapsed;
            if age >= state.lifetime_ticks {
                return false;
            }
            let mut alpha = state.alpha;
            for _ in 0..elapsed {
                alpha = fround(alpha - state.alpha_loss_per_tick);
            }
            let mut rotation = state.rotation_deg + state.angular_velocity_deg * elapsed as f64;
            if matches!(effect.kind, EnemyProjectileEffectKind::ArrowTumble) {
                alpha = state.alpha;
                rotation = state.rotation_deg;
                for _ in 0..elapsed {
                    alpha = fround(alpha - fround(0.1));
                    state.position = Vec2::new(
                        fround(state.position.x + state.velocity.x),
                        fround(state.position.y + state.velocity.y),
                    );
                    rotation = fround(rotation + state.angular_velocity_deg);
                    state.velocity = Vec2::new(
                        fround(state.velocity.x * fround(0.98)),
                        fround(state.velocity.y * fround(0.98)),
                    );
                }
            } else {
                match effect.kind {
                    EnemyProjectileEffectKind::FireBurst => {
                        state.entry = 251 + ((age / 4).min(3)) as usize
                    }
                    EnemyProjectileEffectKind::PoisonBubble { growth_per_tick, maximum_scale } => {
                        for _ in 0..elapsed {
                            state.scale = fround(state.scale + growth_per_tick).min(maximum_scale);
                        }
                    }
                    EnemyProjectileEffectKind::DemonBombParticle => {
                        for _ in 0..elapsed {
                            state.scale = fround(state.scale * 0.95);
                        }
                    }
                    _ => {}
                }
                if state.velocity != Vec2::ZERO {
                    state.position = Vec2::new(
                        state.position.x + state.velocity.x * elapsed as f64,
                        state.position.y + state.velocity.y * elapsed as f64,
                    );
                }
            }
            if alpha < 0.0
                || (alpha == 0.0 && effect.kind != EnemyProjectileEffectKind::DemonBombParticle)
            {
                return false;
            }
            if effect.kind == EnemyProjectileEffectKind::DemonBombParticle {
                let red = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
                let green_blue = ((alpha * 2.0 - 1.0).clamp(0.0, 1.0) * 255.0).round() as u32;
                state.tint = red << 16 | green_blue << 8 | green_blue;
            }
            state.age_ticks = age;
            state.alpha = alpha;
            state.last_step_tick = tick;
            state.rotation_deg = rotation;
            true
        });
    }
}
