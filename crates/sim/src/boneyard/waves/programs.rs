//! Generated-Boneyard survival scripts from `native-survival-boss-encounter.ts`,
//! `native-survival-skeleton-bosses.ts` and `native-spider-wave-program.ts`.

use crate::enemies::config::AuthoredEnemyRecipe;
use crate::enemies::{EnemyFlag, EnemyToken, SpawnPositionPolicy};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BossKind {
    Heartmonger,
    Faculty,
    Discorporeal,
}

impl BossKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Heartmonger => "heartmonger",
            Self::Faculty => "faculty",
            Self::Discorporeal => "discorporeal",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BossDefinition {
    pub kind: BossKind,
    pub wave_ordinal: u64,
    pub completion_skeletons: usize,
    pub recipes: Vec<AuthoredEnemyRecipe>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonBossDefinition {
    pub wave_ordinal: u64,
    pub recipe: AuthoredEnemyRecipe,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct PortalDefinition {
    pub frequency: u8,
    pub maximum_health: f64,
    pub name: String,
    pub recipe_uid: usize,
    pub placement_policy: SpawnPositionPolicy,
    pub script_uid: usize,
    pub spawn_count: usize,
    pub start_wave: u64,
    pub trigger_uid: usize,
    pub recipe: AuthoredEnemyRecipe,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub enum SpiderCommand {
    Pause,
    Unpause,
    Advance,
    SpawnOffscreen,
    EndRepeat,
    EndIf,
    Label,
    Repeat { count: usize },
    Spawn { flags: Vec<EnemyFlag> },
    Sleep { ticks: u64 },
    IfMonsters { next: usize },
    Jump { next: usize },
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct SpiderDefinition {
    pub name: String,
    pub trigger_uid: usize,
    pub script_uid: usize,
    pub start_wave: u64,
    pub commands: Vec<SpiderCommand>,
}

/// Exact per-source authored programs. This is generated data; execution below is Rust.
#[derive(Clone, Debug, PartialEq)]
pub struct SurvivalPrograms {
    pub source_sha256: String,
    pub bosses: Vec<BossDefinition>,
    pub ironmaw: SkeletonBossDefinition,
    pub foulshaft: SkeletonBossDefinition,
    pub slumpgut_recipe: AuthoredEnemyRecipe,
    pub spiders: Vec<SpiderDefinition>,
    pub portal_timeline_uid: Option<usize>,
    pub portals: Vec<PortalDefinition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BossPhase {
    Eligible,
    BossWait,
    CompletionDelay,
    Completion,
    Retired,
}

impl BossPhase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::BossWait => "boss-wait",
            Self::CompletionDelay => "completion-delay",
            Self::Completion => "completion",
            Self::Retired => "retired",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BossBirth {
    Boss,
    Skeleton,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BossStep {
    pub birth: Option<BossBirth>,
    pub release_wave: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BossEncounter {
    pub kind: BossKind,
    pub phase: BossPhase,
    pub skeletons_remaining: usize,
    pub ticks_remaining: u64,
}

impl BossEncounter {
    #[must_use]
    pub fn new(kind: BossKind) -> Self {
        Self { kind, phase: BossPhase::Eligible, skeletons_remaining: 0, ticks_remaining: 0 }
    }

    #[must_use]
    pub fn timeline_paused(&self) -> bool {
        !matches!(self.phase, BossPhase::Eligible | BossPhase::Retired)
    }

    pub fn step(
        &mut self,
        definition: &BossDefinition,
        wave_ordinal: u64,
        live_boss_count: usize,
    ) -> BossStep {
        let mut result = BossStep { birth: None, release_wave: false };
        match self.phase {
            BossPhase::Eligible => {
                if wave_ordinal == definition.wave_ordinal {
                    self.phase = BossPhase::BossWait;
                    self.skeletons_remaining = definition.completion_skeletons;
                    self.ticks_remaining = 100;
                    result.birth = Some(BossBirth::Boss);
                }
            }
            BossPhase::Retired => {}
            _ if self.ticks_remaining > 1 => self.ticks_remaining -= 1,
            BossPhase::BossWait => {
                self.phase = if live_boss_count > 0 {
                    BossPhase::BossWait
                } else {
                    BossPhase::CompletionDelay
                };
                self.ticks_remaining = if live_boss_count > 0 { 200 } else { 500 };
            }
            BossPhase::CompletionDelay => {
                self.phase = BossPhase::Completion;
                self.ticks_remaining = 75;
            }
            BossPhase::Completion => {
                self.skeletons_remaining -= 1;
                result.birth = Some(BossBirth::Skeleton);
                result.release_wave = self.skeletons_remaining == 0;
                self.phase =
                    if result.release_wave { BossPhase::Retired } else { BossPhase::Completion };
                self.ticks_remaining = if result.release_wave { 0 } else { 75 };
            }
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum FoulshaftPhase {
    #[default]
    Eligible,
    Reinforcements,
    BossWait,
    Retired,
}

impl FoulshaftPhase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::Reinforcements => "reinforcements",
            Self::BossWait => "boss-wait",
            Self::Retired => "retired",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SkeletonBossState {
    pub foulshaft_archers_remaining: usize,
    pub foulshaft_phase: FoulshaftPhase,
    pub foulshaft_ticks_remaining: u64,
    pub ironmaw_spawned: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonBirth {
    pub enemy_token: EnemyToken,
    pub flags: Vec<EnemyFlag>,
    pub authored_recipe: Option<AuthoredEnemyRecipe>,
    pub archer_strafing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonStep {
    pub births: Vec<SkeletonBirth>,
    pub release_wave: bool,
}

impl SkeletonBossState {
    #[must_use]
    pub fn timeline_paused(&self) -> bool {
        matches!(self.foulshaft_phase, FoulshaftPhase::Reinforcements | FoulshaftPhase::BossWait)
    }

    pub fn step(
        &mut self,
        ironmaw: &SkeletonBossDefinition,
        foulshaft: &SkeletonBossDefinition,
        wave_ordinal: u64,
        live_boss_count: usize,
    ) -> SkeletonStep {
        let mut result = SkeletonStep { births: Vec::new(), release_wave: false };
        if !self.ironmaw_spawned && wave_ordinal == ironmaw.wave_ordinal {
            result.births.push(SkeletonBirth {
                enemy_token: EnemyToken::Skeleton,
                flags: Vec::new(),
                authored_recipe: Some(ironmaw.recipe.clone()),
                archer_strafing: false,
            });
            self.ironmaw_spawned = true;
        }
        if self.foulshaft_phase == FoulshaftPhase::Eligible {
            if wave_ordinal == foulshaft.wave_ordinal {
                result.births.push(SkeletonBirth {
                    enemy_token: EnemyToken::SkeletonArcher,
                    flags: Vec::new(),
                    authored_recipe: Some(foulshaft.recipe.clone()),
                    archer_strafing: false,
                });
                result.births.push(opening_archer());
                self.foulshaft_archers_remaining = 9;
                self.foulshaft_phase = FoulshaftPhase::Reinforcements;
                self.foulshaft_ticks_remaining = 100;
            }
            return result;
        }
        if self.foulshaft_phase == FoulshaftPhase::Retired {
            return result;
        }
        if self.foulshaft_ticks_remaining > 1 {
            self.foulshaft_ticks_remaining -= 1;
            return result;
        }
        if self.foulshaft_phase == FoulshaftPhase::Reinforcements
            && self.foulshaft_archers_remaining > 0
        {
            result.births.push(opening_archer());
            self.foulshaft_archers_remaining -= 1;
            self.foulshaft_ticks_remaining = 100;
            return result;
        }
        if live_boss_count > 0 || result.births.iter().any(|birth| birth.authored_recipe.is_some())
        {
            for enemy_token in [EnemyToken::SkeletonArcher, EnemyToken::Skeleton] {
                result.births.push(SkeletonBirth {
                    enemy_token,
                    flags: vec![EnemyFlag::HpUp],
                    authored_recipe: None,
                    archer_strafing: false,
                });
            }
            self.foulshaft_phase = FoulshaftPhase::BossWait;
            self.foulshaft_ticks_remaining = 200;
            return result;
        }
        self.foulshaft_phase = FoulshaftPhase::Retired;
        self.foulshaft_ticks_remaining = 0;
        result.release_wave = true;
        result
    }
}

fn opening_archer() -> SkeletonBirth {
    SkeletonBirth {
        enemy_token: EnemyToken::SkeletonArcher,
        flags: vec![EnemyFlag::Hooded, EnemyFlag::HpUp],
        authored_recipe: None,
        archer_strafing: true,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SpiderState {
    pub active: bool,
    pub phase_index: usize,
    pub cursor: usize,
    pub repeat_start: usize,
    pub repeat_remaining: usize,
    pub sleep_ticks_remaining: u64,
    pub timeline_paused: bool,
    pub position_policy: SpawnPositionPolicy,
}

impl Default for SpiderState {
    fn default() -> Self {
        Self {
            active: false,
            phase_index: 0,
            cursor: 0,
            repeat_start: 0,
            repeat_remaining: 0,
            sleep_ticks_remaining: 0,
            timeline_paused: false,
            position_policy: SpawnPositionPolicy::Dark,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpiderBirth {
    pub flags: Vec<EnemyFlag>,
    pub position_policy: SpawnPositionPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpiderStep {
    pub advance_wave: bool,
    pub births: Vec<SpiderBirth>,
}

impl SpiderState {
    /// ScriptCall executes at most ten commands, stopping early on sleep. Spawns affect
    /// later IF MONSTERS commands in this same tick before the enemy store receives them.
    pub fn step(
        &mut self,
        definitions: &[SpiderDefinition],
        wave_ordinal: u64,
        monster_count: usize,
    ) -> SpiderStep {
        let mut result = SpiderStep { advance_wave: false, births: Vec::new() };
        let Some(definition) = definitions.get(self.phase_index) else { return result };
        if !self.active && wave_ordinal < definition.start_wave {
            return result;
        }
        self.active = true;
        if self.sleep_ticks_remaining > 0 {
            self.sleep_ticks_remaining -= 1;
            if self.sleep_ticks_remaining > 0 {
                return result;
            }
        }
        for _ in 0..10 {
            let Some(command) = definition.commands.get(self.cursor) else {
                self.active = false;
                self.phase_index += 1;
                self.cursor = 0;
                return result;
            };
            self.cursor += 1;
            match command {
                SpiderCommand::Pause => self.timeline_paused = true,
                SpiderCommand::Unpause => self.timeline_paused = false,
                SpiderCommand::Advance => result.advance_wave = true,
                SpiderCommand::SpawnOffscreen => {
                    self.position_policy = SpawnPositionPolicy::Offscreen
                }
                SpiderCommand::Repeat { count } => {
                    self.repeat_start = self.cursor;
                    self.repeat_remaining = *count;
                }
                SpiderCommand::EndRepeat => {
                    self.repeat_remaining -= 1;
                    if self.repeat_remaining > 0 {
                        self.cursor = self.repeat_start;
                    }
                }
                SpiderCommand::Spawn { flags } => result.births.push(SpiderBirth {
                    flags: flags.clone(),
                    position_policy: self.position_policy,
                }),
                SpiderCommand::Sleep { ticks } => self.sleep_ticks_remaining = *ticks,
                SpiderCommand::IfMonsters { next } => {
                    if monster_count + result.births.len() == 0 {
                        self.cursor = *next;
                    }
                }
                SpiderCommand::Jump { next } => self.cursor = *next,
                SpiderCommand::EndIf | SpiderCommand::Label => {}
            }
            if self.sleep_ticks_remaining > 0 {
                return result;
            }
        }
        result
    }
}
