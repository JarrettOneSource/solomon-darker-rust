//! Borrowed atomic host transactions from `prepared-mod-host.ts`.

use solomon_sim::enemies::EnemySpawnIntent;
use solomon_sim::modding::runtime::*;
use solomon_sim::session::GameSession;

use crate::host_intents::apply_game_intent;
use crate::host_runtime::PreparedModRuntime;
use crate::intents::{ModIntentAdapter, ModIntentExecutionContext, ModIntentTransaction};
use crate::player_control::{ModControlLease, ModPlayerControl, select_mod_player_skill};

pub(crate) struct HostIntentAdapter<'a> {
    pub game: &'a mut GameSession,
    pub runtime: &'a mut PreparedModRuntime,
    pub controls: &'a mut ModPlayerControl,
    pub spawns: &'a mut Vec<EnemySpawnIntent>,
    pub presentation: &'a mut Vec<PreparedModPresentationIntent>,
}

struct TransactionCheckpoint {
    statuses: ModStatusCheckpoint,
    powerups: ModPowerupCheckpoint,
    enemies: ModEnemyCheckpoint,
    scenes: ModSceneCheckpoint,
    semantic_state: ModSemanticStateCheckpoint,
    spell_effects: ModSpellEffectCheckpoint,
    controls: Vec<(String, ModControlLease)>,
}

impl TransactionCheckpoint {
    fn capture(runtime: &PreparedModRuntime, controls: &ModPlayerControl) -> Self {
        Self {
            statuses: runtime.statuses.checkpoint(),
            powerups: runtime.powerups.checkpoint(),
            enemies: runtime.enemies.checkpoint(),
            scenes: runtime.scenes.checkpoint(),
            semantic_state: runtime.semantic_state.checkpoint(),
            spell_effects: runtime.spell_effects.checkpoint(),
            controls: controls.checkpoint(),
        }
    }
    fn restore(
        &self,
        runtime: &mut PreparedModRuntime,
        controls: &mut ModPlayerControl,
    ) -> Result<(), String> {
        runtime.statuses.restore(self.statuses.clone())?;
        runtime.powerups.restore(self.powerups.clone())?;
        runtime.enemies.restore(self.enemies.clone())?;
        runtime.scenes.restore(self.scenes.clone())?;
        runtime.semantic_state.restore(self.semantic_state.clone())?;
        runtime.spell_effects.restore(self.spell_effects.clone())?;
        controls.restore(self.controls.clone());
        Ok(())
    }
}

struct HostIntentTransaction<'a> {
    game: &'a mut GameSession,
    candidate: Option<GameSession>,
    previous: Option<GameSession>,
    runtime: &'a mut PreparedModRuntime,
    controls: &'a mut ModPlayerControl,
    checkpoint: TransactionCheckpoint,
    spawns: &'a mut Vec<EnemySpawnIntent>,
    births: Vec<EnemySpawnIntent>,
    presentation: &'a mut Vec<PreparedModPresentationIntent>,
    projected: Vec<PreparedModPresentationIntent>,
    initial_spawn_count: usize,
    initial_presentation_count: usize,
}

impl ModIntentAdapter for HostIntentAdapter<'_> {
    fn prepare<'a>(
        &'a mut self,
        intents: &'a [ModIntent],
        context: &'a ModIntentExecutionContext,
    ) -> Result<Box<dyn ModIntentTransaction + 'a>, String> {
        let checkpoint = TransactionCheckpoint::capture(self.runtime, self.controls);
        // Source builds an immutable candidate and writes it only at commit.
        // This transient clone is the same canonical Session type, including its
        // actual RNG streams. It is never stepped or retained as another world.
        let mut candidate = self.game.clone();
        let mut births = Vec::new();
        let mut projected = Vec::new();
        let prepared = (|| {
            for intent in intents {
                if intent.kind == "input" {
                    self.controls.accept(&candidate, intent, context)?;
                    continue;
                }
                if intent.kind == "select_skill" {
                    select_mod_player_skill(&mut candidate, &intent.fields, context)?;
                    continue;
                }
                let applied = apply_game_intent(&mut candidate, self.runtime, intent, context)?;
                if let Some(spawn) = applied.spawn {
                    births.push(spawn);
                }
                if let Some(presentation) = applied.presentation {
                    projected.push(presentation);
                }
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = prepared {
            checkpoint.restore(self.runtime, self.controls)?;
            return Err(error);
        }
        Ok(Box::new(HostIntentTransaction {
            game: self.game,
            candidate: Some(candidate),
            previous: None,
            runtime: self.runtime,
            controls: self.controls,
            checkpoint,
            initial_spawn_count: self.spawns.len(),
            initial_presentation_count: self.presentation.len(),
            spawns: self.spawns,
            births,
            presentation: self.presentation,
            projected,
        }))
    }
}

impl ModIntentTransaction for HostIntentTransaction<'_> {
    fn commit(&mut self) -> Result<(), String> {
        // The exclusive game borrow enforces Source's state-identity guard:
        // nothing can replace authority between prepare and this commit.
        if self.candidate.is_none() {
            return Err("authoritative game state changed during a mod transaction".into());
        }
        if self.presentation.len() + self.projected.len() > 1_024 {
            return Err("pending mod presentation intent limit reached".into());
        }
        let candidate = self.candidate.take().unwrap();
        self.previous = Some(std::mem::replace(self.game, candidate));
        self.spawns.extend(self.births.iter().cloned());
        self.presentation.extend(self.projected.iter().cloned());
        Ok(())
    }
    fn rollback(&mut self, _reason: &str) -> Result<(), String> {
        self.checkpoint.restore(self.runtime, self.controls)?;
        if let Some(previous) = self.previous.take() {
            *self.game = previous;
            self.spawns.truncate(self.initial_spawn_count);
            self.presentation.truncate(self.initial_presentation_count);
        }
        Ok(())
    }
}
