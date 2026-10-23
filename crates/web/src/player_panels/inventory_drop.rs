//! Current `HubInventoryActions.tsx`: send the pure drop plan through real UI owners.

use solomon_sim::audio::AudioCue;
use solomon_sim::economy::{
    HubEconomyRejection, HubInventoryAction, HubTraderId, HubTransferGesture, TransferDirection,
};
use solomon_sim::hud::{HudRect, native_hud_control_layout};
use solomon_sim::player_panels::inventory_drop::{
    InventoryDropContext, InventoryDropEffect, InventoryDropSource, drop_inventory_source,
};

use super::{PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels, Press, Target};

/// The source locks inventory transitions, while footer navigation can still close the book.
pub(super) fn locks_target(target: Target) -> bool {
    matches!(
        target,
        Target::Back
            | Target::Item(_)
            | Target::Cell(_)
            | Target::Equipment(_)
            | Target::Belt(_)
            | Target::Unforge
            | Target::RenameSack
            | Target::Shop(_)
            | Target::Hagatha(_)
            | Target::Storage(_)
            | Target::ShopCell
            | Target::OwnedPerk(_)
            | Target::Dowse
    )
}

impl PlayerPanels {
    pub(super) fn drop(
        &mut self,
        press: Press,
        destination: Option<Target>,
        snapshot: &PlayerPanelSnapshot<'_>,
        now_ms: f64,
    ) -> Result<(), String> {
        self.transfer_double_activation = false;
        let source = match press.target {
            Target::Item(item_id) => Some(InventoryDropSource::Backpack { item_id }),
            Target::Equipment(slot) => press
                .inventory_item_id
                .map(|item_id| InventoryDropSource::Equipment { slot, item_id }),
            _ => None,
        };
        if let Some(source) = source {
            return self.drop_inventory_item(source, snapshot, now_ms);
        }
        match (press.target, destination) {
            (Target::Storage(item_id), Some(Target::Cell(_) | Target::Item(_))) => {
                self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::Transfer {
                    item_id,
                    direction: TransferDirection::ToBackpack,
                    gesture: HubTransferGesture::Drag,
                }));
            }
            (Target::Skill(skill_id), Some(Target::Belt(slot))) => {
                self.sound(AudioCue::PickSkill);
                self.actions.push(PlayerPanelAction::Belt(
                    solomon_sim::belt::NativeBeltCommand::BindSkill {
                        skill_id: Some(skill_id),
                        slot,
                    },
                ));
            }
            _ => {}
        }
        Ok(())
    }

    fn drop_inventory_item(
        &mut self,
        source: InventoryDropSource,
        snapshot: &PlayerPanelSnapshot<'_>,
        now_ms: f64,
    ) -> Result<(), String> {
        if self.inventory_transition_locked() {
            return Ok(());
        }
        let belt_rects = native_hud_control_layout(1600.0, 900.0, Some(1.0)).belt;
        let companion = matches!(self.panel, Some(PlayerPanel::Service(_)));
        // These are the settled companion panels' existing physical drop surfaces,
        // expressed in the same native logical stage as the captured pointer.
        let storage_drop_rect = (self.panel == Some(PlayerPanel::Service(HubTraderId::Luthacus)))
            .then_some(HudRect::new(539.0, 56.5, 522.0, 297.0));
        let reference_drop_rect = (self.panel == Some(PlayerPanel::Service(HubTraderId::Shlorio))
            && !snapshot.economy.dowsing_rolled)
            .then_some(HudRect::new(750.0, 101.0, 100.0, 149.0));
        let effects = drop_inventory_source(
            source,
            self.pointer,
            &InventoryDropContext {
                economy: snapshot.economy,
                sack_path: &self.sack_path,
                belt_rects: &belt_rects,
                belt_start: self.hotbar.start(),
                companion,
                storage_drop_rect,
                reference_drop_rect,
            },
        );
        for effect in effects {
            match effect {
                InventoryDropEffect::Action(action) => {
                    self.actions.push(PlayerPanelAction::Inventory(action));
                }
                InventoryDropEffect::BindBelt(command) => {
                    self.sound(AudioCue::PickSkill);
                    self.actions.push(PlayerPanelAction::Belt(command));
                }
                InventoryDropEffect::Flyby(request) => {
                    self.start_inventory_flyby(request, snapshot, now_ms)?;
                }
                InventoryDropEffect::Sound { cue, playback_rate } => {
                    self.sound_at_rate(cue, playback_rate);
                }
                InventoryDropEffect::SelectReference(item_id) => {
                    self.dowsing_reference_item = Some(item_id);
                    self.sound(AudioCue::BackpackOpen);
                    self.sound(AudioCue::BackpackClose);
                }
                InventoryDropEffect::ConfirmUnforge(item_id) => {
                    self.begin_unforge(item_id, snapshot);
                }
                InventoryDropEffect::RequiredClothing(slot) => {
                    self.show_inventory_rejection(
                        &HubInventoryAction::Unequip {
                            slot,
                            destination_sack_id: None,
                            destination_slot: None,
                        },
                        HubEconomyRejection::RequiredClothing,
                    );
                }
            }
        }
        Ok(())
    }
}
