//! Current Source `protocol/codecs/{flame-presentation,weld-flame}.ts`.
//! Typed wire shape is checked by serde; these checks bind a complete recipe to
//! the accepted snapshot before geometry or retained replay can consume it.

use std::collections::BTreeSet;

use solomon_sim::Vec2;
use solomon_sim::float::{fround, hypot};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::spells::weld::{
    NATIVE_FLAME_MAXIMUM_PATH_POINTS, NATIVE_FLAME_MAXIMUM_RIBBON_PAIRS, NativeFlameChannelRole,
    NativeFlameChannelVisual, NativeFlameFadeVisual, NativeFlameLashFactoryInput,
    NativeFlameLashGeometryKind, NativeFlameRibbonBirth, NativeWeldFlameLashFadeVariant,
    native_camera_magnitude_squared, native_flame_fade_alpha, native_flame_lash_birth_phase,
    native_flame_lash_factory_geometry, native_flame_lash_path_points,
    native_flame_lash_ribbon_pair_count,
};

use super::{
    FlamePresentationFrame, FlamePresentationRecipe, MAX_FLAME_PRESENTATION_BIRTHS,
    MAX_SAFE_INTEGER,
};

impl FlamePresentationFrame {
    /// Validate against the same stopped clock and allocation cursor as the
    /// enclosing snapshot. A valid standalone history frame is not sufficient.
    pub fn validate_snapshot(
        &self,
        world_key: &str,
        world_tick: u64,
        next_primary_id: u64,
    ) -> Result<(), String> {
        let field = "snapshot.flamePresentation";
        safe_tick(self.epoch(), &format!("{field}.epoch"), true)?;
        safe_tick(self.through_tick(), &format!("{field}.throughTick"), false)?;
        if self.world_key() != world_key || self.through_tick() != world_tick {
            return Err(format!("{field} does not match the stopped snapshot world"));
        }
        let Self::Continuous { after_tick, births, through_tick, .. } = self else {
            return Ok(());
        };
        if births.len() > MAX_FLAME_PRESENTATION_BIRTHS {
            return Err(format!(
                "{field}.births may contain at most {MAX_FLAME_PRESENTATION_BIRTHS} entries"
            ));
        }
        safe_tick(*after_tick, &format!("{field}.afterTick"), false)?;
        if after_tick > through_tick {
            return Err(format!("{field} has a reversed coverage interval"));
        }
        let mut identities = BTreeSet::new();
        for (index, birth) in births.iter().enumerate() {
            let name = format!("{field}.births[{index}]");
            safe_tick(birth.id, &format!("{name}.id"), true)?;
            if birth.id >= next_primary_id {
                return Err(format!("{name}.id is not allocated"));
            }
            player_id(&birth.owner_id, &format!("{name}.ownerId"))?;
            limited_string(&birth.world_key, &format!("{name}.worldKey"), 256)?;
            if birth.world_key != world_key
                && !(world_key == "hub" && birth.world_key.starts_with("hub:"))
            {
                return Err(format!("{name} belongs to another world"));
            }
            safe_tick(birth.birth_tick, &format!("{name}.birthTick"), false)?;
            safe_tick(birth.end_tick_exclusive, &format!("{name}.endTickExclusive"), false)?;
            let lifetime = recipe(&birth.recipe, birth.birth_tick, &format!("{name}.recipe"))?;
            if birth.kind != birth.recipe.kind() {
                return Err(format!("{name}.kind does not match its recipe"));
            }
            if birth.birth_tick > *through_tick
                || birth.end_tick_exclusive < birth.birth_tick
                || birth.end_tick_exclusive > birth.birth_tick + lifetime
                || (birth.birth_tick <= *after_tick && birth.end_tick_exclusive <= *after_tick)
            {
                return Err(format!("{name} exceeds its native presentation interval"));
            }
            if !identities.insert(birth.id) {
                return Err(format!("{field} repeats a birth identity"));
            }
        }
        Ok(())
    }
}

fn recipe(value: &FlamePresentationRecipe, birth_tick: u64, field: &str) -> Result<u64, String> {
    match value {
        FlamePresentationRecipe::Ribbon { birth, enhanced_effects, weak, painter_registration } => {
            channel(birth, *enhanced_effects, *weak, birth_tick, &format!("{field}.birth"))?;
            registration(
                *painter_registration,
                WorldManagerLane::Actor,
                &format!("{field}.painterRegistration"),
            )?;
            Ok(1)
        }
        FlamePresentationRecipe::Fade {
            birth,
            position,
            alpha_step,
            base_scale,
            wrapper_scalar,
            variant,
            painter_registration,
        } => {
            let NativeFlameFadeVisual::NativeBirth(birth) = birth else {
                return Err(format!("{field} cannot replay an incomplete fade"));
            };
            if birth.birth_alpha != 0.5 && birth.birth_alpha != 1.0 {
                return Err(format!("{field} cannot replay an incomplete fade"));
            }
            registration(
                *painter_registration,
                WorldManagerLane::Transient,
                &format!("{field}.painterRegistration"),
            )?;
            let source = *variant == NativeWeldFlameLashFadeVariant::Source;
            positive(*alpha_step, &format!("{field}.alphaStep"))?;
            if *alpha_step != 0.4_f32 as f64
                && *alpha_step != fround(if source { 0.1 } else { 0.2 })
            {
                return Err(format!("{field}.alphaStep is not a native Flame fade branch"));
            }
            if (source && birth.birth_alpha != 0.5)
                || (*variant == NativeWeldFlameLashFadeVariant::Chain && birth.birth_alpha != 1.0)
            {
                return Err(format!("{field} violates its native Flame birth-update clock"));
            }
            positive(*base_scale, &format!("{field}.baseScale"))?;
            let (minimum, maximum) = if source { (0.75, 1.25) } else { (0.5, 1.0) };
            if *base_scale < minimum || *base_scale > maximum {
                return Err(format!("{field}.baseScale exceeds its Flame Lash branch"));
            }
            positive(*wrapper_scalar, &format!("{field}.wrapperScalar"))?;
            if *wrapper_scalar < 0.75 || *wrapper_scalar > 1.5 || (source && *wrapper_scalar != 1.0)
            {
                return Err(format!("{field}.wrapperScalar exceeds its native Flame branch"));
            }
            if *base_scale != fround(*base_scale) || *wrapper_scalar != fround(*wrapper_scalar) {
                return Err(format!("{field} requires native float Flame birth samples"));
            }
            point(*position, &format!("{field}.position"))?;
            let mut lifetime = 0;
            while native_flame_fade_alpha(birth.birth_alpha, *alpha_step, lifetime) > 0.0 {
                lifetime += 1;
            }
            Ok(lifetime)
        }
    }
}

fn channel(
    visual: &NativeFlameChannelVisual,
    enhanced_effects: bool,
    weak: bool,
    birth_tick: u64,
    field: &str,
) -> Result<(), String> {
    let NativeFlameChannelVisual::NativeBirth(birth) = visual else {
        return Err(format!(
            "{} cannot replay a suppressed legacy ribbon",
            field.trim_end_matches(".birth")
        ));
    };
    let main = birth.role == NativeFlameChannelRole::Main;
    if main && !weak {
        let camera = birth
            .camera_displacement
            .ok_or_else(|| format!("{field}.cameraDisplacement must be an object"))?;
        native_float(camera.x, &format!("{field}.cameraDisplacement.x"), -4.0, 4.0)?;
        native_float(camera.y, &format!("{field}.cameraDisplacement.y"), -4.0, 4.0)?;
        if native_camera_magnitude_squared(camera) > 16.000004_f32 as f64 {
            return Err(format!("{field}.cameraDisplacement exceeds the native magnitude"));
        }
    } else if birth.camera_displacement.is_some() {
        return Err(format!("{field}.cameraDisplacement is forbidden for weak/chain births"));
    }
    native_float(birth.width, &format!("{field}.width"), 0.5, 1.0)?;
    if (main && birth.width != if weak { 0.75 } else { 1.0 })
        || (!main && (weak || birth.width > 0.8_f32 as f64))
    {
        return Err(format!("{field}.width does not match its Flame role"));
    }
    point(birth.start, &format!("{field}.start"))?;
    point(birth.control, &format!("{field}.control"))?;
    point(birth.end, &format!("{field}.end"))?;
    point(birth.offset, &format!("{field}.offset"))?;
    if birth.offset.x != 0.0
        || (birth.offset.y != 0.0 && birth.offset.y != -20.0)
        || (!main && birth.offset.y != -20.0)
    {
        return Err(format!("{field}.offset does not match its native caller"));
    }
    for point in [birth.start, birth.control, birth.end] {
        if point.x != fround(point.x) || point.y != fround(point.y) {
            return Err(format!("{field} requires stored native Flame coordinates"));
        }
    }
    let first_leg = hypot(birth.control.x - birth.start.x, birth.control.y - birth.start.y);
    let second_leg = hypot(birth.end.x - birth.control.x, birth.end.y - birth.control.y);
    let spacing = if enhanced_effects { 7.0 } else { 30.0 };
    if !first_leg.is_finite()
        || !second_leg.is_finite()
        || first_leg > NATIVE_FLAME_MAXIMUM_RIBBON_PAIRS as f64 * spacing * 2.0
        || first_leg > (NATIVE_FLAME_MAXIMUM_PATH_POINTS + 1) as f64 * 100.0
        || second_leg + 15.0 > (NATIVE_FLAME_MAXIMUM_PATH_POINTS + 1) as f64 * 100.0
    {
        return Err(format!("{field} exceeds the Flame geometry work budget"));
    }
    let geometry = native_flame_lash_factory_geometry(NativeFlameLashFactoryInput {
        start: birth.start,
        control: birth.control,
        end: birth.end,
        offset: birth.offset,
    })
    .map_err(|_| format!("{field} has invalid native Flame geometry"))?;
    let bounds = geometry.split_bounds;
    if [
        geometry.span,
        geometry.extended_end.x,
        geometry.extended_end.y,
        geometry.body_end.x,
        geometry.body_end.y,
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(format!("{field} has non-finite native Flame geometry"));
    }
    match (geometry.kind, birth.ribbon) {
        (NativeFlameLashGeometryKind::Short, NativeFlameRibbonBirth::Short { width_factor }) => {
            native_float(width_factor, &format!("{field}.ribbon.widthFactor"), 0.75, 1.0)?;
        }
        (NativeFlameLashGeometryKind::Long, NativeFlameRibbonBirth::Long { phase_degrees }) => {
            finite(phase_degrees, &format!("{field}.ribbon.phaseDegrees"))?;
            if phase_degrees != native_flame_lash_birth_phase(birth_tick) {
                return Err(format!("{field}.ribbon.phaseDegrees does not match its birth clock"));
            }
        }
        _ => return Err(format!("{field}.ribbon.kind does not match its native span")),
    }
    let path = &birth.path;
    native_float(
        path.intensity,
        &format!("{field}.path.intensity"),
        if weak { 0.0625 } else { 0.25 },
        if weak { 0.25 } else { 1.0 },
    )?;
    if path.radius_factors.len() > NATIVE_FLAME_MAXIMUM_PATH_POINTS {
        return Err(format!(
            "{field}.path.radiusFactors may contain at most {NATIVE_FLAME_MAXIMUM_PATH_POINTS} entries"
        ));
    }
    for (index, factor) in path.radius_factors.iter().enumerate() {
        native_float(*factor, &format!("{field}.path.radiusFactors[{index}]"), 0.75, 1.0)?;
    }
    registration(
        path.creator_registration,
        WorldManagerLane::Actor,
        &format!("{field}.path.creatorRegistration"),
    )?;
    if path.misc_append_ordinal as u64 > MAX_SAFE_INTEGER
        || (main && path.misc_append_ordinal != 0)
        || (!main && path.misc_append_ordinal == 0)
    {
        return Err(format!("{field}.path.miscAppendOrdinal does not match its factory role"));
    }
    native_flame_lash_ribbon_pair_count(
        &geometry,
        enhanced_effects,
        NATIVE_FLAME_MAXIMUM_RIBBON_PAIRS,
    )
    .map_err(|_| format!("{field} exceeds the Flame geometry work budget"))?;
    let points = native_flame_lash_path_points(&geometry, NATIVE_FLAME_MAXIMUM_PATH_POINTS)
        .map_err(|_| format!("{field} exceeds the Flame geometry work budget"))?;
    if points.len() != path.radius_factors.len() {
        return Err(format!(
            "{field}.path.radiusFactors must match the admitted native path points"
        ));
    }
    Ok(())
}

fn safe_tick(value: u64, field: &str, positive: bool) -> Result<(), String> {
    if positive && value == 0 {
        return Err(format!("{field} must be positive"));
    }
    if value > MAX_SAFE_INTEGER {
        return Err(format!("{field} must be a safe integer"));
    }
    Ok(())
}
fn finite(value: f64, field: &str) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{field} must be finite"));
    }
    Ok(())
}
fn positive(value: f64, field: &str) -> Result<(), String> {
    finite(value, field)?;
    if value <= 0.0 {
        return Err(format!("{field} must be positive"));
    }
    Ok(())
}
fn point(value: Vec2, field: &str) -> Result<(), String> {
    finite(value.x, &format!("{field}.x"))?;
    finite(value.y, &format!("{field}.y"))
}
fn native_float(value: f64, field: &str, minimum: f64, maximum: f64) -> Result<(), String> {
    finite(value, field)?;
    if value < minimum || value > maximum {
        return Err(format!("{field} must be within [{minimum},{maximum}]"));
    }
    if value != fround(value) {
        return Err(format!("{field} must be a native float"));
    }
    Ok(())
}
fn registration(
    value: PainterRegistration,
    lane: WorldManagerLane,
    field: &str,
) -> Result<(), String> {
    if value.manager_lane != lane {
        let lane = if lane == WorldManagerLane::Actor { "actor" } else { "transient" };
        return Err(format!("{field}.managerLane must be {lane}"));
    }
    safe_tick(value.registration_ordinal as u64, &format!("{field}.registrationOrdinal"), false)
}
fn limited_string(value: &str, field: &str, maximum: usize) -> Result<(), String> {
    if value.is_empty() || value.encode_utf16().count() > maximum {
        return Err(format!("{field} must be a nonempty string of at most {maximum} characters"));
    }
    Ok(())
}
fn player_id(value: &str, field: &str) -> Result<(), String> {
    limited_string(value, field, 128)?;
    // Own names of the standard ECMAScript Object.prototype, rejected by Source.
    if matches!(
        value,
        "constructor"
            | "__defineGetter__"
            | "__defineSetter__"
            | "hasOwnProperty"
            | "__lookupGetter__"
            | "__lookupSetter__"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "toString"
            | "valueOf"
            | "__proto__"
            | "toLocaleString"
    ) {
        return Err(format!("{field} is reserved"));
    }
    Ok(())
}
