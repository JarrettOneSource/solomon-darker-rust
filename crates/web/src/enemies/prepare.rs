//! Build the enemy queues from canonical simulation rows and source presentation plans.

use solomon_sim::enemies::actor_presentation::{
    EnemyActorAppearanceContext, EnemyArtAtlas, EnemyArtPoints, EnemySpiderOutlineTarget,
    EnemySpriteLayer, native_enemy_actor_plan, native_enemy_actor_plan_with_input,
    native_maggot_plan, native_maggot_plan_with_input,
};
use solomon_sim::enemies::boss_presentation::native_boss_spell_plan;
use solomon_sim::enemies::boss_spell::BossSpellKind;
use solomon_sim::enemies::death_presentation::native_enemy_death_effect_plan_with_input;
use solomon_sim::enemies::model::{DeathPresentationOwner, EnemyBrain, SpiderPhase};
use solomon_sim::enemies::presentation::{
    NativeEnemyEffectPresentationInput, NativeEnemyProjectileKind, NativeEnemyProjectilePayload,
    NativeEnemyProjectileVisual, native_enemy_projectile_plan_at_age,
};
use solomon_sim::enemies::projectile_presentation::{
    enemy_projectile_effect_bypasses_world_tint, native_enemy_projectile_effect_plan_with_input,
};
use solomon_sim::enemies::projectiles::EnemyProjectileRead;
use solomon_sim::enemies::projectiles::{
    EnemyProjectileEffectKind, EnemyProjectileKind, EnemyProjectilePayload,
};
use solomon_sim::enemies::rain_presentation::NativeRainCloud;
use solomon_sim::enemies::spider_remains::native_dead_spider_entry;
use solomon_sim::enemies::tendrils::HeartmongerTendrils;
use solomon_sim::enemies::underlay::{
    native_enemy_underlay_plan, native_enemy_underlay_plan_with_input,
};
use solomon_sim::enemies::web_presentation::{native_fade_line_mesh, native_silk_mesh};
use solomon_sim::enemies::world::EnemyWorld;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::{NativeRng, Vec2};

use super::{EnemyDraw, EnemyDrawGroup, EnemyPainter, EnemyRoot, EnemyView, EnemyViewFrame};

impl EnemyView {
    pub(super) fn prepare_actors(&mut self, world: &EnemyWorld, frame: &EnemyViewFrame<'_>) {
        self.tendrils.retain(|id, _| world.actors.iter().any(|actor| actor.id == *id));
        for actor in world.actors.iter() {
            if matches!(&actor.brain,EnemyBrain::Spider(brain) if brain.phase==SpiderPhase::Captured)
            {
                continue;
            }
            let target = frame
                .players
                .iter()
                .find(|player| Some(player.id) == actor.target_player_id.as_deref());
            let pike_target = if let EnemyBrain::Skeleton(brain) = &actor.brain {
                brain.pike.as_ref().map(|pike| {
                    frame
                        .players
                        .iter()
                        .find(|player| player.id == pike.player_id)
                        .map_or(pike.position, |player| player.position)
                })
            } else {
                None
            };
            let context = EnemyActorAppearanceContext {
                tick: frame.tick,
                application_tick: frame.application_tick,
                complex_lighting: frame.complex_lighting,
                spider_exposure: (frame.light_at)(actor.position),
                spider_target: target.map(|player| EnemySpiderOutlineTarget {
                    position: player.position,
                    primary_tint: player.primary_tint,
                }),
                pike_target_position: pike_target,
            };
            let sample = frame.presentation.and_then(|metadata| metadata.actors.get(&actor.id));
            let plan = match sample {
                Some(sample) => {
                    native_enemy_actor_plan_with_input(&actor, context, sample, &self.art)
                }
                None => native_enemy_actor_plan(&actor, context, &self.art),
            };
            let mut group = EnemyDrawGroup::at(actor.position);
            if let Some(origin) = plan.attack_effect_origin {
                self.attack_origins.insert(actor.id, origin);
            }
            group.scale = plan.root_scale;
            group.material_tint = Some((frame.actor_tint)(&actor));
            for layer in plan.layers {
                for segment in &plan.segments {
                    if segment.before_role == layer.role {
                        group.draws.push(EnemyDraw::Segment(segment.clone()));
                    }
                }
                group.draws.push(EnemyDraw::Sprite { layer, region_tint: true });
            }
            self.push_root(
                PainterRegistration {
                    manager_lane: WorldManagerLane::Actor,
                    registration_ordinal: actor.registration_ordinal,
                },
                actor.position.y,
                0.0,
                None,
                group,
            );
            let shadows = (frame.actor_shadows)(&actor);
            let layers = match sample {
                Some(sample) => native_enemy_underlay_plan_with_input(
                    &actor,
                    frame.tick,
                    &shadows.records,
                    shadows.light_scalar,
                    frame.complex_shadows,
                    &self.art,
                    sample,
                ),
                None => native_enemy_underlay_plan(
                    &actor,
                    frame.tick,
                    &shadows.records,
                    shadows.light_scalar,
                    frame.complex_shadows,
                    &self.art,
                ),
            };
            let mut underlay = EnemyDrawGroup::at(actor.position);
            underlay.draws.extend(layers.into_iter().map(EnemyDraw::Underlay));
            if let EnemyBrain::Heartmonger(brain) = &actor.brain {
                let tendrils = self.tendrils.entry(actor.id).or_insert_with(|| {
                    let mut rng = NativeRng::new(actor.id as i64 + actor.spawn_tick as i64);
                    HeartmongerTendrils::new(actor.position, actor.heading_deg, &mut rng)
                });
                let facing =
                    (((actor.heading_deg + 10.0) / 20.0).trunc() as i64).rem_euclid(18) as usize;
                let gait = sample.map_or(brain.leg_phase, |sample| sample.gait_pose);
                let entry = 200 + gait.trunc() as usize * 18 + facing;
                tendrils.move_to(
                    gait,
                    actor.position,
                    self.art.points(EnemyArtAtlas::Heartmonger, entry),
                );
                let direction =
                    shadows.records.first().map_or(Vec2::ZERO, |record| record.direction);
                for layer in tendrils.layers(actor.position, direction) {
                    underlay.draws.push(EnemyDraw::Sprite { layer, region_tint: false });
                }
            }
            self.underlays.push((actor.registration_ordinal, underlay));
        }
        for maggot in &world.maggots {
            let group = EnemyDrawGroup::sprites(
                maggot.position,
                match frame.presentation.and_then(|metadata| metadata.maggots.get(&maggot.id)) {
                    Some(sample) => {
                        native_maggot_plan_with_input(maggot, sample, frame.complex_lighting)
                    }
                    None => native_maggot_plan(maggot, frame.tick, frame.complex_lighting),
                },
                true,
            );
            self.push_root(
                PainterRegistration {
                    manager_lane: WorldManagerLane::Actor,
                    registration_ordinal: maggot.registration_ordinal,
                },
                maggot.position.y,
                0.0,
                Some(maggot.position),
                group,
            );
            let shadow = EnemySpriteLayer {
                scale: f64::from(0.45_f32),
                ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 67, "maggot-ground-shadow")
            };
            self.underlays.push((
                maggot.registration_ordinal,
                EnemyDrawGroup::sprites(maggot.position, vec![shadow], false),
            ));
        }
    }

    pub(super) fn prepare_projectiles(&mut self, world: &EnemyWorld, frame: &EnemyViewFrame<'_>) {
        for projectile in world.projectiles.iter() {
            let state = projectile.state();
            let visual = NativeEnemyProjectileVisual {
                age_ticks: state.age_ticks,
                heading_degrees: frame
                    .presentation
                    .and_then(|metadata| metadata.projectile_heading_deg.get(&state.id))
                    .copied()
                    .unwrap_or(state.heading_deg),
                id: state.id as u64,
                kind: match projectile.kind() {
                    EnemyProjectileKind::Arrow => NativeEnemyProjectileKind::Arrow,
                    EnemyProjectileKind::DemonBomb => NativeEnemyProjectileKind::DemonBomb,
                    EnemyProjectileKind::Firebolt => NativeEnemyProjectileKind::Firebolt,
                    EnemyProjectileKind::GuidedMissile => NativeEnemyProjectileKind::GuidedMissile,
                    EnemyProjectileKind::PoisonPool => NativeEnemyProjectileKind::PoisonPool,
                },
                lifetime_ticks: state.lifetime_ticks,
                payload: match state.payload {
                    EnemyProjectilePayload::Cold => NativeEnemyProjectilePayload::Cold,
                    EnemyProjectilePayload::Fire => NativeEnemyProjectilePayload::Fire,
                    EnemyProjectilePayload::None => NativeEnemyProjectilePayload::None,
                    EnemyProjectilePayload::Normal => NativeEnemyProjectilePayload::Normal,
                    EnemyProjectilePayload::Poison => NativeEnemyProjectilePayload::Poison,
                },
                position: state.position,
                speed: state.speed,
                vertical_offset: state.vertical_offset,
                visual_phase_degrees: frame
                    .presentation
                    .and_then(|metadata| metadata.projectile_visual_phase_deg.get(&state.id))
                    .copied()
                    .unwrap_or(state.visual_phase_deg),
                visual_scale: state.visual_scale,
            };
            let age = frame
                .presentation
                .and_then(|metadata| metadata.projectile_age_ticks.get(&state.id))
                .copied()
                .unwrap_or(state.age_ticks as f64);
            let plan = native_enemy_projectile_plan_at_age(&visual, frame.tick as f64, age);
            let underlays = plan.underlays.into_iter().map(EnemySpriteLayer::from).collect();
            self.pre_world.push(EnemyDrawGroup::sprites(plan.position, underlays, false));
            if projectile.kind() == EnemyProjectileKind::PoisonPool {
                continue;
            }
            let mut group = EnemyDrawGroup::at(plan.position);
            if let Some(streak) = plan.streak {
                group.draws.push(super::meshes::arrow_streak(streak, plan.position));
            }
            let mut layers: Vec<EnemySpriteLayer> =
                plan.layers.into_iter().map(EnemySpriteLayer::from).collect();
            for trail in &world.projectile_effects {
                if trail.kind != EnemyProjectileEffectKind::FireboltTrail
                    || trail.state.owner_projectile_id != state.id
                {
                    continue;
                }
                let sample = frame
                    .presentation
                    .and_then(|metadata| metadata.projectile_effects.get(&trail.state.id))
                    .copied()
                    .unwrap_or(NativeEnemyEffectPresentationInput {
                        age_ticks: trail.state.age_ticks as f64,
                        rotation_radians: trail.state.rotation_deg * std::f64::consts::PI / 180.0,
                    });
                let trail_plan = native_enemy_projectile_effect_plan_with_input(trail, 1.0, sample);
                for mut layer in trail_plan.layers {
                    layer.alpha *= ((state.lifetime_ticks as f64 - age).max(0.0) / 100.0).min(1.0);
                    layer.offset = Vec2::new(
                        trail.state.position.x - plan.position.x,
                        trail.state.position.y - plan.position.y,
                    );
                    layers.push(layer);
                }
            }
            for (index, layer) in layers.iter().enumerate() {
                let own_tint = projectile.kind() == EnemyProjectileKind::Firebolt
                    || (projectile.kind() == EnemyProjectileKind::Arrow
                        && state.payload == EnemyProjectilePayload::Fire
                        && index > 0);
                group
                    .draws
                    .push(EnemyDraw::Sprite { layer: layer.clone(), region_tint: !own_tint });
            }
            let hit = world
                .puppet_hits
                .iter()
                .find(|hit| hit.target_id == format!("projectile:{}", state.id));
            if let Some(hit) = hit {
                let alpha = match frame.puppet_hit_alphas {
                    Some(alphas) => *alphas
                        .get(&hit.target_id)
                        .expect("a displayed projectile hit retains its sampled alpha"),
                    None => hit.feedback.alpha(
                        frame
                            .presentation
                            .map_or(frame.tick as f64, |metadata| metadata.presentation_tick),
                    ),
                };
                if alpha > 0.0 {
                    if let Some(streak) = plan.streak {
                        group.draws.push(super::meshes::arrow_streak(streak, plan.position));
                    }
                    for (index, mut layer) in layers.into_iter().enumerate() {
                        let own_color = projectile.kind() == EnemyProjectileKind::Firebolt
                            || (projectile.kind() == EnemyProjectileKind::Arrow && index > 0);
                        layer.texture_diffuse = true;
                        if !own_color {
                            layer.alpha *= alpha;
                            let red =
                                if frame.complex_lighting { f64::from(0.65_f32) } else { 1.0 };
                            layer.tint =
                                ((((layer.tint >> 16) & 255) as f64 * red).trunc() as u32) << 16;
                        }
                        let own_tint = projectile.kind() == EnemyProjectileKind::Firebolt
                            || (projectile.kind() == EnemyProjectileKind::Arrow
                                && state.payload == EnemyProjectilePayload::Fire
                                && index > 0);
                        group.draws.push(EnemyDraw::Sprite { layer, region_tint: !own_tint });
                    }
                }
            }
            self.push_root(
                state.painter_registration,
                state.position.y,
                0.0,
                Some(state.position),
                group,
            );
        }
    }

    pub(super) fn prepare_effects(&mut self, world: &EnemyWorld, frame: &EnemyViewFrame<'_>) {
        for effect in &world.projectile_effects {
            if effect.kind == EnemyProjectileEffectKind::FireboltTrail {
                continue;
            }
            let sample = frame
                .presentation
                .and_then(|metadata| metadata.projectile_effects.get(&effect.state.id))
                .copied()
                .unwrap_or(NativeEnemyEffectPresentationInput {
                    age_ticks: effect.state.age_ticks as f64,
                    rotation_radians: effect.state.rotation_deg * std::f64::consts::PI / 180.0,
                });
            let plan = native_enemy_projectile_effect_plan_with_input(
                effect,
                (frame.point_gain)(effect.state.position),
                sample,
            );
            let bypass = enemy_projectile_effect_bypasses_world_tint(&effect.kind);
            let group = EnemyDrawGroup::sprites(plan.position, plan.layers, !bypass);
            if let Some(glow) = plan.ground_glow {
                let layer = EnemySpriteLayer {
                    alpha: glow.alpha,
                    scale: glow.scale,
                    tint: glow.tint,
                    ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 15, "enemy-fire-ground-glow")
                };
                self.pre_world.push(EnemyDrawGroup::sprites(glow.position, vec![layer], false));
            }
            match effect.kind {
                EnemyProjectileEffectKind::DemonExplosionArray
                | EnemyProjectileEffectKind::PoisonBubble { .. } => self.pre_world.push(group),
                EnemyProjectileEffectKind::DemonExplosionCore => self.post_world.push(group),
                _ => {
                    let bias = match effect.kind {
                        EnemyProjectileEffectKind::FireBurst => 50.0,
                        EnemyProjectileEffectKind::DemonBombParticle => 30.0,
                        EnemyProjectileEffectKind::GuidedImpact => 100.0,
                        _ => 0.0,
                    };
                    self.push_root(
                        effect.state.painter_registration,
                        effect.state.position.y,
                        bias,
                        if bypass { None } else { Some(effect.state.position) },
                        group,
                    );
                }
            }
        }
        for effect in world.death_effects.iter().chain(frame.extra_death_effects) {
            if effect.spawn_tick > world.last_step_tick.unwrap_or(0) {
                continue;
            }
            let sample = frame
                .presentation
                .and_then(|metadata| metadata.death_effects.get(&effect.id))
                .copied()
                .unwrap_or(NativeEnemyEffectPresentationInput {
                    age_ticks: effect.age_ticks as f64,
                    rotation_radians: effect.rotation_deg * std::f64::consts::PI / 180.0,
                });
            let plan = native_enemy_death_effect_plan_with_input(effect, frame.view_height, sample);
            let mut group = EnemyDrawGroup::at(effect.position);
            group.draws.extend(plan.gradients.into_iter().map(EnemyDraw::Gradient));
            for layer in plan.layers {
                group.draws.push(EnemyDraw::Sprite { layer, region_tint: false });
            }
            match effect.presentation_owner {
                DeathPresentationOwner::Background => self.background.push(group),
                DeathPresentationOwner::PreWorldQueue => self.pre_world.push(group),
                DeathPresentationOwner::DirectPostWorld => self.post_world.push(group),
                DeathPresentationOwner::LateWorldOverlay => self.late_world.push(group),
                DeathPresentationOwner::WorldSorted => self.push_root(
                    effect
                        .painter_registration
                        .expect("world-sorted enemy death effect requires its registration"),
                    effect.position.y,
                    effect.painter_sort_bias.unwrap_or(0.0),
                    None,
                    group,
                ),
            }
        }
    }

    pub(super) fn prepare_boss_spells(&mut self, world: &EnemyWorld, frame: &EnemyViewFrame<'_>) {
        self.rain_clouds
            .retain(|id, _| world.boss_spells.iter().any(|spell| spell.owner.id == *id));
        for spell in world.boss_spells.iter() {
            if matches!(spell.kind, BossSpellKind::Blightning { .. }) {
                let layers =
                    solomon_sim::enemies::lightning_presentation::native_dark_lightning_body(
                        &spell,
                    );
                self.append_lightning_body(
                    spell.owner.position,
                    &layers,
                    spell.owner.painter_registration,
                    true,
                );
                continue;
            }
            let plan = native_boss_spell_plan(&spell, frame.tick, frame.view_height);
            let mut group = EnemyDrawGroup::at(spell.owner.position);
            group.draws.extend(plan.gradients.into_iter().map(EnemyDraw::Gradient));
            if let BossSpellKind::MouthBeamSegment { vertices, uv_offset, start_alpha, end_alpha } =
                *spell.kind
            {
                group.draws.push(EnemyDraw::MouthBeam {
                    vertices,
                    uv_offset,
                    start_alpha,
                    end_alpha,
                });
            }
            if matches!(spell.kind, BossSpellKind::RainOfBones { .. }) {
                let rain = self
                    .rain_clouds
                    .entry(spell.owner.id)
                    .or_insert_with(|| NativeRainCloud::new(&spell, frame.tick));
                for layer in rain.update(&spell, frame.tick) {
                    group.draws.push(EnemyDraw::Sprite { layer, region_tint: false });
                }
            }
            for layer in plan.layers {
                group.draws.push(EnemyDraw::Sprite { layer, region_tint: false });
            }
            let bias = match *spell.kind {
                BossSpellKind::UltraBanish { .. } => 50.0,
                BossSpellKind::GreenFire { .. } => -25.0,
                BossSpellKind::HeartmongerFlicker { .. } => -200.0,
                BossSpellKind::DeathMagic { painter_sort_bias, .. } => painter_sort_bias,
                _ => 0.0,
            };
            if matches!(spell.kind, BossSpellKind::RainOfBones { .. }) {
                let mut owner = spell.owner.painter_registration.layer(spell.owner.position.y, 0.0);
                owner.visible = false;
                self.roots.push(EnemyRoot {
                    painter: EnemyPainter { layer: owner, region_light_point: None },
                    group: EnemyDrawGroup::at(spell.owner.position),
                });
                let mut insertion =
                    spell.owner.painter_registration.layer(spell.owner.position.y + 350.0, 0.0);
                insertion.is_insertion = true;
                self.roots.push(EnemyRoot {
                    painter: EnemyPainter { layer: insertion, region_light_point: None },
                    group,
                });
            } else {
                self.push_root(
                    spell.owner.painter_registration,
                    spell.owner.position.y,
                    bias,
                    None,
                    group,
                );
            }
        }
        self.prepare_mage_lightning(world, frame);
    }

    pub(super) fn prepare_spider_webs(&mut self, world: &EnemyWorld, frame: &EnemyViewFrame<'_>) {
        for silk in &world.silks {
            let mut rng = NativeRng::new((silk.id as u64 ^ frame.application_tick) as i64);
            let mesh = native_silk_mesh(&silk.state, frame.light_at, &mut rng);
            let mut group = EnemyDrawGroup::at(Vec2::ZERO);
            group.draws.push(EnemyDraw::Web(mesh.mesh));
            self.push_root(silk.painter_registration, silk.state.position.y, 0.0, None, group);
        }
        for fragment in &world.silk_fragments {
            let mut group = EnemyDrawGroup::at(Vec2::ZERO);
            group.draws.push(EnemyDraw::Web(native_fade_line_mesh(&fragment.state)));
            self.pre_world.push(group);
        }
    }

    pub(super) fn prepare_crows_and_remains(
        &mut self,
        world: &EnemyWorld,
        frame: &EnemyViewFrame<'_>,
    ) {
        if let Some(metadata) = frame.presentation {
            for crow in &metadata.crows {
                let layer = EnemySpriteLayer {
                    offset: Vec2::new(0.0, crow.height),
                    tint: crow.tint,
                    ..EnemySpriteLayer::new(EnemyArtAtlas::Heartmonger, crow.entry, "crow")
                };
                self.post_world.push(EnemyDrawGroup::sprites(crow.position, vec![layer], false));
            }
        } else {
            for actor in world.actors.iter() {
                if let EnemyBrain::Heartmonger(brain) = &actor.brain {
                    for crow in &brain.crows {
                        self.append_crow(crow);
                    }
                }
            }
            for crow in &world.detached_crows {
                self.append_crow(&crow.flight);
            }
        }
        for actor in &world.spider_remains {
            let state = &actor.state;
            if let Some(decal) = &state.decal {
                let layer = EnemySpriteLayer {
                    alpha: decal.alpha,
                    scale: decal.scale,
                    rotation_radians: decal.rotation_deg * std::f64::consts::PI / 180.0,
                    ..EnemySpriteLayer::new(EnemyArtAtlas::DeadHawg, decal.entry, "spider-decal")
                };
                self.background.push(EnemyDrawGroup::sprites(decal.position, vec![layer], false));
            }
            let Some(entry) = native_dead_spider_entry(state) else { continue };
            if state.life < 10.0 {
                continue;
            }
            let body = EnemySpriteLayer {
                alpha: (state.life - 10.0).clamp(0.0, 1.0),
                ..EnemySpriteLayer::new(EnemyArtAtlas::DeadHawg, entry, "dead-spider")
            };
            let shadow = EnemySpriteLayer { tint: 0, offset: Vec2::new(0.0, 1.0), ..body.clone() };
            self.pre_world.push(EnemyDrawGroup::sprites(state.position, vec![shadow, body], false));
        }
    }

    fn append_crow(&mut self, crow: &solomon_sim::enemies::model::CrowState) {
        let Some(position) = crow.position else { return };
        let facing = (((crow.heading_deg + 10.0) / 20.0).trunc() as i64).rem_euclid(18) as usize;
        let channel = (crow.light_intensity.clamp(0.0, 1.0) * 255.0).round() as u32;
        let layer = EnemySpriteLayer {
            offset: Vec2::new(0.0, -crow.height),
            tint: channel * 0x010101,
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::Heartmonger,
                2 + crow.flap_phase.trunc() as usize * 18 + facing,
                "crow",
            )
        };
        self.post_world.push(EnemyDrawGroup::sprites(position, vec![layer], false));
    }

    pub(super) fn push_root(
        &mut self,
        registration: PainterRegistration,
        world_y: f64,
        sort_bias: f64,
        region_light_point: Option<Vec2>,
        group: EnemyDrawGroup,
    ) {
        self.roots.push(EnemyRoot {
            painter: EnemyPainter {
                layer: registration.layer(world_y, sort_bias),
                region_light_point,
            },
            group,
        });
    }
}
