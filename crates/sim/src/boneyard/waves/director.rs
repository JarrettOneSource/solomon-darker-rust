//! Native survival TimeLine execution from `core-kernels/boneyard-wave-director.ts`.
//!
//! The director emits authored spawn requests. Enemy construction owns their final collision
//! placement and stats; population feedback comes from living actors, never a local estimate.

use crate::enemies::config::EnemyClassification;
use crate::enemies::{EnemySpawnIntent, EnemyToken, SpawnLocationPolicy, SpawnPositionPolicy};
use crate::float;
use crate::float::fround;
use crate::rng::seed_from_text;
use crate::{NativeRng, Vec2};

use super::programs::{
    BossBirth, BossEncounter, BossKind, SkeletonBossState, SpiderState, SurvivalPrograms,
};
use super::timeline::{
    LULL_RELEASE_TO_NEXT_SPAWN_TICKS, PAUSE_NODE_GAP_TICKS, WAVE_LABEL_TO_FIRST_SPAWN_TICKS,
    compile_boneyard_opening, compile_boneyard_wave_section,
};
use super::{
    CompiledSpawnBurst, CompiledWaveSection, RETAIL_WAVES, WaveDef, native_survival_programs,
};
use crate::boneyard::BoneyardBounds;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum WavePhase {
    Dormant,
    Opening,
    OpeningThreshold,
    Spawning,
    WaveThreshold,
    WaveLullDelay,
    WaveLull,
    Interwave,
}

impl WavePhase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Dormant => "dormant",
            Self::Opening => "opening",
            Self::OpeningThreshold => "opening-threshold",
            Self::Spawning => "spawning",
            Self::WaveThreshold => "wave-threshold",
            Self::WaveLullDelay => "wave-lull-delay",
            Self::WaveLull => "wave-lull",
            Self::Interwave => "interwave",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SlumpgutPhase {
    Eligible,
    IntervalCountdown,
    ScriptSleep,
    Retired,
}

impl SlumpgutPhase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::IntervalCountdown => "interval-countdown",
            Self::ScriptSleep => "script-sleep",
            Self::Retired => "retired",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PortalScriptPhase {
    BossWait,
    Idle,
    Intro,
    Retired,
    Spawning,
}

impl PortalScriptPhase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BossWait => "boss-wait",
            Self::Idle => "idle",
            Self::Intro => "intro",
            Self::Retired => "retired",
            Self::Spawning => "spawning",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WaveTickContext<'a> {
    pub bounds: BoneyardBounds,
    pub live_boss_count: usize,
    pub live_enemy_count: usize,
    pub live_zombie_count: usize,
    /// Player positions in registration order, matching the source Record insertion order.
    pub players: &'a [Vec2],
    pub tick: u64,
}

/// The immutable section supplying the current spawn graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum WaveSection {
    Opening,
    Schedule { index: usize },
}

/// All mutable wave authority, including each private random word and script cursor.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WaveState {
    pub active_section: Option<WaveSection>,
    pub spider_state: SpiderState,
    pub active_burst_index: Option<usize>,
    pub active_group_index: Option<i32>,
    pub active_group_member_index: usize,
    pub burst_started: bool,
    pub burst_spawn_remaining: usize,
    pub burst_spread_ticks_remaining: f64,
    pub boss_encounters: Vec<BossEncounter>,
    pub interwave_delay_ticks: u64,
    pub lull_threshold: usize,
    pub low_population_ticks: u64,
    pub next_spawn_intent_id: usize,
    pub next_schedule_index: Option<usize>,
    pub pending_spawn_budget: usize,
    pub phase: WavePhase,
    pub portal_phase_index: usize,
    pub portal_script_phase: PortalScriptPhase,
    pub portal_spawn_remaining: usize,
    pub portal_ticks_remaining: u64,
    pub portal_timeline_paused: bool,
    pub population_threshold: usize,
    pub rng: NativeRng,
    pub schedule_index: usize,
    pub skeleton_bosses: Option<SkeletonBossState>,
    pub slumpgut_phase: SlumpgutPhase,
    pub slumpgut_poll_cursor: u64,
    pub slumpgut_recipe_uid: Option<usize>,
    pub slumpgut_ticks_remaining: u64,
    pub spawn_countdown: f64,
    pub spawn_delay_ticks: u64,
    pub wave_event_id: u64,
    pub wave_ordinal: u64,
}

/// Mutable wave authority and the compiled tables that determine its future spawns.
///
/// Old native checkpoints omitted these tables. Their missing fields select the
/// original package compiler during migration, rather than changing a saved run.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WaveCheckpoint {
    pub seed: String,
    pub source_sha256: Option<String>,
    pub state: WaveState,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub active_bursts: Option<Vec<CompiledSpawnBurst>>,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub compiled_schedule: Option<Vec<CompiledWaveSection>>,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub opening_bursts: Option<Vec<CompiledSpawnBurst>>,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub opening_release_threshold: Option<usize>,
}

/// Borrowed network projection; JSON matches the owned durable checkpoint.
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct WaveSnapshot<'a> {
    pub seed: &'a str,
    pub source_sha256: Option<&'a str>,
    pub state: &'a WaveState,
    pub active_bursts: Option<&'a [CompiledSpawnBurst]>,
    pub compiled_schedule: Option<&'a [CompiledWaveSection]>,
    pub opening_bursts: Option<&'a [CompiledSpawnBurst]>,
    pub opening_release_threshold: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaveDirector {
    pub state: WaveState,
    seed: String,
    pub active_bursts: Vec<CompiledSpawnBurst>,
    pub compiled_schedule: Vec<CompiledWaveSection>,
    pub opening_bursts: Vec<CompiledSpawnBurst>,
    pub opening_release_threshold: usize,
    pub schedule: Vec<WaveDef>,
    pub programs: Option<SurvivalPrograms>,
}

impl WaveDirector {
    #[must_use]
    pub fn new(seed: &str, source_sha256: Option<&str>) -> Self {
        Self::with_schedule(seed, RETAIL_WAVES, source_sha256)
    }

    #[must_use]
    pub fn with_schedule(seed: &str, schedule: &[WaveDef], source_sha256: Option<&str>) -> Self {
        validate_schedule(schedule);
        let mut compiler_rng =
            NativeRng::new(i64::from(seed_from_text(&format!("{seed}:wave-compiler"))));
        let opening = compile_boneyard_opening(&mut compiler_rng);
        let mut compiled_schedule = Vec::with_capacity(schedule.len());
        for (index, wave) in schedule.iter().enumerate() {
            compiled_schedule.push(compile_boneyard_wave_section(
                wave,
                index + 1,
                &mut compiler_rng,
            ));
        }
        let programs = source_sha256.map(|sha| {
            native_survival_programs(sha)
                .expect("default Boneyard source has its retained survival programs")
        });
        let boss_encounters = programs.as_ref().map_or_else(Vec::new, |programs| {
            programs.bosses.iter().map(|definition| BossEncounter::new(definition.kind)).collect()
        });
        let skeleton_bosses = programs.as_ref().map(|_| SkeletonBossState::default());
        let slumpgut_recipe_uid = programs.as_ref().map(|programs| programs.slumpgut_recipe.uid);
        let portal_script_phase =
            if programs.is_some() { PortalScriptPhase::Idle } else { PortalScriptPhase::Retired };
        Self {
            seed: seed.into(),
            state: WaveState {
                active_section: None,
                spider_state: SpiderState::default(),
                active_burst_index: None,
                active_group_index: None,
                active_group_member_index: 0,
                burst_started: false,
                burst_spawn_remaining: 0,
                burst_spread_ticks_remaining: 0.0,
                boss_encounters,
                interwave_delay_ticks: 0,
                lull_threshold: 0,
                low_population_ticks: 0,
                next_spawn_intent_id: 1,
                next_schedule_index: None,
                pending_spawn_budget: 0,
                phase: WavePhase::Dormant,
                portal_phase_index: 0,
                portal_script_phase,
                portal_spawn_remaining: 0,
                portal_ticks_remaining: 0,
                portal_timeline_paused: false,
                population_threshold: 0,
                rng: NativeRng::new(i64::from(seed_from_text(&format!("{seed}:wave-runtime")))),
                schedule_index: 0,
                skeleton_bosses,
                slumpgut_phase: SlumpgutPhase::Eligible,
                slumpgut_poll_cursor: 0,
                slumpgut_recipe_uid,
                slumpgut_ticks_remaining: 0,
                spawn_countdown: 0.0,
                spawn_delay_ticks: 0,
                wave_event_id: 0,
                wave_ordinal: 0,
            },
            active_bursts: Vec::new(),
            compiled_schedule,
            opening_bursts: opening.bursts,
            opening_release_threshold: opening.release_threshold,
            schedule: schedule.to_vec(),
            programs,
        }
    }

    #[must_use]
    pub fn checkpoint(&self) -> WaveCheckpoint {
        WaveCheckpoint {
            seed: self.seed.clone(),
            source_sha256: self.programs.as_ref().map(|programs| programs.source_sha256.clone()),
            state: self.state.clone(),
            active_bursts: Some(self.active_bursts.clone()),
            compiled_schedule: Some(self.compiled_schedule.clone()),
            opening_bursts: Some(self.opening_bursts.clone()),
            opening_release_threshold: Some(self.opening_release_threshold),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> WaveSnapshot<'_> {
        WaveSnapshot {
            seed: &self.seed,
            source_sha256: self.programs.as_ref().map(|programs| programs.source_sha256.as_str()),
            state: &self.state,
            active_bursts: Some(&self.active_bursts),
            compiled_schedule: Some(&self.compiled_schedule),
            opening_bursts: Some(&self.opening_bursts),
            opening_release_threshold: Some(self.opening_release_threshold),
        }
    }

    pub fn validate_checkpoint(&self, checkpoint: &WaveCheckpoint) -> Result<(), String> {
        if checkpoint.seed != self.seed
            || checkpoint.source_sha256.as_deref()
                != self.programs.as_ref().map(|programs| programs.source_sha256.as_str())
        {
            return Err("wave checkpoint identity differs from the compiled schedule".into());
        }
        let legacy_schedule;
        let compiled_schedule = if let Some(schedule) = &checkpoint.compiled_schedule {
            schedule.as_slice()
        } else {
            legacy_schedule = compile_legacy_schedule(&checkpoint.seed, &self.schedule);
            &legacy_schedule
        };
        let opening = compile_opening_for_seed(&checkpoint.seed);
        let section_bursts = match checkpoint.state.active_section {
            None => &[][..],
            Some(WaveSection::Opening) => {
                checkpoint.opening_bursts.as_deref().unwrap_or(&opening.bursts)
            }
            Some(WaveSection::Schedule { index }) => compiled_schedule
                .get(index)
                .ok_or("wave checkpoint refers to an unknown schedule section")?
                .bursts
                .as_slice(),
        };
        let bursts = checkpoint.active_bursts.as_deref().unwrap_or(section_bursts);
        if checkpoint.state.active_burst_index.is_some_and(|index| index >= bursts.len()) {
            return Err("wave checkpoint refers to an unknown spawn burst".into());
        }
        Ok(())
    }

    /// Source saves retain their compiled graphs, including graphs made by an older
    /// compiler. Replacing the state must therefore replace these tables as well.
    pub fn apply_checkpoint(&mut self, checkpoint: WaveCheckpoint) -> Result<(), String> {
        self.validate_checkpoint(&checkpoint)?;
        self.compiled_schedule = match checkpoint.compiled_schedule {
            Some(schedule) => schedule,
            None => compile_legacy_schedule(&checkpoint.seed, &self.schedule),
        };
        let opening = compile_opening_for_seed(&checkpoint.seed);
        self.opening_bursts = checkpoint.opening_bursts.unwrap_or(opening.bursts);
        self.opening_release_threshold =
            checkpoint.opening_release_threshold.unwrap_or(opening.release_threshold);
        self.active_bursts = match checkpoint.active_bursts {
            Some(bursts) => bursts,
            None => match checkpoint.state.active_section {
                None => Vec::new(),
                Some(WaveSection::Opening) => self.opening_bursts.clone(),
                Some(WaveSection::Schedule { index }) => {
                    self.compiled_schedule[index].bursts.clone()
                }
            },
        };
        self.state = checkpoint.state;
        Ok(())
    }

    /// The storage edge validates the document. Authored programs remain static;
    /// retained compiled tables and private runtime state come from the checkpoint.
    #[must_use]
    pub fn restore(checkpoint: WaveCheckpoint) -> Self {
        Self::restore_with_schedule(checkpoint, RETAIL_WAVES)
    }

    #[must_use]
    pub fn restore_with_schedule(checkpoint: WaveCheckpoint, schedule: &[WaveDef]) -> Self {
        let mut director =
            Self::with_schedule(&checkpoint.seed, schedule, checkpoint.source_sha256.as_deref());
        director
            .apply_checkpoint(checkpoint)
            .expect("validated wave checkpoint has its retained spawn graph");
        director
    }

    pub fn start(&mut self) {
        if self.state.phase != WavePhase::Dormant {
            return;
        }
        self.state.lull_threshold = 0;
        self.state.low_population_ticks = 0;
        self.state.population_threshold = self.opening_release_threshold;
        self.state.schedule_index = 0;
        self.state.active_section = Some(WaveSection::Opening);
        self.begin_bursts(self.opening_bursts.clone(), WavePhase::Opening);
    }

    /// `canSpawnNextBoneyardWave`: no pending births or native script hold may be skipped.
    #[must_use]
    pub fn can_spawn_next_wave(&self) -> bool {
        self.state.pending_spawn_budget == 0
            && !matches!(
                self.state.phase,
                WavePhase::Dormant | WavePhase::Opening | WavePhase::Spawning
            )
            && !self.timeline_paused()
    }

    /// `spawnNextBoneyardWave` uses the ordinary signed NEXT edge and native spawn queue.
    pub fn spawn_next_wave(&mut self) {
        if !self.can_spawn_next_wave() {
            return;
        }
        if self.state.wave_ordinal == 0 {
            self.begin_schedule_row(0);
        } else {
            self.state.interwave_delay_ticks = 0;
            self.state.phase = WavePhase::Interwave;
            self.step_interwave();
        }
    }

    /// Script births count for later scripts in the same source tick. A script holding the
    /// TimeLine before or after its step prevents ordinary waves from advancing this tick.
    pub fn step(&mut self, context: WaveTickContext<'_>) -> Vec<EnemySpawnIntent> {
        let was_paused = self.timeline_paused();
        let heart_births = self.step_generated_bosses(context);
        let slump_births = self.step_slumpgut(count_births(context, &heart_births));
        // The reference passes only Slumpgut births into this specific script boundary.
        let skeleton_births = self.step_skeletons(count_births(context, &slump_births));
        let mut births = heart_births;
        births.extend(slump_births);
        births.extend(skeleton_births);
        let spider_births = self.step_spiders(count_births(context, &births));
        births.extend(spider_births);
        let portal_births = self.step_portals(count_births(context, &births));
        births.extend(portal_births);
        if was_paused || self.timeline_paused() {
            return births;
        }
        let context = count_births(context, &births);
        if self.state.phase != WavePhase::Dormant && context.live_enemy_count < 11 {
            self.state.low_population_ticks += 1;
            if self.state.low_population_ticks > 10 && context.live_enemy_count == 0 {
                self.state.low_population_ticks = 999_999_999;
            }
        }
        births.extend(self.step_ordinary_waves(context));
        births
    }

    /// The encounter's release tick polls Slumpgut only; ordinary TimeLine and other scripts
    /// begin on the following world tick (`stepBoneyardWorldTick::resolveSpawnIntents`).
    pub fn step_run_release(&mut self, context: WaveTickContext<'_>) -> Vec<EnemySpawnIntent> {
        self.step_slumpgut(context)
    }

    fn timeline_paused(&self) -> bool {
        self.state.boss_encounters.iter().any(BossEncounter::timeline_paused)
            || self.state.portal_timeline_paused
            || self.state.skeleton_bosses.as_ref().is_some_and(SkeletonBossState::timeline_paused)
            || self.state.spider_state.timeline_paused
    }

    fn step_generated_bosses(&mut self, context: WaveTickContext<'_>) -> Vec<EnemySpawnIntent> {
        let mut births = Vec::new();
        if self.state.phase == WavePhase::Dormant {
            return births;
        }
        let ordinal = self.state.wave_ordinal;
        for index in 0..self.state.boss_encounters.len() {
            let kind = self.state.boss_encounters[index].kind;
            let definition = self
                .programs
                .as_ref()
                .expect("boss encounters have authored programs")
                .bosses
                .iter()
                .find(|definition| definition.kind == kind)
                .expect("boss encounter kind has its authored definition");
            let step = self.state.boss_encounters[index].step(
                definition,
                ordinal,
                count_births(context, &births).live_boss_count,
            );
            if let Some(birth) = step.birth {
                let recipes = if birth == BossBirth::Skeleton {
                    vec![None]
                } else {
                    definition.recipes.iter().cloned().map(Some).collect()
                };
                for recipe in recipes {
                    let token = if recipe.is_none() {
                        EnemyToken::Skeleton
                    } else {
                        match kind {
                            BossKind::Heartmonger => EnemyToken::Heartmonger,
                            BossKind::Faculty => EnemyToken::DireFaculty,
                            BossKind::Discorporeal => EnemyToken::DemonSkull,
                        }
                    };
                    let mut intent = self.place_spawn(
                        token,
                        SpawnLocationPolicy::Anywhere,
                        SpawnPositionPolicy::Dark,
                        context,
                        ordinal,
                    );
                    intent.pathfinding_mode = if recipe.is_some() { 2 } else { 1 };
                    intent.enable_discorporeal_health_gates =
                        recipe.is_some() && kind == BossKind::Discorporeal;
                    intent.authored_recipe = recipe;
                    births.push(intent);
                }
            }
            if step.release_wave {
                self.start_next_wave();
            }
        }
        births
    }

    fn step_skeletons(&mut self, context: WaveTickContext<'_>) -> Vec<EnemySpawnIntent> {
        if self.state.skeleton_bosses.is_none() || self.state.phase == WavePhase::Dormant {
            return Vec::new();
        }
        let programs = self.programs.as_ref().expect("skeleton bosses have authored programs");
        let step = self.state.skeleton_bosses.as_mut().expect("skeleton state is present").step(
            &programs.ironmaw,
            &programs.foulshaft,
            self.state.wave_ordinal,
            context.live_boss_count,
        );
        let ordinal = self.state.wave_ordinal;
        let mut births = Vec::new();
        for birth in step.births {
            let mut intent = self.place_spawn(
                birth.enemy_token,
                SpawnLocationPolicy::Anywhere,
                SpawnPositionPolicy::Dark,
                context,
                ordinal,
            );
            intent.pathfinding_mode = if birth.authored_recipe.is_some() { 2 } else { 1 };
            intent.authored_recipe = birth.authored_recipe;
            intent.archer_strafing = birth.archer_strafing;
            intent.flags = birth.flags;
            births.push(intent);
        }
        if step.release_wave {
            self.start_next_wave();
        }
        births
    }

    fn step_spiders(&mut self, context: WaveTickContext<'_>) -> Vec<EnemySpawnIntent> {
        if self.state.phase == WavePhase::Dormant {
            return Vec::new();
        }
        let Some(programs) = &self.programs else { return Vec::new() };
        let step = self.state.spider_state.step(
            &programs.spiders,
            self.state.wave_ordinal,
            context.live_enemy_count,
        );
        let mut births = Vec::new();
        for birth in step.births {
            let mut intent = self.place_spawn(
                EnemyToken::Spider,
                SpawnLocationPolicy::Anywhere,
                birth.position_policy,
                context,
                self.state.wave_ordinal,
            );
            intent.flags = birth.flags;
            births.push(intent);
        }
        if step.advance_wave {
            self.start_next_wave();
        }
        births
    }

    fn step_slumpgut(&mut self, context: WaveTickContext<'_>) -> Vec<EnemySpawnIntent> {
        match self.state.slumpgut_phase {
            SlumpgutPhase::Retired => return Vec::new(),
            SlumpgutPhase::Eligible => {
                let prior_cursor = self.state.slumpgut_poll_cursor;
                self.state.slumpgut_poll_cursor = (prior_cursor + 1) % 4;
                if prior_cursor == 0 && context.live_zombie_count > 75 {
                    self.state.slumpgut_phase = SlumpgutPhase::IntervalCountdown;
                    self.state.slumpgut_ticks_remaining = 1000;
                }
                return Vec::new();
            }
            _ if self.state.slumpgut_ticks_remaining > 1 => {
                self.state.slumpgut_ticks_remaining -= 1;
                return Vec::new();
            }
            SlumpgutPhase::IntervalCountdown => {
                self.state.slumpgut_phase = SlumpgutPhase::ScriptSleep;
                self.state.slumpgut_ticks_remaining = 1500;
                return Vec::new();
            }
            SlumpgutPhase::ScriptSleep => {}
        }
        let uid = self
            .state
            .slumpgut_recipe_uid
            .expect("default Boneyard wave director has a Slumpgut recipe uid");
        let mut recipe = self
            .programs
            .as_ref()
            .expect("Slumpgut has retained recipe data")
            .slumpgut_recipe
            .clone();
        recipe.uid = uid;
        let mut intent = self.place_spawn(
            EnemyToken::Zombie,
            SpawnLocationPolicy::Anywhere,
            SpawnPositionPolicy::Light,
            context,
            self.state.wave_ordinal,
        );
        intent.authored_recipe = Some(recipe);
        intent.flanking = false;
        intent.pathfinding_mode = 2;
        intent.zombie_body_type = 1;
        self.state.slumpgut_phase = SlumpgutPhase::Retired;
        self.state.slumpgut_ticks_remaining = 0;
        vec![intent]
    }

    fn step_portals(&mut self, context: WaveTickContext<'_>) -> Vec<EnemySpawnIntent> {
        if self.state.portal_script_phase == PortalScriptPhase::Retired || self.programs.is_none() {
            return Vec::new();
        }
        if self.state.portal_script_phase == PortalScriptPhase::Idle {
            let phase = self
                .programs
                .as_ref()
                .expect("portal program is present")
                .portals
                .get(self.state.portal_phase_index);
            let Some(phase) = phase else {
                self.state.portal_script_phase = PortalScriptPhase::Retired;
                return Vec::new();
            };
            if self.state.wave_ordinal < phase.start_wave {
                return Vec::new();
            }
            self.state.portal_spawn_remaining = phase.spawn_count;
            if phase.name == "Deep Portal" {
                self.state.portal_script_phase = PortalScriptPhase::Intro;
                self.state.portal_ticks_remaining = 150;
                self.state.portal_timeline_paused = true;
                return Vec::new();
            }
            self.state.portal_script_phase = PortalScriptPhase::Spawning;
            self.state.portal_ticks_remaining = 0;
            return vec![self.emit_portal(context)];
        }
        if self.state.portal_ticks_remaining > 1 {
            self.state.portal_ticks_remaining -= 1;
            return Vec::new();
        }
        if matches!(
            self.state.portal_script_phase,
            PortalScriptPhase::Intro | PortalScriptPhase::Spawning
        ) {
            self.state.portal_script_phase = PortalScriptPhase::Spawning;
            self.state.portal_ticks_remaining = 0;
            return vec![self.emit_portal(context)];
        }
        if context.live_boss_count > 0 {
            self.state.portal_ticks_remaining = 200;
            return Vec::new();
        }
        self.state.portal_phase_index += 1;
        self.state.portal_script_phase = PortalScriptPhase::Idle;
        self.state.portal_ticks_remaining = 0;
        self.state.portal_timeline_paused = false;
        self.start_next_wave();
        Vec::new()
    }

    fn emit_portal(&mut self, context: WaveTickContext<'_>) -> EnemySpawnIntent {
        assert!(
            self.state.portal_spawn_remaining > 0,
            "Portal spawner has remaining authored births"
        );
        let phase = &self.programs.as_ref().expect("portal program is present").portals
            [self.state.portal_phase_index];
        let recipe = phase.recipe.clone();
        let position_policy = phase.placement_policy;
        let first_barrier = phase.name == "Deep Portal";
        let mut intent = self.place_spawn(
            EnemyToken::Portal,
            SpawnLocationPolicy::Anywhere,
            position_policy,
            context,
            self.state.wave_ordinal,
        );
        intent.authored_recipe = Some(recipe);
        intent.navigation_clearance = Some(25.0);
        intent.pathfinding_mode = 2;
        intent.placement_radius = Some(45.0);
        intent.reachability_radius = Some(25.0);
        self.state.portal_spawn_remaining -= 1;
        if self.state.portal_spawn_remaining > 0 {
            self.state.portal_script_phase = PortalScriptPhase::Spawning;
            self.state.portal_ticks_remaining = 25;
        } else if first_barrier {
            self.state.portal_script_phase = PortalScriptPhase::BossWait;
            self.state.portal_ticks_remaining = 100;
        } else {
            self.state.portal_phase_index += 1;
            self.state.portal_script_phase = PortalScriptPhase::Idle;
            self.state.portal_ticks_remaining = 0;
        }
        intent
    }

    fn step_ordinary_waves(&mut self, context: WaveTickContext<'_>) -> Vec<EnemySpawnIntent> {
        match self.state.phase {
            WavePhase::Dormant => {}
            WavePhase::Opening | WavePhase::Spawning => return self.step_spawner(context),
            WavePhase::OpeningThreshold => {
                if context.live_enemy_count < self.state.population_threshold {
                    self.begin_schedule_row(0);
                }
            }
            WavePhase::WaveThreshold => {
                if context.live_enemy_count < self.state.population_threshold {
                    self.state.interwave_delay_ticks = PAUSE_NODE_GAP_TICKS;
                    self.state.phase = WavePhase::WaveLullDelay;
                }
            }
            WavePhase::WaveLullDelay => {
                if self.state.interwave_delay_ticks > 1 {
                    self.state.interwave_delay_ticks -= 1;
                } else {
                    self.state.interwave_delay_ticks = 0;
                    self.state.phase = WavePhase::WaveLull;
                }
            }
            WavePhase::WaveLull => {
                if self.state.low_population_ticks > 1
                    || context.live_enemy_count < self.state.lull_threshold
                {
                    self.state.interwave_delay_ticks =
                        LULL_RELEASE_TO_NEXT_SPAWN_TICKS - WAVE_LABEL_TO_FIRST_SPAWN_TICKS;
                    self.state.next_schedule_index = None;
                    self.state.phase = WavePhase::Interwave;
                }
            }
            WavePhase::Interwave => {
                self.step_interwave();
            }
        }
        Vec::new()
    }

    fn step_spawner(&mut self, context: WaveTickContext<'_>) -> Vec<EnemySpawnIntent> {
        if self.state.spawn_countdown > 0.0 {
            self.state.spawn_countdown = (self.state.spawn_countdown - 1.0).max(0.0);
            if self.state.burst_started {
                self.state.burst_spread_ticks_remaining =
                    (self.state.burst_spread_ticks_remaining - 1.0).max(0.0);
            }
            self.state.spawn_delay_ticks = self.state.spawn_countdown.ceil() as u64;
        }
        if self.state.spawn_countdown > 0.0
            && (!self.state.burst_started || self.state.burst_spread_ticks_remaining > 0.0)
        {
            return Vec::new();
        }
        let mut births = Vec::new();
        while self.state.burst_spawn_remaining > 0 {
            let burst = &self.active_bursts
                [self.state.active_burst_index.expect("spawner has an active burst")];
            let entry_index = self.state.rng.draw_integer(burst.entries.len() as i32) as usize;
            let entry = &burst.entries[entry_index];
            let enemy_token = entry.enemy;
            let flags = entry.flags.clone();
            let location_policy = burst.location_policy;
            let position_policy = burst.position_policy;
            let mut intent = self.place_spawn(
                enemy_token,
                location_policy,
                position_policy,
                context,
                self.state.wave_ordinal,
            );
            intent.flags = flags;
            self.state.active_group_member_index = entry_index;
            self.state.burst_started = true;
            self.state.burst_spawn_remaining -= 1;
            self.state.pending_spawn_budget -= 1;
            self.state.spawn_countdown = 0.0;
            self.state.spawn_delay_ticks = 0;
            births.push(intent);
            if self.state.burst_spawn_remaining == 0 {
                self.advance_burst();
                if !matches!(self.state.phase, WavePhase::Opening | WavePhase::Spawning)
                    || self.state.spawn_countdown > 0.0
                {
                    return births;
                }
                continue;
            }
            if self.state.burst_spread_ticks_remaining > 0.0 {
                self.reset_spawner_interval();
                return births;
            }
        }
        self.finish_bursts();
        births
    }

    fn reset_spawner_interval(&mut self) {
        let burst = &self.active_bursts
            [self.state.active_burst_index.expect("spawner has an active burst")];
        let base_interval = self.state.burst_spread_ticks_remaining
            / self.state.burst_spawn_remaining.max(1) as f64;
        self.state.spawn_countdown = if burst.steady {
            base_interval
        } else {
            base_interval
                + f64::from(
                    self.state.rng.draw_integer((base_interval / 2.0).trunc().max(1.0) as i32),
                )
        };
        self.state.spawn_delay_ticks = self.state.spawn_countdown.ceil() as u64;
    }

    fn advance_burst(&mut self) {
        let current_index = self.state.active_burst_index.unwrap_or(0);
        let next_index = current_index + 1;
        let Some(next) = self.active_bursts.get(next_index) else {
            self.finish_bursts();
            return;
        };
        let countdown =
            self.active_bursts[current_index].after_delay_ticks + next.start_delay_ticks;
        self.state.active_burst_index = Some(next_index);
        self.state.active_group_index = Some(next.group_index);
        self.state.active_group_member_index = 0;
        self.state.burst_started = false;
        self.state.burst_spawn_remaining = next.count;
        self.state.burst_spread_ticks_remaining = next.spread_ticks;
        self.state.spawn_countdown = countdown as f64;
        self.state.spawn_delay_ticks = countdown;
    }

    fn finish_bursts(&mut self) {
        if !matches!(self.state.phase, WavePhase::Opening | WavePhase::Spawning) {
            return;
        }
        self.state.active_burst_index = None;
        self.state.active_group_index = None;
        self.state.burst_spawn_remaining = 0;
        self.state.burst_started = false;
        self.state.burst_spread_ticks_remaining = 0.0;
        self.state.phase = if self.state.phase == WavePhase::Opening {
            WavePhase::OpeningThreshold
        } else {
            WavePhase::WaveThreshold
        };
        self.state.spawn_countdown = 0.0;
        self.state.spawn_delay_ticks = 0;
    }

    fn select_next_schedule_row(&mut self) {
        let wave = &self.schedule[self.state.schedule_index];
        let edge = self.state.rng.draw_integer(wave.next.len() as i32) as usize;
        self.state.next_schedule_index =
            Some((self.state.schedule_index as i64 + i64::from(wave.next[edge])) as usize);
    }

    fn step_interwave(&mut self) {
        self.state.interwave_delay_ticks = self.state.interwave_delay_ticks.saturating_sub(1);
        if self.state.next_schedule_index.is_none() && self.state.interwave_delay_ticks <= 50 {
            self.select_next_schedule_row();
        }
        if self.state.interwave_delay_ticks == 0 {
            self.begin_schedule_row(
                self.state.next_schedule_index.unwrap_or(self.state.schedule_index),
            );
        }
    }

    fn start_next_wave(&mut self) {
        self.select_next_schedule_row();
        self.begin_schedule_row(
            self.state.next_schedule_index.unwrap_or(self.state.schedule_index),
        );
    }

    fn begin_schedule_row(&mut self, schedule_index: usize) {
        let section = self
            .compiled_schedule
            .get(schedule_index)
            .expect("wave schedule has its requested row")
            .clone();
        self.state.lull_threshold = section.lull_threshold;
        self.state.low_population_ticks = 0;
        self.state.population_threshold = section.release_threshold;
        self.state.schedule_index = schedule_index;
        self.state.wave_event_id += 1;
        self.state.wave_ordinal += 1;
        self.state.active_section = Some(WaveSection::Schedule { index: schedule_index });
        self.begin_bursts(section.bursts, WavePhase::Spawning);
    }

    fn begin_bursts(&mut self, bursts: Vec<CompiledSpawnBurst>, phase: WavePhase) {
        assert!(!bursts.is_empty(), "compiled wave section has spawn bursts");
        self.state.active_burst_index = Some(0);
        self.state.pending_spawn_budget = bursts.iter().map(|burst| burst.count).sum();
        self.active_bursts = bursts;
        let first = &self.active_bursts[0];
        self.state.active_group_index = Some(first.group_index);
        self.state.active_group_member_index = 0;
        self.state.burst_started = false;
        self.state.burst_spawn_remaining = first.count;
        self.state.burst_spread_ticks_remaining = first.spread_ticks;
        self.state.interwave_delay_ticks = 0;
        self.state.next_schedule_index = None;
        self.state.phase = phase;
        self.state.spawn_countdown = first.start_delay_ticks as f64;
        self.state.spawn_delay_ticks = first.start_delay_ticks;
    }

    fn place_spawn(
        &mut self,
        enemy_token: EnemyToken,
        location_policy: SpawnLocationPolicy,
        position_policy: SpawnPositionPolicy,
        context: WaveTickContext<'_>,
        wave_ordinal: u64,
    ) -> EnemySpawnIntent {
        let position =
            place_enemy(&mut self.state.rng, location_policy, context.players, context.bounds);
        let mut intent = EnemySpawnIntent::new(
            enemy_token,
            position,
            self.state.next_spawn_intent_id,
            context.tick,
            wave_ordinal,
        );
        intent.location_policy = location_policy;
        intent.position_policy = position_policy;
        self.state.next_spawn_intent_id += 1;
        intent
    }
}

fn count_births<'a>(
    mut context: WaveTickContext<'a>,
    births: &[EnemySpawnIntent],
) -> WaveTickContext<'a> {
    context.live_enemy_count += births.len();
    context.live_zombie_count +=
        births.iter().filter(|birth| birth.enemy_token == EnemyToken::Zombie).count();
    context.live_boss_count += births
        .iter()
        .filter(|birth| {
            birth
                .authored_recipe
                .as_ref()
                .is_some_and(|recipe| recipe.classification != EnemyClassification::Normal)
        })
        .count();
    context
}

fn place_enemy(
    rng: &mut NativeRng,
    policy: SpawnLocationPolicy,
    players: &[Vec2],
    bounds: BoneyardBounds,
) -> Vec2 {
    if policy == SpawnLocationPolicy::NearPlayer {
        let index = rng.draw_integer(players.len().max(1) as i32) as usize;
        let player = players
            .get(index)
            .copied()
            .unwrap_or(Vec2::new(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0));
        let angle = rng.draw_scaled_float(360.0, false) * std::f64::consts::PI / 180.0;
        return Vec2::new(
            fround(player.x + float::cos(angle) * 100.0),
            fround(player.y + float::sin(angle) * 100.0),
        );
    }
    let x = rng.draw_scaled_float(bounds.width, false);
    let y = rng.draw_scaled_float(bounds.height, false);
    Vec2::new(fround(bounds.x + x), fround(bounds.y + y))
}

fn validate_schedule(schedule: &[WaveDef]) {
    assert!(!schedule.is_empty(), "wave schedule must not be empty");
    for (index, wave) in schedule.iter().enumerate() {
        assert!(
            wave.spawn > 0 && wave.max_enemies > 0,
            "wave {index} has positive native spawn directives"
        );
        assert!(
            !wave.groups.is_empty() && wave.groups.iter().all(|group| !group.entries.is_empty()),
            "wave {index} has nonempty groups"
        );
        assert!(
            !wave.next.is_empty()
                && wave.next.iter().all(|offset| {
                    let next = index as i64 + i64::from(*offset);
                    next >= 0 && next < schedule.len() as i64
                }),
            "wave {index} has valid signed NEXT edges"
        );
        for range in [wave.spawn_delay, wave.wave_delay] {
            assert!(
                range[0] >= 0 && range[1] >= range[0],
                "wave {index} has ordered native delay ranges"
            );
        }
    }
}

/// Old native checkpoints had no retained graphs. This is a one-time migration
/// using their original compiler, not a fallback for missing Source save fields.
fn compile_legacy_schedule(seed: &str, schedule: &[WaveDef]) -> Vec<CompiledWaveSection> {
    let mut rng = NativeRng::new(i64::from(seed_from_text(&format!("{seed}:wave-compiler"))));
    let _ = compile_boneyard_opening(&mut rng);
    let mut compiled = Vec::with_capacity(schedule.len());
    for (index, wave) in schedule.iter().enumerate() {
        compiled.push(super::legacy_timeline::compile_legacy_wave_section(
            wave,
            index + 1,
            &mut rng,
        ));
    }
    compiled
}

fn compile_opening_for_seed(seed: &str) -> super::CompiledOpening {
    let mut rng = NativeRng::new(i64::from(seed_from_text(&format!("{seed}:wave-compiler"))));
    compile_boneyard_opening(&mut rng)
}
