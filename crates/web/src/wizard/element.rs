//! PlayerWizard selected-element helper `0x0053B1D0` and its actual callers.
//! Ordinary calls retain native angle/depth admission and body transforms;
//! Stone and Harden call once, with front=true, after the material.

use super::*;

struct BareElementPlan {
    socket: Vec2,
    scale: f64,
    angle: usize,
    front: bool,
    back: bool,
}

impl WizardView {
    fn selected_element_alive(&self, wizard: &HubWizard) -> bool {
        self.player_presentation.ground_life_state.is_none_or(|life| life == PlayerLifeState::Alive)
            && wizard.primary_cast.selected_primary_id.is_some()
    }

    pub(super) fn equipped_element_plan(
        &self,
        wizard: &HubWizard,
    ) -> Option<PlayerEquippedElementPlan> {
        if !self.selected_element_alive(wizard) {
            return None;
        }
        let kind = self.equipment.appearance.weapon.as_ref()?.kind();
        let state = &self.player_presentation;
        let poses = player_character_attachment_poses(
            &wizard.primary_cast,
            selected_cast_element(wizard.primary_cast.selected_primary_id, self.equipment.element),
            state.staff_action_pose,
            state.secondary_cast,
        );
        Some(player_equipped_element_plan(
            wizard.heading_index,
            poses,
            kind,
            state.secondary_cast,
            state.overlay_effect_phase,
            wizard.gait_degrees,
        ))
    }

    fn bare_element_plan(&self, wizard: &HubWizard, tick: f64) -> Option<BareElementPlan> {
        if !self.selected_element_alive(wizard) || self.equipment.appearance.weapon.is_some() {
            return None;
        }
        let state = &self.player_presentation;
        let pose = player_character_attachment_poses(
            &wizard.primary_cast,
            selected_cast_element(wizard.primary_cast.selected_primary_id, self.equipment.element),
            state.staff_action_pose,
            state.secondary_cast,
        )
        .bare?;
        if pose >= 4 || (pose == 2 && !wizard.primary_cast.channel_active) {
            return None;
        }
        let heading = wizard.heading_index;
        let first = &self.records[484 + pose * HEADINGS + heading];
        let second = &self.records[676 + pose * HEADINGS + heading];
        let reference = self.records[316].points[0][1] as f64 + 15.0;
        let depth = f64::from(first.points[0][1].max(second.points[0][1]));
        let native_pi = solomon_sim::boneyard::terrain_surface::NATIVE_GROUND_PI;
        let radians = fround(native_pi * fround(2.0 * tick.floor()) / 180.0);
        let bob = fround(solomon_sim::float::sin(radians));
        let query = self.records[484 + heading].points[1];
        Some(BareElementPlan {
            socket: Vec2::new(first.points[1][0] as f64, fround(first.points[1][1] as f64 + bob)),
            scale: fround(0.8_f32 as f64 * (1.0 + 10.0 * state.overlay_effect_phase)),
            angle: native_player_element_call_angle(
                Vec2::new(query[0] as f64, query[1] as f64),
                None,
            ),
            front: depth <= reference,
            back: depth >= reference,
        })
    }

    #[allow(clippy::too_many_arguments)] // Native helper arguments include depth side and body translation.
    pub(super) fn draw_selected_element(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
        offset: Vec2,
        front: bool,
    ) {
        let (socket, scale) = if let Some(element) = self.equipped_element_plan(wizard) {
            (element.socket, element.scale)
        } else if let Some(element) = self.bare_element_plan(wizard, tick) {
            if !(if front { element.front } else { element.back }) {
                return;
            }
            (element.socket, element.scale)
        } else {
            return;
        };
        let position = Vec2::new(
            wizard.position.x + socket.x + offset.x,
            wizard.position.y + socket.y + offset.y,
        );
        let primary = if self.player_presentation.plane_active {
            80
        } else {
            wizard.primary_cast.selected_primary_id.expect("helper has a selected primary")
        };
        self.draw_damage_x4(renderer, camera, position, tick, scale, spells);
        spells.draw_element_vfx(
            renderer,
            camera,
            position,
            &native_selected_primary_element_vfx_plan(primary, tick, scale),
        );
    }

    pub(super) fn draw_back_bare_element(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
    ) {
        if self.player_presentation.stoneskin {
            return;
        }
        if let Some(element) = self.bare_element_plan(wizard, tick)
            && (element.angle <= 90 || element.angle > 270)
        {
            self.draw_selected_element(renderer, camera, wizard, tick, spells, Vec2::ZERO, true);
        }
    }

    pub(super) fn draw_early_selected_element(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
    ) {
        if self.player_presentation.stoneskin {
            return;
        }
        let offset = if let Some(element) = self.equipped_element_plan(wizard) {
            (element.passes.front_overlay && !element.overlay_after_head)
                .then_some(element.attachment_offset)
        } else if let Some(element) = self.bare_element_plan(wizard, tick) {
            (self.player_presentation.overlay_effect_phase <= 0.1_f32 as f64
                && (element.angle <= 90 || element.angle > 270))
                .then(|| solomon_sim::presentation::front_attachment_offset(wizard.gait_degrees))
        } else {
            None
        };
        if let Some(offset) = offset {
            self.draw_selected_element(renderer, camera, wizard, tick, spells, offset, false);
        }
    }

    pub(super) fn draw_late_selected_elements(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
    ) {
        if self.player_presentation.stoneskin {
            return;
        }
        let (base, overlay, offset) = if let Some(element) = self.equipped_element_plan(wizard) {
            (
                element.passes.front_base,
                element.passes.front_overlay && element.overlay_after_head,
                element.attachment_offset,
            )
        } else if let Some(element) = self.bare_element_plan(wizard, tick) {
            (
                (90..=270).contains(&element.angle),
                self.player_presentation.overlay_effect_phase > 0.1_f32 as f64,
                solomon_sim::presentation::front_attachment_offset(wizard.gait_degrees),
            )
        } else {
            return;
        };
        if base {
            self.draw_selected_element(renderer, camera, wizard, tick, spells, Vec2::ZERO, false);
        }
        if overlay {
            self.draw_selected_element(renderer, camera, wizard, tick, spells, offset, false);
        }
    }
}
