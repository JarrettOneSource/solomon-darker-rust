//! Mod_ElectricBurn's private FadeLightning births and ordinary actor lifetime.
//! The cosmetic stream is keyed by the allocated actor ID, never gameplay RNG.

use super::program::actor;
use super::{
    NativeSecondaryActorKind as Kind, NativeSecondaryActorState as Actor,
    NativeSecondarySimulationState, NativeSecondaryTarget,
};
use crate::Vec2;
use crate::float::{cos, fround, sin};
use crate::spells::air::{
    NativeAirLightningInput, NativeAirPresentationRandom, NativeAirRibbonLayer,
    build_native_air_lightning_body,
};

pub(super) fn spawn_arc(
    state: &mut NativeSecondarySimulationState,
    owner: &str,
    world_key: &str,
    origin: Vec2,
    target: &NativeSecondaryTarget,
    enhanced: bool,
    tick: u64,
) {
    let mut child = actor(
        Kind::ElectricBurnArc,
        owner,
        world_key,
        None,
        1,
        Vec2::new(origin.x, origin.y - 10.0),
    );
    child.enhanced = enhanced;
    child.lifetime_ticks = 2;
    child.phase = tick as f64;
    child.midpoint = Vec2::new(
        fround((origin.x + target.position.x) * 0.5),
        fround((origin.y + target.position.y) * 0.5 - 10.0),
    );
    child.endpoint = Vec2::new(target.position.x, target.position.y - 30.0);
    child.target_id = Some(target.id);
    state.spawn_actor(child);
}

pub(super) fn spawn_flare(
    state: &mut NativeSecondarySimulationState,
    owner: &str,
    world_key: &str,
    target: &NativeSecondaryTarget,
    alpha: f64,
    enhanced: bool,
) {
    let mut random = NativeAirPresentationRandom::new(state.next_actor_id, 0x454c4543);
    let scale = fround(1.0 + random.float(0.5));
    let rotation = random.float(std::f64::consts::PI * 2.0);
    let radius = random.float(10.0);
    let heading = random.float(std::f64::consts::PI * 2.0);
    let light_radius = fround(0.75 + random.float(0.75));
    let decay = fround(if enhanced { 0.2 } else { 0.4 });
    let mut child = actor(
        Kind::ElectricBurnFlare,
        owner,
        world_key,
        None,
        1,
        Vec2::new(
            fround(target.position.x + cos(heading) * radius),
            fround(target.position.y - 20.0 + sin(heading) * radius),
        ),
    );
    child.alpha = alpha;
    child.enhanced = enhanced;
    child.phase = decay;
    child.quantity = alpha;
    child.lifetime_ticks = (alpha / decay).ceil() as u64;
    child.scale = scale;
    child.rotation_radians = rotation;
    child.radius = light_radius;
    state.spawn_actor(child);
}

pub(super) fn step_flare(source: &Actor, actor: &mut Actor, retain: &mut bool) {
    let alpha = fround(source.alpha - source.phase);
    actor.alpha = alpha.max(0.0);
    actor.rotation_radians = source.rotation_radians + std::f64::consts::PI / 180.0;
    *retain = alpha >= 0.0;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeElectricBurnArcBand {
    pub clip_y: f64,
    pub clip_height: f64,
    pub painter_y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeElectricBurnArcPlan {
    pub body: Vec<NativeAirRibbonLayer>,
    pub bounds_y: f64,
    pub bands: Vec<NativeElectricBurnArcBand>,
}

/// `NativeElectricBurnArcView`: born quality owns the factory geometry; current
/// quality changes only its ordinary ZAnimSplit masks. No glow belongs here.
#[must_use]
pub fn native_electric_burn_arc_plan(
    actor: &Actor,
    age_ticks: f64,
    enhanced_effects: bool,
) -> Option<NativeElectricBurnArcPlan> {
    assert_eq!(actor.kind, Kind::ElectricBurnArc, "ElectricBurn arc retains its birth kind");
    if age_ticks >= 2.0 {
        return None;
    }
    let body = build_native_air_lightning_body(NativeAirLightningInput {
        chained: false,
        enhanced_effects: actor.enhanced,
        age_ticks: age_ticks.floor().max(0.0) as u64,
        birth_tick: actor.phase as u64,
        endpoint: Vec2::new(
            actor.endpoint.x - actor.position.x,
            actor.endpoint.y - actor.position.y,
        ),
        midpoint: Vec2::new(
            actor.midpoint.x - actor.position.x,
            actor.midpoint.y - actor.position.y,
        ),
        id: actor.id,
        underpowered: false,
    })?;
    let mut ys = body.iter().flat_map(|layer| {
        layer
            .mesh
            .vertices
            .iter()
            .chain(layer.branch.iter().flat_map(|branch| &branch.mesh.vertices))
            .map(|point| point.y)
    });
    let first = ys.next().unwrap_or(0.0);
    let (minimum, maximum) =
        ys.fold((first, first), |(minimum, maximum), y| (minimum.min(y), maximum.max(y)));
    let height = if enhanced_effects { 25.0 } else { 50.0 };
    let count = ((maximum - minimum) / height).ceil() as usize;
    let bands = (0..count)
        .map(|index| {
            // NativeZAnimSplit uses ordinary Number arithmetic, unlike Flame's stores.
            let y = minimum + index as f64 * height;
            NativeElectricBurnArcBand { clip_y: y, clip_height: height, painter_y: y + height }
        })
        .collect();
    Some(NativeElectricBurnArcPlan { body, bounds_y: minimum, bands })
}
