//! Source-shaped wire adapters borrow canonical simulation birth data. The
//! connection byte budget therefore uses the same field/number spelling as JS.
use serde::{Deserialize, Serialize};
use solomon_sim::Vec2;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::spells::weld::{
    FlameChannelBirth, FlameChannelRole, FlameChannelVisual, FlameFadeBirth, FlameFadeVisual,
    FlamePathBirth, FlameRibbonBirth, WeldFlameLashFadeVariant,
};

#[derive(Serialize, Deserialize)]
#[serde(remote = "Vec2", deny_unknown_fields)]
pub(super) struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Deserialize)]
struct PointValue(#[serde(with = "Point")] Vec2);

pub(super) mod optional_point {
    use super::*;

    pub fn serialize<S>(value: &Option<Vec2>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.map(PointValue).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec2>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<PointValue>::deserialize(deserializer).map(|value| value.map(|point| point.0))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "WorldManagerLane", rename_all = "lowercase")]
pub(super) enum Lane {
    Actor,
    Transient,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "PainterRegistration", rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Registration {
    #[serde(with = "Lane")]
    pub manager_lane: WorldManagerLane,
    pub registration_ordinal: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "FlameRibbonBirth",
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(super) enum RibbonBirth {
    Short { width_factor: f64 },
    Long { phase_degrees: f64 },
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "FlamePathBirth", rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PathBirth {
    pub intensity: f64,
    pub radius_factors: Vec<f64>,
    #[serde(with = "Registration")]
    pub creator_registration: PainterRegistration,
    pub misc_append_ordinal: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "FlameChannelBirth", rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ChannelBirth {
    pub role: FlameChannelRole,
    #[serde(with = "optional_point")]
    pub camera_displacement: Option<Vec2>,
    #[serde(with = "Point")]
    pub start: Vec2,
    #[serde(with = "Point")]
    pub control: Vec2,
    #[serde(with = "Point")]
    pub end: Vec2,
    #[serde(with = "Point")]
    pub offset: Vec2,
    pub width: f64,
    #[serde(with = "RibbonBirth")]
    pub ribbon: FlameRibbonBirth,
    #[serde(with = "PathBirth")]
    pub path: FlamePathBirth,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "FlameChannelVisual", tag = "kind", deny_unknown_fields)]
pub(super) enum ChannelVisual {
    #[serde(rename = "legacy-suppressed")]
    LegacySuppressed,
    #[serde(rename = "native-birth-v1")]
    Birth(#[serde(with = "ChannelBirth")] FlameChannelBirth),
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "FlameFadeBirth", rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FadeBirth {
    pub birth_alpha: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "FlameFadeVisual", tag = "kind", deny_unknown_fields)]
pub(super) enum FadeVisual {
    #[serde(rename = "legacy-suppressed")]
    LegacySuppressed,
    #[serde(rename = "native-birth-v1")]
    Birth(#[serde(with = "FadeBirth")] FlameFadeBirth),
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "WeldFlameLashFadeVariant", rename_all = "lowercase")]
pub(super) enum FadeVariant {
    Chain,
    Endpoint,
    Source,
}
