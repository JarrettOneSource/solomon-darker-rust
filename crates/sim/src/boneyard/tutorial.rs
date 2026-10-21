//! Stock Tutorial progression from `core-kernels/native-tutorial.ts`.
//!
//! Narration owns its PCM-derived clock independently of audio playback. The state retains
//! all stage, wave, camera and random cursors so a saved Tutorial resumes the same future.

mod callouts;
mod camera;
mod presentation;
mod tables;

pub use callouts::*;
pub use camera::*;
pub use presentation::*;
pub use tables::*;

use crate::enemies::{EnemySpawnIntent, SpawnLocationPolicy, SpawnPositionPolicy};
use crate::float;
use crate::float::fround;
use crate::rng::seed_from_text;
use crate::{NativeRng, Vec2};

use super::encounter::SolomonPhase;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeTutorialStage {
    Movement = 0,
    ConfrontSolomon = 1,
    ThrowMissiles = 2,
    ClearSkeletons = 3,
    WaitForPraise = 4,
    CastAcidRain = 5,
    ClearItemWave = 6,
    AnnounceAmulet = 7,
    CollectAmulet = 8,
    OpenInventory = 9,
    CloseInventory = 10,
    ClubEnemies = 11,
    OpenSkills = 12,
    CloseSkills = 13,
    ClearArcherWave = 14,
    AwaitArcherWave = 15,
    WaitForPotionDrop = 16,
    CollectPotion = 17,
    DrinkPotion = 18,
    Survive = 19,
}

impl NativeTutorialStage {
    pub const ALL: [Self; 20] = [
        Self::Movement,
        Self::ConfrontSolomon,
        Self::ThrowMissiles,
        Self::ClearSkeletons,
        Self::WaitForPraise,
        Self::CastAcidRain,
        Self::ClearItemWave,
        Self::AnnounceAmulet,
        Self::CollectAmulet,
        Self::OpenInventory,
        Self::CloseInventory,
        Self::ClubEnemies,
        Self::OpenSkills,
        Self::CloseSkills,
        Self::ClearArcherWave,
        Self::AwaitArcherWave,
        Self::WaitForPotionDrop,
        Self::CollectPotion,
        Self::DrinkPotion,
        Self::Survive,
    ];

    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub fn from_id(id: u8) -> Option<Self> {
        Self::ALL.get(usize::from(id)).copied()
    }
}

pub use crate::tutorial_surface::NativeTutorialSurfaceAction;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeTutorialNarrationEvent {
    pub cue: NativeTutorialCue,
    pub event_id: u64,
    pub speaker: NativeTutorialSpeaker,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeTutorialNarrationState {
    pub current: Option<NativeTutorialNarrationEvent>,
    pub next_event_id: u64,
    pub pending: Vec<NativeTutorialCue>,
    pub ticks_remaining: u64,
}

impl Default for NativeTutorialNarrationState {
    fn default() -> Self {
        Self { current: None, next_event_id: 1, pending: Vec::new(), ticks_remaining: 0 }
    }
}

impl NativeTutorialNarrationState {
    fn start(&mut self, cue: NativeTutorialCue) {
        let definition = cue.definition();
        self.current = Some(NativeTutorialNarrationEvent {
            cue,
            event_id: self.next_event_id,
            speaker: definition.speaker,
            text: definition.text.to_owned(),
        });
        self.next_event_id += 1;
        self.ticks_remaining = definition.duration_ticks;
    }

    fn enqueue(&mut self, cues: &[NativeTutorialCue]) {
        let Some((first, remaining)) = cues.split_first() else {
            return;
        };
        if self.current.is_none() {
            self.start(*first);
            self.pending.clear();
            self.pending.extend_from_slice(remaining);
        } else {
            self.pending.extend_from_slice(cues);
        }
    }

    fn step(&mut self) {
        if self.current.is_some() && self.ticks_remaining > 1 {
            self.ticks_remaining -= 1;
            return;
        }
        if self.pending.is_empty() {
            if self.current.is_some() {
                self.current = None;
                self.ticks_remaining = 0;
            }
        } else {
            let next = self.pending.remove(0);
            self.start(next);
        }
    }

    #[must_use]
    pub fn idle(&self) -> bool {
        self.current.is_none() && self.pending.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeTutorialState {
    pub active: bool,
    pub camera_lock_age_ticks: f64,
    pub camera_lock_triggered: bool,
    pub camera_lock_ticks_remaining: u64,
    pub damage_protection: bool,
    pub dialogue_armed: bool,
    pub intro_active: bool,
    pub intro_blend: f64,
    pub intro_delay_ticks_remaining: u64,
    pub intro_fade: f64,
    pub intro_movement_ticks_remaining: u64,
    pub inventory_opened: bool,
    pub inventory_seen: bool,
    pub item_drop_armed: bool,
    pub movement_anchor: Vec2,
    pub movement_instruction_acknowledged: bool,
    pub narration: NativeTutorialNarrationState,
    pub next_spawn_intent_id: usize,
    pub primary_cast_sequence_at_start: u64,
    pub rng: NativeRng,
    pub selected_skill_hud_acknowledged: bool,
    pub skills_opened: bool,
    pub skills_seen: bool,
    pub solomon_dialogue_queued: bool,
    pub solomon_retreat_queued: bool,
    pub stage: NativeTutorialStage,
    pub stage_ticks: u64,
    pub survival_enabled: bool,
    pub survival_interval_cursor: usize,
    pub survival_last_checked_ticks: [u64; 3],
    pub wave_ordinal: u64,
    pub wave_spawn_cursor: usize,
    pub wave_ticks: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeTutorialTickInput {
    pub acid_rain_cast_sequence: u64,
    pub acid_rain_last_skill_id: Option<usize>,
    pub camera_lock_safety_clear: bool,
    pub current_health: f64,
    pub enemy_count: usize,
    pub ground_sack_count: usize,
    pub has_top_level_non_potion_item: bool,
    pub health_potion_count: usize,
    pub level: u32,
    pub level_up_pending: bool,
    pub maximum_health: f64,
    pub player_action_idle: bool,
    pub player_position: Vec2,
    pub primary_cast_sequence: u64,
    pub solomon_phase: Option<SolomonPhase>,
    pub solomon_run_event_id: u64,
    pub tick: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeTutorialTickResult {
    pub force_offer_skill_ids: Option<[usize; 3]>,
    pub grant_experience: f64,
    pub spawn_intents: Vec<EnemySpawnIntent>,
}

impl NativeTutorialState {
    #[must_use]
    pub fn new(player_position: Vec2, primary_cast_sequence: u64, seed: &str) -> Self {
        Self {
            active: true,
            camera_lock_age_ticks: 0.0,
            camera_lock_triggered: false,
            camera_lock_ticks_remaining: 0,
            damage_protection: true,
            dialogue_armed: true,
            intro_active: true,
            intro_blend: 0.0,
            intro_delay_ticks_remaining: 25,
            intro_fade: 1.0,
            intro_movement_ticks_remaining: 0,
            inventory_opened: false,
            inventory_seen: false,
            item_drop_armed: true,
            movement_anchor: Vec2::new(
                fround(player_position.x + 50.0),
                fround(player_position.y - 100.0),
            ),
            movement_instruction_acknowledged: false,
            narration: NativeTutorialNarrationState::default(),
            next_spawn_intent_id: 1,
            primary_cast_sequence_at_start: primary_cast_sequence,
            rng: NativeRng::new(i64::from(seed_from_text(&format!("{seed}:tutorial")))),
            selected_skill_hud_acknowledged: false,
            skills_opened: false,
            skills_seen: false,
            solomon_dialogue_queued: false,
            solomon_retreat_queued: false,
            stage: NativeTutorialStage::Movement,
            stage_ticks: 0,
            survival_enabled: false,
            survival_interval_cursor: 0,
            survival_last_checked_ticks: [0; 3],
            wave_ordinal: 0,
            wave_spawn_cursor: 0,
            wave_ticks: 0,
        }
    }

    pub fn apply_surface_action(&mut self, action: NativeTutorialSurfaceAction) {
        use NativeTutorialStage as Stage;
        match action {
            NativeTutorialSurfaceAction::PrimarySelectorOpened
            | NativeTutorialSurfaceAction::ConcentrationASelectorOpened => {
                if native_tutorial_hud_access(self).spell {
                    self.selected_skill_hud_acknowledged = true;
                }
            }
            NativeTutorialSurfaceAction::InventoryOpened => {
                if !matches!(self.stage, Stage::OpenInventory | Stage::CloseInventory) {
                    return;
                }
                self.inventory_opened = true;
                self.inventory_seen = true;
                if self.stage == Stage::OpenInventory {
                    self.enter_stage(Stage::CloseInventory);
                }
            }
            NativeTutorialSurfaceAction::InventoryClosed => {
                if !matches!(self.stage, Stage::OpenInventory | Stage::CloseInventory)
                    || !self.inventory_seen
                {
                    return;
                }
                self.inventory_opened = false;
                if self.stage == Stage::OpenInventory {
                    self.enter_stage(Stage::CloseInventory);
                }
            }
            NativeTutorialSurfaceAction::SkillsOpened => {
                if !matches!(self.stage, Stage::OpenSkills | Stage::CloseSkills) {
                    return;
                }
                self.skills_opened = true;
                self.skills_seen = true;
                if self.stage == Stage::OpenSkills {
                    self.enter_stage(Stage::CloseSkills);
                }
            }
            NativeTutorialSurfaceAction::SkillsClosed => {
                if !matches!(self.stage, Stage::OpenSkills | Stage::CloseSkills)
                    || !self.skills_seen
                {
                    return;
                }
                self.skills_opened = false;
                if self.stage == Stage::OpenSkills {
                    self.enter_stage(Stage::CloseSkills);
                }
            }
        }
    }

    pub fn acknowledge_movement_instruction(
        &mut self,
        movement_epoch_active: bool,
        user_movement_requested: bool,
    ) {
        if self.active
            && !self.intro_active
            && self.stage == NativeTutorialStage::Movement
            && movement_epoch_active
            && user_movement_requested
        {
            self.movement_instruction_acknowledged = true;
        }
    }

    pub fn step(&mut self, input: NativeTutorialTickInput) -> NativeTutorialTickResult {
        use NativeTutorialCue as Cue;
        use NativeTutorialStage as Stage;
        if self.camera_lock_triggered && !input.camera_lock_safety_clear {
            self.camera_lock_age_ticks = 0.0;
            self.camera_lock_triggered = false;
            self.camera_lock_ticks_remaining = 0;
        }
        if self.intro_active {
            self.step_intro();
            return NativeTutorialTickResult::default();
        }
        self.tick_base(&input);
        let mut result = NativeTutorialTickResult::default();
        if input.solomon_phase == Some(SolomonPhase::Speaking) && !self.solomon_dialogue_queued {
            self.narration.enqueue(&[
                Cue::OhBoyAnotherWizard,
                Cue::BeenDispatched,
                Cue::DoTheDispatching,
                Cue::YourPerversions,
                Cue::ToDeathExactly,
            ]);
            self.solomon_dialogue_queued = true;
        }
        if matches!(
            input.solomon_phase,
            Some(SolomonPhase::RetreatAccelerating | SolomonPhase::Escaping)
        ) && !self.solomon_retreat_queued
        {
            self.narration.enqueue(&[
                Cue::SolomonLaugh1,
                Cue::CowardComeBack,
                Cue::SolomonGetHimBoys,
            ]);
            self.solomon_retreat_queued = true;
        }
        if self.active {
            match self.stage {
                Stage::Movement => {
                    if self.dialogue_armed && self.stage_ticks > 50 {
                        self.narration.enqueue(&[Cue::ShowYourself]);
                        self.dialogue_armed = false;
                    }
                    let delta = Vec2::new(
                        input.player_position.x - self.movement_anchor.x,
                        input.player_position.y - self.movement_anchor.y,
                    );
                    if delta.x * delta.x + delta.y * delta.y > 40_000.0 {
                        self.dialogue_armed = true;
                        self.enter_stage(Stage::ConfrontSolomon);
                    }
                }
                Stage::ConfrontSolomon => {
                    if input.solomon_run_event_id > 0 {
                        self.enter_stage(Stage::ThrowMissiles);
                        self.start_wave(1, input.tick);
                    }
                }
                Stage::ThrowMissiles => {
                    if self.dialogue_armed && input.enemy_count > 0 {
                        self.narration.enqueue(&[Cue::IAmSirmin, Cue::NeverHeardOfYou]);
                        self.dialogue_armed = false;
                    }
                    if input.primary_cast_sequence > self.primary_cast_sequence_at_start {
                        self.enter_stage(Stage::ClearSkeletons);
                    }
                }
                Stage::ClearSkeletons => {
                    if input.enemy_count == 0 {
                        self.narration.enqueue(&[Cue::EasilyVanquished]);
                        self.enter_stage(Stage::WaitForPraise);
                    }
                }
                Stage::WaitForPraise => {
                    if self.narration.idle() {
                        self.enter_stage(Stage::CastAcidRain);
                    }
                }
                Stage::CastAcidRain => {
                    if input.acid_rain_cast_sequence > 0
                        && input.acid_rain_last_skill_id == Some(NATIVE_TUTORIAL_ACID_RAIN_SKILL_ID)
                    {
                        self.narration.enqueue(&[
                            Cue::CamePrepared,
                            Cue::AcidRainHuh,
                            Cue::Surrender,
                        ]);
                        self.enter_stage(Stage::ClearItemWave);
                        self.start_wave(2, input.tick);
                    }
                }
                Stage::ClearItemWave => {
                    if input.enemy_count == 0 {
                        self.enter_stage(Stage::AnnounceAmulet);
                    }
                }
                Stage::AnnounceAmulet => {
                    self.narration.enqueue(&[Cue::CarelessFool]);
                    self.enter_stage(if input.has_top_level_non_potion_item {
                        Stage::OpenInventory
                    } else {
                        Stage::CollectAmulet
                    });
                }
                Stage::CollectAmulet => {
                    if input.has_top_level_non_potion_item {
                        self.enter_stage(Stage::OpenInventory);
                    }
                }
                Stage::OpenInventory => {
                    if self.inventory_opened {
                        self.enter_stage(Stage::CloseInventory);
                    }
                }
                Stage::CloseInventory => {
                    if self.inventory_seen && !self.inventory_opened {
                        self.narration.enqueue(&[
                            Cue::Unredeemable,
                            Cue::SoundLikeMother,
                            Cue::AcceptYourFate,
                        ]);
                        self.enter_stage(Stage::ClubEnemies);
                        self.start_wave(3, input.tick);
                        result.force_offer_skill_ids = Some([65, 67, 60]);
                    }
                }
                Stage::ClubEnemies => {
                    if self.stage_ticks > 100 && input.enemy_count == 0 && input.level < 2 {
                        result.grant_experience = 10.0;
                    }
                    if input.level > 1 && !input.level_up_pending && input.player_action_idle {
                        self.narration.enqueue(&[Cue::MakeMeStronger, Cue::LevellingUp]);
                        self.enter_stage(Stage::OpenSkills);
                    }
                }
                Stage::OpenSkills => {
                    if self.skills_opened {
                        self.enter_stage(Stage::CloseSkills);
                    }
                }
                Stage::CloseSkills => {
                    if self.skills_seen && !self.skills_opened {
                        self.damage_protection = false;
                        self.enter_stage(Stage::AwaitArcherWave);
                        self.start_wave(4, input.tick);
                    }
                }
                Stage::AwaitArcherWave => {
                    if input.enemy_count > 2 {
                        self.dialogue_armed = true;
                        self.enter_stage(Stage::ClearArcherWave);
                    }
                }
                Stage::ClearArcherWave => {
                    if self.dialogue_armed
                        && input.enemy_count < 4
                        && input.current_health < input.maximum_health
                    {
                        self.narration.enqueue(&[Cue::LookingBeatUp]);
                        self.dialogue_armed = false;
                    }
                    if input.enemy_count == 0 {
                        self.enter_stage(Stage::WaitForPotionDrop);
                        self.start_wave(5, input.tick);
                    }
                }
                Stage::WaitForPotionDrop => {
                    if input.ground_sack_count > 0 {
                        self.enter_stage(Stage::CollectPotion);
                    }
                }
                Stage::CollectPotion => {
                    if input.ground_sack_count == 0 {
                        self.enter_stage(Stage::DrinkPotion);
                    }
                }
                Stage::DrinkPotion => {
                    if input.health_potion_count == 0 {
                        self.narration.enqueue(&[Cue::FaceTheWrath, Cue::ImBored]);
                        self.enter_stage(Stage::Survive);
                        self.start_wave(6, input.tick);
                        result.force_offer_skill_ids = Some([8, 72, 57]);
                    }
                }
                Stage::Survive => {
                    if input.enemy_count > 5 {
                        self.active = false;
                    }
                }
            }
        }
        result.spawn_intents = self.materialize_scheduled_spawns(&input);
        result
    }

    fn enter_stage(&mut self, stage: NativeTutorialStage) {
        self.stage = stage;
        self.stage_ticks = 0;
    }

    fn start_wave(&mut self, wave_ordinal: u64, tick: u64) {
        self.survival_enabled = wave_ordinal == 6;
        self.survival_interval_cursor = 0;
        if wave_ordinal == 6 {
            self.survival_last_checked_ticks = [tick; 3];
        }
        self.wave_ordinal = wave_ordinal;
        self.wave_spawn_cursor = 0;
        self.wave_ticks = 0;
    }

    fn tick_base(&mut self, input: &NativeTutorialTickInput) {
        if self.camera_lock_triggered {
            self.camera_lock_age_ticks = (self.camera_lock_age_ticks + 1.0)
                .min(NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS as f64);
            self.camera_lock_ticks_remaining = self.camera_lock_ticks_remaining.saturating_sub(1);
        } else if input.camera_lock_safety_clear
            && point_inside(input.player_position, NATIVE_TUTORIAL_CAMERA_TRIGGER)
        {
            self.camera_lock_triggered = true;
            self.camera_lock_age_ticks = 0.0;
            self.camera_lock_ticks_remaining = NATIVE_TUTORIAL_CAMERA_CLEANUP_TICKS;
        }
        self.intro_movement_ticks_remaining = self.intro_movement_ticks_remaining.saturating_sub(1);
        self.narration.step();
        self.stage_ticks += 1;
        self.wave_ticks = if self.wave_ordinal == 0 { 0 } else { self.wave_ticks + 1 };
    }

    fn step_intro(&mut self) {
        if self.intro_delay_ticks_remaining == 20 {
            self.intro_movement_ticks_remaining = 250;
        }
        self.intro_delay_ticks_remaining = self.intro_delay_ticks_remaining.saturating_sub(1);
        if self.intro_delay_ticks_remaining > 0 {
            return;
        }
        self.intro_blend = advance_native_tutorial_intro_blend(self.intro_blend);
        if self.intro_blend < 1.0 {
            return;
        }
        self.intro_fade = fround(self.intro_fade - 0.02).max(0.0);
        self.intro_active = self.intro_fade > 0.0;
    }

    fn materialize_scheduled_spawns(
        &mut self,
        input: &NativeTutorialTickInput,
    ) -> Vec<EnemySpawnIntent> {
        let mut intents = Vec::new();
        let batches = native_tutorial_wave_batches(self.wave_ordinal);
        while let Some(batch) = batches.get(self.wave_spawn_cursor) {
            if batch.tick > self.wave_ticks {
                break;
            }
            intents.extend(self.materialize_batch(*batch, input.player_position, input.tick));
            self.wave_spawn_cursor += 1;
        }
        if self.survival_enabled {
            let cursor = self.survival_interval_cursor;
            self.survival_interval_cursor = (cursor + 1) % 3;
            let periods = [100, 100, 150];
            if input.tick - self.survival_last_checked_ticks[cursor] >= periods[cursor] {
                self.survival_last_checked_ticks[cursor] = input.tick;
                let eligible = match cursor {
                    0 => input.enemy_count < 100,
                    1 => input.enemy_count > 10 && input.enemy_count < 150 && input.level < 4,
                    2 => input.level > 3,
                    _ => unreachable!("Tutorial owns three survival interval cursors"),
                };
                if eligible {
                    let batch = NativeTutorialSpawnBatch {
                        count: Some(1),
                        group_uid: Some(if cursor == 2 { 10086 } else { 10078 }),
                        position_policy: SpawnPositionPolicy::Light,
                        recipe_uid: None,
                        tick: 0,
                    };
                    intents.extend(self.materialize_batch(
                        batch,
                        input.player_position,
                        input.tick,
                    ));
                }
            }
        }
        intents
    }

    fn materialize_batch(
        &mut self,
        batch: NativeTutorialSpawnBatch,
        player_position: Vec2,
        spawn_tick: u64,
    ) -> Vec<EnemySpawnIntent> {
        let group = batch
            .group_uid
            .map(|uid| native_tutorial_uid_group(uid).expect("authored Tutorial group exists"));
        let spawn_count = if batch.recipe_uid.is_some() {
            1
        } else {
            let group = group.expect("Tutorial batch selects a recipe or group");
            batch.count.unwrap_or(group.member_uids.len()).min(group.member_uids.len())
        };
        let placement_group_id =
            group.filter(|group| group.share_final_root).map(|_| self.next_spawn_intent_id);
        let mut intents = Vec::with_capacity(spawn_count);
        for index in 0..spawn_count {
            let recipe_uid = if let Some(uid) = batch.recipe_uid {
                uid
            } else {
                let group = group.expect("group batch retains its authored group");
                let member = if batch.count.is_none_or(|count| count >= group.member_uids.len()) {
                    index
                } else {
                    self.rng.draw_integer(group.member_uids.len() as i32) as usize
                };
                group.member_uids[member]
            };
            // Even a one-player selection advances the native stream before its angle.
            self.rng.draw_integer(1);
            let radians = self.rng.draw_scaled_float(360.0, false) * std::f64::consts::PI / 180.0;
            let position = Vec2::new(
                fround(player_position.x + float::cos(radians) * 100.0),
                fround(player_position.y + float::sin(radians) * 100.0),
            );
            let definition = native_tutorial_monster_recipe(recipe_uid)
                .expect("authored Tutorial recipe exists");
            let mut intent = EnemySpawnIntent::new(
                definition.enemy_token,
                position,
                self.next_spawn_intent_id,
                spawn_tick,
                self.wave_ordinal,
            );
            self.next_spawn_intent_id += 1;
            intent.authored_recipe = Some(definition.recipe);
            intent.flanking = definition.flanking;
            intent.pathfinding_mode = definition.pathfinding_mode;
            intent.location_policy = SpawnLocationPolicy::NearPlayer;
            intent.placement_group_id = placement_group_id;
            intent.position_policy = batch.position_policy;
            intents.push(intent);
        }
        intents
    }
}

fn advance_native_tutorial_intro_blend(source: f64) -> f64 {
    fround(source + 0.0025).min(1.0)
}

#[must_use]
pub const fn native_tutorial_dialogue_ticks() -> u64 {
    844 + 547 + 267 + 782 + 614
}

#[must_use]
pub fn native_tutorial_hostile_scene_paused(state: &NativeTutorialState) -> bool {
    state.active && !state.intro_active && state.stage == NativeTutorialStage::ThrowMissiles
}
