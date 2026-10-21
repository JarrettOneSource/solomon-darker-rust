//! Current Source `native-weld-flame-birth.ts`: the factory owns constructor
//! words and registrations. Path lights reuse these samples without RNG draws.

use super::effects::{
    NativeWeldChannelActorState, NativeWeldChannelInput, NativeWeldFlameLashFadeInput,
    NativeWeldFlameLashFadeState, NativeWeldFlameLashFadeVariant, create_native_weld_channel_actor,
    create_native_weld_flame_lash_fade,
};
use super::flame_geometry::{
    NATIVE_FLAME_MAXIMUM_PATH_POINTS, NativeFlameLashFactoryInput, NativeFlameLashGeometryKind,
    native_flame_lash_birth_phase, native_flame_lash_factory_geometry,
    native_flame_lash_path_points,
};
use super::flame_state::{
    NativeFlameChannelBirth, NativeFlameChannelRole, NativeFlameChannelVisual,
    NativeFlameFadeVisual, NativeFlamePathBirth, NativeFlameRibbonBirth,
    native_flame_fade_wrapper_intensity,
};
use super::model::NativeWeldActorBirth;
use crate::boneyard::lighting::NativeBoneyardLightSource;
use crate::float::fround;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::{NativeRng, Vec2};

pub struct NativeFlameLashFactoryBirthInput<'a> {
    pub channel: NativeWeldChannelInput<'a>,
    pub source_fade_id: u64,
    pub geometry: NativeFlameLashFactoryInput,
    pub role: NativeFlameChannelRole,
    pub camera_displacement: Option<Vec2>,
    pub width: f64,
    pub creator_registration: PainterRegistration,
    pub misc_append_ordinal: usize,
}

pub struct NativeFlameLashFactoryBirth {
    pub channel: NativeWeldChannelActorState,
    pub source: NativeWeldFlameLashFadeState,
}

/// Source flare first, short-only width, path intensity, then each ordered radius.
pub fn create_native_flame_lash_factory(
    input: NativeFlameLashFactoryBirthInput<'_>,
    rng: &mut NativeRng,
    painters: &mut WorldPainterOrder,
) -> Result<NativeFlameLashFactoryBirth, String> {
    let needs_camera = input.role == NativeFlameChannelRole::Main && !input.channel.underpowered;
    if needs_camera != input.camera_displacement.is_some() {
        return Err("Flame camera birth sample does not match its caller".into());
    }
    let source = create_native_weld_flame_lash_fade(
        NativeWeldFlameLashFadeInput {
            birth: NativeWeldActorBirth {
                id: input.source_fade_id,
                origin: input.geometry.start,
                ..input.channel.birth
            },
            alpha: 0.5,
            variant: NativeWeldFlameLashFadeVariant::Source,
            enhanced_effects: input.channel.enhanced_effects,
        },
        rng,
        Some(painters),
    );
    let geometry = native_flame_lash_factory_geometry(input.geometry)?;
    let ribbon = match geometry.kind {
        NativeFlameLashGeometryKind::Short => NativeFlameRibbonBirth::Short {
            width_factor: fround(0.75 + rng.draw_scaled_float(0.25, false)),
        },
        NativeFlameLashGeometryKind::Long => NativeFlameRibbonBirth::Long {
            phase_degrees: native_flame_lash_birth_phase(input.channel.birth.tick),
        },
    };
    let mut intensity = fround(0.25 + rng.draw_scaled_float(0.75, false));
    if input.channel.underpowered {
        intensity = fround(intensity * 0.25);
    }
    let points = native_flame_lash_path_points(&geometry, NATIVE_FLAME_MAXIMUM_PATH_POINTS)?;
    let mut radius_factors = Vec::with_capacity(points.len());
    for _ in points {
        radius_factors.push(fround(0.75 + rng.draw_scaled_float(0.25, false)));
    }
    let birth = NativeFlameChannelBirth {
        role: input.role,
        camera_displacement: input.camera_displacement,
        start: geometry.start,
        control: geometry.control,
        end: Vec2::new(fround(input.geometry.end.x), fround(input.geometry.end.y)),
        offset: Vec2::new(fround(input.geometry.offset.x), fround(input.geometry.offset.y)),
        width: input.width,
        ribbon,
        path: NativeFlamePathBirth {
            intensity,
            radius_factors,
            creator_registration: input.creator_registration,
            misc_append_ordinal: input.misc_append_ordinal,
        },
    };
    let mut channel = create_native_weld_channel_actor(NativeWeldChannelInput {
        flame_visual: Some(NativeFlameChannelVisual::NativeBirth(birth)),
        ..input.channel
    });
    channel.common.owner.painter_registrations.push(painters.register(WorldManagerLane::Actor));
    Ok(NativeFlameLashFactoryBirth { channel, source })
}

pub fn native_flame_lash_path_lights(
    birth: &NativeFlameChannelBirth,
    enhanced_effects: bool,
) -> Result<Vec<NativeBoneyardLightSource>, String> {
    let geometry = native_flame_lash_factory_geometry(NativeFlameLashFactoryInput {
        start: birth.start,
        control: birth.control,
        end: birth.end,
        offset: birth.offset,
    })?;
    let points = native_flame_lash_path_points(&geometry, NATIVE_FLAME_MAXIMUM_PATH_POINTS)?;
    if points.len() != birth.path.radius_factors.len() {
        return Err("Flame path lost a birth radius".into());
    }
    let mut lights = Vec::with_capacity(points.len());
    for (index, position) in points.into_iter().enumerate() {
        lights.push(NativeBoneyardLightSource::new(
            position,
            birth.path.radius_factors[index],
            birth.path.intensity,
            enhanced_effects,
        ));
    }
    Ok(lights)
}

#[must_use]
pub fn native_flame_lash_fade_light(
    actor: &NativeWeldFlameLashFadeState,
) -> Option<NativeBoneyardLightSource> {
    let NativeFlameFadeVisual::NativeBirth(birth) = actor.flame_visual else { return None };
    if actor.variant == NativeWeldFlameLashFadeVariant::Source || actor.alpha <= 0.0 {
        return None;
    }
    Some(NativeBoneyardLightSource::new(
        actor.position,
        fround(birth.birth_alpha * actor.wrapper_scalar),
        native_flame_fade_wrapper_intensity(birth.birth_alpha, actor.common.owner.age_ticks),
        false,
    ))
}
