//! Archived run records and stable ranking from `core-kernels/hall-of-fame.ts`.
//! The session supplies completed score/profile data; this module never awards score.

pub mod presentation;

use crate::profile::{WizardDiscipline, WizardElement};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameSkill {
    pub rank: u64,
    pub skill_id: u64,
}

/// Archived browser rows admit every Source safe integer. These widths do not
/// change the gameplay owners; stock art converts IDs only at its lookup boundary.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameEntry {
    pub account_username: Option<String>,
    pub awesomeness: u64,
    pub awesomest_kill: Option<String>,
    pub completed_at_utc: String,
    pub discipline: WizardDiscipline,
    pub elapsed_ticks: u64,
    pub element: WizardElement,
    pub heading_index: u64,
    pub highest_skills: Vec<HallOfFameSkill>,
    pub level: u64,
    pub monsters_killed: u64,
    pub perks_used: Vec<u64>,
    pub portrait_scale: f64,
    pub run_id: String,
    pub wave: u64,
    pub wizard_name: String,
}

/// `hall-of-fame-entry.ts`: only the archived death portrait completes a record.
/// Permanent ranks retain learned order as their tie-break, without equipment ranks.
#[must_use]
pub fn completed_hall_of_fame_entry(
    run_id: &str,
    phase: crate::game_run::GameRunPhase,
    score: &crate::score::NativeHallOfFameRunState,
    player: &crate::player::PlayerState,
    wave: u64,
    completed_at_utc: String,
) -> Option<HallOfFameEntry> {
    if phase != crate::game_run::GameRunPhase::GameOver {
        return None;
    }
    let mut highest_skills: Vec<_> = player
        .book
        .learned_skill_order
        .iter()
        .filter_map(|&skill_id| {
            let rank = player.book.permanent_ranks[skill_id];
            (rank > 0)
                .then_some(HallOfFameSkill { rank: u64::from(rank), skill_id: skill_id as u64 })
        })
        .collect();
    highest_skills.sort_by_key(|skill| std::cmp::Reverse(skill.rank));
    highest_skills.truncate(3);
    Some(HallOfFameEntry {
        account_username: None,
        awesomeness: score.awesomeness,
        awesomest_kill: score.awesomest_kill.clone(),
        completed_at_utc,
        discipline: player.selection.discipline,
        elapsed_ticks: score.elapsed_ticks?,
        element: player.selection.element,
        heading_index: score.portrait_heading_index? as u64,
        highest_skills,
        level: u64::from(player.progression.level),
        monsters_killed: score.monsters_killed,
        perks_used: player
            .economy
            .owned_perk_selectors
            .iter()
            .take(9)
            .map(|&selector| u64::from(selector))
            .collect(),
        portrait_scale: score.portrait_scale?,
        run_id: run_id.to_owned(),
        wave,
        wizard_name: player.selection.name.clone(),
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HallOfFameBoard {
    #[default]
    Awesomeness,
    Wave,
    Kills,
    Time,
}

impl HallOfFameBoard {
    pub const ALL: [Self; 4] = [Self::Awesomeness, Self::Wave, Self::Kills, Self::Time];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Awesomeness => "awesomeness",
            Self::Wave => "wave",
            Self::Kills => "kills",
            Self::Time => "time",
        }
    }

    fn value(self, entry: &HallOfFameEntry) -> u64 {
        match self {
            Self::Awesomeness => entry.awesomeness,
            Self::Wave => entry.wave,
            Self::Kills => entry.monsters_killed,
            Self::Time => entry.elapsed_ticks,
        }
    }
}

/// Stable source order breaks equal scores. Other boards use Awesomeness first as a tie-break.
#[must_use]
pub fn rank_hall_of_fame_entries(
    entries: &[HallOfFameEntry],
    board: HallOfFameBoard,
    limit: usize,
) -> Vec<&HallOfFameEntry> {
    let mut ranked: Vec<_> = entries.iter().collect();
    ranked.sort_by(|left, right| {
        let order = board.value(right).cmp(&board.value(left));
        if board == HallOfFameBoard::Awesomeness {
            order
        } else {
            order.then_with(|| right.awesomeness.cmp(&left.awesomeness))
        }
    });
    ranked.truncate(limit);
    ranked
}

#[must_use]
pub fn format_hall_of_fame_time(elapsed_ticks: u64) -> String {
    let seconds = elapsed_ticks / 100;
    format!("{}:{:02}:{:02}", seconds / 3600, seconds % 3600 / 60, seconds % 60)
}
