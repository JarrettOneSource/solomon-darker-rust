//! Mage pulse factories and Faculty ribbons from `native-mage-lightning-pulse-presentation.ts`
//! and `native-dark-lightning.ts`, reusing the canonical Air meshes and corona factories.

use super::boss_spell::{BossSpellKind, BossSpellRead};
use super::projectiles::{MageLightningContact, MageLightningPulse};
use crate::Vec2;
use crate::spells::air::{
    NativeAirCoronaInput, NativeAirCoronaPlan, NativeAirLightPlan, NativeAirLightningInput,
    NativeAirPresentationRandom, NativeAirRibbonInput, NativeAirRibbonLayer,
    build_native_air_corona_plan, build_native_air_lightning_plan,
    build_native_air_path_light_sources, build_native_air_ribbon_with_style,
};

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyMageLightningPlan {
    pub age_ticks: u64,
    pub body: Option<Vec<NativeAirRibbonLayer>>,
    pub contact: NativeAirCoronaPlan,
    pub contact_target_player_id: Option<String>,
    pub endpoint: Vec2,
    pub midpoint: Vec2,
    pub path_lights: Vec<NativeAirLightPlan>,
    pub source: Vec2,
    pub source_corona: Option<NativeAirCoronaPlan>,
}

#[must_use]
pub fn native_mage_lightning_pulse_plan(
    pulse: &MageLightningPulse,
    presentation_tick: u64,
) -> Option<EnemyMageLightningPlan> {
    let age = presentation_tick.saturating_sub(pulse.tick);
    let (alphas, base_scale): (&[f64], f64) = match pulse.contact {
        MageLightningContact::World { .. } => (&[1.0, 0.8, 0.6, 0.4, 0.2], 1.0),
        MageLightningContact::TargetAttached { .. } => (&[1.0, 0.6, 0.2], 0.5),
    };
    let alpha = *alphas.get(age as usize)?;
    let local = |point: Vec2| Vec2::new(point.x - pulse.source.x, point.y - pulse.source.y);
    let factory = build_native_air_lightning_plan(NativeAirLightningInput {
        chained: false,
        enhanced_effects: pulse.enhanced_effects,
        age_ticks: age,
        birth_tick: pulse.tick,
        endpoint: local(pulse.endpoint),
        id: u64::from(pulse.seed),
        midpoint: local(pulse.midpoint),
        underpowered: false,
    });
    let mut random = NativeAirPresentationRandom::new(u64::from(pulse.seed), 0x4d414745);
    let scale = base_scale + random.float(1.0) * 0.25;
    let angle = random.float(1.0) * std::f64::consts::PI * 2.0
        + age as f64 * (std::f64::consts::PI / 180.0);
    let (center, target_id) = match &pulse.contact {
        MageLightningContact::World { position } => (*position, None),
        MageLightningContact::TargetAttached { local_offset, target_player_id } => {
            (*local_offset, Some(target_player_id.clone()))
        }
    };
    let contact = build_native_air_corona_plan(NativeAirCoronaInput {
        alpha,
        angle,
        center,
        random_salt: 0x434f524f ^ age as u32,
        scale,
        seed: u64::from(pulse.seed),
    });
    let path_lights = if age == 0 {
        build_native_air_path_light_sources(
            u64::from(pulse.seed),
            pulse.tick,
            pulse.source,
            pulse.midpoint,
            pulse.endpoint,
            false,
            pulse.enhanced_effects,
        )
    } else {
        Vec::new()
    };
    Some(EnemyMageLightningPlan {
        age_ticks: age,
        body: factory.body,
        contact,
        contact_target_player_id: target_id,
        endpoint: pulse.endpoint,
        midpoint: pulse.midpoint,
        path_lights,
        source: pulse.source,
        source_corona: factory.source_corona,
    })
}

/// The two dark ribbons are independent meshes with different native taper widths.
#[must_use]
pub fn native_dark_lightning_body(spell: &impl BossSpellRead) -> Vec<NativeAirRibbonLayer> {
    let spell_view = spell.view();
    let BossSpellKind::Blightning { endpoint, midpoint, enhanced_effects } = *spell_view.kind
    else {
        panic!("Dark Lightning presentation requires Blightning");
    };
    let origin = spell_view.owner.position;
    let points = [
        Vec2::ZERO,
        Vec2::new(midpoint.x - origin.x, midpoint.y - origin.y),
        Vec2::new(endpoint.x - origin.x, endpoint.y - origin.y),
    ];
    let common = NativeAirRibbonInput {
        enhanced_effects,
        ribbon_family: crate::spells::air::NativeAirRibbonFamily::Air,
        points,
        id: spell_view.owner.id as u64,
        birth_tick: spell_view.owner.spawn_tick,
        width: 1.0,
        base_phase_degrees: -3.0 * spell_view.owner.spawn_tick as f64,
        phase_offset: 0.0,
        tint: 0xffffff,
        alpha: 1.0,
        dark: true,
        taper_scale: 1.75,
    };
    vec![
        build_native_air_ribbon_with_style(common),
        build_native_air_ribbon_with_style(NativeAirRibbonInput {
            width: 0.75,
            phase_offset: 15.0,
            tint: 0xff0000,
            alpha: 0.5,
            taper_scale: 1.25,
            ..common
        }),
    ]
}
