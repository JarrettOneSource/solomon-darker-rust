//! Current `primary-spells.ts::flameLashMainPresentation` and native view clipping.
//! Gameplay reserves the main ID here; receiver damage owns its later birth.
use super::{
    NativeFlameChannelRole, NativeFlameLashFactoryBirthInput, NativeFlameLashFactoryInput,
    NativeWeldActorBirth, NativeWeldChannelInput, NativeWeldFlameLashFadeInput,
    NativeWeldFlameLashFadeVariant, NativeWeldWorldActor, create_native_flame_lash_factory,
    create_native_weld_flame_lash_fade,
};
use crate::boneyard::BoneyardBounds;
use crate::float::{fround, hypot};
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::profile::primary::NativePrimarySkillProfile;
use crate::spells::model::{
    PrimarySpellChannelEmission, PrimarySpellFlameLashPresentation, PrimarySpellTransient,
    SpellState,
};
use crate::spells::targeting::AIR_PRIMARY_TARGET_Y_OFFSET;
use crate::{NativeRng, Vec2};

pub struct NativeFlameMainPresentationInput {
    pub origin: Vec2,
    pub aim: Vec2,
    pub endpoint: Vec2,
    pub midpoint: Vec2,
    pub target_id: Option<String>,
    pub view_bounds: Option<BoneyardBounds>,
    pub creator_registration: Option<PainterRegistration>,
    pub camera_displacement: Option<Vec2>,
}
pub fn native_flame_lash_main_presentation(
    input: NativeFlameMainPresentationInput,
) -> Result<PrimarySpellFlameLashPresentation, String> {
    let preclip_endpoint = Vec2::new(
        input.endpoint.x,
        fround(
            input.endpoint.y
                - if input.target_id.is_some() { AIR_PRIMARY_TARGET_Y_OFFSET } else { 0.0 },
        ),
    );
    let (start, end) = if let Some(bounds) = input.view_bounds {
        clip_native_flame_line(input.origin, preclip_endpoint, bounds)?
    } else {
        (input.origin, preclip_endpoint)
    };
    let length = fround(hypot(end.x - start.x, end.y - start.y));
    let half_length = fround(length * 0.5);
    Ok(PrimarySpellFlameLashPresentation {
        camera_displacement: Some(input.camera_displacement),
        midpoint: Some(input.midpoint),
        target_id: input.target_id.clone(),
        native_geometry: Some(NativeFlameLashFactoryInput {
            start,
            control: Vec2::new(
                fround(start.x + fround(input.aim.x * half_length)),
                fround(start.y + fround(input.aim.y * half_length)),
            ),
            end,
            offset: Vec2::new(0.0, if input.target_id.is_some() { -20.0 } else { 0.0 }),
        }),
        preclip_endpoint: Some(preclip_endpoint),
        creator_registration: Some(input.creator_registration.unwrap_or(PainterRegistration {
            manager_lane: WorldManagerLane::Actor,
            registration_ordinal: 0,
        })),
    })
}

/// Inclusive boundaries. A shared outcode still births a collapsed native ribbon.
fn clip_native_flame_line(
    start: Vec2,
    end: Vec2,
    bounds: BoneyardBounds,
) -> Result<(Vec2, Vec2), String> {
    let (mut first, mut last) = (start, end);
    let right = fround(bounds.x + bounds.width);
    let bottom = fround(bounds.y + bounds.height);
    let outcode = |p: Vec2| {
        (if p.x < bounds.x {
            1
        } else if p.x > right {
            2
        } else {
            0
        }) | (if p.y < bounds.y {
            4
        } else if p.y > bottom {
            8
        } else {
            0
        })
    };
    for _ in 0..8 {
        let a = outcode(first);
        let b = outcode(last);
        if a | b == 0 {
            return Ok((first, last));
        }
        if a & b != 0 {
            return Ok((first, first));
        }
        let code = if a != 0 { a } else { b };
        let point = if code & 4 != 0 {
            let y = bounds.y;
            Vec2::new(first.x + (last.x - first.x) * (y - first.y) / (last.y - first.y), y)
        } else if code & 8 != 0 {
            let y = bottom;
            Vec2::new(first.x + (last.x - first.x) * (y - first.y) / (last.y - first.y), y)
        } else if code & 2 != 0 {
            let x = right;
            Vec2::new(x, first.y + (last.y - first.y) * (x - first.x) / (last.x - first.x))
        } else {
            let x = bounds.x;
            Vec2::new(x, first.y + (last.y - first.y) * (x - first.x) / (last.x - first.x))
        };
        let point = Vec2::new(fround(point.x), fround(point.y));
        if a != 0 { first = point } else { last = point }
    }
    Err("Flame line clipping did not converge".into())
}

impl SpellState {
    pub fn finalize_hub_flame_lash_visuals(
        &mut self,
        emissions: &[PrimarySpellChannelEmission],
        tick: u64,
        rng: &mut NativeRng,
        enhanced_effects: bool,
        painters: &mut WorldPainterOrder,
    ) -> Result<(), String> {
        for emission in emissions {
            let NativePrimarySkillProfile::Weld(profile) = &emission.primary_skill else {
                continue;
            };
            if profile.build_id != 1003 {
                continue;
            }
            let Some(main) = &emission.flame_lash_presentation else { continue };
            let (Some(geometry), Some(creator_registration), Some(camera_displacement)) =
                (main.native_geometry, main.creator_registration, main.camera_displacement)
            else {
                continue;
            };
            let birth = NativeWeldActorBirth {
                build_id: 1003,
                direction: emission.direction,
                id: emission.id,
                origin: emission.origin,
                owner_id: &emission.owner_id,
                tick,
                vector: &profile.vector.values,
                world_key: &emission.world_key,
            };
            if let Some(endpoint) =
                main.preclip_endpoint.or(emission.endpoint).filter(|point| point.x > -9999.0)
            {
                let fade = create_native_weld_flame_lash_fade(
                    NativeWeldFlameLashFadeInput {
                        birth: NativeWeldActorBirth { id: self.next_id, origin: endpoint, ..birth },
                        alpha: if emission.underpowered { 0.5 } else { 1.0 },
                        variant: NativeWeldFlameLashFadeVariant::Endpoint,
                        enhanced_effects,
                    },
                    rng,
                    Some(painters),
                );
                self.transients
                    .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::FlameLashFade(fade)));
                self.next_id += 1;
            }
            let factory = create_native_flame_lash_factory(
                NativeFlameLashFactoryBirthInput {
                    channel: NativeWeldChannelInput {
                        birth,
                        endpoint: emission.endpoint,
                        midpoint: main.midpoint,
                        target_id: main.target_id.clone(),
                        underpowered: emission.underpowered,
                        enhanced_effects,
                        flame_visual: None,
                    },
                    source_fade_id: self.next_id,
                    geometry,
                    role: NativeFlameChannelRole::Main,
                    camera_displacement,
                    width: if emission.underpowered { 0.75 } else { 1.0 },
                    creator_registration,
                    misc_append_ordinal: 0,
                },
                rng,
                painters,
            )?;
            self.next_id += 1;
            self.transients.push(PrimarySpellTransient::Weld(NativeWeldWorldActor::FlameLashFade(
                factory.source,
            )));
            self.transients
                .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::Channel(factory.channel)));
        }
        Ok(())
    }
}
