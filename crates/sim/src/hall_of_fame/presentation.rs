//! Native Hall box/row geometry from `hall-of-fame-presentation.ts` (`0x005A2C80`).
//! Coordinates are inside the 1200×695 box at stage (200,80), before viewport scaling.

use crate::Vec2;
use crate::float;
use crate::hud::HudRect;

pub const HALL_BOX: HudRect = HudRect { x: 200.0, y: 80.0, width: 1200.0, height: 695.0 };
pub const HALL_ROW_HEIGHT: f64 = 250.0;
pub const HALL_EXPANDED_HEIGHT: f64 = 150.0;
pub const HALL_CONTENT_TOP: f64 = 80.0;
pub const HALL_TILE_SIZE: f64 = 264.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HallFont {
    Body,
    Heading,
    Medium,
    Menu,
}

impl HallFont {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Heading => "heading",
            Self::Medium => "medium",
            Self::Menu => "menu",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HallTextAlign {
    Center,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HallTextAnchor {
    pub align: HallTextAlign,
    pub font: HallFont,
    pub position: Vec2,
}

impl HallTextAnchor {
    const fn new(x: f64, y: f64, font: HallFont, align: HallTextAlign) -> Self {
        Self { position: Vec2::new(x, y), font, align }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HallChevronPlacement {
    pub position: Vec2,
    pub rotation: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HallSkillCellLayout {
    pub center: Vec2,
    pub numeral: HallTextAnchor,
}

impl HallSkillCellLayout {
    #[must_use]
    pub fn badge(self, numeral_width: f64) -> HudRect {
        HudRect {
            x: self.center.x + 22.0 - numeral_width,
            y: self.center.y + 11.0,
            width: numeral_width + 3.0,
            height: 15.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HallExpandedLayout {
    pub awesomest_kill: HallTextAnchor,
    pub awesomest_label: HallTextAnchor,
    pub highest_skills: HallTextAnchor,
    pub kills_frame: HudRect,
    pub monsters_killed: HallTextAnchor,
    pub perk_centers: [Vec2; 9],
    pub perks_used: HallTextAnchor,
    pub skill_cells: [HallSkillCellLayout; 3],
    pub survival: HallTextAnchor,
    pub time_label: HallTextAnchor,
    pub time_value: HallTextAnchor,
    pub top: f64,
    pub wave_label: HallTextAnchor,
    pub wave_value: HallTextAnchor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HallRowLayout {
    pub account: HallTextAnchor,
    pub awesomeness: HallTextAnchor,
    pub chevron: HallChevronPlacement,
    pub expanded: Option<HallExpandedLayout>,
    pub height: f64,
    pub highlight: HudRect,
    pub level: HallTextAnchor,
    pub name: HallTextAnchor,
    pub ornament: Vec2,
    pub rank: HallTextAnchor,
    pub separator_y: f64,
    pub wizard: Vec2,
}

#[must_use]
pub fn hall_row_layout(
    row_top: f64,
    expanded: bool,
    rank_width: f64,
    awesomeness_width: f64,
) -> HallRowLayout {
    use HallFont::{Body, Heading, Medium, Menu};
    use HallTextAlign::Center;

    let pen = row_top + 15.0;
    let rank_baseline = pen + HALL_ROW_HEIGHT / 2.0 - 65.0;
    let height = row_height(expanded);
    HallRowLayout {
        account: HallTextAnchor::new(600.0, pen + 170.0, Body, Center),
        awesomeness: HallTextAnchor::new(600.0, pen + 155.0, Medium, Center),
        chevron: hall_chevron_placement(
            600.0 - awesomeness_width / 2.0 - 25.0,
            pen + 140.0,
            expanded,
        ),
        expanded: expanded.then(|| expanded_layout(pen + HALL_ROW_HEIGHT - 75.0)),
        height,
        highlight: HudRect { x: 50.0, y: row_top - 25.0, width: 1100.0, height: height - 10.0 },
        level: HallTextAnchor::new(600.0, pen + 140.0, Medium, Center),
        name: HallTextAnchor::new(600.0, pen + 125.0, Menu, Center),
        ornament: Vec2::new(540.0 - rank_width / 2.0 - 11.0, rank_baseline - 10.5),
        rank: HallTextAnchor::new(540.0, rank_baseline, Heading, Center),
        separator_y: row_top + height - 50.0,
        wizard: Vec2::new(600.0, pen + 73.0),
    }
}

#[must_use]
pub fn hall_chevron_placement(x: f64, y: f64, expanded: bool) -> HallChevronPlacement {
    let (half_width, half_height) = if expanded { (11.0, 10.0) } else { (10.0, 11.0) };
    // Native mirrored UI9 moves down one pixel when expanded; quads round halves upward.
    let left = (x - half_width + 0.5).floor();
    let top = (y - half_height + 0.5).floor() + if expanded { 1.0 } else { 0.0 };
    HallChevronPlacement {
        position: Vec2::new(left + half_width, top + half_height),
        rotation: if expanded { 180.0 } else { 90.0 },
    }
}

#[must_use]
pub fn hall_row_tops(expanded_rows: &[bool]) -> (Vec<f64>, f64) {
    let mut tops = Vec::with_capacity(expanded_rows.len());
    let mut cursor = HALL_CONTENT_TOP;
    for expanded in expanded_rows {
        tops.push(cursor);
        cursor += row_height(*expanded);
    }
    (tops, cursor)
}

#[must_use]
pub fn hall_scroll_extent(content_height: f64) -> f64 {
    (content_height - HALL_BOX.height).max(0.0)
}

#[must_use]
pub fn hall_current_row_scroll_target(row_top: f64, content_height: f64) -> f64 {
    (row_top - HALL_BOX.height * 0.25).clamp(0.0, hall_scroll_extent(content_height))
}

#[must_use]
pub fn hall_scroll_ease(tick: f64) -> f64 {
    float::sin(tick.clamp(0.0, 89.0) * std::f64::consts::PI / 180.0)
}

#[must_use]
pub fn hall_highlight_fill_alpha(tick: f64) -> f64 {
    0.1 + 0.05 * pulse(tick)
}

#[must_use]
pub fn hall_current_frame_alpha(tick: f64) -> f64 {
    0.5 + 0.2 * pulse(tick)
}

#[must_use]
pub fn hall_tile_offset(scroll_top: f64) -> f64 {
    scroll_top.rem_euclid(HALL_TILE_SIZE)
}

#[must_use]
pub fn hall_visible_row_range(
    tops: &[f64],
    expanded_rows: &[bool],
    scroll_top: f64,
) -> (usize, usize) {
    let window_top = scroll_top - HALL_ROW_HEIGHT;
    let window_bottom = scroll_top + HALL_BOX.height + HALL_ROW_HEIGHT;
    let mut start = tops.len();
    let mut end = 0;
    for (index, top) in tops.iter().enumerate() {
        let bottom = top + row_height(expanded_rows[index]);
        if bottom < window_top || top - 25.0 > window_bottom {
            continue;
        }
        start = start.min(index);
        end = end.max(index + 1);
    }
    if start < end { (start, end) } else { (0, 0) }
}

#[must_use]
pub fn hall_separator_halves(y: f64) -> [HudRect; 2] {
    [
        HudRect { x: 150.0, y: y - 1.0, width: 450.0, height: 2.0 },
        HudRect { x: 600.0, y: y - 1.0, width: 450.0, height: 2.0 },
    ]
}

fn pulse(tick: f64) -> f64 {
    float::sin(tick * 3.0 * std::f64::consts::PI / 180.0)
}

fn row_height(expanded: bool) -> f64 {
    HALL_ROW_HEIGHT + if expanded { HALL_EXPANDED_HEIGHT } else { 0.0 }
}

fn expanded_layout(top: f64) -> HallExpandedLayout {
    use HallFont::{Body, Medium, Menu};
    use HallTextAlign::{Center, Left, Right};

    let kills_y = top + HALL_EXPANDED_HEIGHT / 2.0 - 25.0 + 8.0;
    let skill_row_y = top + 108.0;
    HallExpandedLayout {
        awesomest_kill: HallTextAnchor::new(625.0, kills_y + 40.0, Menu, Center),
        awesomest_label: HallTextAnchor::new(625.0, kills_y + 20.0, Medium, Center),
        highest_skills: HallTextAnchor::new(90.0, top + 70.0, Medium, Left),
        kills_frame: HudRect { x: 475.0, y: kills_y - 30.0, width: 300.0, height: 90.0 },
        monsters_killed: HallTextAnchor::new(625.0, kills_y, Medium, Center),
        perk_centers: std::array::from_fn(|index| {
            Vec2::new(1006.0 + 42.0 * (index % 3) as f64, top + 33.0 + 42.0 * (index / 3) as f64)
        }),
        perks_used: HallTextAnchor::new(1110.0, top, Medium, Right),
        skill_cells: std::array::from_fn(|index| {
            let cell_x = 90.0 + 60.0 * index as f64;
            HallSkillCellLayout {
                center: Vec2::new(cell_x + 30.0, skill_row_y),
                numeral: HallTextAnchor::new(cell_x + 53.0, skill_row_y + 22.0, Body, Right),
            }
        }),
        survival: HallTextAnchor::new(90.0, top, Medium, Left),
        time_label: HallTextAnchor::new(121.0, top + 20.0, Medium, Left),
        time_value: HallTextAnchor::new(170.0, top + 20.0, Medium, Left),
        top,
        wave_label: HallTextAnchor::new(110.0, top + 35.0, Medium, Left),
        wave_value: HallTextAnchor::new(170.0, top + 35.0, Medium, Left),
    }
}
