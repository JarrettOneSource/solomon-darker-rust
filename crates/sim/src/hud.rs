//! Screen-space HUD rules from Website `native-hud-layout.ts` and
//! `native-hud-presentation.ts`. `tests/hud_input_goldens.rs` replays their oracle.

pub mod belt_pull_off;
pub mod mobile;
pub mod quickbar;
pub mod skill_selector;

pub const NATIVE_HUD_BASE_HEALTH: f64 = 50.0;
pub const NATIVE_HUD_BASE_MANA: f64 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl HudRect {
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudControlLayout {
    pub backpack: HudRect,
    pub belt: [HudRect; 8],
    pub tome: HudRect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudHealthPresentation {
    pub core_width: f64,
    pub fill_progress: f64,
    pub fill_width: f64,
    pub shield_progress: f64,
    pub shield_width: f64,
    pub track_width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudManaPresentation {
    pub core_width: f64,
    pub fill_progress: f64,
    pub fill_width: f64,
    pub reserve_progress: f64,
    pub reserve_width: f64,
    pub track_width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudHealthLayerKind {
    Health,
    Shield,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudHealthLayer {
    pub kind: HudHealthLayerKind,
    pub progress: f64,
    pub record: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HudBeltEntry {
    Skill { record: usize, unavailable: bool },
    HealthPotion { count: u32 },
    ManaPotion { count: u32 },
    Item { record: Option<usize>, count: u32 },
}

/// Values supplied by the local simulation or an authoritative runtime, never invented by
/// the view. Icon records are stock Skills/Inventory records, not skill identifiers.
#[derive(Clone, Debug, PartialEq)]
pub struct HudState {
    pub current_health: f64,
    pub maximum_health: f64,
    pub current_mana: f64,
    pub maximum_mana: f64,
    pub shield_current: f64,
    pub shield_maximum: f64,
    pub reserved_mana: f64,
    pub poisoned: bool,
    pub experience_progress: f64,
    pub primary_icon_record: usize,
    pub concentration_icon_records: [Option<usize>; 2],
    pub belt: [Option<HudBeltEntry>; 8],
    pub gold: u32,
}

impl HudState {
    /// Website `createPlayerCombat`, `createPlayerSkillBook` (Ether), `createNativePlayerBelt`
    /// and `createHubEconomy`: level one, Magic Missile/Call Leviathan, one potion of each, 500 gold.
    #[must_use]
    pub const fn new_game() -> Self {
        Self {
            current_health: NATIVE_HUD_BASE_HEALTH,
            maximum_health: NATIVE_HUD_BASE_HEALTH,
            current_mana: NATIVE_HUD_BASE_MANA,
            maximum_mana: NATIVE_HUD_BASE_MANA,
            shield_current: 0.0,
            shield_maximum: 0.0,
            reserved_mana: 0.0,
            poisoned: false,
            experience_progress: 0.0,
            primary_icon_record: 35,
            concentration_icon_records: [None; 2],
            belt: [
                Some(HudBeltEntry::Skill { record: 38, unavailable: true }),
                None,
                None,
                Some(HudBeltEntry::HealthPotion { count: 1 }),
                Some(HudBeltEntry::ManaPotion { count: 1 }),
                None,
                None,
                None,
            ],
            gold: 500,
        }
    }
}

/// `nativeHudControlLayout` and `nativeHudModalSlideLayout`. `None` is the resting writer;
/// a modal writer at progress zero starts the belt half a pixel lower than the resting HUD.
#[must_use]
pub fn native_hud_control_layout(
    width: f64,
    height: f64,
    modal_progress: Option<f64>,
) -> HudControlLayout {
    assert!(
        width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0,
        "HUD needs a positive back-buffer size"
    );
    let slide = modal_progress.map_or(0.0, |progress| {
        assert!((0.0..=1.0).contains(&progress), "HUD modal progress must be within [0, 1]");
        15.0 * progress
    });
    let control_top = height - 75.0 + slide;
    let belt_top = control_top + if modal_progress.is_some() { 8.0 } else { 7.5 };
    let backpack = HudRect::new(width / 2.0 - 69.5, control_top, 58.0, 62.0);
    let tome = HudRect::new(width / 2.0 + 10.5, control_top, 58.0, 62.0);
    let belt = std::array::from_fn(|index| {
        let center = if index < 4 {
            backpack.x + 29.0 - 265.0 + 60.0 * index as f64
        } else {
            tome.x + 29.0 + 85.0 + 60.0 * (index - 4) as f64
        };
        HudRect::new(center - 26.5, belt_top, 53.0, 53.0)
    });
    HudControlLayout { backpack, belt, tome }
}

#[must_use]
pub fn native_health_hud_presentation(
    current: f64,
    maximum: f64,
    shield_current: f64,
    shield_maximum: f64,
) -> HudHealthPresentation {
    let core_width = 2.0 * (NATIVE_HUD_BASE_HEALTH + 0.25 * (maximum - NATIVE_HUD_BASE_HEALTH));
    let fill_progress = clamp_unit(current / maximum).powi(2);
    let shield_progress =
        if shield_maximum > 0.0 { clamp_unit(shield_current / shield_maximum) } else { 0.0 };
    HudHealthPresentation {
        core_width,
        fill_progress,
        fill_width: core_width * fill_progress,
        shield_progress,
        shield_width: core_width * shield_progress,
        track_width: core_width + 10.0,
    }
}

#[must_use]
pub fn native_health_hud_layers(health: f64, shield: f64, poisoned: bool) -> Vec<HudHealthLayer> {
    let record = if poisoned { 52 } else { 26 };
    let health =
        HudHealthLayer { kind: HudHealthLayerKind::Health, progress: clamp_unit(health), record };
    let shield =
        HudHealthLayer { kind: HudHealthLayerKind::Shield, progress: clamp_unit(shield), record };
    if shield.progress == 0.0 {
        return vec![health];
    }
    if shield.progress <= health.progress { vec![shield, health] } else { vec![health, shield] }
}

#[must_use]
pub fn native_mana_hud_presentation(
    current: f64,
    maximum: f64,
    reserved: f64,
) -> HudManaPresentation {
    let core_width = NATIVE_HUD_BASE_MANA + 0.25 * (maximum - NATIVE_HUD_BASE_MANA);
    let fill_progress = clamp_unit(current / maximum);
    let reserve_progress = clamp_unit(reserved / maximum);
    HudManaPresentation {
        core_width,
        fill_progress,
        fill_width: core_width * fill_progress,
        reserve_progress,
        reserve_width: core_width * reserve_progress,
        track_width: core_width + 10.0,
    }
}

#[must_use]
pub fn clamp_unit(value: f64) -> f64 {
    if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_health_uses_quarter_fill_while_half_mana_uses_half_fill() {
        assert_eq!(native_health_hud_presentation(25.0, 50.0, 0.0, 0.0).fill_width, 25.0);
        assert_eq!(native_mana_hud_presentation(50.0, 100.0, 0.0).fill_width, 50.0);
    }
}
