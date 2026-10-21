//! Current `game-host.ts` Enhanced Effects admission and leader-owned changes.

use super::{HostedGameHost, HostedWorlds};

impl HostedGameHost {
    pub(super) fn initialize_enhanced_effects(
        &mut self,
        id: &str,
        enabled: bool,
        resumed: bool,
        replaced: bool,
    ) {
        if replaced {
            return;
        }
        let shared = matches!(self.worlds, HostedWorlds::Global(_));
        if !shared && self.players.len() == 1 && !resumed {
            self.worlds.state_for_player_mut(id).unwrap().set_enhanced_effects(enabled);
        }
        if self
            .worlds
            .parties()
            .party_for_player(id)
            .is_none_or(|party| party.leader_player_id != id)
        {
            return;
        }
        let state = self.worlds.state_for_player(id).unwrap();
        let enabled =
            if shared && state.boneyard.is_none() { enabled } else { state.enhanced_effects };
        self.worlds
            .parties_mut()
            .set_enhanced_effects(id, enabled)
            .expect("the admitted leader owns the selected party");
    }

    pub(in crate::host) fn set_enhanced_effects(&mut self, id: &str, enabled: bool) {
        if self
            .worlds
            .parties()
            .party_for_player(id)
            .is_none_or(|party| party.leader_player_id != id)
        {
            self.social_dirty = true;
            self.publish_dirty = true;
            return;
        }
        self.worlds
            .parties_mut()
            .set_enhanced_effects(id, enabled)
            .expect("the checked leader owns the selected party");
        let active = self.worlds.state_for_player(id).is_some_and(|state| state.boneyard.is_some());
        if !matches!(self.worlds, HostedWorlds::Global(_)) || active {
            self.worlds.state_for_player_mut(id).unwrap().set_enhanced_effects(enabled);
            self.pending_player_saves.extend(self.players.iter().map(|player| player.id.clone()));
        }
        self.social_dirty = true;
        self.publish_dirty = true;
    }
}
