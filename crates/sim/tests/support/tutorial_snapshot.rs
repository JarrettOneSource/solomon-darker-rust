//! The complete retained Tutorial state used by kernel and real-world replays.

use serde_json::{Value, json};
use solomon_sim::boneyard::tutorial::{NativeTutorialNarrationState, NativeTutorialState};

pub fn narration_row(narration: &NativeTutorialNarrationState) -> Value {
    json!({ "current": narration.current.as_ref().map(|event|json!({ "cue": event.cue.id(),
        "eventId": event.event_id, "speaker": event.speaker.id(), "text": event.text })),
        "nextEventId": narration.next_event_id,
        "pending": narration.pending.iter().map(|cue|cue.id()).collect::<Vec<_>>(),
        "ticksRemaining": narration.ticks_remaining })
}

pub fn state_row(state: &NativeTutorialState) -> Value {
    json!({
        "active": state.active, "cameraLockAgeTicks": state.camera_lock_age_ticks,
        "cameraLockTriggered": state.camera_lock_triggered, "cameraLockTicksRemaining": state.camera_lock_ticks_remaining,
        "damageProtection": state.damage_protection, "dialogueArmed": state.dialogue_armed,
        "introActive": state.intro_active, "introBlend": state.intro_blend,
        "introDelayTicksRemaining": state.intro_delay_ticks_remaining, "introFade": state.intro_fade,
        "introMovementTicksRemaining": state.intro_movement_ticks_remaining,
        "inventoryOpened": state.inventory_opened, "inventorySeen": state.inventory_seen,
        "itemDropArmed": state.item_drop_armed, "movementAnchor": json!({ "x": state.movement_anchor.x, "y": state.movement_anchor.y }),
        "movementInstructionAcknowledged": state.movement_instruction_acknowledged,
        "narration": narration_row(&state.narration), "nextSpawnIntentId": state.next_spawn_intent_id,
        "primaryCastSequenceAtStart": state.primary_cast_sequence_at_start,
        "rngState": { "indexA": state.rng.index_a, "indexB": state.rng.index_b, "words": state.rng.words.as_slice() },
        "selectedSkillHudAcknowledged": state.selected_skill_hud_acknowledged,
        "skillsOpened": state.skills_opened, "skillsSeen": state.skills_seen,
        "solomonDialogueQueued": state.solomon_dialogue_queued, "solomonRetreatQueued": state.solomon_retreat_queued,
        "stage": state.stage.id(), "stageTicks": state.stage_ticks,
        "survivalEnabled": state.survival_enabled, "survivalIntervalCursor": state.survival_interval_cursor,
        "survivalLastCheckedTicks": state.survival_last_checked_ticks,
        "waveOrdinal": state.wave_ordinal, "waveSpawnCursor": state.wave_spawn_cursor, "waveTicks": state.wave_ticks,
    })
}
