//! The Dark Cloud menu's tab strip: the five authored tab bands and their bounded,
//! horizontally scrollable view scale.

use crate::hud::HudRect;

pub const DARK_CLOUD_TAB_STRIP_WIDTH: f64 = 1122.0;
pub const DARK_CLOUD_TAB_STRIP_HEIGHT: f64 = 69.0;

/// A Dark Cloud tab: mods, subscribed mods, parties, layouts or Boneyards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DarkCloudTab {
    Mods,
    Subscribed,
    Parties,
    Layouts,
    Boneyards,
}

impl DarkCloudTab {
    pub const ALL: [Self; 5] =
        [Self::Mods, Self::Subscribed, Self::Parties, Self::Layouts, Self::Boneyards];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Mods => "mods",
            Self::Subscribed => "subscribed",
            Self::Parties => "parties",
            Self::Layouts => "layouts",
            Self::Boneyards => "boneyards",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mods => "MODS",
            Self::Subscribed => "SUBSCRIBED MODS",
            Self::Parties => "PARTIES",
            Self::Layouts => "LAYOUTS",
            Self::Boneyards => "BONEYARDS",
        }
    }

    fn band(self) -> (f64, f64) {
        match self {
            Self::Mods => (0.0, 170.0),
            Self::Subscribed => (170.0, 340.0),
            Self::Parties => (510.0, 170.0),
            Self::Layouts => (680.0, 202.0),
            Self::Boneyards => (882.0, 240.0),
        }
    }
}

/// The laid-out tab strip: its width, height and scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DarkCloudTabStrip {
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

impl DarkCloudTabStrip {
    #[must_use]
    pub fn bounds(self, tab: DarkCloudTab) -> HudRect {
        let (left, width) = tab.band();
        HudRect::new(
            left * self.width / DARK_CLOUD_TAB_STRIP_WIDTH,
            0.0,
            width * self.width / DARK_CLOUD_TAB_STRIP_WIDTH,
            self.height,
        )
    }
}

/// Below a scale of 0.4 the labels stop being readable, so the strip stays wider and
/// scrolls instead of shrinking the fifth tab out of view.
#[must_use]
pub fn dark_cloud_tab_strip(width: f64, height: f64) -> DarkCloudTabStrip {
    let scale = 1.0_f64
        .min(height / DARK_CLOUD_TAB_STRIP_HEIGHT)
        .min((width / DARK_CLOUD_TAB_STRIP_WIDTH).max(0.4));
    DarkCloudTabStrip { width: width.max(DARK_CLOUD_TAB_STRIP_WIDTH * scale), height, scale }
}
