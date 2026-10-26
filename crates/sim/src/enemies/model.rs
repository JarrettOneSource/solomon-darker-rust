//! The enemy actor, family brains and combat messages from Website `enemies/model.ts`.
//!
//! Rules live in the family modules. Public fields retain the reference's names so the
//! renderer, wave director and spell contacts can consume one actor row without adapters.

use super::config::EnemyConfig;
use super::path::NativeEnemyPathState;
use crate::Vec2;
pub use crate::combat::PuppetHitState as PuppetHit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyLifeState {
    Alive,
    Dying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SkeletonAction {
    Claw,
    Pike,
    Weapon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SkeletonPhase {
    Approach,
    Attack,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum ArcherPhase {
    RangeControl,
    Attack,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum MagePhase {
    RangeControl,
    Cast,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum MageCastProgram {
    Long,
    Short,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum FlightPhase {
    Flight,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum ZombiePhase {
    Approach,
    Swipe,
    Knockback,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum DemonPhase {
    Approach,
    Bomb,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum CoffinPhase {
    Hidden,
    Rising,
    Holding,
    Opening,
    Open,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PortalPhase {
    Active,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BossPhase {
    RangeControl,
    Cast,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HeartmongerPhase {
    Approach,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SpiderPhase {
    Active,
    Death,
    Captured,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum CocoonPhase {
    Active,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum FacultyActionKind {
    Throw,
    TwoHand,
    Lightning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum CrowPhase {
    Orbit,
    Dive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum DemonSkullRangedAttack {
    Eyes,
    Mouth,
    Spit,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SkeletonPike {
    pub player_id: String,
    pub position: Vec2,
    pub distance: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SkeletonBrain {
    pub action_progress: f64,
    pub marker_emitted: bool,
    pub vertical_offset: f64,
    pub vertical_velocity: f64,
    pub action: SkeletonAction,
    pub contact_target_player_id: Option<String>,
    pub pike: Option<SkeletonPike>,
    pub phase: SkeletonPhase,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ArcherStrafeState {
    pub direction: i8,
    pub limb_heading_deg: f64,
    pub movement_ramp: f64,
    pub turn_blend: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ArcherBrain {
    pub action_progress: f64,
    pub marker_emitted: bool,
    pub vertical_offset: f64,
    pub vertical_velocity: f64,
    pub aim_seed: u32,
    pub attack_range: f64,
    pub phase: ArcherPhase,
    pub range_easy_pending: bool,
    pub strafe: ArcherStrafeState,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct MageBrain {
    pub action_progress: f64,
    pub marker_emitted: bool,
    pub vertical_offset: f64,
    pub vertical_velocity: f64,
    pub attack_range: f64,
    pub cast_program: MageCastProgram,
    pub cast_roll: f64,
    pub disabled_primary_ticks: u64,
    pub lightning_target_player_id: Option<String>,
    pub lightning_target_position: Option<Vec2>,
    pub lightning_ticks_remaining: u64,
    pub phase: MagePhase,
    pub range_easy_pending: bool,
    pub shield_ticks_remaining: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ImpBrain {
    pub base_horizontal_speed: f64,
    pub body_rotation_deg: f64,
    pub body_variant: usize,
    pub effect_alpha: f64,
    pub effect_phase: f64,
    pub horizontal_speed: f64,
    pub vertical_offset: f64,
    pub vertical_velocity: f64,
    pub escape_heading_deg: Option<f64>,
    pub phase: FlightPhase,
    pub visual_rng_state: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ZombieBrain {
    pub action_progress: f64,
    pub action_rate: f64,
    pub action_swing: f64,
    pub angular_offset_deg: f64,
    pub attack_side: u8,
    pub body_phase_deg: f64,
    pub body_type: u8,
    pub contact_target_player_id: Option<String>,
    pub front_arm_base_rotation_deg: f64,
    pub head_base_rotation_deg: f64,
    pub head_phase_deg: f64,
    pub head_type: u8,
    pub impact_state_ticks_remaining: u64,
    pub marker_emitted: bool,
    pub phase: ZombiePhase,
    pub phase_ticks_remaining: u64,
    pub rear_arm_base_rotation_deg: f64,
    pub vertical_offset: f64,
    pub vertical_velocity: f64,
    pub visual_rng_state: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WraithBrain {
    /// Actor +0x36 is cleared by every living Wraith tick, even when movement is held.
    pub collision_recipient: bool,
    pub base_flyby_speed: f64,
    pub contact_cooldown_ticks: u64,
    pub current_speed: f64,
    pub current_turn_gain: f64,
    pub flyby_ticks_remaining: u64,
    pub resting_speed: f64,
    pub target_turn_gain: f64,
    pub phase: FlightPhase,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DemonExtremity {
    pub current: Vec2,
    pub lift_y: f64,
    pub phase: f64,
    pub start: Vec2,
    pub target: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DemonArticulation {
    pub completed_steps: u64,
    pub front: DemonExtremity,
    pub front_base_rotation_deg: f64,
    pub rear: DemonExtremity,
    pub rear_base_rotation_deg: f64,
    pub step_interval_ticks: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DemonBrain {
    pub action_progress: f64,
    pub articulation: DemonArticulation,
    pub marker_emitted: bool,
    pub phase: DemonPhase,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct CoffinBrain {
    pub anchor_position: Vec2,
    pub launch_rotation_deg: f64,
    pub launch_scale: i8,
    pub maggot_charge: f64,
    pub phase: CoffinPhase,
    pub phase_tick: u64,
    pub phase_ticks_remaining: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PortalBrain {
    pub anchor_position: Option<Vec2>,
    pub age_ticks: u64,
    pub alpha: f64,
    pub aura_phase: f64,
    pub body_phase: f64,
    pub fixed_scale: f64,
    pub ticks_until_ejection: u64,
    pub hurt_ticks_remaining: u64,
    pub phase: PortalPhase,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DemonSkullBrain {
    pub body_heading_deg: f64,
    pub body_offset: Vec2,
    pub body_phase_deg: f64,
    pub body_pose: f64,
    pub charge_glow: f64,
    pub eye_charge: f64,
    pub flair_glow: f64,
    pub flicker_phase_deg: f64,
    pub jitter: Vec2,
    pub light_intensity: f64,
    pub spin: f64,
    pub attack_ticks_remaining: i64,
    pub capabilities: u8,
    pub heading_delay_ticks: u64,
    pub pending_attack: Option<DemonSkullRangedAttack>,
    pub recoil: Vec2,
    pub scream_active: bool,
    pub seen: bool,
    pub speed: f64,
    pub target_speed: f64,
    pub death_countdown: u64,
    pub actions: Vec<DemonSkullAction>,
    pub phase: BossPhase,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DemonSkullAction {
    pub retired: bool,
    pub program: DemonSkullActionProgram,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum DemonSkullActionProgram {
    Scream,
    Bite {
        progress: f64,
        rate: f64,
        marker_emitted: bool,
    },
    Eyes {
        warmup_ticks: u64,
        shots_remaining: usize,
        recovery_ticks: u64,
        attack_speed: f64,
    },
    Mouth {
        warmup_ticks: u64,
        warmup_jitter: f64,
        warmup_turn_speed: f64,
        beam_power: f64,
        tracking_delay_ticks: i64,
        tracking_ramp: f64,
        recovery_ticks: i64,
    },
    Spit {
        shots_remaining: usize,
        cooldown_ticks: i64,
        mouth_ticks: i64,
        target_heading_deg: f64,
    },
    Flair {
        remaining_ticks: u64,
        settling_ticks: u64,
        glow: f64,
        original_heading_deg: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct FacultyAction {
    pub end: f64,
    pub frames: Vec<f64>,
    pub hand_mask: u8,
    pub kind: FacultyActionKind,
    pub marker: f64,
    pub progress: f64,
    pub rate: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct FacultyBrain {
    pub body_heading_deg: f64,
    pub action: Option<FacultyAction>,
    pub attack_range: f64,
    pub body_pose: f64,
    pub completed_dispatches: usize,
    pub disabled_primary_ticks: u64,
    pub gait_phase: f64,
    pub hand_mask: u8,
    pub heading_locked: bool,
    pub lightning_active: bool,
    pub light_intensity: f64,
    pub light_phase: f64,
    pub orbit_sign: f64,
    pub primary_poll: u64,
    /// Continues below zero until the next successful secondary selection.
    pub secondary_cooldown: i64,
    pub phase: BossPhase,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct CrowState {
    pub age: u64,
    pub dive_speed: f64,
    pub flap_cycles: f64,
    pub flap_phase: f64,
    pub glide_ticks: u64,
    pub heading_deg: f64,
    pub height: f64,
    pub height_per_distance: f64,
    pub id: usize,
    pub light_intensity: f64,
    pub maximum_speed: f64,
    pub orbit_phase_deg: f64,
    pub orbit_radius: f64,
    pub phase: CrowPhase,
    pub position: Option<Vec2>,
    pub speed: f64,
    pub target_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HeartmongerBrain {
    pub activated: bool,
    pub crows: Vec<CrowState>,
    pub crow_cooldown_ticks: u64,
    pub head_variant: u8,
    pub summon_ticks_remaining: u64,
    pub phase: HeartmongerPhase,
    pub leg_phase: f64,
    pub torso_phase: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SpiderBrain {
    pub action_state: u8,
    pub preferred_distance: f64,
    pub body_heading_deg: f64,
    pub move_ticks_remaining: i64,
    pub strafe_sign: f64,
    pub strafe_ticks_remaining: i64,
    pub action_ticks_remaining: i64,
    pub frame: f64,
    pub spit_ticks_remaining: u64,
    pub attached: bool,
    pub vertical_offset: f64,
    pub attachment_angle_deg: f64,
    pub attachment_distance: f64,
    pub suck_ticks: u64,
    pub phase: SpiderPhase,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct CocoonBrain {
    pub phase: CocoonPhase,
    pub owner_player_id: Option<String>,
    pub owner_position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyBrain {
    Skeleton(SkeletonBrain),
    Archer(ArcherBrain),
    Mage(MageBrain),
    Imp(ImpBrain),
    Zombie(ZombieBrain),
    Wraith(WraithBrain),
    Demon(DemonBrain),
    Coffin(CoffinBrain),
    Portal(PortalBrain),
    DemonSkull(DemonSkullBrain),
    Faculty(FacultyBrain),
    Heartmonger(HeartmongerBrain),
    Spider(SpiderBrain),
    Cocoon(CocoonBrain),
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyLighting {
    pub charge: f64,
    pub glow: f64,
    pub provider_copies: u8,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyActor {
    pub ether_drain_captured: bool,
    pub hit_feedback: PuppetHit,
    /// Native +0x80 controls movement independently of visible feedback +0x78.
    pub hit_reaction_timer: f64,
    pub blizzard_push_accumulator: f64,
    pub blizzard_push_last_tick: Option<u64>,
    pub body_gait_phase: f64,
    pub body_pose: f64,
    pub brain: EnemyBrain,
    pub config: EnemyConfig,
    pub current_health: f64,
    pub death_epoch: Option<u64>,
    pub death_presentation_started: bool,
    pub death_started_tick: Option<u64>,
    pub death_tick: u64,
    pub gait_pose: f64,
    pub head_facing_offset: i8,
    pub heading_deg: f64,
    pub hurricane_contact_cooldown: f64,
    pub id: usize,
    pub last_damaged_by_player_id: Option<String>,
    pub last_damage_tick: Option<u64>,
    pub last_movement_tick: Option<u64>,
    pub lethal_magic_damage: bool,
    pub life_state: EnemyLifeState,
    pub registration_ordinal: usize,
    pub lighting: EnemyLighting,
    pub loot_seed: u32,
    pub next_movement_tick: u64,
    pub next_target_refresh_tick: u64,
    pub native_cell_binding_order: usize,
    pub native_registration_order: usize,
    pub path: NativeEnemyPathState,
    pub position: Vec2,
    pub reward_granted: bool,
    pub rest_body_pose: f64,
    pub shield_health: f64,
    pub shadow_lateral_offset: f64,
    pub shield_maximum_health: f64,
    pub shield_pulse: f64,
    pub shield_sound_cooldown_ticks: u64,
    pub source_spawn_intent_id: usize,
    pub spawn_tick: u64,
    pub staff_action_factor: f64,
    pub staff_movement_factor: f64,
    pub stride_phase_deg: f64,
    pub target_player_id: Option<String>,
    pub terminal_emitted: bool,
    pub wave_ordinal: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyTarget {
    pub id: String,
    pub summoned: bool,
    pub alive: bool,
    pub collision_radius: f64,
    pub connected: bool,
    pub eligible: bool,
    pub heading_deg: Option<f64>,
    pub position: Vec2,
    pub velocity_per_tick: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyDamageRequest {
    /// Source omission enables enhanced births; this is call policy, not world state.
    pub enhanced_effects: Option<bool>,
    pub hit_strength: f64,
    pub ether_drain_capture: bool,
    pub magic: bool,
    pub actor_id: usize,
    pub amount: f64,
    pub has_magic_damage: bool,
    pub source_player_id: Option<String>,
    pub suppress_hit_reaction: bool,
    pub suppress_hurt_sound: bool,
    pub tick: u64,
}

#[cfg(feature = "persistence")]
pub(super) const fn default_enhanced_effects() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyDamageResult {
    pub accepted: bool,
    pub health_damage: f64,
    pub killed: bool,
    /// Native +0x88 terminal dispatch; releasing a Cocoon is a distinct event.
    pub special_death_accepted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyPlayerDamageSource {
    pub position: Vec2,
    pub collision_radius: f64,
    pub reflectable_actor_id: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyPlayerDamage {
    pub hit_strength: Option<f64>,
    pub webbed_strength: Option<f64>,
    pub actor_id: usize,
    pub physical_damage: f64,
    pub magic_damage: f64,
    pub cold_slow_ticks: u64,
    pub dazzle_ticks: u64,
    pub event_id: usize,
    pub poison_damage: f64,
    pub poison_duration: f64,
    pub poison_contact_damage: Option<f64>,
    pub suppress_hit_response: bool,
    pub suppress_flash: bool,
    pub player_id: String,
    /// Omitted infers the actor; explicit null deliberately suppresses that source.
    #[cfg_attr(
        feature = "persistence",
        serde(
            default,
            deserialize_with = "deserialize_damage_source",
            skip_serializing_if = "Option::is_none"
        )
    )]
    pub source: Option<Option<EnemyPlayerDamageSource>>,
    pub mana_damage_maximum_fraction: Option<f64>,
    pub tragic_circle: bool,
    pub crow_blind_chance_percent: Option<f64>,
}

// As with EnemyEvent.target_player_id, ordinary nested Option deserialization
// would merge an omitted source with the explicit null used by Spider Silk.
#[cfg(feature = "persistence")]
fn deserialize_damage_source<'de, D>(
    deserializer: D,
) -> Result<Option<Option<EnemyPlayerDamageSource>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <Option<EnemyPlayerDamageSource> as serde::Deserialize>::deserialize(deserializer).map(Some)
}

impl EnemyPlayerDamage {
    #[must_use]
    pub fn new(actor_id: usize, event_id: usize, player_id: String) -> Self {
        Self {
            hit_strength: None,
            webbed_strength: None,
            actor_id,
            physical_damage: 0.0,
            magic_damage: 0.0,
            cold_slow_ticks: 0,
            dazzle_ticks: 0,
            event_id,
            poison_damage: 0.0,
            poison_duration: 0.0,
            poison_contact_damage: None,
            suppress_hit_response: false,
            suppress_flash: false,
            player_id,
            source: None,
            mana_damage_maximum_fraction: None,
            tragic_circle: false,
            crow_blind_chance_percent: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyPlayerKnockback {
    pub actor_id: usize,
    pub delta: Vec2,
    pub event_id: usize,
    pub player_id: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyRetirement {
    pub actor_id: usize,
    pub event_id: usize,
}

/// Terminal reward source from `enemies/model.ts`; actual drop selection belongs to
/// the world's loot store. Authored recipes alone carry policies and a recipe UID.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyLootSource {
    pub actor_seed: u32,
    pub enemy_token: super::EnemyToken,
    pub on_death_program: Option<crate::boneyard::loot::BoneyardLootDeathProgram>,
    pub policies: Option<super::config::EnemyLootPolicies>,
    pub participant_slot: u8,
    pub position: Vec2,
    pub recipe_uid: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyReward {
    pub actor_id: usize,
    pub event_id: usize,
    pub experience: f64,
    pub loot_source: EnemyLootSource,
    pub player_id: Option<String>,
}

pub use crate::painter::PainterRegistration as EnemyPainterRegistration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemySound {
    SpiderDie,
    BansheeDie,
    CoffinBreak,
    DemonDie,
    FireyDeath,
    Flash,
    ImpSplit,
    MaggotSqueak1,
    MaggotSqueak2,
    MaggotSquish1,
    MaggotSquish2,
    MaggotSquish3,
    PortalDie,
    SkeletonDie,
    ZombieDie,
    ZombieDieGroan,
    ZombiePoisonSplat,
    ArmorCrash1,
    ArmorCrash2,
    ArmorCrash3,
    BoneCrack,
    HitShield,
    PopShield,
    PortalHurt,
    ZombieOuch,
    WizardOuch1,
    WizardOuch2,
    WizardOuch3,
    Disintegrate,
    ShootWeb1,
    ShootWeb2,
    ShootWeb3,
    Webbed1,
    Webbed2,
    Bite1,
    Bite2,
    Bite3,
    ImpVocal1,
    ImpVocal2,
    ImpVocal3,
    ImpVocal4,
    ImpVocal5,
    ImpVocal6,
    ImpVocal7,
    ImpVocal8,
    FireballHit,
    PortalOpen,
    ShootArrow,
    MagicMissileHit,
    ThrowFire,
    ThrowSpell,
    SpitFire,
    Knock,
    ThrowDark,
    ChainClank1,
    ChainClank2,
    SpinAttack,
    MagicStorm,
    MagicShieldExplode,
    DistortReality,
    EyeLaserCharge,
    Jaw,
    SkullBite,
    UnholySpits,
    MagicCircle,
    BigFire,
    LightningStart,
    FlameLashStart,
    Crow1,
    Crow2,
    Wings,
    Frosted,
    Freeze,
    Poisoned,
    Blind,
}
impl EnemySound {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SpiderDie => "spider-die",
            Self::BansheeDie => "banshee-die",
            Self::CoffinBreak => "coffin-break",
            Self::DemonDie => "demon-die",
            Self::FireyDeath => "firey-death",
            Self::Flash => "flash",
            Self::ImpSplit => "imp-split",
            Self::MaggotSqueak1 => "maggot-squeak-1",
            Self::MaggotSqueak2 => "maggot-squeak-2",
            Self::MaggotSquish1 => "maggot-squish-1",
            Self::MaggotSquish2 => "maggot-squish-2",
            Self::MaggotSquish3 => "maggot-squish-3",
            Self::PortalDie => "portal-die",
            Self::SkeletonDie => "skeleton-die",
            Self::ZombieDie => "zombie-die",
            Self::ZombieDieGroan => "zombie-die-groan",
            Self::ZombiePoisonSplat => "zombie-poison-splat",
            Self::ArmorCrash1 => "armor-crash-1",
            Self::ArmorCrash2 => "armor-crash-2",
            Self::ArmorCrash3 => "armor-crash-3",
            Self::BoneCrack => "bone-crack",
            Self::HitShield => "hit-shield",
            Self::PopShield => "pop-shield",
            Self::PortalHurt => "portal-hurt",
            Self::ZombieOuch => "zombie-ouch",
            Self::WizardOuch1 => "wizard-ouch-1",
            Self::WizardOuch2 => "wizard-ouch-2",
            Self::WizardOuch3 => "wizard-ouch-3",
            Self::Disintegrate => "disintegrate",
            Self::ShootWeb1 => "shoot-web-1",
            Self::ShootWeb2 => "shoot-web-2",
            Self::ShootWeb3 => "shoot-web-3",
            Self::Webbed1 => "webbed-1",
            Self::Webbed2 => "webbed-2",
            Self::Bite1 => "bite-1",
            Self::Bite2 => "bite-2",
            Self::Bite3 => "bite-3",
            Self::ImpVocal1 => "imp-vocal-1",
            Self::ImpVocal2 => "imp-vocal-2",
            Self::ImpVocal3 => "imp-vocal-3",
            Self::ImpVocal4 => "imp-vocal-4",
            Self::ImpVocal5 => "imp-vocal-5",
            Self::ImpVocal6 => "imp-vocal-6",
            Self::ImpVocal7 => "imp-vocal-7",
            Self::ImpVocal8 => "imp-vocal-8",
            Self::FireballHit => "fireball-hit",
            Self::PortalOpen => "portal-open",
            Self::ShootArrow => "shoot-arrow",
            Self::MagicMissileHit => "magic-missile-hit",
            Self::ThrowFire => "throw-fire",
            Self::ThrowSpell => "throw-spell",
            Self::SpitFire => "spit-fire",
            Self::Knock => "knock",
            Self::ThrowDark => "throw-dark",
            Self::ChainClank1 => "chain-clank-1",
            Self::ChainClank2 => "chain-clank-2",
            Self::SpinAttack => "spin-attack",
            Self::MagicStorm => "magic-storm",
            Self::MagicShieldExplode => "magic-shield-explode",
            Self::DistortReality => "distort-reality",
            Self::EyeLaserCharge => "eye-laser-charge",
            Self::Jaw => "jaw",
            Self::SkullBite => "skull-bite",
            Self::UnholySpits => "unholy-spits",
            Self::MagicCircle => "magic-circle",
            Self::BigFire => "big-fire",
            Self::LightningStart => "lightning-start",
            Self::FlameLashStart => "flame-lash-start",
            Self::Crow1 => "crow-1",
            Self::Crow2 => "crow-2",
            Self::Wings => "wings",
            Self::Frosted => "frosted",
            Self::Freeze => "freeze",
            Self::Poisoned => "poisoned",
            Self::Blind => "blind",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyEventKind {
    CocoonReleased,
    PlayerStatusSound,
    PlayerCharmLost,
    PlayerCheatDeath,
    AttackMarker,
    CoffinMaggotRelease,
    EnemyActionSound,
    EnemyDeath,
    EnemyDeathSound,
    EnemyDamageSound,
    PlayerDamageSound,
    EnemyRetired,
    EnemySpawned,
    EnemyTerminalOutput,
    ProjectileImpact,
    ProjectileRetired,
    PlayerDeflected,
    MageLightningContact,
    ProjectileSpawned,
    Reward,
    EnemyDialogueStop,
    EnemyStream,
    EnemyScreenFlash,
    EnemyCameraShake,
}
impl EnemyEventKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::CocoonReleased => "cocoon-released",
            Self::PlayerStatusSound => "player-status-sound",
            Self::PlayerCharmLost => "player-charm-lost",
            Self::PlayerCheatDeath => "player-cheat-death",
            Self::AttackMarker => "attack-marker",
            Self::CoffinMaggotRelease => "coffin-maggot-release",
            Self::EnemyActionSound => "enemy-action-sound",
            Self::EnemyDeath => "enemy-death",
            Self::EnemyDeathSound => "enemy-death-sound",
            Self::EnemyDamageSound => "enemy-damage-sound",
            Self::PlayerDamageSound => "player-damage-sound",
            Self::EnemyRetired => "enemy-retired",
            Self::EnemySpawned => "enemy-spawned",
            Self::EnemyTerminalOutput => "enemy-terminal-output",
            Self::ProjectileImpact => "projectile-impact",
            Self::ProjectileRetired => "projectile-retired",
            Self::PlayerDeflected => "player-deflected",
            Self::MageLightningContact => "mage-lightning-contact",
            Self::ProjectileSpawned => "projectile-spawned",
            Self::Reward => "reward",
            Self::EnemyDialogueStop => "enemy-dialogue-stop",
            Self::EnemyStream => "enemy-stream",
            Self::EnemyScreenFlash => "enemy-screen-flash",
            Self::EnemyCameraShake => "enemy-camera-shake",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyTerminalOutput {
    SpiderCollapse,
    DiscorporealBanish,
    FacultyBreak,
    ArcherShatter,
    CoffinBreak,
    DemonSplit,
    HeartmongerShatter,
    ImpSplit,
    MageShatter,
    PortalBreak,
    SkeletonShatter,
    WraithFragments,
    ZombieCollapse,
}
impl EnemyTerminalOutput {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SpiderCollapse => "spider-collapse",
            Self::DiscorporealBanish => "discorporeal-banish",
            Self::FacultyBreak => "faculty-break",
            Self::ArcherShatter => "archer-shatter",
            Self::CoffinBreak => "coffin-break",
            Self::DemonSplit => "demon-split",
            Self::HeartmongerShatter => "heartmonger-shatter",
            Self::ImpSplit => "imp-split",
            Self::MageShatter => "mage-shatter",
            Self::PortalBreak => "portal-break",
            Self::SkeletonShatter => "skeleton-shatter",
            Self::WraithFragments => "wraith-fragments",
            Self::ZombieCollapse => "zombie-collapse",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyEffectAtlas {
    BadGuys,
    DeadHawg,
    Demon,
    Unholy,
}
impl EnemyEffectAtlas {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BadGuys => "BadGuys",
            Self::DeadHawg => "DeadHawg",
            Self::Demon => "Demon",
            Self::Unholy => "Unholy",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyEffectBlend {
    Normal,
    Add,
}
impl EnemyEffectBlend {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Add => "add",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyDeathEffectKind {
    BoulderBit,
    Banish,
    Bouncer,
    SmokyBouncer,
    Fade,
    FadeAdditive,
    FadePerspective,
    FadePerspectiveClipped,
    FadeScalePerspective,
    FadeScale,
    FireArray,
    LateSplat,
    MoveFade,
    MoveFadePerspective,
    SpriteArray,
    Unbind,
    BanishBlack,
    Scrap,
    BlackSmokyBouncer,
    MoveFadeSin,
}
impl EnemyDeathEffectKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BoulderBit => "boulder-bit",
            Self::Banish => "banish",
            Self::Bouncer => "bouncer",
            Self::SmokyBouncer => "smoky-bouncer",
            Self::Fade => "fade",
            Self::FadeAdditive => "fade-additive",
            Self::FadePerspective => "fade-perspective",
            Self::FadePerspectiveClipped => "fade-perspective-clipped",
            Self::FadeScalePerspective => "fade-scale-perspective",
            Self::FadeScale => "fade-scale",
            Self::FireArray => "fire-array",
            Self::LateSplat => "late-splat",
            Self::MoveFade => "move-fade",
            Self::MoveFadePerspective => "move-fade-perspective",
            Self::SpriteArray => "sprite-array",
            Self::Unbind => "unbind",
            Self::BanishBlack => "banish-black",
            Self::Scrap => "scrap",
            Self::BlackSmokyBouncer => "black-smoky-bouncer",
            Self::MoveFadeSin => "move-fade-sin",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum DeathPresentationOwner {
    Background,
    DirectPostWorld,
    PreWorldQueue,
    WorldSorted,
    LateWorldOverlay,
}
impl DeathPresentationOwner {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::DirectPostWorld => "direct-post-world",
            Self::PreWorldQueue => "pre-world-queue",
            Self::WorldSorted => "world-sorted",
            Self::LateWorldOverlay => "late-world-overlay",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum CameraShakeAttenuation {
    Fixed,
    Point,
    Hit,
    HitSquared,
}
impl CameraShakeAttenuation {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Point => "point",
            Self::Hit => "hit",
            Self::HitSquared => "hit-squared",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ScrapOscillation {
    pub phase_deg: f64,
    pub step_deg: f64,
    pub amplitude_deg: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyCameraShake {
    pub attenuation: CameraShakeAttenuation,
    pub displacement: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyScreenFlash {
    pub alpha: f64,
    pub blue: f64,
    pub decay_per_tick: f64,
    pub green: f64,
    pub point_attenuated: bool,
    pub red: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyEvent {
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub camera_shake: Option<EnemyCameraShake>,
    pub actor_id: usize,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub count: Option<usize>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub deflect_pitch: Option<f64>,
    pub event_id: usize,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub gain_scale: Option<f64>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub output: Option<EnemyTerminalOutput>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub painter_registration: Option<EnemyPainterRegistration>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub pitch: Option<f64>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub projectile_id: Option<usize>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub sound: Option<EnemySound>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub source_position: Option<Vec2>,
    #[cfg_attr(
        feature = "persistence",
        serde(
            default,
            deserialize_with = "deserialize_event_target",
            skip_serializing_if = "Option::is_none"
        )
    )]
    pub target_player_id: Option<Option<String>>,
    pub tick: u64,
    pub kind: EnemyEventKind,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub stream: Option<String>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub screen_flash_only_if_clear: Option<bool>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub screen_flash: Option<EnemyScreenFlash>,
}

// Source events distinguish an omitted target from an explicitly untargeted impact.
// Serde's ordinary nested Option reader collapses both to None.
#[cfg(feature = "persistence")]
fn deserialize_event_target<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <Option<String> as serde::Deserialize>::deserialize(deserializer).map(Some)
}

impl EnemyEvent {
    #[must_use]
    pub fn new(kind: EnemyEventKind, actor_id: usize, tick: u64) -> Self {
        Self {
            kind,
            actor_id,
            tick,
            event_id: 0,
            camera_shake: None,
            count: None,
            deflect_pitch: None,
            gain_scale: None,
            output: None,
            painter_registration: None,
            pitch: None,
            projectile_id: None,
            sound: None,
            source_position: None,
            target_player_id: None,
            stream: None,
            screen_flash_only_if_clear: None,
            screen_flash: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct EnemyDeathEffect {
    pub age_ticks: u64,
    pub alpha: f64,
    pub alpha_multiplier: f64,
    pub alpha_loss_per_tick: f64,
    pub angular_velocity_deg: f64,
    pub atlas: EnemyEffectAtlas,
    pub blend_mode: EnemyEffectBlend,
    pub bounce_retention: f64,
    pub bounce_velocity: f64,
    pub entry: usize,
    pub first_entry: usize,
    pub frame_count: usize,
    pub frame_phase: f64,
    pub frame_velocity: f64,
    pub frame_velocity_damping: f64,
    pub frame_ticks: f64,
    pub height: f64,
    pub id: usize,
    pub kind: EnemyDeathEffectKind,
    pub last_step_tick: Option<u64>,
    pub lifetime_ticks: u64,
    pub opacity_timer: f64,
    pub owner_actor_id: usize,
    pub painter_registration: Option<EnemyPainterRegistration>,
    pub presentation_owner: DeathPresentationOwner,
    pub position: Vec2,
    pub role: String,
    pub rotation_deg: f64,
    pub scale: f64,
    pub scale_y: f64,
    pub scale_multiplier: f64,
    pub shadow: bool,
    pub spawn_tick: u64,
    pub tint: u32,
    pub vertical_velocity: f64,
    pub velocity: Vec2,
    pub velocity_damping: f64,
    pub painter_sort_bias: Option<f64>,
    pub scrap_oscillation: Option<ScrapOscillation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum MaggotLaunchTrajectory {
    Edge,
    Lid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum MaggotMovementPhase {
    Crawl,
    Emerging,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WebbedState {
    pub severity: f64,
    pub strength: f64,
    pub cocoon_health: f64,
    pub hit_pulse: f64,
}

impl EnemyDamageRequest {
    #[must_use]
    pub fn new(actor_id: usize, amount: f64, tick: u64, source_player_id: Option<String>) -> Self {
        Self {
            enhanced_effects: None,
            actor_id,
            amount,
            tick,
            source_player_id,
            hit_strength: 1.0,
            ether_drain_capture: false,
            magic: false,
            has_magic_damage: false,
            suppress_hit_reaction: false,
            suppress_hurt_sound: false,
        }
    }
}

impl EnemyDamageResult {
    pub const REJECTED: Self =
        Self { accepted: false, health_damage: 0.0, killed: false, special_death_accepted: false };
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyMaggotActor {
    pub ether_drain_captured: bool,
    pub hit_feedback: PuppetHit,
    pub blizzard_push_accumulator: f64,
    pub blizzard_push_last_tick: Option<u64>,
    pub combat_active: bool,
    /// Native category/grid admission used by scenery presentation.
    pub native_tree_query_member: bool,
    pub collision_radius: f64,
    pub current_health: f64,
    pub death_offsets: Vec<Vec2>,
    pub damage: f64,
    pub death_epoch: Option<u64>,
    pub death_started_tick: Option<u64>,
    pub death_tick: u64,
    pub gait_pose: f64,
    pub heading_deg: f64,
    pub hurricane_contact_cooldown: f64,
    pub id: usize,
    pub emergence_tick: u64,
    pub emergence_phase: f64,
    pub launch_trajectory: MaggotLaunchTrajectory,
    pub launch_velocity: Vec2,
    pub landing_bounce_velocity: f64,
    pub last_attack_tick: Option<u64>,
    pub last_damaged_by_player_id: Option<String>,
    pub last_damage_tick: Option<u64>,
    pub last_movement_tick: Option<u64>,
    pub life_state: EnemyLifeState,
    pub registration_ordinal: usize,
    pub maximum_health: f64,
    pub next_attack_tick: u64,
    pub next_movement_tick: u64,
    pub next_target_refresh_tick: u64,
    pub native_cell_binding_order: usize,
    pub native_registration_order: usize,
    pub owner_coffin_actor_id: usize,
    pub path: NativeEnemyPathState,
    pub poison_damage: f64,
    pub poison_duration: f64,
    pub position: Vec2,
    pub movement_phase: MaggotMovementPhase,
    pub spawn_tick: u64,
    pub staff_action_factor: f64,
    pub staff_movement_factor: f64,
    pub target_player_id: Option<String>,
    pub terminal_emitted: bool,
    pub vertical_offset: f64,
    pub vertical_velocity: f64,
    pub visual_scale: f64,
}
