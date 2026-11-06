//! Current Source `native-weld-flame-birth.ts`: the factory owns constructor
//! words and registrations. Path lights reuse these samples without RNG draws.

use super::effects::{
    WeldChannelActorState, WeldChannelInput, WeldFlameLashFadeInput, WeldFlameLashFadeState,
    WeldFlameLashFadeVariant, create_weld_channel_actor, create_weld_flame_lash_fade,
};
use super::flame_geometry::{
    FLAME_MAXIMUM_PATH_POINTS, FlameLashFactoryInput, FlameLashGeometryKind,
    flame_lash_birth_phase, flame_lash_factory_geometry, flame_lash_path_points,
};
use super::flame_state::{
    FlameChannelBirth, FlameChannelRole, FlameChannelVisual, FlameFadeVisual, FlamePathBirth,
    FlameRibbonBirth, flame_fade_wrapper_intensity,
};
use super::model::WeldActorBirth;
use crate::boneyard::lighting::BoneyardLightSource;
use crate::float::fround;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::{Rng, Vec2};

pub struct FlameLashFactoryBirthInput<'a> {
    pub channel: WeldChannelInput<'a>,
    pub source_fade_id: u64,
    pub geometry: FlameLashFactoryInput,
    pub role: FlameChannelRole,
    pub camera_displacement: Option<Vec2>,
    pub width: f64,
    pub creator_registration: PainterRegistration,
    pub misc_append_ordinal: usize,
}

pub struct FlameLashFactoryBirth {
    pub channel: WeldChannelActorState,
    pub source: WeldFlameLashFadeState,
}

/// Source flare first, short-only width, path intensity, then each ordered radius.
pub fn create_flame_lash_factory(
    input: FlameLashFactoryBirthInput<'_>,
    rng: &mut Rng,
    painters: &mut WorldPainterOrder,
) -> Result<FlameLashFactoryBirth, String> {
    let needs_camera = input.role == FlameChannelRole::Main && !input.channel.underpowered;
    if needs_camera != input.camera_displacement.is_some() {
        return Err("Flame camera birth sample does not match its caller".into());
    }
    let source = create_weld_flame_lash_fade(
        WeldFlameLashFadeInput {
            birth: WeldActorBirth {
                id: input.source_fade_id,
                origin: input.geometry.start,
                ..input.channel.birth
            },
            alpha: 0.5,
            variant: WeldFlameLashFadeVariant::Source,
            enhanced_effects: input.channel.enhanced_effects,
        },
        rng,
        Some(painters),
    );
    let geometry = flame_lash_factory_geometry(input.geometry)?;
    let ribbon = match geometry.kind {
        FlameLashGeometryKind::Short => FlameRibbonBirth::Short {
            width_factor: fround(0.75 + rng.draw_scaled_float(0.25, false)),
        },
        FlameLashGeometryKind::Long => FlameRibbonBirth::Long {
            phase_degrees: flame_lash_birth_phase(input.channel.birth.tick),
        },
    };
    let mut intensity = fround(0.25 + rng.draw_scaled_float(0.75, false));
    if input.channel.underpowered {
        intensity = fround(intensity * 0.25);
    }
    let points = flame_lash_path_points(&geometry, FLAME_MAXIMUM_PATH_POINTS)?;
    let mut radius_factors = Vec::with_capacity(points.len());
    for _ in points {
        radius_factors.push(fround(0.75 + rng.draw_scaled_float(0.25, false)));
    }
    let birth = FlameChannelBirth {
        role: input.role,
        camera_displacement: input.camera_displacement,
        start: geometry.start,
        control: geometry.control,
        end: Vec2::new(fround(input.geometry.end.x), fround(input.geometry.end.y)),
        offset: Vec2::new(fround(input.geometry.offset.x), fround(input.geometry.offset.y)),
        width: input.width,
        ribbon,
        path: FlamePathBirth {
            intensity,
            radius_factors,
            creator_registration: input.creator_registration,
            misc_append_ordinal: input.misc_append_ordinal,
        },
    };
    let mut channel = create_weld_channel_actor(WeldChannelInput {
        flame_visual: Some(FlameChannelVisual::Birth(birth)),
        ..input.channel
    });
    channel.common.owner.painter_registrations.push(painters.register(WorldManagerLane::Actor));
    Ok(FlameLashFactoryBirth { channel, source })
}

pub fn flame_lash_path_lights(
    birth: &FlameChannelBirth,
    enhanced_effects: bool,
) -> Result<Vec<BoneyardLightSource>, String> {
    let geometry = flame_lash_factory_geometry(FlameLashFactoryInput {
        start: birth.start,
        control: birth.control,
        end: birth.end,
        offset: birth.offset,
    })?;
    let points = flame_lash_path_points(&geometry, FLAME_MAXIMUM_PATH_POINTS)?;
    if points.len() != birth.path.radius_factors.len() {
        return Err("Flame path lost a birth radius".into());
    }
    let mut lights = Vec::with_capacity(points.len());
    for (index, position) in points.into_iter().enumerate() {
        lights.push(BoneyardLightSource::new(
            position,
            birth.path.radius_factors[index],
            birth.path.intensity,
            enhanced_effects,
        ));
    }
    Ok(lights)
}

#[must_use]
pub fn flame_lash_fade_light(actor: &WeldFlameLashFadeState) -> Option<BoneyardLightSource> {
    let FlameFadeVisual::Birth(birth) = actor.flame_visual else { return None };
    if actor.variant == WeldFlameLashFadeVariant::Source || actor.alpha <= 0.0 {
        return None;
    }
    Some(BoneyardLightSource::new(
        actor.position,
        fround(birth.birth_alpha * actor.wrapper_scalar),
        flame_fade_wrapper_intensity(birth.birth_alpha, actor.common.owner.age_ticks),
        false,
    ))
}
