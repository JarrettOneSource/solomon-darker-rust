//! Lua controller leases and observations from `host/mod-player-control.ts`.
//! Entrance guidance reuses the canonical Source navigator; observations borrow
//! real Session rows and never advance a player, enemy, RNG or clock.

use solomon_sim::boneyard::loot::LootActorRead;
use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::belt::{NativeBeltEntry, PLAYER_BELT_SLOT_COUNT};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::loot::{NativeLootDropKind, NativeOrbKind};
use solomon_sim::ml_bot::{BoneyardEntranceNavigator, MlBotEntranceNavigationError};
use solomon_sim::modding::definition::WebLuaScopeKind;
use solomon_sim::modding::extensions::GameSimulationExtensions;
use solomon_sim::modding::runtime::ModIntent;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::profile::{NATIVE_SKILL_CATALOG, PlayerSkillChoice, native_skill_category};
use solomon_sim::session::{GameSession, GameSessionInput};

use crate::intents::ModIntentExecutionContext;

#[derive(Clone)]
pub struct ModControlLease {
    pub expires_at_ms: f64,
    pub input: GameSessionInput,
    pub mod_id: String,
    pub owner: String,
}

#[derive(Default)]
pub struct ModPlayerControl {
    entrances: BTreeMap<String, BoneyardEntranceNavigator>,
    leases: Vec<(String, ModControlLease)>,
    next_decision_at_ms: f64,
    now_ms: f64,
    run_id: Option<String>,
}

impl ModPlayerControl {
    pub fn begin_frame(
        &mut self,
        game: &GameSession,
        inputs: &BTreeMap<String, GameSessionInput>,
        now_ms: f64,
    ) -> Result<Vec<String>, String> {
        if !now_ms.is_finite() {
            return Err("player control time must be finite".into());
        }
        self.now_ms = now_ms;
        let run_id = if game.boneyard.is_some() && game.run.phase == GameRunPhase::Active {
            game.run.run_id.clone()
        } else {
            None
        };
        if run_id != self.run_id {
            self.clear();
            self.run_id = run_id;
        }
        let mut players = Vec::new();
        if self.run_id.is_some() {
            for id in &game.participant_order {
                if inputs.contains_key(id)
                    && game
                        .participants
                        .get(id)
                        .expect("participant exists")
                        .player
                        .combat
                        .life_state
                        == PlayerLifeState::Alive
                {
                    players.push(id.clone());
                }
            }
        }
        self.leases.retain(|(id, lease)| players.contains(id) && now_ms < lease.expires_at_ms);
        self.entrances.retain(|id, _| players.contains(id));
        if now_ms < self.next_decision_at_ms {
            return Ok(Vec::new());
        }
        self.next_decision_at_ms = now_ms + 100.0;
        Ok(players)
    }

    pub fn apply(&self, inputs: &mut BTreeMap<String, GameSessionInput>) {
        for (id, lease) in &self.leases {
            if let Some(human) = inputs.get_mut(id) {
                *human = GameSessionInput { viewport: human.viewport, ..lease.input };
            }
        }
    }

    pub fn observe(&mut self, game: &GameSession, player_id: &str) -> Result<LuaObject, String> {
        let entrance = self
            .entrances
            .entry(player_id.into())
            .or_default()
            .input(game, player_id)
            .map_err(entrance_error)?;
        let mut observation = mod_player_control_observation(game, player_id)?;
        let entry = entrance.map_or(LuaValue::Null, |input| {
            object_value([("movement", point_value(input.movement))])
        });
        observation.insert("entry".into(), entry);
        Ok(observation)
    }

    pub fn accept(
        &mut self,
        game: &GameSession,
        intent: &ModIntent,
        context: &ModIntentExecutionContext,
    ) -> Result<(), String> {
        let player_id = control_participant(game, context)?;
        let previous = self.leases.iter().position(|(id, _)| id == player_id);
        if previous.is_some_and(|index| {
            self.leases[index].1.mod_id != intent.mod_id
                || self.leases[index].1.owner != intent.owner
        }) {
            return Err("player input is already owned by another Lua controller".into());
        }
        if intent.fields.get("release") == Some(&LuaValue::Boolean(true)) {
            only_fields(&intent.fields, &["release"])?;
            self.release(player_id);
            return Ok(());
        }
        only_fields(&intent.fields, &["aim", "movement", "primary", "quickbar"])?;
        let input = decode_mod_player_input(&intent.fields)?;
        let lease = ModControlLease {
            expires_at_ms: self.now_ms + 250.0,
            input,
            mod_id: intent.mod_id.clone(),
            owner: intent.owner.clone(),
        };
        if let Some(index) = previous {
            self.leases[index].1 = lease;
        } else {
            self.leases.push((player_id.into(), lease));
        }
        Ok(())
    }

    pub fn release(&mut self, player_id: &str) {
        self.leases.retain(|(id, _)| id != player_id);
    }
    pub fn checkpoint(&self) -> Vec<(String, ModControlLease)> {
        self.leases.clone()
    }
    pub fn restore(&mut self, checkpoint: Vec<(String, ModControlLease)>) {
        self.leases = checkpoint;
    }
    pub fn clear(&mut self) {
        self.leases.clear();
        self.entrances.clear();
        self.next_decision_at_ms = 0.0;
        self.run_id = None;
    }
}

pub fn select_mod_player_skill(
    game: &mut GameSession,
    fields: &LuaObject,
    context: &ModIntentExecutionContext,
    extensions: Option<&dyn GameSimulationExtensions>,
) -> Result<(), String> {
    only_fields(fields, &["choice_index", "offer_sequence", "skill_id"])?;
    let id = control_participant(game, context)?.to_owned();
    let choice = PlayerSkillChoice {
        choice_index: usize::try_from(control_integer(fields.get("choice_index"), "choice_index")?)
            .map_err(|_| "Lua skill choice does not match the current player offer")?,
        offer_sequence: control_integer(fields.get("offer_sequence"), "offer_sequence")?,
        skill_id: usize::try_from(control_integer(fields.get("skill_id"), "skill_id")?)
            .map_err(|_| "Lua skill choice does not match the current player offer")?,
    };
    if !game.choose_skill_with_extensions(&id, choice, extensions) {
        return Err("Lua skill choice does not match the current player offer".into());
    }
    Ok(())
}

pub fn uses_player_control_rule(value: &LuaValue) -> bool {
    match value {
        LuaValue::Object(fields) => {
            if fields.get("operation").and_then(LuaValue::as_str) == Some("rules.on")
                && fields
                    .get("fields")
                    .and_then(LuaValue::as_object)
                    .and_then(|fields| fields.get("event"))
                    .and_then(LuaValue::as_str)
                    == Some("player.control")
            {
                return true;
            }
            fields.iter().any(|(_, value)| uses_player_control_rule(value))
        }
        LuaValue::Array(values) => values.iter().any(uses_player_control_rule),
        _ => false,
    }
}

pub fn mod_player_control_observation(
    game: &GameSession,
    player_id: &str,
) -> Result<LuaObject, String> {
    let world = game.boneyard.as_ref().ok_or("player control requires a Boneyard")?;
    let player =
        game.participants.get(player_id).ok_or("player control requires an existing player")?;
    let character = &player.wizard;
    let profile = &player.player;
    let secondary = game.secondary.players.get(player_id);
    let bounds = world
        .arena_transition
        .as_ref()
        .map_or(world.loaded.scene.bounds, |transition| transition.combat_bounds);
    let mut enemies: Vec<_> = world
        .enemies
        .actors
        .iter()
        .filter(|enemy| {
            enemy.current_health > 0.0
                && enemy.life_state != solomon_sim::enemies::model::EnemyLifeState::Dying
        })
        .collect();
    enemies.sort_by(|left, right| {
        distance_squared(left.position, character.position)
            .total_cmp(&distance_squared(right.position, character.position))
            .then(left.id.cmp(&right.id))
    });
    let mut loot: Vec<_> = world.loot.actors.iter().collect();
    loot.sort_by(|left, right| {
        distance_squared(left.position, character.position)
            .total_cmp(&distance_squared(right.position, character.position))
            .then(left.id.cmp(&right.id))
    });
    let mut abilities = Vec::new();
    for (slot, entry) in profile.belt.entries.iter().enumerate() {
        let Some(NativeBeltEntry::Skill { skill_id }) = entry else {
            continue;
        };
        let skill_id = *skill_id;
        if native_skill_category(skill_id) != Some(2)
            || profile.book.permanent_ranks[skill_id] == 0
            || profile.book.effective_ranks[skill_id] == 0
        {
            continue;
        }
        let active = secondary.is_some_and(|state| match skill_id {
            12 => state.planewalker_ticks_remaining > 0,
            23 => state.firewalker,
            78 => state.mindstar,
            79 => state.regenerate,
            _ => false,
        });
        abilities.push(object_value([
            ("slot", LuaValue::Number(slot as f64)),
            ("skill_id", LuaValue::Number(skill_id as f64)),
            (
                "cooldown_ticks",
                LuaValue::Number(
                    secondary.map_or(0.0, |state| state.cooldown_ticks_by_skill[skill_id]),
                ),
            ),
            ("active", LuaValue::Boolean(active)),
        ]));
    }
    let offer = profile.progression.pending_offer.as_ref().map_or(LuaValue::Null, |offer| {
        object_value([
            ("sequence", LuaValue::Number(offer.sequence as f64)),
            (
                "options",
                LuaValue::Array(
                    offer
                        .options
                        .iter()
                        .enumerate()
                        .map(|(index, option)| {
                            object_value([
                                ("choice_index", LuaValue::Number(index as f64)),
                                ("skill_id", LuaValue::Number(option.skill_id as f64)),
                                ("rank", LuaValue::Number(f64::from(option.target_rank))),
                                (
                                    "name",
                                    LuaValue::String(
                                        NATIVE_SKILL_CATALOG
                                            .get(option.skill_id)
                                            .map_or("", |definition| definition.name)
                                            .into(),
                                    ),
                                ),
                                (
                                    "category",
                                    native_skill_category(option.skill_id)
                                        .map_or(LuaValue::Null, |value| {
                                            LuaValue::Number(f64::from(value))
                                        }),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    });
    let player_value = object_value([
        ("id", LuaValue::String(player_id.into())),
        ("position", point_value(character.position)),
        ("health", LuaValue::Number(profile.combat.current_health)),
        ("maximum_health", LuaValue::Number(profile.combat.maximum_health)),
        ("mana", LuaValue::Number(profile.combat.current_mana)),
        ("maximum_mana", LuaValue::Number(profile.combat.maximum_mana)),
        ("level", LuaValue::Number(f64::from(profile.progression.level))),
        ("primary_skill_id", LuaValue::Number(profile.book.primary_skill_id as f64)),
        (
            "secondary_ready",
            LuaValue::Boolean(
                profile.combat.can_cast()
                    && profile.progression.pending_offer.is_none()
                    && secondary.is_none_or(|state| {
                        state.cast_action.is_none()
                            && state.cast_spin_ticks_remaining == 0
                            && state.global_cooldown_ticks <= 0.0
                    }),
            ),
        ),
        (
            "held_quickbar",
            secondary
                .and_then(|state| state.held_slot)
                .map_or(LuaValue::Null, |value| LuaValue::Number(value as f64)),
        ),
        ("secondary_abilities", LuaValue::Array(abilities)),
        ("offer", offer),
    ]);
    Ok(object([
        (
            "bounds",
            object_value([
                ("x", LuaValue::Number(bounds.x)),
                ("y", LuaValue::Number(bounds.y)),
                ("w", LuaValue::Number(bounds.width)),
                ("h", LuaValue::Number(bounds.height)),
            ]),
        ),
        (
            "enemies",
            LuaValue::Array(
                enemies
                    .into_iter()
                    .take(64)
                    .map(|enemy| {
                        object_value([
                            ("id", LuaValue::Number(enemy.id as f64)),
                            ("health", LuaValue::Number(enemy.current_health)),
                            ("maximum_health", LuaValue::Number(enemy.config.maximum_health)),
                            ("position", point_value(enemy.position)),
                            ("kind", LuaValue::String(enemy.config.enemy_token.id().into())),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("enemy_count", LuaValue::Number(world.enemies.actors.len() as f64)),
        (
            "loot",
            LuaValue::Array(
                loot.into_iter()
                    .take(64)
                    .map(|actor| {
                        object_value([
                            ("id", LuaValue::Number(actor.id as f64)),
                            (
                                "kind",
                                LuaValue::String(
                                    match actor.kind {
                                        NativeLootDropKind::Orb => "orb",
                                        NativeLootDropKind::Gold => "gold",
                                        NativeLootDropKind::Sack => "sack",
                                        NativeLootDropKind::Bonus => "bonus",
                                    }
                                    .into(),
                                ),
                            ),
                            (
                                "orb_kind",
                                actor.reward().orb_kind.map_or(LuaValue::Null, |kind| {
                                    LuaValue::String(
                                        match kind {
                                            NativeOrbKind::Health => "health",
                                            NativeOrbKind::Mana => "mana",
                                        }
                                        .into(),
                                    )
                                }),
                            ),
                            ("position", point_value(actor.position)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("player", player_value),
        (
            "run_id",
            game.run.run_id.as_ref().map_or(LuaValue::Null, |id| LuaValue::String(id.clone())),
        ),
        (
            "wave",
            LuaValue::Number(
                world.waves.as_ref().map_or(0, |waves| waves.state.wave_ordinal) as f64
            ),
        ),
    ]))
}

fn control_participant<'a>(
    game: &'a GameSession,
    context: &'a ModIntentExecutionContext,
) -> Result<&'a str, String> {
    let id = context.context.get("participant_id").and_then(LuaValue::as_str);
    let valid = id.is_some_and(|id| {
        game.participants.contains_key(id)
            && context.scope.kind == WebLuaScopeKind::ParticipantRun
            && context.scope.id == format!("{id}:{}", game.run.run_id.as_deref().unwrap_or("null"))
            && game.boneyard.is_some()
            && game.run.phase == GameRunPhase::Active
    });
    if valid {
        Ok(id.unwrap())
    } else {
        Err("player control intent must belong to the active participant run".into())
    }
}

fn decode_mod_player_input(fields: &LuaObject) -> Result<GameSessionInput, String> {
    let aim = fields
        .get("aim")
        .filter(|value| !matches!(value, LuaValue::Null))
        .map(|value| vector(Some(value), "mod player input.aim"))
        .transpose()?;
    let primary = match fields.get("primary") {
        None | Some(LuaValue::Null) => false,
        Some(LuaValue::Boolean(value)) => *value,
        _ => return Err("mod player input.cast.primary must be boolean".into()),
    };
    let quickbar = match fields.get("quickbar") {
        None | Some(LuaValue::Null) => None,
        Some(value) => {
            let number = value
                .as_number()
                .filter(|value| value.is_finite())
                .ok_or("mod player input.cast.quickbar must be finite")?;
            if number.fract() != 0.0 {
                return Err("mod player input.cast.quickbar must be an integer".into());
            }
            if !(0.0..PLAYER_BELT_SLOT_COUNT as f64).contains(&number) {
                return Err(
                    "mod player input.cast.quickbar must be null or an integer from 0 through 23"
                        .into(),
                );
            }
            Some(number as usize)
        }
    };
    let movement = vector(fields.get("movement"), "mod player input.movement")?;
    if solomon_sim::float::hypot(movement.x, movement.y) > 1.001 {
        return Err("mod player input.movement magnitude exceeds one".into());
    }
    Ok(GameSessionInput { aim, movement, primary, quickbar, ..GameSessionInput::default() })
}
fn vector(value: Option<&LuaValue>, field: &str) -> Result<Vec2, String> {
    let fields =
        value.and_then(LuaValue::as_object).ok_or_else(|| format!("{field} must be an object"))?;
    if let Some((key, _)) = fields.iter().find(|(key, _)| !["x", "y"].contains(key)) {
        return Err(format!("{field}.{key} is not allowed"));
    }
    let coordinate = |key| {
        fields
            .get(key)
            .and_then(LuaValue::as_number)
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("{field}.{key} must be finite"))
    };
    Ok(Vec2::new(coordinate("x")?, coordinate("y")?))
}
fn only_fields(fields: &LuaObject, allowed: &[&str]) -> Result<(), String> {
    match fields.iter().find(|(key, _)| !allowed.contains(key)) {
        Some((key, _)) => Err(format!("unknown player control field: {key}")),
        None => Ok(()),
    }
}
fn control_integer(value: Option<&LuaValue>, field: &str) -> Result<u64, String> {
    value
        .and_then(LuaValue::as_number)
        .filter(|value| {
            value.is_finite()
                && value.fract() == 0.0
                && (0.0..=9_007_199_254_740_991.0).contains(value)
        })
        .map(|value| value as u64)
        .ok_or_else(|| format!("player control {field} must be a nonnegative integer"))
}
fn distance_squared(left: Vec2, right: Vec2) -> f64 {
    let x = left.x - right.x;
    let y = left.y - right.y;
    x * x + y * y
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
fn point_value(point: Vec2) -> LuaValue {
    object_value([("x", LuaValue::Number(point.x)), ("y", LuaValue::Number(point.y))])
}
fn entrance_error(error: MlBotEntranceNavigationError) -> String {
    match error {
        MlBotEntranceNavigationError::NoPlayer(id) => {
            format!("Boneyard entrance navigation has no player {id}")
        }
        MlBotEntranceNavigationError::NoRoute { start, target } => {
            let start = crate::json::stringify(&point_value(start))
                .expect("navigation coordinates are finite");
            let target = target.map_or_else(
                || "Solomon".into(),
                |point| {
                    crate::json::stringify(&point_value(point))
                        .expect("navigation target is finite")
                },
            );
            format!("ML bot found no collision-safe route to {target} from {start}")
        }
    }
}
