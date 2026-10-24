//! Source `GameChat.tsx`'s accepted additions to its polite, non-atomic live list.

use std::collections::BTreeSet;

#[derive(Default)]
pub(in crate::social) struct ChatAnnouncements {
    epoch: u64,
    last_sequence: Option<u64>,
    pending: BTreeSet<u64>,
}

impl ChatAnnouncements {
    pub(in crate::social) fn reset(&mut self) {
        self.epoch += 1;
        self.last_sequence = None;
        self.pending.clear();
    }

    pub(in crate::social) fn observe(
        &mut self,
        sequences: impl IntoIterator<Item = u64>,
        visible: bool,
    ) {
        let sequences: BTreeSet<_> = sequences.into_iter().collect();
        let newest = sequences.last().copied().unwrap_or(0);
        if let Some(previous) = self.last_sequence {
            self.pending.retain(|sequence| sequences.contains(sequence));
            if visible {
                self.pending.extend(sequences.into_iter().filter(|sequence| *sequence > previous));
            } else {
                self.pending.clear();
            }
            self.last_sequence = Some(previous.max(newest));
        } else {
            // Existing session history is initial content, not a new live-list addition.
            self.last_sequence = Some(newest);
        }
    }

    pub(in crate::social) fn take_visible(
        &mut self,
        sequences: impl IntoIterator<Item = u64>,
    ) -> Vec<u64> {
        let pending = std::mem::take(&mut self.pending);
        sequences.into_iter().filter(|sequence| pending.contains(sequence)).collect()
    }

    #[must_use]
    pub(in crate::social) fn owner(&self, suffix: &str) -> String {
        format!("social/chat:{}/{suffix}", self.epoch)
    }
}

#[cfg(test)]
mod tests {
    use super::ChatAnnouncements;

    #[test]
    fn initial_history_is_quiet_and_accepted_additions_keep_distinct_sequence_owners() {
        let mut announcements = ChatAnnouncements::default();
        announcements.observe([1, 2], true);
        assert!(announcements.take_visible([1, 2]).is_empty());
        announcements.observe([1, 2, 3, 4], true);
        assert_eq!(announcements.take_visible([1, 2, 3, 4]), vec![3, 4]);
        assert_ne!(announcements.owner("message:3"), announcements.owner("message:4"));
        assert!(announcements.take_visible([1, 2, 3, 4]).is_empty());
    }

    #[test]
    fn filtered_arrivals_do_not_replay_when_the_channel_changes() {
        let mut announcements = ChatAnnouncements::default();
        announcements.observe([], true);
        announcements.observe([1, 2], true);
        assert_eq!(announcements.take_visible([2]), vec![2]);
        announcements.observe([1, 2], true);
        assert!(announcements.take_visible([1, 2]).is_empty());
    }

    #[test]
    fn hidden_arrivals_and_retired_history_do_not_replay() {
        let mut announcements = ChatAnnouncements::default();
        announcements.observe([], true);
        announcements.observe([1], true);
        announcements.observe([1, 2], false);
        announcements.observe([1, 2], true);
        assert!(announcements.take_visible([1, 2]).is_empty());
        announcements.observe([2, 3], true);
        announcements.observe([4], true);
        assert_eq!(announcements.take_visible([3, 4]), vec![4]);
    }

    #[test]
    fn real_session_replacement_seeds_history_and_changes_message_and_status_owners() {
        let mut announcements = ChatAnnouncements::default();
        announcements.observe([], true);
        announcements.observe([1], true);
        assert_eq!(announcements.take_visible([1]), vec![1]);
        let message_owner = announcements.owner("message:1");
        let status_owner = announcements.owner("status");
        announcements.reset();
        announcements.observe([1], true);
        assert!(announcements.take_visible([1]).is_empty());
        announcements.observe([1, 2], true);
        assert_eq!(announcements.take_visible([1, 2]), vec![2]);
        assert_ne!(announcements.owner("message:1"), message_owner);
        assert_ne!(announcements.owner("status"), status_owner);
    }
}
