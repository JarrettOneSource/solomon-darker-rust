//! GPU Party scroll/copy state. The host retains membership and directory authority.

use solomon_sim::social::party_ui::NativePartyMenuTab;

#[derive(Default)]
pub(in crate::social) struct PartyMenuInteraction {
    pub scroll_y: f64,
    pub copied: bool,
    copied_until: Option<f64>,
}
impl PartyMenuInteraction {
    pub fn select_tab(&mut self, _tab: NativePartyMenuTab, opening: bool) {
        if opening {
            self.scroll_y = 0.0;
        }
    }
    pub fn copied(&mut self, copied: bool, now: f64) {
        self.copied = copied;
        self.copied_until = copied.then_some(now + 1500.0);
    }
    pub fn expire_copy(&mut self, now: f64) {
        if self.copied_until.is_some_and(|until| now >= until) {
            self.copied = false;
            self.copied_until = None;
        }
    }
}
