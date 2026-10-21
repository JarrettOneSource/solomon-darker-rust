//! Ground loot, Goodies and completion cleanup from `core-server/boneyard-loot-store.ts`.
//!
//! The shared RNG belongs here, including enemy seed writes and presentation draws. A session
//! lends its one painter allocator for births; the store keeps no parallel registration clock.

mod effects;
pub mod messages;
pub mod presentation;

use crate::float;
use std::collections::HashSet;

use crate::economy::{HubEconomyState, HubInventoryItem, HubItemKind, base_item};
use crate::enemies::model::EnemyDeathEffect;
use crate::float::fround;
use crate::loot::items::resolve_native_goodie_contents;
use crate::loot::{
    NativeBonusKind, NativeLootArenaInput, NativeLootDropKind, NativeLootDropSource,
    NativeLootDropSpec, NativeLootItemIds, NativeLootKeyInput, NativeLootModifiers,
    NativeLootParticipantInput, NativeLootPlacement, NativeLootPolicies, NativeLootPolicy,
    NativeLootScriptAction, NativeLootSelectionInput, NativeOrbKind, initial_native_key_drop_level,
    materialize_native_loot_script_action, roll_native_enemy_loot,
};
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::rng::seed_from_text;
use crate::{NativeRng, Vec2};

use super::BoneyardBounds;

pub const NATIVE_LOOT_WORLD_ID_MINIMUM: usize = 1;
pub const NATIVE_LOOT_WORLD_ID_MAXIMUM: usize = 2047;
pub const NATIVE_LOOT_EVENT_RETENTION_TICKS: u64 = 100;
pub const NATIVE_GOODIE_KEY_FEEDBACK_INTERVAL_TICKS: i64 = 200;
const NATIVE_ORB_DECAY_DELAY_TICKS: i64 = 900;
const NATIVE_ORB_DECAY_PER_TICK: f64 = 0.002_f32 as f64;
const NATIVE_ORB_ACTIVE_VALUE_FLOOR: f64 = 0.01_f32 as f64;
const NATIVE_BONUS_LIFETIME_TICKS: i64 = 1200;
const NATIVE_GOODIE_EFFECT_ID_START: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BoneyardLootSound {
    DropBag1,
    DropBag2,
    DropCoins,
    DropPotion,
    GotoOrb,
    PickupBag,
    PickupCoin,
}

impl BoneyardLootSound {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DropBag1 => "drop-bag-1",
            Self::DropBag2 => "drop-bag-2",
            Self::DropCoins => "drop-coins",
            Self::DropPotion => "drop-potion",
            Self::GotoOrb => "goto-orb",
            Self::PickupBag => "pickup-bag",
            Self::PickupCoin => "pickup-coin",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BoneyardLootEventType {
    GoodieKeyNeeded,
    GoodiePhase,
    LootDropSound,
    LootPickup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BoneyardGoodiePhase {
    Intact,
    Opening,
    Broken,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardLootActor {
    pub activation_delay_ticks: i64,
    pub age_ticks: u64,
    pub alpha: f64,
    pub amount: u64,
    pub animation_phase: f64,
    pub bonus_kind: Option<NativeBonusKind>,
    pub bounce_height: f64,
    pub bounce_velocity: f64,
    pub frame_phase: f64,
    pub id: usize,
    pub item: Option<HubInventoryItem>,
    pub kind: NativeLootDropKind,
    pub last_step_tick: Option<u64>,
    pub lifetime_ticks_remaining: Option<i64>,
    pub native_type_id: u32,
    pub orb_kind: Option<NativeOrbKind>,
    pub orb_value: f64,
    pub painter_registration: PainterRegistration,
    pub position: Vec2,
    pub rotation_deg: f64,
    pub scatter_seed: u32,
    pub scatter_active: bool,
    pub scatter_progress: f64,
    pub settled_sound_emitted: bool,
    pub source: NativeLootDropSource,
    pub spawn_tick: u64,
    pub tier: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardGoodieInput {
    pub eid: String,
    pub position: Vec2,
    pub reward_seed: Option<u32>,
    pub scenery_registration_ordinal: Option<usize>,
    pub subtype: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardGoodieState {
    pub active: bool,
    pub activated_by_player_id: Option<String>,
    pub eid: String,
    pub exhausted: bool,
    pub id: usize,
    pub phase: BoneyardGoodiePhase,
    pub position: Vec2,
    pub reward_seed: u32,
    pub scenery_registration_ordinal: usize,
    pub subtype: u32,
    pub timer: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardLootStore {
    pub actors: Vec<BoneyardLootActor>,
    pub effects: Vec<EnemyDeathEffect>,
    pub goodies: Vec<BoneyardGoodieState>,
    pub last_step_tick: Option<u64>,
    pub last_key_needed_tick: i64,
    pub last_successful_item_level: i32,
    pub next_actor_id: usize,
    pub next_event_id: u64,
    pub next_effect_id: usize,
    pub next_goodie_id: usize,
    pub next_item_id: u64,
    pub next_key_drop_level: u32,
    pub shared_rng: NativeRng,
}

#[derive(Clone, Copy, Debug)]
pub struct BoneyardLootParticipant<'a> {
    pub advanced_unlocks: &'a [bool; 8],
    pub alive: bool,
    pub connected: bool,
    pub heading_index: usize,
    pub level: u32,
    pub modifiers: NativeLootModifiers,
    pub owned_recipe_indexes: &'a [usize],
    pub player_id: &'a str,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardLootPickup {
    pub actor_id: usize,
    pub amount: u64,
    pub bonus_kind: Option<NativeBonusKind>,
    pub item: Option<HubInventoryItem>,
    pub kind: NativeLootDropKind,
    pub orb_kind: Option<NativeOrbKind>,
    pub orb_value: f64,
    pub player_id: String,
    pub source_position: Vec2,
    pub tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardLootEvent {
    pub actor_id: usize,
    pub event_id: u64,
    pub goodie_id: Option<usize>,
    pub phase: Option<BoneyardGoodiePhase>,
    pub playback_rate: Option<f64>,
    pub player_id: Option<String>,
    pub position: Vec2,
    pub sound: Option<BoneyardLootSound>,
    pub text: Option<String>,
    pub tick: u64,
    pub event_type: BoneyardLootEventType,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneyardLootStep {
    pub events: Vec<BoneyardLootEvent>,
    pub pickups: Vec<BoneyardLootPickup>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHagathaLastWordLoot {
    pub actor_ids: Vec<usize>,
    pub gold: u64,
    pub items: Vec<HubInventoryItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BoneyardLootDeathProgram {
    MinibossDie,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BoneyardGoodieInteraction {
    Opened { goodie_id: usize },
    MissingKey { event: Option<BoneyardLootEvent> },
    NoTarget,
}

impl BoneyardLootStore {
    #[must_use]
    pub fn new(seed: &str, source_goodies: &[BoneyardGoodieInput]) -> Self {
        let mut shared_rng =
            NativeRng::new(i64::from(seed_from_text(&format!("{seed}:native-loot"))));
        let mut goodies = Vec::with_capacity(source_goodies.len());
        for (source_order, source) in source_goodies.iter().enumerate() {
            let reward_seed =
                source.reward_seed.unwrap_or_else(|| shared_rng.draw_integer(1000) as u32);
            assert!(reward_seed < 1000, "Goodie reward seed is in0..1000");
            goodies.push(BoneyardGoodieState {
                active: false,
                activated_by_player_id: None,
                eid: source.eid.clone(),
                exhausted: false,
                id: source_order + 1,
                phase: BoneyardGoodiePhase::Intact,
                position: source.position,
                reward_seed,
                scenery_registration_ordinal: source
                    .scenery_registration_ordinal
                    .unwrap_or(source_order),
                subtype: source.subtype.unwrap_or(0),
                timer: 0,
            });
        }
        let next_key_drop_level = initial_native_key_drop_level(&mut shared_rng);
        Self {
            actors: Vec::new(),
            effects: Vec::new(),
            goodies,
            last_step_tick: None,
            last_key_needed_tick: -NATIVE_GOODIE_KEY_FEEDBACK_INTERVAL_TICKS - 1,
            last_successful_item_level: -1,
            next_actor_id: NATIVE_LOOT_WORLD_ID_MINIMUM,
            next_event_id: 1,
            next_effect_id: NATIVE_GOODIE_EFFECT_ID_START,
            next_goodie_id: source_goodies.len() + 1,
            next_item_id: 1,
            next_key_drop_level,
            shared_rng,
        }
    }

    /// The actual enemy producer callback draws from this stream, rather than actor entropy.
    pub fn roll_loot_seed(&mut self, bound: u32) -> u32 {
        assert!(
            matches!(bound, 1_000_000 | 10_000_000),
            "native enemy seed writers use their source producer bound"
        );
        self.shared_rng.draw_integer(bound as i32) as u32
    }

    #[must_use]
    pub fn native_hagatha_last_word_loot(&self) -> NativeHagathaLastWordLoot {
        let mut output =
            NativeHagathaLastWordLoot { actor_ids: Vec::new(), gold: 0, items: Vec::new() };
        for actor in &self.actors {
            if actor.native_type_id != 2012 && actor.native_type_id != 2013 {
                continue;
            }
            output.actor_ids.push(actor.id);
            if actor.native_type_id == 2012 {
                output.gold += actor.amount;
            }
            if actor.native_type_id == 2013
                && let Some(item) = &actor.item
            {
                output.items.push(item.clone());
            }
        }
        output
    }

    pub fn remove_actors(&mut self, actor_ids: &[usize]) {
        let removed: HashSet<_> = actor_ids.iter().copied().collect();
        self.actors.retain(|actor| !removed.contains(&actor.id));
        self.effects.retain(|effect| !removed.contains(&effect.owner_actor_id));
    }

    pub fn retire_goodies_outside_bounds(&mut self, bounds: BoneyardBounds) {
        self.goodies.retain(|goodie| {
            goodie.position.x >= bounds.x
                && goodie.position.y >= bounds.y
                && goodie.position.x <= bounds.x + bounds.width
                && goodie.position.y <= bounds.y + bounds.height
        });
    }

    pub fn activate_goodie(&mut self, eid: &str, player_id: &str) {
        for goodie in &mut self.goodies {
            if goodie.eid != eid || goodie.active || goodie.exhausted {
                continue;
            }
            goodie.active = true;
            goodie.activated_by_player_id = Some(player_id.to_owned());
            goodie.timer = 0;
        }
    }

    /// `boneyard-goodie-interaction.ts`: strict radius 50 around a facing probe 25 ahead.
    /// Equal-distance targets retain their source order. Active/opened Goodies cannot target.
    #[must_use]
    pub fn nearest_goodie(
        &self,
        position: Vec2,
        heading_index: usize,
    ) -> Option<&BoneyardGoodieState> {
        let heading = heading_index as f64 * std::f64::consts::PI / 12.0;
        let query = Vec2::new(
            fround(position.x + float::sin(heading) * 25.0),
            fround(position.y - float::cos(heading) * 25.0),
        );
        let mut nearest = None;
        let mut nearest_distance_squared = 50.0 * 50.0;
        for goodie in &self.goodies {
            if goodie.active || goodie.exhausted || goodie.phase != BoneyardGoodiePhase::Intact {
                continue;
            }
            let dx = goodie.position.x - query.x;
            let dy = goodie.position.y - query.y;
            let distance_squared = dx * dx + dy * dy;
            if distance_squared < nearest_distance_squared {
                nearest = Some(goodie);
                nearest_distance_squared = distance_squared;
            }
        }
        nearest
    }

    /// `game-simulation.ts::interactWithBoneyardGoodie`, after session admission checks
    /// active run, living player and level-up barrier. Key consumption uses the canonical
    /// recursive inventory transaction; a failed attempt only advances key-feedback state.
    pub fn interact_goodie(
        &mut self,
        economy: &mut HubEconomyState,
        player_id: &str,
        position: Vec2,
        heading_index: usize,
        tick: u64,
    ) -> BoneyardGoodieInteraction {
        let Some(goodie) = self.nearest_goodie(position, heading_index) else {
            return BoneyardGoodieInteraction::NoTarget;
        };
        let goodie_id = goodie.id;
        let eid = goodie.eid.clone();
        if !economy.consume_wizard_key() {
            return BoneyardGoodieInteraction::MissingKey {
                event: self.goodie_key_needed(goodie_id, player_id, tick),
            };
        }
        self.activate_goodie(&eid, player_id);
        BoneyardGoodieInteraction::Opened { goodie_id }
    }

    pub fn goodie_key_needed(
        &mut self,
        goodie_id: usize,
        player_id: &str,
        tick: u64,
    ) -> Option<BoneyardLootEvent> {
        let goodie = self.goodies.iter().find(|goodie| goodie.id == goodie_id)?;
        if tick as i64 <= self.last_key_needed_tick + NATIVE_GOODIE_KEY_FEEDBACK_INTERVAL_TICKS {
            return None;
        }
        let mut event =
            self.event(BoneyardLootEventType::GoodieKeyNeeded, goodie.id, goodie.position, tick);
        event.goodie_id = Some(goodie_id);
        event.player_id = Some(player_id.to_owned());
        event.text = Some("I need a key!".to_owned());
        self.last_key_needed_tick = tick as i64;
        Some(event)
    }

    /// Returns the number rejected by the stock 1..=2047 ground-actor address space.
    pub fn spawn_specs(
        &mut self,
        specs: Vec<NativeLootDropSpec>,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
    ) -> usize {
        let mut rejected = 0;
        let mut occupied: HashSet<_> = self.actors.iter().map(|actor| actor.id).collect();
        for spec in specs {
            let Some(id) = self.allocate_actor_id(&occupied) else {
                rejected += 1;
                continue;
            };
            occupied.insert(id);
            self.actors.push(BoneyardLootActor {
                activation_delay_ticks: spec.activation_delay_ticks,
                age_ticks: 0,
                alpha: if spec.kind == NativeLootDropKind::Bonus { 1.0 } else { 0.0 },
                amount: spec.amount.unwrap_or(0),
                animation_phase: spec.phase,
                bonus_kind: spec.bonus_kind,
                bounce_height: if spec.kind == NativeLootDropKind::Sack { -25.0 } else { 0.0 },
                bounce_velocity: if spec.kind == NativeLootDropKind::Sack {
                    0.1_f32 as f64
                } else {
                    0.0
                },
                frame_phase: 0.0,
                id,
                item: spec.item,
                kind: spec.kind,
                last_step_tick: tick.checked_sub(1),
                lifetime_ticks_remaining: match spec.kind {
                    NativeLootDropKind::Orb => Some(NATIVE_ORB_DECAY_DELAY_TICKS),
                    NativeLootDropKind::Bonus => Some(NATIVE_BONUS_LIFETIME_TICKS),
                    _ => None,
                },
                native_type_id: spec.native_type_id,
                orb_kind: spec.orb_kind,
                orb_value: spec.value.unwrap_or(0.0),
                painter_registration: painter_order.register(WorldManagerLane::Actor),
                position: spec.position,
                rotation_deg: spec.rotation_deg.unwrap_or(0.0),
                scatter_seed: spec.scatter_seed.unwrap_or(0),
                scatter_active: spec.kind == NativeLootDropKind::Gold,
                scatter_progress: 0.0,
                settled_sound_emitted: false,
                source: spec.source,
                spawn_tick: tick,
                tier: spec.tier.unwrap_or(0),
            });
        }
        rejected
    }

    pub fn spawn_custom_items(
        &mut self,
        items: &[HubInventoryItem],
        position: Vec2,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
    ) -> usize {
        let mut specs = Vec::with_capacity(items.len());
        for source in items {
            let mut item = source.clone();
            item.id = self.next_item_id;
            self.next_item_id += 1;
            let mut spec = NativeLootDropSpec::new(
                NativeLootDropKind::Sack,
                0,
                position,
                NativeLootDropSource::Enemy,
            );
            spec.item = Some(item);
            specs.push(spec);
        }
        self.spawn_specs(specs, tick, painter_order)
    }

    /// Arena last-drop level, key schedule and remaining chests are store-owned source fields.
    /// The borrowed selection supplies actual player, enemy and geometry values.
    pub fn materialize_enemy_loot(
        &mut self,
        input: NativeLootSelectionInput<'_>,
        on_death_program: Option<BoneyardLootDeathProgram>,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
    ) -> usize {
        let mut ids = NativeLootItemIds::new(self.next_item_id);
        let ground = self.ground_recipe_indexes();
        let selected_input = self.enemy_selection_input(input, &ground);
        let rolled = roll_native_enemy_loot(&selected_input, &mut self.shared_rng, &mut ids);
        self.last_successful_item_level = rolled.materialization.last_successful_item_level;
        self.next_key_drop_level = rolled.materialization.next_key_drop_level;
        self.next_item_id = ids.next_id;
        // The ordinary carriers enter the store before a miniboss death script
        // selects its reward. Its next selection sees those direct named items.
        let mut rejected = self.spawn_specs(rolled.materialization.drops, tick, painter_order);
        if on_death_program == Some(BoneyardLootDeathProgram::MinibossDie) {
            let action = if self.shared_rng.draw_integer(2) == 1 {
                NativeLootScriptAction::DropRandomGold { minimum: 300, maximum: 600 }
            } else {
                NativeLootScriptAction::DropRandomItem { mode: 0 }
            };
            let ground = self.ground_recipe_indexes();
            let selected_input = self.enemy_selection_input(input, &ground);
            let scripted = materialize_native_loot_script_action(
                &selected_input,
                action,
                &mut self.shared_rng,
                &mut ids,
            );
            self.last_successful_item_level = scripted.last_successful_item_level;
            self.next_key_drop_level = scripted.next_key_drop_level;
            self.next_item_id = ids.next_id;
            rejected += self.spawn_specs(scripted.drops, tick, painter_order);
        }
        rejected
    }

    /// `groundRecipeIndexes` examines only the direct item of retained type-2013
    /// carriers. Inventory Sack contents and other actor types do not exclude recipes.
    fn ground_recipe_indexes(&self) -> Vec<usize> {
        let mut recipes = Vec::new();
        for actor in &self.actors {
            if actor.native_type_id == 2013
                && let Some(recipe) = actor.item.as_ref().and_then(|item| item.recipe_index)
            {
                recipes.push(recipe);
            }
        }
        recipes
    }

    fn enemy_selection_input<'a>(
        &self,
        mut input: NativeLootSelectionInput<'a>,
        ground_recipe_indexes: &'a [usize],
    ) -> NativeLootSelectionInput<'a> {
        input.ground_recipe_indexes = ground_recipe_indexes;
        input.arena.last_successful_item_level = self.last_successful_item_level;
        input.key = NativeLootKeyInput {
            current: self.next_key_drop_level,
            level: input.arena.level as u32,
            remaining: self
                .goodies
                .iter()
                .filter(|goodie| !goodie.active && !goodie.exhausted)
                .count(),
        };
        input.explicit_gold_amount = None;
        input.drop_delay_context = 0;
        input
    }

    /// Catches up missing source ticks with the supplied participant snapshot, just like the host.
    pub fn step(
        &mut self,
        participants: &[BoneyardLootParticipant<'_>],
        placement: &dyn NativeLootPlacement,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
    ) -> BoneyardLootStep {
        assert!(self.last_step_tick.is_none_or(|last| tick > last), "loot ticks move forward");
        validate_participants(participants);
        let first_tick = self.last_step_tick.map_or(tick, |last| last + 1);
        let mut output = BoneyardLootStep::default();
        for step_tick in first_tick..=tick {
            self.step_effects(placement, step_tick);
            self.step_goodies(
                participants,
                placement,
                step_tick,
                painter_order,
                &mut output.events,
            );
            let mut retained = Vec::with_capacity(self.actors.len());
            for actor in std::mem::take(&mut self.actors) {
                if let Some(actor) =
                    self.step_actor(actor, participants, step_tick, painter_order, &mut output)
                {
                    retained.push(actor);
                }
            }
            self.actors = retained;
        }
        self.last_step_tick = Some(tick);
        output
    }

    fn allocate_actor_id(&mut self, occupied: &HashSet<usize>) -> Option<usize> {
        for _ in 0..2048 {
            let candidate = self.next_actor_id;
            self.next_actor_id = if candidate >= NATIVE_LOOT_WORLD_ID_MAXIMUM {
                NATIVE_LOOT_WORLD_ID_MINIMUM
            } else {
                candidate + 1
            };
            if !occupied.contains(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn event(
        &mut self,
        event_type: BoneyardLootEventType,
        actor_id: usize,
        position: Vec2,
        tick: u64,
    ) -> BoneyardLootEvent {
        let event_id = self.next_event_id;
        self.next_event_id += 1;
        BoneyardLootEvent {
            actor_id,
            event_id,
            goodie_id: None,
            phase: None,
            playback_rate: None,
            player_id: None,
            position,
            sound: None,
            text: None,
            tick,
            event_type,
        }
    }

    fn step_actor(
        &mut self,
        mut actor: BoneyardLootActor,
        participants: &[BoneyardLootParticipant<'_>],
        tick: u64,
        painter_order: &mut WorldPainterOrder,
        output: &mut BoneyardLootStep,
    ) -> Option<BoneyardLootActor> {
        actor.age_ticks += 1;
        actor.last_step_tick = Some(tick);
        if actor.kind == NativeLootDropKind::Orb {
            return self.step_orb(actor, participants, tick, painter_order, output);
        }
        if actor.kind == NativeLootDropKind::Bonus {
            return self.step_bonus(actor, participants, tick, painter_order, output);
        }
        actor.activation_delay_ticks -= 1;
        if actor.activation_delay_ticks < 1 && actor.kind == NativeLootDropKind::Gold {
            if actor.scatter_active {
                actor.scatter_progress = fround(actor.scatter_progress + 0.5);
                if actor.scatter_progress > 8.0 {
                    actor.scatter_active = false;
                    if !actor.settled_sound_emitted {
                        let pitch = self.draw_loot_pitch();
                        self.emit_sound(
                            &actor,
                            tick,
                            BoneyardLootSound::DropCoins,
                            pitch,
                            &mut output.events,
                        );
                        actor.settled_sound_emitted = true;
                    }
                }
            } else {
                actor.animation_phase = fround(actor.animation_phase + 2.0);
            }
        }
        if actor.activation_delay_ticks < 1
            && actor.kind == NativeLootDropKind::Sack
            && actor.bounce_velocity != 0.0
        {
            let height = fround(actor.bounce_height + actor.bounce_velocity);
            let velocity = fround(actor.bounce_velocity * 1.5);
            if height > 0.0 {
                actor.bounce_height = 0.0;
                actor.bounce_velocity = 0.0;
                if !actor.settled_sound_emitted {
                    let sound = self.sack_drop_sound(&actor);
                    self.emit_sound(&actor, tick, sound, 1.0, &mut output.events);
                    actor.settled_sound_emitted = true;
                }
            } else {
                actor.bounce_height = height;
                actor.bounce_velocity = velocity;
            }
        }
        if actor.activation_delay_ticks >= 1 {
            return Some(actor);
        }
        let Some(participant) = self.first_capture_participant(&actor, participants) else {
            return Some(actor);
        };
        self.accept_pickup(&actor, participant, tick, painter_order, output);
        None
    }

    fn step_orb(
        &mut self,
        mut actor: BoneyardLootActor,
        participants: &[BoneyardLootParticipant<'_>],
        tick: u64,
        painter_order: &mut WorldPainterOrder,
        output: &mut BoneyardLootStep,
    ) -> Option<BoneyardLootActor> {
        let delay = actor.lifetime_ticks_remaining.unwrap_or(NATIVE_ORB_DECAY_DELAY_TICKS) - 1;
        actor.alpha = fround(actor.alpha + (0.05_f32 as f64)).min(1.0);
        actor.animation_phase = fround(actor.animation_phase + 2.5);
        if delay < 1 {
            actor.orb_value = fround(actor.orb_value - NATIVE_ORB_DECAY_PER_TICK);
        }
        if actor.orb_value <= 0.0 {
            return None;
        }
        actor.lifetime_ticks_remaining = Some(delay);
        if actor.orb_value <= NATIVE_ORB_ACTIVE_VALUE_FLOOR {
            return Some(actor);
        }
        for participant in participants {
            if !participant.alive || !participant.connected {
                continue;
            }
            let dx = participant.position.x - actor.position.x;
            let dy = participant.position.y - actor.position.y;
            let distance_squared = dx * dx + dy * dy;
            let pull_radius =
                fround(60.0 * participant.modifiers.pickup_factor * participant.modifiers.orb_pull);
            if distance_squared >= pull_radius * pull_radius {
                continue;
            }
            let capture_radius = fround(20.0 * participant.modifiers.pickup_factor);
            if distance_squared < capture_radius * capture_radius {
                self.accept_pickup(&actor, participant, tick, painter_order, output);
                return None;
            }
            let distance = distance_squared.sqrt();
            if distance > 0.0 {
                actor.position.x = fround(actor.position.x + dx / distance * 1.5);
                actor.position.y = fround(actor.position.y + dy / distance * 1.5);
            }
        }
        Some(actor)
    }

    fn step_bonus(
        &mut self,
        mut actor: BoneyardLootActor,
        participants: &[BoneyardLootParticipant<'_>],
        tick: u64,
        painter_order: &mut WorldPainterOrder,
        output: &mut BoneyardLootStep,
    ) -> Option<BoneyardLootActor> {
        let delay = actor.lifetime_ticks_remaining.unwrap_or(NATIVE_BONUS_LIFETIME_TICKS) - 1;
        if delay < 1 {
            actor.alpha = fround(actor.alpha - (0.01_f32 as f64));
        }
        if actor.alpha <= 0.0 {
            return None;
        }
        actor.lifetime_ticks_remaining = Some(delay);
        actor.frame_phase = fround(actor.frame_phase + (0.2_f32 as f64));
        if actor.frame_phase > 18.0 {
            actor.frame_phase = 0.0;
        }
        actor.animation_phase = fround(actor.animation_phase + 2.5);
        actor.rotation_deg = fround(actor.rotation_deg + 1.0);
        let Some(participant) = self.first_capture_participant(&actor, participants) else {
            return Some(actor);
        };
        self.accept_pickup(&actor, participant, tick, painter_order, output);
        None
    }

    fn first_capture_participant<'a, 'b>(
        &mut self,
        actor: &BoneyardLootActor,
        participants: &'a [BoneyardLootParticipant<'b>],
    ) -> Option<&'a BoneyardLootParticipant<'b>> {
        for participant in participants {
            if !participant.alive || !participant.connected {
                continue;
            }
            let dx = actor.position.x - participant.position.x;
            let dy = actor.position.y - participant.position.y;
            let base_radius = if actor.kind == NativeLootDropKind::Bonus { 20.0 } else { 30.0 };
            let radius = fround(base_radius * participant.modifiers.pickup_factor);
            if dx * dx + dy * dy >= radius * radius {
                continue;
            }
            if actor.kind == NativeLootDropKind::Gold
                && participant.modifiers.pickup_factor > 1.259_999_99
                && self.shared_rng.draw_integer(15) != 1
            {
                continue;
            }
            return Some(participant);
        }
        None
    }

    fn accept_pickup(
        &mut self,
        actor: &BoneyardLootActor,
        participant: &BoneyardLootParticipant<'_>,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
        output: &mut BoneyardLootStep,
    ) {
        self.spawn_pickup_effects(actor, tick, painter_order);
        output.pickups.push(BoneyardLootPickup {
            actor_id: actor.id,
            amount: actor.amount,
            bonus_kind: actor.bonus_kind,
            item: actor.item.clone(),
            kind: actor.kind,
            orb_kind: actor.orb_kind,
            orb_value: actor.orb_value,
            player_id: participant.player_id.to_owned(),
            source_position: actor.position,
            tick,
        });
        let sound = match actor.kind {
            NativeLootDropKind::Gold => Some(BoneyardLootSound::PickupCoin),
            NativeLootDropKind::Orb => Some(BoneyardLootSound::GotoOrb),
            NativeLootDropKind::Sack => Some(BoneyardLootSound::PickupBag),
            NativeLootDropKind::Bonus => None,
        };
        let playback_rate = sound.map(|sound| {
            if sound == BoneyardLootSound::GotoOrb { 1.0 } else { self.draw_loot_pitch() }
        });
        let text = match actor.kind {
            NativeLootDropKind::Gold => Some(format!("{} GOLD", actor.amount)),
            NativeLootDropKind::Sack => actor.item.as_ref().map(|item| item.name.clone()),
            NativeLootDropKind::Bonus => match actor.bonus_kind {
                Some(NativeBonusKind::SkillChoice) => Some("BONUS SKILL POINT".to_owned()),
                Some(NativeBonusKind::DamageX4) => Some("DAMAGE x4".to_owned()),
                _ => None,
            },
            NativeLootDropKind::Orb => None,
        };
        let mut event =
            self.event(BoneyardLootEventType::LootPickup, actor.id, actor.position, tick);
        event.player_id = Some(participant.player_id.to_owned());
        event.sound = sound;
        event.playback_rate = playback_rate;
        event.text = text;
        output.events.push(event);
    }

    fn sack_drop_sound(&mut self, actor: &BoneyardLootActor) -> BoneyardLootSound {
        if actor.item.as_ref().is_some_and(|item| item.native_type_id == 7001) {
            return BoneyardLootSound::DropPotion;
        }
        if self.shared_rng.draw_integer(2) == 0 {
            BoneyardLootSound::DropBag1
        } else {
            BoneyardLootSound::DropBag2
        }
    }

    fn draw_loot_pitch(&mut self) -> f64 {
        fround(self.shared_rng.draw_scaled_float(0.1_f32 as f64, true) + 1.0)
    }

    fn emit_sound(
        &mut self,
        actor: &BoneyardLootActor,
        tick: u64,
        sound: BoneyardLootSound,
        playback_rate: f64,
        events: &mut Vec<BoneyardLootEvent>,
    ) {
        let mut event =
            self.event(BoneyardLootEventType::LootDropSound, actor.id, actor.position, tick);
        event.sound = Some(sound);
        event.playback_rate = Some(playback_rate);
        events.push(event);
    }

    fn step_goodies(
        &mut self,
        participants: &[BoneyardLootParticipant<'_>],
        placement: &dyn NativeLootPlacement,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
        events: &mut Vec<BoneyardLootEvent>,
    ) {
        let mut goodies = Vec::with_capacity(self.goodies.len());
        for mut goodie in std::mem::take(&mut self.goodies) {
            if !goodie.active || goodie.exhausted {
                goodies.push(goodie);
                continue;
            }
            goodie.timer += 1;
            if goodie.timer == 100 || goodie.timer == 200 {
                goodie.phase = if goodie.timer == 100 {
                    BoneyardGoodiePhase::Opening
                } else {
                    BoneyardGoodiePhase::Broken
                };
                if goodie.timer == 100 {
                    self.spawn_goodie_break_effects(&goodie, tick, painter_order);
                }
                let mut event = self.event(
                    BoneyardLootEventType::GoodiePhase,
                    goodie.id,
                    goodie.position,
                    tick,
                );
                event.goodie_id = Some(goodie.id);
                event.phase = Some(goodie.phase);
                events.push(event);
            }
            if goodie.timer == 250 {
                let participant = participants.iter().find(|participant| {
                    Some(participant.player_id) == goodie.activated_by_player_id.as_deref()
                });
                goodie.active = false;
                goodie.activated_by_player_id = None;
                goodie.exhausted = true;
                goodie.phase = BoneyardGoodiePhase::Broken;
                self.materialize_goodie(&goodie, participant, placement, tick, painter_order);
            }
            goodies.push(goodie);
        }
        self.goodies = goodies;
    }

    fn materialize_goodie(
        &mut self,
        goodie: &BoneyardGoodieState,
        participant: Option<&BoneyardLootParticipant<'_>>,
        placement: &dyn NativeLootPlacement,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
    ) {
        let mut ids = NativeLootItemIds::new(self.next_item_id);
        let advanced = [false; 8];
        let ground = self.ground_recipe_indexes();
        let contents = resolve_native_goodie_contents(
            &mut self.shared_rng,
            &mut ids,
            goodie.reward_seed % 18,
            participant.map_or(1, |player| player.level),
            participant.map_or(&advanced, |player| player.advanced_unlocks),
            participant.map_or(&[], |player| player.owned_recipe_indexes),
            &ground,
        );
        let position = Vec2::new(goodie.position.x, goodie.position.y + 25.0);
        if contents.gold > 0 {
            let input = NativeLootSelectionInput {
                actor_seed: 0,
                ground_recipe_indexes: &[],
                item_recipe_pool: None,
                arena: NativeLootArenaInput {
                    disable_mask: 0,
                    item_level_maximum: 100,
                    item_level_minimum: 0,
                    last_successful_item_level: -1,
                    level: 0,
                    mode: 0,
                    special_suppression: false,
                },
                explicit_gold_amount: Some(contents.gold),
                drop_delay_context: 0,
                inventory_has_health_potion: false,
                key: NativeLootKeyInput { current: 1, level: 0, remaining: 0 },
                nearby_mask_two_count: 0,
                participant: NativeLootParticipantInput {
                    advanced_unlocks: &advanced,
                    level: 1,
                    modifiers: participant
                        .map_or_else(NativeLootModifiers::default, |player| player.modifiers),
                    owned_recipe_indexes: &[],
                    slot: 0,
                },
                placement,
                policies: NativeLootPolicies {
                    gold: NativeLootPolicy::Forced,
                    ..NativeLootPolicies::DISABLED
                },
                scene_forces_health_potion: false,
                source_position: position,
                world_badguy_count: 0,
                world_has_health_potion_sack: false,
            };
            let rolled = roll_native_enemy_loot(&input, &mut self.shared_rng, &mut ids);
            self.next_item_id = ids.next_id;
            let mut drops = rolled.materialization.drops;
            for drop in &mut drops {
                drop.source = NativeLootDropSource::Goodie;
            }
            self.spawn_specs(drops, tick, painter_order);
            return;
        }
        self.next_item_id = ids.next_id;
        if contents.items.is_empty() {
            return;
        }
        let mut bundle =
            base_item(self.next_item_id, HubItemKind::Sack, "Sack", 7008, Some(0), &[70]);
        self.next_item_id += 1;
        bundle.contents = Some(contents.items);
        let mut spec = NativeLootDropSpec::new(
            NativeLootDropKind::Sack,
            0,
            position,
            NativeLootDropSource::Goodie,
        );
        spec.item = Some(bundle);
        self.spawn_specs(vec![spec], tick, painter_order);
    }
}

fn validate_participants(participants: &[BoneyardLootParticipant<'_>]) {
    let mut ids = HashSet::new();
    for participant in participants {
        assert!(ids.insert(participant.player_id), "loot participant order contains a duplicate");
        assert!(
            participant.position.x.is_finite() && participant.position.y.is_finite(),
            "loot participant positions are finite"
        );
        assert!(participant.heading_index < 24, "loot participant heading is in0..24");
    }
}
