//! Current Source `client/flame-presentation.ts`: connection births augment only
//! sampled display spells. A reset never invents missing legacy birth samples.

use std::collections::BTreeMap;

use solomon_sim::spells::model::SpellState;
use solomon_sim::spells::weld::{
    NATIVE_FLAME_CAMERA_MAX_PRESENTATION_TICKS, NativeFlameCameraProposal, NativeFlameChannelVisual,
};

use super::{
    FlamePresentationBirth, FlamePresentationFrame, FlamePresentationRecipe,
    FlamePresentationReplay, MAX_FLAME_PRESENTATION_BIRTHS, flame_presentation_birth,
    materialize_flame_presentation,
};

pub struct RetainedFlamePresentation {
    replay: FlamePresentationReplay,
    reset_tick: u64,
    epoch: u64,
}

impl RetainedFlamePresentation {
    pub fn new(initial: Option<&FlamePresentationFrame>) -> Result<Self, String> {
        let mut replay = FlamePresentationReplay::new(
            MAX_FLAME_PRESENTATION_BIRTHS,
            Some(retained_camera_end_tick),
        );
        if let Some(frame) = initial {
            // A new timeline has no interpolation history, even if admission
            // supplies a continuous frame. Complete live actors provide fallback.
            replay.accept(&FlamePresentationFrame::reset(
                frame.epoch(),
                frame.world_key().into(),
                frame.through_tick(),
            ))?;
        }
        Ok(Self {
            replay,
            reset_tick: initial.map_or(0, FlamePresentationFrame::through_tick),
            epoch: initial.map_or(0, FlamePresentationFrame::epoch),
        })
    }

    /// Returns true when the timeline must discard its previous snapshot bracket.
    pub fn accept(
        &mut self,
        frame: Option<&FlamePresentationFrame>,
        oldest_tick: u64,
    ) -> Result<bool, String> {
        let Some(frame) = frame else { return Ok(false) };
        if frame.epoch() < self.epoch
            || (matches!(frame, FlamePresentationFrame::Reset { .. })
                && frame.epoch() == self.epoch
                && frame.through_tick() < self.replay.through_tick())
        {
            return Ok(false);
        }
        let replaced = self.epoch != 0 && frame.epoch() != self.epoch;
        if matches!(frame, FlamePresentationFrame::Reset { .. }) {
            self.reset_tick = frame.through_tick();
        }
        self.replay.discard_before_tick(oldest_tick)?;
        self.replay.accept(frame)?;
        self.epoch = frame.epoch();
        Ok(replaced)
    }

    pub fn camera_displacements(
        &self,
        spells: &SpellState,
        stopped_world_tick: f64,
        live_region_tick: f64,
    ) -> Result<Vec<NativeFlameCameraProposal>, String> {
        if stopped_world_tick < self.reset_tick as f64 {
            return Ok(Vec::new());
        }
        let mut known = BTreeMap::new();
        for birth in self.replay.history_at(stopped_world_tick) {
            known.insert(birth.id, birth.clone());
        }
        for actor in spells.transients.iter() {
            if let Some(birth) = flame_presentation_birth(actor)? {
                known.entry(birth.id).or_insert(birth);
            }
        }
        let mut proposals = Vec::new();
        for birth in known.into_values() {
            let FlamePresentationRecipe::Ribbon {
                birth: NativeFlameChannelVisual::NativeBirth(visual),
                ..
            } = birth.recipe
            else {
                continue;
            };
            let Some(displacement) = visual.camera_displacement else { continue };
            if birth.birth_tick as f64 > stopped_world_tick
                || live_region_tick
                    >= (birth.birth_tick + NATIVE_FLAME_CAMERA_MAX_PRESENTATION_TICKS) as f64
            {
                continue;
            }
            proposals.push(NativeFlameCameraProposal {
                id: birth.id,
                birth_tick: birth.birth_tick,
                world_key: birth.world_key,
                displacement,
            });
        }
        proposals.sort_by_key(|proposal| (proposal.birth_tick, proposal.id));
        Ok(proposals)
    }

    /// The caller supplies its private display copy, never the accepted Session.
    pub fn sample(&self, spells: &mut SpellState, stopped_world_tick: f64) -> Result<(), String> {
        let mut retained = BTreeMap::new();
        for birth in self.replay.sample(stopped_world_tick)? {
            retained.insert(birth.id, birth.clone());
        }
        let births = spells
            .transients
            .iter()
            .map(flame_presentation_birth)
            .collect::<Result<Vec<_>, _>>()?;
        let mut replaced_ids = std::collections::BTreeSet::new();
        for birth in births.into_iter().flatten() {
            replaced_ids.insert(birth.id);
            let admitted = self.replay.retained(&birth).unwrap_or(&birth);
            if stopped_world_tick >= self.reset_tick as f64
                && admitted.birth_tick as f64 <= stopped_world_tick
                && stopped_world_tick < admitted.end_tick_exclusive as f64
            {
                retained.entry(admitted.id).or_insert_with(|| admitted.clone());
            }
        }
        spells.transients.retain(|actor| !replaced_ids.contains(&actor.common().id));
        if stopped_world_tick >= self.reset_tick as f64 {
            for birth in retained.values() {
                if let Some(actor) = materialize_flame_presentation(birth, stopped_world_tick) {
                    spells.transients.push(actor);
                }
            }
        }
        spells.transients.sort_by_key(|actor| actor.common().id);
        Ok(())
    }
}

fn retained_camera_end_tick(birth: &FlamePresentationBirth) -> u64 {
    if let FlamePresentationRecipe::Ribbon {
        birth: NativeFlameChannelVisual::NativeBirth(visual),
        ..
    } = &birth.recipe
        && visual.camera_displacement.is_some()
    {
        return birth.birth_tick + NATIVE_FLAME_CAMERA_MAX_PRESENTATION_TICKS;
    }
    birth.end_tick_exclusive
}
