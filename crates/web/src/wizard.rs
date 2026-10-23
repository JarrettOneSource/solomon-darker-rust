//! Native wizard clothing, attachment and orb passes. Clothes records retain
//! their authored socket endpoints and painter-depth point.

use solomon_assets::SpriteRecord;
use solomon_sim::Vec2;
use solomon_sim::appearance::StarterEquipmentAppearance;
use solomon_sim::appearance::equipment::*;
use solomon_sim::boneyard::ground_glyph::{
    NativeGroundGlyphPlan, NativeGroundPlayer, native_player_ground_glyph,
};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::economy::HubEquipmentState;
use solomon_sim::element_vfx::{native_element_vfx_plan, native_selected_primary_element_vfx_plan};
use solomon_sim::enemies::model::WebbedState;
use solomon_sim::float::{fround, hypot};
use solomon_sim::hub::HubWizard;
use solomon_sim::player::PlayerState;
use solomon_sim::player_presentation::*;
use solomon_sim::presentation::{multiply_native_tints, wizard_draw_plan};
use solomon_sim::profile::WizardElement;
use solomon_sim::secondary::NativeSecondaryPlayerState;
use solomon_sim::secondary::presentation::{
    NativePlayerMagicShieldPlan, native_player_magic_shield_plan,
};
use solomon_sim::spells::casting::*;

use crate::courtyard::{Camera, registered_origin};
use crate::mod_presentation_assets::{
    ModSpriteDraw, ModWearableLayer, ModWearableTextureFrames, mod_wearable_frame,
};
use crate::spells::SpellView;
use solomon_graphics::{BlendMode, Quad, RenderTarget, Renderer, Texture};

mod equipment;
mod material;

pub use equipment::WizardEquipment;

const PLAYER_CELL: f64 = 170.0;
const HEADINGS: usize = 24;
const DYNAMIC_ROBE_PRIMARY_BASE: usize = 868;
const DYNAMIC_ROBE_SECONDARY_BASE: usize = 1228;
const FIXED_ROBE_PRIMARY_BASES: [usize; 2] = [1612, 2428];
const FIXED_ROBE_SECONDARY_BASES: [usize; 2] = [2020, 2836];
const HAT_PRIMARY_BASES: [usize; 4] = [316, 340, 364, 388];
const HAT_SECONDARY_BASES: [usize; 4] = [412, 412, 412, 436];

#[derive(Clone, Copy)]
struct WeaponFrame<'a> {
    position: Vec2,
    heading: usize,
    pose: usize,
    wand_pose: usize,
    front_offset: Vec2,
    weapon: Option<&'a PlayerRenderableWeaponAppearance>,
    unselected: bool,
    bare: bool,
    material_tint: u32,
    tick: f64,
    enchant: bool,
}

/// The actual participant and source presentation clock, sampled once for this draw.
pub struct WizardPlayerPresentation<'a> {
    pub player: &'a PlayerState,
    pub secondary: Option<&'a NativeSecondaryPlayerState>,
    pub webbed: Option<WebbedState>,
    pub overlay_effect_phase: f64,
    pub staff_action_pose: Option<usize>,
    pub complex_lighting: bool,
    pub tick: f64,
    pub damage_x4_ticks_remaining: f64,
}

#[derive(Default)]
struct WizardPresentationState {
    enchant_primary_skill_id: Option<u32>,
    enchant_staff_rank: u32,
    overlay_effect_phase: f64,
    staff_action_pose: Option<usize>,
    secondary_cast: Option<PlayerSecondaryCastPresentation>,
    plane_active: bool,
    weld_build_id: Option<u32>,
    harden_coating: f64,
    webbed: Option<WebbedState>,
    hit_alpha: f64,
    complex_lighting: bool,
    magic_shield: Option<NativePlayerMagicShieldPlan>,
    damage_x4_ticks_remaining: f64,
    ground_life_state: Option<PlayerLifeState>,
    ground_death_tick: u64,
    ground_corpse_consumed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WizardDrawPass {
    World,
    Preview,
    Diffuse,
    Hit,
}

pub struct WizardView {
    records: Vec<SpriteRecord>,
    atlas: Texture,
    shadow: Texture,
    attachment_back: Texture,
    attachment_front: Texture,
    robe_attachment: Texture,
    equipment: WizardEquipment,
    material_tint: u32,
    player_presentation: WizardPresentationState,
    diffuse_capture: RenderTarget,
}

impl WizardView {
    #[allow(clippy::too_many_arguments)] // Existing atlas banks and the renderer-owned material capture.
    pub fn new(
        records: Vec<SpriteRecord>,
        atlas: Texture,
        shadow: Texture,
        appearance: StarterEquipmentAppearance,
        attachment_back: Texture,
        attachment_front: Texture,
        robe_attachment: Texture,
        renderer: &Renderer,
    ) -> Result<Self, String> {
        if records.len() < 3724 {
            return Err("Clothes.bundle is missing the native wizard attachment banks".into());
        }
        Ok(Self {
            records,
            atlas,
            shadow,
            attachment_back,
            attachment_front,
            robe_attachment,
            equipment: WizardEquipment::starter(appearance),
            material_tint: 0xffffff,
            player_presentation: WizardPresentationState::default(),
            diffuse_capture: renderer.create_render_target(256)?,
        })
    }

    pub fn set_appearance(&mut self, appearance: StarterEquipmentAppearance) {
        let garment = PlayerEquipmentTintedSelector {
            primary_tint: appearance.primary_tint,
            secondary_tint: appearance.secondary_tint,
            selector: 0,
        };
        self.equipment.appearance.hat = Some(PlayerRenderableGarmentAppearance::Native(garment));
        self.equipment.appearance.robe = Some(PlayerRenderableGarmentAppearance::Native(garment));
        self.equipment.hat = None;
        self.equipment.robe = None;
    }

    pub fn set_equipment(&mut self, equipment: &WizardEquipment) {
        self.equipment = equipment.clone();
    }

    pub fn set_material_tint(&mut self, tint: u32) {
        self.material_tint = tint;
    }

    /// Presentation borrows the canonical participant. Lighting's overlay phase
    /// is distinct from the primary action's weapon pulse.
    pub fn set_player_presentation(&mut self, input: WizardPlayerPresentation<'_>) {
        let WizardPlayerPresentation {
            player,
            secondary,
            webbed,
            overlay_effect_phase,
            staff_action_pose,
            complex_lighting,
            tick,
            damage_x4_ticks_remaining,
        } = input;
        assert!(
            staff_action_pose.is_none_or(|pose| pose < 10),
            "native staff pose is within 0..10"
        );
        let secondary_cast = secondary.and_then(|state| {
            if let Some(action) = state.cast_action {
                Some(PlayerSecondaryCastPresentation::Action(action))
            } else if state.cast_spin_ticks_remaining > 0 {
                Some(PlayerSecondaryCastPresentation::Spin)
            } else {
                None
            }
        });
        let plane_active = secondary.is_some_and(|state| state.planewalker_ticks_remaining > 0);
        let stoneskin = secondary.is_some_and(|state| state.stoneskin_ticks_remaining > 0);
        self.player_presentation = WizardPresentationState {
            enchant_primary_skill_id: Some(if plane_active {
                80
            } else {
                player.book.primary_skill_id as u32
            }),
            enchant_staff_rank: player.book.effective_ranks[65],
            overlay_effect_phase,
            staff_action_pose,
            secondary_cast,
            plane_active,
            weld_build_id: if plane_active { None } else { player.book.weld_build_id },
            harden_coating: if stoneskin { 0.0 } else { player.skill_runtime.harden.coating },
            webbed: if stoneskin || player.skill_runtime.harden.coating > 0.0 || plane_active {
                None
            } else {
                webbed
            },
            hit_alpha: player.combat.hit_overlay_alpha(tick),
            complex_lighting,
            magic_shield: Some(native_player_magic_shield_plan(secondary, tick)),
            damage_x4_ticks_remaining,
            ground_life_state: Some(player.combat.life_state),
            ground_death_tick: player.combat.death_tick,
            ground_corpse_consumed: player.combat.corpse_consumed,
        };
    }

    pub fn draw(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
    ) {
        self.draw_ground(
            renderer,
            camera,
            native_player_ground_glyph(
                NativeGroundPlayer {
                    position: wizard.position,
                    heading_degrees: wizard.heading_degrees,
                    scale: 1.0,
                    life_state: self
                        .player_presentation
                        .ground_life_state
                        .unwrap_or(PlayerLifeState::Alive),
                    death_tick: self.player_presentation.ground_death_tick,
                    corpse_consumed: self.player_presentation.ground_corpse_consumed,
                },
                false,
            ),
        );
        self.draw_body(renderer, camera, wizard, tick, spells);
    }

    /// Boneyard owns a separate player-ground group before its native actor rows.
    pub fn draw_body(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
    ) {
        self.draw_living_layers(renderer, camera, wizard, tick, spells, WizardDrawPass::World);
        self.draw_material_capture(renderer, camera, wizard, tick, spells);
    }

    pub fn draw_ground(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        plan: Option<NativeGroundGlyphPlan>,
    ) {
        let Some(plan) = plan else {
            return;
        };
        let width = 25.0 * plan.scale_x;
        let height = 25.0 * plan.scale_y;
        renderer.draw(
            &self.shadow,
            Quad {
                source: [0.0, 0.0, 25.0, 25.0],
                destination: camera.project(
                    plan.position.x - width / 2.0,
                    plan.position.y - height / 2.0,
                    width,
                    height,
                ),
                tint: [1.0, 1.0, 1.0, ((plan.alpha * 255.0).trunc() / 255.0) as f32],
                blend: BlendMode::Normal,
            },
        );
    }

    /// Inventory's authored pose: heading nine, robe zero, fixed zero (fourteen
    /// for a wand), no world shadow and a base element orb at phase zero.
    pub fn draw_preview(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        spells: &SpellView,
    ) {
        let mut wizard = HubWizard::at(position);
        wizard.set_facing(135.0);
        self.draw_actor(renderer, camera, &wizard, 0.0, spells, WizardDrawPass::Preview);
    }

    fn draw_actor(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
        pass: WizardDrawPass,
    ) {
        let preview = pass == WizardDrawPass::Preview;
        let hit = pass == WizardDrawPass::Hit;
        let position = wizard.position;
        let heading = wizard.heading_index;
        let cast = &wizard.primary_cast;
        let element = selected_cast_element(cast.selected_primary_id, self.equipment.element);
        let appearance = &self.equipment.appearance;
        let presentation = &self.player_presentation;
        let poses = player_character_attachment_poses(
            cast,
            element,
            presentation.staff_action_pose,
            presentation.secondary_cast,
        );
        let pose = if preview { 0 } else { poses.staff };
        let wand_pose = if preview { 0 } else { poses.wand };
        let unselected = !preview && cast.selected_primary_id.is_none();
        let offsets = wizard_draw_plan(heading, wizard.walk_cycle_primary, wizard.gait_degrees);
        let native_robe =
            matches!(appearance.robe, Some(PlayerRenderableGarmentAppearance::Native(_)));
        let fixed_pose = if unselected && native_robe {
            13
        } else if appearance
            .weapon
            .as_ref()
            .is_some_and(|weapon| weapon.kind() == NativePlayerWeaponKind::Wand)
        {
            14 + wand_pose
        } else {
            pose
        };
        let defaults = player_death_equipment_appearance(
            self.equipment.element,
            &HubEquipmentState::default(),
        );
        // Inventory preview sprites use their item colours directly, independent
        // of the current world's light/status material installed on this view.
        let material_tint = if preview { 0xffffff } else { self.material_tint };
        let robe_tints = appearance.robe.as_ref().map_or(
            [defaults.robe.primary_tint, defaults.robe.secondary_tint],
            PlayerRenderableGarmentAppearance::tints,
        );
        let hat_tints = appearance.hat.as_ref().map_or(
            [defaults.hat.primary_tint, defaults.hat.secondary_tint],
            PlayerRenderableGarmentAppearance::tints,
        );
        let mut tints = [robe_tints[0], robe_tints[1], hat_tints[0], hat_tints[1]];
        for tint in &mut tints {
            *tint = multiply_native_tints(*tint, material_tint);
            if hit {
                *tint = self.hit_tint(*tint);
            }
        }
        let [robe_primary_tint, robe_secondary_tint, hat_primary_tint, hat_secondary_tint] = tints;
        let frame = WeaponFrame {
            position,
            heading,
            pose,
            wand_pose,
            front_offset: offsets.front_attachment_offset,
            weapon: appearance.weapon.as_ref(),
            unselected,
            bare: preview || poses.bare,
            material_tint: if hit { self.hit_tint(material_tint) } else { material_tint },
            tick,
            enchant: !preview && !hit,
        };
        self.draw_weapon(renderer, camera, frame, false);
        let robe_pose = if preview { 0 } else { offsets.robe_pose % 5 };
        if let Some(bank) = &self.equipment.robe {
            self.draw_mod_layer(
                renderer,
                camera,
                position,
                bank,
                ModWearableLayer::Primary,
                heading,
                robe_pose,
                Vec2::ZERO,
                robe_primary_tint,
            );
            if bank.secondary.is_some() {
                self.draw_mod_layer(
                    renderer,
                    camera,
                    position,
                    bank,
                    ModWearableLayer::Secondary,
                    heading,
                    robe_pose,
                    Vec2::ZERO,
                    robe_secondary_tint,
                );
            }
        } else {
            let selector = match &appearance.robe {
                Some(PlayerRenderableGarmentAppearance::Native(robe)) => robe.selector,
                None => defaults.robe.selector,
                Some(PlayerRenderableGarmentAppearance::Mod(_)) => {
                    unreachable!("custom robe was prepared")
                }
            };
            for (base, tint) in [
                (DYNAMIC_ROBE_PRIMARY_BASE, robe_primary_tint),
                (DYNAMIC_ROBE_SECONDARY_BASE, robe_secondary_tint),
            ] {
                self.draw_cell_layer(
                    renderer,
                    camera,
                    position,
                    base + selector * 5 * HEADINGS + robe_pose * HEADINGS + heading,
                    Vec2::ZERO,
                    tint,
                );
            }
        }
        if unselected && native_robe {
            self.draw_attachment(
                renderer,
                camera,
                position,
                &self.robe_attachment,
                [0, heading],
                offsets.fixed_robe_offset,
                if hit { self.hit_tint(material_tint) } else { material_tint },
            );
        }
        let fixed_index = fixed_pose * HEADINGS + heading;
        let robe_has_secondary =
            self.equipment.robe.as_ref().is_none_or(|bank| bank.secondary.is_some());
        for (base, tint, secondary) in [
            (FIXED_ROBE_PRIMARY_BASES[0], robe_primary_tint, false),
            (FIXED_ROBE_PRIMARY_BASES[1], robe_primary_tint, false),
            (FIXED_ROBE_SECONDARY_BASES[0], robe_secondary_tint, true),
            (FIXED_ROBE_SECONDARY_BASES[1], robe_secondary_tint, true),
        ] {
            if secondary && !robe_has_secondary {
                continue;
            }
            self.draw_cell_layer(
                renderer,
                camera,
                position,
                base + fixed_index,
                offsets.fixed_robe_offset,
                tint,
            );
        }
        self.draw_weapon(renderer, camera, frame, true);
        if !hit
            && appearance
                .weapon
                .as_ref()
                .is_some_and(|weapon| weapon.kind() == NativePlayerWeaponKind::Staff)
            && (preview || !unselected)
        {
            let socket = player_staff_attachment_offset(heading, pose);
            let orb = if preview {
                Vec2::new(position.x + socket.x, position.y + socket.y)
            } else {
                Vec2::new(
                    position.x + socket.x + offsets.front_attachment_offset.x,
                    position.y + socket.y + offsets.front_attachment_offset.y,
                )
            };
            if preview {
                spells.draw_element_vfx(
                    renderer,
                    camera,
                    orb,
                    &native_element_vfx_plan(self.equipment.element, 0.0, 1.0),
                );
            } else {
                let passes = player_character_staff_orb_passes(
                    heading,
                    pose,
                    presentation.overlay_effect_phase,
                );
                let scale = player_equipped_element_effect_scale(presentation.overlay_effect_phase);
                let plan = native_selected_primary_element_vfx_plan(
                    if presentation.plane_active { 80 } else { cast.selected_primary_id.unwrap() },
                    tick,
                    scale,
                );
                for visible in [passes.front_base, passes.front_overlay] {
                    if visible {
                        self.draw_damage_x4(renderer, camera, orb, tick, scale, spells);
                        spells.draw_element_vfx(renderer, camera, orb, &plan);
                    }
                }
            }
        }
        if let Some(bank) = &self.equipment.hat {
            self.draw_mod_layer(
                renderer,
                camera,
                position,
                bank,
                ModWearableLayer::Primary,
                heading,
                0,
                offsets.head_offset,
                hat_primary_tint,
            );
            if bank.secondary.is_some() {
                self.draw_mod_layer(
                    renderer,
                    camera,
                    position,
                    bank,
                    ModWearableLayer::Secondary,
                    heading,
                    0,
                    offsets.head_offset,
                    hat_secondary_tint,
                );
            }
        } else {
            let selector = match &appearance.hat {
                Some(PlayerRenderableGarmentAppearance::Native(hat)) => hat.selector,
                None => defaults.hat.selector,
                Some(PlayerRenderableGarmentAppearance::Mod(_)) => {
                    unreachable!("custom hat was prepared")
                }
            };
            for (base, tint) in [
                (HAT_PRIMARY_BASES[selector], hat_primary_tint),
                (HAT_SECONDARY_BASES[selector], hat_secondary_tint),
            ] {
                self.draw_cell_layer(
                    renderer,
                    camera,
                    position,
                    base + heading,
                    offsets.head_offset,
                    tint,
                );
            }
        }
    }

    fn draw_weapon(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        frame: WeaponFrame<'_>,
        front: bool,
    ) {
        let offset = if front { frame.front_offset } else { Vec2::ZERO };
        if frame.unselected {
            self.draw_attachment(
                renderer,
                camera,
                frame.position,
                if front { &self.attachment_front } else { &self.attachment_back },
                [4, frame.heading],
                offset,
                frame.material_tint,
            );
            return;
        }
        match frame.weapon {
            Some(PlayerRenderableWeaponAppearance::ModStaff(_)) => {
                if player_character_staff_is_front(frame.heading, frame.pose) != front {
                    return;
                }
                let bank = self.equipment.staff.as_ref().expect("custom staff was prepared");
                self.draw_mod_layer(
                    renderer,
                    camera,
                    frame.position,
                    bank,
                    ModWearableLayer::Primary,
                    frame.heading,
                    frame.pose,
                    offset,
                    frame.material_tint,
                );
            }
            Some(PlayerRenderableWeaponAppearance::Native(PlayerWeaponAppearance {
                kind: NativePlayerWeaponKind::Staff,
                selector,
            })) => {
                let index = frame.pose * HEADINGS + frame.heading;
                let hand = &self.records[3244 + index];
                if (hand.points[0][1] > 0.5) != front {
                    return;
                }
                self.draw_segment(
                    renderer,
                    camera,
                    frame.position,
                    5 + *selector,
                    hand.points[1],
                    hand.points[2],
                    offset,
                    frame.material_tint,
                    BlendMode::Normal,
                );
                let presentation = &self.player_presentation;
                let enchant = if !frame.enchant {
                    None
                } else {
                    native_enchant_staff_draw_plan(NativeEnchantStaffDrawInput {
                        attachment: NativeStaffAttachmentFrame {
                            start: Vec2::new(
                                f64::from(hand.points[1][0]),
                                f64::from(hand.points[1][1]),
                            ),
                            end: Vec2::new(
                                f64::from(hand.points[2][0]),
                                f64::from(hand.points[2][1]),
                            ),
                            front,
                        },
                        effective_rank: presentation.enchant_staff_rank,
                        heading_index: frame.heading,
                        living: true,
                        native_staff: true,
                        pose: frame.pose,
                        selected_primary_skill_id: presentation.enchant_primary_skill_id,
                        selector: *selector,
                        tick: frame.tick,
                        weld_build_id: presentation.weld_build_id,
                        width_sample: None,
                    })
                };
                if let Some(plan) = enchant {
                    self.draw_segment(
                        renderer,
                        camera,
                        frame.position,
                        5 + *selector,
                        hand.points[1],
                        hand.points[2],
                        offset,
                        frame.material_tint,
                        BlendMode::Add,
                    );
                    self.draw_staff_aura(renderer, camera, frame.position, offset, plan);
                }
                self.draw_cell_layer(
                    renderer,
                    camera,
                    frame.position,
                    3244 + index,
                    offset,
                    frame.material_tint,
                );
                self.draw_cell_layer(
                    renderer,
                    camera,
                    frame.position,
                    3484 + index,
                    offset,
                    frame.material_tint,
                );
            }
            Some(PlayerRenderableWeaponAppearance::Native(PlayerWeaponAppearance {
                kind: NativePlayerWeaponKind::Wand,
                ..
            })) => {
                // The pinned Website installs the same complete wand composite in both
                // passes. Preserve its hand/body order and front gait offset.
                let index = frame.wand_pose * HEADINGS + frame.heading;
                self.draw_cell_layer(
                    renderer,
                    camera,
                    frame.position,
                    604 + index,
                    offset,
                    frame.material_tint,
                );
                self.draw_cell_layer(
                    renderer,
                    camera,
                    frame.position,
                    796 + index,
                    offset,
                    frame.material_tint,
                );
                let points = &self.records[796 + index].points;
                self.draw_segment(
                    renderer,
                    camera,
                    frame.position,
                    15,
                    points[0],
                    points[1],
                    offset,
                    frame.material_tint,
                    BlendMode::Normal,
                );
            }
            None if frame.bare => {
                for base in [484, 676] {
                    let index = base + frame.heading;
                    if (self.records[index].points[0][1] > 0.5) == front {
                        self.draw_cell_layer(
                            renderer,
                            camera,
                            frame.position,
                            index,
                            offset,
                            frame.material_tint,
                        );
                    }
                }
            }
            None => {}
        }
    }

    #[allow(clippy::too_many_arguments)] // One validated bank frame, actor transform and tint.
    fn draw_mod_layer(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        bank: &ModWearableTextureFrames,
        layer: ModWearableLayer,
        heading: usize,
        pose: usize,
        offset: Vec2,
        tint: u32,
    ) {
        let texture = mod_wearable_frame(bank, layer, heading, pose)
            .expect("prepared custom wearable has the current heading and layer");
        let center = camera.project(position.x + offset.x, position.y + offset.y, 0.0, 0.0);
        texture.draw(
            renderer,
            &ModSpriteDraw {
                center: Vec2::new(f64::from(center[0]), f64::from(center[1])),
                scale: Vec2::new(camera.scale, camera.scale),
                anchor: Vec2::new(0.5, 0.5),
                rotation_radians: 0.0,
                tint: tint_rgba(tint),
                blend: BlendMode::Normal,
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_segment(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        index: usize,
        start: [f32; 2],
        end: [f32; 2],
        offset: Vec2,
        material_tint: u32,
        blend: BlendMode,
    ) {
        let record = &self.records[index];
        let start = Vec2::new(f64::from(start[0]), f64::from(start[1]));
        let end = Vec2::new(f64::from(end[0]), f64::from(end[1]));
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = hypot(dx, dy);
        let half_width = f64::from(record.logical_width) / 2.0;
        let side = Vec2::new(-dy / length * half_width, dx / length * half_width);
        let points = [
            Vec2::new(start.x - side.x, start.y - side.y),
            Vec2::new(start.x + side.x, start.y + side.y),
            Vec2::new(end.x + side.x, end.y + side.y),
            Vec2::new(end.x - side.x, end.y - side.y),
        ];
        let corners = points.map(|point| {
            let p = camera.project(
                position.x + offset.x + point.x,
                position.y + offset.y + point.y,
                0.0,
                0.0,
            );
            [p[0], p[1]]
        });
        renderer.draw_quad(
            &self.atlas,
            [record.x as f32, record.y as f32, record.width as f32, record.height as f32],
            corners,
            [tint_rgba(material_tint); 4],
            blend,
        );
    }

    fn draw_staff_aura(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        offset: Vec2,
        plan: NativeEnchantStaffDrawPlan,
    ) {
        let Some(index) = plan.aura_record else {
            return;
        };
        let record = &self.records[index];
        let points = [plan.vertices[0], plan.vertices[1], plan.vertices[3], plan.vertices[2]];
        let corners = points.map(|point| {
            let projected = camera.project(
                position.x + offset.x + point.x,
                position.y + offset.y + point.y,
                0.0,
                0.0,
            );
            [projected[0], projected[1]]
        });
        let mut near = tint_rgba(plan.tint);
        let mut far = near;
        near[3] = ((plan.near_alpha.clamp(0.0, 1.0) * 255.0).trunc() / 255.0) as f32;
        far[3] = ((plan.far_alpha.clamp(0.0, 1.0) * 255.0).trunc() / 255.0) as f32;
        renderer.draw_quad(
            &self.atlas,
            [record.x as f32, record.y as f32, record.width as f32, record.height as f32],
            corners,
            [near, near, far, far],
            BlendMode::Add,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_attachment(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        texture: &Texture,
        frame: [usize; 2],
        offset: Vec2,
        material_tint: u32,
    ) {
        renderer.draw(
            texture,
            Quad {
                source: [
                    (frame[0] as f64 * PLAYER_CELL) as f32,
                    (frame[1] as f64 * PLAYER_CELL) as f32,
                    PLAYER_CELL as f32,
                    PLAYER_CELL as f32,
                ],
                destination: camera.project(
                    position.x + offset.x - PLAYER_CELL / 2.0,
                    position.y + offset.y - PLAYER_CELL / 2.0,
                    PLAYER_CELL,
                    PLAYER_CELL,
                ),
                tint: tint_rgba(material_tint),
                blend: BlendMode::Normal,
            },
        );
    }

    fn draw_cell_layer(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        index: usize,
        offset: Vec2,
        tint: u32,
    ) {
        let record = &self.records[index];
        let [left, top] = registered_origin(record);
        let cell_left = left + ((PLAYER_CELL - f64::from(record.logical_width)) / 2.0).floor();
        let cell_top = top + ((PLAYER_CELL - f64::from(record.logical_height)) / 2.0).floor();
        renderer.draw(
            &self.atlas,
            Quad {
                source: [
                    record.x as f32,
                    record.y as f32,
                    record.width as f32,
                    record.height as f32,
                ],
                destination: camera.project(
                    position.x + offset.x + cell_left - PLAYER_CELL / 2.0,
                    position.y + offset.y + cell_top - PLAYER_CELL / 2.0,
                    f64::from(record.width),
                    f64::from(record.height),
                ),
                tint: tint_rgba(tint),
                blend: BlendMode::Normal,
            },
        );
    }
}

fn tint_rgba(tint: u32) -> [f32; 4] {
    [
        ((tint >> 16) & 255) as f32 / 255.0,
        ((tint >> 8) & 255) as f32 / 255.0,
        (tint & 255) as f32 / 255.0,
        1.0,
    ]
}

fn selected_cast_element(selected: Option<u32>, fallback: WizardElement) -> WizardElement {
    match selected {
        Some(8) => WizardElement::Ether,
        Some(16 | 1000..=1009) => WizardElement::Fire,
        Some(24) => WizardElement::Air,
        Some(32) => WizardElement::Water,
        Some(40) => WizardElement::Earth,
        _ => fallback,
    }
}
