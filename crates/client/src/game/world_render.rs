//! Each frame's world drawing: the Boneyard, the Hub courtyard and private
//! rooms with their wizards, actors and overlays, and the inventory's wizard preview.

use crate::battle::{BattleFrame, BattlePlayer};
use crate::courtyard::{ActorPainters, Camera, CourtyardPass, CourtyardScene};
use crate::hud::HudMode;
use crate::menu::MenuUi;
use crate::npc::NpcView;
use crate::rooms::{PrivateRoomFrame, PrivateRoomScenes};
use crate::spells::SpellView;
use crate::teacher::{TeacherPass, TeacherView};
use crate::wizard::{WizardPlayerPresentation, WizardView};
use crate::world_overlays::{WorldOverlayFrame, WorldOverlayPlayer};
use crate::world_screen_feedback::{ScreenFeedbackScene, WorldScreenFeedbackFrame};
use solomon_graphics::Renderer;
use solomon_sim::Vec2;
use solomon_sim::hub::depth::HubDepth;
use solomon_sim::hub::interaction::{HubInteractionAvailability, HubInteractionId};
use solomon_sim::hub::painter::{PainterLayer, courtyard_player_sort_bias};
use solomon_sim::hub::teacher::hub_teacher_draw_plan;
use solomon_sim::painter::overlays::PainterOwnerOverlay;
use solomon_sim::session::GameSession;
use solomon_sim::settings::GameSettings;
use solomon_sim::spells::presentation::SpellDrawQueue;

use super::{Game, NativeGame, spectator};

impl Game {
    /// Local simulation delivers every tick here, including ticks between draws.
    /// Tree scans must observe those poses rather than treating them as missing history.
    pub(super) fn advance_local_scenery(&mut self) {
        if self.session.boneyard.is_none() || self.battle.is_none() {
            return;
        }
        let (camera, layout) = self.scene_view();
        let focus = self.boneyard_camera_focus();
        let world = self.session.boneyard.as_ref().unwrap();
        let poses = crate::presentation::scenery_actor_poses(
            world,
            self.session.participant_order.iter().map(|id| {
                solomon_sim::boneyard::scenery::TreePlayerPose {
                    id: id.clone(),
                    position: self.session.participants[id].wizard.position,
                }
            }),
            None,
        );
        self.battle.as_mut().unwrap().scene.scenery.advance_to(&crate::boneyard::TreeFrameSource {
            run_id: &world.loaded.run_id,
            local_player_id: &self.local_player_id,
            focus_player_id: focus.player_id.as_deref(),
            display_tick: self.session.ticks as f64,
            actor_poses: &poses,
            camera_bounds: solomon_sim::boneyard::BoneyardBounds::new(
                camera.origin.x,
                camera.origin.y,
                layout.logical_size.x / camera.scale,
                layout.logical_size.y / camera.scale,
            ),
            camera_limits: world.camera_bounds(),
            viewport: layout.logical_size,
            zoom: camera.scale,
            timeline: None,
        });
    }

    pub(super) fn render_world(&mut self) -> Result<(), String> {
        self.hub_guidance.clear();
        self.social.set_guidance_bounds(&[]);
        #[cfg(target_arch = "wasm32")]
        self.hub_guidance.apply_diagnostics(self.surface.canvas(), 1.0)?;
        let displayed = DisplayedSession::of(self.native.as_ref(), &self.session);
        let DisplayedSession { display, session, presentation_tick, clocks } = displayed;
        self.hud.set_mode(if session.boneyard.is_some() { HudMode::Run } else { HudMode::Hub })?;
        if !self.has_game || self.menu.covers_world() {
            self.clear_world_frame();
            return Ok(());
        }
        let player = &session.participants[&self.local_player_id].player;
        prepare_local_wizard(
            &mut self.wizard_view,
            &mut self.mod_assets,
            &displayed,
            player,
            &self.local_player_id,
            &self.menu,
        )?;
        let (camera, layout) = self.scene_view();
        let spectator_status =
            spectator::status(session, &self.local_player_id, &self.spectator_camera);
        self.spectator_ui.update(spectator_status.as_ref(), layout)?;
        let viewport = layout.logical_size;
        let (width, height) = self.fit_canvas();
        self.renderer.set_world_pixel_centers(true);
        self.renderer.begin_frame(width, height, [viewport.x as f32, viewport.y as f32]);
        if self.panels.covers_world() {
            return self.draw_inventory_preview(width, height);
        }
        let camera_center = Vec2::new(
            camera.origin.x + viewport.x / (2.0 * camera.scale),
            camera.origin.y + viewport.y / (2.0 * camera.scale),
        );
        let screen_feedback = self.world_screen_feedback.update(WorldScreenFeedbackFrame {
            scene: if session.boneyard.is_some() {
                ScreenFeedbackScene::Boneyard
            } else {
                ScreenFeedbackScene::Hub
            },
            world_key: &session
                .world_key(&self.local_player_id)
                .ok_or("The display has no local world")?,
            tick: presentation_tick,
            stopped_world_tick: solomon_sim::game_run::game_run_world_tick(
                presentation_tick,
                &session.run,
            ),
            flame_epoch: display.and_then(|frame| frame.flame_epoch),
            flame_camera_displacements: display
                .map_or(&[], |frame| frame.flame_camera_displacements.as_slice()),
            screen_flashes: &session.screen_flashes,
            spells: &session.spells,
            secondary: &session.secondary,
            clocks,
            context: solomon_sim::screen_feedback::SecondaryScreenFeedbackContext {
                camera_center,
                visible_world_width: viewport.x / camera.scale,
                local_player_alternate: player.combat.life_state
                    != solomon_sim::combat::PlayerLifeState::Alive,
            },
        });
        let view = WorldCamera { camera, viewport, camera_center, screen_feedback };
        let mut overlay_diagnostics = Default::default();
        if session.boneyard.is_some() {
            self.draw_boneyard_world(view, &mut overlay_diagnostics)?;
        } else {
            self.draw_hub_world(view, &mut overlay_diagnostics)?;
        }
        #[cfg(target_arch = "wasm32")]
        overlay_diagnostics.apply(self.surface.canvas())?;
        self.world_overlay_diagnostics = overlay_diagnostics;
        // Drawing the world needed `self` mutably, so borrow the displayed session again;
        // the world draws leave it unchanged.
        let session = DisplayedSession::of(self.native.as_ref(), &self.session).session;
        if let Some(transition) = session.participants[&self.local_player_id].wizard.transition {
            self.renderer.draw_solid(
                [0.0, 0.0, viewport.x as f32, viewport.y as f32],
                [0.0, 0.0, 0.0, transition.alpha as f32],
            );
        }
        self.renderer.end_frame();
        self.record_mod_scene_projection()?;
        self.hud.update(
            &self.hud_state,
            self.surface.logical_width(),
            self.surface.logical_height(),
        )?;
        Ok(())
    }

    /// Without a game, or under a menu that covers the world, the world frame is empty.
    fn clear_world_frame(&mut self) {
        self.spectator_ui.hide();
        self.mod_overlays.reset();
        self.mod_projection = None;
        let (width, height) = self.fit_canvas();
        self.renderer.set_world_pixel_centers(false);
        self.renderer.begin_frame(
            width,
            height,
            [self.surface.logical_width() as f32, self.surface.logical_height() as f32],
        );
        self.renderer.end_frame();
    }

    /// A player panel covers the world: draw only the inventory seal and wizard preview.
    fn draw_inventory_preview(&mut self, width: i32, height: i32) -> Result<(), String> {
        self.renderer.set_world_pixel_centers(false);
        let client_width = self.surface.logical_width().max(1) as f32;
        let client_height = self.surface.logical_height().max(1) as f32;
        self.renderer.begin_frame(width, height, [client_width, client_height]);
        self.renderer.set_saturation(1.0);
        let ratio_x = f64::from(width) / f64::from(client_width);
        let ratio_y = f64::from(height) / f64::from(client_height);
        let clip = |bounds: solomon_sim::hud::HudRect| {
            [
                (bounds.x * ratio_x).round() as i32,
                (f64::from(height) - (bounds.y + bounds.height) * ratio_y).round() as i32,
                (bounds.width * ratio_x).round() as i32,
                (bounds.height * ratio_y).round() as i32,
            ]
        };
        if let Some(underlay) = self.panels.inventory_preview_underlay()? {
            let texture = self
                .gpu_ui
                .image_texture(underlay.texture_path)
                .ok_or("The stock Inventory seal is not ready")?;
            let rect = |r: solomon_sim::hud::HudRect| {
                [r.x as f32, r.y as f32, r.width as f32, r.height as f32]
            };
            self.renderer.draw_clipped(clip(underlay.clip), underlay.alpha as f32, |renderer| {
                renderer.draw(
                    &texture.clamped_to(rect(underlay.source)),
                    solomon_graphics::Quad {
                        source: rect(underlay.source),
                        destination: rect(underlay.destination),
                        tint: [
                            ((underlay.tint >> 16) & 255) as f32 / 255.0,
                            ((underlay.tint >> 8) & 255) as f32 / 255.0,
                            (underlay.tint & 255) as f32 / 255.0,
                            1.0,
                        ],
                        blend: solomon_graphics::BlendMode::Normal,
                    },
                );
            });
        }
        if let Some(preview) = self.panels.inventory_preview_placement() {
            self.renderer.draw_clipped(clip(preview.clip), preview.alpha as f32, |renderer| {
                self.wizard_view.draw_preview(
                    renderer,
                    &Camera { origin: Vec2::ZERO, scale: preview.scale },
                    Vec2::new(preview.center.x / preview.scale, preview.center.y / preview.scale),
                    &self.spell_view,
                );
            });
        }
        self.renderer.end_frame();
        Ok(())
    }

    /// The Boneyard, drawn by the battle views from every participant, with the world
    /// overlays above its scenery.
    fn draw_boneyard_world(
        &mut self,
        view: WorldCamera,
        overlay_diagnostics: &mut crate::world_overlays::WorldOverlayDiagnostics,
    ) -> Result<(), String> {
        let DisplayedSession { display, session, presentation_tick, clocks } =
            DisplayedSession::of(self.native.as_ref(), &self.session);
        let WorldCamera { camera, viewport, screen_feedback, .. } = view;
        let host = self.native.as_ref().and_then(NativeGame::host_snapshot);
        let overlay_players = world_overlay_players(session, host, &self.local_player_id);
        let world = session.boneyard.as_ref().expect("the displayed session is in a Boneyard");
        let camera_focus = self.boneyard_camera_focus();
        // Keep one level-up view, the one driven by this client's picker.
        self.level_up.retain(|id, _| id == &self.local_player_id);
        let local = &session.participants[&self.local_player_id];
        let level_up = self.level_up.entry(self.local_player_id.clone()).or_default().sample(
            &world.loaded.run_id,
            &local.player,
            session.level_up_barrier.as_ref().map(|barrier| barrier.barrier_id),
            self.last_time.unwrap_or(0.0),
            (local.wizard.position.y - camera.origin.y) * camera.scale,
        );
        let players = battle_players(session, host, &self.local_player_id, level_up.as_ref());
        self.battle_views.draw(
            &mut self.renderer,
            &mut self.battle.as_mut().expect("prepared active Boneyard").scene,
            &mut self.spell_view,
            &mut self.wizard_view,
            self.mod_assets.as_mut(),
            &BattleFrame {
                enhanced_effects: session.enhanced_effects,
                mod_effects: &session.mod_effects,
                mod_catalog: self
                    .hosted
                    .as_ref()
                    .and_then(|hosted| hosted.welcome.as_ref())
                    .map_or(&[], |welcome| welcome.mod_catalog.as_slice()),
                world,
                presentation_tick,
                stopped_world_tick: solomon_sim::game_run::game_run_world_tick(
                    presentation_tick,
                    &session.run,
                ),
                clocks,
                enemy_presentation: display.and_then(|frame| frame.enemies.as_ref()),
                level_up_barrier_active: session.level_up_barrier.is_some(),
                local_player_id: &self.local_player_id,
                camera_focus_player_id: camera_focus.player_id.as_deref(),
                scenery_timeline: self.native.as_ref().map(NativeGame::presentation_timeline),
                players: &players,
                spells: &session.spells,
                secondary: &session.secondary,
                camera: &camera,
                viewport,
                settings: self.menu.settings(),
                screen_feedback,
                application_time_ms: self.last_time.unwrap_or(0.0),
                device_resolution: self.surface.scale_factor(),
            },
            |renderer, camera| {
                *overlay_diagnostics = self.world_overlays.draw(
                    renderer,
                    &WorldOverlayFrame {
                        world_key: &world.loaded.run_id,
                        players: &overlay_players,
                        local_player_id: &self.local_player_id,
                        camera,
                        viewport,
                        now_ms: self.last_time.unwrap_or(0.0),
                        renderable: true,
                    },
                );
            },
        )?;
        Ok(())
    }

    /// The College: a private room or the courtyard, then the nameplates and the secondary
    /// abilities' screen flash.
    fn draw_hub_world(
        &mut self,
        view: WorldCamera,
        overlay_diagnostics: &mut crate::world_overlays::WorldOverlayDiagnostics,
    ) -> Result<(), String> {
        let DisplayedSession { session, presentation_tick, clocks, .. } =
            DisplayedSession::of(self.native.as_ref(), &self.session);
        let player = &session.participants[&self.local_player_id].player;
        let host = self.native.as_ref().and_then(NativeGame::host_snapshot);
        let overlay_players = world_overlay_players(session, host, &self.local_player_id);
        let WorldCamera { camera, viewport, camera_center, screen_feedback } = view;
        let base_camera = camera;
        let transform = solomon_sim::boneyard::presentation::enemy_world_feedback_transform(
            camera_center,
            base_camera.scale,
            viewport,
            session.participants[&self.local_player_id].wizard.position,
            if self.menu.settings().zoom_effects { screen_feedback.camera_magnitude } else { 0.0 },
        );
        let displacement = if self.menu.settings().zoom_effects {
            screen_feedback.camera_displacement
        } else {
            Vec2::ZERO
        };
        let camera = Camera {
            origin: Vec2::new(
                -(transform.position.x + displacement.x) / transform.scale,
                -(transform.position.y + displacement.y) / transform.scale,
            ),
            scale: transform.scale,
        };
        let hub_players = hub_region_players(session, &self.local_player_id);
        let mut hub_equipment = Vec::with_capacity(hub_players.len());
        for participant in &hub_players {
            hub_equipment.push(crate::wizard::WizardEquipment::prepare(
                participant.player.selection.element,
                &participant.player.economy.equipment,
                self.mod_assets.as_mut(),
            )?);
        }
        let player_painters = hub_player_painters(&hub_players);
        if session.participants[&self.local_player_id].wizard.region
            == solomon_sim::hub::regions::HubRegion::Courtyard
            && !self.menu.is_blocking()
            && !self.panels.is_open()
            && !self.social.input_active()
            && !self.skill_picker.is_open()
            && !player.economy.college_intro_pending
        {
            self.hub_guidance.prepare(
                &player.economy.npc.help_flags,
                session.ticks,
                &camera,
                viewport,
            );
        }
        let hub = HubDraw {
            session,
            local_player_id: &self.local_player_id,
            player,
            hub_players: &hub_players,
            hub_equipment: &hub_equipment,
            base_camera: &base_camera,
            camera: &camera,
            viewport,
            presentation_tick,
            clocks,
            settings: self.menu.settings(),
            teacher_view: &self.teacher_view,
            npc_view: &self.npc_view,
        };
        if session.participants[&self.local_player_id].wizard.region.is_private() {
            draw_private_room(
                &mut self.renderer,
                &self.rooms,
                &mut self.wizard_view,
                &self.spell_view,
                &hub,
                &player_painters,
            );
        } else {
            draw_courtyard(
                &mut self.renderer,
                &mut self.scene,
                &mut self.spell_view,
                &mut self.wizard_view,
                &hub,
                player_painters,
                &self.hub_guidance,
            );
        }
        self.hub_guidance.draw_screen(&mut self.renderer);
        let display_scale = f64::from(self.surface.logical_width()) / viewport.x;
        self.social.set_guidance_bounds(&self.hub_guidance.screen_bounds(display_scale));
        #[cfg(target_arch = "wasm32")]
        self.hub_guidance.apply_diagnostics(self.surface.canvas(), display_scale)?;
        let nameplate_camera = Camera {
            origin: Vec2::new(
                -transform.position.x / transform.scale,
                -transform.position.y / transform.scale,
            ),
            scale: transform.scale,
        };
        *overlay_diagnostics = self.world_overlays.draw(
            &mut self.renderer,
            &WorldOverlayFrame {
                world_key: "hub",
                players: &overlay_players,
                local_player_id: &self.local_player_id,
                camera: &nameplate_camera,
                viewport,
                now_ms: self.last_time.unwrap_or(0.0),
                renderable: !player.economy.college_intro_pending,
            },
        );
        if let Some(overlay) = solomon_sim::screen_feedback::present_secondary_screen_overlay(
            screen_feedback.overlay,
            self.menu.settings().reduced_screen_flashes,
        ) {
            self.renderer.draw_solid(
                [0.0, 0.0, viewport.x as f32, viewport.y as f32],
                [
                    ((overlay.color >> 16) & 255) as f32 / 255.0,
                    ((overlay.color >> 8) & 255) as f32 / 255.0,
                    (overlay.color & 255) as f32 / 255.0,
                    overlay.alpha as f32,
                ],
            );
        }
        Ok(())
    }
}

/// The session one world frame shows: the native host's sampled display frame when it has
/// one, otherwise this client's own session, with its presentation tick and clocks. It
/// borrows only those two `Game` fields, so the world can be drawn with the others.
#[derive(Clone, Copy)]
struct DisplayedSession<'a> {
    display: Option<&'a crate::presentation::PresentationFrame>,
    session: &'a GameSession,
    presentation_tick: f64,
    clocks: Option<&'a crate::presentation::PresentationClocks>,
}

impl<'a> DisplayedSession<'a> {
    fn of(native: Option<&'a NativeGame>, session: &'a GameSession) -> Self {
        let display = native.and_then(|native| native.display.as_ref());
        let session = display.map_or(session, |display| display.session.as_ref());
        let presentation_tick = display.map_or(session.ticks as f64, |frame| frame.tick);
        let clocks = display.map(|frame| &frame.clocks);
        Self { display, session, presentation_tick, clocks }
    }
}

/// The unshaken world camera and the screen feedback sampled before the Boneyard or the
/// College draws.
struct WorldCamera {
    camera: Camera,
    viewport: Vec2,
    camera_center: Vec2,
    screen_feedback: crate::world_screen_feedback::WorldScreenFeedbackSample,
}

/// What a College frame draws from: the accepted session and local wizard, this region's
/// participants with their equipment, the base and shaken cameras, the presentation clock and
/// settings, and the teacher and NPC views.
struct HubDraw<'a> {
    session: &'a GameSession,
    local_player_id: &'a str,
    player: &'a solomon_sim::player::PlayerState,
    hub_players: &'a [&'a solomon_sim::session::SessionParticipant],
    hub_equipment: &'a [crate::wizard::WizardEquipment],
    base_camera: &'a Camera,
    camera: &'a Camera,
    viewport: Vec2,
    presentation_tick: f64,
    clocks: Option<&'a crate::presentation::PresentationClocks>,
    settings: &'a GameSettings,
    teacher_view: &'a TeacherView,
    npc_view: &'a NpcView,
}

/// The courtyard's actor queue after the participants: the teacher's passes, the NPCs and the
/// primary spells, each range found by its first index.
struct CourtyardActors {
    teacher: solomon_sim::hub::teacher::HubTeacherDrawPlan,
    teacher_start: usize,
    teacher_painters: Vec<(PainterLayer, TeacherPass)>,
    npc_start: usize,
    npc_painters: Vec<(PainterLayer, crate::npc::HubNpcActor)>,
    spell_start: usize,
    spell_painters: Vec<(usize, PainterLayer)>,
    marker_availability: HubInteractionAvailability,
}

/// Prepare the shared wizard view for the local wizard: equipment, material tint and
/// presentation.
fn prepare_local_wizard(
    wizard_view: &mut WizardView,
    mod_assets: &mut Option<crate::mod_presentation_assets::ModPresentationAssets>,
    displayed: &DisplayedSession<'_>,
    player: &solomon_sim::player::PlayerState,
    local_player_id: &str,
    menu: &MenuUi,
) -> Result<(), String> {
    let DisplayedSession { session, presentation_tick, clocks, .. } = *displayed;
    let equipment = crate::wizard::WizardEquipment::prepare(
        player.selection.element,
        &player.economy.equipment,
        mod_assets.as_mut(),
    )?;
    wizard_view.set_equipment(&equipment);
    wizard_view.set_material_tint(solomon_sim::presentation::player_material_tint(
        0xffffff,
        session.secondary.players.get(local_player_id),
        &player.combat,
    ));
    wizard_view.set_player_presentation(WizardPlayerPresentation {
        player,
        secondary: session.secondary.players.get(local_player_id),
        stoneskin_warp: session.secondary.stoneskin_warp.as_deref(),
        enhanced_effects: session.enhanced_effects,
        webbed: session
            .boneyard
            .as_ref()
            .and_then(|world| world.enemies.webbed_players.get(local_player_id).copied()),
        overlay_effect_phase: session.participants[local_player_id].lighting.overlay_effect_phase,
        // Active Boneyard actions are sampled per participant by BattleViews.
        staff_action_pose: None,
        complex_lighting: menu.settings().complex_lighting,
        tick: presentation_tick,
        damage_x4_ticks_remaining: clocks
            .and_then(|clocks| clocks.player_damage_x4.get(local_player_id))
            .copied()
            .unwrap_or(player.progression.damage_x4_ticks_remaining as f64),
    });
    Ok(())
}

/// The participants the world overlays label: everyone in a Boneyard, or everyone in the
/// local wizard's College region.
fn world_overlay_players<'a>(
    session: &'a GameSession,
    host: Option<&'a solomon_protocol::HostSnapshot>,
    local_player_id: &str,
) -> Vec<WorldOverlayPlayer<'a>> {
    let local_region = session.participants[local_player_id].wizard.region;
    session
        .participant_order
        .iter()
        .filter_map(|id| {
            let participant = &session.participants[id];
            if session.boneyard.is_none() && participant.wizard.region != local_region {
                return None;
            }
            Some(WorldOverlayPlayer {
                id,
                name: &participant.player.selection.name,
                element: participant.player.selection.element,
                position: participant.wizard.position,
                current_health: participant.player.combat.current_health,
                maximum_health: participant.player.combat.maximum_health,
                activity: if session.boneyard.is_some() {
                    None
                } else {
                    host.and_then(|host| host.hub_activities.get(id)).copied()
                },
            })
        })
        .collect()
}

/// Each participant's borrowed state for the battle views, in participant order.
fn battle_players<'a>(
    session: &'a GameSession,
    host: Option<&solomon_protocol::HostSnapshot>,
    local_player_id: &str,
    level_up: Option<&'a solomon_sim::player_presentation::LevelUpPresentationFrame>,
) -> Vec<BattlePlayer<'a>> {
    session
        .participant_order
        .iter()
        .map(|id| {
            let participant = &session.participants[id];
            let lighting = &participant.lighting;
            BattlePlayer {
                id,
                player: &participant.player,
                wizard: &participant.wizard,
                light_registration: lighting.light_registration,
                // Both the action and the weapon phase drive the overlay effect.
                overlay_effect_phase: participant
                    .wizard
                    .primary_cast
                    .weapon_pulse
                    .max(lighting.overlay_effect_phase),
                drive_active: participant.wizard.primary_cast.owns_facing()
                    || matches!(
                        participant.player.combat.life_state,
                        solomon_sim::combat::PlayerLifeState::Dying
                            | solomon_sim::combat::PlayerLifeState::Spectating
                    ),
                materializing: host.is_some_and(|host| host.materializing_player_ids.contains(id)),
                blindness_ticks_remaining: lighting.blindness_ticks_remaining,
                level_up: if id == local_player_id { level_up } else { None },
            }
        })
        .collect()
}

/// The participants in the local wizard's College region, in participant order.
fn hub_region_players<'a>(
    session: &'a GameSession,
    local_player_id: &str,
) -> Vec<&'a solomon_sim::session::SessionParticipant> {
    session
        .participant_order
        .iter()
        .map(|id| &session.participants[id])
        .filter(|participant| {
            participant.wizard.region == session.participants[local_player_id].wizard.region
        })
        .collect()
}

/// Each College participant's painter layer at its feet, with the courtyard's sort bias
/// outside the private rooms.
fn hub_player_painters(
    hub_players: &[&solomon_sim::session::SessionParticipant],
) -> Vec<PainterLayer> {
    hub_players
        .iter()
        .map(|participant| {
            participant.lighting.light_registration.layer(
                participant.wizard.position.y,
                if participant.wizard.region.is_private() {
                    0.0
                } else {
                    courtyard_player_sort_bias(
                        participant.wizard.position,
                        participant.wizard.heading_index as i64,
                    )
                },
            )
        })
        .collect()
}

/// A private room around the local wizard, with each participant in it at its painter row.
fn draw_private_room(
    renderer: &mut Renderer,
    rooms: &PrivateRoomScenes,
    wizard_view: &mut WizardView,
    spell_view: &SpellView,
    hub: &HubDraw<'_>,
    player_painters: &[PainterLayer],
) {
    let HubDraw {
        session,
        local_player_id,
        hub_players,
        hub_equipment,
        camera,
        presentation_tick,
        clocks,
        settings,
        ..
    } = *hub;
    rooms.draw(
        renderer,
        camera,
        &PrivateRoomFrame {
            local_wizard: &session.participants[local_player_id].wizard,
            player_painters,
            tick: session.ticks,
            dowser_frame: session.dowser_clock.frame(),
        },
        |renderer, index| {
            let participant = hub_players[index];
            prepare_hub_wizard(
                wizard_view,
                participant,
                &hub_equipment[index],
                session.secondary.players.get(&participant.id),
                session.secondary.stoneskin_warp.as_deref(),
                session.enhanced_effects,
                settings.complex_lighting,
                presentation_tick,
                clocks,
            );
            wizard_view.draw(renderer, camera, &participant.wizard, presentation_tick, spell_view);
        },
    );
}

/// The courtyard: the teacher, NPCs and primary spells join the participants in one painter
/// queue, drawn between the teacher's and the Useful Thyngs tent's depth bands.
fn draw_courtyard(
    renderer: &mut Renderer,
    scene: &mut CourtyardScene,
    spell_view: &mut SpellView,
    wizard_view: &mut WizardView,
    hub: &HubDraw<'_>,
    player_painters: Vec<PainterLayer>,
    guidance: &crate::hub_guidance::HubGuidanceView,
) {
    let HubDraw {
        session,
        local_player_id,
        player,
        hub_players,
        base_camera,
        camera,
        viewport,
        clocks,
        teacher_view,
        npc_view,
        ..
    } = *hub;
    let teacher = hub_teacher_draw_plan(
        session.ambient.teacher_tick,
        0,
        session.ambient.teacher_world_release,
    );
    let teacher_painters = TeacherView::painters(&teacher);
    let npc_painters = npc_view.painters(&session.npcs);
    spell_view.prepare(
        &session.spells,
        &format!("hub:{}", session.participants[local_player_id].wizard.region.id()),
        session.ticks,
        &|_| 1.0,
        &[],
        clocks,
        session.enhanced_effects,
        Some(solomon_sim::spells::weld::FlameLashBounds {
            x: base_camera.origin.x,
            y: base_camera.origin.y,
            width: viewport.x / base_camera.scale,
            height: viewport.y / base_camera.scale,
        }),
    );
    let marker_availability = HubInteractionAvailability {
        skorcha_position: session.npcs.skorcha.skorcha.as_ref().map(|skorcha| skorcha.position),
        skorcha_variant: session.npcs.skorcha.skorcha.as_ref().map(|skorcha| skorcha.variant),
        story_office: player.economy.college_intro_pending,
    };
    let mut actors = player_painters;
    let teacher_start = actors.len();
    for (painter, _) in &teacher_painters {
        actors.push(*painter);
    }
    let npc_start = actors.len();
    for (painter, _) in &npc_painters {
        actors.push(*painter);
    }
    let spell_start = actors.len();
    let spell_painters: Vec<_> =
        spell_view.painters().map(|(index, painter)| (index, painter.layer)).collect();
    actors.extend(spell_painters.iter().map(|(_, layer)| *layer));
    let overlays: Vec<_> = spell_view
        .painters()
        .enumerate()
        .filter_map(|(offset, (_, painter))| {
            let owner = painter.overlay_owner_id.as_deref()?;
            let owner_draw_id =
                hub_players.iter().position(|participant| participant.id == owner)?;
            Some(PainterOwnerOverlay {
                draw_id: spell_start + offset,
                owner_draw_id,
                view_id: painter.view_id,
            })
        })
        .collect();
    let live_views: Vec<_> = spell_view.live_view_creation_orders().collect();
    let courtyard = CourtyardActors {
        teacher,
        teacher_start,
        teacher_painters,
        npc_start,
        npc_painters,
        spell_start,
        spell_painters,
        marker_availability,
    };
    scene.draw(
        renderer,
        camera,
        &session.ambient,
        session.participants[local_player_id].wizard.position.y,
        &ActorPainters { layers: &actors, owner_overlays: &overlays, live_views: &live_views },
        |renderer, pass| match pass {
            CourtyardPass::Actor(index) => {
                draw_courtyard_actor(renderer, wizard_view, spell_view, hub, &courtyard, index)
            }
            CourtyardPass::Band(HubDepth::TeacherPreWorld) => {
                teacher_view.draw(renderer, camera, &courtyard.teacher, TeacherPass::Flare);
                spell_view.draw_queue(renderer, camera, SpellDrawQueue::PreWorld);
            }
            CourtyardPass::Band(HubDepth::TeacherPostWorld) => {
                teacher_view.draw(renderer, camera, &courtyard.teacher, TeacherPass::Core);
                spell_view.draw_queue(renderer, camera, SpellDrawQueue::PostWorld);
            }
            CourtyardPass::Band(HubDepth::UsefulThyngsShadow) => {
                npc_view.draw_tent_shadow(renderer, camera);
            }
            CourtyardPass::Band(HubDepth::CourtyardOnboarding) => guidance.draw_world(renderer),
            CourtyardPass::Band(_) => {}
        },
    );
}

/// One courtyard actor at its painter row: a participant, a teacher pass and its marker, an
/// NPC and its marker, or a primary spell.
fn draw_courtyard_actor(
    renderer: &mut Renderer,
    wizard_view: &mut WizardView,
    spell_view: &SpellView,
    hub: &HubDraw<'_>,
    courtyard: &CourtyardActors,
    index: usize,
) {
    let HubDraw {
        session,
        local_player_id,
        hub_players,
        hub_equipment,
        camera,
        presentation_tick,
        clocks,
        settings,
        teacher_view,
        npc_view,
        ..
    } = *hub;
    let CourtyardActors { teacher_start, npc_start, spell_start, marker_availability, .. } =
        *courtyard;
    if index < teacher_start {
        let participant = hub_players[index];
        prepare_hub_wizard(
            wizard_view,
            participant,
            &hub_equipment[index],
            session.secondary.players.get(&participant.id),
            session.secondary.stoneskin_warp.as_deref(),
            session.enhanced_effects,
            settings.complex_lighting,
            presentation_tick,
            clocks,
        );
        wizard_view.draw(renderer, camera, &participant.wizard, presentation_tick, spell_view);
    } else if index < npc_start {
        let pass = courtyard.teacher_painters[index - teacher_start].1;
        teacher_view.draw(renderer, camera, &courtyard.teacher, pass);
        if matches!(pass, TeacherPass::Actor) {
            npc_view.draw_courtyard_marker(
                renderer,
                camera,
                session.participants[local_player_id].marker_epoch.frame(
                    HubInteractionId::Teacher,
                    session.ticks,
                    marker_availability,
                    None,
                ),
            );
        }
    } else if index < spell_start {
        let actor = courtyard.npc_painters[index - npc_start].1;
        npc_view.draw_actor(renderer, camera, &session.npcs, actor);
        if let Some(id) = actor.interaction() {
            npc_view.draw_courtyard_marker(
                renderer,
                camera,
                session.participants[local_player_id].marker_epoch.frame(
                    id,
                    session.ticks,
                    marker_availability,
                    None,
                ),
            );
        }
    } else {
        spell_view.draw(
            renderer,
            camera,
            courtyard.spell_painters[index - spell_start].0,
            [1.0; 3],
        );
    }
}

/// One shared view is prepared from each actual Hub participant immediately before drawing.
#[allow(clippy::too_many_arguments)] // One accepted participant plus shared world material inputs.
fn prepare_hub_wizard(
    view: &mut WizardView,
    participant: &solomon_sim::session::SessionParticipant,
    equipment: &crate::wizard::WizardEquipment,
    secondary: Option<&solomon_sim::secondary::SecondaryPlayerState>,
    stoneskin_warp: Option<&[f64]>,
    enhanced_effects: bool,
    complex_lighting: bool,
    tick: f64,
    clocks: Option<&crate::presentation::PresentationClocks>,
) {
    let player = &participant.player;
    view.set_equipment(equipment);
    view.set_material_tint(solomon_sim::presentation::player_material_tint(
        0xffffff,
        secondary,
        &player.combat,
    ));
    view.set_player_presentation(WizardPlayerPresentation {
        player,
        secondary,
        stoneskin_warp,
        enhanced_effects,
        webbed: None,
        overlay_effect_phase: participant.lighting.overlay_effect_phase,
        staff_action_pose: None,
        complex_lighting,
        tick,
        damage_x4_ticks_remaining: clocks
            .and_then(|clocks| clocks.player_damage_x4.get(&participant.id))
            .copied()
            .unwrap_or(player.progression.damage_x4_ticks_remaining as f64),
    });
}
