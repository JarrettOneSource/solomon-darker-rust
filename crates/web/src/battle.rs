//! Boneyard presentation over accepted session state. Views retain GPU resources and
//! paint-time effects; the session remains the sole owner of actors, clocks and registrations.

use std::collections::BTreeMap;

use solomon_sim::boneyard::BoneyardWorld;
use solomon_sim::boneyard::arena::ArenaPhase;
use solomon_sim::boneyard::lighting::{
    NativeBoneyardLightIndex, NativeBoneyardLightManagerView, NativePlayerLightOwner,
    native_boneyard_light_tint, native_lantern_light_source, native_player_light_source,
};
use solomon_sim::boneyard::presentation::{
    native_arena_displacement_cover, native_enemy_world_feedback_transform,
};
use solomon_sim::boneyard::shadows::{NativeShadowCaster, native_shadow_records};
use solomon_sim::boneyard::solomon_presentation::{
    SolomonPainterKind, SolomonSetPieceLighting, native_solomon_set_piece_lighting,
};
use solomon_sim::boneyard::world_light::{
    BoneyardEntityLightPresentation, BoneyardLightEnvironment, WorldLightOwner, WorldLightPlayer,
    collect_boneyard_entity_lights, native_region_point_gain, ordered_boneyard_light_sources,
};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::crow_blindness::native_crow_blindness_frame;
use solomon_sim::enemies::EnemyToken;
use solomon_sim::enemies::model::{EnemyActor, EnemyEvent};
use solomon_sim::enemies::puppet_hits::EnemyPuppetHitKind;
use solomon_sim::enemies::underlay::native_enemy_directional_shadow_family;
use solomon_sim::hub::painter::PainterLayer;
use solomon_sim::modding::inventory::{GameSimulationModEffect, ModConsumableCatalogEntry};
use solomon_sim::painter::PainterRegistration;
use solomon_sim::painter::overlays::PainterOwnerOverlay;
use solomon_sim::player::PlayerState;
use solomon_sim::player_presentation::NativeLevelUpPresentationFrame;
use solomon_sim::presentation::{native_player_material_tint, native_puppet_hit_tint};
use solomon_sim::screen_feedback::{
    native_secondary_world_shake, present_native_secondary_screen_overlay,
};
use solomon_sim::secondary::presentation::{
    NativeSecondaryAtlas, NativeSecondaryBlend, NativeSecondarySpriteDraw,
};
use solomon_sim::secondary::{
    NativeSecondaryActorKind, NativeSecondarySimulationState, native_secondary_target_effect,
    native_secondary_target_material_tint,
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
use crate::player_effects::{PlayerEffectPlayer, PlayerEffectsView};
use crate::presentation::{
    EnemyPresentationMetadata, NativePresentationTimeline, PresentationClocks,
};
use crate::secondary::{SecondaryPuppetHit, SecondaryView};
use crate::solomon::{SolomonView, SolomonViewFrame};
use crate::spells::SpellView;
use crate::wizard::{WizardEquipment, WizardPlayerPresentation, WizardView};
use crate::world_screen_feedback::WorldScreenFeedbackSample;
use solomon_graphics::Renderer;

/// Scenery and its light target have the same lifetime as one loaded map.
pub struct BattleScene {
    pub scenery: BoneyardView,
    pub lights: NativeBoneyardLightIndex,
    pub weather: BoneyardWeatherView,
    pub compact_masks: CompactMaskView,
    /// BoneyardWorldRenderer's retained draw count, independent of snapshot ticks.
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
    pub scenery_timeline: Option<&'a NativePresentationTimeline>,
    pub players: &'a [BattlePlayer<'a>],
    pub spells: &'a SpellState,
    pub secondary: &'a NativeSecondarySimulationState,
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

pub struct BattlePlayer<'a> {
    pub id: &'a str,
    pub player: &'a PlayerState,
    pub wizard: &'a HubWizard,
    pub light_registration: PainterRegistration,
    pub overlay_effect_phase: f64,
    pub drive_active: bool,
    pub materializing: bool,
    pub blindness_ticks_remaining: f64,
    pub level_up: Option<&'a NativeLevelUpPresentationFrame>,
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

impl BattleViews {
    pub async fn load(renderer: &Renderer, spells: &SpellView) -> Result<Self, String> {
        let bad_guys = spells.bad_guys();
        let dead_hawg = spells.dead_hawg();
        Ok(Self {
            enemies: EnemyView::load(
                renderer,
                bad_guys.records.clone(),
                bad_guys.texture.clone(),
                dead_hawg.records.clone(),
                dead_hawg.texture.clone(),
            )
            .await?,
            secondary: SecondaryView::load(renderer, spells).await?,
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
        let actor_poses = crate::presentation::scenery_actor_poses(
            world,
            frame.players.iter().map(|player| {
                solomon_sim::boneyard::scenery::NativeTreePlayerPose {
                    id: player.id.to_owned(),
                    position: player.wizard.position,
                }
            }),
            frame.enemy_presentation,
        );
        scene.scenery.advance_to(&crate::boneyard::TreeFrameSource {
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
        let tick = world.ticks;
        scene.compact_frame_count += 1;
        let compact_frame =
            if frame.level_up_barrier_active { tick } else { scene.compact_frame_count };
        let world_key = format!("boneyard:{}", world.loaded.run_id);
        let camera_center = Vec2::new(
            camera.origin.x + viewport.x / (2.0 * camera.scale),
            camera.origin.y + viewport.y / (2.0 * camera.scale),
        );
        let point_gain = |position| {
            native_region_point_gain(
                position,
                camera_center,
                viewport.x / camera.scale,
                local.player.combat.life_state != PlayerLifeState::Alive,
            )
        };
        spells.prepare(
            frame.spells,
            &world_key,
            compact_frame,
            &point_gain,
            &world.enemies.puppet_hits,
            frame.clocks,
            frame.enhanced_effects,
            Some(solomon_sim::spells::weld::NativeFlameLashBounds {
                x: camera.origin.x,
                y: camera.origin.y,
                width: viewport.x / camera.scale,
                height: viewport.y / camera.scale,
            }),
        );
        self.secondary.prepare(
            renderer,
            &frame.secondary.actors,
            &world_key,
            Some(compact_frame as f64),
            frame.clocks.map(|clocks| &clocks.secondary),
            &point_gain,
            frame.enhanced_effects,
        )?;

        let light_players: Vec<_> = frame
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
            .collect();
        let (mut providers, mut misc) = collect_boneyard_entity_lights(
            &world.loaded.run_id,
            &light_players,
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
                point_gain_at: &point_gain,
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
            if let Some(mut source) = native_player_light_source(
                NativePlayerLightOwner {
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
                        solomon_sim::player_presentation::native_level_up_player_light_radius(
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
                sources: vec![native_lantern_light_source(
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
            NativeBoneyardLightManagerView {
                camera_position: camera_center,
                zoom: camera.scale,
                viewport,
            },
            quality,
        );
        let light_at = |position| {
            if frame.settings.complex_lighting { scene.lights.scalar_at(position) } else { 1.0 }
        };
        scene.weather.prepare(
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
            &light_at,
        );
        let ground_light_players: Vec<_> = light_players
            .iter()
            .map(|player| CompactMaskPlayer { id: player.id, position: player.position })
            .collect();
        let mut ground_players: Vec<_> =
            frame.players.iter().filter(|player| !player.materializing).collect();
        ground_players.sort_by_key(|player| player.light_registration.registration_ordinal);
        let player_ground_plans: Vec<_> = ground_players
            .iter()
            .map(|participant| {
                solomon_sim::boneyard::ground_glyph::native_player_ground_glyph(
                    solomon_sim::boneyard::ground_glyph::NativeGroundPlayer {
                        position: participant.wizard.position,
                        heading_degrees: participant.wizard.heading_degrees,
                        scale: 1.0,
                        life_state: participant.player.combat.life_state,
                        death_tick: participant.player.combat.death_tick,
                        corpse_consumed: participant.player.combat.corpse_consumed,
                    },
                    scene.compact_masks.special_surface_at(
                        participant.wizard.position,
                        &world.enemies.spider_remains,
                        spells,
                    ),
                )
            })
            .collect();
        // Source compact admission uses the full scene until sealing completes;
        // Tutorial camera cleanup and the rain viewport are separate bounds.
        let compact_bounds = world
            .arena_transition
            .as_ref()
            .filter(|transition| transition.phase == ArenaPhase::Sealed)
            .map_or(world.loaded.scene.bounds, |transition| transition.combat_bounds);
        scene.compact_masks.prepare(
            renderer,
            spells,
            &ground_light_players,
            &world.enemies.spider_remains,
            compact_bounds,
            compact_frame as f64,
        )?;
        let tint_at = |position| native_boneyard_light_tint(light_at(position));
        let effect_players: Vec<_> = frame
            .players
            .iter()
            .map(|participant| PlayerEffectPlayer {
                id: participant.id,
                player: participant.player,
                wizard: participant.wizard,
                material_tint: native_player_material_tint(
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
        let actor_tint = |actor: &EnemyActor| {
            native_secondary_target_material_tint(
                tint_at(actor.position),
                native_secondary_target_effect(
                    &frame.secondary.target_effects,
                    &world_key,
                    actor.id,
                ),
            )
        };
        let actor_shadows = |actor: &EnemyActor| {
            let enabled = native_enemy_directional_shadow_family(actor.config.enemy_token)
                && (frame.settings.complex_shadows
                    || actor.config.enemy_token == EnemyToken::Heartmonger);
            EnemyViewShadow {
                records: if enabled {
                    native_shadow_records(
                        &NativeShadowCaster {
                            id: format!("enemy:{}", actor.id),
                            position: actor.position,
                            outline: Vec::new(),
                            program: None,
                        },
                        &scene.lights,
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
                primary_tint: solomon_sim::profile::primary::native_primary_spell_tint(
                    player.player.book.primary_skill_id,
                    player.player.book.weld_build_id,
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
                view_height: viewport.y,
                players: &players,
                light_at: &light_at,
                point_gain: &point_gain,
                actor_shadows: &actor_shadows,
                actor_tint: &actor_tint,
            },
        );
        let mut puppet_hits = BTreeMap::new();
        for actor in &frame.secondary.actors {
            if actor.world_key != world_key || actor.kind != NativeSecondaryActorKind::Leviathan {
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
        self.loot.prepare(
            &world.loot,
            frame.presentation_tick,
            &world.enemies.puppet_hits,
            frame.clocks.map(|clocks| &clocks.puppet_hits),
            frame.settings.complex_lighting,
            frame.mod_catalog,
            mod_assets,
        )?;
        if let Some(dig) = &world.loaded.scene.solomon_dig {
            let lighting = if frame.settings.complex_lighting {
                native_solomon_set_piece_lighting(
                    world.encounter.as_ref().map_or(dig.position, |encounter| encounter.position),
                    world.lantern_position.unwrap_or(dig.lantern_position),
                    &tint_at,
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
        // These roots follow the assigned player depth without entering the native
        // painter queue. They precede primary overlays promoted to owner depth + 0.5.
        let mut player_followers: Vec<_> =
            self.player_effects.painters().filter(|(_, painter)| painter.layer.is_none()).collect();
        player_followers.sort_by(|(_, left), (_, right)| {
            left.owner_depth_offset.total_cmp(&right.owner_depth_offset)
        });
        let layers: Vec<_> = painters.iter().map(|(layer, _, _)| *layer).collect();
        // Logical light admission uses the unshaken camera. Only the final world
        // transform changes, with the local wizard's screen position as its anchor.
        let magnitude = if frame.settings.zoom_effects {
            world.enemy_world_feedback.magnitude.max(frame.screen_feedback.camera_magnitude)
        } else {
            0.0
        };
        let displacement = if frame.settings.zoom_effects {
            native_secondary_world_shake(
                &frame.secondary.actors,
                &world_key,
                frame.screen_feedback.camera_displacement,
            )
        } else {
            Vec2::ZERO
        };
        let transform = native_enemy_world_feedback_transform(
            camera_center,
            camera.scale,
            viewport,
            local.wizard.position,
            magnitude,
        );
        let camera = &Camera {
            origin: Vec2::new(
                -(transform.position.x + displacement.x) / transform.scale,
                -(transform.position.y + displacement.y) / transform.scale,
            ),
            scale: transform.scale,
        };
        let cover = native_arena_displacement_cover(
            displacement,
            viewport,
            frame.settings.complex_lighting,
        );
        let mut tree_wobbles = BTreeMap::new();
        for actor in &frame.secondary.actors {
            if actor.kind != NativeSecondaryActorKind::EarthquakeSceneryWobble
                || actor.world_key != world_key
            {
                continue;
            }
            if let Some(object) =
                actor.target_id.and_then(|index| world.loaded.scene.objects.get(index))
            {
                tree_wobbles.insert(object.eid.clone(), actor.phase);
            }
        }
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
                BoneyardPass::PreWorld => {
                    scene.compact_masks.draw_ground(renderer, camera, spells);
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
                        scene.compact_masks.draw_player(renderer, camera, player.id);
                    }
                    for &plan in &player_ground_plans {
                        wizard.draw_ground(renderer, camera, plan);
                    }
                    self.enemies.draw_background(renderer, camera);
                    self.secondary.draw_background(renderer, camera);
                    spells.draw_queue(renderer, camera, SpellDrawQueue::PreWorld);
                    self.secondary.draw_pre_world(renderer, camera);
                    self.enemies.draw_pre_world(renderer, camera);
                    self.enemies.draw_underlays(renderer, camera);
                    scene.weather.draw_splashes(renderer, camera, spells);
                }
                BoneyardPass::PostWorld => {
                    self.enemies.draw_post_world(renderer, camera);
                    spells.draw_queue(renderer, camera, SpellDrawQueue::PostWorld);
                    self.mod_effects.update(
                        frame.mod_effects,
                        frame.presentation_tick,
                        &|id| frame.players.iter().find(|player| player.id == id).map(|player| player.wizard.position),
                        &|position| native_boneyard_light_tint(scene.lights.scalar_at(position)),
                    );
                    self.mod_effects.draw(renderer, camera, spells);
                    self.enemies.draw_late_world(renderer, camera);
                }
                BoneyardPass::LightComposite => {
                    if let Some(cover) = cover {
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
                BoneyardPass::Actor(index) => {
                    let (_, painter, point) = &painters[index];
                    let tint = point.map_or(0xffffff, tint_at);
                    let rgb = [
                        ((tint >> 16) & 255) as f32 / 255.0,
                        ((tint >> 8) & 255) as f32 / 255.0,
                        (tint & 255) as f32 / 255.0,
                    ];
                    match painter {
                        BattlePainter::Player(index) => {
                            let participant = &frame.players[*index];
                            let player = participant.player;
                            wizard.set_equipment(&equipment[*index]);
                            wizard.set_material_tint(native_player_material_tint(
                                tint_at(participant.wizard.position),
                                frame.secondary.players.get(participant.id),
                                &player.combat,
                            ));
                            let damage_x4_ticks_remaining = frame.clocks
                                .and_then(|clocks| clocks.player_damage_x4.get(participant.id))
                                .copied()
                                .unwrap_or(player.progression.damage_x4_ticks_remaining as f64);
                            wizard.set_player_presentation(WizardPlayerPresentation {
                                player,
                                secondary: frame.secondary.players.get(participant.id),
                                webbed: world.enemies.webbed_players.get(participant.id).copied(),
                                overlay_effect_phase: participant.overlay_effect_phase,
                                staff_action_pose: frame.spells.transients.iter().find_map(
                                    |effect| match effect {
                                        solomon_sim::spells::model::PrimarySpellTransient::Staff(
                                            solomon_sim::spells::staff::NativePlayerStaffTransient::Action(action),
                                        ) if action.common().owner_id == participant.id => {
                                            Some(solomon_sim::spells::staff::native_player_staff_action_pose(action))
                                        }
                                        _ => None,
                                    },
                                ),
                                complex_lighting: frame.settings.complex_lighting,
                                tick: frame.presentation_tick,
                                damage_x4_ticks_remaining,
                            });
                            if !self.player_effects.draw_death_body(
                                renderer,
                                camera,
                                participant.id,
                            ) {
                                wizard.draw_body(
                                    renderer,
                                    camera,
                                    participant.wizard,
                                    frame.presentation_tick,
                                    spells,
                                );
                            }
                            for (index, _) in player_followers
                                .iter()
                                .filter(|(_, effect)| effect.owner_id == participant.id)
                            {
                                self.player_effects
                                    .draw(renderer, camera, *index, spells, 0xffffff);
                            }
                        }
                        BattlePainter::Primary(index) => spells.draw(renderer, camera, *index, rgb),
                        BattlePainter::Secondary(index) => {
                            self.secondary.draw(renderer, camera, *index, rgb)
                        }
                        BattlePainter::Enemy(index) => {
                            self.enemies.draw(renderer, camera, *index, rgb)
                        }
                        BattlePainter::Solomon(kind) => self.solomon.draw(renderer, camera, *kind),
                        BattlePainter::Loot(index) => {
                            self.loot.draw(renderer, camera, &self.secondary, *index, rgb)
                        }
                        BattlePainter::PlayerEffect(index) => {
                            self.player_effects.draw(renderer, camera, *index, spells, tint)
                        }
                        BattlePainter::Staff(index) => {
                            let solomon_sim::spells::model::PrimarySpellTransient::Staff(staff) =
                                &frame.spells.transients[*index]
                            else {
                                unreachable!("prepared staff painter")
                            };
                            self.secondary.draw_staff_transient(renderer, camera, staff, tint);
                        }
                    }
                }
            },
        );
        scene.weather.draw_streaks(renderer, camera);
        draw_world_overlays(renderer, camera);
        if let Some(overlay) = present_native_secondary_screen_overlay(
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
        if let Some(blindness) = native_crow_blindness_frame(
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
                &NativeSecondarySpriteDraw {
                    alpha: blindness.stamp_alpha,
                    atlas: NativeSecondaryAtlas::DeadHawg,
                    blend: NativeSecondaryBlend::Normal,
                    color_mode: None,
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
        Ok(())
    }
}
