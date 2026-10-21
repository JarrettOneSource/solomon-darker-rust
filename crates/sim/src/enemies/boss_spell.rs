//! Persistent Faculty and Heartmonger spell rows from `native-boss-spell.ts`.

use super::faculty_spells::{NativeDarkFireballState, NativeDireFireState, NativeRainOfBonesState};
use super::guided_missile::NativeGuidedMissileState;
use crate::Vec2;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BossSpellOwner {
    pub age_ticks: u64,
    pub damage: f64,
    pub id: usize,
    pub painter_registration: PainterRegistration,
    pub owner_actor_id: usize,
    pub position: Vec2,
    pub spawn_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BossSpellLight {
    pub radius: f64,
    pub intensity: f64,
    pub loss_per_tick: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BossSpellKind {
    UltraBanish {
        remaining_ticks: u64,
        alpha: f64,
        flash_alpha: f64,
        light_radius: f64,
        mega_death: bool,
    },
    UnholySoul {
        origin: Vec2,
        angle_deg: f64,
        angular_speed: f64,
        scale: f64,
        height: f64,
        vertical_speed: f64,
        life: f64,
        fast: bool,
    },
    EyeLaser {
        heading_deg: f64,
        phase_deg: f64,
        velocity: Vec2,
    },
    UnholySpit {
        origin: Vec2,
        travel: Vec2,
        spread_direction: Vec2,
        progress: f64,
        progress_per_tick: f64,
        height: f64,
        spawn_fire: bool,
        spawn_imps: bool,
    },
    GreenFire {
        fire: Box<crate::spells::model::PrimarySpellFirePatch>,
        glow: bool,
    },
    UnholyBurst {
        frame_phase: f64,
        frame_velocity: f64,
        offset_y: f64,
        offset_step_y: f64,
        light_intensity: f64,
    },
    MouthBeamSegment {
        vertices: [Vec2; 4],
        uv_offset: f64,
        start_alpha: f64,
        end_alpha: f64,
    },
    HeartmongerFlicker {
        phase_deg: f64,
    },
    HeartmongerSoul {
        life_phase_deg: f64,
        bob_phase_deg: f64,
    },
    Blightning {
        endpoint: Vec2,
        midpoint: Vec2,
        #[cfg_attr(
            feature = "persistence",
            serde(default = "super::model::default_enhanced_effects")
        )]
        enhanced_effects: bool,
    },
    DeathMagic {
        scale: f64,
        alpha: f64,
        alpha_loss_per_tick: f64,
        painter_sort_bias: f64,
        light: Option<BossSpellLight>,
    },
    FallingBone {
        entry: usize,
        height: f64,
        color_ramp: f64,
        rotation_deg: f64,
    },
    SkullMissile {
        heading_deg: f64,
        minimum_speed: f64,
        phase_deg: f64,
        remaining_ticks: f64,
        speed: f64,
        turn_rate: f64,
        visual_scale: f64,
        target_player_id: Option<String>,
    },
    DarkFireball {
        arc_phase: f64,
        arc_phase_step: f64,
        heading_deg: f64,
        remaining_ticks: f64,
        velocity: Vec2,
        ground_fire_damage: f64,
    },
    RainOfBones {
        alpha: f64,
        phase: f64,
        remaining_ticks: i64,
        rotation_sign: f64,
        scale: f64,
    },
    TragicCircle {
        remaining_ticks: u64,
    },
    DireFire(NativeDireFireState),
}

impl BossSpellKind {
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::UltraBanish { .. } => "ultra-banish",
            Self::UnholySoul { .. } => "unholy-soul",
            Self::EyeLaser { .. } => "eye-laser",
            Self::UnholySpit { .. } => "unholy-spit",
            Self::GreenFire { .. } => "green-fire",
            Self::UnholyBurst { .. } => "unholy-burst",
            Self::MouthBeamSegment { .. } => "mouth-beam-segment",
            Self::HeartmongerFlicker { .. } => "heartmonger-flicker",
            Self::HeartmongerSoul { .. } => "heartmonger-soul",
            Self::Blightning { .. } => "blightning",
            Self::DeathMagic { .. } => "death-magic",
            Self::FallingBone { .. } => "falling-bone",
            Self::SkullMissile { .. } => "skull-missile",
            Self::DarkFireball { .. } => "dark-fireball",
            Self::RainOfBones { .. } => "rain-of-bones",
            Self::TragicCircle { .. } => "tragic-circle",
            Self::DireFire(_) => "dire-fire",
        }
    }

    #[must_use]
    pub const fn manager_lane(&self) -> WorldManagerLane {
        match self {
            Self::UnholySpit { .. } | Self::GreenFire { .. } => WorldManagerLane::Actor,
            Self::SkullMissile { .. }
            | Self::DarkFireball { .. }
            | Self::RainOfBones { .. }
            | Self::TragicCircle { .. }
            | Self::DireFire(_) => WorldManagerLane::Actor,
            _ => WorldManagerLane::Transient,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeBossSpell {
    pub owner: BossSpellOwner,
    pub kind: BossSpellKind,
}

impl NativeBossSpell {
    #[must_use]
    pub fn guided_missile_state(&self) -> NativeGuidedMissileState {
        let BossSpellKind::SkullMissile {
            heading_deg,
            minimum_speed,
            phase_deg,
            remaining_ticks,
            speed,
            turn_rate,
            visual_scale,
            ..
        } = self.kind
        else {
            panic!("Skull Missile state required");
        };
        NativeGuidedMissileState {
            heading_deg,
            minimum_speed,
            phase_deg,
            position: self.owner.position,
            remaining_ticks,
            speed,
            turn_rate,
            visual_scale,
        }
    }

    #[must_use]
    pub fn dark_fireball_state(&self) -> NativeDarkFireballState {
        let BossSpellKind::DarkFireball {
            arc_phase,
            arc_phase_step,
            heading_deg,
            remaining_ticks,
            velocity,
            ..
        } = self.kind
        else {
            panic!("Dark Fireball state required");
        };
        NativeDarkFireballState {
            age_ticks: self.owner.age_ticks,
            arc_phase,
            arc_phase_step,
            heading_deg,
            position: self.owner.position,
            remaining_ticks,
            velocity,
        }
    }

    #[must_use]
    pub fn rain_state(&self) -> NativeRainOfBonesState {
        let BossSpellKind::RainOfBones { alpha, phase, remaining_ticks, rotation_sign, scale } =
            self.kind
        else {
            panic!("Rain of Bones state required");
        };
        NativeRainOfBonesState {
            alpha,
            phase,
            position: self.owner.position,
            remaining_ticks,
            rotation_sign,
            scale,
        }
    }

    pub fn set_guided_missile_state(&mut self, state: NativeGuidedMissileState) {
        let BossSpellKind::SkullMissile { target_player_id, .. } = &self.kind else {
            panic!("Skull Missile state required");
        };
        self.kind = BossSpellKind::SkullMissile {
            heading_deg: state.heading_deg,
            minimum_speed: state.minimum_speed,
            phase_deg: state.phase_deg,
            remaining_ticks: state.remaining_ticks,
            speed: state.speed,
            turn_rate: state.turn_rate,
            visual_scale: state.visual_scale,
            target_player_id: target_player_id.clone(),
        };
        self.owner.position = state.position;
    }

    pub fn set_dark_fireball_state(&mut self, state: NativeDarkFireballState) {
        let BossSpellKind::DarkFireball { ground_fire_damage, .. } = self.kind else {
            panic!("Dark Fireball state required");
        };
        self.kind = BossSpellKind::DarkFireball {
            arc_phase: state.arc_phase,
            arc_phase_step: state.arc_phase_step,
            heading_deg: state.heading_deg,
            remaining_ticks: state.remaining_ticks,
            velocity: state.velocity,
            ground_fire_damage,
        };
        self.owner.age_ticks = state.age_ticks;
        self.owner.position = state.position;
    }
}

impl super::world::EnemyWorld {
    pub(super) fn create_boss_spell_owner(
        &mut self,
        owner_actor_id: usize,
        spawn_tick: u64,
        damage: f64,
        position: Vec2,
        lane: WorldManagerLane,
        order: &mut WorldPainterOrder,
    ) -> BossSpellOwner {
        let id = self.next_projectile_id;
        self.next_projectile_id += 1;
        BossSpellOwner {
            age_ticks: 0,
            damage,
            id,
            painter_registration: order.register(lane),
            owner_actor_id,
            position,
            spawn_tick,
        }
    }
    pub(super) fn create_boss_spell(
        &mut self,
        owner_actor_id: usize,
        spawn_tick: u64,
        damage: f64,
        position: Vec2,
        kind: BossSpellKind,
        order: &mut WorldPainterOrder,
    ) -> NativeBossSpell {
        let owner = self.create_boss_spell_owner(
            owner_actor_id,
            spawn_tick,
            damage,
            position,
            kind.manager_lane(),
            order,
        );
        NativeBossSpell { owner, kind }
    }
}
