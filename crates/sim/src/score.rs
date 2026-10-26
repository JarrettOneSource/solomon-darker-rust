//! Run score, streak and archive portrait from `core-kernels/hall-of-fame-score.ts`.

use crate::NativeRng;
use crate::enemies::config::{
    ArrowType, EnemyConfig, EnemyFamilyConfig, MageElement, SkeletonWeapon,
};
use crate::float::fround;
use crate::heading::heading_index;

pub const NATIVE_SCORE_ARCHIVE_DEATH_TICK: u64 = 300;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct NativeHallOfFameRunState {
    pub awesomeness: u64,
    pub awesomest_kill: Option<String>,
    pub awesomest_kill_maximum_health: f64,
    pub elapsed_ticks: Option<u64>,
    pub kill_streak: u64,
    pub monsters_killed: u64,
    pub portrait_heading_index: Option<usize>,
    pub portrait_scale: Option<f64>,
    pub started_at_tick: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeHallOfFamePlayerScoreState {
    pub current_health: f64,
    pub maximum_health: f64,
    pub level: u32,
    pub score_health_multiplier_enabled: bool,
}

impl NativeHallOfFameRunState {
    #[must_use]
    pub fn new(started_at_tick: u64) -> Self {
        Self {
            awesomeness: 0,
            awesomest_kill: None,
            awesomest_kill_maximum_health: 0.0,
            elapsed_ticks: None,
            kill_streak: 0,
            monsters_killed: 0,
            portrait_heading_index: None,
            portrait_scale: None,
            started_at_tick,
        }
    }

    pub fn archive(
        &mut self,
        archive_tick: u64,
        portrait_heading_degrees: f64,
        portrait_scale: f64,
    ) {
        assert!(archive_tick >= self.started_at_tick, "score archive precedes the run");
        if self.elapsed_ticks.is_none() {
            self.elapsed_ticks = Some(archive_tick - self.started_at_tick);
            self.portrait_heading_index = Some(heading_index(portrait_heading_degrees));
            self.portrait_scale = Some(fround(portrait_scale));
        }
    }

    pub fn reset_kill_streak(&mut self) {
        self.kill_streak = 0;
    }

    pub fn record_enemy_kill(
        &mut self,
        enemy: &EnemyConfig,
        player: NativeHallOfFamePlayerScoreState,
        region_pulse_accumulator: f64,
        gameplay_rng: &mut NativeRng,
    ) {
        self.record_awesomest_kill(enemy, player, region_pulse_accumulator, gameplay_rng);
        self.record_ordinary_kill(player, region_pulse_accumulator);
    }

    /// Best-kill and ordinary-kill are separate source events during some terminal programs.
    pub fn record_awesomest_kill(
        &mut self,
        enemy: &EnemyConfig,
        player: NativeHallOfFamePlayerScoreState,
        region_pulse_accumulator: f64,
        gameplay_rng: &mut NativeRng,
    ) {
        if self.awesomest_kill_maximum_health > 0.0 {
            self.kill_streak += 1;
        }
        if enemy.maximum_health > self.awesomest_kill_maximum_health {
            let roll = gameplay_rng.draw_integer(5) as u64;
            self.awesomeness += native_hall_of_fame_award(
                71 + roll * u64::from(player.level),
                region_pulse_accumulator,
                player,
                self.kill_streak,
            );
            self.awesomest_kill = Some(native_hall_of_fame_enemy_name(enemy));
            self.awesomest_kill_maximum_health = enemy.maximum_health;
        }
    }

    pub fn record_ordinary_kill(
        &mut self,
        player: NativeHallOfFamePlayerScoreState,
        region_pulse_accumulator: f64,
    ) {
        self.awesomeness +=
            native_hall_of_fame_award(1, region_pulse_accumulator, player, self.kill_streak);
        self.monsters_killed += 1;
    }
}

#[must_use]
pub fn native_hall_of_fame_award(
    base_points: u64,
    region_pulse_accumulator: f64,
    player: NativeHallOfFamePlayerScoreState,
    kill_streak: u64,
) -> u64 {
    let pulse_gate = (region_pulse_accumulator + 0.5).min(1.0).trunc() as u64;
    let mut points = base_points * pulse_gate;
    if player.score_health_multiplier_enabled {
        if player.current_health < 0.0 {
            points *= 3;
        } else if player.current_health / player.maximum_health < 0.1_f32 as f64 {
            points *= 2;
        }
    }
    let level_window = u64::from(player.level) * 100;
    let multiplier = ((level_window + kill_streak) / level_window).clamp(1, 5);
    points * multiplier
}

#[must_use]
pub fn native_hall_of_fame_enemy_name(enemy: &EnemyConfig) -> String {
    if let Some(name) = &enemy.recipe_name {
        return name.clone();
    }
    match &enemy.family {
        EnemyFamilyConfig::DemonSkull { .. } => "The Discorporeal".into(),
        EnemyFamilyConfig::Faculty(_) => "Dire Faculty".into(),
        EnemyFamilyConfig::Heartmonger { .. } => "Heartmonger".into(),
        EnemyFamilyConfig::Skeleton { armor, headgear, weapon } => {
            if *armor && *weapon != SkeletonWeapon::Claw {
                return match weapon {
                    SkeletonWeapon::Axe => "Skeletal Raider",
                    SkeletonWeapon::Flail => "Skeletal Crusher",
                    SkeletonWeapon::Mace => "Skeletal Brute",
                    SkeletonWeapon::Pike => "Skeletal Lancer",
                    SkeletonWeapon::Sword => "Skeletal Knight",
                    SkeletonWeapon::Claw => unreachable!(),
                }
                .into();
            }
            let prefix = ["", "Armored ", "Horned ", "Hooded "][*headgear as usize];
            let burning = if enemy.burning { "Burning " } else { "" };
            let weapon = match weapon {
                SkeletonWeapon::Axe => " Axeman",
                SkeletonWeapon::Claw => "",
                SkeletonWeapon::Flail => " Flailman",
                SkeletonWeapon::Mace => " Maceman",
                SkeletonWeapon::Pike => " Pikeman",
                SkeletonWeapon::Sword => " Swordsman",
            };
            format!("{prefix}{burning}Skeleton{weapon}")
        }
        EnemyFamilyConfig::Archer { arrow_type, headgear, .. } => {
            let skeleton = ["Skeleton", "Armored Skeleton", "Horned Skeleton", "Hooded Skeleton"]
                [*headgear as usize];
            let element = match arrow_type {
                ArrowType::Normal => "",
                ArrowType::Fire => " Fire",
                ArrowType::Poison => " Poison",
            };
            format!("{skeleton}{element} Archer")
        }
        EnemyFamilyConfig::Mage { element, .. } => match element {
            MageElement::Fire => "Skeleton Firemage",
            MageElement::Frost => "Skeleton Frostmage",
            MageElement::Lightning => "Skeleton Stormcaller",
            MageElement::Poison => "Skeleton Poisoncaster",
        }
        .into(),
        EnemyFamilyConfig::Imp { .. } => "Imp".into(),
        EnemyFamilyConfig::Spider { .. } => "Spider".into(),
        EnemyFamilyConfig::Cocoon => "Cocoon".into(),
        EnemyFamilyConfig::Portal { .. } => "Deep Portal".into(),
        EnemyFamilyConfig::Zombie { rotten, .. } => {
            if *rotten { "Rotten Zombie" } else { "Zombie" }.into()
        }
        EnemyFamilyConfig::Wraith => "Wraith".into(),
        EnemyFamilyConfig::Demon { split_count } => {
            if *split_count < 1 { "Lesser Demon" } else { "Lesser Demon Legion" }.into()
        }
        EnemyFamilyConfig::Coffin { maggot_poison_damage, .. } => {
            if *maggot_poison_damage <= 0.0 { "Putrid Coffin" } else { "Tainted Coffin" }.into()
        }
    }
}
