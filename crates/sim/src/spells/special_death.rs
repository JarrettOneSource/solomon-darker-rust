//! The detonation a damage contact can carry into the death it causes. This ephemeral
//! native +0x88 terminal context is consumed by the admitted death dispatcher, never
//! persisted as status.
/// Whether a special death detonates as Fire or as Steam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellSpecialDeathKind {
    Fire,
    Steam,
}
/// A special death's explosion and ember damage, radius, fragment count and private seed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpellSpecialDeathPayload {
    pub kind: SpellSpecialDeathKind,
    pub explode_damage: f64,
    pub explode_radius: f64,
    pub ember_damage: f64,
    pub ember_fragments: u32,
    pub private_seed: u32,
}
