//! StormCloud 006021A0 owns five bolt births per accepted hit. Each birth owns a
//! two-tick body and two FadeLightning children (00476230), which retire after
//! six float32 .2 subtractions. Cosmetic samples never advance gameplay RNG.

use super::step::StepEngine;
use super::{
    SecondaryActorKind, SecondaryActorState, SecondaryAudioCue, SecondaryEventKind, SecondaryWorld,
};
use crate::Vec2;
use crate::float::{self, fround};
use crate::spells::air::{
    AirCoronaInput, AirLightPlan, AirLightningPlan, AirPresentationRandom, AirRibbonFamily,
    AirRibbonInput, build_air_corona_plan, build_air_ribbon_with_style,
};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct StormBurst {
    #[cfg_attr(feature = "persistence", serde(deserialize_with = "remaining_ticks"))]
    pub remaining_ticks: u8,
    pub source: Vec2,
    pub midpoint: Vec2,
    pub endpoint: Vec2,
    pub target_id: usize,
}

pub(super) fn emit_pending<W: SecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    actor: &mut SecondaryActorState,
) {
    let Some(burst) = actor.storm_burst.as_mut() else { return };
    let mut strike = super::program::actor(
        SecondaryActorKind::StormStrike,
        &actor.owner_id,
        &actor.world_key,
        Some(27),
        1,
        burst.source,
    );
    strike.enhanced = engine.context.enhanced_effects_for(&actor.owner_id);
    strike.endpoint = burst.endpoint;
    strike.midpoint = burst.midpoint;
    strike.lifetime_ticks = 6;
    strike.phase = engine.context.tick as f64;
    strike.target_id = Some(burst.target_id);
    strike.id = engine.state.spawn_actor(strike.clone());
    if burst.remaining_ticks == 5 {
        for cue in [SecondaryAudioCue::LightningStart, SecondaryAudioCue::Thunder] {
            let mut event = super::program::actor_event(
                &strike,
                engine.context.tick,
                SecondaryEventKind::Impact,
                Some(cue),
            );
            event.position = burst.endpoint;
            engine.state.emit_event(event);
        }
    }
    burst.remaining_ticks -= 1;
    if burst.remaining_ticks == 0 {
        actor.storm_burst = None;
    }
}

struct CoronaSamples {
    source_angle: f64,
    source_scale: f64,
    contact_angle: f64,
    contact_scale: f64,
    contact_center: Vec2,
    light_radius: f64,
}

fn samples(actor: &SecondaryActorState) -> CoronaSamples {
    let mut random = AirPresentationRandom::new(actor.id, 0x53544f52);
    let source_angle = random.float(360.0) * std::f64::consts::PI / 180.0;
    let source_scale = fround(0.5 + random.float(1.0));
    let contact_angle = random.float(360.0) * std::f64::consts::PI / 180.0;
    let offset_radius = random.float(10.0);
    let offset_angle = random.float(360.0) * std::f64::consts::PI / 180.0;
    let contact_center = Vec2::new(
        fround(actor.endpoint.x + float::cos(offset_angle) * offset_radius),
        fround(actor.endpoint.y + float::sin(offset_angle) * offset_radius),
    );
    CoronaSamples {
        source_angle,
        source_scale,
        contact_angle,
        contact_center,
        contact_scale: fround(1.0 + random.float(0.5)),
        light_radius: fround(0.75 + random.float(0.75)),
    }
}

fn fade(age: u64) -> f64 {
    let mut alpha = 1.0;
    for _ in 0..age.min(6) {
        alpha = fround(alpha - fround(0.2));
    }
    alpha.max(0.0)
}

#[must_use]
pub fn contact_light(actor: &SecondaryActorState, age: u64) -> Option<AirLightPlan> {
    if age >= 6 {
        return None;
    }
    let samples = samples(actor);
    let mut intensity = 1.0;
    for _ in 0..age {
        intensity = fround(intensity - fround(0.05));
    }
    Some(AirLightPlan {
        casts_directional_shadow: false,
        intensity,
        position: samples.contact_center,
        radius: samples.light_radius,
    })
}

/// Geometry is local to the actor source; its contact light stays in world space.
#[must_use]
pub fn presentation(actor: &SecondaryActorState, age: u64) -> AirLightningPlan {
    let midpoint =
        Vec2::new(actor.midpoint.x - actor.position.x, actor.midpoint.y - actor.position.y);
    let endpoint =
        Vec2::new(actor.endpoint.x - actor.position.x, actor.endpoint.y - actor.position.y);
    let body = (age < 2).then(|| {
        let ribbon = AirRibbonInput {
            points: [Vec2::ZERO, midpoint, endpoint],
            id: actor.id,
            birth_tick: actor.phase as u64,
            width: 0.5,
            base_phase_degrees: -3.0 * actor.phase,
            phase_offset: 0.0,
            tint: 0xffffff,
            alpha: 1.0,
            dark: false,
            taper_scale: 1.0,
            enhanced_effects: actor.enhanced,
            ribbon_family: AirRibbonFamily::Air,
        };
        vec![
            build_air_ribbon_with_style(ribbon),
            build_air_ribbon_with_style(AirRibbonInput {
                width: 0.375,
                phase_offset: 15.0,
                tint: 0x00ffff,
                alpha: 0.5,
                ..ribbon
            }),
        ]
    });
    let samples = samples(actor);
    let alpha = fade(age);
    let angle_step = age as f64 * std::f64::consts::PI / 180.0;
    let source_corona = (alpha > 0.0).then(|| {
        build_air_corona_plan(AirCoronaInput {
            alpha,
            angle: samples.source_angle + angle_step,
            center: Vec2::ZERO,
            random_salt: 0x53524345 ^ age as u32,
            scale: samples.source_scale,
            seed: actor.id,
        })
    });
    let contact_corona = build_air_corona_plan(AirCoronaInput {
        alpha,
        angle: samples.contact_angle + angle_step,
        center: Vec2::new(
            samples.contact_center.x - actor.position.x,
            samples.contact_center.y - actor.position.y,
        ),
        random_salt: 0x46414445 ^ age as u32,
        scale: samples.contact_scale,
        seed: actor.id,
    });
    AirLightningPlan {
        body,
        contact_corona,
        source_corona,
        contact_light: contact_light(actor, age),
        source: Vec2::ZERO,
        midpoint,
        endpoint,
    }
}

#[cfg(feature = "persistence")]
fn remaining_ticks<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let ticks = <u8 as serde::Deserialize>::deserialize(deserializer)?;
    if (1..=4).contains(&ticks) {
        Ok(ticks)
    } else {
        Err(serde::de::Error::custom("saved Storm burst must have one to four pending births"))
    }
}
