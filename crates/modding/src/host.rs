//! The live native prepared owner from `host/prepared-mod-host.ts`.
//! One real VM/session and one content-runtime collection borrow the canonical
//! game per operation. No executable state enters a game checkpoint.

mod lifecycle;
mod projection;
mod roster;
mod services;

pub use projection::PreparedModRuntimeProjection;
pub use services::PreparedModUiActionInput;
pub use solomon_sim::modding::events::WebLuaDerivedEvent as PreparedModDerivedEvent;

use solomon_sim::enemies::EnemySpawnIntent;
use solomon_sim::hub::interaction::BoastFailureProducer;
use solomon_sim::modding::definition::WebLuaScopeKind;
use solomon_sim::modding::inventory::GameSimulationModConsumption;
use solomon_sim::modding::runtime::*;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::session::{GameSession, GameSessionInput};
use std::collections::BTreeMap;

use crate::host_runtime::{PreparedModRuntime, filter_equipment_modifiers};
use crate::host_transaction::HostIntentAdapter;
use crate::package::MaterializedWebSessionContent;
use crate::player_control::{ModPlayerControl, uses_player_control_rule};
use crate::session::{PreparedModSession, PreparedModSource};

pub struct PreparedModHost {
    manifest_sha256: String,
    runtime: PreparedModRuntime,
    session: PreparedModSession,
    controls: ModPlayerControl,
    has_player_controls: bool,
    active_boneyard_content_id: Option<String>,
    enemy_spawns: Vec<EnemySpawnIntent>,
    presentation: Vec<PreparedModPresentationIntent>,
    reported_enemy_health: BTreeMap<u64, f64>,
    logs: Vec<String>,
    closed: bool,
}

pub struct PreparedModCastInput<'a> {
    pub content_id: &'a str,
    pub context: LuaObject,
    pub player_id: &'a str,
    pub request_id: u64,
}

impl PreparedModHost {
    pub fn prepare(
        package: &MaterializedWebSessionContent,
        game: &mut GameSession,
    ) -> Result<Self, String> {
        let runtime = PreparedModRuntime::prepare(package)?;
        let sources = sources(package);
        let session = PreparedModSession::new(&sources)?;
        Self::initialize(runtime, session, package.manifest.manifest_sha256.clone(), game)
    }
    pub fn with_clock(
        package: &MaterializedWebSessionContent,
        game: &mut GameSession,
        now: impl FnMut() -> f64 + 'static,
    ) -> Result<Self, String> {
        let runtime = PreparedModRuntime::prepare(package)?;
        let sources = sources(package);
        let session = PreparedModSession::with_clock(&sources, now)?;
        Self::initialize(runtime, session, package.manifest.manifest_sha256.clone(), game)
    }
    fn initialize(
        runtime: PreparedModRuntime,
        session: PreparedModSession,
        manifest_sha256: String,
        game: &mut GameSession,
    ) -> Result<Self, String> {
        let mut has_player_controls = false;
        for package in session.catalog()? {
            if package
                .reducers
                .iter()
                .any(|reducer| reducer.on.iter().any(|event| event == "player.control"))
                || package.rules.iter().any(|rule| {
                    uses_player_control_rule(
                        &crate::json::lua_value(rule).expect("compiled rules are serializable"),
                    )
                })
            {
                has_player_controls = true;
                break;
            }
        }
        let mut host = Self {
            manifest_sha256,
            runtime,
            session,
            controls: ModPlayerControl::default(),
            has_player_controls,
            active_boneyard_content_id: None,
            enemy_spawns: Vec::new(),
            presentation: Vec::new(),
            reported_enemy_health: BTreeMap::new(),
            logs: Vec::new(),
            closed: false,
        };
        let initialized = host.step_raw(
            &PreparedModStepInput {
                events: vec![PreparedModEvent {
                    context: LuaObject::new(),
                    event: "session.started".into(),
                    payload: object_value([("event", LuaValue::String("session.started".into()))]),
                    scope: ModStateScope { id: "session".into(), kind: WebLuaScopeKind::Session },
                }],
                tick: game.ticks,
            },
            game,
        )?;
        if !initialized.accepted {
            return Err(if initialized.errors.is_empty() {
                "Web Lua session initialization failed".into()
            } else {
                initialized.errors.join("; ")
            });
        }
        Ok(host)
    }
    pub fn runtime(&self) -> &PreparedModRuntime {
        &self.runtime
    }
    pub fn checkpoint(&self) -> Result<PreparedModHostCheckpoint, String> {
        self.require_open()?;
        self.runtime.checkpoint(&self.session)
    }
    pub fn save_state(&self) -> Result<LuaObject, String> {
        self.require_open()?;
        solomon_assets::prepared_mod_save::encode_prepared_mod_save_state(
            self.session.catalog()?,
            &self.checkpoint()?,
        )
    }
    pub fn restore_save_state(&mut self, state: &LuaObject) -> Result<(), String> {
        self.require_open()?;
        let checkpoint = solomon_assets::prepared_mod_save::decode_prepared_mod_save_state(
            self.session.catalog()?,
            state,
        )?;
        self.restore(&checkpoint)
    }
    pub fn restore(&mut self, checkpoint: &PreparedModHostCheckpoint) -> Result<(), String> {
        self.require_open()?;
        self.runtime.restore(&mut self.session, checkpoint)?;
        self.reported_enemy_health.clear();
        self.controls.clear();
        Ok(())
    }
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.enemy_spawns.clear();
        self.presentation.clear();
        self.reported_enemy_health.clear();
        self.controls.clear();
        self.active_boneyard_content_id = None;
        self.session.close();
    }
    pub fn drain_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }
    pub fn drain_enemy_spawns(&mut self) -> Result<Vec<EnemySpawnIntent>, String> {
        self.require_open()?;
        Ok(std::mem::take(&mut self.enemy_spawns))
    }
    pub fn drain_presentation(&mut self) -> Result<Vec<PreparedModPresentationIntent>, String> {
        self.require_open()?;
        Ok(std::mem::take(&mut self.presentation))
    }
    pub fn active_scene(&self, owner_id: &str) -> Result<Option<&ActiveModScene>, String> {
        self.require_open()?;
        Ok(self.runtime.scenes.active(owner_id))
    }
    pub fn bind_mod_quickbar(
        &mut self,
        player_id: &str,
        slot: u32,
        content_id: Option<&str>,
    ) -> Result<(), String> {
        self.require_open()?;
        self.runtime.skills.bind(player_id, slot, content_id)
    }
    pub fn consume(
        &mut self,
        consumption: &GameSimulationModConsumption,
        game: &mut GameSession,
    ) -> Result<PreparedModStepResult, String> {
        self.require_open()?;
        let content = &consumption.content;
        if self.runtime.content.potion(&content.content_id).is_none() {
            return Err(format!("consumed mod content is unavailable: {}", content.content_id));
        }
        self.act_raw(
            &PreparedModActionInput {
                action: "content.use".into(),
                context: object([
                    ("content_id", LuaValue::String(content.content_id.clone())),
                    ("participant_id", LuaValue::String(consumption.player_id.clone())),
                    ("use_id", LuaValue::Number(consumption.use_id as f64)),
                ]),
                payload: object_value([
                    ("content_id", LuaValue::String(content.content_id.clone())),
                    ("duration_ms", LuaValue::Number(content.duration_ms)),
                    ("participant_id", LuaValue::String(consumption.player_id.clone())),
                    ("use_id", LuaValue::Number(consumption.use_id as f64)),
                ]),
                request_id: consumption.use_id,
                scope: participant_scope(game, &consumption.player_id),
                tick: consumption.tick,
            },
            game,
        )
    }
    pub fn cast(
        &mut self,
        input: PreparedModCastInput<'_>,
        game: &mut GameSession,
    ) -> Result<PreparedModStepResult, String> {
        self.require_open()?;
        if !self.runtime.skills.spell_unlocked(input.player_id, input.content_id)? {
            return Err("mod spell has not been unlocked".into());
        }
        let source = game.clone();
        let checkpoint = self.runtime.spells.checkpoint();
        let spell =
            self.runtime.content.spell(input.content_id).ok_or("mod spell is unavailable")?;
        let mana = game
            .participants
            .get(input.player_id)
            .ok_or("mod spell player is unavailable")?
            .player
            .combat
            .current_mana;
        let status_delta =
            self.runtime.statuses.filter_mana(input.player_id, -spell.mana, game.ticks)?;
        let cost = if status_delta >= 0.0 {
            0.0
        } else {
            filter_equipment_modifiers(
                &game
                    .participants
                    .get(input.player_id)
                    .expect("participant exists")
                    .player
                    .economy
                    .equipment,
                "mana_spend",
                self.runtime.skills.filter(input.player_id, "mana_spend", -status_delta)?,
            )
        };
        let secondary = spell.slot == solomon_sim::modding::content::ModSpellSlot::Secondary;
        if cost > mana {
            game.fail_boast_with_extensions(
                input.player_id,
                BoastFailureProducer::ManaUnderflow,
                Some(&self.runtime),
            );
        }
        let admission = self.runtime.spells.admit(
            input.player_id,
            input.content_id,
            game.ticks,
            mana,
            Some(cost),
        )?;
        // Source debits from the original state after its separate underflow
        // boast producer; retain that exact assignment order.
        *game = source.clone();
        game.participants
            .get_mut(input.player_id)
            .unwrap()
            .player
            .combat
            .set_mana(mana - admission.mana_cost);
        let mut context = input.context;
        context.insert("participant_id".into(), LuaValue::String(input.player_id.into()));
        let result = self.act_raw(
            &PreparedModActionInput {
                action: "content.cast".into(),
                context,
                payload: object_value([
                    ("content_id", LuaValue::String(input.content_id.into())),
                    ("participant_id", LuaValue::String(input.player_id.into())),
                ]),
                request_id: input.request_id,
                scope: participant_scope(&source, input.player_id),
                tick: source.ticks,
            },
            game,
        );
        match result {
            Ok(result) if result.accepted => {
                if secondary {
                    game.fail_boast_with_extensions(
                        input.player_id,
                        BoastFailureProducer::SecondaryCast,
                        Some(&self.runtime),
                    );
                }
                Ok(result)
            }
            Ok(result) => {
                *game = source;
                self.runtime.spells.restore(checkpoint)?;
                Ok(result)
            }
            Err(error) => {
                *game = source;
                self.runtime.spells.restore(checkpoint)?;
                Err(error)
            }
        }
    }
    pub fn apply_player_controls(
        &mut self,
        inputs: &mut BTreeMap<String, GameSessionInput>,
        now_ms: f64,
        game: &mut GameSession,
    ) -> Result<bool, String> {
        self.require_open()?;
        if !self.has_player_controls {
            return Ok(false);
        }
        let had_barrier = game.level_up_barrier.is_some();
        for id in self.controls.begin_frame(game, inputs, now_ms)? {
            let payload = self.controls.observe(game, &id)?;
            let result = self.step_raw(
                &PreparedModStepInput {
                    events: vec![PreparedModEvent {
                        context: object([
                            ("event", LuaValue::String("player.control".into())),
                            ("participant_id", LuaValue::String(id.clone())),
                        ]),
                        event: "player.control".into(),
                        payload: LuaValue::Object(payload),
                        scope: ModStateScope {
                            id: format!("{id}:{}", game.run.run_id.as_deref().unwrap_or("null")),
                            kind: WebLuaScopeKind::ParticipantRun,
                        },
                    }],
                    tick: game.ticks,
                },
                game,
            )?;
            if !result.accepted || !result.errors.is_empty() || result.budget_exceeded {
                self.controls.release(&id);
            }
        }
        self.controls.apply(inputs);
        Ok(had_barrier && game.level_up_barrier.is_none())
    }
    fn step_raw(
        &mut self,
        input: &PreparedModStepInput,
        game: &mut GameSession,
    ) -> Result<PreparedModStepResult, String> {
        let mut adapter = HostIntentAdapter {
            game,
            runtime: &mut self.runtime,
            controls: &mut self.controls,
            spawns: &mut self.enemy_spawns,
            presentation: &mut self.presentation,
        };
        let result = self.session.step(input, &mut adapter)?;
        self.logs.extend(result.errors.iter().cloned());
        Ok(result)
    }
    fn act_raw(
        &mut self,
        input: &PreparedModActionInput,
        game: &mut GameSession,
    ) -> Result<PreparedModStepResult, String> {
        let mut adapter = HostIntentAdapter {
            game,
            runtime: &mut self.runtime,
            controls: &mut self.controls,
            spawns: &mut self.enemy_spawns,
            presentation: &mut self.presentation,
        };
        let result = self.session.act(input, &mut adapter)?;
        self.logs.extend(result.errors.iter().cloned());
        Ok(result)
    }
    fn require_open(&self) -> Result<(), String> {
        if self.closed { Err("prepared mod host is closed".into()) } else { Ok(()) }
    }
}

fn sources(package: &MaterializedWebSessionContent) -> Vec<PreparedModSource> {
    package
        .mod_sources
        .iter()
        .zip(&package.compiled_mods)
        .map(|(source, compiled)| PreparedModSource {
            compiled: compiled.clone(),
            entry_script: source.entry_script.clone(),
            entry_script_path: "scripts/main.lua".into(),
            identity: source.identity.clone(),
        })
        .collect()
}
fn participant_scope(game: &GameSession, id: &str) -> ModStateScope {
    ModStateScope {
        id: format!("{id}:{}", game.run.run_id.as_deref().unwrap_or("profile")),
        kind: WebLuaScopeKind::ParticipantRun,
    }
}
fn object<const N: usize>(values: [(&str, LuaValue); N]) -> LuaObject {
    let mut fields = LuaObject::new();
    for (key, value) in values {
        fields.insert(key.into(), value);
    }
    fields
}
fn object_value<const N: usize>(values: [(&str, LuaValue); N]) -> LuaValue {
    LuaValue::Object(object(values))
}
