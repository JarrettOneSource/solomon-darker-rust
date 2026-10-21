//! Queued command application from `host/lua/web-lua-game-api.ts`.

use crate::enemies::EnemySpawnIntent;
use crate::modding::extensions::GameSimulationExtensions;
use crate::session::GameSession;

use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppliedWebLuaCommands {
    pub enemy_spawn_intents: Vec<EnemySpawnIntent>,
    pub next_run_seed: Option<u32>,
}

pub fn apply_web_lua_commands(
    game: &mut GameSession,
    commands: &[WebLuaCommand],
    extensions: Option<&dyn GameSimulationExtensions>,
) -> AppliedWebLuaCommands {
    let mut applied = AppliedWebLuaCommands::default();
    for command in commands {
        if command.player_id().is_some_and(|id| !game.participants.contains_key(id)) {
            continue;
        }
        match command {
            WebLuaCommand::GrantExperience { amount, player_id } => {
                game.grant_experience_with_extensions(player_id, *amount, extensions);
            }
            WebLuaCommand::GrantGold { amount, player_id } => {
                if !amount.is_finite()
                    || amount.fract() != 0.0
                    || !(1.0..=9_007_199_254_740_991.0).contains(amount)
                {
                    continue;
                }
                let economy = &mut game.participants.get_mut(player_id).unwrap().player.economy;
                if economy.gold >= 10_000_000 {
                    continue;
                }
                let gold = (economy.gold as f64 + amount).min(10_000_000.0) as u64;
                if gold != economy.gold {
                    economy.gold = gold;
                    economy.revision += 1;
                }
            }
            WebLuaCommand::GrantItem { item_key, player_id, quantity } => {
                if let Some(items) = create_web_lua_developer_grant_items(item_key, *quantity) {
                    game.participants
                        .get_mut(player_id)
                        .unwrap()
                        .player
                        .economy
                        .grant_inventory_items(&items);
                }
            }
            WebLuaCommand::GrantSkill { player_id, ranks, skill_id } => {
                let Some(skill) = web_lua_developer_skill(*skill_id) else { continue };
                if skill.weld_only
                    || !ranks.is_finite()
                    || ranks.fract() != 0.0
                    || *ranks < 1.0
                    || *ranks > 9_007_199_254_740_991.0
                {
                    continue;
                }
                let participant = game.participants.get_mut(player_id).unwrap();
                participant.player.grant_skill_ranks(
                    *skill_id as usize,
                    (*ranks).min(f64::from(u32::MAX)) as u32,
                    &mut game.gameplay_rng,
                );
            }
            WebLuaCommand::GrantWeld { build_id, player_id } => {
                if web_lua_developer_weld(*build_id).is_none() {
                    continue;
                }
                let participant = game.participants.get_mut(player_id).unwrap();
                let before = (
                    participant.player.book.primary_skill_id,
                    participant.player.book.weld_build_id,
                );
                participant.player.grant_weld_build(*build_id, &mut game.gameplay_rng);
                let book = &participant.player.book;
                if before != (book.primary_skill_id, book.weld_build_id) {
                    let primary = book.weld_build_id.unwrap_or(book.primary_skill_id as u32);
                    participant.wizard.primary_cast.reset_for_selection(primary);
                }
            }
            WebLuaCommand::RestoreHealth { amount, player_id } => {
                game.participants.get_mut(player_id).unwrap().player.combat.restore_health(*amount);
            }
            WebLuaCommand::RestoreMana { amount, player_id } => {
                game.participants.get_mut(player_id).unwrap().player.combat.restore_mana(*amount);
            }
            WebLuaCommand::SetGold { player_id, value } => {
                let economy = &mut game.participants.get_mut(player_id).unwrap().player.economy;
                economy.gold = *value;
                economy.revision += 1;
            }
            WebLuaCommand::SetMana { player_id, value } => {
                let combat = &mut game.participants.get_mut(player_id).unwrap().player.combat;
                combat.set_mana(value.min(combat.maximum_mana));
            }
            WebLuaCommand::SetNextRunSeed { seed } => {
                applied.next_run_seed = Some(*seed);
            }
            WebLuaCommand::SpawnNextWave { run_id, wave_event_id } => {
                if !next_wave_available(game) || game.run.run_id.as_deref() != Some(run_id.as_str())
                {
                    continue;
                }
                let waves = game
                    .boneyard
                    .as_mut()
                    .expect("available Boneyard")
                    .waves
                    .as_mut()
                    .expect("available wave director");
                if waves.state.wave_event_id == *wave_event_id {
                    waves.spawn_next_wave();
                }
            }
            WebLuaCommand::SpawnEnemy { request_id, token, x, y } => {
                let Some(world) = &game.boneyard else { continue };
                applied.enemy_spawn_intents.push(EnemySpawnIntent::new(
                    *token,
                    Vec2::new(*x, *y),
                    0x4000_0000 + *request_id as usize,
                    game.ticks + 1,
                    world.waves.as_ref().map_or(0, |waves| waves.state.wave_ordinal),
                ));
            }
        }
    }
    applied
}
