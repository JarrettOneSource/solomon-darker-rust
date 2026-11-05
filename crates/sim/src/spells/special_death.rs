//! Ephemeral native +0x88 terminal context from `native-special-death-payload.ts`.
//! It is consumed by the admitted death dispatcher, never persisted as status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellSpecialDeathKind {
    Fire,
    Steam,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpellSpecialDeathPayload {
    pub kind: SpellSpecialDeathKind,
    pub explode_damage: f64,
    pub explode_radius: f64,
    pub ember_damage: f64,
    pub ember_fragments: u32,
    pub private_seed: u32,
}
