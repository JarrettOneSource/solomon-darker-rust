//! Source Flash/Mindblast materialization and scripted fires. Pure harmful-hit
//! decisions live in `harmful`; this module turns admitted responses into actors.

use super::program::*;
use super::{
    NativeSecondaryActorKind as Kind, NativeSecondaryAudioCue as Cue,
    NativeSecondaryEventKind as Event, NativeSecondaryEventState, NativeSecondaryScreenFlashState,
    NativeSecondarySimulationState, apply_native_secondary_dazzle,
};
use crate::Vec2;
use crate::float::fround;
use crate::harmful::PlayerFlashResponse;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::profile::WizardElement;

pub struct NativePlayerFlashResponseInput<'a> {
    pub owner_id: &'a str,
    pub position: Vec2,
    pub response: &'a PlayerFlashResponse,
    pub target_ids: &'a [usize],
    pub tick: u64,
    pub world_key: &'a str,
}

pub struct NativePlayerMindblastInput<'a> {
    pub direct_damage: Option<f64>,
    pub element: WizardElement,
    pub level: u32,
    pub light_registration: PainterRegistration,
    pub owner_id: &'a str,
    pub position: Vec2,
    pub presentation_scale: Option<f64>,
    pub world_key: &'a str,
}

pub struct NativePlayerRescueInput<'a> {
    pub light_registration: PainterRegistration,
    pub owner_id: &'a str,
    pub position: Vec2,
    pub world_key: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativePlayerMindblastTriggerResult {
    pub direct_damage: f64,
    pub direct_radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeScriptFire {
    pub damage: f64,
    pub lifetime_ticks: u64,
    pub position: Vec2,
    pub radius: f64,
}

impl NativeSecondarySimulationState {
    /// An admitted rescue owns its zero-damage gameplay wave. Replaying feedback
    /// does not recreate the wave or allocate another painter.
    pub fn spawn_player_rescue_shockwave(
        &mut self,
        input: NativePlayerRescueInput<'_>,
        order: &mut WorldPainterOrder,
    ) {
        let mut wave =
            actor(Kind::RescueShockwave, input.owner_id, input.world_key, None, 1, input.position);
        wave.radius = 175.0;
        wave.quantity = 6.0;
        wave.alpha = 2.0;
        wave.phase = 0.35_f32 as f64;
        wave.slow_factor = 0.0375_f32 as f64;
        wave.damage = 0.0;
        wave.lifetime_ticks = 100;
        wave.light_registration = Some(input.light_registration);
        wave.painter_registrations.push(order.register(WorldManagerLane::Transient));
        self.spawn_actor(wave);
    }

    pub fn materialize_player_flash_response(&mut self, input: NativePlayerFlashResponseInput<'_>) {
        let mut targets = input.target_ids.to_vec();
        targets.sort_unstable();
        targets.dedup();
        assert_eq!(targets.len(), input.target_ids.len(), "Flash response target IDs are unique");
        for id in input.target_ids {
            apply_native_secondary_dazzle(
                &mut self.target_effects,
                input.world_key,
                *id,
                input.response.duration_ticks,
            );
        }
        for scale in &input.response.grow_scales {
            assert!(
                scale.is_finite() && (1.0..=2.0).contains(scale),
                "Flash response grow scale is inside [1,2]"
            );
            let mut child = actor(
                Kind::FlashResponseGrow,
                input.owner_id,
                input.world_key,
                Some(53),
                1,
                input.position,
            );
            child.lifetime_ticks = 20;
            child.scale = *scale;
            self.spawn_actor(child);
        }
        for _ in 0..4 {
            let mut child = actor(
                Kind::FlashResponseFade,
                input.owner_id,
                input.world_key,
                Some(53),
                1,
                Vec2::new(input.position.x, fround(input.position.y - 25.0)),
            );
            child.lifetime_ticks = 20;
            child.scale = 6.0;
            self.spawn_actor(child);
        }
        let mut event = NativeSecondaryEventState::new(
            Event::Impact,
            input.owner_id.into(),
            input.world_key.into(),
            input.tick,
        );
        event.camera_displacement = Some(input.response.camera_displacement);
        event.cue = Some(Cue::FlashSpell);
        event.pitch = input.response.pitch;
        event.position = input.position;
        event.screen_flash = Some(screen_flash(1.0, 1.0, 1.0, 0.05, true));
        event.skill_id = Some(53);
        self.emit_event(event);
    }

    pub fn emit_player_screen_flash(
        &mut self,
        owner_id: &str,
        world_key: &str,
        position: Vec2,
        screen_flash: NativeSecondaryScreenFlashState,
        tick: u64,
    ) {
        let mut event =
            NativeSecondaryEventState::new(Event::Impact, owner_id.into(), world_key.into(), tick);
        event.position = position;
        event.screen_flash = Some(screen_flash);
        self.emit_event(event);
    }

    pub fn trigger_player_mindblast(
        &mut self,
        input: NativePlayerMindblastInput<'_>,
        mut order: Option<&mut WorldPainterOrder>,
    ) -> NativePlayerMindblastTriggerResult {
        assert!(input.level > 0, "Mindblast level is positive");
        let scale = input.presentation_scale.unwrap_or(9.0);
        let damage = input.direct_damage.unwrap_or(if input.element == WizardElement::Ether {
            f64::from(input.level) * 0.5
        } else {
            0.0
        });
        assert!(
            scale.is_finite() && scale > 0.0,
            "Mindblast presentation scale is finite and positive"
        );
        assert!(
            damage.is_finite() && damage >= 0.0,
            "Mindblast direct damage is finite and non-negative"
        );
        let presentation_rng = self.rng.clone();
        advance_rng(&mut self.rng, 502);
        let variant = match input.element {
            WizardElement::Ether => 0,
            WizardElement::Fire => 1,
            WizardElement::Air => 2,
            WizardElement::Water => 3,
            WizardElement::Earth => 4,
        };
        let mut fallback_ordinal = self.next_actor_id as usize;
        let mut register = || {
            if let Some(order) = order.as_deref_mut() {
                order.register(WorldManagerLane::Transient)
            } else {
                let registration = PainterRegistration {
                    manager_lane: WorldManagerLane::Transient,
                    registration_ordinal: fallback_ordinal,
                };
                fallback_ordinal += 1;
                registration
            }
        };
        let mut burst = actor(
            Kind::MindblastBurst,
            input.owner_id,
            input.world_key,
            None,
            input.level,
            input.position,
        );
        burst.lifetime_ticks = 230;
        burst.painter_registrations.push(register());
        burst.presentation_rng = Some(presentation_rng);
        burst.scale = scale;
        burst.variant = variant;
        self.spawn_actor(burst);
        let mut wave = actor(
            Kind::MindblastShockwave,
            input.owner_id,
            input.world_key,
            None,
            1,
            input.position,
        );
        wave.lifetime_ticks = 36;
        wave.light_registration = Some(input.light_registration);
        wave.painter_registrations.push(register());
        wave.phase = 0.35_f32 as f64;
        wave.quantity = 8.0;
        wave.radius = 75.0;
        wave.slow_factor = 0.0375_f32 as f64;
        wave.variant = variant;
        self.spawn_actor(wave);
        NativePlayerMindblastTriggerResult {
            direct_damage: damage,
            direct_radius: fround(495.0 * scale / 9.0),
        }
    }

    pub fn spawn_script_fires(
        &mut self,
        owner_id: &str,
        world_key: &str,
        fires: &[NativeScriptFire],
        order: &mut WorldPainterOrder,
    ) {
        for fire in fires {
            let phase = self.rng.draw_scaled_float(32.0, false);
            let mirror = self.rng.draw_sign(1.0);
            let registration = order.register(WorldManagerLane::Actor);
            let mut child = actor(Kind::FirePatch, owner_id, world_key, Some(73), 1, fire.position);
            child.damage = fire.damage;
            child.enhanced = true;
            child.lifetime_ticks = fire.lifetime_ticks;
            child.light_registration = Some(registration);
            child.painter_registrations.push(registration);
            child.phase = phase;
            child.quantity = mirror;
            child.radius = 0.0;
            child.scale = fround(fire.radius / 100.0);
            child.slow_factor = fround(fire.lifetime_ticks as f64 * FIRE_LIFE_STEP);
            self.spawn_actor(child);
        }
    }
}
