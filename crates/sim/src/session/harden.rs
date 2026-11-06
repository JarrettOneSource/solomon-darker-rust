//! A player's Harden effects: contact-born chips, and the sounds and breakup of coating
//! transitions.
//! Effects share the primary spell queue; impact sounds share the secondary event cursor.

use crate::harden_effects::{
    HardenBreakupInput, HardenChip, HardenEffect, HardenEffectOwner, create_harden_breakup,
};
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::profile::PlayerSkillRuntimeState;
use crate::secondary::{
    SecondaryAudioCue, SecondaryEventKind, SecondaryEventState, SecondarySimulationState,
};
use crate::spells::model::{PrimarySpellTransient, SpellState};
use crate::{Rng, Vec2};

/// Crossing this coating plays the formed sound twice.
const HARDEN_FORMED_THRESHOLD: f64 = 0.25;

/// A Harden chip knocked off by a contact, waiting to be enrolled as a shard.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingPlayerHardenChip {
    pub chip: HardenChip,
    pub owner_id: String,
    pub position: Vec2,
    pub world_key: String,
}

/// What synchronizing one player's Harden effects reads and writes: coating before and
/// after, position, spells, secondary events, RNG and painter order.
pub struct PlayerHardenContext<'a> {
    pub enhanced_effects: bool,
    pub player_id: &'a str,
    pub before: Option<&'a PlayerSkillRuntimeState>,
    pub after: &'a PlayerSkillRuntimeState,
    pub position: Vec2,
    pub world_key: &'a str,
    pub spells: &'a mut SpellState,
    pub secondary: &'a mut SecondarySimulationState,
    pub tick: u64,
    pub rng: &'a mut Rng,
    pub painter_order: &'a mut WorldPainterOrder,
}

pub fn synchronize_player_harden_effects(
    mut context: PlayerHardenContext<'_>,
    chips: &[PendingPlayerHardenChip],
) {
    // Contact chips are enrolled before the coating transition, even if that same
    // resource tick tears the coating down and creates a complete breakup.
    for pending in chips {
        let effect = HardenEffect::Shard {
            owner: HardenEffectOwner::new(
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
            SecondaryAudioCue::IceShatter,
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
        coating_sound(&mut context, SecondaryAudioCue::Harden, 0.8_f32 as f64, 1.0);
    }
    if before < HARDEN_FORMED_THRESHOLD && after >= HARDEN_FORMED_THRESHOLD {
        coating_sound(&mut context, SecondaryAudioCue::Harden, 1.0, 1.0);
        coating_sound(&mut context, SecondaryAudioCue::Harden, 1.0, 1.0);
    }
    if before > 0.0 && after == 0.0 {
        let breakup = create_harden_breakup(
            HardenBreakupInput {
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
        coating_sound(&mut context, SecondaryAudioCue::IceShatter, breakup.pitch, before);
    }
}

fn enroll(context: &mut PlayerHardenContext<'_>, mut effect: HardenEffect) {
    // Shards belong to the transient post-world queue; the expanding burst occupies an
    // ordinary actor row.
    let lane = match effect {
        HardenEffect::Shard { .. } => WorldManagerLane::Transient,
        HardenEffect::Burst { .. } => WorldManagerLane::Actor,
    };
    effect.common_mut().painter_registrations.push(context.painter_order.register(lane));
    context.spells.transients.push(PrimarySpellTransient::Harden(effect));
}

fn coating_sound(
    context: &mut PlayerHardenContext<'_>,
    cue: SecondaryAudioCue,
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
    cue: SecondaryAudioCue,
    pitch: f64,
    gain: f64,
) {
    let mut event = SecondaryEventState::new(
        SecondaryEventKind::Impact,
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
