//! Boneyard presentation over accepted session state. Views retain GPU resources and
//! paint-time effects; the session remains the sole owner of actors, clocks and registrations.

use std::collections::BTreeMap;

use solomon_sim::boneyard::BoneyardWorld;
use solomon_sim::boneyard::arena::ArenaPhase;
use solomon_sim::boneyard::ground_glyph::GroundGlyphPlan;
use solomon_sim::boneyard::lighting::{
    BoneyardLightIndex, BoneyardLightManagerView, PlayerLightOwner, boneyard_light_tint,
    lantern_light_source, player_light_source,
};
use solomon_sim::boneyard::presentation::{
    ArenaDisplacementCover, arena_displacement_cover, enemy_world_feedback_transform,
};
use solomon_sim::boneyard::shadows::{ShadowCaster, shadow_records};
use solomon_sim::boneyard::solomon_presentation::{
    SolomonPainterKind, SolomonSetPieceLighting, solomon_set_piece_lighting,
};
use solomon_sim::boneyard::world_light::{
    BoneyardEntityLightPresentation, BoneyardLightEnvironment, WorldLightOwner, WorldLightPlayer,
    collect_boneyard_entity_lights, ordered_boneyard_light_sources, region_point_gain,
};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::crow_blindness::crow_blindness_frame;
use solomon_sim::enemies::EnemyToken;
use solomon_sim::enemies::model::{EnemyActor, EnemyEvent};
use solomon_sim::enemies::puppet_hits::EnemyPuppetHitKind;
use solomon_sim::enemies::underlay::enemy_directional_shadow_family;
use solomon_sim::hub::painter::PainterLayer;
use solomon_sim::modding::inventory::{GameSimulationModEffect, ModConsumableCatalogEntry};
use solomon_sim::painter::PainterRegistration;
use solomon_sim::painter::overlays::PainterOwnerOverlay;
use solomon_sim::player::PlayerState;
use solomon_sim::player_presentation::LevelUpPresentationFrame;
use solomon_sim::presentation::{native_puppet_hit_tint, player_material_tint};
use solomon_sim::profile::WeldBuildId;
use solomon_sim::screen_feedback::{present_secondary_screen_overlay, secondary_world_shake};
use solomon_sim::secondary::presentation::{SecondaryAtlas, SecondaryBlend, SecondarySpriteDraw};
use solomon_sim::secondary::{
    SecondaryActorKind, SecondarySimulationState, secondary_target_effect,
    secondary_target_material_tint,
};
use solomon_sim::settings::{GameSettings, game_light_quality};
use solomon_sim::spells::model::SpellState;
use solomon_sim::spells::presentation::SpellDrawQueue;
use solomon_sim::{HubWizard, Vec2};

use crate::boneyard::{BoneyardFrame, BoneyardGoodieShadow, BoneyardPass, BoneyardView};
use crate::boneyard_weather::BoneyardWeatherView;
use crate::compact_masks::{CompactMaskPlayer, CompactMaskView, draw_environment_aperture};
use crate::courtyard::{ActorPainters, Camera};
use crate::enemies::{EnemyView, EnemyViewFrame, EnemyViewPlayer, EnemyViewShadow};
use crate::loot::{LootPainterId, LootView};
use crate::mod_consumable_effects::ModConsumableEffectsView;
use crate::mod_presentation_assets::ModPresentationAssets;
use crate::player_effects::{PlayerEffectPainter, PlayerEffectPlayer, PlayerEffectsView};
use crate::presentation::{EnemyPresentationMetadata, PresentationClocks, PresentationTimeline};
use crate::secondary::{SecondaryPuppetHit, SecondaryView};
use crate::solomon::{SolomonView, SolomonViewFrame};
use crate::spells::SpellView;
use crate::wizard::{WizardEquipment, WizardPlayerPresentation, WizardView};
use crate::world_screen_feedback::WorldScreenFeedbackSample;
use solomon_graphics::Renderer;

/// Scenery and its light target have the same lifetime as one loaded map.
pub struct BattleScene {
    pub scenery: BoneyardView,
    pub lights: BoneyardLightIndex,
    pub weather: BoneyardWeatherView,
    pub compact_masks: CompactMaskView,
    /// Frames drawn for this map, counted independently of snapshot ticks.
    pub compact_frame_count: u64,
}

/// Borrowed facts from one accepted session snapshot, with the current presentation camera.
pub struct BattleFrame<'a> {
    pub world: &'a BoneyardWorld,
    pub presentation_tick: f64,
    pub stopped_world_tick: f64,
    pub clocks: Option<&'a PresentationClocks>,
    pub enemy_presentation: Option<&'a EnemyPresentationMetadata>,
    pub local_player_id: &'a str,
    pub camera_focus_player_id: Option<&'a str>,
    pub scenery_timeline: Option<&'a PresentationTimeline>,
    pub players: &'a [BattlePlayer<'a>],
    pub spells: &'a SpellState,
    pub secondary: &'a SecondarySimulationState,
    pub mod_effects: &'a [GameSimulationModEffect],
    pub mod_catalog: &'a [ModConsumableCatalogEntry],
    pub camera: &'a Camera,
    pub viewport: Vec2,
    pub settings: &'a GameSettings,
    pub enhanced_effects: bool,
    pub screen_feedback: WorldScreenFeedbackSample,
    pub application_time_ms: f64,
    pub level_up_barrier_active: bool,
    pub device_resolution: f64,
}

/// One participant's borrowed state for drawing a Boneyard frame.
pub struct BattlePlayer<'a> {
    pub id: &'a str,
    pub player: &'a PlayerState,
    pub wizard: &'a HubWizard,
    pub light_registration: PainterRegistration,
    pub overlay_effect_phase: f64,
    pub drive_active: bool,
    pub materializing: bool,
    pub blindness_ticks_remaining: f64,
    pub level_up: Option<&'a LevelUpPresentationFrame>,
}

/// Shared atlas uploads survive new runs. Every effect cache resets at a world replacement.
pub struct BattleViews {
    pub enemies: EnemyView,
    pub secondary: SecondaryView,
    pub solomon: SolomonView,
    pub loot: LootView,
    pub player_effects: PlayerEffectsView,
    pub mod_effects: ModConsumableEffectsView,
}

enum BattlePainter {
    Player(usize),
    Primary(usize),
    Secondary(usize),
    Enemy(usize),
    Solomon(SolomonPainterKind),
    Loot(usize),
    PlayerEffect(usize),
    Staff(usize),
}

/// Facts about one Boneyard frame that its preparation steps share: the accepted frame and
/// its local participant, the tick, the frame compact masks and effects animate on, the
/// world key and the unshaken camera's center.
struct FrameFacts<'a> {
    frame: &'a BattleFrame<'a>,
    local: &'a BattlePlayer<'a>,
    tick: u64,
    compact_frame: u64,
    world_key: &'a str,
    camera_center: Vec2,
}

/// The frame's actor painters in queue order, with the owner overlays, live views and Goodie
/// shadows the scenery draws beside them.
struct ActorQueue<'a> {
    painters: Vec<(PainterLayer, BattlePainter, Option<Vec2>)>,
    overlays: Vec<PainterOwnerOverlay>,
    live_views: Vec<(u64, u64)>,
    goodie_shadows: Vec<BoneyardGoodieShadow<'a>>,
}

/// The world camera after screen shake, and the cover over the edge the shake exposes.
struct WorldShake {
    camera: Camera,
    cover: Option<ArenaDisplacementCover>,
}

/// What the scenery's passes draw from: the prepared views, the shaken camera, the light
/// tint, and each participant's equipment, ground glyph and following effects.
struct WorldPasses<'a> {
    frame: &'a BattleFrame<'a>,
    camera: &'a Camera,
    spells: &'a SpellView,
    compact_masks: &'a CompactMaskView,
    weather: &'a BoneyardWeatherView,
    lights: &'a BoneyardLightIndex,
    enemies: &'a EnemyView,
    secondary: &'a SecondaryView,
    solomon: &'a SolomonView,
    loot: &'a LootView,
    player_effects: &'a PlayerEffectsView,
    equipment: &'a [WizardEquipment],
    ground_light_players: &'a [CompactMaskPlayer<'a>],
    player_ground_plans: &'a [Option<GroundGlyphPlan>],
    painters: &'a [(PainterLayer, BattlePainter, Option<Vec2>)],
    player_followers: &'a [(usize, &'a PlayerEffectPainter)],
    cover: Option<ArenaDisplacementCover>,
    tint_at: &'a dyn Fn(Vec2) -> u32,
}

impl BattleViews {
    pub async fn load(renderer: &Renderer, spells: &SpellView) -> Result<Self, String> {
        let bad_guys = spells.bad_guys();
        let dead_hawg = spells.dead_hawg();
        let enemies = EnemyView::load(
            renderer,
            bad_guys.records.clone(),
            bad_guys.texture.clone(),
            dead_hawg.records.clone(),
            dead_hawg.texture.clone(),
        )
        .await?;
        let secondary = SecondaryView::load(renderer, spells, enemies.demon_atlas()).await?;
        Ok(Self {
            enemies,
            secondary,
            solomon: SolomonView::load(
                renderer,
                &bad_guys.records,
                bad_guys.texture.clone(),
                &dead_hawg.records,
                dead_hawg.texture.clone(),
            )
            .await?,
            loot: LootView::default(),
            player_effects: PlayerEffectsView::load(renderer).await?,
            mod_effects: ModConsumableEffectsView::new(renderer)?,
        })
    }

    pub fn reset(&mut self) {
        self.enemies.reset();
        self.secondary.memory.fire_explosion_point_gains.clear();
        self.solomon.reset();
        self.loot.reset();
        self.player_effects.reset();
        self.mod_effects.reset();
    }

    /// Consume every accepted tick, including ticks between rendered frames.
    pub fn consume_events(&mut self, events: &[EnemyEvent]) {
        self.enemies.consume_events(events);
    }

    #[allow(clippy::too_many_arguments)] // One shared renderer, scene, atlas owners and borrowed frame.
    pub fn draw(
        &mut self,
        renderer: &mut Renderer,
        scene: &mut BattleScene,
        spells: &mut SpellView,
        wizard: &mut WizardView,
        mut mod_assets: Option<&mut ModPresentationAssets>,
        frame: &BattleFrame<'_>,
        mut draw_world_overlays: impl FnMut(&mut Renderer, &Camera),
    ) -> Result<(), String> {
        let world = frame.world;
        let mut equipment = Vec::with_capacity(frame.players.len());
        for participant in frame.players {
            equipment.push(WizardEquipment::prepare(
                renderer,
                participant.player.selection.element,
                &participant.player.economy.equipment,
                mod_assets.as_deref_mut(),
            )?);
        }
        let local = frame
            .players
            .iter()
            .find(|player| player.id == frame.local_player_id)
            .ok_or("battle presentation is missing its local participant")?;
        let camera = frame.camera;
        let viewport = frame.viewport;
        advance_scenery(&mut scene.scenery, frame);
        let tick = world.ticks;
        scene.compact_frame_count += 1;
        let compact_frame =
            if frame.level_up_barrier_active { tick } else { scene.compact_frame_count };
        let world_key = format!("boneyard:{}", world.loaded.run_id);
        let camera_center = Vec2::new(
            camera.origin.x + viewport.x / (2.0 * camera.scale),
            camera.origin.y + viewport.y / (2.0 * camera.scale),
        );
        let facts =
            FrameFacts { frame, local, tick, compact_frame, world_key: &world_key, camera_center };
        let point_gain = |position| {
            region_point_gain(
                position,
                camera_center,
                viewport.x / camera.scale,
                local.player.combat.life_state != PlayerLifeState::Alive,
            )
        };
        self.prepare_spells(renderer, spells, &facts, &point_gain)?;
        let light_players = light_players(frame);
        self.rebuild_lights(renderer, scene, spells, &facts, &light_players, &point_gain)?;
        let light_at = |position| {
            if frame.settings.complex_lighting { scene.lights.scalar_at(position) } else { 1.0 }
        };
        prepare_weather(&mut scene.weather, &facts, &light_at);
        let ground_light_players: Vec<_> = light_players
            .iter()
            .map(|player| CompactMaskPlayer { id: player.id, position: player.position })
            .collect();
        let player_ground_plans = prepare_ground(
            renderer,
            &mut scene.compact_masks,
            spells,
            &facts,
            &ground_light_players,
        )?;
        let tint_at = |position| boneyard_light_tint(light_at(position));
        self.prepare_player_effects(frame, &tint_at);
        self.prepare_enemies(&facts, &scene.lights, &light_at, &tint_at, &point_gain);
        self.prepare_secondary_puppet_hits(renderer, &facts)?;
        self.loot.prepare(
            &world.loot,
            frame.presentation_tick,
            &world.enemies.puppet_hits,
            frame.clocks.map(|clocks| &clocks.puppet_hits),
            frame.settings.complex_lighting,
            frame.mod_catalog,
            mod_assets,
        )?;
        self.prepare_solomon(&facts, &tint_at);
        let ActorQueue { painters, overlays, live_views, goodie_shadows } =
            self.actor_queue(spells, &facts);
        // These roots follow the assigned player depth without entering the native
        // painter queue. They precede primary overlays promoted to owner depth + 0.5.
        let mut player_followers: Vec<_> =
            self.player_effects.painters().filter(|(_, painter)| painter.layer.is_none()).collect();
        player_followers.sort_by(|(_, left), (_, right)| {
            left.owner_depth_offset.total_cmp(&right.owner_depth_offset)
        });
        let layers: Vec<_> = painters.iter().map(|(layer, _, _)| *layer).collect();
        let WorldShake { camera, cover } = world_shake(&facts);
        let camera = &camera;
        let tree_wobbles = tree_wobbles(&facts);
        let passes = WorldPasses {
            frame,
            camera,
            spells,
            compact_masks: &scene.compact_masks,
            weather: &scene.weather,
            lights: &scene.lights,
            enemies: &self.enemies,
            secondary: &self.secondary,
            solomon: &self.solomon,
            loot: &self.loot,
            player_effects: &self.player_effects,
            equipment: &equipment,
            ground_light_players: &ground_light_players,
            player_ground_plans: &player_ground_plans,
            painters: &painters,
            player_followers: &player_followers,
            cover,
            tint_at: &tint_at,
        };
        scene.scenery.draw(
            renderer,
            &BoneyardFrame {
                camera,
                logical_view: viewport,
                lights: &scene.lights,
                reference_y: local.wizard.position.y,
                gates: &world.gate_leaves,
                presentation_frame: compact_frame,
                presentation_tick: frame.presentation_tick,
                stopped_world_tick: frame.stopped_world_tick,
                puppet_hits: &world.enemies.puppet_hits,
                clocks: frame.clocks,
                tree_wobbles: &tree_wobbles,
                cleanup_bounds: world.cleanup_bounds(),
                enhanced_effects: frame.enhanced_effects,
                complex_lighting: frame.settings.complex_lighting,
                complex_shadows: frame.settings.complex_shadows,
                special_surface_at: &|position| {
                    scene.compact_masks.special_surface_at(
                        position,
                        &world.enemies.spider_remains,
                        spells,
                    )
                },
                goodies: &goodie_shadows,
            },
            &ActorPainters { layers: &layers, owner_overlays: &overlays, live_views: &live_views },
            |renderer, pass| match pass {
                BoneyardPass::PreWorld => passes.draw_pre_world(renderer, wizard),
                BoneyardPass::PostWorld => passes.draw_post_world(renderer, &mut self.mod_effects),
                BoneyardPass::LightComposite => passes.draw_displacement_cover(renderer),
                BoneyardPass::Actor(index) => passes.draw_actor(renderer, wizard, index),
            },
        );
        scene.weather.draw_streaks(renderer, camera);
        draw_world_overlays(renderer, camera);
        self.draw_screen_overlays(renderer, frame, local);
        Ok(())
    }

    /// The primary spells' and secondary abilities' draws for this world.
    fn prepare_spells(
        &mut self,
        renderer: &mut Renderer,
        spells: &mut SpellView,
        facts: &FrameFacts<'_>,
        point_gain: &dyn Fn(Vec2) -> f64,
    ) -> Result<(), String> {
        let FrameFacts { frame, compact_frame, world_key, .. } = *facts;
        let world = frame.world;
        let camera = frame.camera;
        let viewport = frame.viewport;
        spells.prepare(
            frame.spells,
            world_key,
            compact_frame,
            point_gain,
            &world.enemies.puppet_hits,
            frame.clocks,
            frame.enhanced_effects,
            Some(solomon_sim::spells::weld::FlameLashBounds {
                x: camera.origin.x,
                y: camera.origin.y,
                width: viewport.x / camera.scale,
                height: viewport.y / camera.scale,
            }),
        );
        self.secondary.prepare(
            renderer,
            frame.secondary.actors.iter(),
            world_key,
            Some(compact_frame as f64),
            frame.clocks.map(|clocks| &clocks.secondary),
            point_gain,
            frame.enhanced_effects,
        )?;
        Ok(())
    }

    /// Collect this frame's light sources in the light manager's order, then rebuild the
    /// light field at the current quality.
    fn rebuild_lights(
        &self,
        renderer: &Renderer,
        scene: &mut BattleScene,
        spells: &SpellView,
        facts: &FrameFacts<'_>,
        light_players: &[WorldLightPlayer<'_>],
        point_gain: &dyn Fn(Vec2) -> f64,
    ) -> Result<(), String> {
        let FrameFacts { frame, tick, compact_frame, camera_center, .. } = *facts;
        let world = frame.world;
        let camera = frame.camera;
        let viewport = frame.viewport;
        let (mut providers, mut misc) = collect_boneyard_entity_lights(
            &world.loaded.run_id,
            light_players,
            &world.enemies,
            tick,
            BoneyardLightEnvironment {
                primary_spells: Some(frame.spells),
                secondary_abilities: Some(frame.secondary),
            },
            BoneyardEntityLightPresentation {
                frame: compact_frame,
                primary_age_ticks: frame.clocks.map(|clocks| &clocks.primary),
                secondary_age_ticks: frame.clocks.map(|clocks| &clocks.secondary),
                multiple_shadows: frame.settings.multiple_shadows,
                point_gain_at: point_gain,
                fire_explosion_point_gain: &|id, position| {
                    spells.fire_explosion_point_gain(id).unwrap_or_else(|| point_gain(position))
                },
                ring_fire_explosion_point_gain: &|id, position| {
                    self.secondary
                        .memory
                        .fire_explosion_point_gains
                        .get(&id)
                        .copied()
                        .unwrap_or_else(|| point_gain(position))
                },
            },
        );
        for player in frame.players.iter().filter(|player| !player.materializing) {
            if let Some(mut source) = player_light_source(
                PlayerLightOwner {
                    id: player.id,
                    position: player.wizard.position,
                    heading_index: player.wizard.heading_index,
                    drive_active: player.drive_active,
                    overlay_effect_phase: player.overlay_effect_phase,
                },
                compact_frame,
                player.id == frame.local_player_id,
            ) {
                if player.id == frame.local_player_id
                    && let Some(level_up) = player.level_up
                    && let Some(radius) =
                        solomon_sim::player_presentation::level_up_player_light_radius(
                            player.overlay_effect_phase,
                            level_up,
                        )
                {
                    source.radius = radius;
                }
                providers.push(WorldLightOwner {
                    registration: player.light_registration,
                    sources: vec![source],
                });
            }
        }
        if let Some(position) = world.lantern_position {
            providers.push(WorldLightOwner {
                registration: world
                    .lantern_light_registration
                    .expect("active Lantern has its manager registration"),
                sources: vec![lantern_light_source(
                    position,
                    compact_frame,
                    frame.settings.multiple_shadows,
                )],
            });
        }
        let (sources, tail) = ordered_boneyard_light_sources(&mut providers, &mut misc);
        let quality = game_light_quality(frame.settings);
        scene.scenery.set_light_quality(renderer, viewport, frame.device_resolution, quality)?;
        scene.lights.rebuild(
            &sources,
            &tail,
            BoneyardLightManagerView {
                camera_position: camera_center,
                zoom: camera.scale,
                viewport,
            },
            quality,
        );
        Ok(())
    }

    fn prepare_player_effects(&mut self, frame: &BattleFrame<'_>, tint_at: &dyn Fn(Vec2) -> u32) {
        let world = frame.world;
        let effect_players: Vec<_> = frame
            .players
            .iter()
            .map(|participant| PlayerEffectPlayer {
                id: participant.id,
                player: participant.player,
                wizard: participant.wizard,
                material_tint: player_material_tint(
                    tint_at(participant.wizard.position),
                    frame.secondary.players.get(participant.id),
                    &participant.player.combat,
                ),
                level_up: participant.level_up,
            })
            .collect();
        self.player_effects.prepare(
            &world.loaded.run_id,
            frame.presentation_tick,
            &effect_players,
            &world.death_weapons,
        );
    }

    fn prepare_enemies(
        &mut self,
        facts: &FrameFacts<'_>,
        lights: &BoneyardLightIndex,
        light_at: &dyn Fn(Vec2) -> f64,
        tint_at: &dyn Fn(Vec2) -> u32,
        point_gain: &dyn Fn(Vec2) -> f64,
    ) {
        let FrameFacts { frame, tick, compact_frame, world_key, .. } = *facts;
        let world = frame.world;
        let viewport = frame.viewport;
        let actor_tint = |actor: &EnemyActor| {
            secondary_target_material_tint(
                tint_at(actor.position),
                secondary_target_effect(&frame.secondary.target_effects, world_key, actor.id),
            )
        };
        let actor_shadows = |actor: &EnemyActor| {
            let enabled = enemy_directional_shadow_family(actor.config.enemy_token)
                && (frame.settings.complex_shadows
                    || actor.config.enemy_token == EnemyToken::Heartmonger);
            EnemyViewShadow {
                records: if enabled {
                    shadow_records(
                        &ShadowCaster {
                            id: format!("enemy:{}", actor.id),
                            position: actor.position,
                            outline: Vec::new(),
                            program: None,
                        },
                        lights,
                        compact_frame,
                    )
                } else {
                    Vec::new()
                },
                light_scalar: light_at(actor.position),
            }
        };
        let players: Vec<_> = frame
            .players
            .iter()
            .map(|player| EnemyViewPlayer {
                id: player.id,
                position: player.wizard.position,
                primary_tint: solomon_sim::profile::primary::primary_spell_tint(
                    player.player.book.primary_skill_id,
                    player.player.book.weld_build_id.map(WeldBuildId::id),
                ),
            })
            .collect();
        self.enemies.prepare(
            &world.enemies,
            &EnemyViewFrame {
                tick,
                presentation: frame.enemy_presentation,
                puppet_hit_alphas: frame.clocks.map(|clocks| &clocks.puppet_hits),
                extra_death_effects: &world.loot.effects,
                application_tick: (frame.application_time_ms / 10.0).floor() as u64,
                complex_lighting: frame.settings.complex_lighting,
                complex_shadows: frame.settings.complex_shadows,
                enhanced_effects: frame.enhanced_effects,
                view_height: viewport.y,
                players: &players,
                light_at,
                point_gain,
                actor_shadows: &actor_shadows,
                actor_tint: &actor_tint,
            },
        );
    }

    /// The puppet-hit flash on each of this world's Leviathans.
    fn prepare_secondary_puppet_hits(
        &mut self,
        renderer: &mut Renderer,
        facts: &FrameFacts<'_>,
    ) -> Result<(), String> {
        let FrameFacts { frame, world_key, .. } = *facts;
        let world = frame.world;
        let mut puppet_hits = BTreeMap::new();
        for actor in frame.secondary.actors.iter() {
            if actor.world_key != world_key || actor.kind != SecondaryActorKind::Leviathan {
                continue;
            }
            let target_id = format!("secondary:{}", actor.id);
            if let Some(hit) =
                world.enemies.puppet_hits.iter().find(|hit| {
                    hit.kind == EnemyPuppetHitKind::Leviathan && hit.target_id == target_id
                })
            {
                puppet_hits.insert(
                    actor.id,
                    SecondaryPuppetHit {
                        alpha: frame
                            .clocks
                            .and_then(|clocks| clocks.puppet_hits.get(&target_id))
                            .copied()
                            .unwrap_or_else(|| hit.feedback.alpha(frame.presentation_tick)),
                        tint: native_puppet_hit_tint(frame.settings.complex_lighting, 0xffffff),
                    },
                );
            }
        }
        self.secondary.prepare_puppet_hits(renderer, &puppet_hits)?;
        Ok(())
    }

    /// Solomon's dig, lit at his and the Lantern's positions when lighting is complex.
    fn prepare_solomon(&mut self, facts: &FrameFacts<'_>, tint_at: &dyn Fn(Vec2) -> u32) {
        let FrameFacts { frame, tick, .. } = *facts;
        let world = frame.world;
        if let Some(dig) = &world.loaded.scene.solomon_dig {
            let lighting = if frame.settings.complex_lighting {
                solomon_set_piece_lighting(
                    world.encounter.as_ref().map_or(dig.position, |encounter| encounter.position),
                    world.lantern_position.unwrap_or(dig.lantern_position),
                    tint_at,
                )
            } else {
                SolomonSetPieceLighting {
                    body_tint: 0xffffff,
                    dirt_tint: 0xffffff,
                    lantern_tint: 0xffffff,
                }
            };
            self.solomon.prepare(
                dig,
                world.encounter.as_ref(),
                &SolomonViewFrame {
                    tick,
                    lantern_position: world.lantern_position,
                    lantern_registration: world
                        .lantern_light_registration
                        .expect("Solomon Lantern retains its registration"),
                    solomon_registration: world
                        .solomon_painter_registration
                        .expect("Solomon retains his registration"),
                    lighting,
                },
            );
        }
    }

    /// Every actor painter in queue order: participants, primary spells, secondary abilities,
    /// enemies, Solomon, loot, player effects, then staff actions.
    fn actor_queue<'a>(&self, spells: &SpellView, facts: &FrameFacts<'a>) -> ActorQueue<'a> {
        let FrameFacts { frame, world_key, .. } = *facts;
        let world = frame.world;
        let mut painters: Vec<(PainterLayer, BattlePainter, Option<Vec2>)> = frame
            .players
            .iter()
            .enumerate()
            .filter(|(_, player)| !player.materializing)
            .map(|(index, player)| {
                (
                    // The third death pose sinks behind ordinary world actors.
                    player.light_registration.layer(
                        player.wizard.position.y,
                        if player.player.combat.death_frame() == Some(3) { -1000.0 } else { 0.0 },
                    ),
                    BattlePainter::Player(index),
                    None,
                )
            })
            .collect();
        let spell_start = painters.len();
        let overlays: Vec<_> = spells
            .painters()
            .enumerate()
            .filter_map(|(offset, (_, painter))| {
                let owner = painter.overlay_owner_id.as_ref()?;
                let owner_draw_id = painters.iter().position(|(_, painter, _)| {
                matches!(painter, BattlePainter::Player(index) if frame.players[*index].id == owner)
            })?;
                Some(PainterOwnerOverlay {
                    draw_id: spell_start + offset,
                    owner_draw_id,
                    view_id: painter.view_id,
                })
            })
            .collect();
        let live_views: Vec<_> = spells.live_view_creation_orders().collect();
        painters.extend(spells.painters().map(|(index, painter)| {
            (painter.layer, BattlePainter::Primary(index), painter.region_light_point)
        }));
        painters.extend(self.secondary.painters().map(|(index, painter)| {
            (painter.layer, BattlePainter::Secondary(index), painter.region_light_point)
        }));
        painters.extend(self.enemies.painters().map(|(index, painter)| {
            (painter.layer, BattlePainter::Enemy(index), painter.region_light_point)
        }));
        painters.extend(
            self.solomon
                .painters()
                .map(|(kind, layer)| (layer, BattlePainter::Solomon(kind), None)),
        );
        let mut goodie_shadows = Vec::new();
        for (index, painter) in self.loot.painters() {
            if let LootPainterId::Goodie(id) = painter.id {
                goodie_shadows.push(BoneyardGoodieShadow {
                    actor_index: painters.len(),
                    goodie: world
                        .loot
                        .goodies
                        .iter()
                        .find(|goodie| goodie.id == id)
                        .expect("prepared Goodie has its actual store owner"),
                });
            }
            painters.push((painter.layer, BattlePainter::Loot(index), Some(painter.position)));
        }
        painters.extend(self.player_effects.painters().filter_map(|(index, painter)| {
            painter.layer.map(|layer| {
                (layer, BattlePainter::PlayerEffect(index), painter.region_light_point)
            })
        }));
        painters.extend(frame.spells.transients.iter().enumerate().filter_map(
            |(index, effect)| {
                if effect.common().world_key != world_key {
                    return None;
                }
                let solomon_sim::spells::model::PrimarySpellTransient::Staff(staff) = effect else {
                    return None;
                };
                SecondaryView::staff_painter(staff).map(|painter| {
                    (painter.layer, BattlePainter::Staff(index), painter.region_light_point)
                })
            },
        ));
        ActorQueue { painters, overlays, live_views, goodie_shadows }
    }

    /// Over the finished world: the secondary abilities' screen flash, then crow blindness.
    fn draw_screen_overlays(
        &self,
        renderer: &mut Renderer,
        frame: &BattleFrame<'_>,
        local: &BattlePlayer<'_>,
    ) {
        let viewport = frame.viewport;
        if let Some(overlay) = present_secondary_screen_overlay(
            frame.screen_feedback.overlay,
            frame.settings.reduced_screen_flashes,
        ) {
            renderer.draw_solid(
                [0.0, 0.0, viewport.x as f32, viewport.y as f32],
                [
                    ((overlay.color >> 16) & 255) as f32 / 255.0,
                    ((overlay.color >> 8) & 255) as f32 / 255.0,
                    (overlay.color & 255) as f32 / 255.0,
                    overlay.alpha as f32,
                ],
            );
        }
        if let Some(blindness) = crow_blindness_frame(
            local.blindness_ticks_remaining,
            frame.application_time_ms,
            viewport,
        ) {
            renderer.draw_solid(
                [0.0, 0.0, viewport.x as f32, viewport.y as f32],
                [0.0, 0.0, 0.0, blindness.cover_alpha as f32],
            );
            self.secondary.draw_sprite(
                renderer,
                &Camera { origin: Vec2::ZERO, scale: 1.0 },
                blindness.stamp_position,
                &SecondarySpriteDraw {
                    alpha: blindness.stamp_alpha,
                    atlas: SecondaryAtlas::DeadHawg,
                    blend: SecondaryBlend::Normal,
                    color_mode: None,
                    clip: None,
                    entry: 1,
                    offset: Vec2::ZERO,
                    role: "crow-blindness-stamp".to_owned(),
                    rotation_radians: 0.0,
                    scale_x: 1.25,
                    scale_y: 1.25,
                    tint: 0xffffff,
                },
                [1.0; 3],
            );
        }
    }
}

/// Advance the scenery's trees to this frame.
fn advance_scenery(scenery: &mut BoneyardView, frame: &BattleFrame<'_>) {
    let world = frame.world;
    let camera = frame.camera;
    let viewport = frame.viewport;
    let actor_poses = crate::presentation::scenery_actor_poses(
        world,
        frame.players.iter().map(|player| solomon_sim::boneyard::scenery::TreePlayerPose {
            id: player.id.to_owned(),
            position: player.wizard.position,
        }),
        frame.enemy_presentation,
    );
    scenery.advance_to(&crate::boneyard::TreeFrameSource {
        run_id: &world.loaded.run_id,
        local_player_id: frame.local_player_id,
        focus_player_id: frame.camera_focus_player_id,
        display_tick: frame.presentation_tick,
        actor_poses: &actor_poses,
        camera_bounds: solomon_sim::boneyard::BoneyardBounds::new(
            camera.origin.x,
            camera.origin.y,
            viewport.x / camera.scale,
            viewport.y / camera.scale,
        ),
        camera_limits: world.camera_bounds(),
        viewport,
        zoom: camera.scale,
        timeline: frame.scenery_timeline,
    });
}

/// The participants who cast light: everyone but a wizard still materializing.
fn light_players<'a>(frame: &BattleFrame<'a>) -> Vec<WorldLightPlayer<'a>> {
    let viewport = frame.viewport;
    frame
        .players
        .iter()
        .filter(|player| !player.materializing)
        .map(|player| WorldLightPlayer {
            id: player.id,
            position: player.wizard.position,
            heading_index: player.wizard.heading_index,
            weapon_pulse: player.wizard.primary_cast.weapon_pulse,
            overlay_effect_phase: player.overlay_effect_phase,
            light_registration: Some(player.light_registration),
            viewport: Some(viewport),
        })
        .collect()
}

fn prepare_weather(
    weather: &mut BoneyardWeatherView,
    facts: &FrameFacts<'_>,
    light_at: &dyn Fn(Vec2) -> f64,
) {
    let FrameFacts { frame, tick, .. } = *facts;
    let world = frame.world;
    let camera = frame.camera;
    let viewport = frame.viewport;
    weather.prepare(
        tick,
        solomon_sim::boneyard::BoneyardBounds {
            x: camera.origin.x,
            y: camera.origin.y,
            width: viewport.x / camera.scale,
            height: viewport.y / camera.scale,
        },
        viewport.y / camera.scale,
        &|position, radius| {
            solomon_sim::boneyard::collision::boneyard_body_collides(
                position,
                &world.dynamic_collision,
                radius,
            )
        },
        light_at,
    );
}

/// Each participant's ground glyph, then this frame's compact ground masks.
fn prepare_ground(
    renderer: &mut Renderer,
    compact_masks: &mut CompactMaskView,
    spells: &SpellView,
    facts: &FrameFacts<'_>,
    ground_light_players: &[CompactMaskPlayer<'_>],
) -> Result<Vec<Option<GroundGlyphPlan>>, String> {
    let FrameFacts { frame, compact_frame, .. } = *facts;
    let world = frame.world;
    let mut ground_players: Vec<_> =
        frame.players.iter().filter(|player| !player.materializing).collect();
    ground_players.sort_by_key(|player| player.light_registration.registration_ordinal);
    let player_ground_plans: Vec<_> = ground_players
        .iter()
        .map(|participant| {
            solomon_sim::boneyard::ground_glyph::player_ground_glyph(
                solomon_sim::boneyard::ground_glyph::GroundPlayer {
                    position: participant.wizard.position,
                    heading_degrees: participant.wizard.heading_degrees,
                    scale: 1.0,
                    life_state: participant.player.combat.life_state,
                    death_tick: participant.player.combat.death_tick,
                    corpse_consumed: participant.player.combat.corpse_consumed,
                },
                compact_masks.special_surface_at(
                    participant.wizard.position,
                    &world.enemies.spider_remains,
                    spells,
                ),
            )
        })
        .collect();
    // Compact masks admit the full scene until the arena seals;
    // Tutorial camera cleanup and the rain viewport are separate bounds.
    let compact_bounds = world
        .arena_transition
        .as_ref()
        .filter(|transition| transition.phase == ArenaPhase::Sealed)
        .map_or(world.loaded.scene.bounds, |transition| transition.combat_bounds);
    compact_masks.prepare(
        renderer,
        spells,
        ground_light_players,
        &world.enemies.spider_remains,
        compact_bounds,
        compact_frame as f64,
    )?;
    Ok(player_ground_plans)
}

fn world_shake(facts: &FrameFacts<'_>) -> WorldShake {
    let FrameFacts { frame, local, world_key, camera_center, .. } = *facts;
    let world = frame.world;
    let camera = frame.camera;
    let viewport = frame.viewport;
    // Logical light admission uses the unshaken camera. Only the final world
    // transform changes, with the local wizard's screen position as its anchor.
    let magnitude = if frame.settings.zoom_effects {
        world.enemy_world_feedback.magnitude.max(frame.screen_feedback.camera_magnitude)
    } else {
        0.0
    };
    let displacement = if frame.settings.zoom_effects {
        secondary_world_shake(
            &frame.secondary.actors,
            world_key,
            frame.screen_feedback.camera_displacement,
        )
    } else {
        Vec2::ZERO
    };
    let transform = enemy_world_feedback_transform(
        camera_center,
        camera.scale,
        viewport,
        local.wizard.position,
        magnitude,
    );
    let camera = Camera {
        origin: Vec2::new(
            -(transform.position.x + displacement.x) / transform.scale,
            -(transform.position.y + displacement.y) / transform.scale,
        ),
        scale: transform.scale,
    };
    let cover = arena_displacement_cover(displacement, viewport, frame.settings.complex_lighting);
    WorldShake { camera, cover }
}

/// The scenery objects an earthquake is wobbling, with each wobble's phase.
fn tree_wobbles(facts: &FrameFacts<'_>) -> BTreeMap<String, f64> {
    let FrameFacts { frame, world_key, .. } = *facts;
    let world = frame.world;
    let mut tree_wobbles = BTreeMap::new();
    for actor in frame.secondary.actors.iter() {
        if actor.kind != SecondaryActorKind::EarthquakeSceneryWobble || actor.world_key != world_key
        {
            continue;
        }
        if let Some(object) =
            actor.target_id.and_then(|index| world.loaded.scene.objects.get(index))
        {
            tree_wobbles.insert(object.eid.clone(), actor.phase);
        }
    }
    tree_wobbles
}

impl WorldPasses<'_> {
    /// Before the world rows: compact ground masks and apertures, ground glyphs, background
    /// and pre-world effects, enemy underlays and rain splashes.
    fn draw_pre_world(&self, renderer: &mut Renderer, wizard: &WizardView) {
        let WorldPasses {
            frame,
            camera,
            spells,
            compact_masks,
            weather,
            enemies,
            secondary,
            ground_light_players,
            player_ground_plans,
            ..
        } = *self;
        let world = frame.world;
        compact_masks.draw_ground(renderer, camera, spells);
        for (slot, player) in ground_light_players.iter().enumerate() {
            if world.loaded.scene.environment_mode != 0 {
                draw_environment_aperture(
                    renderer,
                    camera,
                    spells,
                    player.position,
                    frame.application_time_ms,
                    slot,
                );
            }
            compact_masks.draw_player(renderer, camera, player.id);
        }
        for &plan in player_ground_plans {
            wizard.draw_ground(renderer, camera, plan);
        }
        enemies.draw_background(renderer, camera);
        secondary.draw_background(renderer, camera);
        spells.draw_queue(renderer, camera, SpellDrawQueue::PreWorld);
        secondary.draw_pre_world(renderer, camera);
        enemies.draw_pre_world(renderer, camera);
        enemies.draw_underlays(renderer, camera);
        weather.draw_splashes(renderer, camera, spells);
    }

    /// After the world rows: enemy post-world effects, the post-world spell queue, mod
    /// consumable effects, then late enemy effects.
    fn draw_post_world(&self, renderer: &mut Renderer, mod_effects: &mut ModConsumableEffectsView) {
        let WorldPasses { frame, camera, spells, lights, enemies, .. } = *self;
        enemies.draw_post_world(renderer, camera);
        spells.draw_queue(renderer, camera, SpellDrawQueue::PostWorld);
        mod_effects.update(
            frame.mod_effects,
            frame.presentation_tick,
            &|id| {
                frame
                    .players
                    .iter()
                    .find(|player| player.id == id)
                    .map(|player| player.wizard.position)
            },
            &|position| boneyard_light_tint(lights.scalar_at(position)),
        );
        mod_effects.draw(renderer, camera, spells);
        enemies.draw_late_world(renderer, camera);
    }

    /// The black strips over the edges an arena shake exposes.
    fn draw_displacement_cover(&self, renderer: &mut Renderer) {
        if let Some(cover) = self.cover {
            for rectangle in cover.rectangles {
                renderer.draw_solid(
                    [
                        (cover.position.x + rectangle.x) as f32,
                        (cover.position.y + rectangle.y) as f32,
                        rectangle.width as f32,
                        rectangle.height as f32,
                    ],
                    [0.0, 0.0, 0.0, 1.0],
                );
            }
        }
    }

    /// One actor painter at its row, tinted by the light at its region point.
    fn draw_actor(&self, renderer: &mut Renderer, wizard: &mut WizardView, index: usize) {
        let WorldPasses {
            frame,
            camera,
            spells,
            enemies,
            secondary,
            solomon,
            loot,
            player_effects,
            painters,
            tint_at,
            ..
        } = *self;
        let (_, painter, point) = &painters[index];
        let tint = point.map_or(0xffffff, tint_at);
        let rgb = [
            ((tint >> 16) & 255) as f32 / 255.0,
            ((tint >> 8) & 255) as f32 / 255.0,
            (tint & 255) as f32 / 255.0,
        ];
        match painter {
            BattlePainter::Player(index) => self.draw_player(renderer, wizard, *index),
            BattlePainter::Primary(index) => spells.draw(renderer, camera, *index, rgb),
            BattlePainter::Secondary(index) => secondary.draw(renderer, camera, *index, rgb),
            BattlePainter::Enemy(index) => enemies.draw(renderer, camera, *index, rgb),
            BattlePainter::Solomon(kind) => solomon.draw(renderer, camera, *kind),
            BattlePainter::Loot(index) => loot.draw(renderer, camera, secondary, *index, rgb),
            BattlePainter::PlayerEffect(index) => {
                player_effects.draw(renderer, camera, *index, spells, tint)
            }
            BattlePainter::Staff(index) => {
                let solomon_sim::spells::model::PrimarySpellTransient::Staff(staff) = frame
                    .spells
                    .transients
                    .get(*index)
                    .expect("staff painter refers to a live effect")
                else {
                    unreachable!("prepared staff painter")
                };
                secondary.draw_staff_transient(renderer, camera, staff, tint);
            }
        }
    }

    /// A participant's wizard, or its death body, then the player effects that follow it.
    fn draw_player(&self, renderer: &mut Renderer, wizard: &mut WizardView, index: usize) {
        let WorldPasses {
            frame,
            camera,
            spells,
            player_effects,
            equipment,
            player_followers,
            tint_at,
            ..
        } = *self;
        let world = frame.world;
        let participant = &frame.players[index];
        let player = participant.player;
        wizard.set_equipment(&equipment[index]);
        wizard.set_material_tint(player_material_tint(
            tint_at(participant.wizard.position),
            frame.secondary.players.get(participant.id),
            &player.combat,
        ));
        let damage_x4_ticks_remaining = frame
            .clocks
            .and_then(|clocks| clocks.player_damage_x4.get(participant.id))
            .copied()
            .unwrap_or(player.progression.damage_x4_ticks_remaining as f64);
        wizard.set_player_presentation(WizardPlayerPresentation {
            player,
            secondary: frame.secondary.players.get(participant.id),
            stoneskin_warp: frame.secondary.stoneskin_warp.as_deref(),
            enhanced_effects: frame.enhanced_effects,
            webbed: world.enemies.webbed_players.get(participant.id).copied(),
            overlay_effect_phase: participant.overlay_effect_phase,
            staff_action_pose: frame.spells.transients.iter().find_map(|effect| match effect {
                solomon_sim::spells::model::PrimarySpellTransient::Staff(
                    solomon_sim::spells::staff::PlayerStaffTransient::Action(action),
                ) if action.common().owner_id == participant.id => {
                    Some(solomon_sim::spells::staff::player_staff_action_pose(action))
                }
                _ => None,
            }),
            complex_lighting: frame.settings.complex_lighting,
            tick: frame.presentation_tick,
            damage_x4_ticks_remaining,
        });
        if !player_effects.draw_death_body(renderer, camera, participant.id) {
            wizard.draw_body(renderer, camera, participant.wizard, frame.presentation_tick, spells);
        }
        for (index, _) in
            player_followers.iter().filter(|(_, effect)| effect.owner_id == participant.id)
        {
            player_effects.draw(renderer, camera, *index, spells, 0xffffff);
        }
    }
}
