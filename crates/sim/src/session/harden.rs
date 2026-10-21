//! Contact-born chips and coating transitions from `core-server/player-harden-effects.ts`.
//! Effects share the primary spell queue; impact sounds share the secondary event cursor.

use crate::harden_effects::{
    NativeHardenBreakupInput, NativeHardenChip, NativeHardenEffect, NativeHardenEffectOwner,
    create_native_harden_breakup,
};
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::profile::PlayerSkillRuntimeState;
use crate::secondary::{
    NativeSecondaryAudioCue, NativeSecondaryEventKind, NativeSecondaryEventState,
    NativeSecondarySimulationState,
};
use crate::spells::model::{PrimarySpellTransient, SpellState};
use crate::{NativeRng, Vec2};

/// `native-harden.ts`: this crossing plays the formed sound twice.
const NATIVE_HARDEN_FORMED_THRESHOLD: f64 = 0.25;

#[derive(Clone, Debug, PartialEq)]
pub struct PendingPlayerHardenChip {
    pub chip: NativeHardenChip,
    pub owner_id: String,
    pub position: Vec2,
    pub world_key: String,
}

pub struct PlayerHardenContext<'a> {
    pub enhanced_effects: bool,
    pub player_id: &'a str,
    pub before: Option<&'a PlayerSkillRuntimeState>,
    pub after: &'a PlayerSkillRuntimeState,
    pub position: Vec2,
    pub world_key: &'a str,
    pub spells: &'a mut SpellState,
    pub secondary: &'a mut NativeSecondarySimulationState,
    pub tick: u64,
    pub rng: &'a mut NativeRng,
    pub painter_order: &'a mut WorldPainterOrder,
}

pub fn synchronize_player_harden_effects(
    mut context: PlayerHardenContext<'_>,
    chips: &[PendingPlayerHardenChip],
) {
    // Contact chips are enrolled before the coating transition, even if that same
    // resource tick tears the coating down and creates a complete breakup.
    for pending in chips {
        let effect = NativeHardenEffect::Shard {
            owner: NativeHardenEffectOwner::new(
                context.spells.next_id,
                &pending.owner_id,
                &pending.world_key,
                context.tick,
            ),
            body: pending.chip.shard,
        };
        context.spells.next_id += 1;
        enroll(&mut context, effect);
        sound(
            &mut context,
            &pending.owner_id,
            pending.position,
            &pending.world_key,
            NativeSecondaryAudioCue::IceShatter,
            pending.chip.pitch,
            1.0,
        );
    }
    let Some(before) = context.before else {
        return;
    };
    let before = before.harden.coating;
    let after = context.after.harden.coating;
    if before == 0.0 && after > 0.0 {
        coating_sound(&mut context, NativeSecondaryAudioCue::Harden, 0.8_f32 as f64, 1.0);
    }
    if before < NATIVE_HARDEN_FORMED_THRESHOLD && after >= NATIVE_HARDEN_FORMED_THRESHOLD {
        coating_sound(&mut context, NativeSecondaryAudioCue::Harden, 1.0, 1.0);
        coating_sound(&mut context, NativeSecondaryAudioCue::Harden, 1.0, 1.0);
    }
    if before > 0.0 && after == 0.0 {
        let breakup = create_native_harden_breakup(
            NativeHardenBreakupInput {
                coating: before,
                position: context.position,
                owner_id: context.player_id,
                world_key: context.world_key,
                tick: context.tick,
                first_id: context.spells.next_id,
                enhanced_effects: context.enhanced_effects,
            },
            context.rng,
        );
        context.spells.next_id = breakup.next_id;
        for effect in breakup.effects {
            enroll(&mut context, effect);
        }
        coating_sound(&mut context, NativeSecondaryAudioCue::IceShatter, breakup.pitch, before);
    }
}

fn enroll(context: &mut PlayerHardenContext<'_>, mut effect: NativeHardenEffect) {
    // `nativePrimaryPainterRegistrationContract`: shards belong to the transient
    // post-world queue; the expanding burst occupies an ordinary actor row.
    let lane = match effect {
        NativeHardenEffect::Shard { .. } => WorldManagerLane::Transient,
        NativeHardenEffect::Burst { .. } => WorldManagerLane::Actor,
    };
    effect.common_mut().painter_registrations.push(context.painter_order.register(lane));
    context.spells.transients.push(PrimarySpellTransient::Harden(effect));
}

fn coating_sound(
    context: &mut PlayerHardenContext<'_>,
    cue: NativeSecondaryAudioCue,
    pitch: f64,
    gain: f64,
) {
    sound(context, context.player_id, context.position, context.world_key, cue, pitch, gain);
}

fn sound(
    context: &mut PlayerHardenContext<'_>,
    owner_id: &str,
    position: Vec2,
    world_key: &str,
    cue: NativeSecondaryAudioCue,
    pitch: f64,
    gain: f64,
) {
    let mut event = NativeSecondaryEventState::new(
        NativeSecondaryEventKind::Impact,
        owner_id.into(),
        world_key.into(),
        context.tick,
    );
    event.cue = Some(cue);
    event.gain = gain;
    event.pitch = pitch;
    event.position = position;
    event.skill_id = Some(36);
    context.secondary.emit_event(event);
}
