//! Legal mask plans and interpreted intents from Source `ml-bot-policy/actions.ts`.

use crate::Vec2;
use crate::economy::HubInventoryAction;
use crate::float::hypot;
use crate::heading::heading_vector;
use crate::profile::primary::{NativePrimaryCastMode, native_primary_cast_mode};
use crate::profile::weld::{NativeWeldCastKind, native_weld_primary_cast_mode};
use crate::session::{GameSession, GameSessionInput};

use super::enemies::MlBotPolicyEnemyRow;
use super::numbers::directions;
use super::observer::MlBotPolicyFrame;
use super::spec::scales;
use super::world_state::MlBotPolicyCommittedAction;
use super::{MlBotPolicyActionIndices, MlBotPolicyActionMaskPlan, MlBotPolicyActionMasks};

/// Only the fields read by Source actions.ts. The asynchronous host retains this
/// request-time context, instead of cloning an entire Session or substituting the
/// completion-time player's position, book or life state.
#[derive(Clone, Copy, Debug)]
pub struct MlBotPolicyActionContext {
    position: Vec2,
    heading_index: usize,
    primary_skill_id: usize,
    weld_build_id: Option<u32>,
    accepts_input: bool,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyDecision {
    pub committed_action: MlBotPolicyCommittedAction,
    pub hub_action: Option<HubInventoryAction>,
    pub input: GameSessionInput,
    pub masks: MlBotPolicyActionMasks,
    pub target_id: Option<usize>,
}

impl MlBotPolicyActionContext {
    pub fn capture(session: &GameSession, player_id: &str) -> Result<Self, String> {
        let participant = session
            .participant(player_id)
            .ok_or_else(|| format!("ML bot policy action has no player {player_id}"))?;
        Ok(Self {
            position: participant.wizard.position,
            heading_index: participant.wizard.heading_index,
            primary_skill_id: participant.player.book.primary_skill_id,
            weld_build_id: participant.player.book.weld_build_id,
            accepts_input: participant.player.combat.can_accept_input(),
        })
    }

    pub fn masks(&self, frame: &MlBotPolicyFrame) -> Result<MlBotPolicyActionMaskPlan, String> {
        let gated = frame.player.block_a[22] == 1.0
            || frame.player.block_a[23] == 1.0
            || !self.accepts_input;
        let mut movement = vec![0; 9];
        let mut target = vec![0; 9];
        movement[0] = 1;
        target[0] = 1;
        if !gated {
            for index in 0..8 {
                movement[index + 1] = u8::from(
                    frame.geometry.patch_and_rays[index]
                        > (scales::RAY_STEP / scales::RAY_RANGE) as f32,
                );
            }
            target[0] = u8::from(frame.target_id.is_some() || frame.enemy_rows.is_empty());
            for slot in target.iter_mut().skip(1).take(frame.enemy_rows.len().min(8)) {
                *slot = 1;
            }
        }
        let primary_free = [16, 40].contains(&self.primary_skill_id)
            || self.primary_skill_id == 52
                && self.weld_build_id.is_some_and(|id| [1006, 1007, 1008, 1009].contains(&id));
        let continuing_primary = frame.player.block_a[6] == 1.0 && self.primary_continuous()?;
        let mut ability_by_target = Vec::with_capacity(9);
        for (target_action, target_legal) in target.iter().enumerate() {
            let mut ability = vec![0; 22];
            ability[0] = 1;
            if !gated && *target_legal == 1 {
                let target = selected_enemy(frame, target_action);
                let in_range = target.is_some_and(|target| {
                    (hypot(
                        target.position.x - self.position.x,
                        target.position.y - self.position.y,
                    ) - target.radius)
                        .max(0.0)
                        <= frame.player.primary_range
                });
                ability[1] = u8::from(
                    continuing_primary
                        || frame.player.block_a[7] == 1.0
                            && frame.player.primary_affordable
                            && (primary_free || in_range),
                );
                for (slot, secondary) in frame.player.secondary_slots.iter().enumerate() {
                    ability[slot + 2] =
                        u8::from(secondary.occupied && secondary.ready && secondary.affordable);
                }
                for (slot, potion) in frame.inventory.potions.iter().take(12).enumerate() {
                    ability[slot + 10] = u8::from(potion.legal);
                }
            }
            ability_by_target.push(ability);
        }
        let mut aim_by_ability = Vec::with_capacity(22);
        for ability_action in 0..22 {
            let mut aim = vec![0; 9];
            aim[0] = 1;
            let secondary = if (2..=9).contains(&ability_action) {
                frame.player.secondary_slots[ability_action - 2].skill_id
            } else {
                None
            };
            let free_aim = if ability_action == 1 {
                primary_free
            } else {
                secondary.is_some_and(|id| {
                    [11, 15, 16, 27, 45, 48, 49, 50, 72, 73, 74, 76, 77].contains(&id)
                })
            };
            if !gated && free_aim {
                aim.fill(1);
            }
            aim_by_ability.push(aim);
        }
        Ok(MlBotPolicyActionMaskPlan { ability_by_target, aim_by_ability, movement, target })
    }

    pub fn resolve(
        &self,
        frame: &MlBotPolicyFrame,
        action: MlBotPolicyActionIndices,
    ) -> Result<MlBotPolicyDecision, String> {
        validate_actions(action)?;
        let plan = self.masks(frame)?;
        require_legal(&plan.movement, action.movement, "movement")?;
        require_legal(&plan.target, action.target, "target")?;
        let selected = selected_enemy(frame, action.target);
        let target_id = selected.map(|target| target.id);
        let ability = &plan.ability_by_target[action.target];
        require_legal(ability, action.ability, "ability")?;
        let aim = &plan.aim_by_ability[action.ability];
        require_legal(aim, action.aim, "aim")?;
        let hub_action = (action.ability >= 10).then(|| HubInventoryAction::Consume {
            item_id: frame.inventory.potions[action.ability - 10].item_id,
        });
        let input = if hub_action.is_some() {
            GameSessionInput::default()
        } else {
            GameSessionInput {
                aim: (action.ability != 0).then(|| self.aim_point(selected, action.aim)),
                primary: action.ability == 1,
                quickbar: (2..=9).contains(&action.ability).then(|| action.ability - 2),
                movement: if action.movement == 0 {
                    Vec2::ZERO
                } else {
                    directions()[action.movement - 1]
                },
                ..GameSessionInput::default()
            }
        };
        Ok(MlBotPolicyDecision {
            committed_action: MlBotPolicyCommittedAction {
                ability_action: action.ability,
                movement: input.movement,
                target_action: action.target,
                target_switched: target_id != frame.target_id,
            },
            hub_action,
            input,
            masks: MlBotPolicyActionMasks {
                ability: ability.clone(),
                aim: aim.clone(),
                movement: plan.movement,
                target: plan.target,
            },
            target_id,
        })
    }

    fn primary_continuous(&self) -> Result<bool, String> {
        if self.primary_skill_id != 52 {
            return Ok(native_primary_cast_mode(self.primary_skill_id)
                == NativePrimaryCastMode::Continuous);
        }
        let id = self.weld_build_id.ok_or("ML bot policy selected Weld has no build")?;
        Ok(native_weld_primary_cast_mode(id) == NativeWeldCastKind::Channel)
    }

    fn aim_point(&self, target: Option<&MlBotPolicyEnemyRow>, aim: usize) -> Vec2 {
        if aim > 0 {
            let direction = directions()[aim - 1];
            let origin = target.map_or(self.position, |target| target.position);
            return Vec2::new(
                origin.x + direction.x * scales::AIM_OFFSET_WORLD,
                origin.y + direction.y * scales::AIM_OFFSET_WORLD,
            );
        }
        if let Some(target) = target {
            return target.position;
        }
        let direction = heading_vector(self.heading_index);
        Vec2::new(
            self.position.x + direction.x * scales::AIM_OFFSET_WORLD,
            self.position.y + direction.y * scales::AIM_OFFSET_WORLD,
        )
    }
}

pub fn create_ml_bot_policy_action_mask_plan(
    session: &GameSession,
    player_id: &str,
    frame: &MlBotPolicyFrame,
) -> Result<MlBotPolicyActionMaskPlan, String> {
    MlBotPolicyActionContext::capture(session, player_id)?.masks(frame)
}

pub fn resolve_ml_bot_policy_decision(
    session: &GameSession,
    player_id: &str,
    frame: &MlBotPolicyFrame,
    action: MlBotPolicyActionIndices,
) -> Result<MlBotPolicyDecision, String> {
    validate_actions(action)?;
    MlBotPolicyActionContext::capture(session, player_id)?.resolve(frame, action)
}

fn selected_enemy(frame: &MlBotPolicyFrame, target_action: usize) -> Option<&MlBotPolicyEnemyRow> {
    if target_action == 0 {
        return frame.target_id.and_then(|id| frame.enemy_rows.iter().find(|row| row.id == id));
    }
    frame.enemy_rows.get(target_action - 1)
}

fn validate_actions(action: MlBotPolicyActionIndices) -> Result<(), String> {
    for (head, value, count) in [
        ("movement", action.movement, 9),
        ("target", action.target, 9),
        ("ability", action.ability, 22),
        ("aim", action.aim, 9),
    ] {
        if value >= count {
            return Err(format!(
                "ML bot policy {head} action must be an integer within 0..{}",
                count - 1
            ));
        }
    }
    Ok(())
}

fn require_legal(mask: &[u8], action: usize, head: &str) -> Result<(), String> {
    if mask[action] == 1 {
        Ok(())
    } else {
        Err(format!("ML bot policy selected illegal {head} action {action}"))
    }
}
