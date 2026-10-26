//! Shared world admission and reset order from the public Game simulation API.

use super::*;
use crate::NativeRng;
use crate::boneyard::tutorial::NATIVE_TUTORIAL_FIRES;
use crate::hub::NATIVE_HUB_COLLISION_RNG_SEED;
use crate::hub::interaction::HubNpcMarkerEpoch;
use crate::hub::painter::HUB_FIRST_PLAYER_PAINTER_ORDINAL;
use crate::painter::WorldManagerLane;
use crate::player::PlayerState;
use crate::profile::NewGameSelection;
use crate::score::NativeHallOfFameRunState;
use crate::screen_feedback::NativeSecondaryScreenFeedbackPresentation;
use crate::secondary::NativeScriptFire;

impl GameSession {
    #[must_use]
    pub fn new(id: String, selection: NewGameSelection, options: GameSessionOptions) -> Self {
        Self::new_participants(vec![(id, selection)], options)
            .expect("one participant has a unique identity")
    }

    /// Configurations arrive in the source Record's entity order, which controls
    /// creation RNG and painter enrollment. Barrier cohorts sort IDs separately.
    pub fn new_participants(
        selections: Vec<(String, NewGameSelection)>,
        options: GameSessionOptions,
    ) -> Result<Self, String> {
        let mut seen = std::collections::HashSet::new();
        if selections.iter().any(|(id, _)| !seen.insert(id)) {
            return Err("participant identities must be unique".into());
        }
        let mut painter_order = WorldPainterOrder {
            next_actor_ordinal: HUB_FIRST_PLAYER_PAINTER_ORDINAL,
            next_transient_ordinal: 0,
        };
        let mut gameplay_rng = NativeRng::new(options.gameplay_rng_seed);
        let mut participants = Participants::default();
        let mut participant_order = Vec::with_capacity(selections.len());
        let mut next_entity_id = 1;
        for (id, selection) in selections {
            let player = PlayerState::new(selection, &mut gameplay_rng);
            let registration = painter_order.register(WorldManagerLane::Actor);
            let participant = new_hub_participant(
                id.clone(),
                next_entity_id,
                player,
                registration,
                options.hub_trader_animation_seed,
                0,
            );
            participant_order.push(id.clone());
            participants.insert(id, participant);
            next_entity_id += 1;
        }
        // The constructor warms students privately, then enrolls only survivors
        // after every player. Retired warmup actors consume no live painter slots.
        let npcs = HubNpcState::new(options.hub_trader_animation_seed, Some(&mut painter_order));
        Ok(Self {
            inventory_dye_sessions: std::collections::BTreeMap::new(),
            screen_flashes: crate::screen_flash::NativeScreenFlashState::default(),
            enhanced_effects: options.enhanced_effects,
            participants,
            participant_order,
            next_entity_id,
            npcs,
            ambient: HubAmbientState::new(),
            dowser_clock: HubCommonTraderClock::new(options.hub_trader_animation_seed ^ 5016),
            hub_trader_animation_seed: options.hub_trader_animation_seed,
            hub_memorial: crate::hub::memorial::create_hub_memorial_state(),
            collision_rng: NATIVE_HUB_COLLISION_RNG_SEED,
            gameplay_rng,
            combat_rng: NativeRng::new(options.combat_rng_seed),
            secondary: NativeSecondarySimulationState::new(0),
            spells: SpellState::default(),
            hall_of_fame_clock_started_at_tick: 0,
            painter_order,
            run: GameRunLifecycle::new(),
            level_up_barrier: None,
            next_level_up_barrier_id: 1,
            mod_effects: crate::ComponentEntities::default(),
            next_mod_consumable_use_id: 1,
            ticks: 0,
            boneyard: None,
        })
    }

    #[must_use]
    pub fn can_enter_boneyard(&self) -> bool {
        self.boneyard.is_none()
            && self.run.phase == crate::game_run::GameRunPhase::Hub
            && self.level_up_barrier.is_none()
    }

    pub fn install_boneyard(&mut self, loaded: LoadedBoneyard) -> Result<(), String> {
        if self.level_up_barrier.is_some() {
            return Err("cannot enter a Boneyard during a level-up barrier".into());
        }
        if self.run.phase != crate::game_run::GameRunPhase::Hub {
            return Err("a game run can start only in the College".into());
        }
        if loaded.choice_id == "stock-tutorial" && self.participants.len() != 1 {
            return Err("the stock Tutorial requires exactly one authoritative player".into());
        }
        let mut world = BoneyardWorld::new(loaded);
        self.painter_order = WorldPainterOrder::default();
        for id in &self.participant_order {
            let participant = self.participants.get_mut(id).unwrap();
            *participant.lighting =
                PlayerLightingState::new(self.painter_order.register(WorldManagerLane::Actor));
        }
        if world.loaded.scene.solomon_dig.is_some() {
            world.lantern_light_registration =
                Some(self.painter_order.register(WorldManagerLane::Actor));
            world.solomon_painter_registration =
                Some(self.painter_order.register(WorldManagerLane::Actor));
        }
        for id in &self.participant_order {
            let participant = self.participants.get_mut(id).unwrap();
            *participant.score =
                NativeHallOfFameRunState::new(self.hall_of_fame_clock_started_at_tick);
            *participant.tutorial_profile_economy = if world.tutorial.is_some() {
                let mut baseline = participant.player.economy.clone();
                if !baseline.college_intro_pending {
                    baseline.college_intro_pending = true;
                    baseline.revision += 1;
                }
                Some(Box::new(baseline))
            } else {
                None
            };
            *participant.wizard = world.spawn_wizard();
            *participant.college_intro = None;
            *participant.world_aim = None;
            participant.player.reset_for_new_run(true);
            reset_selected_primary(participant.wizard, participant.player);
            if world.tutorial.is_some() {
                participant.player.prepare_tutorial_loadout();
                reset_selected_primary(participant.wizard, participant.player);
            }
        }
        self.secondary.reset_world();
        if world.tutorial.is_some() {
            let fires = NATIVE_TUTORIAL_FIRES.map(|fire| NativeScriptFire {
                damage: fire.damage,
                lifetime_ticks: fire.lifetime_ticks,
                position: fire.position,
                radius: fire.radius,
            });
            self.secondary.spawn_script_fires(
                &self.participant_order[0],
                &format!("boneyard:{}", world.loaded.run_id),
                &fires,
                &mut self.painter_order,
            );
        }
        self.mod_effects.clear();
        self.inventory_dye_sessions.clear();
        self.spells = SpellState::default();
        self.screen_flashes.reset();
        self.run.start(world.loaded.run_id.clone(), &self.participant_order);
        self.level_up_barrier = None;
        self.boneyard = Some(world);
        Ok(())
    }

    /// Direct return resets the live run without transferring ground loot or restoring
    /// the completed Tutorial profile. Completed-run return follows the archive path.
    pub fn return_to_hub(&mut self) {
        let seed = self.gameplay_rng.draw_integer(0x4000_0000) as u32;
        self.rebuild_hub(seed);
        self.run = GameRunLifecycle::new();
    }

    pub(super) fn rebuild_hub(&mut self, seed: u32) {
        self.painter_order = WorldPainterOrder {
            next_actor_ordinal: HUB_FIRST_PLAYER_PAINTER_ORDINAL,
            next_transient_ordinal: 0,
        };
        for id in &self.participant_order {
            let participant = self.participants.get_mut(id).unwrap();
            *participant.lighting =
                PlayerLightingState::new(self.painter_order.register(WorldManagerLane::Actor));
            *participant.wizard = HubWizard::spawn();
            *participant.college_intro = None;
            participant.player.reset_for_new_run(false);
            reset_selected_primary(participant.wizard, participant.player);
            *participant.marker_epoch = HubNpcMarkerEpoch::new(
                seed,
                self.ticks,
                &participant.player.economy.npc.help_flags,
            );
            *participant.tutorial_profile_economy = None;
            *participant.world_aim = None;
        }
        self.npcs = HubNpcState::new(seed, Some(&mut self.painter_order));
        self.ambient = HubAmbientState::new();
        self.dowser_clock = HubCommonTraderClock::new(seed ^ 5016);
        self.hub_trader_animation_seed = seed;
        self.hub_memorial = crate::hub::memorial::create_hub_memorial_state();
        self.collision_rng = NATIVE_HUB_COLLISION_RNG_SEED;
        self.boneyard = None;
        self.level_up_barrier = None;
        self.mod_effects.clear();
        self.inventory_dye_sessions.clear();
        self.spells = SpellState::default();
        self.screen_flashes.reset();
        self.secondary.reset_world();
    }
}

pub(super) fn new_hub_participant(
    id: String,
    entity_id: u64,
    player: PlayerState,
    light: PainterRegistration,
    seed: u32,
    tick: u64,
) -> SessionParticipant {
    let marker_epoch = HubNpcMarkerEpoch::new(seed, tick, &player.economy.npc.help_flags);
    SessionParticipant {
        id,
        entity_id,
        player: Box::new(player),
        wizard: HubWizard::spawn(),
        lighting: PlayerLightingState::new(light),
        college_intro: None,
        marker_epoch,
        score: NativeHallOfFameRunState::new(tick),
        screen_feedback: NativeSecondaryScreenFeedbackPresentation::new(0, "hub:courtyard".into()),
        world_aim: None,
        tutorial_profile_economy: None,
    }
}

pub(super) fn reset_selected_primary(wizard: &mut HubWizard, player: &PlayerState) {
    let selected = if player.book.primary_skill_id == 52 {
        player.book.weld_build_id.expect("selected Weld has a build")
    } else {
        player.book.primary_skill_id as u32
    };
    wizard.primary_cast.selected_primary_id = Some(selected);
}
